//! Combat, PvP, PK flags (`-103`), social/enemy revenge (`-80`, `-99`), pickup, revive, whispers, and leaderboards.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{CoreStats, EntityKey, GameRng, Gender, ItemInstance, Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_economy::{InventoryError, ItemCatalog};
use nro_player::{combat::IncomingHit, npoint::NPoint};
use nro_protocol::{FlagAction, Packet, PacketWriter, ProtocolError, SessionHandle, SocialAction, TradeAction};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::{DbItemCatalog, ServerSessionHandler, effective_appearance, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt};
use crate::runtime::{LeaderboardSlot, unix_now_ms};
use crate::state::ActivePlayerEntry;

const DEFAULT_FLAGS: &[(i16, i16)] = &[(519, 2325), (520, 2326), (521, 2327), (522, 2328), (523, 2329), (524, 2330), (525, 2331), (526, 2332), (527, 2333)];

static ACTIVE_PVP: OnceLock<Mutex<HashMap<PlayerId, PlayerId>>> = OnceLock::new();
fn lock_pvp() -> MutexGuard<'static, HashMap<PlayerId, PlayerId>> {
    ACTIVE_PVP.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
static PENDING_PVP: OnceLock<DashMap<(PlayerId, PlayerId), i64>> = OnceLock::new();
fn pending_pvp() -> &'static DashMap<(PlayerId, PlayerId), i64> { PENDING_PVP.get_or_init(DashMap::new) }

#[inline]
pub(super) const fn is_offline_map(map_id: MapId) -> bool { matches!(map_id.0, 21..=23 | 39..=41 | 45..=50) }

fn dedup_take<T: Copy + PartialEq>(items: impl IntoIterator<Item = T>, limit: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(limit.min(16));
    for item in items { if !out.contains(&item) { out.push(item); if out.len() >= limit { break; } } }
    out
}

struct CastPrep { skill_paint_id: u8, mob_dmg: u64, pvp_dmg: u64, is_crit: bool, new_mp: i64, now_ms: i64, max_fight: usize }
enum RevivePrep { Alive, NoGems, InTrade, Ready { pid: PlayerId, gender: Gender, stats: CoreStats, orig_loc: Location, ek: Option<EntityKey>, money_pkt: Packet } }

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
                let pvp_ok = (atk_flag > 0 && victim.c_flag > 0 && (atk_flag != victim.c_flag || atk_flag == 8)) || lock_pvp().get(&atk_pid) == Some(&tpid);
                if !pvp_ok { return None; }
                let mut np = NPoint::new(victim.snapshot.stats);
                np.hp_max = i32::try_from(victim.snapshot.stats.hp_max.clamp(1, i64::from(i32::MAX))).unwrap_or(i32::MAX);
                np.hp = i32::try_from(victim.snapshot.stats.hp.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX);
                np.def = victim.snapshot.stats.def.max(0);
                let mut rng = GameRng::from_seed(cast.now_ms.cast_unsigned() ^ target_sid);
                let shield = super::router_skill::is_shield_active(tpid, cast.now_ms);
                let out = np.apply_injured(&IncomingHit { raw_damage: raw_dmg, attacker_skill: None, attacker_pen: (0, 0, 0), hit_flags: (false, false, false), defense_state: (false, false, shield, false, false) }, &mut rng);
                if out.shield_broken { super::router_skill::clear_shield(tpid); }
                let final_dmg = i64::from(out.damage_taken.max(0));
                let hp_after = victim.snapshot.stats.hp.saturating_sub(final_dmg).max(0);
                victim.snapshot.stats.hp = hp_after;
                if let (Some(vek), Ok(tx)) = (victim.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(vek, victim.snapshot.stats)); }
                Some((victim.handle.clone(), hp_after, final_dmg, victim.snapshot.stats, vloc))
            });
            if let Some((vh, hp_after, final_dmg, vstats, vloc)) = hit {
                super::router_skill::clear_ttnl(tpid);
                let _ = vh.try_send(s2c_packets::me_load_point(&vstats));
                self.broadcast_to_zone_peers(loc, None, &s2c_packets::have_attack_player(tpid, hp_after, final_dmg, cast.is_crit, -1));
                if hp_after == 0 {
                    self.handle_player_death_cleanup(tpid, loc, target_sid);
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
        let opp_opt = {
            let mut pvp = lock_pvp();
            let opp = pvp.remove(&pid);
            if let Some(opp_id) = opp && pvp.get(&opp_id) == Some(&pid) { pvp.remove(&opp_id); }
            opp
        };
        if let Some(opp) = opp_opt {
            let (opp_sid, opp_loc) = self.shared.clients.players.get(&opp).map(|s| *s.value())
                .and_then(|os| self.shared.clients.authenticated.get(&os).map(|e| (os, e.snapshot.location))).unwrap_or((sid, loc));
            self.broadcast_pk_type(loc, sid, pid, 0); self.broadcast_pk_type(opp_loc, opp_sid, opp, 0);
        }
        super::router_skill::clear_skill_state(pid);
        self.cleanup_trade_on_disconnect(pid);
    }

    pub(super) fn cleanup_combat_on_disconnect(&self, pid: PlayerId, loc: Location, sid: u64) {
        self.handle_player_death_cleanup(pid, loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
    }

    fn broadcast_pk_type(&self, loc: Location, actor_sid: u64, pid: PlayerId, pk_type: i8) {
        let mut pk = s2c_packets::sub_command_writer(cmd_message::UPDATE_TYPE_PK, 5);
        pk.write_i32(pid.0 as i32); pk.write_i8(pk_type);
        self.broadcast_zone_or_self(loc, actor_sid, &pk.finish(cmd_message::SUB_COMMAND));
    }

    pub(super) async fn handle_pvp_challenge(&self, session: &SessionHandle, sid: u64, action: i8, pvp_type: i8, target_pid: PlayerId) -> Result<(), ProtocolError> {
        let Some((my_pid, my_name, loc, my_hp)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.location, e.snapshot.stats.hp)) else { return Ok(()); };
        if is_offline_map(loc.map_id) || my_hp <= 0 || my_pid == target_pid { return Ok(()); }
        let Some(target_sid) = self.shared.clients.players.get(&target_pid).map(|s| *s.value()) else { return session.send(s2c_packets::thong_bao("Người chơi không còn trực tuyến!")?).await; };
        if action == 0 && let Some(t_entry) = self.shared.clients.authenticated.get(&target_sid) {
            let tloc = t_entry.snapshot.location;
            if t_entry.snapshot.stats.hp <= 0 || tloc.map_id != loc.map_id || tloc.zone_id != loc.zone_id || { let pvp = lock_pvp(); pvp.contains_key(&my_pid) || pvp.contains_key(&target_pid) } { return Ok(()); }
            pending_pvp().insert((my_pid, target_pid), unix_now_ms().saturating_add(30_000));
            let mut pw = PacketWriter::with_capacity(24 + my_name.len());
            pw.write_i8(pvp_type); pw.write_i32(my_pid.0 as i32); pw.write_i32(1_000); pw.write_utf(&format!("{my_name} mời bạn thách đấu!"))?;
            let _ = t_entry.handle.try_send(pw.finish(cmd_message::PLAYER_VS_PLAYER));
        } else if action == 1 {
            let now_ms = unix_now_ms();
            let target_ok = self.shared.clients.authenticated.get(&target_sid).is_some_and(|t| t.snapshot.stats.hp > 0 && t.snapshot.location.map_id == loc.map_id && t.snapshot.location.zone_id == loc.zone_id);
            if !target_ok || pending_pvp().remove(&(target_pid, my_pid)).is_none_or(|(_, exp)| exp < now_ms) { return Ok(()); }
            {
                let mut pvp = lock_pvp();
                if pvp.contains_key(&my_pid) || pvp.contains_key(&target_pid) { return Ok(()); }
                pvp.insert(my_pid, target_pid);
                pvp.insert(target_pid, my_pid);
            }
            for pid in [my_pid, target_pid] { self.broadcast_pk_type(loc, sid, pid, 3); }
        }
        Ok(())
    }

    pub(super) async fn handle_change_flag(&self, session: &SessionHandle, sid: u64, action: FlagAction) -> Result<(), ProtocolError> {
        let tpl_flags: Vec<(i16, i16)> = self.shared.templates.items.iter().filter(|r| r.item_type == 28).take(12).map(|r| (r.id as i16, r.icon_id as i16)).collect();
        let flags: &[(i16, i16)] = if tpl_flags.is_empty() { DEFAULT_FLAGS } else { &tpl_flags };
        match action {
            FlagAction::OpenUi => {
                let mut w = PacketWriter::with_capacity(4 + flags.len() * 9); w.write_i8(0); w.write_i8(flags.len() as i8);
                for &(tid, _) in flags { w.write_i16(tid); w.write_i8(1); w.write_i16(73); w.write_i32(0); }
                session.send(w.finish(cmd_message::CHANGE_FLAG)).await?;
            }
            FlagAction::Choose(idx) => {
                if let Some((pkt, loc)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let cf = idx.clamp(0, flags.len() as i8); entry.c_flag = cf;
                    let mut w = PacketWriter::with_capacity(6); w.write_i8(1); w.write_i32(entry.snapshot.id.0 as i32); w.write_i8(cf);
                    (w.finish(cmd_message::CHANGE_FLAG), entry.snapshot.location)
                }) { self.broadcast_zone_or_self(loc, sid, &pkt); }
            }
            FlagAction::RequestIcon(cf) => {
                let icon = usize::try_from(cf.saturating_sub(1)).ok().and_then(|i| flags.get(i)).map_or(2325, |&(_, ic)| ic);
                let mut w = PacketWriter::with_capacity(4); w.write_i8(2); w.write_i8(cf); w.write_i16(icon);
                session.send(w.finish(cmd_message::CHANGE_FLAG)).await?;
            }
            FlagAction::Unknown(_) => {}
        }
        Ok(())
    }

    pub(super) async fn handle_social(&self, session: &SessionHandle, sid: u64, action: SocialAction, is_friend: bool) -> Result<(), ProtocolError> {
        match action {
            SocialAction::PrimaryTarget(tpid) if is_friend => {
                if let Some(peer) = self.shared.clients.players.get(&tpid).and_then(|s| {
                    let e = self.shared.clients.authenticated.get(s.value())?;
                    let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                    Some(SocialPeer { id: tpid.0 as i32, name: e.snapshot.name.clone(), head: h, body: b, leg: l, bag: bg as i8, power: e.snapshot.stats.power })
                }) && let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) && !me.snapshot.friends.iter().any(|f| f.id == peer.id) { me.snapshot.friends.push(peer); }
                session.send(s2c_packets::thong_bao("Đã thêm vào danh sách bạn bè!")?).await?;
            }
            SocialAction::PrimaryTarget(tpid) => {
                if i32::try_from(tpid.0).is_ok_and(|id| self.shared.clients.authenticated.get(&sid).is_some_and(|me| me.snapshot.enemies.iter().any(|e| e.id == id))) { self.handle_goto_player(session, sid, tpid).await?; }
            }
            SocialAction::RemoveTarget(tpid) => {
                if let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) { (if is_friend { &mut me.snapshot.friends } else { &mut me.snapshot.enemies }).retain(|p| p.id != tpid.0 as i32); }
            }
            SocialAction::OpenList | SocialAction::Unknown(_) => {}
        }
        let peers = self.shared.clients.authenticated.get(&sid).map_or_default(|e| if is_friend { e.snapshot.friends.clone() } else { e.snapshot.enemies.clone() });
        let mut w = PacketWriter::with_capacity(8 + peers.len() * 32); w.write_i8(0); w.write_u8(peers.len().min(100) as u8);
        for p in peers.iter().take(100) {
            let online = self.shared.clients.players.contains_key(&PlayerId(i64::from(p.id)));
            w.write_i32(p.id); for v in [p.head, -1, p.body, p.leg, i16::from(p.bag)] { w.write_i16(v); } w.write_utf(&p.name)?;
            if is_friend { w.write_bool(online); w.write_utf(&p.power.to_string())?; } else { w.write_utf(&p.power.to_string())?; w.write_bool(online); }
        }
        session.send(w.finish(if is_friend { cmd_message::FRIEND } else { cmd_message::ENEMY_LIST })).await
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

    pub(super) async fn handle_revive(&self, session: &SessionHandle, sid: u64, return_home: bool) -> Result<(), ProtocolError> {
        let prep = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if entry.snapshot.stats.hp > 0 { return RevivePrep::Alive; }
            if !return_home {
                if super::router_shop::is_in_trade(entry.snapshot.id) { return RevivePrep::InTrade; }
                if entry.snapshot.wallet.ruby >= 1 { entry.snapshot.wallet.ruby -= 1; } else if entry.snapshot.wallet.gem >= 1 { entry.snapshot.wallet.gem -= 1; } else { return RevivePrep::NoGems; }
            }
            entry.snapshot.stats.hp = entry.snapshot.stats.hp_max.max(1); entry.snapshot.stats.mp = entry.snapshot.stats.mp_max.max(1);
            RevivePrep::Ready { pid: entry.snapshot.id, gender: entry.snapshot.gender, stats: entry.snapshot.stats, orig_loc: entry.snapshot.location, ek: entry.entity_key, money_pkt: wallet_money_pkt(&entry.snapshot.wallet) }
        });
        let Some(RevivePrep::Ready { pid, gender, stats, orig_loc, ek, money_pkt }) = prep else {
            if matches!(prep, Some(RevivePrep::NoGems)) { session.send(s2c_packets::thong_bao("Bạn không đủ ngọc để hồi sinh tại chỗ!")?).await?; }
            else if matches!(prep, Some(RevivePrep::InTrade)) { send_thong_bao(session, "Không thể hồi sinh tại chỗ khi đang giao dịch!")?; }
            return Ok(());
        };
        if let (Some(entity_key), Ok(tx)) = (ek, self.shared.world_router.resolve_sender(orig_loc.map_id, orig_loc.zone_id)) { let _ = tx.send(ZoneCommand::UpdatePlayerStats(entity_key, stats)).await; }
        let (me_live_pkt, _) = s2c_packets::char_revive_packets(pid, stats.hp, stats.hp_max, orig_loc);
        session.send(me_live_pkt).await?;
        if return_home { self.switch_player_zone(session, sid, Some(MapId(21 + i16::from(gender))), ZoneId(0), Some((300, 336))).await?; } else { session.send(money_pkt).await?; }
        let final_loc = self.shared.clients.authenticated.get(&sid).map_or(orig_loc, |e| e.snapshot.location);
        let (_, peer_live_pkt) = s2c_packets::char_revive_packets(pid, stats.hp, stats.hp_max, final_loc);
        session.send(s2c_packets::me_load_point(&stats)).await?;
        self.broadcast_to_zone_peers(final_loc, Some(sid), &peer_live_pkt);
        Ok(())
    }

    pub(super) async fn handle_player_menu(&self, session: &SessionHandle, _sid: u64, target_pid: PlayerId, with_submenu: bool) -> Result<(), ProtocolError> {
        let pwr = self.shared.clients.players.get(&target_pid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| e.snapshot.stats.power)).unwrap_or(2_000);
        let mut w = PacketWriter::with_capacity(28); w.write_i32(target_pid.0 as i32); w.write_i64(pwr); w.write_utf("Chiến binh")?;
        session.send(w.finish(cmd_message::PLAYER_MENU)).await?;
        if with_submenu {
            let mut sub = s2c_packets::sub_command_writer(cmd_message::GET_PLAYER_MENU, 48); sub.write_i8(2);
            for (c1, c2, mid) in [("Kết bạn", "Thêm bạn bè", 0_i16), ("Giao dịch", "Mời giao dịch", 1_i16)] { sub.write_utf(c1)?; sub.write_utf(c2)?; sub.write_i16(mid); }
            session.send(sub.finish(cmd_message::SUB_COMMAND)).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_player_menu_action(&self, session: &SessionHandle, sid: u64, target_pid: PlayerId, menu_id: i16) -> Result<(), ProtocolError> {
        match menu_id { 0 => self.handle_social(session, sid, SocialAction::PrimaryTarget(target_pid), true).await, 1 => self.handle_trade(session, sid, TradeAction::InviteOrAccept(0, Some(target_pid))).await, _ => Ok(()) }
    }

    pub(super) async fn handle_whisper(&self, session: &SessionHandle, sid: u64, target: PlayerId, text: &str) -> Result<(), ProtocolError> {
        let Some(pkt) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
            s2c_packets::chat_global(e.snapshot.id, &e.snapshot.name, &format!("[Mật] {text}"), (h, b, bg, l)).ok()
        }) else { return Ok(()); };
        if let Some(th) = self.handle_for_pid(target) { let _ = th.try_send(pkt); } else { session.send(s2c_packets::thong_bao("Người chơi hiện không trực tuyến!")?).await?; }
        Ok(())
    }

    pub(super) async fn handle_top_query(&self, session: &SessionHandle, _sid: u64, name: &str, _page: i8) -> Result<(), ProtocolError> {
        let slot = if name.contains("NV") || name.contains("Nhiệm") { LeaderboardSlot::Task } else { LeaderboardSlot::Power };
        let rows = self.shared.leaderboards.snapshot(slot).await;
        let mut w = PacketWriter::with_capacity(16 + rows.len() * 36);
        w.write_i8(0); w.write_utf(if name.is_empty() { "Bảng Xếp Hạng Sức Mạnh" } else { name })?; w.write_i8(rows.len().min(50) as i8);
        for r in rows.iter().take(50) {
            w.write_i32(r.rank as i32); w.write_i32(r.rank as i32); for v in [64_i16, -1, 57, 58] { w.write_i16(v); }
            w.write_utf(&r.player_name)?; w.write_utf(&format!("Sức mạnh: {}", r.score))?; w.write_utf("")?;
        }
        session.send(w.finish(cmd_message::TOP)).await
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
