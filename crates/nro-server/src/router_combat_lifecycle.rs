//! Revival, death cleanup (including fusion split & pet respawn), and disconnect preservation.

use nro_consts::const_player;
use nro_core::{Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::super::router_combat_pvp::pending_pvp;
use super::super::{
    ServerSessionHandler, effective_appearance, pkt_fusion, recalculate_session_stats,
    recalculate_snapshot_stats, send_thong_bao, sprite_restore_packets, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_revive(
        &self,
        session: &SessionHandle,
        sid: u64,
        return_home: bool,
    ) -> Result<(), ProtocolError> {
        let prep = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if entry.snapshot.stats.hp > 0 {
                    return None;
                }
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return Some(Err("Không thể hồi sinh khi đang giao dịch!"));
                }
                let prev_mp = entry.snapshot.stats.mp;
                if !return_home {
                    if entry.snapshot.wallet.ruby >= 1 {
                        entry.snapshot.wallet.ruby -= 1;
                    } else if entry.snapshot.wallet.gem >= 1 {
                        entry.snapshot.wallet.gem -= 1;
                    } else {
                        return Some(Err("Bạn không đủ ngọc để hồi sinh tại chỗ!"));
                    }
                }
                entry.snapshot.stats.hp = entry.snapshot.stats.hp_max.max(1);
                entry.snapshot.stats.mp = entry.snapshot.stats.mp_max.max(1);
                let (pid, loc, stats, ek) = (
                    entry.snapshot.id,
                    entry.snapshot.location,
                    entry.snapshot.stats,
                    entry.entity_key,
                );
                Some(Ok((
                    pid,
                    loc,
                    stats,
                    prev_mp,
                    ek,
                    wallet_money_pkt(&entry.snapshot.wallet),
                    entry.snapshot.gender,
                )))
            });
        let (pid, orig_loc, stats, prev_mp, orig_ek, money_pkt, gender) = match prep {
            Some(Ok(v)) => v,
            Some(Err(msg)) => return send_thong_bao(session, msg),
            None => return Ok(()),
        };
        if let (Some(ek), Ok(src_tx)) = (
            orig_ek,
            self.shared
                .world_router
                .resolve_sender(orig_loc.map_id, orig_loc.zone_id),
        ) {
            let _ = src_tx.send(ZoneCommand::UpdatePlayerStats(ek, stats)).await;
        }
        if return_home {
            let home = MapId(21 + i16::from(i8::from(gender)));
            let switched = self
                .switch_player_zone_with_ship(
                    session,
                    sid,
                    Some(home),
                    ZoneId(0),
                    Some((300, 336)),
                    0,
                )
                .await;
            if !matches!(switched, Ok(true)) {
                let rollback_zone =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .map(|mut entry| {
                            entry.snapshot.stats.hp = 0;
                            entry.snapshot.stats.mp = prev_mp;
                            (
                                entry.entity_key,
                                entry.snapshot.location,
                                entry.snapshot.stats,
                            )
                        });
                if let Some((Some(rb_ek), rb_loc, rb_stats)) = rollback_zone
                    && let Ok(tx) = self
                        .shared
                        .world_router
                        .resolve_sender(rb_loc.map_id, rb_loc.zone_id)
                {
                    let _ = tx
                        .send(ZoneCommand::UpdatePlayerStats(rb_ek, rb_stats))
                        .await;
                }
                let _ = switched?;
                return Ok(());
            }
            let (final_loc, final_ek, final_stats) = self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .map_or((orig_loc, orig_ek, stats), |e| {
                    (e.snapshot.location, e.entity_key, e.snapshot.stats)
                });
            if let (Some(dest_ek), Ok(dest_tx)) = (
                final_ek,
                self.shared
                    .world_router
                    .resolve_sender(final_loc.map_id, final_loc.zone_id),
            ) {
                let _ = dest_tx
                    .send(ZoneCommand::UpdatePlayerStats(dest_ek, final_stats))
                    .await;
            }
            let (me_live, peer_live) = s2c_packets::char_revive_packets(
                pid,
                final_stats.hp,
                final_stats.hp_max,
                final_loc,
            );
            session.send(me_live).await?;
            session
                .send(s2c_packets::me_load_point(&final_stats))
                .await?;
            self.broadcast_to_zone_peers(final_loc, Some(sid), &peer_live);
            crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
        } else {
            let (me_live, peer_live) =
                s2c_packets::char_revive_packets(pid, stats.hp, stats.hp_max, orig_loc);
            session.send(me_live).await?;
            session.send(money_pkt).await?;
            session.send(s2c_packets::me_load_point(&stats)).await?;
            self.broadcast_to_zone_peers(orig_loc, Some(sid), &peer_live);
            crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
        }
        Ok(())
    }

    pub(crate) fn handle_player_death_cleanup(&self, pid: PlayerId, loc: Location, sid: u64) {
        self.handle_player_death_cleanup_with_killer(pid, loc, sid, None);
    }

    pub(in crate::router) fn handle_player_death_cleanup_with_killer(
        &self,
        pid: PlayerId,
        loc: Location,
        sid: u64,
        killer_pid: Option<PlayerId>,
    ) {
        if let Some(kpid) = killer_pid
            && kpid != pid
        {
            let killer_peer = self
                .shared
                .clients
                .players
                .get(&kpid)
                .map(|s| *s.value())
                .and_then(|ksid| {
                    let e = self.shared.clients.authenticated.get(&ksid)?;
                    let (h, b, l, bg, _) =
                        effective_appearance(&self.shared.templates, &e.snapshot);
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
        self.cleanup_trade_on_disconnect(pid);
        self.forfeit_active_pvp(pid, loc, sid);
        super::super::router_skill_effects::clear_player_effects(pid);
        let (cleared, reverted) = (
            super::super::router_skill::clear_skill_state(pid),
            super::super::router_skill::take_monkey_revert_pending(pid),
        );
        if let Some((h, st, body_pkt, rpkts, fpkt, unfused_pet)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                let unfused = entry.snapshot.pet.as_mut().is_some_and(|p| {
                    if p.fusion_type != 0 {
                        p.fusion_type = 0;
                        p.fusion_time_left_ms = 0;
                        p.status = 0;
                        true
                    } else {
                        false
                    }
                });
                if unfused {
                    super::super::router_secondary::last_fusion_enter_ms().remove(&pid);
                    super::super::router_secondary::record_pet_unfusion_cooldown(
                        pid,
                        unix_now_ms(),
                    );
                    crate::router_pet::recalculate_pet_in_snapshot(
                        &self.shared,
                        &mut entry.snapshot,
                    );
                }
                let recompute = cleared || unfused;
                if recompute {
                    recalculate_session_stats(
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
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                }
                let need_restore = reverted || unfused;
                let (body_pkt, rpkts) = if need_restore {
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
                    (
                        Some(s2c_packets::inventory_body(eh, &entry.snapshot.items_body)),
                        Some(sprite_restore_packets(pid, eh, eb, el, is_disg)),
                    )
                } else {
                    (None, None)
                };
                let fpkt = unfused.then(|| pkt_fusion(pid, 1));
                let unfused_pet = unfused.then(|| {
                    crate::router_pet::build_pet_companion(&self.shared.templates, &entry.snapshot)
                });
                (
                    entry.handle.clone(),
                    recompute.then_some(entry.snapshot.stats),
                    body_pkt,
                    rpkts,
                    fpkt,
                    unfused_pet,
                )
            })
        {
            if let Some(pkt) = fpkt {
                self.broadcast_zone_or_self(loc, sid, &pkt);
            }
            if let Some(pkts) = rpkts {
                for p in pkts.into_iter().flatten() {
                    self.broadcast_zone_or_self(loc, sid, &p);
                }
            }
            if let Some(bp) = body_pkt {
                let _ = h.try_send(bp);
            }
            if let Some(st) = st {
                let _ = h.try_send(s2c_packets::me_load_point(&st));
            }
            if let Some(pet) = unfused_pet
                && let Ok(tx) = self
                    .shared
                    .world_router
                    .resolve_sender(loc.map_id, loc.zone_id)
            {
                let _ = tx.try_send(ZoneCommand::SyncPlayerPet {
                    player_id: pid,
                    pet,
                });
            }
        }
    }

    pub(in crate::router) fn cleanup_combat_on_zone_change(
        &self,
        pid: PlayerId,
        cur_loc: Location,
        sid: u64,
    ) {
        self.forfeit_active_pvp(pid, cur_loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
        super::super::router_skill::clear_ttnl(pid);
    }

    pub(in crate::router) fn cleanup_combat_on_disconnect(
        &self,
        pid: PlayerId,
        loc: Location,
        sid: u64,
    ) {
        self.forfeit_active_pvp(pid, loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
        super::super::router_skill_effects::clear_player_effects(pid);
        let cleared = super::super::router_skill::clear_skill_state(pid);
        let _ = super::super::router_skill::take_monkey_revert_pending(pid);
        if sid != 0
            && let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid)
        {
            let timed_unfused = entry.snapshot.pet.as_mut().is_some_and(|p| {
                if p.fusion_type == const_player::LUONG_LONG_NHAT_THE {
                    p.fusion_type = const_player::NON_FUSION;
                    p.fusion_time_left_ms = 0;
                    p.status = 0;
                    true
                } else {
                    false
                }
            });
            if timed_unfused {
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut entry.snapshot);
            }
            if cleared || timed_unfused {
                recalculate_snapshot_stats(&mut entry.snapshot, self.shared.config.parity_mode);
            }
        }
    }
}
