//! Shop sell (`7`, `handle_shop_sell`) and Bà Hạt Mít / Whis combine preview (`-81`).

use nro_consts::const_npc;
use nro_core::Gender;
use nro_economy::{CombineContext, CombineOp, CombineService, ItemCatalog};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, npc_menu_index, pending_combine,
    send_thong_bao, snapshot_to_economy_inv, sprite_restore_packets, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use super::{buyback_ledgers, is_in_trade};

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_shop_sell(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: i8,
        where_slot: i8,
        index: i16,
    ) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        let Ok(idx) = usize::try_from(index) else {
            return Ok(());
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        if action == 0 {
            if let Some(text) = self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .and_then(|entry| {
                    let it = if where_slot == 0 {
                        entry.snapshot.items_body.get(idx)?.as_ref()?
                    } else {
                        entry.snapshot.items_bag.get(idx)?.as_ref()?
                    };
                    let (qty, meta) = (it.quantity, catalog.meta_of(it.template_id));
                    let price = if it.template_id.0 == 457 {
                        u64::from(qty) * 37_000_000
                    } else {
                        (meta.base_gold_value / 4).max(1) * u64::from(qty)
                    };
                    let name = self
                        .shared
                        .templates
                        .items
                        .iter()
                        .find(|r| r.id == i32::from(it.template_id.0))
                        .map_or("vật phẩm", |r| r.name.as_str());
                    Some(format!(
                        "Bạn có muốn bán {qty} {name} với giá {price} vàng không?"
                    ))
                })
            {
                session
                    .send(s2c_packets::item_sale_confirm(where_slot, index, &text)?)
                    .await?;
            }
            return Ok(());
        }
        let mut in_trade = false;
        if let Some((payout, bag_pkt, money_pkt, body_out)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if is_in_trade(entry.snapshot.id) {
                    in_trade = true;
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let payout = if where_slot == 1 {
                    let qty = inv.bag_item(idx).ok()?.quantity;
                    let mut ledger = buyback_ledgers().entry(entry.snapshot.id).or_default();
                    ledger.sell_from_bag(&mut inv, idx, qty, &catalog).ok()?
                } else if where_slot == 0 {
                    let item = inv.body.get_mut(idx)?.take()?;
                    let p = (catalog.meta_of(item.template_id).base_gold_value / 4).max(1)
                        * u64::from(item.quantity);
                    inv.add_gold_clamped(p);
                    p
                } else {
                    return None;
                };
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                let body_out = (where_slot == 0).then(|| {
                    super::super::recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                    let (eh, eb, el, ebag, is_disg) =
                        effective_appearance(&self.shared.templates, &entry.snapshot);
                    let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                    if let (Some(ek), Ok(tx)) = (
                        entry.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    (
                        s2c_packets::inventory_body(eh, &entry.snapshot.items_body),
                        s2c_packets::me_load_point(&entry.snapshot.stats),
                        sprite_restore_packets(pid, eh, eb, el, is_disg),
                        s2c_packets::send_flag_bag(pid, ebag),
                        loc,
                    )
                });
                Some((
                    payout,
                    s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    wallet_money_pkt(&entry.snapshot.wallet),
                    body_out,
                ))
            })
        {
            session.send(bag_pkt).await?;
            session.send(money_pkt).await?;
            if let Some((body_pkt, pt_pkt, upd_bodies, flag_bag, loc)) = body_out {
                session.send(body_pkt).await?;
                session.send(pt_pkt).await?;
                for p in upd_bodies.into_iter().flatten() {
                    self.broadcast_zone_or_self(loc, sid, &p);
                }
                self.broadcast_zone_or_self(loc, sid, &flag_bag);
            }
            session
                .send(s2c_packets::thong_bao(&format!(
                    "Đã bán vật phẩm nhận {payout} vàng!"
                ))?)
                .await?;
        } else if in_trade {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_combine_preview(
        &self,
        session: &SessionHandle,
        sid: u64,
        bag_indices: &[i8],
    ) -> Result<(), ProtocolError> {
        let out = self.shared.clients.authenticated.get(&sid).map(|entry| {
            let pid = entry.snapshot.id;
            if is_in_trade(pid) {
                return Err("Không thể thao tác khi đang giao dịch!");
            }
            let Some((npc_id, op, _)) = pending_combine().get(&pid).map(|v| v.value().clone())
            else {
                return Err("Chưa mở giao diện kết hợp vật phẩm!");
            };
            let slots: Vec<usize> = bag_indices
                .iter()
                .filter_map(|&i| usize::try_from(i).ok())
                .collect();
            let wearing_full_destroyer = entry.snapshot.items_body.len() >= 5
                && entry.snapshot.items_body[..5].iter().all(|s| {
                    s.as_ref()
                        .is_some_and(|it| (650..=662).contains(&it.template_id.0))
                });
            let special_tid = match entry.snapshot.gender {
                Gender::TraiDat => 24,
                Gender::Xayda => 25,
                Gender::Namec => 26,
            };
            let (skill_point, skill_mastery) = entry
                .snapshot
                .skills
                .iter()
                .find(|sk| sk.template_id == special_tid)
                .map_or((0_u8, 0_u16), |sk| {
                    (
                        u8::try_from(sk.point.max(0)).unwrap_or(0),
                        u16::try_from(sk.curr_level.max(0)).unwrap_or(0),
                    )
                });
            let ctx = CombineContext {
                gender: entry.snapshot.gender,
                power: entry.snapshot.stats.power,
                wearing_full_destroyer,
                skill_point,
                skill_mastery,
                parity: self.shared.config.parity_mode,
            };
            let res = CombineService::preview(
                &snapshot_to_economy_inv(&entry.snapshot),
                op,
                &slots,
                &ctx,
                &DbItemCatalog(&self.shared.templates),
            );
            pending_combine().insert(pid, (npc_id, op, slots));
            let pkt = match res {
                Ok(q) => {
                    npc_menu_index().insert(pid, (npc_id, const_npc::MENU_START_COMBINE));
                    let btns: &[&str] = if op == CombineOp::PhaLeHoaTrangBi {
                        &["Nâng cấp", "Đập 10 lần", "Đập 100 lần", "Từ chối"]
                    } else {
                        &["Nâng cấp", "Từ chối"]
                    };
                    s2c_packets::npc_open_menu(
                        npc_id,
                        &format!(
                            "Tỉ lệ: {}.{:02}%\nVàng: {} | Ngọc: {}",
                            q.ui_rate_bps / 100,
                            q.ui_rate_bps % 100,
                            q.gold_cost,
                            q.gem_cost
                        ),
                        btns,
                        None,
                    )
                    .ok()
                }
                Err(err) => s2c_packets::npc_open_say(
                    npc_id,
                    &format!("Nguyên liệu chưa hợp lệ: {err}"),
                    None,
                )
                .ok(),
            };
            Ok(pkt)
        });
        match out {
            Some(Ok(Some(p))) => session.send(p).await?,
            Some(Err(msg)) => send_thong_bao(session, msg)?,
            _ => {}
        }
        Ok(())
    }
}
