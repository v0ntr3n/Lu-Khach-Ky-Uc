//! Ground item pickup (`handle_pick_map_item`), `Bùa Thu Hút` magnet sweep, and zone peer broadcasts.

use nro_core::{EntityKey, ItemInstance, Location, MapId, PlayerId, TemplateId, ZoneId};
use nro_economy::{InventoryError, ItemCatalog};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{GroundItem, ZoneCommand, s2c_packets};
use smallvec::SmallVec;
use tokio::sync::{mpsc, oneshot};

use super::super::{
    DbItemCatalog, ServerSessionHandler, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::is_offline_map;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
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
        let now_ms = unix_now_ms();
        let Some((ek, loc, pid, has_magnet)) =
            self.shared.clients.authenticated.get(&sid).and_then(|e| {
                e.entity_key.map(|k| {
                    (
                        k,
                        e.snapshot.location,
                        e.snapshot.id,
                        e.snapshot.charms.td_thu_hut_ms > now_ms,
                    )
                })
            })
        else {
            return Ok(());
        };
        let Ok(zone_tx) = self
            .shared
            .world_router
            .resolve_sender(loc.map_id, loc.zone_id)
        else {
            return Ok(());
        };
        let maybe_picked = if has_magnet {
            let (reply_tx, reply_rx) = oneshot::channel();
            if zone_tx
                .send(ZoneCommand::MagnetPickItems {
                    picker: ek,
                    specific_wire_id: Some(item_map_id),
                    has_empty_bag_slot: true,
                    stackable_bag_tids: SmallVec::new(),
                    reply: reply_tx,
                })
                .await
                .is_ok()
                && let Ok(mut list) = reply_rx.await
            {
                list.pop()
            } else {
                None
            }
        } else {
            let (reply_tx, reply_rx) = oneshot::channel();
            if zone_tx
                .send(ZoneCommand::PickItem(ek, item_map_id, reply_tx))
                .await
                .is_ok()
                && let Ok(Ok(item)) = reply_rx.await
            {
                Some(item)
            } else {
                None
            }
        };
        if let Some(picked) = maybe_picked {
            self.deliver_picked_ground_item(session, sid, pid, loc, &zone_tx, picked, true)
                .await?;
        }
        Ok(())
    }

    /// Periodically pulls ground drops for all online players with active `Bùa Thu Hút` (`td_thu_hut_ms`).
    pub(crate) async fn sweep_magnet_pickups(&self) {
        let now_ms = unix_now_ms();
        let catalog = DbItemCatalog(&self.shared.templates);
        let mut candidates: Vec<(
            u64,
            SessionHandle,
            EntityKey,
            PlayerId,
            Location,
            bool,
            SmallVec<[TemplateId; 16]>,
        )> = Vec::new();

        for rec in &self.shared.clients.authenticated {
            let e = rec.value();
            if e.snapshot.stats.hp <= 0
                || e.snapshot.charms.td_thu_hut_ms <= now_ms
                || !e.handle.sent_key()
                || super::super::router_shop::is_in_trade(e.snapshot.id)
            {
                continue;
            }
            let Some(ek) = e.entity_key else {
                continue;
            };
            let has_empty_bag_slot = e.snapshot.items_bag.iter().any(Option::is_none);
            let mut stackable_bag_tids: SmallVec<[TemplateId; 16]> = SmallVec::new();
            if !has_empty_bag_slot {
                for slot in e.snapshot.items_bag.iter().flatten() {
                    if slot.quantity < 99
                        && catalog.meta_of(slot.template_id).stackable
                        && !stackable_bag_tids.contains(&slot.template_id)
                        && stackable_bag_tids.len() < 16
                    {
                        stackable_bag_tids.push(slot.template_id);
                    }
                }
            }
            candidates.push((
                *rec.key(),
                e.handle.clone(),
                ek,
                e.snapshot.id,
                e.snapshot.location,
                has_empty_bag_slot,
                stackable_bag_tids,
            ));
        }

        for (sid, handle, ek, pid, loc, has_empty_bag_slot, stackable_bag_tids) in candidates {
            let Ok(zone_tx) = self
                .shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id)
            else {
                continue;
            };
            let (reply_tx, reply_rx) = oneshot::channel();
            if zone_tx
                .send(ZoneCommand::MagnetPickItems {
                    picker: ek,
                    specific_wire_id: None,
                    has_empty_bag_slot,
                    stackable_bag_tids,
                    reply: reply_tx,
                })
                .await
                .is_ok()
                && let Ok(picked_items) = reply_rx.await
            {
                for picked in picked_items {
                    let _ = self
                        .deliver_picked_ground_item(&handle, sid, pid, loc, &zone_tx, picked, false)
                        .await;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn deliver_picked_ground_item(
        &self,
        session: &SessionHandle,
        sid: u64,
        pid: PlayerId,
        loc: Location,
        zone_tx: &mpsc::Sender<ZoneCommand>,
        picked: GroundItem,
        notify_errors: bool,
    ) -> Result<(), ProtocolError> {
        let item_map_id = picked.wire_id;
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
            if notify_errors {
                return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
            }
            return Ok(());
        }
        match bag_res {
            Some(Err(InventoryError::ContainerFull(_))) => {
                let _ = zone_tx.send(ZoneCommand::RestoreGroundItem(picked)).await;
                if notify_errors {
                    send_thong_bao(session, "Hành trang đã đầy!")?;
                }
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
