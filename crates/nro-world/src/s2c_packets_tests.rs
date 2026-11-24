use nro_core::{ItemInstance, ItemOption};
use proptest::prelude::*;

use super::*;

proptest! {
    #[test]
    fn prop_stat_wire_clamp_never_negative(raw in any::<u64>()) {
        prop_assert!(clamp_u64_to_wire_i32(raw) >= 0);
        prop_assert!(clamp_u64_to_wire_i64(raw) >= 0);
    }
}

#[test]
fn transport_tab_fixes_capsule_fallthrough_in_hardened_mode() {
    let h = open_transport_tab(&[("Kame", "TD")], &[("M2", "SD")], true, ParityMode::Fixed)
        .expect("fixed");
    let b = open_transport_tab(&[("Kame", "TD")], &[("M2", "SD")], true, ParityMode::Legacy)
        .expect("legacy");
    assert!(h.payload.len() < b.payload.len());
}

#[test]
fn me_load_point_matches_v250_wire_layout() {
    let stats = CoreStats {
        hpg: 500,
        mpg: 300,
        dameg: 50,
        defg: 12,
        critg: 3,
        hp: 900,
        hp_max: 1_000,
        mp: 400,
        mp_max: 600,
        dame: 120,
        def: 45,
        crit: 8,
        speed: 9,
        tiem_nang: 50_000,
        ..CoreStats::default()
    };
    let pkt = me_load_point(&stats);
    assert_eq!(pkt.cmd, cmd_message::ME_LOAD_POINT);
    let mut r = pkt.reader();
    assert_eq!(r.read_i32().expect("hpg"), 500);
    assert_eq!(r.read_i32().expect("mpg"), 300);
    assert_eq!(r.read_i32().expect("dameg"), 50);
    assert_eq!(r.read_i64().expect("hp_full"), 1_000);
    assert_eq!(r.read_i64().expect("mp_full"), 600);
    assert_eq!(r.read_i64().expect("hp"), 900);
    assert_eq!(r.read_i64().expect("mp"), 400);
    assert_eq!(r.read_i8().expect("speed"), 9);
    assert_eq!(r.read_i8().expect("hp1000"), 20);
    assert_eq!(r.read_i8().expect("mp1000"), 20);
    assert_eq!(r.read_i8().expect("dam1000"), 1);
    assert_eq!(r.read_i64().expect("dam_full"), 120);
    assert_eq!(r.read_i64().expect("def_full"), 45);
    assert_eq!(r.read_i8().expect("crit_full"), 8);
    assert_eq!(r.read_i64().expect("tiem_nang"), 50_000);
    assert_eq!(r.read_i16().expect("exp_for_one"), 100);
    assert_eq!(r.read_i32().expect("defg"), 12);
    assert_eq!(r.read_i8().expect("critg"), 3);
    assert_eq!(r.read_i8().expect("giam_st"), 0);
    assert_eq!(r.read_i16().expect("crit_dame"), 0);
    assert!(r.is_empty());
}

#[test]
fn v250_combat_revive_die_inventory_and_wave2_builders_round_trip() {
    let hp_pkt = mob_hp_after_hit(3, 5_000_000_000, 123_456, true);
    let mut r = hp_pkt.reader();
    assert_eq!(r.read_u8().expect("mob"), 3);
    assert_eq!(r.read_i64().expect("rem"), 5_000_000_000);
    assert_eq!(r.read_i64().expect("dmg"), 123_456);
    assert!(r.read_bool().expect("crit"));
    assert_eq!(r.read_i8().expect("eff"), -1);
    assert!(r.is_empty());

    let (me_die, pl_die) =
        char_die_packets(PlayerId(70_000), 5, Location::new(100, 200), 9_000_000_000);
    let mut r = me_die.reader();
    assert_eq!(r.read_i8().expect("pk"), 5);
    assert_eq!(r.read_i16().expect("x"), 100);
    assert_eq!(r.read_i16().expect("y"), 200);
    assert_eq!(r.read_i64().expect("power"), 9_000_000_000);
    assert!(r.is_empty());
    let mut r = pl_die.reader();
    assert_eq!(r.read_i32().expect("pid"), 70_000);
    assert_eq!(r.read_i8().expect("pk"), 5);
    assert_eq!(r.read_i16().expect("x"), 100);
    assert_eq!(r.read_i16().expect("y"), 200);
    assert!(r.is_empty());

    let (_, rev_peer) = char_revive_packets(
        PlayerId(7),
        4_000_000_000,
        8_000_000_000,
        Location::new(10, 20),
    );
    let mut r = rev_peer.reader();
    assert_eq!(r.read_i8().expect("sub"), cmd_message::PLAYER_LOAD_LIVE);
    assert_eq!(r.read_i32().expect("pid"), 7);
    assert_eq!(r.read_i64().expect("hp"), 4_000_000_000);
    assert_eq!(r.read_i64().expect("hp_max"), 8_000_000_000);
    assert_eq!(r.read_i16().expect("x"), 10);
    assert_eq!(r.read_i16().expect("y"), 20);
    assert!(r.is_empty());

    let pick_item = item_map_my_pick(1, 50_000, "ok", false).expect("item pick");
    let mut pr = pick_item.reader();
    assert_eq!(pr.read_i16().expect("id"), 1);
    assert_eq!(pr.read_utf().expect("notice"), "ok");
    assert_eq!(pr.read_i16().expect("clamped_i16_max"), i16::MAX);
    assert!(pr.is_empty());
    let pick_gold = item_map_my_pick(1, 5, "ok", true).expect("gold pick");
    assert_eq!(pick_gold.payload.len(), pick_item.payload.len() + 2);

    let g_sentinel = item_map_add(
        &GroundItemWire {
            item_map_id: 9,
            template_id: TemplateId(76),
            loc: Location::new(10, 20),
            owner_id: Some(PlayerId(-2)),
        },
        ParityMode::Fixed,
    );
    let mut gr = g_sentinel.reader();
    for _ in 0..4 {
        let _ = gr.read_i16().expect("g16");
    }
    assert_eq!(gr.read_i32().expect("owner_mapped"), -1);

    let sys_chat = chat_global(PlayerId(1), "", "Bảo trì", (1, 2, 3, -1)).expect("sys chat");
    let mut scr = sys_chat.reader();
    assert_eq!(scr.read_utf().expect("empty_name"), "");
    assert_eq!(scr.read_utf().expect("text"), "|5|Bảo trì");
    assert!(scr.is_empty());

    let item = ItemInstance::with_options(TemplateId(14), 1, [ItemOption::new(300, 99_999)], 0);
    let empty_opt_item = ItemInstance::new(TemplateId(15), 2);
    let bag = inventory_bag(&[Some(item), Some(empty_opt_item), None]);
    let mut r = bag.reader();
    assert_eq!(r.read_i8().expect("sub"), 0);
    assert_eq!(r.read_u8().expect("count"), 3);
    assert_eq!(r.read_i16().expect("tid"), 14);
    assert_eq!(r.read_i32().expect("qty"), 1);
    assert_eq!(r.read_utf().expect("info"), "");
    assert_eq!(r.read_utf().expect("content"), "");
    assert_eq!(r.read_u8().expect("opt_cnt"), 1);
    assert_eq!(r.read_i16().expect("opt_id"), 300);
    assert_eq!(r.read_i32().expect("opt_param"), 99_999);
    assert_eq!(r.read_i16().expect("tid2"), 15);
    assert_eq!(r.read_i32().expect("qty2"), 2);
    assert_eq!(r.read_utf().expect("info2"), "");
    assert_eq!(r.read_utf().expect("content2"), "");
    assert_eq!(r.read_u8().expect("fallback_cnt"), 1);
    assert_eq!(r.read_i16().expect("fallback_id"), 73);
    assert_eq!(r.read_i32().expect("fallback_param"), 0);
    assert_eq!(r.read_i16().expect("empty_slot"), -1);
    assert!(r.is_empty());

    for g in [0_i8, 1, 2] {
        let cap = update_caption(g);
        assert_eq!(cap.cmd, cmd_message::UPDATE_CAPTION);
        let mut cr = cap.reader();
        assert_eq!(cr.read_i8().expect("cap_len"), 22);
        for _ in 0..22 {
            assert_ne!(cr.read_utf().expect("cap_str"), "");
        }
        assert!(cr.is_empty());
    }
    assert_eq!(change_on_skill(&[1, 2]).payload.len(), 10);
    assert_eq!(register_result(true, "").payload.len(), 1);
    assert!(register_result(false, "dup").payload.len() > 1);
    assert_eq!(inventory_body(28, &[]).cmd, cmd_message::BODY);
    assert_eq!(inventory_box(&[]).cmd, cmd_message::BOX);
    assert_eq!(inventory_box_open(0).payload.as_ref(), &[1, 0]);
    assert_eq!(
        have_attack_player(PlayerId(1), 10, 5, false, -1)
            .payload
            .len(),
        22
    );
    assert_eq!(npc_live(0, 0, 0, 1_000).payload.len(), 11);
    assert_eq!(combine_open("a", "b").cmd, cmd_message::COMBINNE);
    assert_eq!(combine_result(2).payload.as_ref(), &[2]);
    assert_eq!(trade_invite_notify(0, PlayerId(9)).payload.len(), 5);
}

#[test]
fn world_entry_magic_tree_combine_and_trade_builders_round_trip() {
    let rms = key_skill_rms("KSkill", &[1, 2, 3]).expect("rms");
    assert_eq!(rms.cmd, cmd_message::SUB_COMMAND);
    let mut r = rms.reader();
    assert_eq!(r.read_i8().expect("sub"), 61);
    assert_eq!(r.read_utf().expect("key"), "KSkill");
    assert_eq!(r.read_i32().expect("len"), 3);
    assert_eq!(r.read_bytes(3).expect("bytes"), &[1, 2, 3]);
    assert!(r.is_empty());

    assert_eq!(max_stamina(1_000).reader().read_i16().expect("ms"), 1_000);
    assert_eq!(stamina(750).reader().read_i16().expect("st"), 750);
    assert_eq!(active_point(42).reader().read_i32().expect("ap"), 42);
    assert_eq!(have_pet(true).reader().read_i8().expect("pet"), 1);

    let intr = intrinsic_info(715, "Nội tại").expect("intrinsic");
    let mut r = intr.reader();
    assert_eq!(r.read_i8().expect("sub"), 0);
    assert_eq!(r.read_i16().expect("icon"), 715);
    assert_eq!(r.read_utf().expect("info"), "Nội tại");
    assert!(r.is_empty());

    assert_eq!(type_active(true).payload.as_ref(), &[36, 1]);

    let mt = magic_tree_load(84, "Đậu thần", 348, 336, 1, 5, 7, "Cấp 1", 60, false)
        .expect("magic_tree_load");
    assert_eq!(mt.cmd, cmd_message::MAGIC_TREE);
    let menu = magic_tree_menu(&["Thu hoạch", "Nâng cấp"]).expect("magic_tree_menu");
    assert_eq!(menu.cmd, cmd_message::MAGIC_TREE);
    assert_eq!(combine_items(&[0, 3, 5]).payload.as_ref(), &[1, 3, 0, 3, 5]);
    assert_eq!(combine_result_with_icon(2, 512).payload.len(), 3);
    assert_eq!(trade_close().payload.as_ref(), &[7]);
}

#[test]
fn clamping_inventory_slots_and_map_info_counts_matches_emitted_items() {
    let slots: Vec<Option<ItemInstance>> = vec![None; 260];
    let bag = inventory_bag(&slots);
    let mut r = bag.reader();
    assert_eq!(r.read_i8().expect("sub"), 0);
    assert_eq!(r.read_u8().expect("clamped_slots"), u8::MAX);
    for _ in 0..u8::MAX {
        assert_eq!(r.read_i16().expect("empty"), -1);
    }
    assert!(r.is_empty());

    let gi: Vec<GroundItemWire> = (0..130)
        .map(|i| GroundItemWire {
            item_map_id: i,
            template_id: TemplateId(76),
            loc: Location::new(10, 20),
            owner_id: None,
        })
        .collect();
    let snap = MapInfoSnapshot {
        zone: (MapId(0), ZoneId(0)),
        header: (0, 1, 1, 0, 0, 0),
        map_name: "Lang",
        spawn_loc: Location::new(100, 200),
        waypoints: &[],
        mobs: &[],
        npcs: &[],
        ground_items: &gi,
        raw_layers: (Bytes::new(), Bytes::new()),
    };
    let pkt = map_info(&snap).expect("map_info");
    let mut r = pkt.reader();
    for _ in 0..5 {
        let _ = r.read_i8().expect("hdr");
    }
    let _ = r.read_utf().expect("name");
    let _ = r.read_i8().expect("zone");
    let _ = r.read_i16().expect("x");
    let _ = r.read_i16().expect("y");
    assert_eq!(r.read_i8().expect("wp"), 0);
    assert_eq!(r.read_i8().expect("mob"), 0);
    assert_eq!(r.read_i8().expect("npc_pad"), 0);
    assert_eq!(r.read_i8().expect("npc"), 0);
    assert_eq!(r.read_i8().expect("gi_clamped"), i8::MAX);
}

#[test]
fn ui_shop_open_client_input_and_trade_lock_round_trip() {
    let inp = client_input_open(
        "Nhập mã",
        &[
            InputFieldWire {
                label: "Code",
                input_type: 1,
            },
            InputFieldWire {
                label: "PIN",
                input_type: 2,
            },
        ],
    )
    .expect("client_input_open");
    assert_eq!(inp.cmd, cmd_message::CLIENT_INPUT);
    let mut r = inp.reader();
    assert_eq!(r.read_utf().expect("title"), "Nhập mã");
    assert_eq!(r.read_i8().expect("fields"), 2);
    assert_eq!(r.read_utf().expect("l0"), "Code");
    assert_eq!(r.read_i8().expect("t0"), 1);
    assert_eq!(r.read_utf().expect("l1"), "PIN");
    assert_eq!(r.read_i8().expect("t1"), 2);
    assert!(r.is_empty());

    let consign_items = [ShopItemWire {
        template_id: 14,
        buy_coin_gold: 500_000,
        buy_gold_gem: 0,
        power_require: 0,
        icon_spec: 0,
        buy_spec: 0,
        reason: "",
        quantity: 3,
        consign: Some((99, 0, 1, "Vegeta")),
        options: &[],
        is_new: true,
        part_temp: Some((10, 11, 12, 13)),
    }];
    let tabs = [ShopTabWire {
        name: "Ký gửi\nTrang bị",
        max_page: 4,
        items: &consign_items,
    }];
    let shop = shop_open(2, &tabs).expect("shop_open consign");
    assert_eq!(shop.cmd, cmd_message::SHOP);
    let mut r = shop.reader();
    assert_eq!(r.read_i8().expect("type_shop"), 2);
    assert_eq!(r.read_u8().expect("tabs"), 4);
    assert_eq!(r.read_utf().expect("tab_name"), "Ký gửi\nTrang bị");
    assert_eq!(r.read_u8().expect("max_page"), 4);
    assert_eq!(r.read_u8().expect("items"), 1);
    assert_eq!(r.read_i16().expect("tid"), 14);
    assert_eq!(r.read_i16().expect("listing_id"), 99);
    assert_eq!(r.read_i32().expect("coin"), 500_000);
    assert_eq!(r.read_i32().expect("gem"), 0);
    assert_eq!(r.read_i8().expect("buy_type"), 0);
    assert_eq!(r.read_i32().expect("qty"), 3);
    assert_eq!(r.read_i8().expect("is_me"), 1);
    assert_eq!(r.read_u8().expect("opt_n"), 1);
    assert_eq!(r.read_i16().expect("opt_id"), 73);
    assert_eq!(r.read_i32().expect("opt_param"), 0);
    assert_eq!(r.read_i8().expect("is_new"), 1);
    assert_eq!(r.read_i8().expect("has_part"), 1);
    assert_eq!(r.read_i16().expect("h"), 10);
    assert_eq!(r.read_i16().expect("b"), 11);
    assert_eq!(r.read_i16().expect("l"), 12);
    assert_eq!(r.read_i16().expect("bag"), 13);
    assert_eq!(r.read_utf().expect("seller"), "Vegeta");
    for _ in 0..3 {
        assert_eq!(r.read_utf().expect("pad_name"), "");
        assert_eq!(r.read_u8().expect("pad_max_page"), 1);
        assert_eq!(r.read_u8().expect("pad_items"), 0);
    }
    assert!(r.is_empty());

    let offer = trade_lock_offer(1_000, &[ItemInstance::new(TemplateId(16), 5)]);
    let mut r = offer.reader();
    assert_eq!(r.read_i8().expect("sub"), 6);
    assert_eq!(r.read_i32().expect("gold"), 1_000);
    assert_eq!(r.read_i8().expect("n"), 1);
    assert_eq!(r.read_i16().expect("tid"), 16);
    assert_eq!(r.read_i32().expect("qty"), 5);
    assert_eq!(r.read_u8().expect("opt_n"), 1);
    assert_eq!(r.read_i16().expect("opt_id"), 73);
    assert_eq!(r.read_i32().expect("opt_param"), 0);
    assert!(r.is_empty());

    assert_eq!(
        item_sale_confirm(1, 2, "Bán?").expect("sale").cmd,
        cmd_message::ITEM_SALE
    );
    assert_eq!(
        use_item_confirm(0, 1, 2, "Dùng?").cmd,
        cmd_message::USE_ITEM
    );
    assert_eq!(me_load_hp(500).payload.len(), 9);
    assert_eq!(me_load_mp(250).payload.len(), 9);
    assert_eq!(me_add_skill(17).payload.as_ref(), &[23, 0, 17]);
    assert_eq!(me_update_skill(17).payload.as_ref(), &[62, 0, 17]);
    assert_eq!(player_up_exp(2, 150).payload.len(), 5);
    assert_eq!(
        peer_attack_mobs(PlayerId(7), 3, &[0_u8; 15]).payload.len(),
        15
    );
    assert_eq!(
        update_body(PlayerId(7), Some((1, 2, 3, 1))).payload.len(),
        12
    );
    assert_eq!(update_body(PlayerId(7), None).payload.len(), 5);
    assert_eq!(send_flag_bag(PlayerId(7), 205).payload.len(), 6);
    assert_eq!(task_next().cmd, cmd_message::TASK_NEXT);
    assert!(task_next().payload.is_empty());
    assert_eq!(task_update(5).payload.as_ref(), &[0, 5]);
    assert_eq!(trade_remove_rejected_item(3).payload.as_ref(), &[2, 3]);
}
