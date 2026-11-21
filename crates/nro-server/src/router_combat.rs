//! Core combat, attack resolution, death cleanup, item pickup, teleportation, and zone broadcasts.

use std::sync::Arc;

use nro_boss::{EngineEffect, PlayerView};
use nro_core::{EntityKey, ItemInstance, Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_economy::{InventoryError, ItemCatalog};
use nro_player::{combat::TnsmContext, npoint::NPoint, skills::{CcFlags, SkillKind, cc_durations_ms}};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::router_combat_calc::{
    apply_incoming_hit_to_player, apply_lifesteal, attacker_pen_and_lifesteal, compute_attack_damage,
    in_range_px, next_combat_rng, skill_max_range_px,
};
use super::router_combat_pvp::{lock_pvp, pending_pvp};
use super::{DbItemCatalog, ServerSessionHandler, effective_appearance, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot};
use crate::runtime::{LeaderboardSlot, unix_now_ms};
use crate::state::ActivePlayerEntry;

#[inline]
pub(super) const fn is_offline_map(map_id: MapId) -> bool { matches!(map_id.0, 21..=23 | 39..=41 | 45..=50) }

fn dedup_take<T: Copy + PartialEq>(items: impl IntoIterator<Item = T>, limit: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(limit.min(16));
    for item in items { if !out.contains(&item) { out.push(item); if out.len() >= limit { break; } } }
    out
}

struct CastPrep {
    skill_kind: Option<SkillKind>, skill_point: u8, skill_paint_id: u8, mob_dmg: u64, pvp_dmg: u64,
    is_crit: bool, new_mp: i64, now_ms: i64, max_fight: usize, max_range_px: i64,
    attacker_pen: (i32, i32, i32), attacker_drains: (i32, i32, i32),
}

impl ServerSessionHandler {
    #[inline]
    pub(crate) const fn is_offline_map_id(map_id: MapId) -> bool { is_offline_map(map_id) }

    pub(crate) fn active_entry_to_player_view(entry: &ActivePlayerEntry) -> PlayerView {
        let s = &entry.snapshot;
        PlayerView {
            id: s.id, clan_id: (s.clan_id.0 >= 0).then_some(s.clan_id), gender: s.gender, location: s.location,
            stats: s.stats, power: s.stats.power, flag: u8::try_from(entry.c_flag.max(0)).unwrap_or(0),
            gold: u64::try_from(s.wallet.gold.max(0)).unwrap_or(0), has_pet: s.pet.is_some(), carrying_namek_ball: false,
            active_task: (u16::try_from(s.main_task.task_id.max(0)).unwrap_or(0), u8::try_from(s.main_task.sub_index.max(0)).unwrap_or(0)),
            whis_top_level: u32::try_from(s.thachdauwhis.max(0)).unwrap_or(0), last_chat: None,
        }
    }

    pub(super) fn compute_player_tnsm(&self, entry: &ActivePlayerEntry, base_tnsm: u64, now_ms: i64) -> u64 {
        let mut np = NPoint::new(entry.snapshot.stats);
        for opt in entry.snapshot.items_body.iter().flatten().flat_map(|it| it.options.iter()) {
            if matches!(opt.id, 88 | 101 | 156) { np.mods.tl_tnsm.push(opt.param); }
        }
        let (charms, intr) = (&entry.snapshot.charms, &entry.snapshot.intrinsic);
        let ctx = TnsmContext {
            charms_tri_tue: (charms.td_tri_tue_ms > now_ms, charms.td_tri_tue3_ms > now_ms, charms.td_tri_tue4_ms > now_ms),
            vip_active: (entry.snapshot.vip_state.1 > now_ms, entry.snapshot.vip_state.2),
            bonus_flags: (false, false, false, false),
            intrinsic: (u8::try_from(intr.intrinsic_id.max(0)).unwrap_or(0), i32::from(intr.param1)),
            pet_master_bonus: None,
            world_bonus: (matches!(entry.snapshot.location.map_id.0, 135..=138), u8::try_from(entry.c_flag.max(0)).unwrap_or(0), self.shared.config.rates.exp_multiplier),
            parity: self.shared.config.parity_mode,
        };
        np.cal_suc_manh_tiem_nang(base_tnsm, &ctx)
    }

    fn prepare_attack_cast(&self, entry: &mut ActivePlayerEntry, sid: u64, revert_pkt: &mut Option<(Location, Packet)>) -> Option<CastPrep> {
        let (sel, now_ms, pid, loc) = (entry.selected_skill, unix_now_ms(), entry.snapshot.id, entry.snapshot.location);
        super::router_skill::clear_ttnl(pid);
        let pruned = super::router_economy::prune_expired_session_buffs(sid, now_ms)
            | super::router_skill::prune_expired_skill_buffs(pid, now_ms);
        if pruned {
            super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats)); }
            let _ = entry.handle.try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
        }
        if super::router_skill::take_monkey_revert_pending(pid) { *revert_pkt = Some((loc, self.revert_monkey_appearance_entry(entry, loc))); }
        if !matches!(sel, 0..=5 | 9..=11 | 17 | 20 | 24..=26) { return None; }
        let sk_idx = entry.snapshot.skills.iter().position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
        let st = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == sel)?;
        let pt = entry.snapshot.skills[sk_idx].point;
        let lv = st.levels.iter().find(|l| l.point == pt)?;
        if now_ms.saturating_sub(entry.snapshot.skills[sk_idx].last_time_use_ms) < i64::from(lv.cool_down.max(0)).saturating_sub(50) { return None; }
        let hp_cost = if sel == 9 { (entry.snapshot.stats.hp_max.max(0) / 10).max(1) } else { 0 };
        if hp_cost > 0 && entry.snapshot.stats.hp <= hp_cost { return None; }
        let mana_cost = match st.mana_use_type {
            2 => { if entry.snapshot.stats.mp <= 0 { return None; } entry.snapshot.stats.mp }
            1 => (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0))) / 100,
            _ => i64::from(lv.mana_use.max(0)),
        };
        if entry.snapshot.stats.mp < mana_cost { return None; }
        if hp_cost > 0 { entry.snapshot.stats.hp = entry.snapshot.stats.hp.saturating_sub(hp_cost); let _ = entry.handle.try_send(s2c_packets::me_load_hp(entry.snapshot.stats.hp)); }
        let new_mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
        entry.snapshot.stats.mp = new_mp; entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
        let (skill_kind, skill_point) = (SkillKind::from_repr(sel as u8), u8::try_from(pt.clamp(1, 7)).unwrap_or(1));
        let skill_paint_id = u8::try_from(lv.id.clamp(0, 255)).unwrap_or(0);
        let (mob_dmg, pvp_dmg, is_crit) = compute_attack_damage(entry, sid, skill_kind.unwrap_or(SkillKind::Dragon), lv.damage.max(100), skill_point, now_ms, self.shared.config.parity_mode);
        let (attacker_pen, attacker_drains) = attacker_pen_and_lifesteal(&entry.snapshot.items_body);
        Some(CastPrep { skill_kind, skill_point, skill_paint_id, mob_dmg, pvp_dmg, is_crit, new_mp, now_ms, max_fight: usize::try_from(lv.max_fight.max(1)).unwrap_or(1), max_range_px: skill_max_range_px(skill_kind, skill_point), attacker_pen, attacker_drains })
    }

    fn filter_mobs_in_range(&self, loc: Location, raw_mobs: Vec<u8>, cast: &CastPrep) -> Vec<u8> {
        let Some(map_tpl) = self.shared.templates.maps.iter().find(|m| m.id == loc.map_id) else { return dedup_take(raw_mobs, cast.max_fight); };
        let in_range = raw_mobs.into_iter().filter(|&slot| {
            map_tpl.mobs.get(usize::from(slot)).is_some_and(|&(_, _, _, mx, my)| in_range_px(loc, Location { map_id: loc.map_id, zone_id: loc.zone_id, x: mx, y: my }, cast.max_range_px))
        });
        dedup_take(in_range, cast.max_fight)
    }

    async fn resolve_mob_hits(&self, session: &SessionHandle, sid: u64, ek: EntityKey, loc: Location, valid_mobs: Vec<u8>, cast: &CastPrep) -> Result<(), ProtocolError> {
        if valid_mobs.is_empty() { return Ok(()); }
        let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) else { return Ok(()); };
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = ZoneCommand::AttackMobs { attacker: ek, skill_paint_id: cast.skill_paint_id, mob_ids: valid_mobs, damage: cast.mob_dmg, is_crit: cast.is_crit, reply: reply_tx };
        if zone_tx.send(cmd).await.is_ok() && let Ok((tnsm, killed_mobs)) = reply_rx.await {
            if tnsm > 0 {
                let now_ms = unix_now_ms();
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let total_tnsm = self.compute_player_tnsm(&entry, tnsm, now_ms);
                    let gain = i64::try_from(total_tnsm).unwrap_or(i64::MAX);
                    entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(gain);
                    entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(gain);
                    let drained = apply_lifesteal(&mut entry.snapshot.stats, i64::try_from(cast.mob_dmg).unwrap_or(i64::MAX), true, cast.attacker_drains);
                    let _ = zone_tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    let exp = i32::try_from(total_tnsm.min(u64::try_from(i32::MAX).unwrap_or(u64::MAX))).unwrap_or(i32::MAX);
                    (exp, drained.then_some(entry.snapshot.stats))
                });
                if let Some((exp, drained_stats)) = res {
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    session.send(s2c_packets::player_up_exp(2, exp)).await?;
                    if let Some(st) = drained_stats { session.send(s2c_packets::me_load_point(&st)).await?; }
                    self.advance_task_on_power(session, sid).await?;
                }
            }
            for mob_tid in killed_mobs { self.advance_task_on_mob_kill(session, sid, mob_tid).await?; }
        }
        Ok(())
    }

    fn apply_attacker_lifesteal(&self, attacker_sid: u64, loc: Location, dealt_dmg: i64, drains: (i32, i32, i32)) {
        if dealt_dmg <= 0 || (drains.0 <= 0 && drains.1 <= 0) { return; }
        let healed = self.shared.clients.authenticated.get_mut(&attacker_sid).and_then(|mut atk| {
            if !apply_lifesteal(&mut atk.snapshot.stats, dealt_dmg, false, drains) { return None; }
            if let (Some(aek), Ok(tx)) = (atk.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, atk.snapshot.stats)); }
            Some((atk.handle.clone(), atk.snapshot.stats))
        });
        if let Some((ah, astats)) = healed { let _ = ah.try_send(s2c_packets::me_load_point(&astats)); }
    }

    fn resolve_player_hits(&self, attacker_sid: u64, atk_pid: PlayerId, atk_flag: i8, loc: Location, targets: &[PlayerId], cast: &CastPrep) -> Vec<EngineEffect> {
        let raw_dmg = i64::try_from(cast.pvp_dmg).unwrap_or(i64::MAX);
        let piercing = matches!(cast.skill_kind, Some(SkillKind::Makankosappo | SkillKind::TuSat));
        let mut boss_effects = Vec::new();
        for &tpid in targets {
            if tpid.0 < 0 {
                let Some(atk_view) = self.shared.clients.authenticated.get(&attacker_sid).map(|e| Self::active_entry_to_player_view(&e)) else { continue; };
                let boss_skill = nro_boss::SkillKind::from_repr(cast.skill_kind.map_or(0, |s| s as u8)).unwrap_or(nro_boss::SkillKind::Dragon);
                let mut rng = next_combat_rng(cast.now_ms, attacker_sid, tpid.0.cast_unsigned());
                let boss_res = {
                    let mut bosses = self.shared.bosses.lock();
                    if !bosses.instances().get(&tpid.0).is_some_and(|b| b.map_id == loc.map_id && b.zone_id == loc.zone_id && b.stats.hp > 0 && in_range_px(loc, b.location, cast.max_range_px)) { continue; }
                    bosses.apply_hit(tpid, &atk_view, boss_skill, raw_dmg, piercing, false, &mut rng, cast.now_ms.cast_unsigned(), &mut boss_effects).ok().map(|out| {
                        let (hp, bloc, power) = bosses.instances().get(&tpid.0).map_or((0, loc, 0), |b| (b.stats.hp.max(0), b.location, b.stats.power));
                        (out.applied_damage, out.killed, hp, bloc, power)
                    })
                };
                if let Some((applied, killed, hp_after, bloc, power)) = boss_res {
                    self.apply_attacker_lifesteal(attacker_sid, loc, applied, cast.attacker_drains);
                    self.broadcast_zone_or_self(loc, attacker_sid, &s2c_packets::have_attack_player(tpid, hp_after, applied, cast.is_crit && applied > 0, -1));
                    if killed { let (_, peer_die) = s2c_packets::char_die_packets(tpid, 0, bloc, power); self.broadcast_zone_or_self(loc, attacker_sid, &peer_die); }
                }
                continue;
            }
            if is_offline_map(loc.map_id) { continue; }
            let Some(target_sid) = self.shared.clients.players.get(&tpid).map(|s| *s.value()) else { continue; };
            if target_sid == attacker_sid { continue; }
            let hit = self.shared.clients.authenticated.get_mut(&target_sid).and_then(|mut victim| {
                let vloc = victim.snapshot.location;
                if vloc.map_id != loc.map_id || vloc.zone_id != loc.zone_id || victim.snapshot.stats.hp <= 0 || !in_range_px(loc, vloc, cast.max_range_px) { return None; }
                let pvp_ok = (atk_flag > 0 && victim.c_flag > 0 && (atk_flag != victim.c_flag || atk_flag == 8)) || lock_pvp().get(&atk_pid).is_some_and(|&(opp, _)| opp == tpid);
                if !pvp_ok { return None; }
                let v = &mut *victim;
                let (out, reflect_dmg) = apply_incoming_hit_to_player(target_sid, tpid, &mut v.snapshot.stats, &v.snapshot.items_body, raw_dmg, cast.skill_kind, cast.attacker_pen, false, cast.now_ms);
                let final_dmg = if out.dodged { 0 } else { i64::from(out.damage_taken.max(0)) };
                if let (Some(vek), Ok(tx)) = (v.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(vek, v.snapshot.stats)); }
                Some((v.handle.clone(), v.snapshot.stats.hp, final_dmg, out.dodged, reflect_dmg, v.snapshot.stats, vloc))
            });
            if let Some((vh, hp_after, final_dmg, dodged, reflect_dmg, vstats, vloc)) = hit {
                if !dodged && final_dmg > 0 {
                    super::router_skill::clear_ttnl(tpid);
                    self.apply_attacker_lifesteal(attacker_sid, loc, final_dmg, cast.attacker_drains);
                    if cast.skill_kind == Some(SkillKind::DichChuyenTucThoi) {
                        super::router_skill_effects::apply_player_cc(tpid, CcFlags::BLIND_DCTT, 1, cc_durations_ms(cast.skill_point, false).0, cast.now_ms);
                    }
                }
                let _ = vh.try_send(s2c_packets::me_load_point(&vstats));
                self.broadcast_to_zone_peers(loc, None, &s2c_packets::have_attack_player(tpid, hp_after, final_dmg, cast.is_crit && !dodged, -1));
                if reflect_dmg > 0 {
                    let atk_reflected = self.shared.clients.authenticated.get_mut(&attacker_sid).and_then(|mut atk| {
                        if atk.snapshot.stats.hp <= 1 { return None; }
                        let atk_hp_after = atk.snapshot.stats.hp.saturating_sub(reflect_dmg).max(1);
                        let applied = atk.snapshot.stats.hp.saturating_sub(atk_hp_after);
                        if applied <= 0 { return None; }
                        atk.snapshot.stats.hp = atk_hp_after;
                        if let (Some(aek), Ok(tx)) = (atk.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, atk.snapshot.stats)); }
                        Some((atk.handle.clone(), atk_hp_after, applied, atk.snapshot.stats))
                    });
                    if let Some((ah, atk_hp_after, applied, astats)) = atk_reflected {
                        let _ = ah.try_send(s2c_packets::me_load_point(&astats));
                        self.broadcast_to_zone_peers(loc, None, &s2c_packets::have_attack_player(atk_pid, atk_hp_after, applied, false, -1));
                    }
                }
                if hp_after == 0 {
                    self.handle_player_death_cleanup_with_killer(tpid, loc, target_sid, Some(atk_pid));
                    let (me_die, peer_die) = s2c_packets::char_die_packets(tpid, 0, vloc, vstats.power);
                    let _ = vh.try_send(me_die); self.broadcast_to_zone_peers(loc, Some(target_sid), &peer_die);
                }
            }
        }
        boss_effects
    }

    pub(super) async fn handle_attack_mobs(&self, session: &SessionHandle, sid: u64, mob_ids: &[i8]) -> Result<(), ProtocolError> {
        let raw_mobs: Vec<u8> = mob_ids.iter().copied().filter_map(|m| u8::try_from(m).ok()).collect();
        if raw_mobs.is_empty() { return Ok(()); }
        let mut revert_pkt = None;
        let prep = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            let ek = entry.entity_key?; if entry.snapshot.stats.hp <= 0 { return None; }
            Some((ek, entry.snapshot.location, self.prepare_attack_cast(&mut entry, sid, &mut revert_pkt)?))
        });
        if let Some((rloc, rpkt)) = revert_pkt { self.broadcast_zone_or_self(rloc, sid, &rpkt); }
        let Some((ek, loc, cast)) = prep else { return Ok(()); };
        session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
        let valid_mobs = self.filter_mobs_in_range(loc, raw_mobs, &cast);
        self.resolve_mob_hits(session, sid, ek, loc, valid_mobs, &cast).await
    }

    pub(super) fn handle_attack_players(&self, attacker_sid: u64, targets: &[PlayerId]) {
        if targets.is_empty() { return; }
        let (has_boss, mut revert_pkt) = (targets.iter().any(|t| t.0 < 0), None);
        let prep = self.shared.clients.authenticated.get_mut(&attacker_sid).and_then(|mut entry| {
            let loc = entry.snapshot.location;
            if entry.snapshot.stats.hp <= 0 || (!has_boss && is_offline_map(loc.map_id)) { return None; }
            let (atk_pid, atk_flag) = (entry.snapshot.id, entry.c_flag);
            if !has_boss && atk_flag <= 0 && !lock_pvp().contains_key(&atk_pid) { return None; }
            let cast = self.prepare_attack_cast(&mut entry, attacker_sid, &mut revert_pkt)?;
            let _ = entry.handle.try_send(s2c_packets::me_load_mp(cast.new_mp));
            Some((atk_pid, atk_flag, loc, cast))
        });
        if let Some((rloc, rpkt)) = revert_pkt { self.broadcast_zone_or_self(rloc, attacker_sid, &rpkt); }
        let Some((atk_pid, atk_flag, loc, cast)) = prep else { return; };
        let effects = self.resolve_player_hits(attacker_sid, atk_pid, atk_flag, loc, &dedup_take(targets.iter().copied(), cast.max_fight), &cast);
        if !effects.is_empty() { let shared = Arc::clone(&self.shared); tokio::spawn(async move { shared.dispatch_boss_effects(effects).await; }); }
    }

    pub(super) async fn handle_attack_multi(&self, session: &SessionHandle, sid: u64, mobs: &[i8], players: &[PlayerId]) -> Result<(), ProtocolError> {
        let raw_mobs: Vec<u8> = mobs.iter().copied().filter_map(|m| u8::try_from(m).ok()).collect();
        if raw_mobs.is_empty() && players.is_empty() { return Ok(()); }
        let mut revert_pkt = None;
        let prep = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.stats.hp <= 0 { return None; }
            Some((entry.entity_key, entry.snapshot.id, entry.snapshot.location, self.prepare_attack_cast(&mut entry, sid, &mut revert_pkt)?))
        });
        if let Some((rloc, rpkt)) = revert_pkt { self.broadcast_zone_or_self(rloc, sid, &rpkt); }
        let Some((ek, atk_pid, loc, cast)) = prep else { return Ok(()); };
        session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
        let mob_targets = if ek.is_some() { self.filter_mobs_in_range(loc, raw_mobs, &cast) } else { Vec::new() };
        let remaining = cast.max_fight.saturating_sub(mob_targets.len());
        if let Some(ek) = ek { self.resolve_mob_hits(session, sid, ek, loc, mob_targets, &cast).await?; }
        let has_boss = players.iter().any(|t| t.0 < 0);
        if remaining > 0 && !players.is_empty() && (has_boss || !is_offline_map(loc.map_id))
            && let Some(cur_flag) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
                (e.snapshot.stats.hp > 0 && e.snapshot.location == loc && (has_boss || e.c_flag > 0 || lock_pvp().contains_key(&atk_pid))).then_some(e.c_flag)
            })
        {
            let effects = self.resolve_player_hits(sid, atk_pid, cur_flag, loc, &dedup_take(players.iter().copied(), remaining), &cast);
            if !effects.is_empty() { self.shared.dispatch_boss_effects(effects).await; }
        }
        Ok(())
    }

    pub(crate) fn handle_player_death_cleanup(&self, pid: PlayerId, loc: Location, sid: u64) { self.handle_player_death_cleanup_with_killer(pid, loc, sid, None); }

    fn handle_player_death_cleanup_with_killer(&self, pid: PlayerId, loc: Location, sid: u64, killer_pid: Option<PlayerId>) {
        if let Some(kpid) = killer_pid && kpid != pid {
            let killer_peer = self.shared.clients.players.get(&kpid).map(|s| *s.value()).and_then(|ksid| {
                let e = self.shared.clients.authenticated.get(&ksid)?;
                let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                Some(SocialPeer { id: kpid.0 as i32, name: e.snapshot.name.clone(), head: h, body: b, leg: l, bag: bg as i8, power: e.snapshot.stats.power })
            });
            if let Some(peer) = killer_peer && let Some(mut victim) = self.shared.clients.authenticated.get_mut(&sid) && !victim.snapshot.enemies.iter().any(|e| e.id == peer.id) {
                victim.snapshot.enemies.push(peer);
            }
        }
        self.cleanup_trade_on_disconnect(pid);
        self.forfeit_active_pvp(pid, loc, sid);
        super::router_skill_effects::clear_player_effects(pid);
        let (cleared, reverted) = (super::router_skill::clear_skill_state(pid), super::router_skill::take_monkey_revert_pending(pid));
        if let Some((h, st, rpkt, fpkt)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let unfused = entry.snapshot.pet.as_mut().is_some_and(|p| if p.fusion_type != 0 { p.fusion_type = 0; p.status = 0; true } else { false });
            let recompute = cleared || unfused;
            if recompute {
                super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats)); }
            }
            let fpkt = unfused.then(|| { let mut w = nro_protocol::PacketWriter::with_capacity(5); w.write_i8(1); w.write_i32(pid.0 as i32); w.finish(nro_consts::cmd_message::FUSION) });
            (entry.handle.clone(), recompute.then_some(entry.snapshot.stats), (reverted || unfused).then(|| self.revert_monkey_appearance_entry(&entry, loc)), fpkt)
        }) {
            if let Some(pkt) = fpkt { self.broadcast_zone_or_self(loc, sid, &pkt); }
            if let Some(st) = st { let _ = h.try_send(s2c_packets::me_load_point(&st)); }
            if let Some(pkt) = rpkt { self.broadcast_zone_or_self(loc, sid, &pkt); }
        }
    }

    pub(super) fn cleanup_combat_on_zone_change(&self, pid: PlayerId, cur_loc: Location, sid: u64) {
        self.forfeit_active_pvp(pid, cur_loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
        super::router_skill::clear_ttnl(pid);
    }

    pub(super) fn cleanup_combat_on_disconnect(&self, pid: PlayerId, loc: Location, sid: u64) {
        self.handle_player_death_cleanup(pid, loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
    }

    pub(crate) async fn teleport_player(&self, session: &SessionHandle, sid: u64, map_id: MapId, zone_id: ZoneId, coords: (i16, i16)) -> Result<(), ProtocolError> { self.switch_player_zone(session, sid, Some(map_id), zone_id, Some(coords)).await }

    pub(super) async fn handle_pick_map_item(&self, session: &SessionHandle, sid: u64, item_map_id: i16) -> Result<(), ProtocolError> {
        let Some((ek, loc, pid)) = self.shared.clients.authenticated.get(&sid).and_then(|e| e.entity_key.map(|k| (k, e.snapshot.location, e.snapshot.id))) else { return Ok(()); };
        let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) else { return Ok(()); };
        let (reply_tx, reply_rx) = oneshot::channel();
        if zone_tx.send(ZoneCommand::PickItem(ek, item_map_id, reply_tx)).await.is_ok() && let Ok(Ok(picked)) = reply_rx.await {
            let (template_id, quantity) = picked.stack;
            let is_currency = matches!(template_id.0, 76 | 77 | 188..=190 | 861);
            let catalog = DbItemCatalog(&self.shared.templates);
            let mut in_trade = false;
            let bag_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) { in_trade = true; return Ok(None); }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                match template_id.0 {
                    76 | 188..=190 => inv.add_gold_clamped(u64::from(quantity)),
                    77 => inv.gem = inv.gem.saturating_add(quantity),
                    861 => inv.ruby = inv.ruby.saturating_add(quantity),
                    _ => inv.add_to_bag(ItemInstance { template_id, quantity, options: picked.options.iter().copied().collect(), create_time_ms: unix_now_ms() }, catalog.meta_of(template_id).stackable)?,
                }
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                Ok::<_, InventoryError>(Some((s2c_packets::inventory_bag(&entry.snapshot.items_bag), super::wallet_money_pkt(&entry.snapshot.wallet))))
            });
            if in_trade { let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await; return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
            match bag_res {
                Some(Err(InventoryError::ContainerFull(_))) => { let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await; send_thong_bao(session, "Hành trang đã đầy!")?; }
                Some(Err(_) | Ok(None)) | None => { let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await; }
                Some(Ok(Some((bag_pkt, money_pkt)))) => {
                    let (my_p, peer_p) = s2c_packets::item_map_pickup_packets(item_map_id, pid, quantity, "", is_currency)?;
                    session.send(my_p).await?; self.broadcast_to_zone_peers(loc, Some(sid), &peer_p);
                    session.send(bag_pkt).await?; session.send(money_pkt).await?;
                    self.advance_task_on_pick_item(session, sid, template_id).await?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn broadcast_to_zone_peers(&self, loc: Location, exclude_sid: Option<u64>, pkt: &Packet) {
        if is_offline_map(loc.map_id) { return; }
        for rec in &self.shared.clients.authenticated {
            if exclude_sid != Some(*rec.key()) && rec.value().snapshot.location.map_id == loc.map_id && rec.value().snapshot.location.zone_id == loc.zone_id { let _ = rec.value().handle.try_send(pkt.clone()); }
        }
    }

    pub(super) fn broadcast_zone_or_self(&self, loc: Location, actor_sid: u64, pkt: &Packet) {
        if is_offline_map(loc.map_id) { if let Some(rec) = self.shared.clients.authenticated.get(&actor_sid) { let _ = rec.value().handle.try_send(pkt.clone()); } } else { self.broadcast_to_zone_peers(loc, None, pkt); }
    }
}
