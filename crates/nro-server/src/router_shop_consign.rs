//! Consignment market (`-100`, `handle_open_consign`, `handle_consign_action`).

use nro_core::{ItemInstance, MapId, PlayerId};
use nro_economy::{ConsignCurrency, ConsignListing, ConsignTab, ItemCatalog};
use nro_protocol::{ConsignAction, ProtocolError, SessionHandle};
use nro_world::s2c_packets::{self, ConsignItemWire, ShopItemWire, ShopTabWire};

use super::super::{
    DbItemCatalog, ServerSessionHandler, open_shop_state, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::is_in_trade;
use crate::runtime::unix_now_ms;

const CONSIGN_TABS: [(&str, Option<ConsignTab>); 5] = [
    ("Trang bị\nÁo Quần", Some(ConsignTab::AoQuan)),
    ("Phụ kiện\nGăng Tay", Some(ConsignTab::GangTay)),
    ("Phụ kiện\nĐặc biệt", Some(ConsignTab::PhuKien)),
    ("Linh tinh", Some(ConsignTab::LinhTinh)),
    ("Quản lý\nKý gửi", None),
];

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_open_consign(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let Some((pid, bag_items)) = self.consign_bag_snapshot(sid) else {
            return Ok(());
        };
        open_shop_state().insert(pid, "CONSIGN".to_owned());
        let pkt = {
            let market = self.shared.consign_shop.lock();
            let raw: Vec<Vec<ConsignListing>> = CONSIGN_TABS
                .iter()
                .map(|&(_, opt_t)| consign_listings_for(&market, pid, opt_t))
                .collect();
            let mut wire: Vec<Vec<ShopItemWire<'_>>> = raw
                .iter()
                .map(|lst| {
                    lst.iter()
                        .map(|l| {
                            let (gold, gem) = listing_prices(l);
                            ShopItemWire {
                                template_id: l.item.template_id.0,
                                buy_coin_gold: gold,
                                buy_gold_gem: gem,
                                power_require: 0,
                                icon_spec: 0,
                                buy_spec: 0,
                                reason: "",
                                quantity: l.item.quantity as i32,
                                consign: Some((
                                    l.id as i16,
                                    if l.sold { 2_i8 } else { 1_i8 },
                                    i8::from(l.seller_id == pid),
                                    "",
                                )),
                                options: l.item.options.as_slice(),
                                is_new: false,
                                part_temp: None,
                            }
                        })
                        .collect()
                })
                .collect();
            if let Some(tab4) = wire.get_mut(4) {
                tab4.extend(bag_items.iter().map(|(slot, it)| ShopItemWire {
                    template_id: it.template_id.0,
                    buy_coin_gold: -1,
                    buy_gold_gem: -1,
                    power_require: 0,
                    icon_spec: 0,
                    buy_spec: 0,
                    reason: "",
                    quantity: it.quantity as i32,
                    consign: Some((*slot as i16, 0, 0, "")),
                    options: it.options.as_slice(),
                    is_new: false,
                    part_temp: None,
                }));
            }
            let tabs: Vec<ShopTabWire<'_>> = CONSIGN_TABS
                .iter()
                .zip(&wire)
                .map(|(&(name, _), items)| ShopTabWire {
                    name,
                    max_page: u8::try_from(items.len().max(1).div_ceil(20).min(127)).unwrap_or(1),
                    items: &items[..items.len().min(20)],
                })
                .collect();
            s2c_packets::shop_open(2, &tabs)?
        };
        session.send(pkt).await
    }

    pub(in crate::router) async fn handle_consign_action(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: ConsignAction,
    ) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        let Some((activated, can_consign)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            (
                e.activated,
                e.snapshot.location.map_id == MapId(84)
                    || open_shop_state()
                        .get(&e.snapshot.id)
                        .is_some_and(|s| s.value() == "CONSIGN"),
            )
        }) else {
            return Ok(());
        };
        if !activated {
            return send_thong_bao(
                session,
                "Tài khoản chưa kích hoạt thành viên, không thể ký gửi!",
            );
        }
        if !can_consign {
            return send_thong_bao(session, "Chỉ có thể thao tác ký gửi tại Siêu Thị!");
        }
        if let ConsignAction::OpenTab(tab, page) = action {
            let Some(&(_, opt_t)) = usize::try_from(tab).ok().and_then(|i| CONSIGN_TABS.get(i))
            else {
                return Ok(());
            };
            let Some((pid, bag_items)) = self.consign_bag_snapshot(sid) else {
                return Ok(());
            };
            let pkt = {
                let market = self.shared.consign_shop.lock();
                let listings = consign_listings_for(&market, pid, opt_t);
                let mut items: Vec<ConsignItemWire<'_>> = listings
                    .iter()
                    .map(|l| {
                        let (gold, gem) = listing_prices(l);
                        ConsignItemWire {
                            template_id: l.item.template_id.0,
                            listing_id: l.id as i16,
                            buy_coin_gold: gold,
                            buy_gold_gem: gem,
                            buy_type: if l.sold { 2_i8 } else { 1_i8 },
                            quantity: l.item.quantity as i32,
                            is_me: i8::from(l.seller_id == pid),
                            options: l.item.options.as_slice(),
                            part_temp: None,
                            seller_name: "",
                        }
                    })
                    .collect();
                if tab == 4 {
                    items.extend(bag_items.iter().map(|(slot, it)| ConsignItemWire {
                        template_id: it.template_id.0,
                        listing_id: *slot as i16,
                        buy_coin_gold: -1,
                        buy_gold_gem: -1,
                        buy_type: 0,
                        quantity: it.quantity as i32,
                        is_me: 0,
                        options: it.options.as_slice(),
                        part_temp: None,
                        seller_name: "",
                    }));
                }
                let max_page = items.len().max(1).div_ceil(20).min(127) as i8;
                let curr_page = page.clamp(0, (max_page - 1).max(0));
                let start = usize::try_from(curr_page)
                    .unwrap_or(0)
                    .saturating_mul(20)
                    .min(items.len());
                s2c_packets::consign_tab_page(
                    tab,
                    max_page,
                    curr_page,
                    &items[start..(start + 20).min(items.len())],
                )?
            };
            return session.send(pkt).await;
        }
        let (mut in_trade, market_first) =
            (false, matches!(action, ConsignAction::ClaimOrCancel(..)));
        if let Some((res, persist, bag_pkt, money_pkt)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if is_in_trade(entry.snapshot.id) {
                    in_trade = true;
                    return None;
                }
                let catalog = DbItemCatalog(&self.shared.templates);
                let (pid, pwr, mut inv) = (
                    entry.snapshot.id,
                    entry.snapshot.stats.power,
                    snapshot_to_economy_inv(&entry.snapshot),
                );
                let res = {
                    let mut market = self.shared.consign_shop.lock();
                    match action {
                        ConsignAction::Create(tid, mtype, money, qty) => {
                            let cur = if mtype == 1 {
                                ConsignCurrency::Gem
                            } else {
                                ConsignCurrency::Gold
                            };
                            let slot = usize::try_from(tid.0)
                                .ok()
                                .filter(|&s| inv.bag_item(s).is_ok())
                                .unwrap_or(usize::MAX);
                            market
                                .list_item_with_catalog(
                                    &mut inv,
                                    pid,
                                    slot,
                                    cur,
                                    u64::from(money.max(0).cast_unsigned()),
                                    u32::try_from(qty).unwrap_or(0),
                                    &catalog,
                                )
                                .map(|_| ())
                        }
                        ConsignAction::ClaimOrCancel(1, lid) => market.cancel_listing_with_catalog(
                            &mut inv,
                            pid,
                            u64::from(lid.0.max(0).cast_unsigned()),
                            &catalog,
                        ),
                        ConsignAction::ClaimOrCancel(2, lid) => market
                            .claim_proceeds(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned()))
                            .map(|_| ()),
                        ConsignAction::Buy(lid, _, _) => market.buy_item_with_catalog(
                            &mut inv,
                            pid,
                            pwr,
                            u64::from(lid.0.max(0).cast_unsigned()),
                            &catalog,
                        ),
                        ConsignAction::UpToTop(lid) => market.promote_to_top(
                            &mut inv,
                            pid,
                            u64::from(lid.0.max(0).cast_unsigned()),
                        ),
                        _ => Ok(()),
                    }
                };
                let persist = res.is_ok().then(|| {
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    (entry.snapshot.clone(), unix_now_ms())
                });
                Some((
                    res,
                    persist,
                    s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    wallet_money_pkt(&entry.snapshot.wallet),
                ))
            })
        {
            if let Some((snap, now_ms)) = persist {
                if market_first {
                    self.shared.persist_consign_shop().await;
                }
                if let Err(err) = self
                    .shared
                    .players_repo
                    .save_player(&snap, None, self.shared.config.parity_mode, now_ms)
                    .await
                {
                    tracing::warn!(player_id = snap.id.0, error = %err, "failed persisting player after consign action");
                }
                if !market_first {
                    self.shared.persist_consign_shop().await;
                }
            }
            session.send(bag_pkt).await?;
            session.send(money_pkt).await?;
            if let Err(err) = res {
                session
                    .send(s2c_packets::thong_bao(&format!("Ký gửi: {err}"))?)
                    .await?;
            }
            self.handle_open_consign(session, sid).await?;
        } else if in_trade {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        Ok(())
    }

    fn consign_bag_snapshot(&self, sid: u64) -> Option<(PlayerId, Vec<(usize, ItemInstance)>)> {
        let catalog = DbItemCatalog(&self.shared.templates);
        self.shared.clients.authenticated.get(&sid).map(|e| {
            (
                e.snapshot.id,
                e.snapshot
                    .items_bag
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        s.as_ref()
                            .filter(|it| is_consignable_bag_item(it, &catalog))
                            .map(|it| (i, it.clone()))
                    })
                    .collect(),
            )
        })
    }
}

fn listing_prices(l: &ConsignListing) -> (i32, i32) {
    let p = i32::try_from(l.price).unwrap_or(i32::MAX);
    (
        if l.currency == ConsignCurrency::Gold {
            p
        } else {
            -1
        },
        if l.currency == ConsignCurrency::Gem {
            p
        } else {
            -1
        },
    )
}

fn consign_listings_for(
    market: &nro_economy::ConsignShop,
    pid: PlayerId,
    tab: Option<ConsignTab>,
) -> Vec<ConsignListing> {
    match tab {
        Some(t) => market.listings_in_tab(t),
        None => market.seller_listings(pid),
    }
    .into_iter()
    .cloned()
    .collect()
}

fn is_consignable_bag_item(it: &ItemInstance, catalog: &dyn ItemCatalog) -> bool {
    !it.options.iter().any(|o| o.id == 30)
        && (it.options.iter().any(|o| matches!(o.id, 86 | 87))
            || matches!(catalog.meta_of(it.template_id).item_type, 6 | 14 | 15)
            || (14..=20).contains(&it.template_id.0))
}
