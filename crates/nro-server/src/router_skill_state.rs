//! Per-player skill buff state (`TTNL`, `Shield`, `Biến Khỉ`, `Huýt Sáo`) and `SKILL_NOT_FOCUS` wire builder.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::PlayerId;
use nro_player::{
    npoint::NPoint,
    skills::{EffectSkillState, apply_huyt_sao_target},
};
use nro_protocol::{Packet, PacketWriter, SkillSpecialArgs};

use crate::runtime::unix_now_ms;

static TTNL_STATE: OnceLock<DashMap<PlayerId, (i64, u8)>> = OnceLock::new();
pub(super) fn ttnl_state() -> &'static DashMap<PlayerId, (i64, u8)> {
    TTNL_STATE.get_or_init(DashMap::new)
}
static SHIELD_UNTIL: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
pub(super) fn shield_until() -> &'static DashMap<PlayerId, i64> {
    SHIELD_UNTIL.get_or_init(DashMap::new)
}
static MONKEY_UNTIL: OnceLock<DashMap<PlayerId, (u8, i64)>> = OnceLock::new();
pub(super) fn monkey_until() -> &'static DashMap<PlayerId, (u8, i64)> {
    MONKEY_UNTIL.get_or_init(DashMap::new)
}
static MONKEY_REVERT_PENDING: OnceLock<DashMap<PlayerId, ()>> = OnceLock::new();
pub(super) fn monkey_revert_pending() -> &'static DashMap<PlayerId, ()> {
    MONKEY_REVERT_PENDING.get_or_init(DashMap::new)
}
static HUYT_SAO_UNTIL: OnceLock<DashMap<PlayerId, (i32, i64)>> = OnceLock::new();
pub(super) fn huyt_sao_until() -> &'static DashMap<PlayerId, (i32, i64)> {
    HUYT_SAO_UNTIL.get_or_init(DashMap::new)
}

pub(in crate::router) fn is_shield_active(pid: PlayerId, now_ms: i64) -> bool {
    shield_until()
        .get(&pid)
        .is_some_and(|t| *t.value() > now_ms)
}
pub(in crate::router) fn is_monkey_active(pid: PlayerId, now_ms: i64) -> bool {
    monkey_until()
        .get(&pid)
        .is_some_and(|v| v.value().1 > now_ms)
}

pub(in crate::router) fn clear_shield(pid: PlayerId) {
    shield_until().remove(&pid);
}
pub(in crate::router) fn clear_ttnl(pid: PlayerId) {
    ttnl_state().remove(&pid);
}
pub(in crate::router) fn take_monkey_revert_pending(pid: PlayerId) -> bool {
    monkey_revert_pending().remove(&pid).is_some()
}

pub(in crate::router) fn clear_skill_state(pid: PlayerId) -> bool {
    ttnl_state().remove(&pid);
    shield_until().remove(&pid);
    let had_monkey = monkey_until().remove(&pid).is_some();
    if had_monkey {
        monkey_revert_pending().insert(pid, ());
    }
    let had_huyt_sao = huyt_sao_until().remove(&pid).is_some();
    had_monkey || had_huyt_sao
}

pub(super) fn prune_expired_skill_buffs_detail(pid: PlayerId, now_ms: i64) -> (bool, bool) {
    let monkey_expired = monkey_until()
        .get(&pid)
        .is_some_and(|v| v.value().1 <= now_ms);
    if monkey_expired {
        monkey_until().remove(&pid);
    }
    let huyt_expired = huyt_sao_until()
        .get(&pid)
        .is_some_and(|v| v.value().1 <= now_ms);
    if huyt_expired {
        huyt_sao_until().remove(&pid);
    }
    (monkey_expired || huyt_expired, monkey_expired)
}

pub(in crate::router) fn prune_expired_skill_buffs(pid: PlayerId, now_ms: i64) -> bool {
    let (any, monkey_expired) = prune_expired_skill_buffs_detail(pid, now_ms);
    if monkey_expired {
        monkey_revert_pending().insert(pid, ());
    }
    any
}

pub(in crate::router) fn apply_skill_buffs_before_clamp(
    pid: PlayerId,
    stats: &mut nro_core::CoreStats,
) {
    let (now_ms, max_i32) = (unix_now_ms(), i64::from(i32::MAX));
    if let Some((pt, until)) = monkey_until().get(&pid).map(|v| *v.value()) {
        if until > now_ms {
            let lvl = i64::from(pt.clamp(1, 7));
            stats.hp_max = stats
                .hp_max
                .saturating_add(stats.hp_max.saturating_mul((lvl + 3) * 10) / 100)
                .clamp(1, max_i32);
            stats.dame = stats
                .dame
                .saturating_add(stats.dame.saturating_mul(lvl + 3) / 100)
                .clamp(0, max_i32);
            stats.crit = 110;
        } else {
            monkey_until().remove(&pid);
            monkey_revert_pending().insert(pid, ());
        }
    }
    if let Some((pct, until)) = huyt_sao_until().get(&pid).map(|v| *v.value()) {
        if until > now_ms && pct > 0 {
            stats.hp_max = stats
                .hp_max
                .saturating_add(stats.hp_max.saturating_mul(i64::from(pct)) / 100)
                .clamp(1, max_i32);
        } else {
            huyt_sao_until().remove(&pid);
        }
    }
}

pub(super) fn build_skill_not_focus_pkt(
    status: i8,
    pid: i32,
    skill_paint_id: i16,
    special: Option<SkillSpecialArgs>,
) -> Packet {
    let mut w = PacketWriter::with_capacity(20);
    let wire_status = match status {
        2 => 1,
        9 | 10 => 6,
        20 if special.is_some() => 20,
        0 | 1 | 3 | 4 | 5 | 6 | 7 | 8 => status,
        _ => 0,
    };
    w.write_i8(wire_status);
    w.write_i32(pid);
    w.write_i16(skill_paint_id);
    match wire_status {
        0 => {
            w.write_i8(0);
            w.write_i8(0);
        }
        4 | 7 => w.write_i16(3_000),
        20 => {
            w.write_i8(0);
            w.write_i8(special.map_or(1_i8, |a| a.dir));
            w.write_i16(1_500);
            w.write_i8(0);
            w.write_i8(0);
            w.write_i8(-1);
        }
        _ => {}
    }
    w.finish(cmd_message::SKILL_NOT_FOCUS)
}

pub(super) fn stats_to_npoint(stats: nro_core::CoreStats) -> NPoint {
    let mut np = NPoint::new(stats);
    np.hp_max = i32::try_from(stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp_max = i32::try_from(stats.mp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.hp = i32::try_from(stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp = i32::try_from(stats.mp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np
}

pub(super) fn apply_huyt_sao_to_snapshot(
    sid: u64,
    snap: &mut nro_db::player_repo::PlayerSnapshot,
    pt: u8,
    now_ms: i64,
    parity: nro_core::ParityMode,
) -> bool {
    let (pid, gender) = (snap.id, snap.gender);
    let already_active = huyt_sao_until()
        .get(&pid)
        .is_some_and(|t| t.value().1 > now_ms);
    let (pruned, monkey_expired) = prune_expired_skill_buffs_detail(pid, now_ms);
    let monkey_ended = monkey_expired || monkey_revert_pending().remove(&pid).is_some();
    if pruned {
        super::super::recalculate_session_stats(sid, snap, parity);
    }
    let mut np = stats_to_npoint(snap.stats);
    let mut eff = EffectSkillState::default();
    apply_huyt_sao_target(&mut np, &mut eff, gender, pt, now_ms.max(0).cast_unsigned());
    if eff.huyt_sao_pct > 0 {
        huyt_sao_until().insert(pid, (eff.huyt_sao_pct, now_ms.saturating_add(30_000)));
        super::super::recalculate_session_stats(sid, snap, parity);
        if !already_active {
            snap.stats.hp = i64::from(np.hp).min(snap.stats.hp_max);
        }
    } else {
        snap.stats.hp = i64::from(np.hp).clamp(0, snap.stats.hp_max);
    }
    monkey_ended
}
