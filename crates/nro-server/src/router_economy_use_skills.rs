//! Inventory drop (`handle_drop_inventory_item`), equip shortcut check, and skill point upgrade (`handle_skill_up`).

use nro_core::ItemInstance;
use nro_db::json_columns::SkillSlot;
use nro_economy::{ItemCatalog, TradeSession};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};

use super::super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, pkt_fusion,
    recalculate_session_stats, router_economy::resolve_equip_body_slot, send_thong_bao,
    snapshot_to_economy_inv, sprite_restore_packets, sync_economy_inv_to_snapshot,
    unfuse_if_missing_porata,
};
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(super) fn is_bag_equip_shortcut(&self, sid: u64, index: i8) -> bool {
        let Ok(idx) = usize::try_from(index) else {
            return false;
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        self.shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|entry| {
                let Some(Some(it)) = entry.snapshot.items_bag.get(idx) else {
                    return false;
                };
                let meta = catalog.meta_of(it.template_id);
                let is_pet2 = nro_economy::use_item::PET2_SPRITES
                    .iter()
                    .any(|&(pid, _)| pid == it.template_id.0);
                let is_special = matches!(
                    it.template_id.0,
                    193 | 194 | 401..=404 | 454 | 568 | 570 | 759 | 921 | 1655 | 1795 | 1819
                );
                matches!(meta.item_type, 0..=5 | 32 | 35 | 72)
                    && resolve_equip_body_slot(
                        meta.item_type,
                        entry.snapshot.items_body.len().max(11),
                    )
                    .is_some()
                    && !is_pet2
                    && !is_special
            })
    }

    pub(super) async fn handle_drop_inventory_item(
        &self,
        session: &SessionHandle,
        sid: u64,
        where_idx: i8,
        index: i8,
    ) -> Result<(), ProtocolError> {
        let Ok(idx) = usize::try_from(index) else {
            return Ok(());
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        let mut block_err: Option<&'static str> = None;
        let drop_out = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    block_err = Some("Không thể thao tác khi đang giao dịch!");
                    return None;
                }
                if entry.snapshot.mbv_pin.1 {
                    block_err = Some("Hành trang đang khóa bảo vệ, không thể vứt vật phẩm!");
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let peek_item = match where_idx {
                    0 => inv.body.get(idx)?.as_ref()?,
                    1 => inv.bag.get(idx)?.as_ref()?,
                    2 => inv.box_items.get(idx)?.as_ref()?,
                    _ => return None,
                };
                if TradeSession::is_item_untradeable(
                    peek_item,
                    catalog.meta_of(peek_item.template_id),
                ) {
                    block_err = Some("Vật phẩm này không thể vứt bỏ!");
                    return None;
                }
                let taken: ItemInstance = match where_idx {
                    0 => inv.body.get_mut(idx)?.take()?,
                    1 => inv.bag.get_mut(idx)?.take()?,
                    2 => inv.box_items.get_mut(idx)?.take()?,
                    _ => return None,
                };
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                let unfused = (where_idx == 1)
                    && unfuse_if_missing_porata(
                        &self.shared,
                        sid,
                        &mut entry.snapshot,
                        unix_now_ms(),
                    );
                if where_idx == 0 && !unfused {
                    recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                }
                let body_changed = where_idx == 0 || unfused;
                let (eh, eb, el, ebag, is_disg) =
                    effective_appearance(&self.shared.templates, &entry.snapshot);
                let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
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
                let bcast = body_changed.then(|| {
                    (
                        s2c_packets::me_load_point(&entry.snapshot.stats),
                        unfused.then(|| pkt_fusion(pid, 1)),
                        sprite_restore_packets(pid, eh, eb, el, is_disg),
                        s2c_packets::send_flag_bag(pid, ebag),
                    )
                });
                let pet_pkt = unfused
                    .then(|| self.pkt_pet_detail(&entry.snapshot))
                    .flatten();
                Some((
                    taken,
                    pid,
                    loc,
                    unfused,
                    [
                        s2c_packets::inventory_body(eh, &entry.snapshot.items_body),
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        s2c_packets::inventory_box(&entry.snapshot.items_box),
                    ],
                    bcast,
                    pet_pkt,
                ))
            });
        if let Some(msg) = block_err {
            return send_thong_bao(session, msg);
        }
        if let Some((dropped, pid, loc, unfused, inv_pkts, bcast, pet_pkt)) = drop_out {
            if let Ok(tx) = self
                .shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id)
            {
                let _ = tx
                    .send(ZoneCommand::DropItemWithOptions(
                        dropped.template_id,
                        dropped.quantity,
                        loc,
                        Some(pid),
                        GroundDropKind::Standard,
                        dropped.options,
                    ))
                    .await;
            }
            for p in inv_pkts {
                session.send(p).await?;
            }
            if let Some((pt_pkt, f_pkt, upd_bodies, flag_bag)) = bcast {
                session.send(pt_pkt).await?;
                if let Some(fp) = f_pkt {
                    self.broadcast_zone_or_self(loc, sid, &fp);
                }
                for p in upd_bodies.into_iter().flatten() {
                    self.broadcast_zone_or_self(loc, sid, &p);
                }
                self.broadcast_zone_or_self(loc, sid, &flag_bag);
            }
            if let Some(pp) = pet_pkt {
                session.send(pp).await?;
            }
            if unfused {
                crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
            }
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_skill_up(
        &self,
        session: &SessionHandle,
        sid: u64,
        template_id: i16,
        _pt: i8,
    ) -> Result<(), ProtocolError> {
        let pkts = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                let mut out = Vec::new();
                if let Some(st) = self.shared.templates.skills.iter().find(|s| {
                    i16::from(s.id) == template_id && s.nclass_id == i8::from(entry.snapshot.gender)
                }) {
                    let cur_pt = entry
                        .snapshot
                        .skills
                        .iter()
                        .find(|sk| sk.template_id == i32::from(template_id))
                        .map_or(0, |sk| sk.point);
                    let next_pt = (cur_pt + 1).min(st.max_point.max(1));
                    if cur_pt <= 0 {
                        if let Ok(tb) =
                            s2c_packets::thong_bao("Cần học sách kỹ năng trước khi nâng cấp!")
                        {
                            out.push(tb);
                        }
                    } else if next_pt <= cur_pt {
                        if let Ok(tb) = s2c_packets::thong_bao("Kỹ năng đã đạt cấp tối đa!")
                        {
                            out.push(tb);
                        }
                    } else if let Some(lv) = st.levels.iter().find(|l| l.point == next_pt) {
                        let cost = i64::from(lv.price.max(0)).saturating_mul(1_000);
                        if entry.snapshot.stats.power < lv.power_require {
                            if let Ok(tb) = s2c_packets::thong_bao(&format!(
                                "Không đủ sức mạnh để nâng kỹ năng (cần {})!",
                                lv.power_require
                            )) {
                                out.push(tb);
                            }
                        } else if entry.snapshot.stats.tiem_nang < cost {
                            if let Ok(tb) = s2c_packets::thong_bao(&format!(
                                "Không đủ tiềm năng để nâng kỹ năng (cần {cost})!"
                            )) {
                                out.push(tb);
                            }
                        } else {
                            entry.snapshot.stats.tiem_nang -= cost;
                            if let Some(sk) = entry
                                .snapshot
                                .skills
                                .iter_mut()
                                .find(|s| s.template_id == i32::from(template_id))
                            {
                                sk.point = next_pt;
                                out.push(s2c_packets::me_update_skill(lv.id));
                            } else {
                                entry.snapshot.skills.push(SkillSlot {
                                    template_id: i32::from(template_id),
                                    point: next_pt,
                                    last_time_use_ms: 0,
                                    curr_level: 0,
                                });
                                out.push(s2c_packets::me_add_skill(lv.id));
                            }
                        }
                    }
                }
                out.push(s2c_packets::me_load_point(&entry.snapshot.stats));
                out
            });
        if let Some(list) = pkts {
            for p in list {
                session.send(p).await?;
            }
        }
        Ok(())
    }
}
