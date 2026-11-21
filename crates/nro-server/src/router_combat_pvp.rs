//! PvP challenges, PK flags (`-103`), social/enemy revenge (`-80`, `-99`), revive, player menus, whispers, and leaderboards.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{CoreStats, EntityKey, Gender, Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_protocol::{FlagAction, Packet, PacketWriter, ProtocolError, SessionHandle, SocialAction, TradeAction};
use nro_world::{ZoneCommand, s2c_packets};

use super::router_combat::is_offline_map;
use super::{ServerSessionHandler, effective_appearance, send_thong_bao, wallet_money_pkt};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

const DEFAULT_FLAGS: &[(i16, i16)] = &[(519, 2325), (520, 2326), (521, 2327), (522, 2328), (523, 2329), (524, 2330), (525, 2331), (526, 2332), (527, 2333)];
const PVP_CHALLENGE_STAKE: i64 = 1_000;
const FLAG_CHANGE_COOLDOWN_MS: i64 = 60_000;

static ACTIVE_PVP: OnceLock<Mutex<HashMap<PlayerId, (PlayerId, i64)>>> = OnceLock::new();
pub(super) fn lock_pvp() -> MutexGuard<'static, HashMap<PlayerId, (PlayerId, i64)>> {
    ACTIVE_PVP.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
static PENDING_PVP: OnceLock<DashMap<(PlayerId, PlayerId), (i64, i64)>> = OnceLock::new();
pub(super) fn pending_pvp() -> &'static DashMap<(PlayerId, PlayerId), (i64, i64)> { PENDING_PVP.get_or_init(DashMap::new) }
static FLAG_COOLDOWN: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn flag_cooldown() -> &'static DashMap<PlayerId, i64> { FLAG_COOLDOWN.get_or_init(DashMap::new) }
pub(super) fn clear_flag_cooldown(pid: PlayerId) { flag_cooldown().remove(&pid); }

enum RevivePrep { Alive, NoGems, InTrade, Ready { pid: PlayerId, gender: Gender, stats: CoreStats, orig_loc: Location, ek: Option<EntityKey>, money_pkt: Packet } }

impl ServerSessionHandler {
    pub(super) fn broadcast_pk_type(&self, loc: Location, actor_sid: u64, pid: PlayerId, pk_type: i8) {
        let mut pk = s2c_packets::sub_command_writer(cmd_message::UPDATE_TYPE_PK, 5);
        pk.write_i32(pid.0 as i32); pk.write_i8(pk_type);
        self.broadcast_zone_or_self(loc, actor_sid, &pk.finish(cmd_message::SUB_COMMAND));
    }

    pub(super) fn forfeit_active_pvp(&self, pid: PlayerId, loc: Location, sid: u64) {
        let opp_opt = {
            let mut pvp = lock_pvp();
            let opp = pvp.remove(&pid);
            if let Some((opp_id, _)) = opp && pvp.get(&opp_id).is_some_and(|&(other, _)| other == pid) { pvp.remove(&opp_id); }
            opp
        };
        let Some((opp, stake)) = opp_opt else { return; };
        self.cleanup_trade_on_disconnect(opp);
        let opp_sid_opt = self.shared.clients.players.get(&opp).map(|s| *s.value());
        let opp_loc = opp_sid_opt.and_then(|os| self.shared.clients.authenticated.get(&os).map(|e| e.snapshot.location)).unwrap_or(loc);
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
                winner.snapshot.wallet.gold = winner.snapshot.wallet.gold.saturating_add(transferred).min(nro_db::player_repo::LIMIT_GOLD);
                (winner.handle.clone(), wallet_money_pkt(&winner.snapshot.wallet))
            });
            let _ = loser_handle.try_send(loser_money_pkt);
            if let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn đã thua thách đấu và mất {transferred} vàng!")) { let _ = loser_handle.try_send(tb); }
            if let Some((winner_handle, winner_money_pkt)) = winner_res {
                let _ = winner_handle.try_send(winner_money_pkt);
                if let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn đã thắng thách đấu và nhận được {transferred} vàng!")) { let _ = winner_handle.try_send(tb); }
            }
        }
    }

    pub(super) async fn handle_pvp_challenge(&self, session: &SessionHandle, sid: u64, action: i8, pvp_type: i8, target_pid: PlayerId) -> Result<(), ProtocolError> {
        let Some((my_pid, my_name, loc, my_hp, my_gold)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.location, e.snapshot.stats.hp, e.snapshot.wallet.gold)) else { return Ok(()); };
        if is_offline_map(loc.map_id) || my_hp <= 0 || my_pid == target_pid { return Ok(()); }
        let Some(target_sid) = self.shared.clients.players.get(&target_pid).map(|s| *s.value()) else { return session.send(s2c_packets::thong_bao("Người chơi không còn trực tuyến!")?).await; };
        if matches!(action, 0 | 1) && (super::router_shop::is_in_trade(my_pid) || super::router_shop::is_in_trade(target_pid)) {
            return send_thong_bao(session, "Không thể thách đấu khi đang giao dịch!");
        }
        if action == 0 && let Some((tloc, t_hp, t_gold, t_handle)) = self.shared.clients.authenticated.get(&target_sid).map(|t| (t.snapshot.location, t.snapshot.stats.hp, t.snapshot.wallet.gold, t.handle.clone())) {
            if t_hp <= 0 || tloc.map_id != loc.map_id || tloc.zone_id != loc.zone_id || { let pvp = lock_pvp(); pvp.contains_key(&my_pid) || pvp.contains_key(&target_pid) } { return Ok(()); }
            if my_gold < PVP_CHALLENGE_STAKE || t_gold < PVP_CHALLENGE_STAKE {
                return send_thong_bao(session, "Cả hai người chơi cần có đủ vàng để thách đấu!");
            }
            let now_ms = unix_now_ms();
            let pending = pending_pvp();
            if pending.len() >= 1_024 {
                pending.retain(|_, (exp, _)| *exp > now_ms);
                if pending.len() >= 2_048 {
                    return send_thong_bao(session, "Hệ thống đang bận, vui lòng thử lại sau!");
                }
            }
            pending.insert((my_pid, target_pid), (now_ms.saturating_add(30_000), PVP_CHALLENGE_STAKE));
            let mut pw = PacketWriter::with_capacity(24 + my_name.len());
            pw.write_i8(pvp_type); pw.write_i32(my_pid.0 as i32); pw.write_i32(PVP_CHALLENGE_STAKE as i32); pw.write_utf(&format!("{my_name} mời bạn thách đấu!"))?;
            let _ = t_handle.try_send(pw.finish(cmd_message::PLAYER_VS_PLAYER));
        } else if action == 1 {
            let now_ms = unix_now_ms();
            let target_info = self.shared.clients.authenticated.get(&target_sid).map(|t| (t.snapshot.stats.hp > 0 && t.snapshot.location.map_id == loc.map_id && t.snapshot.location.zone_id == loc.zone_id, t.snapshot.wallet.gold));
            let Some((_, (exp, stake))) = pending_pvp().remove(&(target_pid, my_pid)) else { return Ok(()); };
            let Some((target_ok, target_gold)) = target_info else { return Ok(()); };
            if !target_ok || exp < now_ms { return Ok(()); }
            if my_gold < stake || target_gold < stake {
                return send_thong_bao(session, "Cả hai người chơi cần có đủ vàng để thách đấu!");
            }
            {
                let mut pvp = lock_pvp();
                if pvp.contains_key(&my_pid) || pvp.contains_key(&target_pid) || super::router_shop::is_in_trade(my_pid) || super::router_shop::is_in_trade(target_pid) { return Ok(()); }
                pvp.insert(my_pid, (target_pid, stake));
                pvp.insert(target_pid, (my_pid, stake));
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
                let now_ms = unix_now_ms();
                let res = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                    if entry.snapshot.stats.hp <= 0 { return None; }
                    let (pid, cf) = (entry.snapshot.id, idx.clamp(0, flags.len() as i8));
                    if entry.c_flag != cf
                        && let Some(last) = flag_cooldown().get(&pid).map(|v| *v.value())
                        && now_ms.saturating_sub(last) < FLAG_CHANGE_COOLDOWN_MS
                    {
                        let rem = ((FLAG_CHANGE_COOLDOWN_MS - now_ms.saturating_sub(last)).saturating_add(999) / 1_000).max(1);
                        return Some(Err(rem));
                    }
                    entry.c_flag = cf;
                    flag_cooldown().insert(pid, now_ms);
                    let mut w = PacketWriter::with_capacity(6); w.write_i8(1); w.write_i32(pid.0 as i32); w.write_i8(cf);
                    Some(Ok((w.finish(cmd_message::CHANGE_FLAG), entry.snapshot.location)))
                });
                match res {
                    Some(Ok((pkt, loc))) => self.broadcast_zone_or_self(loc, sid, &pkt),
                    Some(Err(rem)) => send_thong_bao(session, &format!("Vui lòng chờ {rem} giây để đổi cờ!"))?,
                    None => {}
                }
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
                let peer = self.shared.clients.players.get(&tpid).map(|s| *s.value()).and_then(|tsid| {
                    let e = self.shared.clients.authenticated.get(&tsid)?;
                    let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                    Some(SocialPeer { id: tpid.0 as i32, name: e.snapshot.name.clone(), head: h, body: b, leg: l, bag: bg as i8, power: e.snapshot.stats.power })
                });
                let msg = match peer {
                    None => "Người chơi hiện không trực tuyến!",
                    Some(peer) => {
                        let added = self.shared.clients.authenticated.get_mut(&sid).map(|mut me| {
                            if me.snapshot.friends.iter().any(|f| f.id == peer.id) {
                                false
                            } else {
                                me.snapshot.friends.push(peer);
                                true
                            }
                        });
                        match added {
                            Some(true) => "Đã thêm vào danh sách bạn bè!",
                            Some(false) => "Người chơi đã có trong danh sách bạn bè!",
                            None => "Không thể thêm bạn bè lúc này!",
                        }
                    }
                };
                session.send(s2c_packets::thong_bao(msg)?).await?;
            }
            SocialAction::PrimaryTarget(tpid) => {
                let target_loc = self.shared.clients.players.get(&tpid).map(|s| *s.value()).and_then(|tsid| {
                    self.shared.clients.authenticated.get(&tsid).map(|e| e.snapshot.location)
                });
                let revenge_prep = i32::try_from(tpid.0).ok().and_then(|id| {
                    self.shared.clients.authenticated.get_mut(&sid).and_then(|mut me| {
                        if !me.snapshot.enemies.iter().any(|e| e.id == id) {
                            return None;
                        }
                        if me.snapshot.stats.hp <= 0 || super::router_shop::is_in_trade(me.snapshot.id) || super::router_shop::is_in_trade(tpid) {
                            return Some(Err("Không thể dịch chuyển lúc này!"));
                        }
                        let Some(tloc) = target_loc else {
                            return Some(Err("Kẻ thù hiện không trực tuyến!"));
                        };
                        let non_normal = self.shared.templates.maps.iter().find(|m| m.id == tloc.map_id).is_some_and(|m| m.map_type != nro_consts::const_map::MAP_NORMAL);
                        if non_normal || matches!(tloc.map_id.0, 21..=23 | 39..=41 | 45..=51 | 53..=62 | 85..=91 | 112..=120 | 127..=129) || tloc.map_id.0 >= 135 {
                            return Some(Err("Không thể dịch chuyển đến khu vực này!"));
                        }
                        let my_pid = me.snapshot.id;
                        if { let pvp = lock_pvp(); pvp.contains_key(&my_pid) || pvp.contains_key(&tpid) } {
                            return Some(Err("Không thể trả thù khi đang trong trận PvP!"));
                        }
                        let used_ruby = if me.snapshot.wallet.ruby >= 1 {
                            me.snapshot.wallet.ruby -= 1;
                            true
                        } else if me.snapshot.wallet.gem >= 1 {
                            me.snapshot.wallet.gem -= 1;
                            false
                        } else {
                            return Some(Err("Bạn không đủ ngọc để trả thù!"));
                        };
                        Some(Ok((my_pid, tloc, used_ruby, wallet_money_pkt(&me.snapshot.wallet))))
                    })
                });
                match revenge_prep {
                    Some(Err(err_msg)) => {
                        session.send(s2c_packets::thong_bao(err_msg)?).await?;
                    }
                    Some(Ok((my_pid, tloc, used_ruby, money_pkt))) => {
                        let switched = self.switch_player_zone_with_ship(session, sid, Some(tloc.map_id), tloc.zone_id, Some((tloc.x, tloc.y)), 0).await;
                        if matches!(switched, Ok(true)) {
                            session.send(money_pkt).await?;
                            let arrived = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.location, e.snapshot.stats.hp));
                            let target_state = self.shared.clients.players.get(&tpid).map(|s| *s.value()).and_then(|tsid| {
                                self.shared.clients.authenticated.get(&tsid).map(|e| (e.snapshot.location, e.snapshot.stats.hp))
                            });
                            if let (Some((mloc, mhp)), Some((tloc, thp))) = (arrived, target_state)
                                && mhp > 0 && thp > 0 && mloc.map_id == tloc.map_id && mloc.zone_id == tloc.zone_id
                            {
                                let inserted = {
                                    let mut pvp = lock_pvp();
                                    if !pvp.contains_key(&my_pid) && !pvp.contains_key(&tpid)
                                        && !super::router_shop::is_in_trade(my_pid) && !super::router_shop::is_in_trade(tpid)
                                    {
                                        pvp.insert(my_pid, (tpid, 0));
                                        pvp.insert(tpid, (my_pid, 0));
                                        true
                                    } else {
                                        false
                                    }
                                };
                                if inserted {
                                    self.broadcast_pk_type(mloc, sid, my_pid, 3);
                                    self.broadcast_pk_type(tloc, sid, tpid, 3);
                                }
                            }
                        } else {
                            if let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) {
                                if used_ruby { me.snapshot.wallet.ruby = me.snapshot.wallet.ruby.saturating_add(1); }
                                else { me.snapshot.wallet.gem = me.snapshot.wallet.gem.saturating_add(1); }
                            }
                            let _ = switched?;
                        }
                    }
                    None => {}
                }
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
        if return_home {
            self.switch_player_zone(session, sid, Some(MapId(21 + i16::from(gender))), ZoneId(0), Some((300, 336))).await?;
        }
        let (final_loc, final_ek) = self.shared.clients.authenticated.get(&sid).map_or((orig_loc, ek), |e| (e.snapshot.location, e.entity_key));
        if let (Some(entity_key), Ok(tx)) = (final_ek, self.shared.world_router.resolve_sender(final_loc.map_id, final_loc.zone_id)) {
            let _ = tx.send(ZoneCommand::UpdatePlayerStats(entity_key, stats)).await;
        }
        let (me_live_pkt, peer_live_pkt) = s2c_packets::char_revive_packets(pid, stats.hp, stats.hp_max, final_loc);
        session.send(me_live_pkt).await?;
        if !return_home {
            session.send(money_pkt).await?;
        }
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
}
