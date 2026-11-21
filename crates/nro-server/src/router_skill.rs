//! Non-targeted skill handler (`cmd -45`, `SKILL_NOT_FOCUS`) matching `Controller.cs:2071-2350`.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{EntityKey, Location, PlayerId};
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

use super::{ServerSessionHandler, effective_appearance, send_thong_bao};
use crate::runtime::{ActivePlayerEntry, LeaderboardSlot, unix_now_ms};

const MONKEY_HEADS: [i16; 7] = [192, 195, 196, 199, 197, 200, 198];
const MAX_TTNL_TICKS: u8 = 10;

static TTNL_STATE: OnceLock<DashMap<PlayerId, (i64, u8)>> = OnceLock::new();
fn ttnl_state() -> &'static DashMap<PlayerId, (i64, u8)> { TTNL_STATE.get_or_init(DashMap::new) }
static SHIELD_UNTIL: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn shield_until() -> &'static DashMap<PlayerId, i64> { SHIELD_UNTIL.get_or_init(DashMap::new) }
static MONKEY_UNTIL: OnceLock<DashMap<PlayerId, (u8, i64)>> = OnceLock::new();
fn monkey_until() -> &'static DashMap<PlayerId, (u8, i64)> { MONKEY_UNTIL.get_or_init(DashMap::new) }
static MONKEY_REVERT_PENDING: OnceLock<DashMap<PlayerId, ()>> = OnceLock::new();
fn monkey_revert_pending() -> &'static DashMap<PlayerId, ()> { MONKEY_REVERT_PENDING.get_or_init(DashMap::new) }
static HUYT_SAO_UNTIL: OnceLock<DashMap<PlayerId, (i32, i64)>> = OnceLock::new();
fn huyt_sao_until() -> &'static DashMap<PlayerId, (i32, i64)> { HUYT_SAO_UNTIL.get_or_init(DashMap::new) }

pub(super) fn is_shield_active(pid: PlayerId, now_ms: i64) -> bool {
    shield_until().get(&pid).is_some_and(|t| *t.value() > now_ms)
}
pub(super) fn is_monkey_active(pid: PlayerId, now_ms: i64) -> bool {
    monkey_until().get(&pid).is_some_and(|v| v.value().1 > now_ms)
}

pub(super) fn clear_shield(pid: PlayerId) { shield_until().remove(&pid); }
pub(super) fn clear_ttnl(pid: PlayerId) { ttnl_state().remove(&pid); }
pub(super) fn take_monkey_revert_pending(pid: PlayerId) -> bool { monkey_revert_pending().remove(&pid).is_some() }

pub(super) fn clear_skill_state(pid: PlayerId) -> bool {
    ttnl_state().remove(&pid);
    shield_until().remove(&pid);
    let had_monkey = monkey_until().remove(&pid).is_some();
    if had_monkey { monkey_revert_pending().insert(pid, ()); }
    let had_huyt_sao = huyt_sao_until().remove(&pid).is_some();
    had_monkey || had_huyt_sao
}

fn prune_expired_skill_buffs_detail(pid: PlayerId, now_ms: i64) -> (bool, bool) {
    let monkey_expired = monkey_until().get(&pid).is_some_and(|v| v.value().1 <= now_ms);
    if monkey_expired { monkey_until().remove(&pid); }
    let huyt_expired = huyt_sao_until().get(&pid).is_some_and(|v| v.value().1 <= now_ms);
    if huyt_expired { huyt_sao_until().remove(&pid); }
    (monkey_expired || huyt_expired, monkey_expired)
}

pub(super) fn prune_expired_skill_buffs(pid: PlayerId, now_ms: i64) -> bool {
    let (any, monkey_expired) = prune_expired_skill_buffs_detail(pid, now_ms);
    if monkey_expired { monkey_revert_pending().insert(pid, ()); }
    any
}

pub(super) fn apply_skill_buffs_before_clamp(pid: PlayerId, stats: &mut nro_core::CoreStats) {
    let (now_ms, max_i32) = (unix_now_ms(), i64::from(i32::MAX));
    if let Some((pt, until)) = monkey_until().get(&pid).map(|v| *v.value()) {
        if until > now_ms {
            let lvl = i64::from(pt.clamp(1, 7));
            stats.hp_max = stats.hp_max.saturating_add(stats.hp_max.saturating_mul((lvl + 3) * 10) / 100).clamp(1, max_i32);
            stats.dame = stats.dame.saturating_add(stats.dame.saturating_mul(lvl + 3) / 100).clamp(0, max_i32);
            stats.crit = 110;
        } else {
            monkey_until().remove(&pid);
            monkey_revert_pending().insert(pid, ());
        }
    }
    if let Some((pct, until)) = huyt_sao_until().get(&pid).map(|v| *v.value()) {
        if until > now_ms && pct > 0 {
            stats.hp_max = stats.hp_max.saturating_add(stats.hp_max.saturating_mul(i64::from(pct)) / 100).clamp(1, max_i32);
        } else { huyt_sao_until().remove(&pid); }
    }
}

fn build_skill_not_focus_pkt(status: i8, pid: i32, skill_paint_id: i16, special: Option<SkillSpecialArgs>) -> Packet {
    let mut w = PacketWriter::with_capacity(20);
    let wire_status = match status {
        2 => 1,
        9 | 10 => 6,
        20 if special.is_some() => 20,
        0 | 1 | 3 | 4 | 5 | 6 | 7 | 8 => status,
        _ => 0,
    };
    w.write_i8(wire_status); w.write_i32(pid); w.write_i16(skill_paint_id);
    match wire_status {
        0 => { w.write_i8(0); w.write_i8(0); }
        4 | 7 => w.write_i16(3_000),
        20 => {
            w.write_i8(0); w.write_i8(special.map_or(1_i8, |a| a.dir)); w.write_i16(1_500);
            w.write_i8(0); w.write_i8(0); w.write_i8(-1);
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

fn apply_huyt_sao_to_snapshot(
    sid: u64, snap: &mut nro_db::player_repo::PlayerSnapshot, pt: u8, now_ms: i64, parity: nro_core::ParityMode,
) -> bool {
    let (pid, gender) = (snap.id, snap.gender);
    let already_active = huyt_sao_until().get(&pid).is_some_and(|t| t.value().1 > now_ms);
    let (pruned, monkey_expired) = prune_expired_skill_buffs_detail(pid, now_ms);
    let monkey_ended = monkey_expired || monkey_revert_pending().remove(&pid).is_some();
    if pruned { super::recalculate_session_stats(sid, snap, parity); }
    let mut np = stats_to_npoint(snap.stats);
    let mut eff = EffectSkillState::default();
    apply_huyt_sao_target(&mut np, &mut eff, gender, pt, now_ms.max(0).cast_unsigned());
    if eff.huyt_sao_pct > 0 {
        huyt_sao_until().insert(pid, (eff.huyt_sao_pct, now_ms.saturating_add(30_000)));
        super::recalculate_session_stats(sid, snap, parity);
        if !already_active { snap.stats.hp = i64::from(np.hp).min(snap.stats.hp_max); }
    } else {
        snap.stats.hp = i64::from(np.hp).clamp(0, snap.stats.hp_max);
    }
    monkey_ended
}

impl ServerSessionHandler {
    pub(crate) fn clear_ttnl_for_player(pid: PlayerId) { clear_ttnl(pid); }

    pub(super) fn revert_monkey_appearance_entry(&self, entry: &ActivePlayerEntry, loc: Location) -> Packet {
        let (eh, eb, el, _, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
        if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
            let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
        }
        s2c_packets::update_body(entry.snapshot.id, is_disg.then_some((eh, eb, el, 0)))
    }

    pub(super) async fn handle_skill_not_focus(
        &self, session: &SessionHandle, sid: u64, status: i8, special: Option<SkillSpecialArgs>,
    ) -> Result<(), ProtocolError> {
        let mut pruned_sync: Option<(Location, Option<nro_core::CoreStats>, Option<Packet>)> = None;
        let outcome = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.stats.hp <= 0 { return None; }
            let pid_obj = entry.snapshot.id;
            let (sel, now_ms) = (special.map_or(entry.selected_skill, |a| i16::from(a.skill_id)), unix_now_ms());
            let (pruned, monkey_expired) = prune_expired_skill_buffs_detail(pid_obj, now_ms);
            let monkey_ended = monkey_expired || monkey_revert_pending().remove(&pid_obj).is_some();
            if pruned || monkey_ended {
                if pruned { super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode); }
                let loc = entry.snapshot.location;
                let revert_pkt = monkey_ended.then(|| self.revert_monkey_appearance_entry(&entry, loc));
                if pruned && let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                pruned_sync = Some((loc, pruned.then_some(entry.snapshot.stats), revert_pkt));
            }
            let status_ok = match status {
                0 => matches!(sel, 6 | 7 | 12 | 18 | 21..=23), 1..=3 => sel == 8, 4 | 5 => matches!(sel, 10 | 11 | 14),
                6 => sel == 13, 7 => sel == 14, 8 => sel == 12, 9 => sel == 19, 10 => sel == 21,
                20 => matches!(sel, 24..=26) && special.is_some(), _ => false,
            };
            if !status_ok { return None; }
            let sk_idx = entry.snapshot.skills.iter().position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
            let st = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == sel)?;
            let pt = entry.snapshot.skills[sk_idx].point;
            let pt_u8 = pt.max(1) as u8;
            let lv = st.levels.iter().find(|l| l.point == pt)?;
            let skill_paint_id = lv.id.max(0);
            let skip_mp = matches!(status, 2 | 3 | 5);
            if !skip_mp {
                let cd = i64::from(lv.cool_down.max(0)).saturating_sub(50);
                if now_ms.saturating_sub(entry.snapshot.skills[sk_idx].last_time_use_ms) < cd { return None; }
            }
            let mana_cost = if skip_mp {
                0
            } else {
                match st.mana_use_type {
                    2 => {
                        if entry.snapshot.stats.mp <= 0 { return Some(Err("Không đủ KI để sử dụng kỹ năng!")); }
                        entry.snapshot.stats.mp
                    }
                    1 => (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0))) / 100,
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
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                    entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                    ttnl_state().insert(pid_obj, (now_ms, 0));
                }
                2 => {
                    let remove_after = {
                        let mut st_entry = ttnl_state().get_mut(&pid_obj)?;
                        let (last_tick_ms, ticks_used) = *st_entry;
                        if ticks_used >= MAX_TTNL_TICKS || now_ms.saturating_sub(last_tick_ms) < 950 { return None; }
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
                    if remove_after { ttnl_state().remove(&pid_obj); }
                }
                3 => { ttnl_state().remove(&pid_obj); }
                4 => { if sel == 14 { super::router_skill_effects::record_tu_sat_prep(pid_obj, now_ms); } }
                _ => {
                    if status == 7 && sel == 14 && !super::router_skill_effects::take_valid_tu_sat_prep(pid_obj, now_ms) { return None; }
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
                        monkey_revert_pending().remove(&pid_obj);
                        let already_monkey = monkey_until().get(&pid_obj).is_some_and(|v| v.value().1 > now_ms);
                        let dur_ms = i64::try_from(monkey_duration_ms(pt_u8, false)).unwrap_or(60_000);
                        monkey_until().insert(pid_obj, (pt_u8, now_ms.saturating_add(dur_ms)));
                        super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                        if !already_monkey { entry.snapshot.stats.hp = entry.snapshot.stats.hp_max; }
                    }
                    if !skip_mp && sel == 7 {
                        let mut np = stats_to_npoint(entry.snapshot.stats);
                        apply_tri_thuong_target(&mut np, pt_u8);
                        entry.snapshot.stats.hp = i64::from(np.hp).min(entry.snapshot.stats.hp_max);
                        entry.snapshot.stats.mp = i64::from(np.mp).min(entry.snapshot.stats.mp_max);
                        ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                    }
                    if !skip_mp && sel == 21 {
                        let _ = apply_huyt_sao_to_snapshot(sid, &mut entry.snapshot, pt_u8, now_ms, self.shared.config.parity_mode);
                        ally_effect = Some((sel, pt_u8, caster_flag, now_ms));
                    }
                    if !skip_mp { entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms; }
                }
            }
            let (pid, loc, stats) = (pid_obj.0 as i32, entry.snapshot.location, entry.snapshot.stats);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                if let Some(parts) = monkey_parts { let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, parts)); }
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
            }
            let skill_pkt = build_skill_not_focus_pkt(status, pid, skill_paint_id, special);
            let monkey_pkt = monkey_parts.map(|(mh, mb, ml)| s2c_packets::update_body(pid_obj, Some((mh, mb, ml, 1))));
            let cc_cast = (!skip_mp && status == 0 && matches!(sel, 6 | 18 | 22 | 23)).then_some((pt_u8, caster_flag, now_ms));
            Some(Ok((sel, pid_obj, loc, stats, skill_pkt, monkey_pkt, tu_sat_aoe, ally_effect, cc_cast)))
        });
        match outcome {
            Some(Ok((sel, pid_obj, loc, stats, skill_pkt, monkey_pkt, tu_sat_aoe, ally_effect, cc_cast))) => {
                let tu_sat_died = status == 7 && sel == 14;
                if tu_sat_died { self.handle_player_death_cleanup(pid_obj, loc, sid); }
                if let Some((pt_u8, c_flag, now_ms)) = cc_cast {
                    self.apply_cast_cc_targets(sid, pid_obj, c_flag, loc, sel, pt_u8, now_ms);
                }
                self.broadcast_zone_or_self(loc, sid, &skill_pkt);
                if let Some(mpkt) = monkey_pkt {
                    self.broadcast_zone_or_self(loc, sid, &mpkt);
                } else if let Some((ploc, _, Some(rpkt))) = pruned_sync {
                    self.broadcast_zone_or_self(ploc, sid, &rpkt);
                }
                if let Some((ek, paint_u8, mob_ids, dmg)) = tu_sat_aoe
                    && let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)
                {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    let cmd = ZoneCommand::AttackMobs {
                        attacker: ek, skill_paint_id: paint_u8, mob_ids, damage: dmg, is_crit: false, reply: reply_tx,
                    };
                    if zone_tx.send(cmd).await.is_ok() && let Ok((tnsm, killed_mobs)) = reply_rx.await {
                        if tnsm > 0 {
                            let now_ms = unix_now_ms();
                            let wire_exp = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                                let total_tnsm = self.compute_player_tnsm(&entry, tnsm, now_ms);
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
                        for mob_tid in killed_mobs { self.advance_task_on_mob_kill(session, sid, mob_tid).await?; }
                    }
                }
                if let Some((eff_sel, pt_u8, caster_flag, now_ms)) = ally_effect && !super::router_combat::is_offline_map(loc.map_id) {
                    let ally_range = super::router_combat_calc::skill_max_range_px(nro_player::skills::SkillKind::from_repr(eff_sel as u8), pt_u8);
                    let ally_sids: Vec<u64> = self.shared.clients.authenticated.iter()
                        .filter_map(|e| {
                            let (a_sid, aloc) = (*e.key(), e.snapshot.location);
                            let same_flag = (caster_flag == 0 && e.c_flag == 0)
                                || (caster_flag > 0 && caster_flag == e.c_flag && caster_flag != 8);
                            (a_sid != sid && aloc.map_id == loc.map_id && aloc.zone_id == loc.zone_id && same_flag && super::router_combat_calc::in_range_px(loc, aloc, ally_range)).then_some(a_sid)
                        })
                        .collect();
                    for a_sid in ally_sids {
                        let upd = self.shared.clients.authenticated.get_mut(&a_sid).and_then(|mut ally| {
                            let aloc = ally.snapshot.location;
                            if aloc.map_id != loc.map_id || aloc.zone_id != loc.zone_id || !super::router_combat_calc::in_range_px(loc, aloc, ally_range) { return None; }
                            let (mut was_dead, mut ally_monkey_revert) = (false, None);
                            if eff_sel == 7 {
                                was_dead = ally.snapshot.stats.hp <= 0;
                                let mut np = stats_to_npoint(ally.snapshot.stats);
                                apply_tri_thuong_target(&mut np, pt_u8);
                                ally.snapshot.stats.hp = i64::from(np.hp).min(ally.snapshot.stats.hp_max);
                                ally.snapshot.stats.mp = i64::from(np.mp).min(ally.snapshot.stats.mp_max);
                            } else if eff_sel == 21 && ally.snapshot.stats.hp > 0
                                && apply_huyt_sao_to_snapshot(a_sid, &mut ally.snapshot, pt_u8, now_ms, self.shared.config.parity_mode)
                            {
                                ally_monkey_revert = Some(self.revert_monkey_appearance_entry(&ally, loc));
                            }
                            if let (Some(aek), Ok(tx)) = (ally.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, ally.snapshot.stats));
                            }
                            Some((ally.handle.clone(), ally.snapshot.id, ally.snapshot.location, ally.snapshot.stats, was_dead, ally_monkey_revert))
                        });
                        if let Some((ah, apid, aloc, astats, was_dead, ally_monkey_revert)) = upd {
                            if let Some(rpkt) = ally_monkey_revert { self.broadcast_zone_or_self(aloc, a_sid, &rpkt); }
                            if was_dead {
                                let (me_live_pkt, peer_live_pkt) = s2c_packets::char_revive_packets(apid, astats.hp, astats.hp_max, aloc);
                                let _ = ah.try_send(me_live_pkt);
                                self.broadcast_to_zone_peers(aloc, Some(a_sid), &peer_live_pkt);
                            }
                            let _ = ah.try_send(s2c_packets::me_load_point(&astats));
                            let _ = ah.try_send(s2c_packets::me_load_hp(astats.hp));
                            if eff_sel == 7 { let _ = ah.try_send(s2c_packets::me_load_mp(astats.mp)); }
                        }
                    }
                }
                let stats = self.shared.clients.authenticated.get(&sid).map_or(stats, |e| e.snapshot.stats);
                session.send(s2c_packets::me_load_point(&stats)).await?;
                if matches!(status, 2 | 6 | 7) || matches!(sel, 7 | 21) {
                    session.send(s2c_packets::me_load_hp(stats.hp)).await?;
                    session.send(s2c_packets::me_load_mp(stats.mp)).await?;
                }
                if tu_sat_died {
                    let (me_die, peer_die) = s2c_packets::char_die_packets(pid_obj, 0, loc, stats.power);
                    session.send(me_die).await?;
                    self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
                }
            }
            other => {
                if let Some((ploc, pstats, rpkt)) = pruned_sync {
                    if let Some(pkt) = rpkt { self.broadcast_zone_or_self(ploc, sid, &pkt); }
                    if let Some(st) = pstats { session.send(s2c_packets::me_load_point(&st)).await?; }
                }
                if let Some(Err(msg)) = other { send_thong_bao(session, msg)?; }
            }
        }
        Ok(())
    }
}
