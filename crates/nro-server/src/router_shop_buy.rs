//! Shop purchase (`6`, `handle_shop_buy`).

#[path = "router_shop_buy_aux.rs"]
mod router_shop_buy_aux;

use nro_core::{ItemInstance, TemplateId};
use nro_economy::{ContainerKind, ItemCatalog, ShopType};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::{
    DbItemCatalog, ServerSessionHandler, open_shop_state, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::is_in_trade;
use crate::runtime::unix_now_ms;
use router_shop_buy_aux::{buy_from_buyback_ledger, buy_from_lucky_round_box};

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_shop_buy(
        &self,
        session: &SessionHandle,
        sid: u64,
        bt: i8,
        item_id: TemplateId,
        qty: i16,
    ) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        let (count, catalog) = (
            if matches!(item_id.0, 517 | 518) {
                1
            } else {
                u32::try_from(qty.max(1)).unwrap_or(1)
            },
            DbItemCatalog(&self.shared.templates),
        );
        let clan_buy = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            let tg = open_shop_state().get(&e.snapshot.id)?;
            let s = self
                .shared
                .templates
                .shops
                .iter()
                .find(|s| s.tag_name.as_deref() == Some(tg.value().as_str()))?;
            let (t, it) = s.tabs.iter().find_map(|t| {
                t.items
                    .iter()
                    .find(|i| i.temp_id == item_id)
                    .map(|i| (t, i))
            })?;
            (60..=62).contains(&t.id).then_some((
                e.snapshot.id,
                e.snapshot.clan_id,
                u32::try_from(
                    u64::try_from(it.cost.max(0))
                        .unwrap_or(0)
                        .saturating_mul(u64::from(count)),
                )
                .unwrap_or(u32::MAX),
            ))
        });
        let mut clan_stage = None;
        if let Some((pid, cid, need)) = clan_buy {
            if cid.0 < 0 {
                return send_thong_bao(session, "Không thể mua: bạn chưa tham gia bang hội!");
            }
            let guard = super::super::router_secondary_clan::clan_op_lock()
                .lock()
                .await;
            let Some(mut clan) = self.shared.clans_repo.get_by_id(cid).await.ok().flatten() else {
                return send_thong_bao(session, "Không thể mua: không tìm thấy bang hội!");
            };
            let Some(m) = clan.members.iter_mut().find(|m| m.id == pid.0 as i32) else {
                return send_thong_bao(session, "Không thể mua: bạn không thuộc bang hội này!");
            };
            if u32::try_from(m.member_point.max(0)).unwrap_or(0) < need {
                return send_thong_bao(session, "Không thể mua: không đủ Capsule Bang!");
            }
            m.member_point = m
                .member_point
                .saturating_sub(i32::try_from(need).unwrap_or(i32::MAX));
            clan_stage = Some((guard, clan));
        }
        let mut buy_ok = false;
        let out = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if is_in_trade(entry.snapshot.id) {
                    return (
                        s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!")
                            .into_iter()
                            .collect(),
                        None,
                    );
                }
                let tag = open_shop_state()
                    .get(&entry.snapshot.id)
                    .map(|t| t.value().clone());
                let shop = tag.as_deref().and_then(|tg| {
                    self.shared
                        .templates
                        .shops
                        .iter()
                        .find(|s| s.tag_name.as_deref() == Some(tg))
                });
                let type_shop = if tag.as_deref() == Some("ITEMS_LUCKY_ROUND") {
                    ShopType::LuckyRoundBox as i8
                } else {
                    shop.map_or(0, |s| s.type_shop)
                };
                if type_shop == 1 {
                    let mut pkts = Vec::new();
                    let in_shop = shop.is_some_and(|s| {
                        s.tabs
                            .iter()
                            .flat_map(|t| &t.items)
                            .any(|i| i.temp_id == item_id)
                    });
                    let note = if !in_shop {
                        "Vật phẩm không có trong cửa hàng!"
                    } else if let Some(sk) = self.learn_shop_skill(&mut entry.snapshot, item_id) {
                        pkts.push(sk);
                        "Học kỹ năng thành công!"
                    } else {
                        "Chưa đủ sức mạnh hoặc tiềm năng!"
                    };
                    pkts.extend([
                        s2c_packets::me_load_point(&entry.snapshot.stats),
                        wallet_money_pkt(&entry.snapshot.wallet),
                    ]);
                    if let Ok(tb) = s2c_packets::thong_bao(note) {
                        pkts.push(tb);
                    }
                    return (pkts, None);
                }
                if type_shop == ShopType::LuckyRoundBox as i8 {
                    return buy_from_lucky_round_box(
                        &mut entry.snapshot,
                        bt,
                        item_id,
                        tag,
                        shop,
                        &catalog,
                    );
                }
                if type_shop == ShopType::BuyBack as i8 {
                    return buy_from_buyback_ledger(
                        &mut entry.snapshot,
                        item_id,
                        tag,
                        shop,
                        &catalog,
                    );
                }
                let charm_mins = tag.as_deref().and_then(|tg| match tg {
                    "BUA_1H" => Some(60_i64),
                    "BUA_8H" => Some(480),
                    "BUA_1M" => Some(43_200),
                    _ => None,
                });
                let rec = shop.and_then(|s| {
                    s.tabs.iter().find_map(|t| {
                        t.items
                            .iter()
                            .find(|i| i.temp_id == item_id)
                            .map(|i| (t, i))
                    })
                });
                let (mut inv, mut new_ep) = (snapshot_to_economy_inv(&entry.snapshot), None::<i32>);
                let res: Result<(), String> = (|| {
                    let Some((tab, it)) = rec else {
                        return Err("Không tìm thấy vật phẩm!".into());
                    };
                    let (is_bill, is_disc, is_ep, is_cc) = (
                        (21..=23).contains(&tab.id),
                        matches!(tab.id, 30 | 50),
                        tab.id == 59,
                        (60..=62).contains(&tab.id),
                    );
                    if is_bill {
                        if entry.snapshot.items_body.len() < 5
                            || !entry.snapshot.items_body[..5].iter().all(|s| {
                                s.as_ref()
                                    .is_some_and(|b| (555..=567).contains(&b.template_id.0))
                            })
                        {
                            return Err("Cần mặc đủ 5 trang bị Thần Linh!".into());
                        }
                        for _ in 0..count {
                            let fid = (663..=667)
                                .map(TemplateId)
                                .find(|&f| inv.count_template_in_bag(f) >= 99)
                                .ok_or_else(|| "Cần 99 thức ăn Thần (663..=667)!".to_owned())?;
                            inv.sub_template_from_bag(fid, 99)
                                .map_err(|e| format!("{e}"))?;
                        }
                    }
                    if is_disc {
                        inv.sub_template_from_bag(TemplateId(459), count)
                            .map_err(|_| "Cần Phiếu giảm giá (459)!".to_owned())?;
                    }
                    let unit = u64::try_from(it.cost.max(0)).unwrap_or(0);
                    let cost = if is_disc {
                        u64::try_from((u128::from(unit) * 20) / 100)
                            .unwrap_or(u64::MAX)
                            .max(1)
                            .saturating_mul(u64::from(count))
                    } else {
                        unit.saturating_mul(u64::from(count))
                    };
                    let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
                    if is_ep {
                        let ep = u32::try_from(entry.snapshot.event_point.max(0)).unwrap_or(0);
                        if ep < c32 {
                            return Err("Không đủ điểm sự kiện!".into());
                        }
                        new_ep = Some(i32::try_from(ep - c32).unwrap_or(0));
                    } else if is_cc {
                        if clan_stage.is_none() {
                            return Err("Không đủ Capsule Bang!".into());
                        }
                    } else if type_shop == 3 {
                        let spec_tid = if it.icon_spec == 4028 {
                            TemplateId(457)
                        } else {
                            self.shared
                                .templates
                                .items
                                .iter()
                                .find(|r| r.icon_id == it.icon_spec)
                                .map(|r| TemplateId(r.id as i16))
                                .ok_or_else(|| "Vật phẩm đổi không hợp lệ!".to_owned())?
                        };
                        match spec_tid.0 {
                            77 => inv.sub_gem(c32).map_err(|e| format!("{e}"))?,
                            861 => inv.sub_ruby(c32).map_err(|e| format!("{e}"))?,
                            _ => {
                                inv.sub_template_from_bag(spec_tid, c32)
                                    .map_err(|e| format!("{e}"))?;
                            }
                        }
                    } else {
                        match it.type_sell {
                            1 => inv.sub_gem(c32),
                            3 => inv.sub_ruby(c32),
                            _ => inv.sub_gold(cost),
                        }
                        .map_err(|e| format!("{e}"))?;
                    }
                    if let Some(mins) = charm_mins {
                        if !entry.snapshot.charms.extend_minutes(
                            item_id.0,
                            mins * i64::from(count),
                            unix_now_ms(),
                        ) {
                            return Err("Bùa không hợp lệ!".into());
                        }
                    } else {
                        match item_id.0 {
                            517 => {
                                inv.expand_slots(ContainerKind::Bag)
                                    .map_err(|e| format!("{e}"))?;
                            }
                            518 => {
                                inv.expand_slots(ContainerKind::Box)
                                    .map_err(|e| format!("{e}"))?;
                            }
                            _ => {
                                let (meta, now_ms) = (catalog.meta_of(item_id), unix_now_ms());
                                if meta.stackable {
                                    inv.add_to_bag(
                                        ItemInstance {
                                            template_id: item_id,
                                            quantity: count,
                                            options: it.options.clone(),
                                            create_time_ms: now_ms,
                                        },
                                        true,
                                    )
                                    .map_err(|e| format!("{e}"))?;
                                } else {
                                    if inv.empty_bag_slots() < count as usize {
                                        return Err("Hành trang không đủ ô trống!".into());
                                    }
                                    for _ in 0..count {
                                        inv.add_to_bag(
                                            ItemInstance {
                                                template_id: item_id,
                                                quantity: 1,
                                                options: it.options.clone(),
                                                create_time_ms: now_ms,
                                            },
                                            false,
                                        )
                                        .map_err(|e| format!("{e}"))?;
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                })();
                if res.is_ok() {
                    buy_ok = true;
                    if let Some(ep) = new_ep {
                        entry.snapshot.event_point = ep;
                    }
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                }
                let mut pkts = if charm_mins.is_some() {
                    vec![wallet_money_pkt(&entry.snapshot.wallet)]
                } else {
                    vec![
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        s2c_packets::inventory_box(&entry.snapshot.items_box),
                        wallet_money_pkt(&entry.snapshot.wallet),
                    ]
                };
                let msg = match &res {
                    Ok(()) => "Mua vật phẩm thành công!".to_owned(),
                    Err(e) => format!("Không thể mua: {e}"),
                };
                if let Ok(tb) = s2c_packets::thong_bao(&msg) {
                    pkts.push(tb);
                }
                let reopen = (res.is_ok() && charm_mins.is_some())
                    .then(|| tag.map(|tg| (shop.map_or(21, |s| i16::from(s.npc_id)), tg)))
                    .flatten();
                (pkts, reopen)
            });
        if let Some((guard, clan)) = clan_stage {
            if buy_ok {
                let _ = self.shared.clans_repo.update_clan(&clan).await;
            }
            drop(guard);
        }
        if let Some((pkts, reopen)) = out {
            for p in pkts {
                session.send(p).await?;
            }
            if let Some((npc_id, tg)) = reopen {
                self.handle_open_shop(session, sid, npc_id, &tg).await?;
            }
        }
        Ok(())
    }
}
