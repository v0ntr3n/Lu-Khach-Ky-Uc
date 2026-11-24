//! Non-targeted skill handler (`cmd -45`, `SKILL_NOT_FOCUS`) matching `Controller.cs:2071-2350`.

#[path = "router_skill_ally.rs"]
mod router_skill_ally;
#[path = "router_skill_state.rs"]
mod router_skill_state;

use router_skill_state::{
    apply_huyt_sao_to_snapshot, build_skill_not_focus_pkt, monkey_revert_pending, monkey_until,
    prune_expired_skill_buffs_detail, shield_until, stats_to_npoint, ttnl_state,
};
pub(super) use router_skill_state::{
    apply_skill_buffs_before_clamp, clear_shield, clear_skill_state, clear_ttnl, is_monkey_active,
    is_shield_active, prune_expired_skill_buffs, take_monkey_revert_pending,
};

use nro_core::{EntityKey, Location, PlayerId};
use nro_player::skills::{
    apply_tri_thuong_target, monkey_duration_ms, shield_duration_ms, utility_skill_params,
};
use nro_protocol::{Packet, ProtocolError, SessionHandle, SkillSpecialArgs};
use nro_world::{ZoneCommand, s2c_packets};

use super::{ServerSessionHandler, effective_appearance, send_thong_bao, sprite_restore_packets};
use crate::runtime::{ActivePlayerEntry, unix_now_ms};

const MONKEY_HEADS: [i16; 7] = [192, 195, 196, 199, 197, 200, 198];
const MAX_TTNL_TICKS: u8 = 10;

impl ServerSessionHandler {
    pub(crate) fn clear_ttnl_for_player(pid: PlayerId) {
        clear_ttnl(pid);
    }

    pub(super) fn revert_monkey_appearance_entry(
        &self,
        entry: &ActivePlayerEntry,
        loc: Location,
    ) -> [Option<Packet>; 2] {
        let (eh, eb, el, _, is_disg) =
            effective_appearance(&self.shared.templates, &entry.snapshot);
        if let (Some(ek), Ok(tx)) = (
            entry.entity_key,
            self.shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id),
        ) {
            let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
        }
        if entry.handle.sent_key() {
            let _ = entry
                .handle
                .try_send(s2c_packets::inventory_body(eh, &entry.snapshot.items_body));
        }
        sprite_restore_packets(entry.snapshot.id, eh, eb, el, is_disg)
    }

    pub(super) async fn handle_skill_not_focus(
        &self,
        session: &SessionHandle,
        sid: u64,
        status: i8,
        special: Option<SkillSpecialArgs>,
    ) -> Result<(), ProtocolError> {
        let mut pruned_sync: Option<(
            Location,
            Option<nro_core::CoreStats>,
            Option<[Option<Packet>; 2]>,
        )> = None;
        let outcome = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if entry.snapshot.stats.hp <= 0 {
                    return None;
                }
                let pid_obj = entry.snapshot.id;
                let (sel, now_ms) = (
                    special.map_or(entry.selected_skill, |a| i16::from(a.skill_id)),
                    unix_now_ms(),
                );
                let (pruned, monkey_expired) = prune_expired_skill_buffs_detail(pid_obj, now_ms);
                let monkey_ended =
                    monkey_expired || monkey_revert_pending().remove(&pid_obj).is_some();
                if pruned || monkey_ended {
                    if pruned {
                        super::recalculate_session_stats(
                            sid,
                            &mut entry.snapshot,
                            self.shared.config.parity_mode,
                        );
                    }
                    let loc = entry.snapshot.location;
                    let revert_pkt =
                        monkey_ended.then(|| self.revert_monkey_appearance_entry(&entry, loc));
                    if pruned
                        && let (Some(ek), Ok(tx)) = (
                            entry.entity_key,
                            self.shared
                                .world_router
                                .resolve_sender(loc.map_id, loc.zone_id),
                        )
                    {
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    pruned_sync = Some((loc, pruned.then_some(entry.snapshot.stats), revert_pkt));
                }
                let status_ok = match status {
                    0 => matches!(sel, 6 | 7 | 12 | 18 | 21..=23),
                    1..=3 => sel == 8,
                    4 | 5 => matches!(sel, 10 | 11 | 14),
                    6 => sel == 13,
                    7 => sel == 14,
                    8 => sel == 12,
                    9 => sel == 19,
                    10 => sel == 21,
                    20 => matches!(sel, 24..=26) && special.is_some(),
                    _ => false,
                };
                if !status_ok {
                    return None;
                }
                let sk_idx = entry
                    .snapshot
                    .skills
                    .iter()
                    .position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
                let st = self
                    .shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| i16::from(s.id) == sel)?;
                let pt = entry.snapshot.skills[sk_idx].point;
                let pt_u8 = pt.max(1) as u8;
                let lv = st.levels.iter().find(|l| l.point == pt)?;
                let skill_paint_id = lv.id.max(0);
                let skip_mp = matches!(status, 2 | 3 | 5);
                if !skip_mp {
                    let cd = i64::from(lv.cool_down.max(0)).saturating_sub(50);
                    if now_ms.saturating_sub(entry.snapshot.skills[sk_idx].last_time_use_ms) < cd {
                        return None;
                    }
                }
                let mana_cost = if skip_mp {
                    0
                } else {
                    match st.mana_use_type {
                        2 => {
                            if entry.snapshot.stats.mp <= 0 {
                                return Some(Err("Không đủ KI để sử dụng kỹ năng!"));
                            }
                            entry.snapshot.stats.mp
                        }
                        1 => {
                            (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0)))
                                / 100
                        }
                        _ => i64::from(lv.mana_use.max(0)),
                    }
                };
                if mana_cost > 0 && entry.snapshot.stats.mp < mana_cost {
                    return Some(Err("Không đủ KI để sử dụng kỹ năng!"));
                }
                let caster_flag = entry.c_flag;
                let mut tu_sat_aoe: Option<(EntityKey, u8, Vec<u8>, u64)> = None;
                let mut monkey_parts: Option<(i16, i16, i16)> = None;
                let mut ally_effect: Option<(i16, u8, i8, i64)> = None;
                match status {
                    1 => {
                        entry.snapshot.stats.mp =
                            entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                        entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                        ttnl_state().insert(pid_obj, (now_ms, 0));
                    }
                    2 => {
                        let remove_after = {
                            let mut st_entry = ttnl_state().get_mut(&pid_obj)?;
                            let (last_tick_ms, ticks_used) = *st_entry;
                            if ticks_used >= MAX_TTNL_TICKS
                                || now_ms.saturating_sub(last_tick_ms) < 950
                            {
                                return None;
                            }
                            let next_ticks = ticks_used.saturating_add(1);
                            *st_entry = (now_ms, next_ticks);
                            let pct = i64::from(utility_skill_params(pt_u8).1);
                            let hp_gain =
                                (entry.snapshot.stats.hp_max.saturating_mul(pct) / 100).max(1);
                            let mp_gain =
                                (entry.snapshot.stats.mp_max.saturating_mul(pct) / 100).max(1);
                            entry.snapshot.stats.hp = entry
                                .snapshot
                                .stats
                                .hp
                                .saturating_add(hp_gain)
                                .min(entry.snapshot.stats.hp_max);
                            entry.snapshot.stats.mp = entry
                                .snapshot
                                .stats
                                .mp
                                .saturating_add(mp_gain)
                                .min(entry.snapshot.stats.mp_max);
                            next_ticks >= MAX_TTNL_TICKS
                                || (entry.snapshot.stats.hp >= entry.snapshot.stats.hp_max
                                    && entry.snapshot.stats.mp >= entry.snapshot.stats.mp_max)
                        };
                        if remove_after {
                            ttnl_state().remove(&pid_obj);
                        }
                    }
                    3 => {
                        ttnl_state().remove(&pid_obj);
                    }
                    4 => {
                        if sel == 14 {
                            super::router_skill_effects::record_tu_sat_prep(pid_obj, now_ms);
                        }
                    }
                    _ => {
                        if status == 7
                            && sel == 14
                            && !super::router_skill_effects::take_valid_tu_sat_prep(pid_obj, now_ms)
                        {
                            return None;
                        }
                        entry.snapshot.stats.mp =
                            entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                        if status == 7 && sel == 14 {
                            tu_sat_aoe = self.prepare_tu_sat_aoe(&mut entry, pt_u8, skill_paint_id);
                        }
                        if status == 9 {
                            let dur_ms = i64::try_from(shield_duration_ms(pt_u8)).unwrap_or(30_000);
                            shield_until().insert(pid_obj, now_ms.saturating_add(dur_ms));
                        }
                        if status == 6 && sel == 13 {
                            let idx = usize::try_from(pt.clamp(1, 7) - 1).unwrap_or(0);
                            monkey_parts = Some((MONKEY_HEADS[idx], 193, 194));
                            monkey_revert_pending().remove(&pid_obj);
                            let already_monkey = monkey_until()
                                .get(&pid_obj)
                                .is_some_and(|v| v.value().1 > now_ms);
                            let dur_ms =
                                i64::try_from(monkey_duration_ms(pt_u8, false)).unwrap_or(60_000);
                            monkey_until().insert(pid_obj, (pt_u8, now_ms.saturating_add(dur_ms)));
                            super::recalculate_session_stats(
                                sid,
                                &mut entry.snapshot,
                                self.shared.config.parity_mode,
                            );
                            if !already_monkey {
                                entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                            }
                        }
                        if !skip_mp && sel == 7 {
                            let mut np = stats_to_npoint(entry.snapshot.stats);
                            apply_tri_thuong_target(&mut np, pt_u8);
                            entry.snapshot.stats.hp =
                                i64::from(np.hp).min(entry.snapshot.stats.hp_max);
                            entry.snapshot.stats.mp =
                                i64::from(np.mp).min(entry.snapshot.stats.mp_max);
                            ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                        }
                        if !skip_mp && sel == 21 {
                            let _ = apply_huyt_sao_to_snapshot(
                                sid,
                                &mut entry.snapshot,
                                pt_u8,
                                now_ms,
                                self.shared.config.parity_mode,
                            );
                            ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                        }
                        if !skip_mp {
                            entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                        }
                    }
                }
                let (pid, loc, stats) = (
                    pid_obj.0 as i32,
                    entry.snapshot.location,
                    entry.snapshot.stats,
                );
                if let (Some(ek), Ok(tx)) = (
                    entry.entity_key,
                    self.shared
                        .world_router
                        .resolve_sender(loc.map_id, loc.zone_id),
                ) {
                    if let Some(parts) = monkey_parts {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, parts));
                    }
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
                }
                let skill_pkt = build_skill_not_focus_pkt(status, pid, skill_paint_id, special);
                let monkey_pkt = monkey_parts
                    .map(|(mh, mb, ml)| s2c_packets::update_body(pid_obj, Some((mh, mb, ml, 1))));
                let cc_cast = (!skip_mp && status == 0 && matches!(sel, 6 | 18 | 22 | 23))
                    .then_some((pt_u8, caster_flag, now_ms));
                Some(Ok((
                    sel,
                    pid_obj,
                    loc,
                    stats,
                    skill_pkt,
                    monkey_pkt,
                    tu_sat_aoe,
                    ally_effect,
                    cc_cast,
                )))
            });
        match outcome {
            Some(Ok((
                sel,
                pid_obj,
                loc,
                stats,
                skill_pkt,
                monkey_pkt,
                tu_sat_aoe,
                ally_effect,
                cc_cast,
            ))) => {
                self.finish_skill_not_focus(
                    session,
                    sid,
                    status,
                    sel,
                    pid_obj,
                    loc,
                    stats,
                    skill_pkt,
                    monkey_pkt,
                    pruned_sync,
                    tu_sat_aoe,
                    ally_effect,
                    cc_cast,
                )
                .await?;
            }
            other => {
                if let Some((ploc, pstats, rpkts)) = pruned_sync {
                    if let Some(pkts) = rpkts {
                        for p in pkts.into_iter().flatten() {
                            self.broadcast_zone_or_self(ploc, sid, &p);
                        }
                    }
                    if let Some(st) = pstats {
                        session.send(s2c_packets::me_load_point(&st)).await?;
                    }
                }
                if let Some(Err(msg)) = other {
                    send_thong_bao(session, msg)?;
                }
            }
        }
        Ok(())
    }
}
