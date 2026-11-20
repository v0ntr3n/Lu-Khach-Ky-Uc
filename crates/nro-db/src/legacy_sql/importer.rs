//! Orchestrates streaming ingestion of all 41 `database team2026.sql` tables into [`FjallEngine`].

use std::collections::BTreeMap;

use chrono::Utc;
use nro_core::{ClanId, ItemOption, TemplateId};
use tracing::info;

use crate::account_repo::{AccountRepo, AccountRow, verify_or_upgrade_password_sync};
use crate::codec::{enc_history_tx_key, enc_i32, enc_i64, enc_super_rank_key, encode_val};
use crate::engine::{FjallEngine, IMPORT_MANIFEST_KEY, WOMENS_DAY_EVENT_KEY};
use crate::legacy_sql::dump_parser::{SqlRowMap, stream_mysqldump_inserts};
use crate::legacy_sql::template_builder::TemplateCollector;
use crate::legacy_sql::{ImportManifest, ImportOptions};
use crate::player_repo::{PlayerRepo, PlayerRow, PlayerSnapshot};
use crate::world_repos::{
    ClanMemberRecord, ClanRecord, ConsignItemRecord, ConsignOptWire, GiftCodeRecord,
    GiftCodeRepo, StoredTradeAudit, SuperRankRepo, WomensDayBuffs,
};
use crate::{DbResult, JsonStr, UnquotedLeaderInfo, parse_lenient_json_vec};

pub fn import_mysqldump_str(
    engine: &FjallEngine,
    sql: &str,
    opts: ImportOptions,
) -> DbResult<ImportManifest> {
    let now = Utc::now();
    let accounts_repo = AccountRepo::new(engine.clone());
    let players_repo = PlayerRepo::new(engine.clone());
    let giftcodes_repo = GiftCodeRepo::new(engine.clone());

    let mut tpl_collector = TemplateCollector::default();
    let mut table_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut sr_batch = engine.ks().db.batch();
    let mut sr_staged = 0_usize;
    let mut max_acc_id = 0_i32;
    let mut max_plr_id = 0_i64;
    let mut max_clan_id = -1_i32;

    let total_rows = stream_mysqldump_inserts(sql, |table, r| {
        let count_entry = table_counts.entry(table.to_owned()).or_insert(0);
        *count_entry += 1;

        if tpl_collector.try_ingest(table, &r)? {
            return Ok(());
        }

        match table {
            "account" => {
                let mut row = decode_account_row(&r, now);
                if opts.upgrade_passwords
                    && !row.password.starts_with("$argon2")
                    && let Ok(Some(phc)) = verify_or_upgrade_password_sync(&row.password, &row.password)
                {
                    row.password = phc;
                }
                max_acc_id = max_acc_id.max(row.id);
                accounts_repo.put_account(&row)?;
            }
            "player" => {
                let prow = decode_player_row(&r, now);
                max_plr_id = max_plr_id.max(i64::from(prow.id));
                let snap = PlayerSnapshot::from_row(prow, opts.parity_mode, true)?;
                players_repo.upsert_imported_player(&snap)?;
            }
            "clan" => {
                let id = r.get("id").i32_or(0);
                max_clan_id = max_clan_id.max(id);
                let members = parse_lenient_json_vec::<JsonStr<ClanMemberRecord>>(
                    "members",
                    r.get("members").as_str_or_empty(),
                )?
                .into_iter()
                .map(|m| m.0)
                .collect();
                let (bdkb_level_done, bdkb_duration_ms) =
                    serde_json::from_str::<(i32, i64)>(r.get("thanhtichbdkb").as_str_or_empty())
                        .unwrap_or((0, 0));
                let clan = ClanRecord {
                    id: ClanId(id),
                    name: r.get("name").as_str_or_empty().to_owned(),
                    name_2: r.get("name_2").as_str_or_empty().to_owned(),
                    slogan: r.get("slogan").as_str_or_empty().to_owned(),
                    img_id: r.get("img_id").i32_or(0) as i8,
                    power_point: r.get("power_point").i64_or(0),
                    max_member: r.get("max_member").i32_or(10) as i8,
                    clan_point: r.get("clan_point").i32_or(0),
                    level: r.get("level").i32_or(1).max(1) as i8,
                    members,
                    create_time: r.get("create_time").datetime_utc(now),
                    bdkb_level_done,
                    bdkb_duration_ms,
                    leader_info: UnquotedLeaderInfo::from_wire_str(
                        r.get("thongtinleader").as_str_or_empty(),
                    ),
                };
                engine.ks().clans.insert(&enc_i32(id)[..], encode_val("clan", &clan)?)?;
            }
            "shop_ky_gui" => {
                let wire_opts: Vec<ConsignOptWire> =
                    parse_lenient_json_vec("itemOption", r.get("itemoption").as_str_or_empty())?;
                let options = wire_opts
                    .into_iter()
                    .map(|o| ItemOption { id: o.id.0 as u16, param: o.param.0 })
                    .collect();
                let item = ConsignItemRecord {
                    id: r.get("id").i32_or(0),
                    player_id: r.get("player_id").i32_or(0),
                    tab: r.get("tab").i32_or(0) as i8,
                    item_id: TemplateId(r.get("item_id").i32_or(0) as i16),
                    gold: r.get("gold").i32_or(0),
                    gem: r.get("gem").i32_or(0),
                    quantity: r.get("quantity").i32_or(1),
                    options,
                    is_up_top: r.get("isuptop").i32_or(0) as i8,
                    is_buy: r.get("isbuy").i32_or(0) == 1,
                };
                engine
                    .ks()
                    .consign_shop
                    .insert(&enc_i32(item.id)[..], encode_val("consign_shop", &item)?)?;
            }
            "super_rank" => {
                let sr = SuperRankRepo::decode_sql_fields(
                    r.get("player_id").i32_or(0),
                    r.get("name").as_str_or_empty().to_owned(),
                    r.get("rank").i32_or(0),
                    r.get("last_pk_time").i64_or(0),
                    r.get("last_reward_time").i64_or(0),
                    r.get("ticket").i32_or(0),
                    r.get("win").i32_or(0),
                    r.get("lose").i32_or(0),
                    r.get("history").as_str_or_empty(),
                    r.get("info").as_str_or_empty(),
                )?;
                sr_batch.insert(
                    &engine.ks().super_rank,
                    &enc_i64(sr.player_id.0)[..],
                    encode_val("super_rank", &sr)?,
                );
                if sr.rank > 0 {
                    sr_batch.insert(
                        &engine.ks().idx_super_rank,
                        &enc_super_rank_key(sr.rank, sr.player_id.0)[..],
                        &[][..],
                    );
                }
                sr_staged += 1;
                if sr_staged >= 512 {
                    std::mem::replace(&mut sr_batch, engine.ks().db.batch()).commit()?;
                    sr_staged = 0;
                }
            }
            "giftcode" => {
                let rec = GiftCodeRecord {
                    id: r.get("id").i32_or(0),
                    code: r.get("code").as_str_or_empty().to_owned(),
                    count_left: r.get("count_left").i32_or(0),
                    items: parse_lenient_json_vec("detail", r.get("detail").as_str_or_empty())?,
                    date_created: r.get("datecreate").datetime_utc(now),
                    expired: r.get("expired").datetime_utc(now),
                };
                giftcodes_repo.insert_raw(&rec)?;
            }
            "history_transaction" => {
                let ts_ms = r.get("time_tran").datetime_utc(now).timestamp_millis();
                let seq = engine.alloc_tx_seq();
                let audit = StoredTradeAudit {
                    player_1: r.get("player_1").as_str_or_empty().to_owned(),
                    player_2: r.get("player_2").as_str_or_empty().to_owned(),
                    item_player_1: r.get("item_player_1").as_str_or_empty().to_owned(),
                    item_player_2: r.get("item_player_2").as_str_or_empty().to_owned(),
                    bag_1_before: r.get("bag_1_before_tran").as_str_or_empty().to_owned(),
                    bag_2_before: r.get("bag_2_before_tran").as_str_or_empty().to_owned(),
                    bag_1_after: r.get("bag_1_after_tran").as_str_or_empty().to_owned(),
                    bag_2_after: r.get("bag_2_after_tran").as_str_or_empty().to_owned(),
                    time_tran_ms: ts_ms,
                };
                engine
                    .ks()
                    .history_tx
                    .insert(&enc_history_tx_key(ts_ms, seq)[..], encode_val("history_tx", &audit)?)?;
            }
            other => {
                let row_seq = *table_counts.get(other).unwrap_or(&1);
                let archive_key = format!("{other}:{row_seq:010}");
                let pairs = r.into_pairs();
                engine
                    .ks()
                    .portal_archive
                    .insert(archive_key.into_bytes(), encode_val("portal_archive", &pairs)?)?;
            }
        }
        Ok(())
    })?;

    if sr_staged > 0 {
        sr_batch.commit()?;
    }

    let templates = tpl_collector.build();
    templates.save_to_engine(engine)?;

    if !engine.ks().events.contains_key(WOMENS_DAY_EVENT_KEY)? {
        engine
            .ks()
            .events
            .insert(WOMENS_DAY_EVENT_KEY, encode_val("event", &WomensDayBuffs::default())?)?;
    }

    engine.bump_sequences(
        max_acc_id.saturating_add(1),
        max_plr_id.saturating_add(1),
        max_clan_id.saturating_add(1),
    );

    let manifest = ImportManifest {
        source_bytes: sql.len() as u64,
        total_rows,
        table_counts,
        imported_at_utc: now,
    };
    engine
        .ks()
        .meta
        .insert(IMPORT_MANIFEST_KEY, encode_val("manifest", &manifest)?)?;
    engine.persist_sync()?;

    info!(
        total_rows = manifest.total_rows,
        tables = manifest.table_counts.len(),
        items = templates.items.len(),
        maps = templates.maps.len(),
        "completed streaming SQL import into embedded fjall"
    );
    Ok(manifest)
}

fn decode_account_row(r: &SqlRowMap<'_>, now: chrono::DateTime<Utc>) -> AccountRow {
    AccountRow {
        id: r.get("id").i32_or(0),
        username: r.get("username").as_str_or_empty().to_owned(),
        password: r.get("password").as_str_or_empty().to_owned(),
        email: r.get("email").as_str_or_empty().to_owned(),
        create_time: r.get("create_time").opt_datetime_utc(),
        update_time: r.get("update_time").opt_datetime_utc(),
        ban: r.get("ban").bool_or(false),
        is_admin: r.get("is_admin").bool_or(false),
        last_time_login: r.get("last_time_login").datetime_utc(now),
        last_time_logout: r.get("last_time_logout").datetime_utc(now),
        ip_address: r.get("ip_address").opt_string(),
        active: r.get("active").i32_or(0),
        thoi_vang: r.get("thoi_vang").i32_or(0),
        server_login: r.get("server_login").i32_or(-1),
        bd_player: r.get("bd_player").f64_opt(),
        is_gift_box: r.get("is_gift_box").bool_opt(),
        gift_time: r.get("gift_time").opt_string(),
        reward: r.get("reward").opt_string(),
        vnd: r.get("vnd").i32_or(0),
        tongnap: r.get("tongnap").i32_or(0),
        token: r.get("token").as_str_or_empty().to_owned(),
        xsrf_token: r.get("xsrf_token").as_str_or_empty().to_owned(),
        newpass: r.get("newpass").as_str_or_empty().to_owned(),
        luotquay: r.get("luotquay").i32_or(0),
        vang: r.get("vang").i64_or(0),
        event_point: r.get("event_point").i32_or(0),
        vip: r.get("vip").i32_or(0),
        tichdiem: r.get("tichdiem").i32_or(0),
        point_post: r.get("point_post").i32_or(0),
        last_post: r.get("last_post").i32_or(0),
        gioithieu: r.get("gioithieu").i32_opt(),
        xacnhan_gioitheu: r.get("xacnhan_gioitheu").i32_or(0),
        baiviet: r.get("baiviet").i32_or(0),
        xacminh: r.get("xacminh").i32_or(0),
        admin: r.get("admin").i32_or(0),
    }
}

fn decode_player_row(r: &SqlRowMap<'_>, now: chrono::DateTime<Utc>) -> PlayerRow {
    let str_col = |name: &str, default: &str| {
        let s = r.get(name).as_str_or_empty();
        if s.is_empty() { default.to_owned() } else { s.to_owned() }
    };
    PlayerRow {
        id: r.get("id").i32_or(0),
        account_id: r.get("account_id").i32_opt(),
        name: str_col("name", ""),
        head: r.get("head").i32_or(-1),
        gender: r.get("gender").i32_or(0),
        have_tennis_space_ship: r.get("have_tennis_space_ship").bool_opt(),
        clan_id: r.get("clan_id").i32_or(-1),
        data_inventory: str_col("data_inventory", "[0,0,0,0,0]"),
        data_location: str_col("data_location", "[21,300,336]"),
        data_point: str_col("data_point", "[0,2000,2000,1000,1000,100,100,10,0,0,0,0,100,100]"),
        data_magic_tree: str_col("data_magic_tree", "[1,5,0,0,0]"),
        items_body: str_col("items_body", "[]"),
        items_bag: str_col("items_bag", "[]"),
        items_box: str_col("items_box", "[]"),
        items_box_lucky_round: str_col("items_box_lucky_round", "[]"),
        items_daban: str_col("items_daban", "[]"),
        friends: str_col("friends", "[]"),
        enemies: str_col("enemies", "[]"),
        data_intrinsic: str_col("data_intrinsic", "[]"),
        data_item_time: str_col("data_item_time", "[]"),
        data_task: str_col("data_task", "[0,0,0,0]"),
        data_mabu_egg: str_col("data_mabu_egg", "[]"),
        data_charm: str_col("data_charm", "[0,0,0,0,0,0,0,0,0,0]"),
        skills: str_col("skills", "[]"),
        skills_shortcut: str_col("skills_shortcut", "[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1]"),
        pet: str_col("pet", "[]"),
        data_black_ball: str_col("data_black_ball", "[]"),
        data_side_task: str_col("data_side_task", "[]"),
        create_time: r.get("create_time").datetime_utc(now),
        notify: r.get("notify").opt_string(),
        baovetaikhoan: str_col("baovetaikhoan", "[]"),
        captcha: str_col("captcha", ""),
        data_card: str_col("data_card", "[]"),
        lasttimepkcommeson: r.get("lasttimepkcommeson").i64_or(0),
        bandokhobau: str_col("bandokhobau", "[]"),
        doanhtrai: r.get("doanhtrai").i64_or(0),
        conduongrandoc: str_col("conduongrandoc", "[]"),
        master_does_not_attack: str_col("masterdoesnotattack", "0"),
        nhanthoivang: str_col("nhanthoivang", "[]"),
        ruonggo: str_col("ruonggo", "[]"),
        sieuthanthuy: str_col("sieuthanthuy", "[]"),
        vodaisinhtu: str_col("vodaisinhtu", "[]"),
        rongxuong: r.get("rongxuong").i64_or(0),
        data_item_event: str_col("data_item_event", "[]"),
        data_luyentap: str_col("data_luyentap", "[]"),
        data_clan_task: str_col("data_clan_task", "[]"),
        data_vip: r.get("data_vip").opt_string(),
        rank: r.get("rank").i32_or(0),
        data_achievement: str_col("data_achievement", "[]"),
        giftcode: str_col("giftcode", "[]"),
        event_point: r.get("event_point").i32_or(0),
        data_event: r.get("data_event").opt_string(),
        data_badges: r.get("databadges").opt_string(),
        data_task_badges: r.get("datataskbadges").opt_string(),
        first_time_login: r.get("firsttimelogin").datetime_utc(now),
        bought_skill: r.get("boughtskill").opt_string(),
        learn_skill: r.get("learnskill").opt_string(),
        daily_gift: str_col("dailygift", "[]"),
        point_sukien: r.get("point_sukien").i32_or(0),
        thachdauwhis: r.get("thachdauwhis").i32_or(0),
        point_sukien1: r.get("point_sukien1").i32_or(0),
        point_maydam: r.get("point_maydam").i32_or(0),
        total_damage_maydam: r.get("total_damage_maydam").i64_or(0),
        data_duahau_egg: str_col("data_duahau_egg", "[]"),
        check_nhan_qua: str_col("checknhanqua", "[1,1]"),
        thanh_tich_bang: str_col("thanhtichbang", "[0,0,0,0]"),
        thanh_tich_khi_gas: str_col("thanhtichkhigas", "[0,0,0,0]"),
        thanh_tich_cdrd: str_col("thanhtichcdrd", "[0,0,0,0]"),
        nhiem_vu_kol: str_col("nhiem_vu_kol", ""),
        point_sukien2: r.get("point_sukien2").i32_or(0),
    }
}
