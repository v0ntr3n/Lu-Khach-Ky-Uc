//! Revival, death cleanup (including fusion split & pet respawn), ground item pickup, and zone broadcasts.

use nro_core::{ItemInstance, Location, MapId, PlayerId, ZoneId};
use nro_db::json_columns::SocialPeer;
use nro_economy::{InventoryError, ItemCatalog};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::super::router_combat_pvp::pending_pvp;
use super::super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, recalculate_session_stats,
    send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::is_offline_map;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_revive(
        &self,
        session: &SessionHandle,
        sid: u64,
        return_home: bool,
    ) -> Result<(), ProtocolError> {
        let revived = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if entry.snapshot.stats.hp > 0 {
                    return None;
                }
                if !return_home {
                    if entry.snapshot.wallet.gem < 1 {
                        return Some(Err("Không đủ 1 ngọc xanh để hồi sinh tại chỗ!"));
                    }
                    entry.snapshot.wallet.gem -= 1;
                    entry.snapshot.stats.hp = entry.snapshot.stats.hp_max.max(1);
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp_max.max(1);
                } else {
                    entry.snapshot.stats.hp = 1;
                    entry.snapshot.stats.mp = 1;
                }
                let (pid, loc, stats) = (
                    entry.snapshot.id,
                    entry.snapshot.location,
                    entry.snapshot.stats,
                );
                if !return_home
                    && let (Some(ek), Ok(tx)) = (
                        entry.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    )
                {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
                }
                Some(Ok((
                    pid,
                    loc,
                    stats,
                    wallet_money_pkt(&entry.snapshot.wallet),
                    entry.snapshot.gender,
                )))
            });
        match revived {
            Some(Ok((pid, loc, stats, money_pkt, gender))) => {
                if return_home {
                    let home = MapId(21 + i16::from(u8::from(gender)));
                    self.switch_player_zone(session, sid, Some(home), ZoneId(0), Some((300, 336)))
                        .await?;
                    let (me_live, _) = s2c_packets::char_revive_packets(
                        pid,
                        stats.hp,
                        stats.hp_max,
                        Location {
                            map_id: home,
                            zone_id: ZoneId(0),
                            x: 300,
                            y: 336,
                        },
                    );
                    session.send(me_live).await?;
                    session.send(s2c_packets::me_load_point(&stats)).await?;
                } else {
                    let (me_live, peer_live) =
                        s2c_packets::char_revive_packets(pid, stats.hp, stats.hp_max, loc);
                    session.send(me_live).await?;
                    session.send(money_pkt).await?;
                    session.send(s2c_packets::me_load_point(&stats)).await?;
                    self.broadcast_to_zone_peers(loc, Some(sid), &peer_live);
                }
            }
            Some(Err(msg)) => send_thong_bao(session, msg)?,
            None => {}
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
        if let Some((h, st, rpkt, fpkt, unfused_pet)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                let unfused = entry.snapshot.pet.as_mut().is_some_and(|p| {
                    if p.fusion_type != 0 {
                        p.fusion_type = 0;
                        p.status = 0;
                        true
                    } else {
                        false
                    }
                });
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
                let fpkt = unfused.then(|| {
                    let mut w = PacketWriter::with_capacity(5);
                    w.write_i8(1);
                    w.write_i32(pid.0 as i32);
                    w.finish(nro_consts::cmd_message::FUSION)
                });
                let unfused_pet = unfused.then(|| {
                    crate::router_pet::build_pet_companion(&self.shared.templates, &entry.snapshot)
                });
                (
                    entry.handle.clone(),
                    recompute.then_some(entry.snapshot.stats),
                    (reverted || unfused).then(|| self.revert_monkey_appearance_entry(&entry, loc)),
                    fpkt,
                    unfused_pet,
                )
            })
        {
            if let Some(pkt) = fpkt {
                self.broadcast_zone_or_self(loc, sid, &pkt);
            }
            if let Some(st) = st {
                let _ = h.try_send(s2c_packets::me_load_point(&st));
            }
            if let Some(pkt) = rpkt {
                self.broadcast_zone_or_self(loc, sid, &pkt);
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
        self.handle_player_death_cleanup(pid, loc, sid);
        pending_pvp().retain(|k, _| k.0 != pid && k.1 != pid);
    }

    pub(crate) async fn teleport_player(
        &self,
        session: &SessionHandle,
        sid: u64,
        map_id: MapId,
        zone_id: ZoneId,
        coords: (i16, i16),
    ) -> Result<(), ProtocolError> {
        self.switch_player_zone(session, sid, Some(map_id), zone_id, Some(coords))
            .await
    }

    pub(in crate::router) async fn handle_pick_map_item(
        &self,
        session: &SessionHandle,
        sid: u64,
        item_map_id: i16,
    ) -> Result<(), ProtocolError> {
        let Some((ek, loc, pid)) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            e.entity_key
                .map(|k| (k, e.snapshot.location, e.snapshot.id))
        }) else {
            return Ok(());
        };
        let Ok(zone_tx) = self
            .shared
            .world_router
            .resolve_sender(loc.map_id, loc.zone_id)
        else {
            return Ok(());
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if zone_tx
            .send(ZoneCommand::PickItem(ek, item_map_id, reply_tx))
            .await
            .is_ok()
            && let Ok(Ok(picked)) = reply_rx.await
        {
            let (template_id, quantity) = picked.stack;
            let is_currency = matches!(template_id.0, 76 | 77 | 188..=190 | 861);
            let catalog = DbItemCatalog(&self.shared.templates);
            let mut in_trade = false;
            let bag_res = self
                .shared
                .clients
                .authenticated
                .get_mut(&sid)
                .map(|mut entry| {
                    if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                        in_trade = true;
                        return Ok(None);
                    }
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    match template_id.0 {
                        76 | 188..=190 => inv.add_gold_clamped(u64::from(quantity)),
                        77 => inv.gem = inv.gem.saturating_add(quantity),
                        861 => inv.ruby = inv.ruby.saturating_add(quantity),
                        _ => inv.add_to_bag(
                            ItemInstance {
                                template_id,
                                quantity,
                                options: picked.options.iter().copied().collect(),
                                create_time_ms: unix_now_ms(),
                            },
                            catalog.meta_of(template_id).stackable,
                        )?,
                    }
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    Ok::<_, InventoryError>(Some((
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        wallet_money_pkt(&entry.snapshot.wallet),
                    )))
                });
            if in_trade {
                let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await;
                return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
            }
            match bag_res {
                Some(Err(InventoryError::ContainerFull(_))) => {
                    let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await;
                    send_thong_bao(session, "Hành trang đã đầy!")?;
                }
                Some(Err(_) | Ok(None)) | None => {
                    let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await;
                }
                Some(Ok(Some((bag_pkt, money_pkt)))) => {
                    let (my_p, peer_p) = s2c_packets::item_map_pickup_packets(
                        item_map_id,
                        pid,
                        quantity,
                        "",
                        is_currency,
                    )?;
                    session.send(my_p).await?;
                    self.broadcast_to_zone_peers(loc, Some(sid), &peer_p);
                    session.send(bag_pkt).await?;
                    session.send(money_pkt).await?;
                    self.advance_task_on_pick_item(session, sid, template_id)
                        .await?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::router) fn broadcast_to_zone_peers(
        &self,
        loc: Location,
        exclude_sid: Option<u64>,
        pkt: &Packet,
    ) {
        if is_offline_map(loc.map_id) {
            return;
        }
        for rec in &self.shared.clients.authenticated {
            if exclude_sid != Some(*rec.key())
                && rec.value().snapshot.location.map_id == loc.map_id
                && rec.value().snapshot.location.zone_id == loc.zone_id
            {
                let _ = rec.value().handle.try_send(pkt.clone());
            }
        }
    }

    pub(in crate::router) fn broadcast_zone_or_self(
        &self,
        loc: Location,
        actor_sid: u64,
        pkt: &Packet,
    ) {
        if is_offline_map(loc.map_id) {
            if let Some(rec) = self.shared.clients.authenticated.get(&actor_sid) {
                let _ = rec.value().handle.try_send(pkt.clone());
            }
        } else {
            self.broadcast_to_zone_peers(loc, None, pkt);
        }
    }
}
