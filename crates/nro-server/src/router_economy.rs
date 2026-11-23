//! Inventory movement (`-40`) and session buff registry.

#[path = "router_economy_transport.rs"]
mod router_economy_transport;

pub(super) use router_economy_transport::apply_session_buffs_before_clamp;

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_core::{GameRng, Gender, ItemInstance};
use nro_economy::{InventoryError, ItemCatalog, ItemMeta, TimedBuffState, opt_id};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, send_thong_bao,
    snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
};
use crate::runtime::unix_now_ms;

static SESSION_BUFFS: OnceLock<DashMap<u64, TimedBuffState>> = OnceLock::new();
pub(super) fn session_buffs() -> &'static DashMap<u64, TimedBuffState> {
    SESSION_BUFFS.get_or_init(DashMap::new)
}
pub(super) fn clear_session_buffs(sid: u64) {
    session_buffs().remove(&sid);
}

pub(super) fn prune_expired_session_buffs(sid: u64, now_ms: i64) -> bool {
    let (removed, empty) = if let Some(mut entry) = session_buffs().get_mut(&sid) {
        let before = entry.active.len();
        entry.active.retain(|&mut (_, exp)| exp > now_ms);
        (entry.active.len() < before, entry.active.is_empty())
    } else {
        return false;
    };
    if empty {
        session_buffs().remove_if(&sid, |_, v| v.active.is_empty());
    }
    removed
}

fn check_equip_requirements(
    item: &ItemInstance,
    meta: ItemMeta,
    wearer_gender: Gender,
    wearer_power: i64,
) -> Result<(), &'static str> {
    if let Some(req_gender) = meta.gender
        && req_gender != wearer_gender
    {
        return Err("Trang bị không phù hợp hành tinh!");
    }
    if wearer_power < meta.power_require {
        return Err("Không đủ sức mạnh để mặc trang bị này!");
    }
    if let Some(req_b) = nro_economy::inventory::get_opt(item, opt_id::POWER_REQ_B)
        && req_b > 0
        && wearer_power < i64::from(req_b).saturating_mul(1_000_000_000)
    {
        return Err("Không đủ sức mạnh để mặc trang bị này!");
    }
    Ok(())
}

impl ServerSessionHandler {
    pub(super) async fn handle_get_item_move(
        &self,
        session: &SessionHandle,
        sid: u64,
        move_type: i8,
        index: i8,
    ) -> Result<(), ProtocolError> {
        if self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| super::router_shop::is_in_trade(e.snapshot.id))
        {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        let Ok(idx) = usize::try_from(index) else {
            return Ok(());
        };
        let (catalog, mut bag_full, mut in_trade, mut equip_err) = (
            DbItemCatalog(&self.shared.templates),
            false,
            false,
            None::<&'static str>,
        );
        let outcome = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) {
                    in_trade = true;
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                while inv.body.len() < 11 {
                    inv.body.push(None);
                }
                let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
                let (mut body_changed, mut pet_changed) = (false, false);
                let moved = match move_type {
                    0 => inv
                        .box_items
                        .get_mut(idx)
                        .and_then(Option::take)
                        .is_some_and(|it| {
                            let ok = inv
                                .add_to_bag(it.clone(), catalog.meta_of(it.template_id).stackable)
                                .is_ok();
                            if !ok && let Some(s) = inv.box_items.get_mut(idx) {
                                *s = Some(it);
                            }
                            ok
                        }),
                    1 => inv
                        .bag
                        .get_mut(idx)
                        .and_then(Option::take)
                        .is_some_and(|it| {
                            if let Some(empty) = inv.box_items.iter_mut().find(|s| s.is_none()) {
                                *empty = Some(it);
                                true
                            } else {
                                if let Some(s) = inv.bag.get_mut(idx) {
                                    *s = Some(it);
                                }
                                false
                            }
                        }),
                    2 => inv
                        .box_items
                        .get_mut(idx)
                        .and_then(Option::take)
                        .is_some_and(|mut it| {
                            let meta = catalog.meta_of(it.template_id);
                            if let Err(msg) = check_equip_requirements(
                                &it,
                                meta,
                                entry.snapshot.gender,
                                entry.snapshot.stats.power,
                            ) {
                                if let Some(s) = inv.box_items.get_mut(idx) {
                                    *s = Some(it);
                                }
                                equip_err = Some(msg);
                                return false;
                            }
                            if let Some(bs) =
                                resolve_equip_body_slot(meta.item_type, inv.body.len())
                                && let Some(worn) = inv.body.get_mut(bs)
                            {
                                nro_economy::inventory::expand_equip_triggers(&mut it, &mut rng);
                                let prev = worn.replace(it);
                                if let Some(s) = inv.box_items.get_mut(idx) {
                                    *s = prev;
                                }
                                body_changed = true;
                                true
                            } else {
                                if let Some(s) = inv.box_items.get_mut(idx) {
                                    *s = Some(it);
                                }
                                false
                            }
                        }),
                    3 => {
                        let ok = inv.body_to_box(idx).is_ok();
                        body_changed = ok;
                        ok
                    }
                    4 => inv.bag_item(idx).ok().cloned().is_some_and(|it| {
                        let meta = catalog.meta_of(it.template_id);
                        if let Err(msg) = check_equip_requirements(
                            &it,
                            meta,
                            entry.snapshot.gender,
                            entry.snapshot.stats.power,
                        ) {
                            equip_err = Some(msg);
                            return false;
                        }
                        let Some(bs) = resolve_equip_body_slot(meta.item_type, inv.body.len())
                        else {
                            return false;
                        };
                        match inv.bag_to_body(idx, bs, &mut rng) {
                            Ok(()) => {
                                body_changed = true;
                                true
                            }
                            Err(InventoryError::ContainerFull(_)) => {
                                bag_full = true;
                                false
                            }
                            Err(_) => false,
                        }
                    }),
                    5 => inv
                        .body
                        .get_mut(idx)
                        .and_then(Option::take)
                        .is_some_and(|it| {
                            let ok = inv.add_to_bag(it.clone(), false).is_ok();
                            if ok {
                                body_changed = true;
                            } else if let Some(s) = inv.body.get_mut(idx) {
                                *s = Some(it);
                            }
                            ok
                        }),
                    6 => inv.bag_item(idx).ok().cloned().is_some_and(|it_check| {
                        let meta = catalog.meta_of(it_check.template_id);
                        let Some(pet) = entry.snapshot.pet.as_mut() else {
                            return false;
                        };
                        if let Err(msg) =
                            check_equip_requirements(&it_check, meta, pet.gender, pet.stats.power)
                        {
                            equip_err = Some(msg);
                            return false;
                        }
                        let min_slots = if matches!(pet.type_pet, 2..=4) { 9 } else { 7 };
                        while pet.items_body.len() < min_slots {
                            pet.items_body.push(None);
                        }
                        let Some(ps) =
                            resolve_equip_body_slot(meta.item_type, pet.items_body.len())
                        else {
                            return false;
                        };
                        let Ok(mut it) = inv.sub_from_bag_slot(idx, 1) else {
                            return false;
                        };
                        if let Some(prev) = pet.items_body[ps].take()
                            && inv.add_to_bag(prev.clone(), false).is_err()
                        {
                            pet.items_body[ps] = Some(prev);
                            if let Some(s) = inv.bag.get_mut(idx)
                                && s.is_none()
                            {
                                *s = Some(it);
                            } else {
                                let _ = inv.add_to_bag(it, meta.stackable);
                            }
                            bag_full = true;
                            return false;
                        }
                        nro_economy::inventory::expand_equip_triggers(&mut it, &mut rng);
                        pet.items_body[ps] = Some(it);
                        pet_changed = true;
                        body_changed = true;
                        true
                    }),
                    7 => entry
                        .snapshot
                        .pet
                        .as_mut()
                        .and_then(|p| p.items_body.get_mut(idx))
                        .and_then(Option::take)
                        .is_some_and(|it| {
                            if inv.add_to_bag(it.clone(), false).is_ok() {
                                pet_changed = true;
                                body_changed = true;
                                true
                            } else {
                                if let Some(p) = entry.snapshot.pet.as_mut()
                                    && let Some(s) = p.items_body.get_mut(idx)
                                {
                                    *s = Some(it);
                                }
                                false
                            }
                        }),
                    _ => false,
                };
                if !moved {
                    return None;
                }
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                if pet_changed {
                    crate::router_pet::recalculate_pet_in_snapshot(
                        &self.shared,
                        &mut entry.snapshot,
                    );
                }
                if body_changed {
                    super::recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                }
                let (eh, eb, el, ebag, is_disg) =
                    effective_appearance(&self.shared.templates, &entry.snapshot);
                let (loc, pid) = (entry.snapshot.location, entry.snapshot.id);
                if body_changed
                    && let (Some(ek), Ok(tx)) = (
                        entry.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    )
                {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                Some((
                    pid,
                    pet_changed,
                    [
                        s2c_packets::inventory_body(eh, &entry.snapshot.items_body),
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        s2c_packets::inventory_box(&entry.snapshot.items_box),
                    ],
                    body_changed.then(|| {
                        (
                            s2c_packets::me_load_point(&entry.snapshot.stats),
                            s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))),
                            s2c_packets::send_flag_bag(pid, ebag),
                            loc,
                        )
                    }),
                    pet_changed
                        .then(|| self.pkt_pet_detail(&entry.snapshot))
                        .flatten(),
                ))
            });
        if in_trade {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        if let Some(msg) = equip_err {
            return send_thong_bao(session, msg);
        }
        if bag_full {
            return send_thong_bao(session, "Hành trang đã đầy!");
        }
        if let Some((pid, pet_changed, inv_pkts, body_bcast, pet_pkt)) = outcome {
            for p in inv_pkts {
                session.send(p).await?;
            }
            if let Some((pt_pkt, upd_body, flag_bag, loc)) = body_bcast {
                session.send(pt_pkt).await?;
                self.broadcast_zone_or_self(loc, sid, &upd_body);
                self.broadcast_zone_or_self(loc, sid, &flag_bag);
            }
            if let Some(pp) = pet_pkt {
                session.send(pp).await?;
            }
            if pet_changed {
                crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
            }
        }
        Ok(())
    }
}

pub(super) const fn resolve_equip_body_slot(item_type: u8, body_len: usize) -> Option<usize> {
    let slot = match item_type {
        0..=5 => item_type as usize,
        25 => 4,
        32 => 6,
        72 => 7,
        11 => 8,
        23 | 24 => 9,
        35 => 10,
        _ => return None,
    };
    if slot < body_len { Some(slot) } else { None }
}
