#[path = "zone_actor_pet_tests.rs"]
mod zone_actor_pet_tests;

use nro_consts::cmd_message;
use nro_core::{AccountId, CoreStats, Gender, ParityMode, SessionKey};

use super::*;

fn test_ingress(pid: i64, name: &str, tx: mpsc::Sender<Packet>) -> PlayerIngress {
    PlayerIngress {
        session: (AccountId(pid as i32), SessionKey::default()),
        identity: (PlayerId(pid), None),
        gender: Gender::TraiDat,
        name: name.into(),
        loc: Location {
            map_id: MapId(0),
            zone_id: ZoneId(0),
            x: 100,
            y: 200,
        },
        stats: CoreStats {
            hp: 1_000,
            hp_max: 1_000,
            ..CoreStats::default()
        },
        parts: (1, 2, 3),
        ship_type: 1,
        outbound_tx: Some(tx),
    }
}

#[tokio::test]
async fn two_phase_handoff_rolls_back_when_destination_is_full() {
    let router = Arc::new(DashMap::new());
    let (z1, tx1) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(0), ZoneId(0)),
            max_players: 5,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router.clone(),
        vec![],
        GameRng::from_seed(1),
    );
    let (z2, tx2) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(5), ZoneId(0)),
            max_players: 0,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router,
        vec![],
        GameRng::from_seed(2),
    );
    tokio::spawn(z1.run());
    tokio::spawn(z2.run());
    let (pkt_tx, _rx) = mpsc::channel(16);
    let ing = test_ingress(7, "Goku", pkt_tx);
    let (r_tx, r_rx) = oneshot::channel();
    tx1.send(ZoneCommand::EnterPlayer(Box::new(ing), r_tx))
        .await
        .expect("send enter");
    let key = r_rx.await.expect("recv enter").expect("admit");
    let (cm_tx, cm_rx) = oneshot::channel();
    tx1.send(ZoneCommand::ChangeMap(
        key,
        (MapId(5), ZoneId(0)),
        Location {
            map_id: MapId(5),
            zone_id: ZoneId(0),
            x: 300,
            y: 300,
        },
        1,
        cm_tx,
    ))
    .await
    .expect("send cm");
    let (err, rb_key) = cm_rx.await.expect("recv cm").expect_err("dest full");
    assert!(matches!(
        err,
        WorldError::ZoneFull {
            map_id: MapId(5),
            ..
        }
    ));
    assert!(rb_key.is_some());
    let _ = tx1.send(ZoneCommand::Shutdown).await;
    let _ = tx2.send(ZoneCommand::Shutdown).await;
}

#[tokio::test]
async fn enter_syncs_existing_players_leave_removes_and_chat_allows_thang() {
    let router = Arc::new(DashMap::new());
    let (z1, tx1) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(0), ZoneId(0)),
            max_players: 5,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router,
        vec![],
        GameRng::from_seed(3),
    );
    tokio::spawn(z1.run());

    let (p1_tx, mut p1_rx) = mpsc::channel(16);
    let (p2_tx, mut p2_rx) = mpsc::channel(16);

    let (r1_tx, r1_rx) = oneshot::channel();
    tx1.send(ZoneCommand::EnterPlayer(
        Box::new(test_ingress(10, "Goku", p1_tx)),
        r1_tx,
    ))
    .await
    .expect("enter p1");
    let k1 = r1_rx.await.expect("p1 reply").expect("p1 admit");
    let _ = p1_rx.recv().await.expect("p1 teleport");

    let (r2_tx, r2_rx) = oneshot::channel();
    tx1.send(ZoneCommand::EnterPlayer(
        Box::new(test_ingress(20, "Vegeta", p2_tx)),
        r2_tx,
    ))
    .await
    .expect("enter p2");
    let k2 = r2_rx.await.expect("p2 reply").expect("p2 admit");
    tx1.send(ZoneCommand::SyncZoneState(k2))
        .await
        .expect("sync p2");

    let mut p2_saw_p1 = false;
    for _ in 0..2 {
        let pkt = p2_rx.recv().await.expect("p2 pkt");
        if pkt.cmd == cmd_message::PLAYER_ADD && pkt.reader().read_i32().expect("pid") == 10 {
            p2_saw_p1 = true;
        }
    }
    assert!(p2_saw_p1);

    tx1.send(ZoneCommand::ChatMap(k1, "xin chao thang".into()))
        .await
        .expect("chat");
    let mut saw_chat = false;
    for _ in 0..4 {
        let pkt = p1_rx.recv().await.expect("p1 pkt");
        if pkt.cmd == cmd_message::CHAT_MAP {
            saw_chat = true;
            break;
        }
    }
    assert!(saw_chat);

    tx1.send(ZoneCommand::LeavePlayer {
        player_id: PlayerId(20),
    })
    .await
    .expect("leave");
    let mut saw_remove = false;
    for _ in 0..4 {
        let pkt = p1_rx.recv().await.expect("p1 pkt");
        if pkt.cmd == cmd_message::PLAYER_REMOVE && pkt.reader().read_i32().expect("pid") == 20 {
            saw_remove = true;
            break;
        }
    }
    assert!(saw_remove);

    let _ = tx1.send(ZoneCommand::Shutdown).await;
}

#[tokio::test]
async fn pick_item_resolves_ground_item_by_wire_id() {
    let router = Arc::new(DashMap::new());
    let (z1, tx1) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(0), ZoneId(0)),
            max_players: 5,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router,
        vec![],
        GameRng::from_seed(4),
    );
    tokio::spawn(z1.run());

    let (p_tx, mut p_rx) = mpsc::channel(16);
    let (r_tx, r_rx) = oneshot::channel();
    tx1.send(ZoneCommand::EnterPlayer(
        Box::new(test_ingress(30, "Gohan", p_tx)),
        r_tx,
    ))
    .await
    .expect("enter");
    let key = r_rx.await.expect("recv").expect("admit");
    let _ = p_rx.recv().await.expect("teleport");

    tx1.send(ZoneCommand::DropItem(
        TemplateId(76),
        250,
        Location::new(100, 200),
        Some(PlayerId(30)),
        GroundDropKind::Standard,
    ))
    .await
    .expect("drop");
    let drop_pkt = p_rx.recv().await.expect("item_map_add");
    assert_eq!(drop_pkt.cmd, cmd_message::ITEMMAP_ADD);
    let wire_id = drop_pkt.reader().read_i16().expect("wire_id");

    let (pick_tx, pick_rx) = oneshot::channel();
    tx1.send(ZoneCommand::PickItem(key, wire_id, pick_tx))
        .await
        .expect("pick");
    let picked = pick_rx.await.expect("pick reply").expect("pick ok");
    assert_eq!(picked.stack, (TemplateId(76), 250));

    let (miss_tx, miss_rx) = oneshot::channel();
    tx1.send(ZoneCommand::PickItem(key, wire_id, miss_tx))
        .await
        .expect("pick again");
    assert!(miss_rx.await.expect("miss reply").is_err());

    tx1.send(ZoneCommand::RestoreGroundItem(picked.clone()))
        .await
        .expect("restore");
    let (repick_tx, repick_rx) = oneshot::channel();
    tx1.send(ZoneCommand::PickItem(key, wire_id, repick_tx))
        .await
        .expect("repick");
    let repicked = repick_rx.await.expect("repick reply").expect("repick ok");
    assert_eq!(repicked.stack, picked.stack);

    let _ = tx1.send(ZoneCommand::Shutdown).await;
}

#[test]
fn seed_mobs_attack_gold_drop_sync_respawn_and_offline_isolation() {
    let router = Arc::new(DashMap::new());
    let (mut z, _tx) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(0), ZoneId(0)),
            max_players: 5,
            map_width: 1200,
            parity: ParityMode::Fixed,
        },
        router.clone(),
        vec![],
        GameRng::from_seed(9),
    );
    z.handle_command(ZoneCommand::SeedMobs(vec![ZoneMobSeed {
        temp_id: 1,
        level: 3,
        max_hp: 10,
        loc: Location::new(150, 200),
    }]));

    let (p1_tx, mut p1_rx) = mpsc::channel(32);
    let (p2_tx, mut p2_rx) = mpsc::channel(32);
    let mut ing1 = test_ingress(101, "Krillin", p1_tx);
    ing1.ship_type = 0;
    let k1 = z.admit_player(ing1).expect("admit p1");
    assert!(p1_rx.try_recv().is_err());

    let mut ing2 = test_ingress(102, "Yamcha", p2_tx);
    ing2.ship_type = 0;
    let k2 = z.admit_player(ing2).expect("admit p2");
    let _ = p1_rx.try_recv().expect("p1 sees p2 add");

    let (rep_tx, mut rep_rx) = oneshot::channel();
    z.handle_command(ZoneCommand::AttackMobs {
        attacker: k1,
        skill_paint_id: 2,
        mob_ids: vec![0],
        damage: 1,
        is_crit: false,
        reply: rep_tx,
    });
    let (tnsm1, killed1) = rep_rx.try_recv().expect("hit1 reply");
    assert_eq!(tnsm1, 10);
    assert_eq!(killed1, Vec::<i16>::new());
    assert_eq!(p1_rx.try_recv().expect("mob_hp").cmd, cmd_message::MOB_HP);
    assert_eq!(
        p2_rx.try_recv().expect("peer_atk").cmd,
        cmd_message::PLAYER_ATTACK_NPC
    );
    assert_eq!(
        p2_rx.try_recv().expect("peer_mob_hp").cmd,
        cmd_message::MOB_HP
    );

    let (rep2_tx, mut rep2_rx) = oneshot::channel();
    z.handle_command(ZoneCommand::AttackMobs {
        attacker: k1,
        skill_paint_id: 2,
        mob_ids: vec![0],
        damage: 50,
        is_crit: true,
        reply: rep2_tx,
    });
    let (tnsm2, killed2) = rep2_rx.try_recv().expect("kill reply");
    assert_eq!(tnsm2, 40);
    assert_eq!(killed2, vec![1]);
    assert_eq!(p1_rx.try_recv().expect("mob_die").cmd, cmd_message::NPC_DIE);

    z.handle_command(ZoneCommand::SyncZoneState(k2));
    let mut saw_dead_mob = false;
    while let Ok(pkt) = p2_rx.try_recv() {
        if pkt.cmd == cmd_message::NPC_DIE {
            let mut r = pkt.reader();
            assert_eq!(r.read_u8().expect("slot"), 0);
            saw_dead_mob = true;
        }
    }
    assert!(saw_dead_mob);

    for _ in 0..100 {
        z.on_tick();
    }
    let live = p1_rx.try_recv().expect("npc_live after 5s");
    assert_eq!(live.cmd, cmd_message::NPC_LIVE);
    let mut lr = live.reader();
    assert_eq!(lr.read_u8().expect("slot"), 0);
    let _ = lr.read_i8().expect("sys");
    let _ = lr.read_i8().expect("boss");
    assert_eq!(lr.read_i64().expect("respawn_hp"), 10);

    let (mut off, _off_tx) = ZoneActor::new(
        ZoneConfig {
            id: (MapId(39), ZoneId(0)),
            max_players: 0,
            map_width: 800,
            parity: ParityMode::Fixed,
        },
        router,
        vec![],
        GameRng::from_seed(10),
    );
    let (o1_tx, mut o1_rx) = mpsc::channel(16);
    let (o2_tx, mut o2_rx) = mpsc::channel(16);
    let mut o_ing1 = test_ingress(201, "SoloA", o1_tx);
    o_ing1.ship_type = 0;
    let mut o_ing2 = test_ingress(202, "SoloB", o2_tx);
    o_ing2.ship_type = 0;
    let ok1 = off.admit_player(o_ing1).expect("offline admit 1");
    let _ok2 = off.admit_player(o_ing2).expect("offline admit 2");
    assert!(o1_rx.try_recv().is_err());
    assert!(o2_rx.try_recv().is_err());
    off.handle_command(ZoneCommand::ChatMap(ok1, "solo".into()));
    assert_eq!(
        o1_rx.try_recv().expect("self echo").cmd,
        cmd_message::CHAT_MAP
    );
    assert!(o2_rx.try_recv().is_err());
}
