//! Outgoing attack damage calculation (`NPoint::get_dame_attack`), incoming-hit mitigation,
//! lifesteal, range validation, and monotonic per-cast RNG seeding.

use std::sync::atomic::{AtomicU64, Ordering};

use nro_core::{CoreStats, GameRng, ItemInstance, Location, ParityMode, PlayerId};
use nro_economy::BuffSlot;
use nro_player::{
    combat::{AttackCalcRequest, IncomingHit, InjuryOutcome},
    npoint::NPoint,
    set_clothes::SetClothes,
    skills::{RANGE_BEAM_PX, RANGE_MELEE_PX, SkillKind},
};

use super::router_economy::session_buffs;
use super::router_skill::{clear_shield, is_monkey_active, is_shield_active};
use crate::state::ActivePlayerEntry;

static CAST_RNG_NONCE: AtomicU64 = AtomicU64::new(1);

/// Creates a per-cast deterministic [`GameRng`] seeded with a monotonic atomic nonce.
pub(super) fn next_combat_rng(now_ms: i64, sid: u64, salt: u64) -> GameRng {
    let nonce = CAST_RNG_NONCE.fetch_add(1, Ordering::Relaxed);
    let seed = now_ms.cast_unsigned()
        ^ sid.rotate_left(17)
        ^ salt.rotate_left(31)
        ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    GameRng::from_seed(seed)
}

/// Computes `(max_range_px)` for `skill` at `skill_point` including `+80px` latency slack.
#[inline]
pub(super) const fn skill_max_range_px(skill: Option<SkillKind>, skill_point: u8) -> i64 {
    const SLACK_PX: i64 = 80;
    let base = match skill {
        Some(sk) if sk.is_melee() => RANGE_MELEE_PX as i64,
        Some(sk) if sk.is_beam() => RANGE_BEAM_PX as i64,
        Some(SkillKind::QuaCauKenhKhi) => 350 + (skill_point as i64) * 30,
        Some(SkillKind::TuSat) => 400 + (skill_point as i64) * 30,
        Some(SkillKind::ThaiDuongHaSan) => 120 + (skill_point as i64) * 30,
        Some(SkillKind::TriThuong | SkillKind::HuytSao) => 350,
        Some(
            SkillKind::Makankosappo
            | SkillKind::SuperKame
            | SkillKind::LienHoanChuong
            | SkillKind::MaPhongBa,
        ) => 500,
        Some(SkillKind::DichChuyenTucThoi) => 600,
        Some(_) | None => 300,
    };
    base + SLACK_PX
}

/// Returns `true` if `a` and `b` are within `max_range_px` Euclidean distance.
#[inline]
pub(super) const fn in_range_px(a: Location, b: Location, max_range_px: i64) -> bool {
    let dx = a.x as i64 - b.x as i64;
    let dy = a.y as i64 - b.y as i64;
    dx * dx + dy * dy <= max_range_px * max_range_px
}

/// Extracts attacker penetration `(opt 18, opt 98, opt 99)` and lifesteal `(opt 95, opt 96, opt 104)`.
pub(super) fn attacker_pen_and_lifesteal(
    items_body: &[Option<ItemInstance>],
) -> ((i32, i32, i32), (i32, i32, i32)) {
    let (mut pen, mut drain) = ((0_i32, 0_i32, 0_i32), (0_i32, 0_i32, 0_i32));
    for opt in items_body.iter().flatten().flat_map(|it| it.options.iter()) {
        match opt.id {
            18 => pen.0 = pen.0.saturating_add(opt.param),
            98 => pen.1 = pen.1.saturating_add(opt.param),
            99 => pen.2 = pen.2.saturating_add(opt.param),
            95 => drain.0 = drain.0.saturating_add(opt.param),
            96 => drain.1 = drain.1.saturating_add(opt.param),
            104 => drain.2 = drain.2.saturating_add(opt.param),
            _ => {}
        }
    }
    (pen, drain)
}

/// Computes `(mob_dmg, pvp_dmg, is_crit)` via [`NPoint::get_dame_attack`].
pub(super) fn compute_attack_damage(
    entry: &ActivePlayerEntry,
    sid: u64,
    skill: SkillKind,
    skill_damage_pct: i32,
    skill_point: u8,
    now_ms: i64,
    parity: ParityMode,
) -> (u64, u64, bool) {
    let stats = entry.snapshot.stats;
    let mut np = NPoint::new(stats);
    np.hp_max = i32::try_from(stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.hp = i32::try_from(stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp_max = i32::try_from(stats.mp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp = i32::try_from(stats.mp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.dame = i32::try_from(stats.dame.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.def = stats.def.max(0);
    np.crit = i32::from(stats.crit);
    for opt in entry.snapshot.items_body.iter().flatten().flat_map(|it| it.options.iter()) {
        match opt.id {
            5 => np.mods.tl_sdcm = np.mods.tl_sdcm.saturating_add(opt.param),
            19 => np.mods.tl_dame_att_mob.push(opt.param),
            _ => {}
        }
    }
    let pet_body = entry
        .snapshot
        .pet
        .as_ref()
        .filter(|p| p.fusion_type != 0)
        .map(|p| p.items_body.as_slice());
    let sets = SetClothes::recompute(&entry.snapshot.items_body, pet_body, false);
    let intr = &entry.snapshot.intrinsic;
    let intrinsic = (
        u8::try_from(intr.intrinsic_id.max(0)).unwrap_or(0),
        i32::from(intr.param1),
    );
    let is_monkey = is_monkey_active(entry.snapshot.id, now_ms);
    let mut rng = next_combat_rng(now_ms, sid, u64::from(skill as u8));
    let pvp_req = AttackCalcRequest {
        skill,
        skill_damage_pct,
        skill_point,
        intrinsic,
        sets,
        combat_flags: (false, false, is_monkey),
        sexy_dame_buff_pct: 0,
        qckk_surrounding_hp: (0, 0),
        parity,
    };
    let mut np_mob = np.clone();
    let pvp_raw = np.get_dame_attack(&pvp_req, &mut rng);
    let is_crit = np.combat_state.0;
    if is_crit {
        np_mob.combat_state.1 = true;
    } else {
        np_mob.crit = 0;
    }
    let mob_req = AttackCalcRequest {
        skill,
        skill_damage_pct,
        skill_point,
        intrinsic,
        sets,
        combat_flags: (true, false, is_monkey),
        sexy_dame_buff_pct: 0,
        qckk_surrounding_hp: (0, 0),
        parity,
    };
    let mob_raw = np_mob.get_dame_attack(&mob_req, &mut rng);
    let charm_pct =
        u64::try_from(entry.snapshot.charms.damage_multiplier_pct(now_ms).max(100)).unwrap_or(100);
    let mob_u64 = u64::try_from(mob_raw.max(1)).unwrap_or(1);
    let mob_dmg = (mob_u64.saturating_mul(charm_pct) / 100).max(1);
    let pvp_dmg = u64::try_from(pvp_raw.max(1)).unwrap_or(1);
    (mob_dmg, pvp_dmg, is_crit)
}

/// Applies HP/MP lifesteal (`opt 95/96/104`) on a non-dodged hit, returning `true` if stats changed.
pub(super) fn apply_lifesteal(
    stats: &mut CoreStats,
    dealt_dmg: i64,
    is_mob: bool,
    drains: (i32, i32, i32),
) -> bool {
    if dealt_dmg <= 0 || stats.hp <= 0 {
        return false;
    }
    let hp_pct = drains.0.saturating_add(if is_mob { drains.2 } else { 0 }).max(0);
    let mp_pct = drains.1.max(0);
    let hp_gain = dealt_dmg.saturating_mul(i64::from(hp_pct)) / 100;
    let mp_gain = dealt_dmg.saturating_mul(i64::from(mp_pct)) / 100;
    let (old_hp, old_mp) = (stats.hp, stats.mp);
    if hp_gain > 0 {
        stats.hp = stats.hp.saturating_add(hp_gain).min(stats.hp_max.max(1));
    }
    if mp_gain > 0 {
        stats.mp = stats.mp.saturating_add(mp_gain).min(stats.mp_max.max(1));
    }
    stats.hp != old_hp || stats.mp != old_mp
}

/// Shared incoming-hit mitigation pipeline (`Giáp Xên`, `piercing`, `attacker_pen`, shield, reflect).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_incoming_hit_to_player(
    target_sid: u64,
    target_pid: PlayerId,
    stats: &mut CoreStats,
    items_body: &[Option<ItemInstance>],
    raw_damage: i64,
    attacker_skill: Option<SkillKind>,
    attacker_pen: (i32, i32, i32),
    force_piercing: bool,
    now_ms: i64,
) -> (InjuryOutcome, i64) {
    let (gx1, gx2) = session_buffs().get(&target_sid).map_or((false, false), |b| {
        (
            b.has_active(now_ms, |s| s == BuffSlot::GiapXen(false)),
            b.has_active(now_ms, |s| s == BuffSlot::GiapXen(true)),
        )
    });
    let piercing = force_piercing
        || matches!(attacker_skill, Some(SkillKind::Makankosappo | SkillKind::TuSat));
    let mut np = NPoint::new(*stats);
    np.hp_max = i32::try_from(stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.hp = i32::try_from(stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.def = stats.def.max(0);
    for opt in items_body.iter().flatten().flat_map(|it| it.options.iter()) {
        let ps = opt.param.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        match opt.id {
            3 => np.mods.vo_hieu_chuong = np.mods.vo_hieu_chuong.saturating_add(opt.param),
            94 => np.mods.tl_giap = np.mods.tl_giap.saturating_add(ps),
            97 => np.mods.utils[0] = np.mods.utils[0].saturating_add(ps),
            108 | 111 => np.mods.tl_ne_don = np.mods.tl_ne_don.saturating_add(ps),
            _ => {}
        }
    }
    let mut rng = next_combat_rng(now_ms, target_sid, target_pid.0.cast_unsigned());
    let shield = is_shield_active(target_pid, now_ms);
    let out = np.apply_injured(
        &IncomingHit {
            raw_damage: raw_damage.max(0),
            attacker_skill,
            attacker_pen,
            hit_flags: (piercing, false, false),
            defense_state: (gx1, gx2, shield, false, false),
        },
        &mut rng,
    );
    if out.shield_broken {
        clear_shield(target_pid);
    }
    let final_dmg = if out.dodged { 0 } else { i64::from(out.damage_taken.max(0)) };
    if !out.dodged {
        stats.hp = stats.hp.saturating_sub(final_dmg).max(0);
    }
    if out.mp_absorbed > 0 {
        stats.mp = stats.mp.saturating_add(i64::from(out.mp_absorbed)).min(stats.mp_max.max(1));
    }
    let reflect_pct = i64::from(np.mods.utils[0].max(0));
    let reflect_dmg = if !piercing && out.reflect_base_damage > 0 && reflect_pct > 0 {
        (i64::from(out.reflect_base_damage).saturating_mul(reflect_pct) / 100).max(1)
    } else {
        0
    };
    (out, reflect_dmg)
}
