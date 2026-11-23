//! Inventory drop, equip shortcut check, skill learning, skill books, and capsule map labels.

use nro_core::{ItemInstance, MapId, TemplateId};
use nro_db::{json_columns::SkillSlot, player_repo::PlayerSnapshot};
use nro_economy::ItemCatalog;
use nro_player::skills::SkillKind;
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};

use super::super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, recalculate_session_stats,
    router_economy::resolve_equip_body_slot, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot,
};

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
        let mut in_trade = false;
        let drop_out = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    in_trade = true;
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let taken: ItemInstance = match where_idx {
                    0 => inv.body.get_mut(idx)?.take()?,
                    1 => inv.bag.get_mut(idx)?.take()?,
                    2 => inv.box_items.get_mut(idx)?.take()?,
                    _ => return None,
                };
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                if where_idx == 0 {
                    recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                }
                let (eh, eb, el, ebag, is_disg) =
                    effective_appearance(&self.shared.templates, &entry.snapshot);
                let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                if where_idx == 0
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
                let bcast = (where_idx == 0).then(|| {
                    (
                        s2c_packets::me_load_point(&entry.snapshot.stats),
                        s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))),
                        s2c_packets::send_flag_bag(pid, ebag),
                    )
                });
                Some((
                    taken,
                    pid,
                    loc,
                    [
                        s2c_packets::inventory_body(eh, &entry.snapshot.items_body),
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        s2c_packets::inventory_box(&entry.snapshot.items_box),
                    ],
                    bcast,
                ))
            });
        if in_trade {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        if let Some((dropped, pid, loc, inv_pkts, bcast)) = drop_out {
            if let Ok(tx) = self
                .shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id)
            {
                let _ = tx
                    .send(ZoneCommand::DropItem(
                        dropped.template_id,
                        dropped.quantity,
                        loc,
                        Some(pid),
                        GroundDropKind::Standard,
                    ))
                    .await;
            }
            for p in inv_pkts {
                session.send(p).await?;
            }
            if let Some((pt_pkt, upd_body, flag_bag)) = bcast {
                session.send(pt_pkt).await?;
                self.broadcast_zone_or_self(loc, sid, &upd_body);
                self.broadcast_zone_or_self(loc, sid, &flag_bag);
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

    pub(in crate::router) fn learn_shop_skill(
        &self,
        snap: &mut PlayerSnapshot,
        item_id: TemplateId,
    ) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let tpl = self
            .shared
            .templates
            .items
            .iter()
            .find(|r| r.id == i32::from(item_id.0));
        let st = tpl
            .and_then(|ir| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name))
            })
            .or_else(|| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.nclass_id == g_cls && s.levels.iter().any(|l| l.id == item_id.0))
            })?;
        let cur_pt = snap
            .skills
            .iter()
            .find(|sk| sk.template_id == i32::from(st.id))
            .map_or(0, |sk| sk.point);
        let next_pt = (cur_pt + 1).min(st.max_point.max(1));
        if next_pt <= cur_pt {
            return None;
        }
        let lv = st
            .levels
            .iter()
            .find(|l| l.point == next_pt)
            .or_else(|| st.levels.first())?;
        let tn_cost = i64::from(lv.price.max(0)).saturating_mul(1_000);
        if snap.stats.power < lv.power_require || snap.stats.tiem_nang < tn_cost {
            return None;
        }
        snap.stats.tiem_nang -= tn_cost;
        if let Some(sk) = snap
            .skills
            .iter_mut()
            .find(|s| s.template_id == i32::from(st.id))
        {
            let was_zero = sk.point <= 0;
            sk.point = next_pt;
            Some(if was_zero {
                s2c_packets::me_add_skill(lv.id)
            } else {
                s2c_packets::me_update_skill(lv.id)
            })
        } else {
            snap.skills.push(SkillSlot {
                template_id: i32::from(st.id),
                point: next_pt,
                last_time_use_ms: 0,
                curr_level: 0,
            });
            Some(s2c_packets::me_add_skill(lv.id))
        }
    }

    pub(in crate::router) fn capsule_label_for(&self, map_id: MapId) -> (&str, &str) {
        self.shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == map_id)
            .map_or(("Trạm tàu vũ trụ", "Trái Đất"), |m| {
                (
                    m.name.as_str(),
                    match m.planet_id {
                        1 => "Namếc",
                        2 => "Xayda",
                        _ => "Trái Đất",
                    },
                )
            })
    }

    pub(in crate::router) fn apply_skill_book(
        &self,
        snap: &mut PlayerSnapshot,
        book_tid: TemplateId,
    ) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let item_rec = self
            .shared
            .templates
            .items
            .iter()
            .find(|r| r.id == i32::from(book_tid.0));
        let st = u16::try_from(book_tid.0)
            .ok()
            .and_then(SkillKind::from_skill_book_item)
            .and_then(|kind| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.id == kind as i8)
            })
            .or_else(|| {
                item_rec.and_then(|ir| {
                    self.shared
                        .templates
                        .skills
                        .iter()
                        .find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name))
                })
            })?;
        let stid = st.id;
        let new_pt = item_rec.map_or(1_i8, |ir| {
            ir.level.clamp(1, i32::from(st.max_point.max(1))) as i8
        });
        if let Some(existing) = snap
            .skills
            .iter_mut()
            .find(|sk| sk.template_id == i32::from(stid))
        {
            let was_zero = existing.point <= 0;
            let final_pt = existing.point.max(new_pt);
            existing.point = final_pt;
            let lv_id = st
                .levels
                .iter()
                .find(|l| l.point == final_pt)
                .or_else(|| st.levels.first())
                .map_or(i16::from(stid), |l| l.id);
            Some(if was_zero {
                s2c_packets::me_add_skill(lv_id)
            } else {
                s2c_packets::me_update_skill(lv_id)
            })
        } else {
            let lv_id = st
                .levels
                .iter()
                .find(|l| l.point == new_pt)
                .or_else(|| st.levels.first())
                .map_or(i16::from(stid), |l| l.id);
            snap.skills.push(SkillSlot {
                template_id: i32::from(stid),
                point: new_pt,
                last_time_use_ms: 0,
                curr_level: 0,
            });
            Some(s2c_packets::me_add_skill(lv_id))
        }
    }
}
