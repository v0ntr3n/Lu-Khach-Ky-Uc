//! Unit tests against real `database team2026.sql` payloads and `proptest` round-trip suites
//! covering both legacy stringified JSON codecs and `postcard` / signed-key binary codecs.

use nro_core::{
    AccountId, CoreStats, Gender, ItemInstance, ItemOption, Location, MapId, ParityMode, PlayerId,
    TemplateId, ZoneId,
};
use proptest::prelude::*;
use smallvec::SmallVec;

use crate::account_repo::{ACCOUNT_PASSWORD_MAX_LEN, verify_or_upgrade_password};
use crate::codec::{dec_i32, dec_i64, decode_val, enc_i32, enc_i64, encode_val};
use crate::json_columns::*;
use crate::player_repo::{NewPlayerSeed, PlayerSnapshot};
use crate::templates::MapWaypointRecord;
use crate::world_repos::{ClanMemberRecord, SuperRankHistoryItem};
use crate::{JsonStr, StrBool, StrNum, UnquotedLeaderInfo, parse_lenient_json_vec};

#[test]
fn real_team2026_player_1367_and_edge_cases_parse_cleanly() {
    let pt = parse_data_point(
        "[0,60000007968,600007968,1000,1000,200000000,100000000,1000000000,0,0,0,0,200000030,100000000]",
    ).expect("data_point");
    assert_eq!(pt.power, 60_000_007_968);
    assert_eq!(pt.hp, 200_000_030);

    let loc = parse_data_location("[185,997,672]").expect("data_location");
    assert_eq!((loc.map_id, loc.x, loc.y), (MapId(185), 997, 672));

    let raw_body = r#"["[0,1,\"[\\\"[47,2]\\\"]\",1757079582056]","[-1,0,\"[]\",1779441784043]"]"#;
    let body = parse_items_container("items_bag", raw_body).expect("triple-escaped items");
    assert_eq!(body.len(), 2);
    let ao = body[0].as_ref().expect("slot 0 occupied");
    assert_eq!((ao.template_id, ao.quantity, ao.options[0].id, ao.options[0].param), (TemplateId(0), 1, 47, 2));
    assert!(body[1].is_none());
    let reencoded = write_items_container("items_bag", &body, 1_779_441_784_043).expect("write");
    assert_eq!(parse_items_container("items_bag", &reencoded).expect("reparse"), body);

    let raw_pet = r#"["[0,2,\"$Đệ tử\",0,0,1]","[0,811400,1000,865,1000,2760,2300,101,45,0,2760,2292]","[\"[-1,0,\\\"[]\\\",1779441784045]\"]","[\"[0,1,0,0]\"]"]"#;
    let pet = parse_pet(raw_pet).expect("quadruple-escaped pet").expect("has pet");
    assert_eq!(pet.name, "$Đệ tử");
    assert_eq!(pet.items_body.len(), 7);
    assert_eq!(pet.skills.len(), 4);

    let frames: Vec<i16> = parse_lenient_json_vec("array_head_2_frames", "[1705,1706,1707,]")
        .expect("trailing comma tolerated");
    assert_eq!(frames, vec![1705, 1706, 1707]);

    let raw_clan_members = r#"["{\"role\":0,\"receive_donate\":0,\"member_point\":0,\"body\":1859,\"join_time\":1757157060,\"leg\":1860,\"head\":1866,\"ask_pea_time\":0,\"name\":\"admin\",\"clan_point\":0,\"id\":1367,\"donate\":0,\"power\":60000007968}"]"#;
    let members: Vec<JsonStr<ClanMemberRecord>> =
        parse_lenient_json_vec("members", raw_clan_members).expect("clan.members");
    assert_eq!(members[0].0.id, 1367);
    assert_eq!(members[0].0.power, 60_000_007_968);
    let leader = UnquotedLeaderInfo::from_wire_str("[1367,admin,383,384,385]");
    assert_eq!(leader.to_wire_string(), "[1367,admin,383,384,385]");

    let raw_hist = r#"["{\"event\":\"Thua erusthan[289]\",\"timestamp\":1749202673928}"]"#;
    let hist: Vec<JsonStr<SuperRankHistoryItem>> =
        parse_lenient_json_vec("history", raw_hist).expect("super_rank.history");
    assert_eq!(hist[0].0.timestamp, 1_749_202_673_928);

    let wp_row0 = r#"["[\"Đồi hoa cúc\",1224,408,1248,432,0,0,1,60,384]"]"#;
    let wp_row162 = r#"[["Bìa rừng nguyên thủy",0,264,24,288,0,0,161,1860,336]]"#;
    assert_eq!(
        parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", wp_row0).expect("row0")[0].0.7,
        MapId(1)
    );
    assert_eq!(
        parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", wp_row162).expect("row162")[0].0.7,
        MapId(161)
    );
}

#[test]
fn postcard_player_snapshot_with_strnum_and_strbool_roundtrips() {
    let seed = NewPlayerSeed {
        account_id: AccountId(3728),
        name: String::from("admin"),
        gender: Gender::Xayda,
        hair: -1,
        test_mode: true,
        now_ms: 1_757_157_060_000,
    };
    let mut snap = PlayerSnapshot::from_seed(PlayerId(1367), &seed);
    snap.radar_cards.push(RadarCardRecord {
        id: StrNum(828),
        amount: StrNum(5),
        max: StrNum(60),
        option: vec![RadarCardOption { id: StrNum(50), active: StrNum(1), param: StrNum(12) }],
        level: StrNum(2),
        used: StrNum(1),
    });
    snap.badges.push(PlayerBadgeRecord {
        id_badges: StrNum(7),
        time_of_use_badges_ms: StrNum(1_800_000_000_000),
        is_use: StrBool(true),
    });
    snap.daily_gifts.push(DailyGiftRecord { id: StrNum(1), da_nhan: StrBool(true) });

    let bytes = encode_val("player", &snap).expect("postcard encode");
    let decoded: PlayerSnapshot = decode_val("player", &bytes).expect("postcard decode");
    assert_eq!(decoded, snap);
}

#[test]
fn item_time_index_drift_legacy_vs_fixed_parity_modes() {
    let state = ItemTimeState {
        co_bon_la_ms: 11_111,
        kho_bau_x2_ms: 22_222,
        bua_santa_ms: 33_333,
        eat_meal_ms: 44_444,
        icon_meal: 99,
        ncd_ms: 55_555,
        kilis_ms: 66_666,
        nuoc_mia3_ms: 77_777,
        ..ItemTimeState::default()
    };

    let wire = write_data_item_time(&state).expect("write 32 slots");
    let fixed = parse_data_item_time(&wire, ParityMode::Fixed).expect("fixed parse");
    assert_eq!(fixed, state);

    let legacy = parse_data_item_time(&wire, ParityMode::Legacy).expect("legacy parse");
    assert_eq!(legacy.kho_bau_x2_ms, 11_111);
    assert_eq!(legacy.bua_santa_ms, 55_555);
}

#[tokio::test]
async fn argon2id_transparent_plaintext_upgrade_fits_varchar_100() {
    let upgraded = verify_or_upgrade_password("1".to_owned(), "1".to_owned())
        .await
        .expect("plaintext matches")
        .expect("returns new PHC hash");
    assert!(upgraded.starts_with("$argon2id$"));
    assert!(upgraded.len() <= ACCOUNT_PASSWORD_MAX_LEN);

    let recheck = verify_or_upgrade_password(upgraded, "1".to_owned())
        .await
        .expect("PHC verifies");
    assert!(recheck.is_none());
}

proptest! {
    #[test]
    fn prop_signed_key_encoding_preserves_total_order(a: i64, b: i64, x: i32, y: i32) {
        prop_assert_eq!(dec_i64(&enc_i64(a)).expect("dec_i64"), a);
        prop_assert_eq!(dec_i32(&enc_i32(x)).expect("dec_i32"), x);
        prop_assert_eq!(enc_i64(a).cmp(&enc_i64(b)), a.cmp(&b));
        prop_assert_eq!(enc_i32(x).cmp(&enc_i32(y)), x.cmp(&y));
    }

    #[test]
    fn prop_roundtrip_point_location_inventory_and_tree(
        limit_power in 0_u8..=9,
        power in 0_i64..=100_000_000_000,
        tiem_nang in 0_i64..=100_000_000_000,
        stamina in 0_u16..=10_000,
        max_stamina in 1_u16..=10_000,
        hpg in 100_i64..=500_000_000,
        mpg in 100_i64..=500_000_000,
        dameg in 10_i64..=100_000_000,
        defg in 0_i32..=1_000_000,
        critg in 0_u8..=100,
        critdragon in 0_i8..=100,
        hp in 1_i64..=500_000_000,
        mp in 1_i64..=500_000_000,
        map_id in 0_i16..=200,
        x in 0_i16..=4_000,
        y in 0_i16..=4_000,
        gold in 0_i64..=200_000_000_000,
        gem in 0_i32..=1_000_000_000,
        ruby in 0_i32..=1_000_000_000,
        coupon in 0_i32..=1_000_000,
        event in 0_i32..=1_000_000,
        tree_lvl in 1_i8..=10,
        peas in 0_i8..=30,
        upg in any::<bool>(),
        t1 in 0_i64..=2_000_000_000_000,
        t2 in 0_i64..=2_000_000_000_000,
    ) {
        let stats = CoreStats {
            limit_power, power, tiem_nang, stamina, max_stamina, hpg, mpg, dameg,
            defg, critg, critdragon, hp, mp, ..CoreStats::default()
        };
        let enc_pt = write_data_point(&stats).expect("write_data_point");
        prop_assert_eq!(parse_data_point(&enc_pt).expect("parse_data_point"), stats);

        let loc = Location { map_id: MapId(map_id), zone_id: ZoneId(0), x, y };
        let enc_loc = write_data_location(&loc).expect("write_data_location");
        prop_assert_eq!(parse_data_location(&enc_loc).expect("parse_data_location"), loc);

        let wallet = InventoryWallet { gold, gem, ruby, coupon, event };
        let enc_w = write_data_inventory(&wallet).expect("write_data_inventory");
        prop_assert_eq!(parse_data_inventory(&enc_w, ParityMode::Fixed).expect("parse_data_inventory"), wallet);

        let tree = MagicTreeState {
            level: tree_lvl, curr_peas: peas, is_upgrade: upg,
            last_time_harvest_ms: t1, last_time_upgrade_ms: t2,
        };
        let enc_tree = write_data_magic_tree(&tree).expect("write_data_magic_tree");
        prop_assert_eq!(parse_data_magic_tree(&enc_tree).expect("parse_data_magic_tree"), tree);
    }

    #[test]
    fn prop_roundtrip_items_social_tasks_intrinsic_and_timers(
        temp_id in 0_i16..=2000,
        qty in 1_u32..=999,
        opt_pairs in prop::collection::vec((0_u16..=250, 0_i32..=100_000), 0..=4),
        ts in 1_i64..=2_000_000_000_000,
        peer_name in "[a-zA-Z0-9_]{1,12}",
        peer_power in 0_i64..=99_999_999_999,
        task_id in 0_i8..=30,
        sub_idx in 0_i8..=10,
        task_cnt in 0_i16..=5_000,
        bh in 0_i64..=600_000,
        bh2 in 0_i64..=600_000,
        meal2 in 0_i64..=600_000,
        icon2 in 0_i32..=20_000,
        co_bon_la in 0_i64..=3_600_000,
        kilis in 0_i64..=3_600_000,
    ) {
        let mut opts = SmallVec::<[ItemOption; 4]>::new();
        for (id, param) in opt_pairs { opts.push(ItemOption { id, param }); }
        let items = vec![
            Some(ItemInstance { template_id: TemplateId(temp_id), quantity: qty, options: opts, create_time_ms: ts }),
            None,
        ];
        let enc_bag = write_items_container("items_bag", &items, ts).expect("write_items_container");
        prop_assert_eq!(parse_items_container("items_bag", &enc_bag).expect("parse_items_container"), items);

        let peers = vec![SocialPeer {
            id: 42, name: peer_name, head: 102, body: 103, leg: 104, bag: 5, power: peer_power,
        }];
        let enc_peers = write_social_list("friends", &peers).expect("write_social_list");
        prop_assert_eq!(parse_social_list("friends", &enc_peers).expect("parse_social_list"), peers);

        let mt = MainTaskProgress { task_id, sub_index: sub_idx, count: task_cnt, last_time_ms: ts };
        let enc_mt = write_data_task(&mt).expect("write_data_task");
        prop_assert_eq!(parse_data_task(&enc_mt).expect("parse_data_task"), mt);

        let st = DailyTaskProgress {
            template_id: i32::from(task_id), received_time_ms: ts, count: 3, max_count: 20, left_task: 9, level: 2,
        };
        let enc_st = write_daily_task("data_side_task", &st).expect("write_daily_task");
        prop_assert_eq!(parse_daily_task("data_side_task", &enc_st).expect("parse_daily_task"), st);

        let intr = IntrinsicState {
            intrinsic_id: 5, param1: 15, param2: 0, count_open: 3,
            is_intrinsic: true, skill_id: 2, cooldown: 500, last_time_use_skill_ms: ts,
        };
        let enc_intr = write_data_intrinsic(&intr).expect("write_data_intrinsic");
        prop_assert_eq!(parse_data_intrinsic(&enc_intr).expect("parse_data_intrinsic"), intr);

        let sc = ItemTimeSieuCap {
            bo_huyet2_ms: bh2, bo_khi2_ms: bh2, giap_xen2_ms: 0, cuong_no2_ms: 0,
            an_danh2_ms: 0, eat_meal2_ms: meal2, icon_meal2: icon2,
        };
        let it = ItemTimeState {
            bo_huyet_ms: bh,
            sieu_cap: sc,
            co_bon_la_ms: co_bon_la,
            kilis_ms: kilis,
            ..ItemTimeState::default()
        };
        let enc_it = write_data_item_time(&it).expect("write_data_item_time");
        prop_assert_eq!(parse_data_item_time(&enc_it, ParityMode::Fixed).expect("parse_data_item_time"), it);
    }
}
