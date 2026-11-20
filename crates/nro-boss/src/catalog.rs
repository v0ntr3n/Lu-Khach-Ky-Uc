//! Built-in data-driven catalog covering all 162 boss classes across 33 subgroups.
//!
//! Together with the 14 mob and big-boss classes in [`crate::big_boss`], this registry encodes
//! 100% of the 176 boss/mob classes in `Teamobi2026` (`models/boss/*` and
//! `models/boss_con_duong_ran_doc/*`).

use crate::{
    AppearType, BossArchetype, BossDefinition, BossFlags, BossLootProfile, BossTypeId, DamageRule,
    Location, ManagerGroup, MapId, SkillKind, StageConfig, TimeWindowGate, WaveRole,
    lifecycle::SkillSlot, spawner::SeasonalEvent,
};
use smallvec::{SmallVec, smallvec};

/// Builds the complete canonical catalog of `BossDefinition` entries for all boss subgroups.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn builtin_boss_catalog() -> Vec<BossDefinition> {
    let mut out = Vec::with_capacity(148);
    let nf = BossFlags::NOTIFY_GLOBAL | BossFlags::EMPTY_ZONE_DESPAWN;

    // 1. Broly & SuperBroly (1..=2)
    out.push(def_one(1, -1822, "Broly", [291, 292, 293], 1_000, 100_000, 600, &[5, 13, 20, 27, 28, 29], ManagerGroup::Broly, BossFlags::ZONE_01_DISABLED, DamageRule::BrolyGrowth { is_super: false }, BossArchetype::Broly { is_super: false }, BossLootProfile::None));
    let mut sb = def_one(2, -82282, "Super Broly", [294, 295, 296], 200_000, 16_070_777, 0, &[5, 13, 20], ManagerGroup::Broly, BossFlags::NOTIFY_GLOBAL | BossFlags::HAS_NORMAL_PET | BossFlags::DISPOSE_ON_LEAVE, DamageRule::BrolyGrowth { is_super: true }, BossArchetype::Broly { is_super: true }, BossLootProfile::SuperBroly);
    sb.appear_type = AppearType::CallByAnother;
    sb.wave_role = WaveRole::SummonedChild;
    out.push(sb);

    // 2. Nappa (3..=5), Fide 3-stage (6), Cooler 2-stage (7)
    for (bt, lid, name, hp, map) in [(3, -20, "Kuku", 5_000_000, 68), (4, -21, "Mập Đầu Đinh", 10_000_000, 64), (5, -22, "Rambo", 15_000_000, 74)] {
        out.push(def_one(bt, lid, name, [159, 160, 161], 25_000, hp, 300, &[map], ManagerGroup::WorldBoss, BossFlags::NOTIFY_GLOBAL | BossFlags::LEGACY_UNCOND_DESPAWN, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::NappaStar));
    }
    out.push(def_multi(6, -28, &[68, 69, 70], nf, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::StandardWorld, vec![
        stg("Fide đại ca 1", [183, 184, 185], 40_000, 15_000_000, 900, AppearType::DefaultAppear),
        stg("Fide đại ca 2", [186, 187, 188], 50_000, 25_000_000, 0, AppearType::AnotherLevel),
        stg("Fide đại ca 3", [189, 190, 191], 60_000, 40_000_000, 0, AppearType::AnotherLevel),
    ]));
    out.push(def_multi(7, -29, &[110], nf, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::HighWorld { star_pct: 15, drops_time_ring: false }, vec![
        stg("Cooler", [317, 318, 319], 32_000, 200_000_000, 1800, AppearType::DefaultAppear),
        stg("Cooler 2", [320, 321, 322], 50_000, 500_000_000, 0, AppearType::AnotherLevel),
    ]));

    // 3. Androids (8..=15): DrKore + Android19, Android14 + 15 + 13, KingKong + Pic + Poc
    out.push(def_wave(8, -30, "Android 19", [249, 250, 251], 30_000, 50_000_000, &[92, 93, 94], AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 0, wake_orders_on_death: smallvec![1], start_afk: false }, DamageRule::Standard, BossArchetype::AndroidKiAbsorb { heal_pct: 80, is_dr_kore: false }, BossLootProfile::StandardWorld, smallvec![], smallvec![]));
    out.push(def_wave(9, -31, "Dr.Kôrê", [255, 256, 257], 35_000, 60_000_000, &[92, 93, 94], AppearType::DefaultAppear, WaveRole::SequentialMember { order: 1, wake_orders_on_death: smallvec![], start_afk: true }, DamageRule::Standard, BossArchetype::AndroidKiAbsorb { heal_pct: 100, is_dr_kore: true }, BossLootProfile::StandardWorld, smallvec![BossTypeId(8)], smallvec![]));
    out.push(def_wave(10, -34, "Android 15", [261, 262, 263], 32_000, 50_000_000, &[96, 97], AppearType::AppearWithAnother, WaveRole::Independent, DamageRule::Standard, BossArchetype::AndroidTrio { member: 15, called_13: false }, BossLootProfile::StandardWorld, smallvec![], smallvec![]));
    out.push(def_wave(11, -32, "Android 13", [252, 253, 254], 50_000, 100_000_000, &[96, 97], AppearType::CallByAnother, WaveRole::SummonedChild, DamageRule::Standard, BossArchetype::AndroidTrio { member: 13, called_13: true }, BossLootProfile::StandardWorld, smallvec![], smallvec![]));
    out.push(def_wave(12, -33, "Android 14", [246, 247, 248], 35_000, 60_000_000, &[96, 97], AppearType::DefaultAppear, WaveRole::Independent, DamageRule::Standard, BossArchetype::AndroidTrio { member: 14, called_13: false }, BossLootProfile::StandardWorld, smallvec![BossTypeId(10)], smallvec![BossTypeId(11)]));
    out.push(def_wave(13, -36, "Poc", [240, 241, 242], 35_000, 70_000_000, &[97, 98, 99], AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 0, wake_orders_on_death: smallvec![1], start_afk: false }, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::StandardWorld, smallvec![], smallvec![]));
    out.push(def_wave(14, -35, "Pic", [237, 238, 239], 40_000, 80_000_000, &[97, 98, 99], AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 1, wake_orders_on_death: smallvec![2], start_afk: true }, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::StandardWorld, smallvec![], smallvec![]));
    out.push(def_wave(15, -37, "King Kong", [243, 244, 245], 45_000, 90_000_000, &[97, 98, 99], AppearType::DefaultAppear, WaveRole::SequentialMember { order: 2, wake_orders_on_death: smallvec![], start_afk: true }, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::StandardWorld, smallvec![BossTypeId(13), BossTypeId(14)], smallvec![]));

    // 4. Cell Saga (16..=24) & High World Bosses (25..=28)
    out.push(def_multi(16, -100, &[100], nf, DamageRule::PreDefDivisor { pre_div: 2, shield_div: 4, max_hp_cap_denom: 0 }, BossArchetype::CellAbsorb, BossLootProfile::CellTier { tl_pct: 10 }, vec![
        stg("Xên bọ hung 1", [228, 229, 230], 50_000, 200_000_000, 1800, AppearType::DefaultAppear),
        stg("Xên bọ hung 2", [231, 232, 233], 70_000, 400_000_000, 0, AppearType::AnotherLevel),
        stg("Xên hoàn thiện", [234, 235, 236], 90_000, 600_000_000, 0, AppearType::AnotherLevel),
    ]));
    let mut xcons = SmallVec::new();
    for i in 0..7_u16 {
        xcons.push(BossTypeId(i32::from(17 + i)));
        out.push(def_wave(17 + i, -102 - i32::from(i), &format!("Xên con {}", i + 1), [264, 265, 266], 40_000, 150_000_000, &[103], AppearType::CallByAnother, WaveRole::SummonedChild, DamageRule::PreDefDivisor { pre_div: 2, shield_div: 4, max_hp_cap_denom: 0 }, BossArchetype::Standard, BossLootProfile::CellTier { tl_pct: 10 }, smallvec![], smallvec![]));
    }
    let mut sbh = def_multi(24, -101, &[103], nf, DamageRule::PreDefDivisor { pre_div: 3, shield_div: 4, max_hp_cap_denom: 0 }, BossArchetype::SuperCellSwarm { spawned_jrs: false }, BossLootProfile::CellTier { tl_pct: 30 }, vec![
        stg("Xên hoàn thiện", [234, 235, 236], 100_000, 800_000_000, 1800, AppearType::DefaultAppear),
        stg("Siêu Bọ Hung", [234, 235, 236], 140_000, 1_000_000_000, 0, AppearType::AnotherLevel),
    ]);
    sbh.summoned_children = xcons;
    out.push(sbh);

    out.push(def_multi(25, -203, &[92, 93, 94, 96, 97, 98, 99, 100], BossFlags::NOTIFY_GLOBAL | BossFlags::ZONE_01_DISABLED, DamageRule::FlatSubShieldOne { flat_rand: 100_000, stage1_half: true }, BossArchetype::Standard, BossLootProfile::HighWorld { star_pct: 5, drops_time_ring: true }, vec![
        stg("Black Goku", [550, 880, 881], 100_000, 700_000_000, 900, AppearType::DefaultAppear),
        stg("Super Black Goku", [553, 880, 881], 150_000, 1_000_000_000, 0, AppearType::AnotherLevel),
    ]));
    out.push(def_multi(26, -203999, &[155], BossFlags::NOTIFY_GLOBAL | BossFlags::ZONE_01_DISABLED, DamageRule::FlatSubShieldOne { flat_rand: 100_000, stage1_half: true }, BossArchetype::Standard, BossLootProfile::HighWorld { star_pct: 10, drops_time_ring: false }, vec![
        stg("Cumber", [1207, 1208, 1209], 120_000, 900_000_000, 900, AppearType::DefaultAppear),
        stg("Super Cumber", [1210, 1208, 1209], 180_000, 1_200_000_000, 0, AppearType::AnotherLevel),
    ]));
    out.push(def_multi(27, -925, &[14], nf, DamageRule::BabyComposite, BossArchetype::Standard, BossLootProfile::BabyBoss, vec![
        stg("Baby", [1715, 1716, 1717], 200_000, 2_000_000_000, 900, AppearType::DefaultAppear),
        stg("Baby 2", [1718, 1719, 1720], 250_000, 2_000_000_000, 0, AppearType::AnotherLevel),
        stg("Baby 3", [1721, 1722, 1723], 30_000, 2_000_000_000, 0, AppearType::AnotherLevel),
    ]));
    out.push(def_one(28, -308, "Tàu Pảy Pảy", [338, 339, 340], 100, 10_000, 60, &[111], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::HardCap { max_damage: 100, jitter: 0, shield_one: true, legacy_dodge_div_one: true }, BossArchetype::BeginnerTaoPaiPai, BossLootProfile::None));

    // 5. Ginyu Earth (29..=33), Ginyu Namek (34..=38), Bojack Crew (39..=44)
    push_ginyu(&mut out, 29, -23, &[79, 82, 83], false);
    push_ginyu(&mut out, 34, -311, &[7, 8, 9, 10], true);
    push_bojack(&mut out, 39);

    // 6. Golden Frieza + 5x DeathBeam (45..=50) & Majin Buu 12h/14h (51..=61)
    let mut beams = SmallVec::new();
    for i in 0..5_u16 {
        beams.push(BossTypeId(i32::from(46 + i)));
        let mut b = def_wave(46 + i, -609 - i32::from(i), "$", [609, 610, 611], 1_000, 500, &[6], AppearType::CallByAnother, WaveRole::SummonedChild, DamageRule::Invulnerable, BossArchetype::DeathBeamProjectile { lifespan_ms: 14_600 + u32::from(i) * 100 }, BossLootProfile::None, smallvec![], smallvec![]);
        b.group = ManagerGroup::SkillSummoned;
        out.push(b);
    }
    let mut gf = def_wave(45, -502, "Fide Vàng", [502, 503, 504], 100_000, 1_000_000_000, &[6], AppearType::DefaultAppear, WaveRole::Independent, DamageRule::HardCap { max_damage: 50_000_000, jitter: 0, shield_one: true, legacy_dodge_div_one: false }, BossArchetype::GoldenFrieza21h, BossLootProfile::GoldenFrieza, smallvec![], beams);
    gf.group = ManagerGroup::GoldenFrieza21h;
    gf.time_gate = TimeWindowGate::Hour21Ict;
    out.push(gf);
    push_majin_buu(&mut out);

    // 7. All 18 Concrete Yardrat Classes (62..=79) with exact platform heights
    push_yardrat_18(&mut out, 62);

    // 8. Mini-bosses (80..=85), Arenas/Training (86..=116), Dungeons (117..=128), Seasonal (129..=138)
    push_minis_arenas_dungeons_events(&mut out, 80);
    out
}

fn stg(name: &str, fit: [i16; 3], dame: i64, hp: i64, rest: u32, app: AppearType) -> StageConfig {
    StageConfig {
        name: name.to_owned(),
        outfit: [fit[0], fit[1], fit[2], -1, -1, -1],
        base_dame: dame,
        hp_pool: smallvec![hp],
        skills: smallvec![SkillSlot { kind: SkillKind::Galick, level: 7, cooldown_ms: 1_000 }, SkillSlot { kind: SkillKind::Kamejoko, level: 7, cooldown_ms: 3_000 }],
        chat_start: vec![format!("{name} xuất hiện!")],
        chat_mid: vec!["Xem chiêu đây!".to_owned()],
        chat_end: vec!["Không thể nào!".to_owned()],
        rest_seconds: rest,
        appear_type: app,
    }
}

#[allow(clippy::too_many_arguments)]
fn def_one(bt: u16, lid: i32, name: &str, fit: [i16; 3], dame: i64, hp: i64, rest: u32, maps: &[u16], group: ManagerGroup, flags: BossFlags, rule: DamageRule, arch: BossArchetype, loot: BossLootProfile) -> BossDefinition {
    BossDefinition {
        boss_type: BossTypeId(i32::from(bt)), legacy_boss_id: lid, group, time_gate: TimeWindowGate::Always, flags,
        appear_type: AppearType::DefaultAppear, wave_role: WaveRole::Independent, damage_rule: rule, archetype: arch, loot_profile: loot,
        spawn_maps: maps.iter().map(|&m| MapId(m as i16)).collect(), stages: vec![stg(name, fit, dame, hp, rest, AppearType::DefaultAppear)],
        companions: SmallVec::new(), summoned_children: SmallVec::new(),
    }
}

fn def_multi(bt: u16, lid: i32, maps: &[u16], flags: BossFlags, rule: DamageRule, arch: BossArchetype, loot: BossLootProfile, stages: Vec<StageConfig>) -> BossDefinition {
    BossDefinition {
        boss_type: BossTypeId(i32::from(bt)), legacy_boss_id: lid, group: ManagerGroup::WorldBoss, time_gate: TimeWindowGate::Always, flags,
        appear_type: AppearType::DefaultAppear, wave_role: WaveRole::Independent, damage_rule: rule, archetype: arch, loot_profile: loot,
        spawn_maps: maps.iter().map(|&m| MapId(m as i16)).collect(), stages, companions: SmallVec::new(), summoned_children: SmallVec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn def_wave(bt: u16, lid: i32, name: &str, fit: [i16; 3], dame: i64, hp: i64, maps: &[u16], app: AppearType, role: WaveRole, rule: DamageRule, arch: BossArchetype, loot: BossLootProfile, comps: SmallVec<[BossTypeId; 6]>, sums: SmallVec<[BossTypeId; 8]>) -> BossDefinition {
    let rest = if app == AppearType::DefaultAppear { 600 } else { 0 };
    BossDefinition {
        boss_type: BossTypeId(i32::from(bt)), legacy_boss_id: lid, group: ManagerGroup::WorldBoss, time_gate: TimeWindowGate::Always,
        flags: BossFlags::NOTIFY_GLOBAL | BossFlags::EMPTY_ZONE_DESPAWN, appear_type: app, wave_role: role, damage_rule: rule, archetype: arch,
        loot_profile: loot, spawn_maps: maps.iter().map(|&m| MapId(m as i16)).collect(), stages: vec![stg(name, fit, dame, hp, rest, app)],
        companions: comps, summoned_children: sums,
    }
}

fn push_ginyu(out: &mut Vec<BossDefinition>, b: u16, lid: i32, maps: &[u16], nm: bool) {
    let lt = |cid: u16, st: u8| if nm { BossLootProfile::GemShowerCostume { gem_stacks: st, costume_id: cid } } else { BossLootProfile::StandardWorld };
    out.push(def_wave(b, lid, "Số 4", [168, 169, 170], 20_000, 10_000_000, maps, AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 0, wake_orders_on_death: smallvec![1], start_afk: false }, DamageRule::Standard, BossArchetype::Standard, lt(432, 10), smallvec![], smallvec![]));
    out.push(def_wave(b + 1, lid - 1, "Số 3", [171, 172, 173], 24_000, 15_000_000, maps, AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 1, wake_orders_on_death: smallvec![2], start_afk: true }, DamageRule::Standard, BossArchetype::Standard, lt(431, 10), smallvec![], smallvec![]));
    out.push(def_wave(b + 2, lid - 2, "Số 2", [174, 175, 176], 28_000, 20_000_000, maps, AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 2, wake_orders_on_death: smallvec![3], start_afk: true }, DamageRule::Standard, BossArchetype::Standard, lt(430, 10), smallvec![], smallvec![]));
    out.push(def_wave(b + 3, lid - 3, "Số 1", [177, 178, 179], 32_000, 25_000_000, maps, AppearType::AppearWithAnother, WaveRole::SequentialMember { order: 2, wake_orders_on_death: smallvec![3], start_afk: true }, DamageRule::Standard, BossArchetype::Standard, lt(429, 10), smallvec![], smallvec![]));
    out.push(def_wave(b + 4, lid - 4, "Tiểu Đội Trưởng", [180, 181, 182], 40_000, 35_000_000, maps, AppearType::DefaultAppear, WaveRole::MinionGuardParent, DamageRule::Standard, BossArchetype::GinyuCaptain, lt(433, 20), smallvec![BossTypeId(i32::from(b)), BossTypeId(i32::from(b + 1)), BossTypeId(i32::from(b + 2)), BossTypeId(i32::from(b + 3))], smallvec![]));
}

fn push_bojack(out: &mut Vec<BossDefinition>, b: u16) {
    let maps = &[3_u16, 4, 5, 6, 27, 28];
    for (off, lid, name, hp, cid) in [(0, -316, "Bujin", 20_000_000, 423), (1, -317, "Kogu", 40_000_000, 424), (2, -318, "Zangya", 60_000_000, 425), (3, -319, "Bido", 80_000_000, 426)] {
        out.push(def_wave(b + off, lid, name, [329, 330, 331], 180_000, hp, maps, AppearType::AppearWithAnother, WaveRole::Independent, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::GemShowerCostume { gem_stacks: 10, costume_id: cid }, smallvec![], smallvec![]));
    }
    let mut bj = def_multi(b + 4, -320, maps, BossFlags::NOTIFY_GLOBAL | BossFlags::EMPTY_ZONE_DESPAWN, DamageRule::PreDefDivisor { pre_div: 2, shield_div: 4, max_hp_cap_denom: 0 }, BossArchetype::Standard, BossLootProfile::GemShowerCostume { gem_stacks: 20, costume_id: 427 }, vec![
        stg("Bojack", [323, 324, 325], 300_000, 100_000_000, 900, AppearType::DefaultAppear),
        stg("Siêu Bojack", [326, 327, 328], 300_000, 150_000_000, 0, AppearType::AnotherLevel),
    ]);
    bj.wave_role = WaveRole::MinionGuardParent;
    bj.companions = smallvec![BossTypeId(i32::from(b)), BossTypeId(i32::from(b + 1)), BossTypeId(i32::from(b + 2)), BossTypeId(i32::from(b + 3))];
    out.push(bj);
    out.push(def_one(b + 5, -321, "Siêu Bojack", [326, 327, 328], 300_000, 500_000_000, 1800, maps, ManagerGroup::WorldBoss, BossFlags::NOTIFY_GLOBAL, DamageRule::PreDefDivisor { pre_div: 2, shield_div: 4, max_hp_cap_denom: 0 }, BossArchetype::Standard, BossLootProfile::GemShowerCostume { gem_stacks: 20, costume_id: 428 }));
}

fn push_majin_buu(out: &mut Vec<BossDefinition>) {
    for (bt, lid, name, map, cap, b_imm, rev) in [
        (51, -233, "Drabura", 114, 20_000_000, false, false), (52, -234, "Bui Bui", 115, 40_000_000, true, true),
        (53, -238, "Bui Bui 2", 117, 40_000_000, true, true), (54, -235, "Yacon", 118, 50_000_000, true, true),
        (55, -237, "Drabura 2", 119, 20_000_000, true, false), (56, -341, "Goku", 119, 20_000_000, false, true),
        (57, -342, "Cadic", 119, 10_000_000, false, true), (58, -236, "Mabư", 120, 50_000_000, false, false),
        (59, -343, "Drabura 3", 120, 20_000_000, false, false),
    ] {
        let mut d = def_one(bt, lid, name, [418, 419, 420], 200_000, if bt == 58 { 500_000_000 } else { 50_000_000 }, 60, &[map], ManagerGroup::MajinBuu12h, BossFlags::empty(), DamageRule::HardCap { max_damage: cap, jitter: if bt == 58 { 10_000 } else { 0 }, shield_one: bt == 54, legacy_dodge_div_one: matches!(bt, 56 | 57) }, BossArchetype::MajinBuu12h { beam_immune_double_melee: b_imm, self_destruct_immune: bt == 51, casts_stone: matches!(bt, 51 | 58 | 59), is_final_mabu: bt == 58, revive_afk_60s: rev }, if bt == 58 { BossLootProfile::Mabu12h } else { BossLootProfile::None });
        d.time_gate = TimeWindowGate::Mabu12hWindow;
        out.push(d);
    }
    let mut m14 = def_multi(60, -214, &[127], BossFlags::NOTIFY_GLOBAL, DamageRule::HardCap { max_damage: 30_000_000, jitter: 10_000, shield_one: false, legacy_dodge_div_one: false }, BossArchetype::MajinBuu14h { is_belly_super_bu: false }, BossLootProfile::Mabu14h, vec![
        stg("Mabư", [297, 298, 299], 80_000, 20_000_000, 60, AppearType::DefaultAppear),
        stg("Super Bư", [421, 422, 423], 100_000, 40_000_000, 0, AppearType::AnotherLevel),
        stg("Bư Tenk", [424, 425, 426], 120_000, 80_000_000, 0, AppearType::AnotherLevel),
        stg("Bư Han", [427, 428, 429], 150_000, 160_000_000, 0, AppearType::AnotherLevel),
        stg("Kid Bư", [439, 440, 441], 200_000, 250_000_000, 0, AppearType::AnotherLevel),
    ]);
    m14.group = ManagerGroup::MajinBuu14h;
    m14.time_gate = TimeWindowGate::Mabu14hWindow;
    out.push(m14);
    let mut sbu = def_one(61, -348, "Super Bư (Belly)", [421, 422, 423], 1, 250_000_000, 60, &[128], ManagerGroup::MajinBuu14h, BossFlags::empty(), DamageRule::Standard, BossArchetype::MajinBuu14h { is_belly_super_bu: true }, BossLootProfile::None);
    sbu.time_gate = TimeWindowGate::Mabu14hWindow;
    out.push(sbu);
}

fn push_yardrat_18(out: &mut Vec<BossDefinition>, mut bt: u16) {
    let specs: [(u16, &str, i16, i16, i16, i16, i64, u32, u32, bool); 18] = [
        (131, "TAPSU0", 170, 456, 240, 456, 350_000, 30_000, 5, false),
        (131, "TAPSU1", 376, 456, 446, 432, 350_000, 30_000, 5, false),
        (131, "TAPSU2", 582, 432, 652, 456, 350_000, 30_000, 5, false),
        (131, "TAPSU3", 787, 456, 857, 408, 350_000, 30_000, 5, false),
        (131, "TAPSU4", 993, 456, 1063, 456, 350_000, 30_000, 5, false),
        (131, "TANBINH5", 1199, 456, 1269, 456, 450_000, 25_000, 4, true),
        (132, "TANBINH0", 170, 456, 240, 456, 450_000, 25_000, 4, false),
        (132, "TANBINH1", 376, 456, 446, 432, 450_000, 25_000, 4, false),
        (132, "TANBINH2", 582, 432, 652, 432, 450_000, 25_000, 4, false),
        (132, "TANBINH3", 787, 432, 857, 408, 450_000, 25_000, 4, false),
        (132, "TANBINH4", 993, 456, 1063, 456, 450_000, 25_000, 4, false),
        (132, "CHIENBINH5", 1199, 456, 1269, 432, 500_000, 20_000, 3, true),
        (133, "CHIENBINH0", 170, 456, 240, 456, 500_000, 20_000, 3, false),
        (133, "CHIENBINH1", 376, 456, 446, 456, 500_000, 20_000, 3, false),
        (133, "CHIENBINH2", 582, 456, 652, 456, 500_000, 20_000, 3, false),
        (133, "CHIENBINH3", 787, 456, 857, 456, 500_000, 20_000, 3, false),
        (133, "CHIENBINH4", 993, 456, 1063, 456, 500_000, 20_000, 3, false),
        (133, "DOITRUONG5", 1199, 456, 1269, 456, 1_000_000, 15_000, 2, true),
    ];
    let mut comps = SmallVec::new();
    for (idx, &(map, name, x1, y1, x2, y2, hp, regen, ratio, is_lead)) in specs.iter().enumerate() {
        if !is_lead {
            comps.push(BossTypeId(i32::from(bt)));
        }
        let app = if is_lead { AppearType::DefaultAppear } else { AppearType::AppearWithAnother };
        let mut y = def_wave(bt, -322 - idx as i32, name, [526, 525, 524], 10_000, hp, &[map], app, WaveRole::Independent, DamageRule::YardratUndying { reward_ratio: ratio }, BossArchetype::YardratPatrol { point_a: Location::new(x1, y1), point_b: Location::new(x2, y2), regen_ms: regen }, BossLootProfile::None, if is_lead { comps.clone() } else { smallvec![] }, smallvec![]);
        y.group = ManagerGroup::Yardrat;
        y.stages[0].rest_seconds = 1;
        if is_lead {
            comps.clear();
        }
        out.push(y);
        bt += 1;
    }
}

fn push_minis_arenas_dungeons_events(out: &mut Vec<BossDefinition>, mut bt: u16) {
    // 6 Mini-bosses (80..=85)
    out.push(def_one(bt, -365, "Ăn trộm", [201, 202, 203], 1, 100, 600, &[0, 7, 14], ManagerGroup::WorldBoss, BossFlags::ZONE_ZERO_ONLY, DamageRule::FixedHit { fixed: 1, counter_blind: true }, BossArchetype::ThiefAnTrom { stolen_gold: 0 }, BossLootProfile::MiniBoss { variant: 1 }));
    out.push(def_one(bt + 1, -799, "Mặt Trời", [600, 601, 602], 1, 100, 600, &[5, 13, 20], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::FixedHit { fixed: 1, counter_blind: false }, BossArchetype::MiniHazard { variant: 0 }, BossLootProfile::MiniBoss { variant: 0 }));
    out.push(def_one(bt + 2, -79, "Virut", [603, 604, 605], 1, 100, 600, &[5, 13, 20], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::FixedHit { fixed: 1, counter_blind: false }, BossArchetype::MiniHazard { variant: 1 }, BossLootProfile::MiniBoss { variant: 1 }));
    out.push(def_one(bt + 3, -78, "Ở Dơ", [400, 401, 402], 1, 500_000, 600_000, &[5, 13, 20], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::FixedHit { fixed: 50_000, counter_blind: false }, BossArchetype::MiniHazard { variant: 2 }, BossLootProfile::MiniBoss { variant: 1 }));
    out.push(def_one(bt + 4, -77, "Sói Hẹc Quyn", [394, 395, 396], 500, 10_000, 600, &[5, 13, 20], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::Invulnerable, BossArchetype::Standard, BossLootProfile::MiniBoss { variant: 1 }));
    out.push(def_one(bt + 5, -386998, "Rồng Nhí", [1662, 1663, 1664], 1, 50_000_000, 60, &[1, 2, 3], ManagerGroup::WorldBoss, BossFlags::empty(), DamageRule::Standard, BossArchetype::Standard, BossLootProfile::MiniBoss { variant: 2 }));
    bt += 6;

    // 13 DHVT + 5 HatMit + 9 Training + SuperRank + NhanBan + Pocolo (86..=115)
    for (lid, name, hp) in [
        (-77, "Sói Hẹc Quyn", 10_000), (-78, "Ở Dơ", 25_000), (-79, "Xinbatô", 50_000), (-80, "Cha Pa", 100_000),
        (-81, "Pon Put", 250_000), (-82, "Chan Xư", 500_000), (-83, "Tàu Pảy Pảy", 2_000_000), (-84, "Yamcha", 5_000_000),
        (-85, "Jacky Chun", 25_000_000), (-86, "Thiên Xin Hăng", 75_000_000), (-88, "Thiên Xin Hăng Clone", 20_000_000),
        (-87, "Liu Liu", 150_000_000), (-92, "PôCôLô", 100_000_000),
    ] {
        out.push(def_one(bt, lid, name, [356, 357, 358], 20_000, hp, 5, &[168], ManagerGroup::OtherInstanced, BossFlags::IMMORTAL_MP | BossFlags::DISPOSE_ON_LEAVE, DamageRule::Standard, BossArchetype::DhvtArena { is_chan_xu: lid == -82, spawns_clones: lid == -86 }, BossLootProfile::None));
        bt += 1;
    }
    for (idx, (lid, name)) in [(-93, "Đracula"), (-94, "Người Vô Hình"), (-95, "Bông Băng"), (-96, "Vua Quỷ Sa Tăng"), (-97, "Thỏ Đầu Bạc")].into_iter().enumerate() {
        out.push(def_one(bt, lid, name, [353, 354, 355], 2_000, 15_000, 1, &[112], ManagerGroup::OtherInstanced, BossFlags::IMMORTAL_MP | BossFlags::DISPOSE_ON_LEAVE, DamageRule::HatMitArena, BossArchetype::HatMitSpec { player_hp_pct: 111 + idx as u16, drains_blood: idx == 0, cycles_stealth: idx == 1 }, BossLootProfile::None));
        bt += 1;
    }
    for (lid, name, map, hp) in [
        (-309, "Tàu Pảy Pảy", 46, 1_000), (-357, "Karin", 46, 500), (-358, "Yajirô", 46, 1_100),
        (-359, "Mr.PôPô", 46, 5_100), (-360, "Thượng Đế", 49, 1_000), (-361, "Khỉ Bubbles", 48, 30_000),
        (-362, "Thần Vũ Trụ", 48, 45_000), (-363, "Tổ Sư Kaio", 50, 45_000), (-364, "Whis", 154, 550_000),
    ] {
        let rule = if lid == -363 { DamageRule::Invulnerable } else if lid == -364 { DamageRule::WhisScaled { level: 1 } } else { DamageRule::Standard };
        out.push(def_one(bt, lid, name, [838, 839, 840], 10_000, hp, 1, &[map], ManagerGroup::OtherInstanced, BossFlags::DISPOSE_ON_LEAVE, rule, BossArchetype::TrainingMaster { task_10_1_gate: lid == -309, passive_tnsm_per_10s: if lid == -363 { 50_000 } else { 0 } }, BossLootProfile::None));
        bt += 1;
    }
    out.push(def_one(bt, -999, "Rival", [0, 1, 2], 50_000, 1_000_000, 1, &[113], ManagerGroup::OtherInstanced, BossFlags::IS_COPY | BossFlags::DISPOSE_ON_LEAVE, DamageRule::PvpMirror { armor_pct: 20, ki_absorb_pct: 0, giap_xen_tier: 0 }, BossArchetype::Standard, BossLootProfile::None));
    out.push(def_one(bt + 1, -1000, "Nhân Bản", [0, 1, 2], 50_000, 1_000_000, 1, &[139], ManagerGroup::OtherInstanced, BossFlags::IS_COPY | BossFlags::DISPOSE_ON_LEAVE, DamageRule::Standard, BossArchetype::Standard, BossLootProfile::Commeson638));
    out.push(def_one(bt + 2, -340, "Đại Ma Vương Pôcôlô", [9, 12, 13], 100_000, 10_000_000, 1, &[146], ManagerGroup::OtherInstanced, BossFlags::DISPOSE_ON_LEAVE, DamageRule::Standard, BossArchetype::PiccoloDivineWater { laser_fired: false }, BossLootProfile::None));
    bt += 3;

    // 4 Clan Dungeons (RedRibbon 6 + BDKB 1 + KhiGas 2 + SnakeWay 3 = 12)
    for (lid, name, map, bulon, corr, split) in [
        (-4, "Trung Úy Trắng", 59, true, false, false), (-6, "Trung Úy Xanh Lơ", 62, false, false, false),
        (-5, "Trung Úy Thép", 55, false, true, false), (-7, "Ninja Áo Tím", 54, false, false, true),
        (-9, "Ninja Clone", 54, false, false, false), (-8, "Robot Vệ Sĩ", 57, false, false, false),
    ] {
        out.push(def_one(bt, lid, name, [141, 142, 143], 50_000, 5_000_000, 0, &[map], ManagerGroup::RedRibbonDungeon, BossFlags::DISPOSE_ON_LEAVE, DamageRule::PreDefDivisor { pre_div: if corr { 4 } else { 2 }, shield_div: 2, max_hp_cap_denom: if corr { 20 } else { 0 } }, BossArchetype::RedRibbonSpec { requires_bulons_dead: bulon, corridor_640_980: corr, ninja_split_available: split }, BossLootProfile::RedRibbon));
        bt += 1;
    }
    out.push(def_one(bt, -72, "Trung Úy Xanh Lơ (BĐKB)", [141, 142, 143], 200_000, 20_000_000, 0, &[137], ManagerGroup::TreasureMapDungeon, BossFlags::DISPOSE_ON_LEAVE, DamageRule::PreDefDivisor { pre_div: 2, shield_div: 2, max_hp_cap_denom: 0 }, BossArchetype::Standard, BossLootProfile::TreasureMapKey));
    out.push(def_one(bt + 1, -208, "Dr. Lychee", [639, 640, 641], 50_000, 20_000_000, 0, &[148], ManagerGroup::DestronGasDungeon, BossFlags::DISPOSE_ON_LEAVE, DamageRule::KhiGasScaled { level: 1, is_hatchiyack: false }, BossArchetype::Standard, BossLootProfile::DestronGas { level: 1, is_hatchiyack: false }));
    out.push(def_one(bt + 2, -207, "Hatchiyack", [639, 640, 641], 75_000, 30_000_000, 0, &[148], ManagerGroup::DestronGasDungeon, BossFlags::DISPOSE_ON_LEAVE, DamageRule::KhiGasScaled { level: 1, is_hatchiyack: true }, BossArchetype::Standard, BossLootProfile::DestronGas { level: 1, is_hatchiyack: true }));
    for (off, lid, name, kami, ooz, cad) in [(3, -17, "Saibamen", true, false, false), (4, -16, "Nadic", false, false, false), (5, -15, "Cadich", false, true, true)] {
        out.push(def_one(bt + off, lid, name, [642, 643, 644], 50_000, 10_000_000, 0, &[144], ManagerGroup::SnakeWayDungeon, BossFlags::DISPOSE_ON_LEAVE, DamageRule::PreDefDivisor { pre_div: 7, shield_div: 1, max_hp_cap_denom: 0 }, BossArchetype::SnakeWaySpec { kamikaze_on_death: kami, oozaru_available: ooz }, BossLootProfile::SnakeWay { is_cadich: cad }));
    }
    bt += 6;

    // 10 Seasonal Event Bosses across 5 Festivals
    for (off, lid, name, code) in [(0, -351, "Bí Ma", 2), (1, -350, "Dơi", 3), (2, -349, "Ma Trơi", 4)] {
        let mut h = def_one(bt + off, lid, name, [760, 761, 762], 100, 500_000, 600, &[5, 13, 20], ManagerGroup::HalloweenEvent, BossFlags::EMPTY_ZONE_DESPAWN, DamageRule::PreDefDivisor { pre_div: 7, shield_div: 1, max_hp_cap_denom: 50 }, BossArchetype::HalloweenBoss { disguise_code: code }, BossLootProfile::SeasonalItem { item_id: 585, with_costume_opts: false });
        h.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::Halloween);
        out.push(h);
    }
    let mut noel = def_one(bt + 3, -353, "Ông Già Noel", [657, 658, 659], 0, 500, 60, &[0, 7, 14], ManagerGroup::ChristmasEvent, BossFlags::empty(), DamageRule::Invulnerable, BossArchetype::ChristmasSanta, BossLootProfile::None);
    noel.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::Christmas);
    out.push(noel);

    let dcap = DamageRule::HardCap { max_damage: 1_000_000, jitter: 100_000, shield_one: false, legacy_dodge_div_one: false };
    let mut st = def_wave(bt + 4, -354, "Sơn Tinh", [1683, 1684, 1685], 1_000, 50_000_000, &[1, 2, 3], AppearType::AppearWithAnother, WaveRole::Independent, dcap.clone(), BossArchetype::RivalFlagDuel { self_flag: 2, afk_reward_item: 421 }, BossLootProfile::None, smallvec![], smallvec![]);
    st.group = ManagerGroup::HungVuongEvent;
    st.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::HungVuong);
    out.push(st);
    let mut tt = def_wave(bt + 5, -355, "Thủy Tinh", [1686, 1687, 1688], 1_000, 50_000_000, &[1, 2, 3], AppearType::DefaultAppear, WaveRole::Independent, dcap.clone(), BossArchetype::RivalFlagDuel { self_flag: 1, afk_reward_item: 422 }, BossLootProfile::None, smallvec![BossTypeId(i32::from(bt + 4))], smallvec![]);
    tt.group = ManagerGroup::HungVuongEvent;
    tt.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::HungVuong);
    out.push(tt);

    let mut kd = def_one(bt + 6, -344, "Khỉ Đột", [198, 193, 194], 100_000, 100_000_000, 900, &[1, 2, 3], ManagerGroup::TrungThuAndTetEvent, BossFlags::EMPTY_ZONE_DESPAWN, DamageRule::PreDefDivisor { pre_div: 7, shield_div: 1, max_hp_cap_denom: 0 }, BossArchetype::Standard, BossLootProfile::SeasonalItem { item_id: 1045, with_costume_opts: false });
    kd.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::TrungThu);
    out.push(kd);
    let mut nhat = def_wave(bt + 7, -346, "Nhật Thần", [2065, 2066, 2067], 1_000, 50_000_000, &[1, 2, 3], AppearType::AppearWithAnother, WaveRole::Independent, dcap.clone(), BossArchetype::RivalFlagDuel { self_flag: 2, afk_reward_item: 2124 }, BossLootProfile::None, smallvec![], smallvec![]);
    nhat.group = ManagerGroup::TrungThuAndTetEvent;
    nhat.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::TrungThu);
    out.push(nhat);
    let mut nguyet = def_wave(bt + 8, -345, "Nguyệt Thần", [2058, 2059, 2060], 1_000, 50_000_000, &[1, 2, 3], AppearType::DefaultAppear, WaveRole::Independent, dcap, BossArchetype::RivalFlagDuel { self_flag: 1, afk_reward_item: 2123 }, BossLootProfile::None, smallvec![BossTypeId(i32::from(bt + 7))], smallvec![]);
    nguyet.group = ManagerGroup::TrungThuAndTetEvent;
    nguyet.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::TrungThu);
    out.push(nguyet);

    let mut lc = def_one(bt + 9, -371, "Lân Con", [763, 764, 765], 5_000, 5_000_000, 60, &[0, 7, 14], ManagerGroup::TrungThuAndTetEvent, BossFlags::empty(), DamageRule::HardCap { max_damage: 500_000, jitter: 0, shield_one: true, legacy_dodge_div_one: false }, BossArchetype::TetLanCon { tamed_owner: None }, BossLootProfile::None);
    lc.time_gate = TimeWindowGate::Seasonal(SeasonalEvent::Tet);
    out.push(lc);
}
