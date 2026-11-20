//! Core combat, attack resolution, death cleanup, item pickup, teleportation, and zone broadcasts.

use nro_core::{EntityKey, GameRng, ItemInstance, Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_economy::{InventoryError, ItemCatalog};
use nro_player::{combat::IncomingHit, npoint::NPoint};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::router_combat_pvp::{lock_pvp, pending_pvp};
use super::{DbItemCatalog, ServerSessionHandler, effective_appearance, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt};
use crate::runtime::{LeaderboardSlot, unix_now_ms};
use crate::state::ActivePlayerEntry;

#[inline]
pub(super) const fn is_offline_map(map_id: MapId) -> bool { matches!(map_id.0, 21..=23 | 39..=41 | 45..=50) }

fn dedup_take<T: Copy + PartialEq>(items: impl IntoIterator<Item = T>, limit: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(limit.min(16));
    for item in items { if !out.contains(&item) { out.push(item); if out.len() >= limit { break; } } }
    out
}

struct CastPrep { skill_paint_id: u8, mob_dmg: u64, pvp_dmg: u64, is_crit: bool, new_mp: i64, now_ms: i64, max_fight: usize }

impl ServerSessionHandler {
    fn prepare_attack_cast(&self, entry: &mut ActivePlayerEntry, sid: u64) -> Option<CastPrep> {
        let (sel, now_ms) = (entry.selected_skill, unix_now_ms());
        super::router_skill::clear_ttnl(entry.snapshot.id);
        if !matches!(sel, 0..=5 | 9..=11 | 17 | 20 | 24..=26) { return None; }
        let sk_idx = entry.snapshot.skills.iter().position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
        let st = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == sel)?;
        let lv = st.levels.iter().find(|l| l.point == entry.snapshot.skills[sk_idx].point)?;
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
        let skill_paint_id = u8::try_from(lv.id.clamp(0, 255)).unwrap_or(0);
        let base_dame = entry.snapshot.stats.dame.max(1).cast_unsigned();
        let pct = u64::try_from(lv.damage.max(100)).unwrap_or(100);
        let charm_pct = u64::try_from(entry.snapshot.charms.damage_multiplier_pct(now_ms).max(100)).unwrap_or(100);
        let is_crit = GameRng::from_seed(now_ms.cast_unsigned() ^ sid).is_true(u32::from(entry.snapshot.stats.crit), 100);
        let raw = base_dame.saturating_mul(pct) / 100;
        let (mut mob_dmg, mut pvp_dmg) = ((raw.saturating_mul(charm_pct) / 100).max(1), raw.max(1));
        if is_crit { mob_dmg = mob_dmg.saturating_mul(2); pvp_dmg = pvp_dmg.saturating_mul(2); }
        Some(CastPrep { skill_paint_id, mob_dmg, pvp_dmg, is_crit, new_mp, now_ms, max_fight: usize::try_from(lv.max_fight.max(1)).unwrap_or(1) })
    }

    async fn resolve_mob_hits(&self, session: &SessionHandle, sid: u64, ek: EntityKey, loc: Location, valid_mobs: Vec<u8>, cast: &CastPrep) -> Result<(), ProtocolError> {
        if valid_mobs.is_empty() { return Ok(()); }
        let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) else { return Ok(()); };
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = ZoneCommand::AttackMobs { attacker: ek, skill_paint_id: cast.skill_paint_id, mob_ids: valid_mobs, damage: cast.mob_dmg, is_crit: cast.is_crit, reply: reply_tx };
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
            for mob_tid in killed_mobs { self.advance_task_on_mob_kill(session, sid, mob_tid).await?; }
        }
        Ok(())
    }

    fn resolve_player_hits(&self, attacker_sid: u64, atk_pid: PlayerId, atk_flag: i8, loc: Location, targets: &[PlayerId], cast: &CastPrep) {
        let raw_dmg = i64::try_from(cast.pvp_dmg).unwrap_or(i64::MAX);
        for &tpid in targets {
            let Some(target_sid) = self.shared.clients.players.get(&tpid).map(|s| *s.value()) else { continue; };
            if target_sid == attacker_sid { continue; }
            let hit = self.shared.clients.authenticated.get_mut(&target_sid).and_then(|mut victim| {
                let vloc = victim.snapshot.location;
                if vloc.map_id != loc.map_id || vloc.zone_id != loc.zone_id || victim.snapshot.stats.hp <= 0 { return None; }
                let pvp_ok = (atk_flag > 0 && victim.c_flag > 0 && (atk_flag != victim.c_flag || atk_flag == 8))
                    || lock_pvp().get(&atk_pid).is_some_and(|&(opp, _)| opp == tpid);
                if !pvp_ok { return None; }
                let mut np = NPoint::new(victim.snapshot.stats);
                np.hp_max = i32::try_from(victim.snapshot.stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
                np.hp = i32::try_from(victim.snapshot.stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
                np.def = victim.snapshot.stats.def.max(0);
                for opt in victim.snapshot.items_body.iter().flatten().flat_map(|it| it.options.iter()) {
                    let ps = opt.param.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
                    match opt.id {
                        3 => np.mods.vo_hieu_chuong = np.mods.vo_hieu_chuong.saturating_add(opt.param),
                        94 => np.mods.tl_giap = np.mods.tl_giap.saturating_add(ps),
                        97 => np.mods.utils[0] = np.mods.utils[0].saturating_add(ps),
                        108 | 111 => np.mods.tl_ne_don = np.mods.tl_ne_don.saturating_add(ps),
                        _ => {}
                    }
                }
                let mut rng = GameRng::from_seed(cast.now_ms.cast_unsigned() ^ target_sid);
                let shield = super::router_skill::is_shield_active(tpid, cast.now_ms);
                let out = np.apply_injured(&IncomingHit { raw_damage: raw_dmg, attacker_skill: None, attacker_pen: (0, 0, 0), hit_flags: (false, false, false), defense_state: (false, false, shield, false, false) }, &mut rng);
                if out.shield_broken { super::router_skill::clear_shield(tpid); }
                let final_dmg = if out.dodged { 0 } else { i64::from(out.damage_taken.max(0)) };
                let hp_after = if out.dodged {
                    victim.snapshot.stats.hp
                } else {
                    victim.snapshot.stats.hp.saturating_sub(final_dmg).max(0)
                };
                victim.snapshot.stats.hp = hp_after;
                if out.mp_absorbed > 0 {
                    let mp_max = victim.snapshot.stats.mp_max.max(1);
                    victim.snapshot.stats.mp = victim.snapshot.stats.mp.saturating_add(i64::from(out.mp_absorbed)).min(mp_max);
                }
                let reflect_pct = i64::from(np.mods.utils[0].max(0));
                let reflect_dmg = if out.reflect_base_damage > 0 && reflect_pct > 0 {
                    (i64::from(out.reflect_base_damage).saturating_mul(reflect_pct) / 100).max(1)
                } else {
                    0
                };
                if let (Some(vek), Ok(tx)) = (victim.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(vek, victim.snapshot.stats)); }
                Some((victim.handle.clone(), hp_after, final_dmg, out.dodged, reflect_dmg, victim.snapshot.stats, vloc))
            });
            if let Some((vh, hp_after, final_dmg, dodged, reflect_dmg, vstats, vloc)) = hit {
                if !dodged && final_dmg > 0 {
                    super::router_skill::clear_ttnl(tpid);
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
                        if let (Some(aek), Ok(tx)) = (atk.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                            let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, atk.snapshot.stats));
                        }
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
    }

    pub(super) async fn handle_attack_mobs(&self, session: &SessionHandle, sid: u64, mob_ids: &[i8]) -> Result<(), ProtocolError> {
        let raw_mobs: Vec<u8> = mob_ids.iter().copied().filter_map(|m| u8::try_from(m).ok()).collect();
        if raw_mobs.is_empty() { return Ok(()); }
        let Some((ek, loc, cast)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            let ek = entry.entity_key?; if entry.snapshot.stats.hp <= 0 { return None; }
            Some((ek, entry.snapshot.location, self.prepare_attack_cast(&mut entry, sid)?))
        }) else { return Ok(()); };
        session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
        self.resolve_mob_hits(session, sid, ek, loc, dedup_take(raw_mobs, cast.max_fight), &cast).await
    }

    pub(super) fn handle_attack_players(&self, attacker_sid: u64, targets: &[PlayerId]) {
        if targets.is_empty() { return; }
        let Some((atk_pid, atk_flag, loc, cast)) = self.shared.clients.authenticated.get_mut(&attacker_sid).and_then(|mut entry| {
            let loc = entry.snapshot.location;
            if entry.snapshot.stats.hp <= 0 || is_offline_map(loc.map_id) { return None; }
            let (atk_pid, atk_flag) = (entry.snapshot.id, entry.c_flag);
            if atk_flag <= 0 && !lock_pvp().contains_key(&atk_pid) { return None; }
            let cast = self.prepare_attack_cast(&mut entry, attacker_sid)?;
            let _ = entry.handle.try_send(s2c_packets::me_load_mp(cast.new_mp));
            Some((atk_pid, atk_flag, loc, cast))
        }) else { return; };
        self.resolve_player_hits(attacker_sid, atk_pid, atk_flag, loc, &dedup_take(targets.iter().copied(), cast.max_fight), &cast);
    }

    pub(super) async fn handle_attack_multi(&self, session: &SessionHandle, sid: u64, mobs: &[i8], players: &[PlayerId]) -> Result<(), ProtocolError> {
        let raw_mobs: Vec<u8> = mobs.iter().copied().filter_map(|m| u8::try_from(m).ok()).collect();
        if raw_mobs.is_empty() && players.is_empty() { return Ok(()); }
        let Some((ek, atk_pid, loc, cast)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.stats.hp <= 0 { return None; }
            Some((entry.entity_key, entry.snapshot.id, entry.snapshot.location, self.prepare_attack_cast(&mut entry, sid)?))
        }) else { return Ok(()); };
        session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
        let mob_targets = if ek.is_some() { dedup_take(raw_mobs, cast.max_fight) } else { Vec::new() };
        let remaining = cast.max_fight.saturating_sub(mob_targets.len());
        if let Some(ek) = ek { self.resolve_mob_hits(session, sid, ek, loc, mob_targets, &cast).await?; }
        if remaining > 0 && !players.is_empty() && !is_offline_map(loc.map_id)
            && let Some(cur_flag) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
                (e.snapshot.stats.hp > 0 && e.snapshot.location == loc && (e.c_flag > 0 || lock_pvp().contains_key(&atk_pid))).then_some(e.c_flag)
            })
        { self.resolve_player_hits(sid, atk_pid, cur_flag, loc, &dedup_take(players.iter().copied(), remaining), &cast); }
        Ok(())
    }

    pub(crate) fn handle_player_death_cleanup(&self, pid: PlayerId, loc: Location, sid: u64) {
        self.handle_player_death_cleanup_with_killer(pid, loc, sid, None);
    }

    fn handle_player_death_cleanup_with_killer(&self, pid: PlayerId, loc: Location, sid: u64, killer_pid: Option<PlayerId>) {
        if let Some(kpid) = killer_pid && kpid != pid {
            let killer_peer = self.shared.clients.players.get(&kpid).map(|s| *s.value()).and_then(|ksid| {
                let e = self.shared.clients.authenticated.get(&ksid)?;
                let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                Some(SocialPeer {
                    id: kpid.0 as i32,
                    name: e.snapshot.name.clone(),
                    head: h,
                    body: b,
                    leg: l,
                    bag: bg as i8,
                    power: e.snapshot.stats.power,
                })
            });
            if let Some(peer) = killer_peer
                && let Some(mut victim) = self.shared.clients.authenticated.get_mut(&sid)
                && !victim.snapshot.enemies.iter().any(|e| e.id == peer.id)
            {
                victim.snapshot.enemies.push(peer);
            }
        }
        let opp_opt = {
            let mut pvp = lock_pvp();
            let opp = pvp.remove(&pid);
            if let Some((opp_id, _)) = opp && pvp.get(&opp_id).is_some_and(|&(other, _)| other == pid) { pvp.remove(&opp_id); }
            opp
        };
        if let Some((opp, stake)) = opp_opt {
            let opp_sid_opt = self.shared.clients.players.get(&opp).map(|s| *s.value());
            let opp_loc = opp_sid_opt
                .and_then(|os| self.shared.clients.authenticated.get(&os).map(|e| e.snapshot.location))
                .unwrap_or(loc);
            self.broadcast_pk_type(loc, sid, pid, 0);
            self.broadcast_pk_type(opp_loc, opp_sid_opt.unwrap_or(sid), opp, 0);
            if stake > 0
                && let Some(opp_sid) = opp_sid_opt
                && let Some((transferred, loser_handle, loser_money_pkt)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut loser| {
                    let transferred = stake.min(loser.snapshot.wallet.gold.max(0));
                    loser.snapshot.wallet.gold = loser.snapshot.wallet.gold.saturating_sub(transferred).max(0);
                    (transferred, loser.handle.clone(), wallet_money_pkt(&loser.snapshot.wallet))
                })
            {
                let winner_res = self.shared.clients.authenticated.get_mut(&opp_sid).map(|mut winner| {
                    winner.snapshot.wallet.gold = winner.snapshot.wallet.gold.saturating_add(transferred);
                    (winner.handle.clone(), wallet_money_pkt(&winner.snapshot.wallet))
                });
                let _ = loser_handle.try_send(loser_money_pkt);
                if let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn đã thua thách đấu và mất {transferred} vàng!")) {
                    let _ = loser_handle.try_send(tb);
                }
                if let Some((winner_handle, winner_money_pkt)) = winner_res {
                    let _ = winner_handle.try_send(winner_money_pkt);
                    if let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn đã thắng thách đấu và nhận được {transferred} vàng!")) {
                        let _ = winner_handle.try_send(tb);
                    }
                }
            }
        }
        super::router_skill::clear_skill_state(pid);
        self.cleanup_trade_on_disconnect(pid);
    }

    pub(super) fn cleanup_combat_on_disconnect(&self, pid: PlayerId, loc: Location, sid: u64) {
        self.handle_player_death_cleanup(pid, loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
    }

    pub(crate) async fn teleport_player(&self, session: &SessionHandle, sid: u64, map_id: MapId, zone_id: ZoneId, coords: (i16, i16)) -> Result<(), ProtocolError> {
        self.switch_player_zone(session, sid, Some(map_id), zone_id, Some(coords)).await
    }

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
                Ok::<_, InventoryError>(Some((s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet))))
            });
            if in_trade {
                let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await;
                return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
            }
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
