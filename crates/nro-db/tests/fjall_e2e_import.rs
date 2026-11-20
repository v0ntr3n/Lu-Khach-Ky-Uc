#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used)]
//! End-to-end integration tests verifying streaming mysqldump ingestion, `GameTemplates` assembly,
//! case-insensitive `AccountRepo::login`, `sub_vnd` CAS, `PlayerRepo` secondary indexes,
//! `SuperRankRepo` reverse range scans, `GiftCodeRepo`, `HistoryTransactionRepo`, and `EventRepo`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, Utc};
use nro_core::{AccountId, Gender, MapId, ParityMode, PlayerId, TemplateId};
use nro_db::account_repo::AccountRepo;
use nro_db::engine::{FjallConfig, FjallEngine};
use nro_db::legacy_sql::{ImportOptions, import_mysqldump_str, verify_fjall_store};
use nro_db::player_repo::{LeaderboardIndexKind, NewPlayerSeed, PlayerRepo};
use nro_db::templates::GameTemplates;
use nro_db::world_repos::{
    ClanRepo, ConsignItemRecord, ConsignShopRepo, EventRepo, GiftCodeRepo, HistoryTransactionRepo,
    SuperRankInfo, SuperRankRecord, SuperRankRepo, TradeAuditEntry, WomensDayBuffs,
};
use smallvec::SmallVec;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(1);

struct TempFjallDir(PathBuf);

impl TempFjallDir {
    fn new() -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("nro_fjall_e2e_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp fjall dir");
        Self(dir)
    }
}

impl Drop for TempFjallDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SAMPLE_MYSQLDUMP: &str = r#"
-- phpMyAdmin SQL Dump (MariaDB 10.4.32 compatible fixture)
SET SQL_MODE = "NO_AUTO_VALUE_ON_ZERO";

INSERT INTO `item_template` (`id`, `TYPE`, `gender`, `NAME`, `description`, `level`, `icon_id`, `part`, `is_up_to_up`, `power_require`, `gold`, `gem`, `head`, `body`, `leg`) VALUES
(0, 0, 0, 'Áo vải 3 lỗ', 'Áo cơ bản', 0, 0, 0, 0, 0, 100, 0, -1, 0, -1),
(457, 27, 3, 'Thỏi vàng', 'Đổi vàng', 0, 4028, -1, 1, 0, 500000000, 0, -1, -1, -1),
(999, 23, 3, 'Thú cưỡi Rồng', 'Mount', 0, 5000, 12, 0, 0, 0, 100, -1, -1, -1);

INSERT INTO `img_by_name` (`id`, `NAME`, `n_frame`) VALUES
(1, 'mount_12_0', 4);

INSERT INTO `array_head_2_frames` (`id`, `data`) VALUES
(46, '[1705,1706,1707,]');

INSERT INTO `map_template` (`id`, `NAME`, `zones`, `max_player`, `data`, `type`, `planet_id`, `bg_type`, `tile_id`, `bg_id`, `waypoints`, `mobs`, `npcs`, `is_map_double`) VALUES
(0, 'Làng Aru', 3, 15, '[]', 0, 0, 0, 1, 0, '["[\\"Đồi hoa cúc\\",1224,408,1248,432,0,0,1,60,384]"]', '[]', '[]', 0),
(21, 'Nhà Gôhan', 1, 15, '[]', 0, 0, 0, 1, 0, '[]', '[]', '[]', 0);

INSERT INTO `shop` (`id`, `npc_id`, `tag_name`, `type_shop`) VALUES
(10, 16, 'santa', 0);

INSERT INTO `tab_shop` (`id`, `shop_id`, `NAME`) VALUES
(41, 10, 'Cửa<>Hàng');

INSERT INTO `item_shop` (`id`, `tab_id`, `temp_id`, `is_new`, `is_sell`, `type_sell`, `cost`, `icon_spec`, `create_time`) VALUES
(501, 10, 457, 1, 1, 1, 50, 0, '2026-05-01 10:00:00');

INSERT INTO `item_shop_option` (`id`, `item_shop_id`, `option_id`, `param`) VALUES
(1, 501, 30, 0);

INSERT INTO `task_main_template` (`id`, `NAME`, `detail`) VALUES
(0, 'Nhiệm vụ mở đầu', 'Gặp Ông Gôhan');

INSERT INTO `task_sub_template` (`task_main_id`, `NAME`, `max_count`, `notify`, `npc_id`, `map`, `ducvupro`) VALUES
(0, 'Nói chuyện', 1, 'Về nhà', 0, 21, 1);

INSERT INTO `account` (`id`, `username`, `password`, `email`, `create_time`, `update_time`, `ban`, `is_admin`, `last_time_login`, `last_time_logout`, `ip_address`, `active`, `thoi_vang`, `server_login`, `bd_player`, `is_gift_box`, `gift_time`, `reward`, `vnd`, `tongnap`, `token`, `xsrf_token`, `newpass`, `luotquay`, `vang`, `event_point`, `vip`, `tichdiem`, `point_post`, `last_post`, `gioithieu`, `xacnhan_gioitheu`, `baiviet`, `xacminh`, `admin`) VALUES
(3728, 'AdminUser', 'secret123', '', '2025-09-05 13:38:39', '2025-09-05 13:38:39', 0, 1, '2026-05-22 09:23:04', '2026-05-22 09:23:15', '127.0.0.1', 1, 10, -1, 1.0, 0, NULL, NULL, 50000, 50000, '', '', '', 0, 0, 0, 1, 0, 0, 0, NULL, 0, 0, 0, 1),
(3729, 'FreshUser', 'pass456', '', '2025-09-05 13:38:39', '2025-09-05 13:38:39', 0, 0, '2026-05-22 09:23:04', '2026-05-22 09:23:15', '127.0.0.1', 0, 0, -1, 1.0, 0, NULL, NULL, 15000, 15000, '', '', '', 0, 0, 0, 0, 0, 0, 0, NULL, 0, 0, 0, 0);

INSERT INTO `player` (`id`, `account_id`, `name`, `head`, `gender`, `have_tennis_space_ship`, `clan_id`, `data_inventory`, `data_location`, `data_point`, `data_magic_tree`, `items_body`, `items_bag`, `items_box`, `items_box_lucky_round`, `items_daban`, `friends`, `enemies`, `data_intrinsic`, `data_item_time`, `data_task`, `data_mabu_egg`, `data_charm`, `skills`, `skills_shortcut`, `pet`, `data_black_ball`, `data_side_task`, `create_time`, `notify`, `baovetaikhoan`, `captcha`, `data_card`, `lasttimepkcommeson`, `bandokhobau`, `doanhtrai`, `conduongrandoc`, `masterDoesNotAttack`, `nhanthoivang`, `ruonggo`, `sieuthanthuy`, `vodaisinhtu`, `rongxuong`, `data_item_event`, `data_luyentap`, `data_clan_task`, `data_vip`, `rank`, `data_achievement`, `giftcode`, `event_point`, `data_event`, `dataBadges`, `dataTaskBadges`, `firstTimeLogin`, `BoughtSkill`, `LearnSkill`, `dailyGift`, `point_sukien`, `thachdauwhis`, `point_sukien1`, `point_maydam`, `total_damage_maydam`, `data_duahau_egg`, `checkNhanQua`, `thanhTichBang`, `thanhTichKhiGas`, `thanhTichCDRD`, `nhiem_vu_kol`, `point_sukien2`) VALUES
(1367, 3728, 'admin', 383, 0, 1, 0, '[5000000,250,10,0,5]', '[51,400,300]', '[0,60000007968,600007968,1000,1000,200000000,100000000,1000000000,0,0,0,0,200000030,100000000]', '[5,10,0,1757000000000,1757000000000]', '[]', '[]', '[]', '[]', '[]', '[]', '[]', '[]', '[]', '[12,1,5,1757000000000]', '[]', '[0,0,0,0,0,0,0,0,0,0]', '[]', '[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1]', '[]', '[]', '[]', '2025-09-05 14:00:00', NULL, '[]', '', '[]', 0, '[]', 0, '[]', '1', '[]', '[]', '[]', '[]', 0, '[]', '[]', '[]', '[]', 1, '[]', '[]', 0, '[]', '[]', '[]', '2025-09-05 14:00:00', '[]', '[]', '[]', 0, 0, 777, 0, 0, '[]', '[1,1]', '[0,0,0,0]', '[0,0,0,0]', '[0,0,0,0]', '', 888);

INSERT INTO `clan` (`id`, `NAME`, `NAME_2`, `slogan`, `img_id`, `power_point`, `max_member`, `clan_point`, `LEVEL`, `members`, `tops`, `create_time`, `thanhTichBDKB`, `thongTinLeader`) VALUES
(0, 'RedRibbon', '', 'Go!', 20, 60000007968, 10, 5, 2, '["{\\"role\\":0,\\"receive_donate\\":0,\\"member_point\\":0,\\"body\\":1859,\\"join_time\\":1757157060,\\"leg\\":1860,\\"head\\":1866,\\"ask_pea_time\\":0,\\"name\\":\\"admin\\",\\"clan_point\\":0,\\"id\\":1367,\\"donate\\":0,\\"power\\":60000007968}"]', '[]', '2025-09-06 11:11:00', '[110,45000]', '[1367,admin,383,384,385]');

INSERT INTO `super_rank` (`id`, `player_id`, `name`, `rank`, `last_pk_time`, `last_reward_time`, `ticket`, `win`, `lose`, `history`, `info`) VALUES
(1, 1367, 'admin', 1, 0, 0, 3, 10, 0, '[]', '{"head":383,"body":384,"leg":385,"hp":200000,"dame":50000,"def":1000}'),
(2, 2001, 'rival_a', 5, 0, 0, 3, 4, 2, '[]', '{"head":64,"body":65,"leg":66,"hp":100000,"dame":20000,"def":500}'),
(3, 2002, 'rival_b', 18, 0, 0, 3, 1, 5, '[]', '{"head":9,"body":10,"leg":11,"hp":80000,"dame":15000,"def":300}');

INSERT INTO `giftcode` (`id`, `code`, `count_left`, `detail`, `datecreate`, `expired`) VALUES
(1, 'tanthu', 2, '[{"id":457,"quantity":50,"options":[{"id":30,"param":0}]}]', '2025-06-08 13:02:44', '2030-01-01 06:12:53');

INSERT INTO `posts` (`id`, `tieude`, `noidung`, `username`, `created_at`, `theloai`, `ghimbai`, `image`, `trangthai`, `tinhtrang`, `like`) VALUES
(1, 'Chào mừng', 'Máy chủ mở cửa', 'admin', '2025-09-01 00:00:00', 1, 1, NULL, 1, 1, 99);
"#;

#[tokio::test]
async fn full_mysqldump_import_and_repository_lifecycle() {
    let tmp = TempFjallDir::new();
    let mut cfg = FjallConfig::new(&tmp.0);
    cfg.strict_schema_check = false;
    let engine = FjallEngine::open(&cfg).await.expect("open fjall");

    let manifest = import_mysqldump_str(&engine, SAMPLE_MYSQLDUMP, ImportOptions::default())
        .expect("import sample mysqldump");
    assert_eq!(manifest.total_rows, 22);

    let report = verify_fjall_store(&engine).await.expect("verify store");
    assert!(engine.check_schema_health().await.unwrap().is_healthy());
    assert_eq!(report.item_templates, 3);
    assert_eq!(report.map_templates, 2);
    assert_eq!(report.shop_templates, 1);
    assert_eq!(report.super_rank_highest, 18);

    let tpl = GameTemplates::load_all(&engine).await.expect("load templates");
    assert_eq!(tpl.arr_head_2_frames[0], vec![1705, 1706, 1707]);
    assert_eq!(tpl.shops[0].tabs[0].name, "Cửa\nHàng");
    assert_eq!(tpl.shops[0].tabs[0].items.len(), 1);
    assert_eq!(tpl.mount_map.get(&TemplateId(999)), Some(&30_012));

    let accounts = AccountRepo::new(engine.clone());
    let now = Utc::now();
    let auth = accounts
        .login("adminuser", "secret123", "10.0.0.8", 0, now)
        .await
        .expect("case-insensitive login succeeds");
    assert_eq!(auth.user_id, AccountId(3728));
    assert!(auth.password_upgraded);
    assert!(accounts.sub_vnd(AccountId(3728), 30_000).await.unwrap());
    assert!(!accounts.sub_vnd(AccountId(3728), 30_000).await.unwrap());

    let players = PlayerRepo::new(engine.clone());
    let offline = players.load_player(PlayerId(1367), ParityMode::Fixed, true).await.unwrap();
    assert_eq!(offline.location.map_id, MapId(51));
    let online = players.load_by_account(AccountId(3728), ParityMode::Fixed).await.unwrap().unwrap();
    assert_eq!(online.location.map_id, MapId(21));

    let dup_err = players
        .create_player(&NewPlayerSeed {
            account_id: AccountId(3729),
            name: String::from("ADMIN"),
            gender: Gender::Namec,
            hair: 9,
            test_mode: false,
            now_ms: now.timestamp_millis(),
        })
        .await;
    assert!(dup_err.is_err());

    let new_pid = players
        .create_player(&NewPlayerSeed {
            account_id: AccountId(3729),
            name: String::from("Piccolo"),
            gender: Gender::Namec,
            hair: 9,
            test_mode: false,
            now_ms: now.timestamp_millis(),
        })
        .await
        .expect("unique character created");
    assert_eq!(new_pid, PlayerId(1368));

    let mut updated = online;
    updated.stats.power = 99_000_000_000;
    updated.point_sukien1 = 1_500;
    updated.point_sukien2 = 42;
    let sr_update = SuperRankRecord {
        player_id: PlayerId(1367),
        name: String::from("admin"),
        rank: 3,
        last_pk_time_ms: 100,
        last_reward_time_ms: 100,
        ticket: 2,
        win: 11,
        lose: 0,
        history: Vec::new(),
        info: SuperRankInfo::default(),
    };
    players.save_player(&updated, Some(&sr_update), ParityMode::Fixed, now.timestamp_millis()).await.unwrap();

    let top_power = players.top_leaderboard(LeaderboardIndexKind::Power, 10).unwrap();
    assert_eq!(top_power.len(), 2);
    assert_eq!((top_power[0].player_id, top_power[0].score), (PlayerId(1367), 99_000_000_000));
    let top_sk1 = players.top_leaderboard(LeaderboardIndexKind::SuKien1, 10).unwrap();
    let top_sk2 = players.top_leaderboard(LeaderboardIndexKind::SuKien2, 10).unwrap();
    assert_eq!((top_sk1[0].score, top_sk2[0].score), (1_500, 42));

    let sr_repo = SuperRankRepo::new(engine.clone());
    let arena_list = sr_repo.list_in_rank_range(10, 10).await.unwrap();
    let ranks: Vec<i32> = arena_list.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![5, 3]);

    let clans = ClanRepo::new(engine.clone());
    assert_eq!(clans.load_all().await.unwrap().len(), 1);
    assert_eq!(clans.next_clan_id().await.unwrap().0, 1);

    let consign = ConsignShopRepo::new(engine.clone());
    consign
        .save_all(&[ConsignItemRecord {
            id: 7, player_id: 1367, tab: 0, item_id: TemplateId(457),
            gold: 500, gem: -1, quantity: 10, options: SmallVec::new(), is_up_top: 1, is_buy: false,
        }])
        .await
        .unwrap();
    assert_eq!(consign.load_all().await.unwrap().len(), 1);

    let gifts = GiftCodeRepo::new(engine.clone());
    assert!(gifts.redeem_decrement(1).await.unwrap());
    assert!(gifts.redeem_decrement(1).await.unwrap());
    assert!(!gifts.redeem_decrement(1).await.unwrap());

    let hist = HistoryTransactionRepo::new(engine.clone());
    hist.insert(&TradeAuditEntry {
        player_1: "admin", player_2: "Piccolo", item_player_1: "457x1", item_player_2: "0",
        bag_1_before: "[]", bag_2_before: "[]", bag_1_after: "[]", bag_2_after: "[]",
        time_tran: now - Duration::days(10),
    }).await.unwrap();
    hist.insert(&TradeAuditEntry {
        player_1: "admin", player_2: "Piccolo", item_player_1: "457x2", item_player_2: "0",
        bag_1_before: "[]", bag_2_before: "[]", bag_1_after: "[]", bag_2_after: "[]",
        time_tran: now,
    }).await.unwrap();
    assert_eq!(hist.delete_older_than_days(now, 7).await.unwrap(), 1);

    let events = EventRepo::new(engine.clone());
    let buffs = WomensDayBuffs { dame_percent_ms: 3_600_000, ..Default::default() };
    events.save_womens_day(&buffs).await.unwrap();
    assert_eq!(events.load_womens_day().await.unwrap(), buffs);

    engine.close().await.unwrap();
}

#[tokio::test]
async fn imports_unescaped_legacy_waypoints_mobs_npcs_and_skills_from_real_dumps() {
    let legacy_dump = r#"
INSERT INTO `map_template` (`id`, `NAME`, `zones`, `max_player`, `data`, `type`, `planet_id`, `bg_type`, `tile_id`, `bg_id`, `waypoints`, `mobs`, `npcs`, `is_map_double`) VALUES
(0, 'Làng Aru', 10, 15, '[0,0,0,1,0]', 0, 0, 0, 1, 0, '[\"[\"Đồi hoa cúc\",1224,408,1248,432,0,0,1,60,384]\",\"[\"Nhà Gôhan\",288,408,360,432,1,1,21,489,336]\"]', '[\"[0,1,100,780,432]\"]', '[[7,228,432]]', 0),
(122, 'Ngũ Hành Sơn', 10, 15, '[]', 0, 0, 0, 1, 0, '[]', '[]', '[[6,517,233,0]]', 0),
(136, 'Hang Bạch Tuộc', 10, 15, '[]', 4, 0, 0, 28, 0, '[]', '[\"[37,8,36252,1308,216],\r\n[71,0,40732000,949,576]\"]', '[]', 0);

INSERT INTO `skill_template` (`nclass_id`, `id`, `NAME`, `max_point`, `mana_use_type`, `type`, `icon_id`, `dam_info`, `slot`, `skills`) VALUES
(0, 0, 'Đấm Dragon', 7, 0, 1, 539, '100%', 0, '[\"{\"power_require\":1000,\"damage\":100,\"dx\":32,\"dy\":18,\"price\":0,\"max_fight\":1,\"mana_use\":1,\"cool_down\":500,\"id\":0,\"point\":1,\"info\":\"tại ông nội\"}\",\"{\"power_require\":2000,\"damage\":110,\"dx\":32,\"dy\":18,\"price\":0,\"max_fight\":1,\"mana_use\":2,\"cool_down\":500,\"id\":1,\"point\":2,\"info\":\"cấp 2\"}\"]');
"#;

    let tmp = TempFjallDir::new();
    let mut cfg = FjallConfig::new(&tmp.0);
    cfg.strict_schema_check = false;
    let engine = FjallEngine::open(&cfg).await.expect("open fjall");

    let manifest = import_mysqldump_str(&engine, legacy_dump, ImportOptions::default())
        .expect("import unescaped legacy dump quirks");
    assert_eq!(manifest.total_rows, 4);

    let tpl = GameTemplates::load_all(&engine).await.expect("load templates");
    assert_eq!(tpl.maps.len(), 3);
    assert_eq!(tpl.maps[0].waypoints.len(), 2);
    assert_eq!(tpl.maps[0].waypoints[0].0, "Đồi hoa cúc");
    assert_eq!(tpl.maps[1].npcs, vec![(6, 517, 233)]);
    assert_eq!(tpl.maps[2].mobs.len(), 2);
    assert_eq!(tpl.skills.len(), 1);
    assert_eq!(tpl.skills[0].levels.len(), 2);
    engine.close().await.unwrap();
}
