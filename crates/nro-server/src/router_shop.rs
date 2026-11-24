//! Shops (`-44`, `handle_open_shop`) and buyback ledger (`BuyBackLedger::items`).

#[path = "router_shop_buy.rs"]
mod router_shop_buy;
#[path = "router_shop_combine.rs"]
mod router_shop_combine;
#[path = "router_shop_consign.rs"]
mod router_shop_consign;

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_core::{ItemInstance, PlayerId, TemplateId};
use nro_economy::{BuyBackLedger, ItemCatalog, ShopType};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets::{self, ShopItemWire, ShopTabWire};

pub(super) use super::router_shop_trade::{is_in_trade, trade_lifecycle_lock};
use super::{DbItemCatalog, ServerSessionHandler, open_shop_state};
use crate::runtime::unix_now_ms;

pub(self) fn buyback_ledgers() -> &'static DashMap<PlayerId, BuyBackLedger> {
    static M: OnceLock<DashMap<PlayerId, BuyBackLedger>> = OnceLock::new();
    M.get_or_init(DashMap::new)
}

pub(super) fn clear_buyback_ledger(pid: PlayerId) {
    buyback_ledgers().remove(&pid);
}

pub(self) fn buyback_items_snapshot(pid: PlayerId) -> Vec<ItemInstance> {
    buyback_ledgers()
        .get(&pid)
        .map_or_else(Vec::new, |l| l.value().items().to_vec())
}

impl ServerSessionHandler {
    pub(self) fn is_sid_in_trade(&self, sid: u64) -> bool {
        self.shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| is_in_trade(e.snapshot.id))
    }

    fn item_part_temp(&self, tid: TemplateId) -> Option<(i16, i16, i16, i16)> {
        self.shared
            .templates
            .items
            .iter()
            .find(|r| r.id == i32::from(tid.0))
            .and_then(|r| {
                (r.head != -1).then_some((r.head as i16, r.body as i16, r.leg as i16, -1))
            })
    }

    pub(super) async fn handle_open_shop(
        &self,
        session: &SessionHandle,
        sid: u64,
        npc_id: i16,
        tag: &str,
    ) -> Result<(), ProtocolError> {
        let Some((pid, gender, charms)) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| (e.snapshot.id, e.snapshot.gender, e.snapshot.charms))
        else {
            return Ok(());
        };
        let shop = self
            .shared
            .templates
            .shops
            .iter()
            .find(|s| s.tag_name.as_deref() == Some(tag))
            .or_else(|| {
                (tag != "ITEMS_LUCKY_ROUND")
                    .then(|| {
                        self.shared
                            .templates
                            .shops
                            .iter()
                            .find(|s| i16::from(s.npc_id) == npc_id)
                    })
                    .flatten()
            });
        if tag == "ITEMS_LUCKY_ROUND"
            || shop.is_some_and(|s| s.type_shop == ShopType::LuckyRoundBox as i8)
        {
            open_shop_state().insert(
                pid,
                shop.and_then(|s| s.tag_name.clone())
                    .unwrap_or_else(|| tag.to_owned()),
            );
            let items: Vec<ItemInstance> = self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .map_or_default(|e| {
                    e.snapshot
                        .items_box_lucky_round
                        .iter()
                        .flatten()
                        .cloned()
                        .collect()
                });
            let wire_items: Vec<ShopItemWire<'_>> = items
                .iter()
                .map(|it| ShopItemWire {
                    template_id: it.template_id.0,
                    buy_coin_gold: 0,
                    buy_gold_gem: 0,
                    power_require: 0,
                    icon_spec: 0,
                    buy_spec: 0,
                    reason: "",
                    quantity: i32::try_from(it.quantity).unwrap_or(i32::MAX),
                    consign: None,
                    options: it.options.as_slice(),
                    is_new: false,
                    part_temp: self.item_part_temp(it.template_id),
                })
                .collect();
            let tab_name = shop
                .and_then(|s| s.tabs.first())
                .map_or("Rương phụ\nQuay số", |t| t.name.as_str());
            return session
                .send(s2c_packets::shop_open(
                    ShopType::LuckyRoundBox as i8,
                    &[ShopTabWire {
                        name: tab_name,
                        max_page: 1,
                        items: &wire_items,
                    }],
                )?)
                .await;
        }
        let Some(shop) = shop else {
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    &format!("Cửa hàng {tag} đang mở!"),
                    None,
                )?)
                .await;
        };
        let resolved = shop.tag_name.clone().unwrap_or_else(|| tag.to_owned());
        let is_charm = matches!(resolved.as_str(), "BUA_1H" | "BUA_8H" | "BUA_1M");
        open_shop_state().insert(pid, resolved);
        if shop.type_shop == ShopType::BuyBack as i8 {
            let (catalog, items) = (
                DbItemCatalog(&self.shared.templates),
                buyback_items_snapshot(pid),
            );
            let wire_items: Vec<ShopItemWire<'_>> = items
                .iter()
                .map(|it| {
                    let meta = catalog.meta_of(it.template_id);
                    let (gold, gem) = if it.template_id.0 == 457 {
                        (
                            i32::try_from(u64::from(it.quantity).saturating_mul(74_000_000))
                                .unwrap_or(i32::MAX),
                            0,
                        )
                    } else if meta.base_gem_value > 0 {
                        (
                            0,
                            i32::try_from(
                                (meta.base_gem_value / 2).max(1).saturating_mul(it.quantity),
                            )
                            .unwrap_or(i32::MAX),
                        )
                    } else {
                        let g = if meta.base_gold_value > 0 {
                            (meta.base_gold_value / 2)
                                .max(1)
                                .saturating_mul(u64::from(it.quantity))
                        } else {
                            u64::from(it.quantity).saturating_mul(100)
                        };
                        (i32::try_from(g).unwrap_or(i32::MAX), 0)
                    };
                    ShopItemWire {
                        template_id: it.template_id.0,
                        buy_coin_gold: gold,
                        buy_gold_gem: gem,
                        power_require: 0,
                        icon_spec: 0,
                        buy_spec: 0,
                        reason: "",
                        quantity: i32::try_from(it.quantity).unwrap_or(i32::MAX),
                        consign: None,
                        options: it.options.as_slice(),
                        is_new: false,
                        part_temp: self.item_part_temp(it.template_id),
                    }
                })
                .collect();
            let tab_name = shop
                .tabs
                .first()
                .map_or("Mua lại\nVật phẩm", |t| t.name.as_str());
            return session
                .send(s2c_packets::shop_open(
                    ShopType::BuyBack as i8,
                    &[ShopTabWire {
                        name: tab_name,
                        max_page: 1,
                        items: &wire_items,
                    }],
                )?)
                .await;
        }
        let (g_i16, now_ms) = (i16::from(i8::from(gender)), unix_now_ms());
        let mut tab_rows = Vec::with_capacity(shop.tabs.len());
        for tab in &shop.tabs {
            let mut rows = Vec::with_capacity(tab.items.len());
            for it in &tab.items {
                let tpl = self
                    .shared
                    .templates
                    .items
                    .iter()
                    .find(|r| r.id == i32::from(it.temp_id.0));
                if (shop.type_shop == 1 || (41..=43).contains(&tab.id))
                    && let Some(r) = tpl
                    && r.gender != -1
                    && r.gender != 3
                    && r.gender != g_i16
                {
                    continue;
                }
                let part_temp = tpl.and_then(|r| {
                    (r.head != -1).then_some((r.head as i16, r.body as i16, r.leg as i16, -1))
                });
                let pwr = tpl.map_or(i64::from(it.cost), |r| {
                    i64::from(r.power_require).max(i64::from(it.cost))
                });
                let opts = is_charm.then(|| {
                    let (mut o, rem) = (
                        it.options.clone(),
                        charms.remaining_option(it.temp_id.0, now_ms),
                    );
                    if let Some(s) = o.iter_mut().find(|x| x.id == 66) {
                        *s = rem;
                    } else {
                        o.push(rem);
                    }
                    o
                });
                rows.push((it, part_temp, pwr, opts));
            }
            tab_rows.push((tab.name.as_str(), rows));
        }
        let built: Vec<(&str, Vec<ShopItemWire<'_>>)> = tab_rows
            .iter()
            .map(|(n, rows)| {
                (
                    *n,
                    rows.iter()
                        .map(|(it, pt, pwr, opts)| ShopItemWire {
                            template_id: it.temp_id.0,
                            buy_coin_gold: if it.type_sell == 0 { it.cost } else { 0 },
                            buy_gold_gem: if matches!(it.type_sell, 1 | 3) {
                                it.cost
                            } else {
                                0
                            },
                            power_require: *pwr,
                            icon_spec: it.icon_spec as i16,
                            buy_spec: it.cost,
                            reason: "",
                            quantity: 1,
                            consign: None,
                            options: opts.as_deref().unwrap_or(&it.options),
                            is_new: it.is_new,
                            part_temp: *pt,
                        })
                        .collect(),
                )
            })
            .collect();
        let wire_tabs: Vec<ShopTabWire<'_>> = built
            .iter()
            .map(|(n, list)| ShopTabWire {
                name: n,
                max_page: 1,
                items: list.as_slice(),
            })
            .collect();
        session
            .send(s2c_packets::shop_open(shop.type_shop, &wire_tabs)?)
            .await
    }
}
