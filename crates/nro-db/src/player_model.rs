//! Player domain snapshot (`PlayerSnapshot`), seed (`NewPlayerSeed`), and 70-column SQL row
//! (`PlayerRow`) definitions shared between `player_repo` and the legacy SQL dump importer.

use chrono::{DateTime, Utc};
use nro_core::{
    AccountId, ClanId, CoreStats, Gender, ItemInstance, Location, ParityMode, PlayerId,
};
use serde::{Deserialize, Serialize};
#[path = "player_model_seed.rs"]
mod player_model_seed;
pub use player_model_seed::NewPlayerSeed;

use crate::json_columns::*;
use crate::{
    DbResult, PositionalArray, UnquotedDungeonRecord, gender_from_sql, gender_to_sql,
    parse_lenient_json_vec,
};

/// Exact 70-column row mapping for `player` (`database team2026.sql`) used by the SQL dump importer.
#[derive(Debug, Clone)]
pub struct PlayerRow {
    pub id: i32,
    pub account_id: Option<i32>,
    pub name: String,
    pub head: i32,
    pub gender: i32,
    pub have_tennis_space_ship: Option<bool>,
    pub clan_id: i32,
    pub data_inventory: String,
    pub data_location: String,
    pub data_point: String,
    pub data_magic_tree: String,
    pub items_body: String,
    pub items_bag: String,
    pub items_box: String,
    pub items_box_lucky_round: String,
    pub items_daban: String,
    pub friends: String,
    pub enemies: String,
    pub data_intrinsic: String,
    pub data_item_time: String,
    pub data_task: String,
    pub data_mabu_egg: String,
    pub data_charm: String,
    pub skills: String,
    pub skills_shortcut: String,
    pub pet: String,
    pub data_black_ball: String,
    pub data_side_task: String,
    pub create_time: DateTime<Utc>,
    pub notify: Option<String>,
    pub baovetaikhoan: String,
    pub captcha: String,
    pub data_card: String,
    pub lasttimepkcommeson: i64,
    pub bandokhobau: String,
    pub doanhtrai: i64,
    pub conduongrandoc: String,
    pub master_does_not_attack: String,
    pub nhanthoivang: String,
    pub ruonggo: String,
    pub sieuthanthuy: String,
    pub vodaisinhtu: String,
    pub rongxuong: i64,
    pub data_item_event: String,
    pub data_luyentap: String,
    pub data_clan_task: String,
    pub data_vip: Option<String>,
    pub rank: i32,
    pub data_achievement: String,
    pub giftcode: String,
    pub event_point: i32,
    pub data_event: Option<String>,
    pub data_badges: Option<String>,
    pub data_task_badges: Option<String>,
    pub first_time_login: DateTime<Utc>,
    pub bought_skill: Option<String>,
    pub learn_skill: Option<String>,
    pub daily_gift: String,
    pub point_sukien: i32,
    pub thachdauwhis: i32,
    pub point_sukien1: i32,
    pub point_maydam: i32,
    pub total_damage_maydam: i64,
    pub data_duahau_egg: String,
    pub check_nhan_qua: String,
    pub thanh_tich_bang: String,
    pub thanh_tich_khi_gas: String,
    pub thanh_tich_cdrd: String,
    pub nhiem_vu_kol: String,
    pub point_sukien2: i32,
}

/// Decoded, strongly-typed player aggregate stored directly via `postcard` in `players`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: PlayerId,
    pub account_id: Option<AccountId>,
    pub name: String,
    pub head: i16,
    pub gender: Gender,
    pub have_tennis_space_ship: bool,
    pub clan_id: ClanId,
    pub wallet: InventoryWallet,
    pub location: Location,
    pub stats: CoreStats,
    pub magic_tree: MagicTreeState,
    pub items_body: Vec<Option<ItemInstance>>,
    pub items_bag: Vec<Option<ItemInstance>>,
    pub items_box: Vec<Option<ItemInstance>>,
    pub items_box_lucky_round: Vec<Option<ItemInstance>>,
    pub items_daban: Vec<Option<ItemInstance>>,
    pub friends: Vec<SocialPeer>,
    pub enemies: Vec<SocialPeer>,
    pub intrinsic: IntrinsicState,
    pub item_time: ItemTimeState,
    pub main_task: MainTaskProgress,
    pub side_task: DailyTaskProgress,
    pub clan_task: DailyTaskProgress,
    pub mabu_egg: Option<EggTimer>,
    pub dua_hau_egg: Option<EggTimer>,
    pub charms: CharmTimers,
    pub skills: Vec<SkillSlot>,
    pub skills_shortcut: [i8; 10],
    pub pet: Option<PetSnapshot>,
    pub black_ball_rewards: [BlackBallSlot; 7],
    pub notify: Option<String>,
    pub mbv_pin: (i32, bool, i64),
    pub radar_cards: Vec<RadarCardRecord>,
    pub last_pk_commeson_ms: i64,
    pub bdkb: (i32, i64),
    pub last_join_doanh_trai_ms: i64,
    pub cdrd: (bool, i64, bool, bool),
    pub master_does_not_attack: bool,
    pub nhan_thoi_vang: (bool, i64),
    pub ruong_go: (i32, i64, i64, i64, i64),
    pub sieu_than_thuy: (bool, i64, bool),
    pub vo_dai_sinh_tu: (bool, i32, i64, i64),
    pub last_shenron_ms: i64,
    pub item_event_counters: [i64; 16],
    pub training: TrainingState,
    pub vip_state: (i32, i64, bool, bool, bool, i8, i64, i32),
    pub rank: i32,
    pub achievements: Vec<(i64, bool)>,
    pub redeemed_giftcodes: Vec<String>,
    pub event_point: i32,
    pub data_event: ([i32; 6], [bool; 4]),
    pub badges: Vec<PlayerBadgeRecord>,
    pub badge_tasks: Vec<BadgeTaskRecord>,
    pub first_time_login: DateTime<Utc>,
    pub bought_skills: Vec<i32>,
    pub learn_skill: (i64, i16, i32),
    pub daily_gifts: Vec<DailyGiftRecord>,
    pub point_sukien: i32,
    pub thachdauwhis: i32,
    pub point_sukien1: i32,
    pub point_sukien2: i32,
    pub point_maydam: i32,
    pub total_damage_maydam: i64,
    pub check_nhan_qua: (i32, i32, Option<String>),
    pub thanh_tich_bang: UnquotedDungeonRecord,
    pub thanh_tich_khi_gas: UnquotedDungeonRecord,
    pub thanh_tich_cdrd: UnquotedDungeonRecord,
    pub kol_progress: KolProgressData,
}

impl PlayerSnapshot {
    /// Decodes all 70 columns of [`PlayerRow`] during legacy SQL dump import.
    pub fn from_row(r: PlayerRow, mode: ParityMode, is_offline: bool) -> DbResult<Self> {
        let gender = gender_from_sql(r.gender)?;
        let g = gender_to_sql(gender);
        let head = if r.head == -1 {
            match g {
                0 => 64,
                1 => 9,
                _ => 6,
            }
        } else {
            r.head as i16
        };
        let raw_loc = parse_data_location(&r.data_location)?;
        let location = if is_offline {
            raw_loc
        } else {
            sanitize_login_location(raw_loc, gender)
        };

        let bvtk = PositionalArray::parse("baovetaikhoan", &r.baovetaikhoan).unwrap_or_default();
        let bdkb = PositionalArray::parse("bandokhobau", &r.bandokhobau).unwrap_or_default();
        let ntv = PositionalArray::parse("nhanthoivang", &r.nhanthoivang).unwrap_or_default();
        let rg = PositionalArray::parse("ruonggo", &r.ruonggo).unwrap_or_default();
        let stt = PositionalArray::parse("sieuthanthuy", &r.sieuthanthuy).unwrap_or_default();
        let vdst = PositionalArray::parse("vodaisinhtu", &r.vodaisinhtu).unwrap_or_default();
        let vip = PositionalArray::parse("data_vip", r.data_vip.as_deref().unwrap_or("[]"))
            .unwrap_or_default();
        let ev = PositionalArray::parse("data_event", r.data_event.as_deref().unwrap_or("[]"))
            .unwrap_or_default();
        let ls = PositionalArray::parse("LearnSkill", r.learn_skill.as_deref().unwrap_or("[]"))
            .unwrap_or_default();
        let cnq = PositionalArray::parse("checkNhanQua", &r.check_nhan_qua).unwrap_or_default();

        Ok(Self {
            id: PlayerId::from(r.id),
            account_id: r.account_id.map(AccountId),
            name: r.name,
            head,
            gender,
            have_tennis_space_ship: r.have_tennis_space_ship.unwrap_or(false),
            clan_id: ClanId(r.clan_id),
            wallet: parse_data_inventory(&r.data_inventory, mode)?,
            location,
            stats: parse_data_point(&r.data_point)?,
            magic_tree: parse_data_magic_tree(&r.data_magic_tree)?,
            items_body: parse_items_container("items_body", &r.items_body)?,
            items_bag: parse_items_container("items_bag", &r.items_bag)?,
            items_box: parse_items_container("items_box", &r.items_box)?,
            items_box_lucky_round: parse_items_container(
                "items_box_lucky_round",
                &r.items_box_lucky_round,
            )?,
            items_daban: parse_items_container("items_daban", &r.items_daban)?,
            friends: parse_social_list("friends", &r.friends)?,
            enemies: parse_social_list("enemies", &r.enemies)?,
            intrinsic: parse_data_intrinsic(&r.data_intrinsic)?,
            item_time: parse_data_item_time(&r.data_item_time, mode)?,
            main_task: parse_data_task(&r.data_task)?,
            side_task: parse_daily_task("data_side_task", &r.data_side_task)?,
            clan_task: parse_daily_task("data_clan_task", &r.data_clan_task)?,
            mabu_egg: parse_data_mabu_egg(&r.data_mabu_egg)?,
            dua_hau_egg: parse_data_dua_hau_egg(&r.data_duahau_egg)?,
            charms: parse_data_charm(&r.data_charm)?,
            skills: parse_skills(&r.skills)?,
            skills_shortcut: parse_skills_shortcut(&r.skills_shortcut)?,
            pet: parse_pet(&r.pet)?,
            black_ball_rewards: parse_data_black_ball(&r.data_black_ball)?,
            notify: r.notify.filter(|s| s != "null"),
            mbv_pin: (bvtk.i32_or(0, 0), bvtk.bool_or(1, false), bvtk.i64_or(2, 0)),
            radar_cards: parse_data_card(&r.data_card)?,
            last_pk_commeson_ms: r.lasttimepkcommeson,
            bdkb: (bdkb.i32_or(0, 0), bdkb.i64_or(1, 0)),
            last_join_doanh_trai_ms: r.doanhtrai,
            cdrd: parse_con_duong_ran_doc(&r.conduongrandoc, mode),
            master_does_not_attack: matches!(
                r.master_does_not_attack
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true"
            ),
            nhan_thoi_vang: (ntv.bool_or(0, false), ntv.i64_or(1, 0)),
            ruong_go: (
                rg.i32_or(0, 0),
                rg.i64_or(1, 50_000_000),
                rg.i64_or(2, 100),
                rg.i64_or(3, 0),
                rg.i64_or(4, 0),
            ),
            sieu_than_thuy: (
                stt.bool_or(0, false),
                stt.i64_or(1, 0),
                stt.bool_or(2, false),
            ),
            vo_dai_sinh_tu: (
                vdst.bool_or(0, false),
                vdst.i32_or(1, 0),
                vdst.i64_or(2, 0),
                vdst.i64_or(3, 0),
            ),
            last_shenron_ms: r.rongxuong,
            item_event_counters: parse_data_item_event(&r.data_item_event, mode),
            training: parse_data_luyentap(&r.data_luyentap),
            vip_state: (
                vip.i32_or(0, 0),
                vip.i64_or(1, 0),
                vip.bool_or(2, false),
                vip.bool_or(3, false),
                vip.bool_or(4, false),
                vip.i32_or(5, 0) as i8,
                vip.i64_or(6, 0),
                vip.i32_or(7, 0),
            ),
            rank: r.rank,
            achievements: parse_data_achievement(&r.data_achievement)?,
            redeemed_giftcodes: parse_lenient_json_vec("giftcode", &r.giftcode).unwrap_or_default(),
            event_point: r.event_point,
            data_event: (
                [
                    ev.i32_or(0, 0),
                    ev.i32_or(1, 0),
                    ev.i32_or(2, 0),
                    ev.i32_or(3, 0),
                    ev.i32_or(4, 0),
                    ev.i32_or(5, 0),
                ],
                [
                    ev.bool_or(6, false),
                    ev.bool_or(7, false),
                    ev.bool_or(8, false),
                    ev.bool_or(9, false),
                ],
            ),
            badges: parse_data_badges(r.data_badges.as_deref())?,
            badge_tasks: parse_data_task_badges(r.data_task_badges.as_deref())?,
            first_time_login: r.first_time_login,
            bought_skills: parse_lenient_json_vec(
                "BoughtSkill",
                r.bought_skill.as_deref().unwrap_or("[]"),
            )?,
            learn_skill: (ls.i64_or(0, -1), ls.i32_or(1, -1) as i16, ls.i32_or(2, 0)),
            daily_gifts: parse_daily_gift(&r.daily_gift)?,
            point_sukien: r.point_sukien,
            thachdauwhis: r.thachdauwhis,
            point_sukien1: r.point_sukien1,
            point_sukien2: r.point_sukien2,
            point_maydam: r.point_maydam,
            total_damage_maydam: r.total_damage_maydam,
            check_nhan_qua: (cnq.i32_or(0, 1), cnq.i32_or(1, 1), cnq.opt_string(2)),
            thanh_tich_bang: UnquotedDungeonRecord::from_wire_str(&r.thanh_tich_bang),
            thanh_tich_khi_gas: UnquotedDungeonRecord::from_wire_str(&r.thanh_tich_khi_gas),
            thanh_tich_cdrd: UnquotedDungeonRecord::from_wire_str(&r.thanh_tich_cdrd),
            kol_progress: parse_kol_progress(&r.nhiem_vu_kol)?,
        })
    }
}
