//! Per-player CC/debuff state (`EffectSkillState`), `TuSat` charge tracking,
//! `KHANG_TDHS` immunity, and periodic buff/effect expiration ticks.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_boss::PlayerDebuffKind;
use nro_core::{ItemInstance, Location, PlayerId};
use nro_player::skills::{CcFlags, EffectSkillState, cc_durations_ms, tdhs_stun_ms_and_range};
use nro_world::{ZoneCommand, s2c_packets};

use super::ServerSessionHandler;
use super::router_combat::is_offline_map;
use super::router_combat_calc::in_range_px;
use super::router_combat_pvp::lock_pvp;
use crate::runtime::unix_now_ms;

static PLAYER_EFFECTS: OnceLock<DashMap<PlayerId, EffectSkillState>> = OnceLock::new();

pub(super) fn player_effects() -> &'static DashMap<PlayerId, EffectSkillState> {
    PLAYER_EFFECTS.get_or_init(DashMap::new)
}

/// Clears all active CC, debuff, and 2-stage preparation state for `pid`.
pub(crate) fn clear_player_effects(pid: PlayerId) {
    player_effects().remove(&pid);
}

/// Returns `true` if `items_body` contains Option `116` (`KHANG_TDHS`).
pub(super) fn has_khang_tdhs(items_body: &[Option<ItemInstance>]) -> bool {
    items_body
        .iter()
        .flatten()
        .flat_map(|it| it.options.iter())
        .any(|o| o.id == 116)
}

/// Returns `true` if `pid` currently has an active action-blocking CC flag.
pub(super) fn is_player_cc_locked(pid: PlayerId, now_ms: i64) -> bool {
    let now_u64 = now_ms.max(0).cast_unsigned();
    let Some(mut eff) = player_effects().get_mut(&pid) else {
        return false;
    };
    let _ = eff.tick(now_u64, i32::MAX);
    eff.is_locked()
}

/// Records `TuSat` (`status == 4`) charge start timestamp (`ms`).
pub(super) fn record_tu_sat_prep(pid: PlayerId, now_ms: i64) {
    player_effects().entry(pid).or_default().stage_prep.2 = Some(now_ms.max(0).cast_unsigned());
}

/// Consumes `TuSat` charge timestamp on `status == 7` and validates elapsed charge (`1.5s..=15s`).
pub(super) fn take_valid_tu_sat_prep(pid: PlayerId, now_ms: i64) -> bool {
    let now_u64 = now_ms.max(0).cast_unsigned();
    let Some(mut eff) = player_effects().get_mut(&pid) else {
        return false;
    };
    eff.stage_prep.2.take().is_some_and(|started| {
        let elapsed = now_u64.saturating_sub(started);
        (1_500..=15_000).contains(&elapsed)
    })
}

/// Applies a timed [`CcFlags`] lock into `PLAYER_EFFECTS` at expiry slot `slot_idx` (`0..6`).
pub(crate) fn apply_player_cc(
    pid: PlayerId,
    flag: CcFlags,
    slot_idx: usize,
    duration_ms: u64,
    now_ms: i64,
) {
    if duration_ms == 0 || slot_idx >= 6 {
        return;
    }
    let until = now_ms.max(0).cast_unsigned().saturating_add(duration_ms);
    let mut eff = player_effects().entry(pid).or_default();
    eff.cc.insert(flag);
    eff.cc_until_ms[slot_idx] = eff.cc_until_ms[slot_idx].max(until);
}

/// Applies a boss [`PlayerDebuffKind`] to `pid`, honoring `KHANG_TDHS` for `Stun`.
/// Returns `false` if resisted.
pub(crate) fn apply_boss_debuff_to_player(
    pid: PlayerId,
    kind: PlayerDebuffKind,
    duration_ms: u64,
    items_body: &[Option<ItemInstance>],
    now_ms: i64,
) -> bool {
    if matches!(kind, PlayerDebuffKind::Stun) && has_khang_tdhs(items_body) {
        return false;
    }
    let now_u64 = now_ms.max(0).cast_unsigned();
    match kind {
        PlayerDebuffKind::Stun => apply_player_cc(pid, CcFlags::STUN, 0, duration_ms, now_ms),
        PlayerDebuffKind::PetrifyStone => {
            apply_player_cc(pid, CcFlags::STONE, 4, duration_ms, now_ms);
        }
        PlayerDebuffKind::Chocolate => {
            let mut eff = player_effects().entry(pid).or_default();
            eff.aux_until_ms.0 = eff.aux_until_ms.0.max(now_u64.saturating_add(duration_ms));
        }
        PlayerDebuffKind::BodyChange
        | PlayerDebuffKind::GravitySlow
        | PlayerDebuffKind::HalloweenCurse(_)
        | PlayerDebuffKind::Contagion { .. } => {}
    }
    true
}

fn is_effect_state_idle(eff: &EffectSkillState) -> bool {
    !eff.is_locked()
        && eff.shield_until_ms == 0
        && eff.monkey_level == 0
        && eff.huyt_sao_pct == 0
        && eff.aux_until_ms == (0, 0)
        && eff.mafuba_jar.is_none()
        && eff.stage_prep == (None, None, None)
        && eff.mob_me.is_none()
}

impl ServerSessionHandler {
    /// Applies PvP CC effects from non-targeted skill casts (`TDHS`, `Thôi Miên`, `Trói`, `Socola`).
    pub(super) fn apply_cast_cc_targets(
        &self,
        caster_sid: u64,
        caster_pid: PlayerId,
        caster_flag: i8,
        loc: Location,
        sel: i16,
        pt_u8: u8,
        now_ms: i64,
    ) {
        if is_offline_map(loc.map_id) || !matches!(sel, 6 | 18 | 22 | 23) {
            return;
        }
        let (_, sleep_ms, troi_ms) = cc_durations_ms(pt_u8, false);
        let (tdhs_ms, tdhs_range) = tdhs_stun_ms_and_range(pt_u8, false);
        let max_range_px = if sel == 6 {
            i64::from(tdhs_range) + 80
        } else {
            380
        };
        let duel_opp = lock_pvp().get(&caster_pid).map(|&(opp, _)| opp);
        let target_sids: Vec<u64> = self
            .shared
            .clients
            .authenticated
            .iter()
            .filter_map(|e| {
                let t_sid = *e.key();
                if t_sid == caster_sid || e.snapshot.stats.hp <= 0 {
                    return None;
                }
                let tloc = e.snapshot.location;
                if tloc.map_id != loc.map_id
                    || tloc.zone_id != loc.zone_id
                    || !in_range_px(loc, tloc, max_range_px)
                {
                    return None;
                }
                let pvp_ok = (caster_flag > 0
                    && e.c_flag > 0
                    && (caster_flag != e.c_flag || caster_flag == 8))
                    || duel_opp == Some(e.snapshot.id);
                pvp_ok.then_some(t_sid)
            })
            .collect();
        for t_sid in target_sids {
            let Some(target) = self.shared.clients.authenticated.get(&t_sid) else {
                continue;
            };
            let tpid = target.snapshot.id;
            match sel {
                6 if !has_khang_tdhs(&target.snapshot.items_body) => {
                    apply_player_cc(tpid, CcFlags::STUN, 0, tdhs_ms, now_ms);
                }
                22 => apply_player_cc(tpid, CcFlags::THOI_MIEN, 3, sleep_ms, now_ms),
                23 => apply_player_cc(tpid, CcFlags::AN_TROI, 2, troi_ms, now_ms),
                18 => {
                    let until = now_ms.max(0).cast_unsigned().saturating_add(15_000);
                    let mut eff = player_effects().entry(tpid).or_default();
                    eff.aux_until_ms.0 = eff.aux_until_ms.0.max(until);
                }
                _ => {}
            }
            if matches!(sel, 22 | 23) {
                break;
            }
        }
    }

    /// Periodic tick (`lot_iv`) pruning expired `SESSION_BUFFS`, `Biến Khỉ`/`Huýt Sáo`, and `PLAYER_EFFECTS`.
    pub(crate) async fn tick_expired_buffs(&self) {
        let now_ms = unix_now_ms();
        let now_u64 = now_ms.max(0).cast_unsigned();
        let sids: Vec<u64> = self
            .shared
            .clients
            .authenticated
            .iter()
            .map(|e| *e.key())
            .collect();
        for sid in sids {
            let revert_info =
                self.shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .and_then(|mut entry| {
                        let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                        let hp_i32 =
                            i32::try_from(entry.snapshot.stats.hp.clamp(0, i64::from(i32::MAX)))
                                .unwrap_or(i32::MAX);
                        let idle = if let Some(mut eff) = player_effects().get_mut(&pid) {
                            let _ = eff.tick(now_u64, hp_i32);
                            is_effect_state_idle(&eff)
                        } else {
                            false
                        };
                        if idle {
                            player_effects().remove_if(&pid, |_, e| is_effect_state_idle(e));
                        }
                        let sess_pruned =
                            super::router_economy::prune_expired_session_buffs(sid, now_ms);
                        let skill_pruned =
                            super::router_skill::prune_expired_skill_buffs(pid, now_ms);
                        let monkey_rev = super::router_skill::take_monkey_revert_pending(pid);
                        if sess_pruned || skill_pruned {
                            super::recalculate_session_stats(
                                sid,
                                &mut entry.snapshot,
                                self.shared.config.parity_mode,
                            );
                            if let (Some(ek), Ok(tx)) = (
                                entry.entity_key,
                                self.shared
                                    .world_router
                                    .resolve_sender(loc.map_id, loc.zone_id),
                            ) {
                                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(
                                    ek,
                                    entry.snapshot.stats,
                                ));
                            }
                            if entry.handle.sent_key() {
                                let _ = entry
                                    .handle
                                    .try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
                            }
                        }
                        monkey_rev.then(|| (loc, self.revert_monkey_appearance_entry(&entry, loc)))
                    });
            if let Some((loc, rpkts)) = revert_info {
                for p in rpkts.into_iter().flatten() {
                    self.broadcast_zone_or_self(loc, sid, &p);
                }
            }
        }
    }
}
