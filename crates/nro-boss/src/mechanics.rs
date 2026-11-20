//! Composable `DamageRule`, `BossArchetype`, and dynamic clan dungeon scaling mechanics.
//!
//! Encodes 100% of custom incoming-damage overrides across all 159 `models/boss/` classes and
//! 3 `models/boss_con_duong_ran_doc/` classes, fixing `DrKore.injured(long)`, `AnTrom`
//! billionaire theft, and `Util.isTrue(tlNeDon, 1)` dodge bugs under [`ParityMode::Fixed`].

use crate::{
    CoreStats, EngineEffect, GameRng, Location, ParityMode, PkMode, PlayerDebuffKind, PlayerId,
    PlayerView, SkillKind, loc_distance,
};
use serde::{Deserialize, Serialize};

/// Composable incoming-damage mitigation pipeline covering every `Boss.injured` override.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DamageRule {
    /// Default `Boss.injured`: `dodge/1000`, `sub_def`, shield `/1`.
    #[default]
    Standard,
    /// Divides damage by `pre_div` pre-defense, `shield_div` under shield, and optional `hp_max / max_hp_cap_denom`.
    PreDefDivisor { pre_div: i64, shield_div: i64, max_hp_cap_denom: i64 },
    /// Baby (`models/boss/Baby/Baby.java`): `* 7 / 10` then `/ 2` pre-defense, `/ 4` shield.
    BabyComposite,
    /// Black Goku & Cumber: stage `>= 1` halves damage, subtracts `0..flat_rand`, shield `= 1`.
    FlatSubShieldOne { flat_rand: i64, stage1_half: bool },
    /// Broly & Super Broly: `hp_max / 100` cap (unless `piercing` or `TuSat`) + on-hit growth.
    BrolyGrowth { is_super: bool },
    /// Hard numerical ceiling (`GoldenFrieza` `50M`, `Mabu` `50M±10k`, `TaoPaiPai` `100`, etc.).
    HardCap { max_damage: i64, jitter: i64, shield_one: bool, legacy_dodge_div_one: bool },
    /// Fixed damage per hit (`AnTrom` `1`, `MatTroi` `1`, `Virut` `1`, `Odo` `50_000`).
    FixedHit { fixed: i64, counter_blind: bool },
    /// Completely immune to direct hit damage (`DeathBeam`, `OngGiaNoel`, `ToSuKaio`, `SoiHecQuyn`).
    Invulnerable,
    /// Yardrat (`TAPSU`..`DOITRUONG`): `10%` dodge, `hp_max / 100` cap, clamps at `1 HP` + `1/reward_ratio` drop.
    YardratUndying { reward_ratio: u32 },
    /// Destron Gas bosses (`DrLychee` `false`, `Hatchiyack` `true`) scaled by `level`.
    KhiGasScaled { level: u32, is_hatchiyack: bool },
    /// Whis training boss (`40%` dodge, divides incoming damage by `level = top + 1`).
    WhisScaled { level: u32 },
    /// Võ Đài Bà Hạt Mít (`10%` dodge, Namek-ball carrier deals `1`, `hp_max / 10` cap).
    HatMitArena,
    /// SuperRank / Rival async PvP mirror (`<=90%` net dodge, `<=86%` armor, shield `= 1`).
    PvpMirror { armor_pct: u8, ki_absorb_pct: u8, giap_xen_tier: u8 },
}

/// Special boss archetype behaviors executed on hit, lethal threshold, and active combat ticks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BossArchetype {
    /// Standard skill-casting world or arena boss.
    #[default]
    Standard,
    /// Android 19 (`80%`) & Dr. Kore (`100%`) Ki-blast absorption (`Kamejoko/Masenko/Antomic`).
    AndroidKiAbsorb { heal_pct: u8, is_dr_kore: bool },
    /// Android 14 / 15 / 13 synchronized fusion-trio state machine.
    AndroidTrio { member: u8, called_13: bool },
    /// Broly / SuperBroly dynamic growth (`tangChiSo`) and `SuperBroly` evolution spawn.
    Broly { is_super: bool },
    /// Imperfect/Semi-Perfect Cell (`XenBoHung`) player absorption (`hapThu`).
    CellAbsorb,
    /// Perfect Cell (`SieuBoHung`) 7x `XenCon` wave summon + death `TuSat` bomb.
    SuperCellSwarm { spawned_jrs: bool },
    /// Babidi spaceship Floor 1–6 bosses (`Drabura`, `BuiBui`, `Yacon`, `Mabu`).
    MajinBuu12h {
        beam_immune_double_melee: bool,
        self_destruct_immune: bool,
        casts_stone: bool,
        is_final_mabu: bool,
        revive_afk_60s: bool,
    },
    /// 14:00 Majin Buu (`Mabu2H` 5 stages + stomach swallow; `SuperBu` damage forwarding).
    MajinBuu14h { is_belly_super_bu: bool },
    /// Golden Frieza 21:00 ICT event boss (`2.1B` bomb + 5x `DeathBeam` summon).
    GoldenFrieza21h,
    /// Invulnerable homing `DeathBeam` (`2.1B` piercing contact damage, `14.6..15.0s` lifespan).
    DeathBeamProjectile { lifespan_ms: u32 },
    /// Captain Ginyu (`TDT` / `TDT_NM`) casting `50%` body-change every `10,000ms`.
    GinyuCaptain,
    /// Yardrat patrol clone (`TAPSU`, `TANBINH`, `CHIENBINH`, `DOITRUONG`).
    YardratPatrol { point_a: Location, point_b: Location, regen_ms: u32 },
    /// Thief `AnTrom` stealing gold up to `10B` and dropping `80%` across 5 piles.
    ThiefAnTrom { stolen_gold: u64 },
    /// World mini-boss (`0 = MatTroi` solar burn, `1 = Virut` plague, `2 = Odo` stink aura).
    MiniHazard { variant: u8 },
    /// Map 111 `TaoPaiPai` granting `damage * (20..=50)` TNSM to beginners (`power < 1.5M`).
    BeginnerTaoPaiPai,
    /// 23rd Martial Arts Congress (`ChanXu` stun+100% crit, `ThienXinHang` 4x `10s` clones).
    DhvtArena { is_chan_xu: bool, spawns_clones: bool },
    /// Võ Đài Bà Hạt Mít (`Dracula` `10%` blood drain, `NguoiVoHinh` `y=10000` stealth).
    HatMitSpec { player_hp_pct: u16, drains_blood: bool, cycles_stealth: bool },
    /// Auto-training tower master (`TauPayPay` Task `10_1` gate, `ToSuKaio` passive TNSM).
    TrainingMaster { task_10_1_gate: bool, passive_tnsm_per_10s: i64 },
    /// Red Ribbon `TrungUyTrang` (Bulon shield), `TrungUyThep` corridor, or `NinjaAoTim` clones.
    RedRibbonSpec { requires_bulons_dead: bool, corridor_640_980: bool, ninja_split_available: bool },
    /// Snake Way (`SAIBAMEN` `100 * hp_max` kamikaze; `CADICH` `<50%` Oozaru `2x hp_max` heal).
    SnakeWaySpec { kamikaze_on_death: bool, oozaru_available: bool },
    /// Demon King `Pocolo` (Map `146`) firing `1B` scripted laser at `1 HP` to grant `winSTT`.
    PiccoloDivineWater { laser_fired: bool },
    /// Halloween boss (`BiMa = 2, Doi = 3, MaTroi = 4`) applying `30m` disguise + `%hp` hit.
    HalloweenBoss { disguise_code: u8 },
    /// Hùng Vương (`SonTinh`/`ThuyTinh`) & Trung Thu (`NguyetThan`/`NhatThan`) flag duelists.
    RivalFlagDuel { self_flag: u8, afk_reward_item: u16 },
    /// Christmas `OngGiaNoel` dropping item `648` gift boxes every `60s` near players.
    ChristmasSanta,
    /// Tết `LanCon`: tamed on lethal hit or `"thang"` chat, then escorts bound player.
    TetLanCon { tamed_owner: Option<PlayerId> },
}

/// Result of evaluating an incoming hit against a boss's [`DamageRule`] and [`BossArchetype`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageOutcome {
    /// Final HP subtracted (`0` if dodged, absorbed, invulnerable, or intercepted).
    pub applied_damage: i64,
    /// Amount of HP healed by beam absorption or Broly regeneration.
    pub healed_hp: i64,
    /// Whether the hit reduced the boss to `hp <= 0` (triggering `BossStatus::Die`).
    pub killed: bool,
    /// Whether `YardratUndying` triggered an instant `590` drop roll without dying.
    pub yardrat_drop_proc: bool,
}

/// Evaluates incoming `raw_damage` (`i64` — fixing `DrKore.injured(int)`) deterministically.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn evaluate_boss_damage(
    boss_id: PlayerId, stage: usize, stats: &mut CoreStats, shield_active: &mut bool,
    rule: &DamageRule, archetype: &mut BossArchetype, attacker: Option<&PlayerView>,
    skill: SkillKind, raw_damage: i64, piercing: bool, bulons_alive: bool,
    sibling_alive: bool, parity: ParityMode, rng: &mut GameRng, effects: &mut Vec<EngineEffect>,
) -> DamageOutcome {
    if raw_damage <= 0 || stats.hp <= 0 {
        return zero_hit();
    }
    if matches!(archetype, BossArchetype::RedRibbonSpec { requires_bulons_dead: true, .. }) && bulons_alive {
        return zero_hit();
    }
    if let BossArchetype::AndroidKiAbsorb { heal_pct, is_dr_kore } = *archetype
        && skill.is_ki_blast()
        && !(is_dr_kore && parity == ParityMode::Legacy)
    {
        let heal = raw_damage.saturating_mul(i64::from(heal_pct)) / 100;
        stats.hp = stats.hp.saturating_add(heal).min(stats.hp_max);
        return DamageOutcome { applied_damage: 0, healed_hp: heal, killed: false, yardrat_drop_proc: false };
    }
    if let BossArchetype::MajinBuu12h { beam_immune_double_melee, self_destruct_immune, .. } = *archetype
        && ((self_destruct_immune && skill == SkillKind::TuSat)
            || (beam_immune_double_melee && (skill.is_ki_blast() || skill == SkillKind::LienHoan)))
    {
        return zero_hit();
    }

    let mut dmg = if matches!(archetype, BossArchetype::MajinBuu12h { beam_immune_double_melee: true, .. }) && skill.is_melee() {
        raw_damage.saturating_mul(2)
    } else {
        raw_damage
    };
    let mut healed_hp = 0_i64;
    let std_dodge = |r: &mut GameRng| !piercing && r.is_true(u32::from(stats.dodge), 1_000);

    match *rule {
        DamageRule::Invulnerable => return zero_hit(),
        DamageRule::FixedHit { fixed, counter_blind } => {
            if counter_blind && let Some(att) = attacker {
                effects.push(EngineEffect::DebuffPlayer { target: att.id, kind: PlayerDebuffKind::Stun, duration_ms: 3_000 });
            }
            dmg = fixed.min(stats.hp);
        }
        DamageRule::YardratUndying { reward_ratio } => {
            if !piercing && rng.is_true(10, 100) { return zero_hit(); }
            if !piercing { dmg = dmg.min((stats.hp_max / 100).max(1)); }
            if stats.hp <= dmg {
                stats.hp = 1;
                return DamageOutcome { applied_damage: dmg, healed_hp: 0, killed: false, yardrat_drop_proc: rng.is_true(1, reward_ratio.max(1)) };
            }
            stats.hp -= dmg;
            return DamageOutcome { applied_damage: dmg, healed_hp: 0, killed: false, yardrat_drop_proc: false };
        }
        DamageRule::BrolyGrowth { is_super } => {
            if std_dodge(rng) { return zero_hit(); }
            if rng.is_true(1, if is_super { 30 } else { 10 }) { healed_hp = broly_grow(stats, is_super, rng); }
            dmg = (dmg - i64::from(stats.def)).max(1);
            if !piercing && skill != SkillKind::TuSat { dmg = dmg.min((stats.hp_max / 100).max(1)); }
        }
        DamageRule::PreDefDivisor { pre_div, shield_div, max_hp_cap_denom } => {
            if std_dodge(rng) { return zero_hit(); }
            dmg = ((dmg / pre_div.max(1)) - i64::from(stats.def)).max(1);
            apply_shield(&mut dmg, stats.hp_max, shield_active, shield_div, piercing);
            if max_hp_cap_denom > 0 { dmg = dmg.min((stats.hp_max / max_hp_cap_denom).max(1)); }
        }
        DamageRule::BabyComposite => {
            if std_dodge(rng) { return zero_hit(); }
            dmg = (((dmg * 7 / 10) / 2) - i64::from(stats.def)).max(1);
            apply_shield(&mut dmg, stats.hp_max, shield_active, 4, piercing);
        }
        DamageRule::FlatSubShieldOne { flat_rand, stage1_half } => {
            if std_dodge(rng) { return zero_hit(); }
            if stage1_half && stage > 0 { dmg /= 2; }
            dmg = (dmg - i64::from(rng.next_int(0, flat_rand as i32)) - i64::from(stats.def)).max(1);
            apply_shield_one(&mut dmg, stats.hp_max, shield_active, piercing);
        }
        DamageRule::HardCap { max_damage, jitter, shield_one, legacy_dodge_div_one } => {
            let den = if legacy_dodge_div_one && parity == ParityMode::Legacy { 1 } else { 1_000 };
            if !piercing && rng.is_true(u32::from(stats.dodge), den) { return zero_hit(); }
            dmg = (dmg - i64::from(stats.def)).max(1);
            if shield_one { apply_shield_one(&mut dmg, stats.hp_max, shield_active, piercing); }
            let cap = (max_damage - i64::from(rng.next_int(0, jitter.max(0) as i32))).max(1);
            if !piercing { dmg = dmg.min(cap); }
        }
        DamageRule::KhiGasScaled { level, is_hatchiyack } => {
            if !piercing && rng.is_true(if is_hatchiyack { level + 10 } else { level }, 1_000) { return zero_hit(); }
            let flat_hi = ((if is_hatchiyack { 200 } else { 100 }) * level) as i32;
            let dr_pct = i64::from(if is_hatchiyack { level / 5 } else { level / 10 }).min(95);
            dmg = (dmg - i64::from(rng.next_int(0, flat_hi)) - i64::from(stats.def)).max(1);
            dmg = (dmg - dmg * dr_pct / 100).max(1);
            apply_shield_one(&mut dmg, stats.hp_max, shield_active, piercing);
        }
        DamageRule::WhisScaled { level } => {
            if !piercing && rng.is_true(400, 1_000) { return zero_hit(); }
            dmg = (dmg / i64::from(level.max(1))).max(1);
        }
        DamageRule::HatMitArena => {
            if !piercing && rng.is_true(100, 1_000) { return zero_hit(); }
            dmg = if attacker.is_some_and(|a| a.carrying_namek_ball) { 1 } else { dmg.min((stats.hp_max / 10).max(1)) };
        }
        DamageRule::PvpMirror { armor_pct, ki_absorb_pct, giap_xen_tier } => {
            if skill.is_ki_blast() && ki_absorb_pct > 0 {
                let heal = dmg * i64::from(ki_absorb_pct) / 100;
                stats.hp = (stats.hp + heal).min(stats.hp_max);
                return DamageOutcome { applied_damage: 0, healed_hp: heal, killed: false, yardrat_drop_proc: false };
            }
            if !piercing && rng.is_true(u32::from(stats.dodge.min(90)), 100) { return zero_hit(); }
            dmg -= dmg * (if piercing { 0 } else { i64::from(armor_pct.min(86)) }) / 100;
            if !piercing {
                dmg = (dmg - i64::from(stats.def)).max(1);
                if giap_xen_tier == 1 { dmg /= 2; } else if giap_xen_tier >= 2 { dmg = dmg * 40 / 100; }
                apply_shield_one(&mut dmg, stats.hp_max, shield_active, false);
            }
        }
        DamageRule::Standard => {
            if std_dodge(rng) { return zero_hit(); }
            dmg = (dmg - i64::from(stats.def)).max(1);
            apply_shield(&mut dmg, stats.hp_max, shield_active, 1, piercing);
        }
    }

    if let Some(att) = attacker {
        if matches!(archetype, BossArchetype::BeginnerTaoPaiPai) && att.power < 1_500_000 {
            effects.push(EngineEffect::GrantTnsm { target: att.id, amount: (dmg * i64::from(rng.next_int(20, 50))).min(9_500_000) });
        }
        if matches!(archetype, BossArchetype::TrainingMaster { task_10_1_gate: true, .. }) && att.active_task != (10, 1) {
            return DamageOutcome { applied_damage: 100, healed_hp: 0, killed: false, yardrat_drop_proc: false };
        }
        if let BossArchetype::TetLanCon { ref mut tamed_owner } = *archetype {
            let said_thang = att.last_chat.as_deref().is_some_and(|c| c.to_ascii_lowercase().contains("thang"));
            if dmg >= stats.hp || said_thang {
                *tamed_owner = Some(att.id);
                stats.hp = stats.hp_max;
                effects.push(EngineEffect::ChangePk { entity: boss_id, mode: PkMode::NonPk });
                return zero_hit();
            }
        }
        if let BossArchetype::PiccoloDivineWater { ref mut laser_fired } = *archetype && dmg >= stats.hp && !*laser_fired {
            *laser_fired = true;
            stats.hp = 1;
            effects.push(EngineEffect::DamagePlayer { target: att.id, damage: 1_000_000_000, piercing: true });
            effects.push(EngineEffect::UnlockSuperDivineWater { target: att.id });
            return zero_hit();
        }
    }

    match archetype {
        BossArchetype::AndroidTrio { member: 13, .. } if dmg >= stats.hp && sibling_alive => return zero_hit(),
        BossArchetype::AndroidTrio { member: 14 | 15, called_13 } if !*called_13 && dmg >= stats.hp => {
            *called_13 = true;
            stats.hp = stats.hp_max;
            return zero_hit();
        }
        BossArchetype::SuperCellSwarm { spawned_jrs } if stage == 0 && !*spawned_jrs && dmg >= stats.hp => {
            *spawned_jrs = true;
            stats.hp = stats.hp_max;
            return zero_hit();
        }
        BossArchetype::MajinBuu14h { is_belly_super_bu: false } if stage == 4 && dmg >= stats.hp && skill != SkillKind::QuaCauKenhKhi => {
            return zero_hit();
        }
        _ => {}
    }

    dmg = dmg.min(stats.hp);
    stats.hp -= dmg;
    if let BossArchetype::SnakeWaySpec { oozaru_available, .. } = archetype
        && *oozaru_available
        && stats.hp > 0
        && stats.hp < stats.hp_max / 2
    {
        *oozaru_available = false;
        stats.hp_max = stats.hp_max.saturating_mul(2).min(2_000_000_000);
        stats.hp = stats.hp_max;
    }
    DamageOutcome { applied_damage: dmg, healed_hp, killed: stats.hp <= 0, yardrat_drop_proc: false }
}

/// Grows Broly or Super Broly `hp_max` & `dame` (`tangChiSo`), capping at `16,070,777`.
pub fn broly_grow(stats: &mut CoreStats, is_super: bool, rng: &mut GameRng) -> i64 {
    let div = i64::from(rng.next_int(if is_super { 80 } else { 10 }, 100));
    let next_max = (stats.hp_max + (stats.hp_max / div).max(1)).min(16_070_777);
    let delta = next_max - stats.hp_max;
    stats.hp_max = next_max;
    stats.hp = (stats.hp + delta).min(stats.hp_max);
    stats.dame = if is_super { (stats.hp_max / 10).min(2_000_000) } else { stats.hp_max / 10 };
    delta
}

/// Steals gold for `AnTrom`, fixing the unreachable billionaire branch in [`ParityMode::Fixed`].
pub fn steal_thief_gold(
    stolen_total: &mut u64, victim: &PlayerView, boss_loc: Location, parity: ParityMode, rng: &mut GameRng,
) -> u64 {
    if *stolen_total >= 10_000_000_000 || loc_distance(boss_loc, victim.location) > 40 {
        return 0;
    }
    let raw = if parity == ParityMode::Fixed && victim.gold >= 1_000_000_000 {
        rng.next_int(1_000_000, 5_000_000) as u64
    } else if victim.gold >= 2_000_000 {
        rng.next_int(200_000, 1_000_000) as u64
    } else if victim.gold >= 1_000_000 {
        rng.next_int(1_000, 2_000) as u64
    } else {
        0
    };
    let stolen = raw.min(victim.gold).min(10_000_000_000 - *stolen_total);
    *stolen_total += stolen;
    stolen
}

/// Clan dungeon kind for dynamic boss and mob stat calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DungeonKind {
    /// Doanh Trại Độc Nhãn (`RedRibbonHQ`, `mult_pct = 100 | 110 | 115 | 120 | 130`).
    RedRibbon { mult_pct: u16 },
    /// Bản Đồ Kho Báu (`TrungUyXanhLo` Map `137`).
    TreasureMap,
    /// Khí Gas Hủy Diệt (`DrLychee` `1.0x` -> `Hatchiyack` `1.5x`).
    DestronGas { is_hatchiyack: bool },
    /// Con Đường Rắn Độc (`role_mult = 1` Saibamen, `5` Nadic, `50` Cadich).
    SnakeWay { role_mult: u16 },
}

/// Input parameters for clan/solo dungeon boss & mob stat scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DungeonParams {
    /// Dungeon level (`1..=110` for BĐKB, Khí Gas, and CDRĐ).
    pub level: u32,
    /// Sum of online clan members' base damage (`RedRibbonHQ`).
    pub clan_total_dame: i64,
    /// Sum of online clan members' max HP (`RedRibbonHQ`).
    pub clan_total_hp: i64,
}

/// Computes exact `(dame, hp_max)` for all 4 instanced clan dungeons without `i32` overflow.
#[must_use]
pub fn scale_dungeon_boss(kind: DungeonKind, params: DungeonParams) -> (i64, i64) {
    let lv = i64::from(params.level.clamp(1, 110));
    match kind {
        DungeonKind::RedRibbon { mult_pct } => {
            let m = i64::from(mult_pct);
            let bd = (params.clan_total_hp.max(20) / 20).min(200_000_000);
            let bh = params.clan_total_dame.max(1).saturating_mul(50).min(2_147_483_647);
            ((bd.saturating_mul(m) / 100).clamp(1, 200_000_000), (bh.saturating_mul(m) / 100).clamp(1, 2_147_483_647))
        }
        DungeonKind::TreasureMap => ((200_000_i64 * lv).min(200_000_000), (20_000_000_i64 * lv).min(2_000_000_000)),
        DungeonKind::DestronGas { is_hatchiyack } => {
            let (d, h) = (10_000 + (1_000_i64 * lv).min(200_000_000), 1_000_000 + (15_000_000_i64 * lv).min(2_000_000_000));
            if is_hatchiyack { ((d * 3 / 2).min(200_000_000), (h * 3 / 2).min(2_000_000_000)) } else { (d, h) }
        }
        DungeonKind::SnakeWay { role_mult } => {
            let m = i64::from(role_mult.max(1));
            let (bd, bh) = (10_000 + (200_000_i64 * lv).min(200_000_000), 500_000 + (2_000_000_i64 * lv).min(2_000_000_000));
            (bd.saturating_mul(m).min(200_000_000), bh.saturating_mul(m).min(2_000_000_000))
        }
    }
}

fn apply_shield(dmg: &mut i64, hp_max: i64, active: &mut bool, div: i64, piercing: bool) {
    if !piercing && *active {
        if *dmg > hp_max { *active = false; }
        *dmg = (*dmg / div.max(1)).max(1);
    }
}

fn apply_shield_one(dmg: &mut i64, hp_max: i64, active: &mut bool, piercing: bool) {
    if !piercing && *active {
        if *dmg > hp_max { *active = false; }
        *dmg = 1;
    }
}

const fn zero_hit() -> DamageOutcome {
    DamageOutcome { applied_damage: 0, healed_hp: 0, killed: false, yardrat_drop_proc: false }
}
