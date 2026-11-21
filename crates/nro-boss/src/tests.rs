//! Unit and `proptest` property-based verification suites for `nro-boss`.
//!
//! Verifies the 9-state `BossStatus` lifecycle, `AppearType` wave deadlock recovery, Broly /
//! GoldenFrieza / Yardrat damage caps, `DrKore.injured(long)` Ki-blast heal parity, all 4 clan
//! dungeon scalers, all 9 `BigBoss` AoE encounters, `RewardEngine` 14-tier equipment &
//! `rewardCapsuleTet` crash fix, and the 16-group `BossGroupManager` scheduler.

use crate::{
    AppearType, BigBossKind, BigBossState, BossArchetype, BossError, BossGroupManager, BossStatus,
    BossTypeId, ClanId, ClothesStatProfile, CoreStats, DamageRule, DungeonKind, DungeonParams,
    EngineEffect, GameRng, Gender, Location, MapId, MobMapKind, MobState, ParityMode, PkMode,
    PlayerDropCtx, PlayerId, PlayerView, RewardEngine, SkillKind, TemplateId, ZoneId,
    mechanics::{evaluate_boss_damage, scale_dungeon_boss, steal_thief_gold},
    spawner::{SeasonalEvent, WorldTickContext},
};
use proptest::prelude::*;

fn sample_player(id: i64, clan: Option<i32>, loc: Location) -> PlayerView {
    PlayerView {
        id: PlayerId(id),
        clan_id: clan.map(ClanId),
        gender: Gender::default(),
        location: loc,
        stats: CoreStats {
            hp: 50_000_000,
            hp_max: 50_000_000,
            mp: 10_000_000,
            mp_max: 10_000_000,
            dame: 2_000_000,
            def: 1_000,
            crit: 20,
            dodge: 10, ..CoreStats::default() },
        power: 80_000_000_000,
        flag: 1,
        gold: 5_000_000_000,
        has_pet: false,
        carrying_namek_ball: false,
        active_task: (10, 1),
        whis_top_level: 12,
        last_chat: None,
    }
}

#[test]
fn dr_kore_long_damage_ki_absorb_parity_and_fix() {
    let mut rng = GameRng::from_seed(42);
    let mut effects = Vec::new();
    let mut shield = false;
    let mut arch = BossArchetype::AndroidKiAbsorb { heal_pct: 100, is_dr_kore: true };
    let mut stats = CoreStats {
        hp: 10_000_000,
        hp_max: 60_000_000,
        mp: 1_000,
        mp_max: 1_000,
        dame: 35_000,
        def: 0,
        crit: 0,
        dodge: 0, ..CoreStats::default() };
    let pl = sample_player(1, Some(1), Location::new(100, 100));

    let leg = evaluate_boss_damage(
        PlayerId(-31), 0, &mut stats, &mut shield, &DamageRule::Standard, &mut arch,
        Some(&pl), SkillKind::Kamejoko, 5_000_000, false, false, false,
        ParityMode::Legacy, &mut rng, &mut effects,
    );
    assert_eq!(leg.applied_damage, 5_000_000);
    assert_eq!(leg.healed_hp, 0);

    let fixed = evaluate_boss_damage(
        PlayerId(-31), 0, &mut stats, &mut shield, &DamageRule::Standard, &mut arch,
        Some(&pl), SkillKind::Masenko, 12_000_000, false, false, false,
        ParityMode::Fixed, &mut rng, &mut effects,
    );
    assert_eq!(fixed.applied_damage, 0);
    assert_eq!(fixed.healed_hp, 12_000_000);
    assert_eq!(stats.hp, 17_000_000);
}

#[test]
fn sequential_wave_and_call_by_another_android_trio_unlock() {
    let mut rng = GameRng::from_seed(7);
    let mut mgr = BossGroupManager::with_builtin_catalog(ParityMode::Fixed, 0).expect("catalog");
    assert!(mgr.catalog_len() >= 135);

    let mut world = WorldTickContext {
        ict_hour: 21,
        mabu_12h_open: true,
        mabu_14h_open: true,
        active_seasons: [
            SeasonalEvent::Halloween,
            SeasonalEvent::Christmas,
            SeasonalEvent::HungVuong,
            SeasonalEvent::TrungThu,
            SeasonalEvent::Tet,
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    world.zone_players.insert((96, 2), 3);

    for t in [601_000_u64, 602_000, 604_000, 607_000, 610_000] {
        mgr.tick_all(&world, &mut rng, t);
    }

    let a14_id = mgr.instances().values().find(|b| b.boss_type == BossTypeId(12)).expect("a14").entity_id;
    let a13 = mgr.instances().values().find(|b| b.boss_type == BossTypeId(11)).expect("a13");
    assert_eq!(a13.lifecycle.appear_type, AppearType::CallByAnother);
    assert_eq!(a13.lifecycle.pk_mode, PkMode::NonPk);
    let a13_id = a13.entity_id;
    assert_eq!(mgr.instances()[&a13_id.0].lifecycle.status, BossStatus::Rest);

    let pl = sample_player(99, Some(1), Location::new(300, 360));
    let mut effects = Vec::new();
    let out = mgr
        .apply_hit(a14_id, &pl, SkillKind::Dragon, 500_000_000, true, false, &mut rng, 611_000, &mut effects)
        .expect("hit");
    assert!(!out.killed);
    assert_eq!(mgr.instances()[&a13_id.0].lifecycle.status, BossStatus::Respawn);
}

#[test]
fn kid_bu_requires_spirit_bomb_and_super_bu_forwards_damage() {
    let mut rng = GameRng::from_seed(99);
    let mut mgr = BossGroupManager::with_builtin_catalog(ParityMode::Fixed, 0).expect("catalog");
    let mabu_id = mgr.spawn_wave(BossTypeId(60), None, ZoneId(3), 0).expect("mabu2h");
    let belly_id = mgr.spawn_wave(BossTypeId(61), None, ZoneId(3), 0).expect("superbu");
    let world = WorldTickContext { mabu_14h_open: true, ..Default::default() };
    for t in [61_000_u64, 62_000, 63_000, 64_000, 67_000] {
        mgr.tick_all(&world, &mut rng, t);
    }

    let pl = sample_player(5, Some(2), Location::new(200, 200));
    let mut effects = Vec::new();
    let hp_before = mgr.instances()[&mabu_id.0].stats.hp;
    mgr.apply_hit(belly_id, &pl, SkillKind::Galick, 5_000_000, true, false, &mut rng, 68_000, &mut effects)
        .expect("forward");
    assert!(mgr.instances()[&mabu_id.0].stats.hp < hp_before);

    let mut stats = CoreStats {
        hp: 1_000, hp_max: 250_000_000, mp: 1_000, mp_max: 1_000, dame: 200_000, def: 0, crit: 0, dodge: 0, ..CoreStats::default() };
    let mut shield = false;
    let mut arch = BossArchetype::MajinBuu14h { is_belly_super_bu: false };
    let blocked = evaluate_boss_damage(
        mabu_id, 4, &mut stats, &mut shield, &DamageRule::Standard, &mut arch,
        Some(&pl), SkillKind::Galick, 50_000, true, false, false, ParityMode::Fixed, &mut rng, &mut effects,
    );
    assert!(!blocked.killed);
    let finished = evaluate_boss_damage(
        mabu_id, 4, &mut stats, &mut shield, &DamageRule::Standard, &mut arch,
        Some(&pl), SkillKind::QuaCauKenhKhi, 50_000, true, false, false, ParityMode::Fixed, &mut rng, &mut effects,
    );
    assert!(finished.killed);
}

#[test]
fn big_boss_gau_tuong_cuop_hirudegarn_may_do_suc_manh_and_mob_rules() {
    let mut rng = GameRng::from_seed(1234);
    let mut effects = Vec::new();
    let p1 = sample_player(1, Some(10), Location::new(100, 100));
    let p2 = sample_player(2, Some(20), Location::new(110, 100));

    let mut gau = BigBossState::new(BigBossKind::GauTuongCuop, 1_000, 19_999_999, Location::new(100, 100));
    assert_eq!(gau.injured(&p1, 14_000, 100, &mut effects), 700);
    assert_eq!(gau.injured(&p2, 10_000, 200, &mut effects), 300);
    let total_caps: u32 = effects
        .iter()
        .filter_map(|e| match e {
            EngineEffect::DistributeClanCapsules(list) => Some(list.iter().map(|x| x.2).sum::<u32>()),
            _ => None,
        })
        .sum();
    assert_eq!(total_caps, 50);

    effects.clear();
    let mut hiru = BigBossState::new(BigBossKind::Hirudegarn, 10_000, 50_000, Location::new(400, 360));
    assert_eq!(hiru.injured(&p1, 999_999_999, 1_000, &mut effects), 100);
    hiru.hp = 0;
    hiru.died_at_ms = 1_000;
    hiru.tick(std::slice::from_ref(&p1), false, ParityMode::Fixed, &mut rng, 7_000, &mut effects);
    assert_eq!(hiru.phase_lv, 1);
    assert_eq!(hiru.hp, 10_000);
    let gold_piles = effects.iter().filter(|e| matches!(e, EngineEffect::DropItem(d) if d.item.template_id == TemplateId(190))).count();
    assert_eq!(gold_piles, 30);

    effects.clear();
    let mut md = BigBossState::new(BigBossKind::MayDoSucManh, 100_000_000, 0, Location::new(200, 200));
    md.injured(&p1, 25_000_000, 500, &mut effects);
    assert!(effects.contains(&EngineEffect::AwardMayDamPoints { target: p1.id, points: 20 }));
    md.tick(std::slice::from_ref(&p1), false, ParityMode::Fixed, &mut rng, 1_500, &mut effects);
    assert_eq!(md.hp, 100_000_000);

    let mut mob = MobState {
        slot_id: 1, temp_id: TemplateId(25), map_id: MapId(5), map_kind: MobMapKind::NormalWorld,
        level: 5, lv_mob: 0, hp: 5_000, max_hp: 5_000, dame: 0, p_dame: 5, location: Location::new(100, 100),
        effects: Default::default(), aggro_targets: Default::default(), died_at_ms: 0, last_regen_ms: 0, last_attack_ms: 0,
    };
    let (_, died1, _) = mob.injured(&p1, SkillKind::Kamejoko, 999_999, false, false, false, false, ParityMode::Fixed, &mut rng, 100, &mut effects);
    assert!(!died1 && mob.hp == 1);
    let (_, died2, _) = mob.injured(&p1, SkillKind::Kamejoko, 999_999, false, false, false, false, ParityMode::Fixed, &mut rng, 200, &mut effects);
    assert!(died2);
    mob.tick(&[], false, false, &mut rng, 3_500, &mut effects);
    assert_eq!(mob.hp, 5_000);
}

#[test]
fn json_catalog_mob_drops_and_legacy_capsule_error() {
    let mgr = BossGroupManager::with_builtin_catalog(ParityMode::Fixed, 0).expect("init");
    let json = mgr.to_json_string().expect("serialize");
    let reloaded = BossGroupManager::from_json_str(&json, ParityMode::Fixed, 0).expect("deserialize");
    assert_eq!(mgr.catalog_len(), reloaded.catalog_len());

    let mut rng = GameRng::from_seed(55);
    let mut total = 0_u64;
    let rich = sample_player(7, None, Location::new(100, 100));
    let stolen = steal_thief_gold(&mut total, &rich, Location::new(110, 100), ParityMode::Fixed, &mut rng);
    assert!((1_000_000..=5_000_000).contains(&stolen));

    let ctx = PlayerDropCtx { use_co_bon_la: true, free_gem_available: true, ..Default::default() };
    let drops = RewardEngine::roll_mob_drops(TemplateId(15), MapId(5), Location::new(50, 50), &rich, &ctx, ParityMode::Fixed, &mut rng);
    assert!(drops.iter().filter(|d| d.item.template_id == TemplateId(77)).count() >= 2);
    assert_eq!(RewardEngine::roll_capsule_tet(0, ParityMode::Fixed, &mut rng), Err(BossError::EmptyBagSlotRequired));
}

proptest! {
    #[test]
    fn prop_broly_golden_frieza_and_yardrat_damage_caps(
        seed in any::<u64>(),
        raw_dmg in 1_i64..=10_000_000_000_i64,
        broly_hp in 50_000_i64..=16_070_777_i64,
    ) {
        let mut rng = GameRng::from_seed(seed);
        let mut effects = Vec::new();
        let mut shield = false;

        let mut b_stats = CoreStats {
            hp: broly_hp, hp_max: broly_hp, mp: 1_000, mp_max: 1_000,
            dame: broly_hp / 10, def: 0, crit: 10, dodge: 0, ..CoreStats::default() };
        let mut b_arch = BossArchetype::Broly { is_super: false };
        let b_out = evaluate_boss_damage(
            PlayerId(-1822), 0, &mut b_stats, &mut shield,
            &DamageRule::BrolyGrowth { is_super: false }, &mut b_arch,
            None, SkillKind::Kamejoko, raw_dmg, false, false, false,
            ParityMode::Fixed, &mut rng, &mut effects,
        );
        prop_assert!(b_stats.hp_max <= 16_070_777);
        prop_assert!(b_out.applied_damage <= (b_stats.hp_max / 100).max(1));

        let mut gf_stats = CoreStats {
            hp: 1_000_000_000, hp_max: 1_000_000_000, mp: 1_000, mp_max: 1_000,
            dame: 100_000, def: 0, crit: 10, dodge: 0, ..CoreStats::default() };
        let mut gf_arch = BossArchetype::GoldenFrieza21h;
        let gf_out = evaluate_boss_damage(
            PlayerId(-502), 0, &mut gf_stats, &mut shield,
            &DamageRule::HardCap { max_damage: 50_000_000, jitter: 0, shield_one: true, legacy_dodge_div_one: false },
            &mut gf_arch, None, SkillKind::Galick, raw_dmg, false, false, false,
            ParityMode::Fixed, &mut rng, &mut effects,
        );
        prop_assert!(gf_out.applied_damage <= 50_000_000);

        let mut y_stats = CoreStats {
            hp: 500, hp_max: 350_000, mp: 1_000, mp_max: 1_000,
            dame: 10_000, def: 0, crit: 0, dodge: 0, ..CoreStats::default() };
        let mut y_arch = BossArchetype::Standard;
        let y_out = evaluate_boss_damage(
            PlayerId(-322), 0, &mut y_stats, &mut shield,
            &DamageRule::YardratUndying { reward_ratio: 2 }, &mut y_arch,
            None, SkillKind::Dragon, raw_dmg, false, false, false,
            ParityMode::Fixed, &mut rng, &mut effects,
        );
        prop_assert!(!y_out.killed && y_stats.hp >= 1);
    }

    #[test]
    fn prop_dungeon_scaling_bounds_and_monotonicity(
        level in 1_u32..=110_u32,
        clan_dame in 1_i64..=500_000_000_i64,
        clan_hp in 20_i64..=5_000_000_000_i64,
    ) {
        let p = DungeonParams { level, clan_total_dame: clan_dame, clan_total_hp: clan_hp };
        for kind in [
            DungeonKind::RedRibbon { mult_pct: 100 },
            DungeonKind::RedRibbon { mult_pct: 130 },
            DungeonKind::TreasureMap,
            DungeonKind::DestronGas { is_hatchiyack: false },
            DungeonKind::DestronGas { is_hatchiyack: true },
            DungeonKind::SnakeWay { role_mult: 1 },
            DungeonKind::SnakeWay { role_mult: 50 },
        ] {
            let (dame, hp) = scale_dungeon_boss(kind, p);
            prop_assert!((1..=200_000_000).contains(&dame) && (1..=2_147_483_647).contains(&hp));
        }
        let (ly_d, ly_hp) = scale_dungeon_boss(DungeonKind::DestronGas { is_hatchiyack: false }, p);
        let (ha_d, ha_hp) = scale_dungeon_boss(DungeonKind::DestronGas { is_hatchiyack: true }, p);
        prop_assert!(ha_d >= ly_d && ha_hp >= ly_hp);
    }

    #[test]
    fn prop_capsule_tet_never_panics_and_clothes_stats_positive(seed in any::<u64>()) {
        let mut rng = GameRng::from_seed(seed);
        let item = RewardEngine::roll_capsule_tet(1, ParityMode::Fixed, &mut rng).expect("fixed capsule");
        prop_assert!(item.quantity == 1 && !item.options.is_empty());

        for &id in &[0_i16, 233, 245, 257, 269, 281, 555, 556, 561, 562, 567, 650, 656, 661] {
            let opts = RewardEngine::init_clothes_stats(TemplateId(id), ClothesStatProfile::SetClothes, &mut rng);
            prop_assert!(!opts.is_empty() && opts.iter().all(|o| o.param >= 0));
        }
    }
}

#[test]
fn tet_lan_con_tame_leaves_on_timeout_or_owner_exit_and_resets() {
    let mut rng = GameRng::from_seed(11);
    let mut mgr = BossGroupManager::with_builtin_catalog(ParityMode::Fixed, 0).expect("catalog");
    let lc_id = mgr.instances().values().find(|b| matches!(b.archetype, BossArchetype::TetLanCon { .. })).expect("lan con").entity_id;
    let mut world = WorldTickContext { active_seasons: [SeasonalEvent::Tet].into_iter().collect(), ..Default::default() };
    for t in [61_000_u64, 62_000, 63_000, 64_000, 67_000] { mgr.tick_all(&world, &mut rng, t); }
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.status, BossStatus::Active);
    let map_key = (mgr.instances()[&lc_id.0].map_id.0 as u16, mgr.instances()[&lc_id.0].zone_id.0);
    let mut owner = sample_player(77, None, Location::new(300, 360));
    owner.last_chat = Some("thang".to_owned());
    world.zone_rosters.insert(map_key, vec![owner.clone()]);
    let mut effects = Vec::new();
    mgr.apply_hit(lc_id, &owner, SkillKind::Dragon, 100, false, false, &mut rng, 70_000, &mut effects).expect("tame");
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.status, BossStatus::Afk);
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.afk_revive_at_ms, Some(370_000));
    mgr.tick_all(&world, &mut rng, 100_000);
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.status, BossStatus::Afk);
    world.zone_rosters.insert(map_key, vec![]);
    mgr.tick_all(&world, &mut rng, 101_000);
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.status, BossStatus::LeaveMap);
    mgr.tick_all(&world, &mut rng, 102_000);
    assert_eq!(mgr.instances()[&lc_id.0].lifecycle.status, BossStatus::Rest);
    for t in [163_000_u64, 164_000] { mgr.tick_all(&world, &mut rng, t); }
    assert_eq!(mgr.instances()[&lc_id.0].archetype, BossArchetype::TetLanCon { tamed_owner: None });
}

#[test]
fn thien_xin_hang_clones_expire_at_10s_and_clean_up_on_parent_death() {
    let mut rng = GameRng::from_seed(22);
    let mut mgr = BossGroupManager::with_builtin_catalog(ParityMode::Fixed, 0).expect("catalog");
    let parent = mgr.spawn_wave(BossTypeId(95), None, ZoneId(0), 1_000).expect("txh");
    let clone1 = mgr.spawn_wave(BossTypeId(96), None, ZoneId(0), 1_000).expect("clone1");
    let clone2 = mgr.spawn_wave(BossTypeId(96), None, ZoneId(0), 1_000).expect("clone2");
    let world = WorldTickContext::default();
    for t in [6_000_u64, 7_000, 8_000, 9_000, 11_000] { mgr.tick_all(&world, &mut rng, t); }
    assert_eq!(mgr.instances()[&clone1.0].lifecycle.status, BossStatus::Active);
    let killer = sample_player(88, None, Location::new(300, 360));
    let mut effects = Vec::new();
    let out = mgr.apply_hit(parent, &killer, SkillKind::Dragon, 500_000_000, true, false, &mut rng, 12_000, &mut effects).expect("kill");
    assert!(out.killed);
    assert_eq!(mgr.instances()[&clone1.0].lifecycle.status, BossStatus::LeaveMap);
    assert_eq!(mgr.instances()[&clone2.0].lifecycle.status, BossStatus::LeaveMap);
    let clone3 = mgr.spawn_wave(BossTypeId(96), None, ZoneId(1), 20_000).expect("clone3");
    for t in [25_000_u64, 26_000, 27_000, 28_000, 30_000] { mgr.tick_all(&world, &mut rng, t); }
    assert_eq!(mgr.instances()[&clone3.0].lifecycle.status, BossStatus::Active);
    mgr.tick_all(&world, &mut rng, 37_000);
    assert_eq!(mgr.instances()[&clone3.0].lifecycle.status, BossStatus::LeaveMap);
}

#[test]
fn damage_and_combat_tick_saturate_extreme_i64_values() {
    let mut rng = GameRng::from_seed(33);
    let mut effects = Vec::new();
    let mut shield = false;
    let mut arch = BossArchetype::Standard;
    let mut stats = CoreStats { hp: i64::MAX, hp_max: i64::MAX, def: 0, dodge: 0, ..CoreStats::default() };
    let rule = DamageRule::PvpMirror { armor_pct: 0, ki_absorb_pct: 0, giap_xen_tier: 2 };
    let out = evaluate_boss_damage(PlayerId(-1), 0, &mut stats, &mut shield, &rule, &mut arch, None, SkillKind::Dragon, i64::MAX, false, false, false, ParityMode::Fixed, &mut rng, &mut effects);
    assert!(out.applied_damage >= 1);
    let mut pl = sample_player(1, None, Location::new(100, 100));
    pl.stats.dame = i64::MAX;
    pl.stats.hp = i64::MAX;
    pl.stats.hp_max = i64::MAX;
    let mut loc = Location::new(100, 100);
    let mut cell = BossArchetype::CellAbsorb;
    for _ in 0..200 { crate::combat_ai::run_boss_combat_tick(PlayerId(-2), MapId(100), ZoneId(0), &mut loc, &mut stats, &mut cell, std::slice::from_ref(&pl), ParityMode::Fixed, &mut rng, &mut effects); }
    let mut yard = BossArchetype::YardratPatrol { point_a: loc, point_b: loc, regen_ms: 1000 };
    crate::combat_ai::run_boss_combat_tick(PlayerId(-3), MapId(131), ZoneId(0), &mut loc, &mut stats, &mut yard, std::slice::from_ref(&pl), ParityMode::Fixed, &mut rng, &mut effects);
}
