//! Inventory movement (`-40`), session buffs, and capsule/Yardrat transport (`-91`, `18`).

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::const_map;
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, MapId, PlayerId, TemplateId, ZoneId};
use nro_economy::{BuffSlot, InventoryError, ItemCatalog, ItemMeta, TimedBuffState, opt_id};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{DbItemCatalog, ServerSessionHandler, capsule_dests, effective_appearance, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot};
use crate::runtime::unix_now_ms;

static SESSION_BUFFS: OnceLock<DashMap<u64, TimedBuffState>> = OnceLock::new();
pub(super) fn session_buffs() -> &'static DashMap<u64, TimedBuffState> { SESSION_BUFFS.get_or_init(DashMap::new) }
pub(super) fn clear_session_buffs(sid: u64) { session_buffs().remove(&sid); }

fn check_equip_requirements(
    item: &ItemInstance,
    meta: ItemMeta,
    wearer_gender: Gender,
    wearer_power: i64,
) -> Result<(), &'static str> {
    if let Some(req_gender) = meta.gender && req_gender != wearer_gender {
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
    pub(super) async fn handle_get_item_move(&self, session: &SessionHandle, sid: u64, move_type: i8, index: i8) -> Result<(), ProtocolError> {
        if self.shared.clients.authenticated.get(&sid).is_some_and(|e| super::router_shop::is_in_trade(e.snapshot.id)) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        let Ok(idx) = usize::try_from(index) else { return Ok(()); };
        let (catalog, mut bag_full, mut in_trade, mut equip_err) = (DbItemCatalog(&self.shared.templates), false, false, None::<&'static str>);
        let outcome = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) { in_trade = true; return None; }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            while inv.body.len() < 11 { inv.body.push(None); }
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let (mut body_changed, mut pet_changed) = (false, false);
            let moved = match move_type {
                0 => inv.box_items.get_mut(idx).and_then(Option::take).is_some_and(|it| {
                    let ok = inv.add_to_bag(it.clone(), catalog.meta_of(it.template_id).stackable).is_ok();
                    if !ok && let Some(s) = inv.box_items.get_mut(idx) { *s = Some(it); }
                    ok
                }),
                1 => inv.bag.get_mut(idx).and_then(Option::take).is_some_and(|it| {
                    if let Some(empty) = inv.box_items.iter_mut().find(|s| s.is_none()) { *empty = Some(it); true }
                    else { if let Some(s) = inv.bag.get_mut(idx) { *s = Some(it); } false }
                }),
                2 => inv.box_items.get_mut(idx).and_then(Option::take).is_some_and(|mut it| {
                    let meta = catalog.meta_of(it.template_id);
                    if let Err(msg) = check_equip_requirements(&it, meta, entry.snapshot.gender, entry.snapshot.stats.power) {
                        if let Some(s) = inv.box_items.get_mut(idx) { *s = Some(it); }
                        equip_err = Some(msg);
                        return false;
                    }
                    if let Some(bs) = resolve_equip_body_slot(meta.item_type, inv.body.len()) && let Some(worn) = inv.body.get_mut(bs) {
                        nro_economy::inventory::expand_equip_triggers(&mut it, &mut rng);
                        let prev = worn.replace(it);
                        if let Some(s) = inv.box_items.get_mut(idx) { *s = prev; }
                        body_changed = true; true
                    } else { if let Some(s) = inv.box_items.get_mut(idx) { *s = Some(it); } false }
                }),
                3 => { let ok = inv.body_to_box(idx).is_ok(); body_changed = ok; ok }
                4 => inv.bag_item(idx).ok().cloned().is_some_and(|it| {
                    let meta = catalog.meta_of(it.template_id);
                    if let Err(msg) = check_equip_requirements(&it, meta, entry.snapshot.gender, entry.snapshot.stats.power) {
                        equip_err = Some(msg);
                        return false;
                    }
                    let Some(bs) = resolve_equip_body_slot(meta.item_type, inv.body.len()) else { return false; };
                    match inv.bag_to_body(idx, bs, &mut rng) {
                        Ok(()) => { body_changed = true; true }
                        Err(InventoryError::ContainerFull(_)) => { bag_full = true; false }
                        Err(_) => false,
                    }
                }),
                5 => inv.body.get_mut(idx).and_then(Option::take).is_some_and(|it| {
                    let ok = inv.add_to_bag(it.clone(), false).is_ok();
                    if ok { body_changed = true; } else if let Some(s) = inv.body.get_mut(idx) { *s = Some(it); }
                    ok
                }),
                6 => inv.bag_item(idx).ok().cloned().is_some_and(|it_check| {
                    let meta = catalog.meta_of(it_check.template_id);
                    let Some(pet) = entry.snapshot.pet.as_mut() else { return false; };
                    if let Err(msg) = check_equip_requirements(&it_check, meta, pet.gender, pet.stats.power) {
                        equip_err = Some(msg);
                        return false;
                    }
                    while pet.items_body.len() < 7 { pet.items_body.push(None); }
                    let Some(ps) = resolve_equip_body_slot(meta.item_type, pet.items_body.len()) else { return false; };
                    let Ok(mut it) = inv.sub_from_bag_slot(idx, 1) else { return false; };
                    if let Some(prev) = pet.items_body[ps].take() && inv.add_to_bag(prev.clone(), false).is_err() {
                        pet.items_body[ps] = Some(prev);
                        if let Some(s) = inv.bag.get_mut(idx) && s.is_none() { *s = Some(it); } else { let _ = inv.add_to_bag(it, meta.stackable); }
                        bag_full = true; return false;
                    }
                    nro_economy::inventory::expand_equip_triggers(&mut it, &mut rng);
                    pet.items_body[ps] = Some(it); pet_changed = true; body_changed = true; true
                }),
                7 => entry.snapshot.pet.as_mut().and_then(|p| p.items_body.get_mut(idx)).and_then(Option::take).is_some_and(|it| {
                    if inv.add_to_bag(it.clone(), false).is_ok() { pet_changed = true; body_changed = true; true }
                    else { if let Some(p) = entry.snapshot.pet.as_mut() && let Some(s) = p.items_body.get_mut(idx) { *s = Some(it); } false }
                }),
                _ => false,
            };
            if !moved { return None; }
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            if body_changed { super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode); }
            let (eh, eb, el, ebag, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
            let (loc, pid) = (entry.snapshot.location, entry.snapshot.id);
            if body_changed && let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            Some((
                [s2c_packets::inventory_body(eh, &entry.snapshot.items_body), s2c_packets::inventory_bag(&entry.snapshot.items_bag), s2c_packets::inventory_box(&entry.snapshot.items_box)],
                body_changed.then(|| (s2c_packets::me_load_point(&entry.snapshot.stats), s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))), s2c_packets::send_flag_bag(pid, ebag), loc)),
                pet_changed.then(|| self.pkt_pet_detail(&entry.snapshot)).flatten(),
            ))
        });
        if in_trade { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        if let Some(msg) = equip_err { return send_thong_bao(session, msg); }
        if bag_full { return send_thong_bao(session, "Hành trang đã đầy!"); }
        if let Some((inv_pkts, body_bcast, pet_pkt)) = outcome {
            for p in inv_pkts { session.send(p).await?; }
            if let Some((pt_pkt, upd_body, flag_bag, loc)) = body_bcast {
                session.send(pt_pkt).await?; self.broadcast_to_zone_peers(loc, None, &upd_body); self.broadcast_to_zone_peers(loc, None, &flag_bag);
            }
            if let Some(pp) = pet_pkt { session.send(pp).await?; }
        }
        Ok(())
    }

    pub(super) async fn handle_map_transport(&self, session: &SessionHandle, sid: u64, opt_idx: Option<i8>) -> Result<(), ProtocolError> {
        let Some(raw_idx) = opt_idx.and_then(|i| usize::try_from(i).ok()) else { return Ok(()); };
        let prep = self.shared.clients.authenticated.get(&sid).and_then(|entry| {
            if entry.snapshot.stats.hp <= 0 { return None; }
            let pid = entry.snapshot.id;
            let (is_194, dests) = capsule_dests().get(&pid).map(|v| v.value().clone())?;
            let &dst_map = dests.get(raw_idx)?;
            let req_tid = if is_194 { TemplateId(194) } else { TemplateId(193) };
            if !entry.snapshot.items_bag.iter().any(|s| s.as_ref().is_some_and(|it| it.template_id == req_tid && it.quantity > 0)) { return None; }
            Some((pid, is_194, dst_map, if entry.snapshot.have_tennis_space_ship { 3 } else { 1 }))
        });
        if let Some((pid, is_194, dst_map, ship)) = prep {
            let moved = self.switch_player_zone_with_ship(session, sid, Some(dst_map), ZoneId(0), Some((250, 5)), ship).await?;
            if moved {
                capsule_dests().remove(&pid);
                if !is_194 && let Some(bp) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    inv.sub_template_from_bag(TemplateId(193), 1).ok()?;
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    Some(s2c_packets::inventory_bag(&entry.snapshot.items_bag))
                }) { session.send(bp).await?; }
            }
        }
        Ok(())
    }

    pub(super) async fn handle_transport_complete(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some(dst) = self.shared.clients.authenticated.get(&sid).and_then(|e| (e.snapshot.stats.hp > 0 && !super::router_shop::is_in_trade(e.snapshot.id) && matches!(e.snapshot.location.map_id, MapId(27) | MapId(102))).then_some(if e.snapshot.location.map_id == MapId(102) { MapId(27) } else { MapId(102) })) else { return Ok(()); };
        self.switch_player_zone(session, sid, Some(dst), ZoneId(0), Some((250, 336))).await
    }

    pub(super) async fn handle_goto_player(&self, session: &SessionHandle, sid: u64, target_pid: PlayerId) -> Result<(), ProtocolError> {
        let Some((can_act, has_yardrat)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            let can_act = e.snapshot.stats.hp > 0 && !super::router_shop::is_in_trade(e.snapshot.id);
            let has_yardrat = e.is_admin || e.snapshot.items_body.iter().flatten().any(|it| {
                (592..=594).contains(&it.template_id.0) || it.options.iter().any(|o| o.id == 33)
            });
            (can_act, has_yardrat)
        }) else { return Ok(()); };
        if !can_act { return send_thong_bao(session, "Không thể dịch chuyển lúc này!"); }
        if !has_yardrat { return send_thong_bao(session, "Cần mặc cải trang Yardrat để dịch chuyển tới người chơi!"); }
        let target_loc = self.shared.clients.players.get(&target_pid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| e.snapshot.location));
        if let Some(tloc) = target_loc {
            let non_normal = self.shared.templates.maps.iter().find(|m| m.id == tloc.map_id).is_some_and(|m| m.map_type != const_map::MAP_NORMAL);
            if non_normal || matches!(tloc.map_id.0, 21..=23 | 39..=41 | 45..=51 | 53..=62 | 85..=91 | 112..=120 | 127..=129) || tloc.map_id.0 >= 135 { return send_thong_bao(session, "Không thể dịch chuyển đến khu vực này!"); }
            self.switch_player_zone(session, sid, Some(tloc.map_id), tloc.zone_id, Some((tloc.x, tloc.y))).await
        } else { session.send(s2c_packets::thong_bao("Không tìm thấy người chơi!")?).await }
    }
}

pub(super) fn apply_session_buffs_before_clamp(sid: u64, stats: &mut CoreStats) {
    let Some(buffs) = session_buffs().get(&sid) else { return; };
    let now_ms = unix_now_ms();
    let has = |p: fn(BuffSlot) -> bool| buffs.has_active(now_ms, p);
    if has(|s| s == BuffSlot::BoHuyet(true)) { stats.hp_max = stats.hp_max.saturating_mul(22) / 10; } else if has(|s| s == BuffSlot::BoHuyet(false)) { stats.hp_max = stats.hp_max.saturating_mul(2); }
    if has(|s| s == BuffSlot::BoKhi(true)) { stats.mp_max = stats.mp_max.saturating_mul(22) / 10; } else if has(|s| s == BuffSlot::BoKhi(false)) { stats.mp_max = stats.mp_max.saturating_mul(2); }
    if has(|s| s == BuffSlot::CuongNo(true)) { stats.dame = stats.dame.saturating_mul(22) / 10; } else if has(|s| s == BuffSlot::CuongNo(false)) { stats.dame = stats.dame.saturating_mul(2); }
    if has(|s| matches!(s, BuffSlot::MealTier1(_))) { stats.dame = stats.dame.saturating_add(stats.dame / 10); }
    if has(|s| matches!(s, BuffSlot::MealTier2(8060 | 880))) { stats.dame = stats.dame.saturating_add(stats.dame / 20); }
    if has(|s| matches!(s, BuffSlot::MealTier2(8062 | 882))) { stats.hp_max = stats.hp_max.saturating_add(stats.hp_max / 20); }
    for t in 1..=3_u8 { if buffs.has_active(now_ms, |s| s == BuffSlot::NuocMia(t)) { stats.hp_max = stats.hp_max.saturating_add(stats.hp_max / 10); } }
    if has(|s| s == BuffSlot::NuocMia(2)) { stats.crit = stats.crit.saturating_add(10); }
    if has(|s| s == BuffSlot::NuocMia(3)) { stats.dame = stats.dame.saturating_add(stats.dame / 10); stats.def = stats.def.saturating_add(stats.def / 10); }
    let max_i32 = i64::from(i32::MAX);
    stats.hp_max = stats.hp_max.clamp(1, max_i32);
    stats.mp_max = stats.mp_max.clamp(1, max_i32);
    stats.dame = stats.dame.clamp(0, max_i32);
}

pub(super) const fn resolve_equip_body_slot(item_type: u8, body_len: usize) -> Option<usize> {
    let slot = match item_type { 0..=5 => item_type as usize, 25 => 4, 32 => 6, 72 => 7, 11 => 8, 23 | 24 => 9, 35 => 10, _ => return None };
    if slot < body_len { Some(slot) } else { None }
}
