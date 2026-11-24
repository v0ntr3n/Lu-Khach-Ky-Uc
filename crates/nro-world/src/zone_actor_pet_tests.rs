use super::*;

#[test]
fn zone_events_satellite_pickup_proximity_and_dungeon_no_respawn() {
    let router = Arc::new(DashMap::new());
    let (ev_tx, mut ev_rx) = mpsc::channel(16);
    let (mut z, _tx) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(135), ZoneId(0)),
            max_players: 5,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router,
        vec![TrapVolume {
            rect: (80, 180, 50, 50),
            base_damage: 100,
        }],
        GameRng::from_seed(42),
    );
    z = z.with_event_tx(ev_tx);
    let (p_tx, mut p_rx) = mpsc::channel(32);
    let mut ing = test_ingress(55, "Piccolo", p_tx);
    ing.identity.1 = Some(nro_core::ClanId(7));
    ing.stats.hp = 500;
    ing.stats.hp_max = 1000;
    ing.ship_type = 0;
    let key = z.admit_player(ing).expect("admit");
    z.handle_command(ZoneCommand::DropItem(
        TemplateId(342),
        1,
        Location::new(110, 200),
        None,
        GroundDropKind::Satellite {
            clan_id: nro_core::ClanId(7),
        },
    ));
    let sat_pkt = p_rx.try_recv().expect("sat add");
    let sat_wid = sat_pkt.reader().read_i16().expect("sat wid");
    assert_eq!(
        z.pick_item(key, sat_wid),
        Err(WorldError::DungeonRule("cannot pick up satellite"))
    );
    z.handle_command(ZoneCommand::DropItem(
        TemplateId(76),
        100,
        Location::new(400, 200),
        None,
        GroundDropKind::Standard,
    ));
    let far_pkt = p_rx.try_recv().expect("far add");
    let far_wid = far_pkt.reader().read_i16().expect("far wid");
    assert_eq!(
        z.pick_item(key, far_wid),
        Err(WorldError::DungeonRule("item too far"))
    );
    z.handle_command(ZoneCommand::SeedMobs(vec![ZoneMobSeed {
        temp_id: 1,
        level: 2,
        max_hp: 10,
        loc: Location::new(100, 200),
    }]));
    let (rep_tx, _rep_rx) = oneshot::channel();
    z.handle_command(ZoneCommand::AttackMobs {
        attacker: key,
        skill_paint_id: 1,
        mob_ids: vec![0],
        damage: 50,
        is_crit: false,
        reply: rep_tx,
    });
    while p_rx.try_recv().is_ok() {}
    for _ in 0..120 {
        z.on_tick();
    }
    assert_eq!(
        z.arena.stats[z.arena.dense_index(key).expect("idx")].hp,
        500
    );
    let (mut saw_dmg, mut saw_heal) = (false, false);
    while let Ok(ZoneEvent::PlayerHpDelta { player_id, delta }) = ev_rx.try_recv() {
        assert_eq!(player_id, PlayerId(55));
        match delta.cmp(&0) {
            std::cmp::Ordering::Less => saw_dmg = true,
            std::cmp::Ordering::Greater => saw_heal = true,
            std::cmp::Ordering::Equal => {}
        }
    }
    assert!(saw_dmg && saw_heal);
    while let Ok(pkt) = p_rx.try_recv() {
        assert_ne!(pkt.cmd, cmd_message::NPC_LIVE);
    }
}

#[test]
fn pet_companion_spawns_on_home_map_follows_refreshes_and_fights_mobs() {
    let router = Arc::new(DashMap::new());
    let (ev_tx, mut ev_rx) = mpsc::channel(16);
    let (mut home, _) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(21), ZoneId(0)),
            max_players: 15,
            map_width: 800,
            parity: ParityMode::Fixed,
        },
        router.clone(),
        vec![],
        GameRng::from_seed(77),
    );
    let (h1_tx, mut h1_rx) = mpsc::channel(16);
    let (h2_tx, mut h2_rx) = mpsc::channel(16);
    let mut hi1 = test_ingress(9, "Gohan", h1_tx);
    hi1.ship_type = 0;
    let mut hi2 = test_ingress(11, "Guest", h2_tx);
    hi2.ship_type = 0;
    let _hk1 = home.admit_player(hi1).expect("admit home owner");
    let _hk2 = home.admit_player(hi2).expect("admit home guest");
    home.handle_command(ZoneCommand::SyncPlayerPet {
        player_id: PlayerId(9),
        pet: Some(PetCompanion {
            name: "$".into(),
            gender: Gender::TraiDat,
            parts: (303, 57, 60),
            flag_bag: -1,
            hp: 1_000,
            hp_max: 1_000,
            dame: 50,
            status: 3,
            master_gender: Gender::TraiDat,
            loc: Location {
                map_id: MapId(21),
                zone_id: ZoneId(0),
                x: 220,
                y: 336,
            },
        }),
    });
    let add = h1_rx.try_recv().expect("owner sees GoHome pet on home map");
    assert_eq!(add.cmd, cmd_message::PLAYER_ADD);
    let mut ar = add.reader();
    assert_eq!(ar.read_i32().expect("neg pid"), -9);
    let _ = ar.read_i32().expect("clan");
    for _ in 0..5 {
        let _ = ar.read_i8().expect("hdr");
    }
    let _ = ar.read_i16().expect("head");
    assert_eq!(ar.read_utf().expect("default_pet_name"), "$Đệ tử");
    assert!(h2_rx.try_recv().is_err());

    home.handle_command(ZoneCommand::SyncPlayerPet {
        player_id: PlayerId(9),
        pet: Some(PetCompanion {
            name: "#Mabư".into(),
            gender: Gender::TraiDat,
            parts: (297, 298, 299),
            flag_bag: -1,
            hp: 1_000,
            hp_max: 1_000,
            dame: 50,
            status: 3,
            master_gender: Gender::TraiDat,
            loc: Location {
                map_id: MapId(21),
                zone_id: ZoneId(0),
                x: 220,
                y: 336,
            },
        }),
    });
    assert_eq!(
        h1_rx.try_recv().expect("remove before re-add").cmd,
        cmd_message::PLAYER_REMOVE
    );
    assert_eq!(
        h1_rx.try_recv().expect("re-add with new parts").cmd,
        cmd_message::PLAYER_ADD
    );

    let (mut field, _) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(1), ZoneId(0)),
            max_players: 15,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router,
        vec![],
        GameRng::from_seed(88),
    );
    field = field.with_event_tx(ev_tx);
    field.handle_command(ZoneCommand::SeedMobs(vec![ZoneMobSeed {
        temp_id: 2,
        level: 2,
        max_hp: 30,
        loc: Location {
            map_id: MapId(1),
            zone_id: ZoneId(0),
            x: 180,
            y: 200,
        },
    }]));
    let (f_tx, mut f_rx) = mpsc::channel(32);
    let mut fi = test_ingress(9, "Gohan", f_tx);
    fi.ship_type = 0;
    let fk = field.admit_player(fi).expect("admit field");
    field.handle_command(ZoneCommand::SyncPlayerPet {
        player_id: PlayerId(9),
        pet: Some(PetCompanion {
            name: "Đệ tử".into(),
            gender: Gender::Xayda,
            parts: (303, 57, 60),
            flag_bag: -1,
            hp: 1_000,
            hp_max: 1_000,
            dame: 40,
            status: 0,
            master_gender: Gender::TraiDat,
            loc: Location {
                map_id: MapId(1),
                zone_id: ZoneId(0),
                x: 100,
                y: 200,
            },
        }),
    });
    assert_eq!(
        f_rx.try_recv().expect("field pet add").cmd,
        cmd_message::PLAYER_ADD
    );
    field.handle_command(ZoneCommand::MovePlayer(
        fk,
        Location {
            map_id: MapId(99),
            zone_id: ZoneId(9),
            x: 150,
            y: 200,
        },
    ));
    let mv = f_rx.try_recv().expect("master receives following pet move");
    assert_eq!(mv.cmd, cmd_message::PLAYER_MOVE);
    assert_eq!(mv.reader().read_i32().expect("pet id"), -9);

    field.handle_command(ZoneCommand::SyncPlayerPet {
        player_id: PlayerId(9),
        pet: Some(PetCompanion {
            name: "$Đệ tử".into(),
            gender: Gender::Xayda,
            parts: (303, 57, 60),
            flag_bag: -1,
            hp: 1_000,
            hp_max: 1_000,
            dame: 40,
            status: 2,
            master_gender: Gender::TraiDat,
            loc: Location::new(100, 200),
        }),
    });
    assert_eq!(
        f_rx.try_recv().expect("status change move").cmd,
        cmd_message::PLAYER_MOVE
    );
    for _ in 0..20 {
        field.on_tick();
    }
    let mut saw_xayda_paint = false;
    while let Ok(pkt) = f_rx.try_recv() {
        if pkt.cmd == cmd_message::PLAYER_ATTACK_NPC {
            let mut r = pkt.reader();
            let _ = r.read_i32().expect("pet");
            assert_eq!(r.read_u8().expect("xayda_paint"), 28);
            saw_xayda_paint = true;
        }
    }
    assert!(saw_xayda_paint);
    assert!(matches!(
        ev_rx.try_recv().expect("pet combat reward"),
        ZoneEvent::PetCombatReward {
            player_id: PlayerId(9),
            killed_mob_temp_id: Some(2),
            ..
        }
    ));
}
