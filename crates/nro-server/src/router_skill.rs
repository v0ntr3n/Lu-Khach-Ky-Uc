//! Non-targeted skill handler (`cmd -45`, `SKILL_NOT_FOCUS`) matching `Controller.cs:2071-2350`.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{EntityKey, PlayerId};
use nro_player::{
    npoint::NPoint,
    skills::{
        EffectSkillState, apply_huyt_sao_target, apply_tri_thuong_target, monkey_duration_ms,
        shield_duration_ms, utility_skill_params,
    },
};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle, SkillSpecialArgs};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::{ServerSessionHandler, send_thong_bao};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

const MONKEY_HEADS: [i16; 7] = [192, 195, 196, 199, 197, 200, 198];
const MAX_TTNL_TICKS: u8 = 10;

static TTNL_STATE: OnceLock<DashMap<PlayerId, (i64, u8)>> = OnceLock::new();
fn ttnl_state() -> &'static DashMap<PlayerId, (i64, u8)> {
    TTNL_STATE.get_or_init(DashMap::new)
}

static SHIELD_UNTIL: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn shield_until() -> &'static DashMap<PlayerId, i64> {
    SHIELD_UNTIL.get_or_init(DashMap::new)
}

static MONKEY_UNTIL: OnceLock<DashMap<PlayerId, (u8, i64)>> = OnceLock::new();
fn monkey_until() -> &'static DashMap<PlayerId, (u8, i64)> {
    MONKEY_UNTIL.get_or_init(DashMap::new)
}

static HUYT_SAO_UNTIL: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn huyt_sao_until() -> &'static DashMap<PlayerId, i64> {
    HUYT_SAO_UNTIL.get_or_init(DashMap::new)
}

pub(super) fn is_shield_active(pid: PlayerId, now_ms: i64) -> bool {
    shield_until().get(&pid).is_some_and(|t| *t.value() > now_ms)
}

pub(super) fn clear_shield(pid: PlayerId) {
    shield_until().remove(&pid);
}

pub(super) fn clear_ttnl(pid: PlayerId) {
    ttnl_state().remove(&pid);
}

pub(super) fn clear_skill_state(pid: PlayerId) {
    ttnl_state().remove(&pid);
    shield_until().remove(&pid);
    monkey_until().remove(&pid);
    huyt_sao_until().remove(&pid);
}

fn build_skill_not_focus_pkt(
    status: i8, pid: i32, skill_paint_id: i16, special: Option<SkillSpecialArgs>,
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
            let dir = special.map_or(1_i8, |a| a.dir);
            w.write_i8(0);
            w.write_i8(dir);
            w.write_i16(1_500);
            w.write_i8(0);
            w.write_i8(0);
            w.write_i8(-1);
        }
        _ => {}
    }
    w.finish(cmd_message::SKILL_NOT_FOCUS)
}

fn stats_to_npoint(stats: nro_core::CoreStats) -> NPoint {
    let mut np = NPoint::new(stats);
    np.hp_max = i32::try_from(stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp_max = i32::try_from(stats.mp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.hp = i32::try_from(stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np.mp = i32::try_from(stats.mp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
    np
}

fn apply_huyt_sao_to_stats(
    pid: PlayerId, stats: &mut nro_core::CoreStats, gender: nro_core::Gender, pt: u8, now_ms: i64,
) {
    let mut np = stats_to_npoint(*stats);
    let mut eff = EffectSkillState::default();
    apply_huyt_sao_target(&mut np, &mut eff, gender, pt, now_ms.max(0).cast_unsigned());
    if eff.huyt_sao_pct > 0 {
        let already_active = huyt_sao_until().get(&pid).is_some_and(|t| *t.value() > now_ms);
        if !already_active {
            let max_i32 = i64::from(i32::MAX);
            stats.hp_max = stats
                .hp_max
                .saturating_add(stats.hp_max.saturating_mul(i64::from(eff.huyt_sao_pct)) / 100)
                .clamp(1, max_i32);
            stats.hp = i64::from(np.hp).min(stats.hp_max);
        }
        huyt_sao_until().insert(pid, now_ms.saturating_add(30_000));
    } else {
        stats.hp = i64::from(np.hp).clamp(0, stats.hp_max);
    }
}

impl ServerSessionHandler {
    pub(crate) fn clear_ttnl_for_player(pid: PlayerId) {
        clear_ttnl(pid);
    }

    pub(super) async fn handle_skill_not_focus(
        &self, session: &SessionHandle, sid: u64, status: i8, special: Option<SkillSpecialArgs>,
    ) -> Result<(), ProtocolError> {
        let outcome = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.stats.hp <= 0 {
                return None;
            }
            let (sel, now_ms) = (special.map_or(entry.selected_skill, |a| i16::from(a.skill_id)), unix_now_ms());
            let status_ok = match status {
                1 | 2 => sel == 8,
                6 => sel == 13,
                7 => sel == 14,
                9 => sel == 19,
                _ => true,
            };
            if !status_ok {
                return None;
            }
            let sk_idx = entry.snapshot.skills.iter().position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
            let st = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == sel)?;
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
                    1 => (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0))) / 100,
                    _ => i64::from(lv.mana_use.max(0)),
                }
            };
            if mana_cost > 0 && entry.snapshot.stats.mp < mana_cost {
                return Some(Err("Không đủ KI để sử dụng kỹ năng!"));
            }
            let pid_obj = entry.snapshot.id;
            let caster_flag = entry.c_flag;
            let mut tu_sat_aoe: Option<(EntityKey, u8, Vec<u8>, u64)> = None;
            let mut monkey_parts: Option<(i16, i16, i16)> = None;
            let mut ally_effect: Option<(i16, u8, i8, i64)> = None;
            match status {
                1 => {
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                    entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                    ttnl_state().insert(pid_obj, (now_ms, 0));
                }
                2 => {
                    let remove_after = {
                        let mut st_entry = ttnl_state().get_mut(&pid_obj)?;
                        let (last_tick_ms, ticks_used) = *st_entry;
                        if ticks_used >= MAX_TTNL_TICKS || now_ms.saturating_sub(last_tick_ms) < 950 {
                            return None;
                        }
                        let next_ticks = ticks_used.saturating_add(1);
                        *st_entry = (now_ms, next_ticks);
                        let pct = i64::from(utility_skill_params(pt_u8).1);
                        let hp_gain = (entry.snapshot.stats.hp_max.saturating_mul(pct) / 100).max(1);
                        let mp_gain = (entry.snapshot.stats.mp_max.saturating_mul(pct) / 100).max(1);
                        entry.snapshot.stats.hp = entry.snapshot.stats.hp.saturating_add(hp_gain).min(entry.snapshot.stats.hp_max);
                        entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_add(mp_gain).min(entry.snapshot.stats.mp_max);
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
                4 => {}
                _ => {
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                    if status == 7 && sel == 14 {
                        let caster_hp = entry.snapshot.stats.hp.max(1);
                        let loc = entry.snapshot.location;
                        let range_px = i64::from(utility_skill_params(pt_u8).3);
                        let mob_ids: Vec<u8> = self.shared.templates.maps.iter()
                            .find(|m| m.id == loc.map_id)
                            .map_or_default(|m| {
                                m.mobs.iter().enumerate().take(127).filter_map(|(idx, &(_, _, _, mx, my))| {
                                    let dx = i64::from(mx) - i64::from(loc.x);
                                    let dy = i64::from(my) - i64::from(loc.y);
                                    (dx * dx + dy * dy <= range_px * range_px).then_some(idx as u8)
                                }).collect()
                            });
                        if let Some(ek) = entry.entity_key && !mob_ids.is_empty() {
                            let paint_u8 = u8::try_from(skill_paint_id.clamp(0, 255)).unwrap_or(0);
                            tu_sat_aoe = Some((ek, paint_u8, mob_ids, u64::try_from(caster_hp).unwrap_or(1)));
                        }
                        entry.snapshot.stats.hp = 0;
                    }
                    if status == 9 {
                        let dur_ms = i64::try_from(shield_duration_ms(pt_u8)).unwrap_or(30_000);
                        shield_until().insert(pid_obj, now_ms.saturating_add(dur_ms));
                    }
                    if status == 6 && sel == 13 {
                        let idx = usize::try_from(pt.clamp(1, 7) - 1).unwrap_or(0);
                        monkey_parts = Some((MONKEY_HEADS[idx], 193, 194));
                        let already_monkey = monkey_until().get(&pid_obj).is_some_and(|v| v.value().1 > now_ms);
                        if !already_monkey {
                            let lvl = i64::from(pt.clamp(1, 7));
                            let hp_pct = (lvl + 3) * 10;
                            let dmg_pct = lvl + 3;
                            let max_i32 = i64::from(i32::MAX);
                            entry.snapshot.stats.hp_max = entry.snapshot.stats.hp_max
                                .saturating_add(entry.snapshot.stats.hp_max.saturating_mul(hp_pct) / 100)
                                .clamp(1, max_i32);
                            entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                            entry.snapshot.stats.dame = entry.snapshot.stats.dame
                                .saturating_add(entry.snapshot.stats.dame.saturating_mul(dmg_pct) / 100)
                                .clamp(0, max_i32);
                            entry.snapshot.stats.crit = 110;
                        }
                        let dur_ms = i64::try_from(monkey_duration_ms(pt_u8, false)).unwrap_or(60_000);
                        monkey_until().insert(pid_obj, (pt_u8, now_ms.saturating_add(dur_ms)));
                    }
                    if !skip_mp && sel == 7 {
                        let mut np = stats_to_npoint(entry.snapshot.stats);
                        apply_tri_thuong_target(&mut np, pt_u8);
                        entry.snapshot.stats.hp = i64::from(np.hp).min(entry.snapshot.stats.hp_max);
                        entry.snapshot.stats.mp = i64::from(np.mp).min(entry.snapshot.stats.mp_max);
                        ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                    }
                    if !skip_mp && sel == 21 {
                        let gender = entry.snapshot.gender;
                        apply_huyt_sao_to_stats(pid_obj, &mut entry.snapshot.stats, gender, pt_u8, now_ms);
                        ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                    }
                    if !skip_mp {
                        entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                    }
                }
            }
            let (pid, loc, stats) = (pid_obj.0 as i32, entry.snapshot.location, entry.snapshot.stats);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                if let Some(parts) = monkey_parts {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, parts));
                }
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
            }
            let skill_pkt = build_skill_not_focus_pkt(status, pid, skill_paint_id, special);
            let die_pkts = (status == 7 && sel == 14)
                .then(|| s2c_packets::char_die_packets(pid_obj, 0, loc, stats.power));
            let monkey_pkt = monkey_parts.map(|(mh, mb, ml)| s2c_packets::update_body(pid_obj, Some((mh, mb, ml, 1))));
            Some(Ok((sel, pid_obj, loc, stats, skill_pkt, die_pkts, monkey_pkt, tu_sat_aoe, ally_effect)))
        });
        match outcome {
            Some(Err(msg)) => send_thong_bao(session, msg)?,
            Some(Ok((sel, pid_obj, loc, stats, skill_pkt, die_pkts, monkey_pkt, tu_sat_aoe, ally_effect))) => {
                self.broadcast_zone_or_self(loc, sid, &skill_pkt);
                if let Some(mpkt) = monkey_pkt {
                    self.broadcast_zone_or_self(loc, sid, &mpkt);
                }
                if let Some((ek, paint_u8, mob_ids, dmg)) = tu_sat_aoe
                    && let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)
                {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    let cmd = ZoneCommand::AttackMobs {
                        attacker: ek,
                        skill_paint_id: paint_u8,
                        mob_ids,
                        damage: dmg,
                        is_crit: false,
                        reply: reply_tx,
                    };
                    if zone_tx.send(cmd).await.is_ok() && let Ok((tnsm, killed_mobs)) = reply_rx.await {
                        if tnsm > 0 {
                            let now_ms = unix_now_ms();
                            let wire_exp = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                                let total_tnsm = tnsm.saturating_mul(u64::try_from(entry.snapshot.charms.tnsm_multiplier(now_ms).max(1)).unwrap_or(1));
                                let gain = i64::try_from(total_tnsm).unwrap_or(i64::MAX);
                                entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(gain);
                                entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(gain);
                                let _ = zone_tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                                i32::try_from(total_tnsm.min(u64::try_from(i32::MAX).unwrap_or(u64::MAX))).unwrap_or(i32::MAX)
                            });
                            if let Some(exp) = wire_exp {
                                self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                                session.send(s2c_packets::player_up_exp(2, exp)).await?;
                                self.advance_task_on_power(session, sid).await?;
                            }
                        }
                        for mob_tid in killed_mobs {
                            self.advance_task_on_mob_kill(session, sid, mob_tid).await?;
                        }
                    }
                }
                if let Some((eff_sel, pt_u8, caster_flag, now_ms)) = ally_effect {
                    let ally_sids: Vec<u64> = self.shared.clients.authenticated.iter()
                        .filter_map(|e| {
                            let a_sid = *e.key();
                            let aloc = e.snapshot.location;
                            let same_flag = (caster_flag == 0 && e.c_flag == 0)
                                || (caster_flag > 0 && caster_flag == e.c_flag && caster_flag != 8);
                            (a_sid != sid && aloc.map_id == loc.map_id && aloc.zone_id == loc.zone_id && same_flag)
                                .then_some(a_sid)
                        })
                        .collect();
                    for a_sid in ally_sids {
                        let upd = self.shared.clients.authenticated.get_mut(&a_sid).map(|mut ally| {
                            if eff_sel == 7 {
                                let mut np = stats_to_npoint(ally.snapshot.stats);
                                apply_tri_thuong_target(&mut np, pt_u8);
                                ally.snapshot.stats.hp = i64::from(np.hp).min(ally.snapshot.stats.hp_max);
                                ally.snapshot.stats.mp = i64::from(np.mp).min(ally.snapshot.stats.mp_max);
                            } else if eff_sel == 21 && ally.snapshot.stats.hp > 0 {
                                let (apid, agender) = (ally.snapshot.id, ally.snapshot.gender);
                                apply_huyt_sao_to_stats(apid, &mut ally.snapshot.stats, agender, pt_u8, now_ms);
                            }
                            if let (Some(aek), Ok(tx)) = (ally.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, ally.snapshot.stats));
                            }
                            (ally.handle.clone(), ally.snapshot.stats)
                        });
                        if let Some((ah, astats)) = upd {
                            let _ = ah.try_send(s2c_packets::me_load_point(&astats));
                            let _ = ah.try_send(s2c_packets::me_load_hp(astats.hp));
                            if eff_sel == 7 {
                                let _ = ah.try_send(s2c_packets::me_load_mp(astats.mp));
                            }
                        }
                    }
                }
                session.send(s2c_packets::me_load_point(&stats)).await?;
                if matches!(status, 2 | 6 | 7) || matches!(sel, 7 | 21) {
                    session.send(s2c_packets::me_load_hp(stats.hp)).await?;
                    session.send(s2c_packets::me_load_mp(stats.mp)).await?;
                }
                if let Some((me_die, peer_die)) = die_pkts {
                    self.handle_player_death_cleanup(pid_obj, loc, sid);
                    session.send(me_die).await?;
                    self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
                }
            }
            None => {}
        }
        Ok(())
    }
}
