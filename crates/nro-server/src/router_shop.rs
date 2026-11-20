//! Shops (`-44`, `6`, `7`), buyback, consignment (`-100`), and combine preview (`-81`).

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::const_npc;
use nro_core::{Gender, ItemInstance, PlayerId, TemplateId};
use nro_economy::{BuyBackLedger, CombineContext, CombineOp, CombineService, ConsignCurrency, ConsignListing, ConsignTab, ContainerKind, ItemCatalog, ShopType};
use nro_protocol::{ConsignAction, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets::{self, ConsignItemWire, ShopItemWire, ShopTabWire}};

pub(super) use super::router_shop_trade::{is_in_trade, trade_lifecycle_lock};
use super::{DbItemCatalog, ServerSessionHandler, effective_appearance, npc_menu_index, open_shop_state, pending_combine, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt};
use crate::runtime::unix_now_ms;

const CONSIGN_TABS: [(&str, Option<ConsignTab>); 5] = [
    ("Trang bị\nÁo Quần", Some(ConsignTab::AoQuan)), ("Phụ kiện\nGăng Tay", Some(ConsignTab::GangTay)),
    ("Phụ kiện\nĐặc biệt", Some(ConsignTab::PhuKien)), ("Linh tinh", Some(ConsignTab::LinhTinh)), ("Quản lý\nKý gửi", None),
];

fn buyback_ledgers() -> &'static DashMap<PlayerId, BuyBackLedger> { static M: OnceLock<DashMap<PlayerId, BuyBackLedger>> = OnceLock::new(); M.get_or_init(DashMap::new) }

#[derive(serde::Deserialize)]
struct BuyBackItemsView {
    items: Vec<ItemInstance>,
}

fn buyback_items_snapshot(pid: PlayerId) -> Vec<ItemInstance> {
    buyback_ledgers()
        .get(&pid)
        .and_then(|ledger| toml::to_string(ledger.value()).ok())
        .and_then(|raw| toml::from_str::<BuyBackItemsView>(&raw).ok())
        .map_or_else(Vec::new, |v| v.items)
}

impl ServerSessionHandler {
    fn is_sid_in_trade(&self, sid: u64) -> bool { self.shared.clients.authenticated.get(&sid).is_some_and(|e| is_in_trade(e.snapshot.id)) }

    pub(super) async fn handle_open_shop(&self, session: &SessionHandle, sid: u64, npc_id: i16, tag: &str) -> Result<(), ProtocolError> {
        let Some((pid, gender, charms)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.gender, e.snapshot.charms)) else { return Ok(()); };
        let Some(shop) = self.shared.templates.shops.iter().find(|s| s.tag_name.as_deref() == Some(tag)).or_else(|| self.shared.templates.shops.iter().find(|s| i16::from(s.npc_id) == npc_id))
        else { return session.send(s2c_packets::npc_open_say(npc_id, &format!("Cửa hàng {tag} đang mở!"), None)?).await; };
        let resolved = shop.tag_name.clone().unwrap_or_else(|| tag.to_owned());
        let is_charm = matches!(resolved.as_str(), "BUA_1H" | "BUA_8H" | "BUA_1M");
        open_shop_state().insert(pid, resolved);
        if shop.type_shop == ShopType::BuyBack as i8 {
            let catalog = DbItemCatalog(&self.shared.templates);
            let items = buyback_items_snapshot(pid);
            let wire_items: Vec<ShopItemWire<'_>> = items.iter().map(|it| {
                let meta = catalog.meta_of(it.template_id);
                let (gold, gem) = if meta.base_gem_value > 0 {
                    (0, i32::try_from((meta.base_gem_value / 2).max(1).saturating_mul(it.quantity)).unwrap_or(i32::MAX))
                } else {
                    let g = if meta.base_gold_value > 0 { (meta.base_gold_value / 2).max(1).saturating_mul(u64::from(it.quantity)) } else { u64::from(it.quantity).saturating_mul(100) };
                    (i32::try_from(g).unwrap_or(i32::MAX), 0)
                };
                let part_temp = self.shared.templates.items.iter().find(|r| r.id == i32::from(it.template_id.0))
                    .and_then(|r| (r.head != -1).then_some((r.head as i16, r.body as i16, r.leg as i16, -1)));
                ShopItemWire {
                    template_id: it.template_id.0, buy_coin_gold: gold, buy_gold_gem: gem,
                    power_require: 0, icon_spec: 0, buy_spec: 0, reason: "",
                    quantity: i32::try_from(it.quantity).unwrap_or(i32::MAX), consign: None,
                    options: it.options.as_slice(), is_new: false, part_temp,
                }
            }).collect();
            let tab_name = shop.tabs.first().map_or("Mua lại\nVật phẩm", |t| t.name.as_str());
            let wire_tabs = [ShopTabWire { name: tab_name, max_page: 1, items: wire_items.as_slice() }];
            return session.send(s2c_packets::shop_open(ShopType::BuyBack as i8, &wire_tabs)?).await;
        }
        let (g_i16, now_ms) = (i16::from(i8::from(gender)), unix_now_ms());
        let mut tab_rows = Vec::with_capacity(shop.tabs.len());
        for tab in &shop.tabs {
            let mut rows = Vec::with_capacity(tab.items.len());
            for it in &tab.items {
                let tpl = self.shared.templates.items.iter().find(|r| r.id == i32::from(it.temp_id.0));
                if (shop.type_shop == 1 || (41..=43).contains(&tab.id)) && let Some(r) = tpl && r.gender != -1 && r.gender != 3 && r.gender != g_i16 { continue; }
                let part_temp = tpl.and_then(|r| (r.head != -1).then_some((r.head as i16, r.body as i16, r.leg as i16, -1)));
                let pwr = tpl.map_or(i64::from(it.cost), |r| i64::from(r.power_require).max(i64::from(it.cost)));
                let opts = is_charm.then(|| { let (mut o, rem) = (it.options.clone(), charms.remaining_option(it.temp_id.0, now_ms)); if let Some(s) = o.iter_mut().find(|x| x.id == 66) { *s = rem; } else { o.push(rem); } o });
                rows.push((it, part_temp, pwr, opts));
            }
            tab_rows.push((tab.name.as_str(), rows));
        }
        let built: Vec<(&str, Vec<ShopItemWire<'_>>)> = tab_rows.iter().map(|(n, rows)| (*n, rows.iter().map(|(it, pt, pwr, opts)| ShopItemWire {
            template_id: it.temp_id.0, buy_coin_gold: if it.type_sell == 0 { it.cost } else { 0 }, buy_gold_gem: if matches!(it.type_sell, 1 | 3) { it.cost } else { 0 },
            power_require: *pwr, icon_spec: it.icon_spec as i16, buy_spec: it.cost, reason: "", quantity: 1, consign: None, options: opts.as_deref().unwrap_or(&it.options), is_new: it.is_new, part_temp: *pt,
        }).collect())).collect();
        let wire_tabs: Vec<ShopTabWire<'_>> = built.iter().map(|(n, list)| ShopTabWire { name: n, max_page: 1, items: list.as_slice() }).collect();
        session.send(s2c_packets::shop_open(shop.type_shop, &wire_tabs)?).await
    }

    pub(super) async fn handle_shop_buy(&self, session: &SessionHandle, sid: u64, _bt: i8, item_id: TemplateId, qty: i16) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        let (count, catalog) = (if matches!(item_id.0, 517 | 518) { 1 } else { u32::try_from(qty.max(1)).unwrap_or(1) }, DbItemCatalog(&self.shared.templates));
        let out = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if is_in_trade(entry.snapshot.id) { return (s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!").into_iter().collect(), None); }
            let tag = open_shop_state().get(&entry.snapshot.id).map(|t| t.value().clone());
            let shop = tag.as_deref().and_then(|tg| self.shared.templates.shops.iter().find(|s| s.tag_name.as_deref() == Some(tg)));
            let type_shop = shop.map_or(0, |s| s.type_shop);
            if type_shop == 1 {
                let mut pkts = Vec::new();
                let note = if let Some(sk) = self.learn_shop_skill(&mut entry.snapshot, item_id) { pkts.push(sk); "Học kỹ năng thành công!" } else { "Chưa đủ sức mạnh hoặc tiềm năng!" };
                pkts.extend([s2c_packets::me_load_point(&entry.snapshot.stats), wallet_money_pkt(&entry.snapshot.wallet)]);
                if let Ok(tb) = s2c_packets::thong_bao(note) { pkts.push(tb); }
                return (pkts, None);
            }
            if type_shop == ShopType::BuyBack as i8 {
                let pid = entry.snapshot.id;
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let res: Result<(), String> = (|| {
                    let hidx = buyback_items_snapshot(pid).iter().position(|it| it.template_id == item_id)
                        .ok_or_else(|| "Không tìm thấy vật phẩm mua lại!".to_owned())?;
                    let mut ledger = buyback_ledgers().entry(pid).or_default();
                    ledger.repurchase(&mut inv, hidx, &catalog).map(|_| ()).map_err(|e| format!("{e}"))
                })();
                if res.is_ok() { sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot); }
                let mut pkts = vec![
                    s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    wallet_money_pkt(&entry.snapshot.wallet),
                ];
                let msg = match &res { Ok(()) => "Mua lại vật phẩm thành công!".to_owned(), Err(e) => format!("Không thể mua lại: {e}") };
                if let Ok(tb) = s2c_packets::thong_bao(&msg) { pkts.push(tb); }
                let reopen = res.is_ok().then(|| tag.map(|tg| (shop.map_or(0, |s| i16::from(s.npc_id)), tg))).flatten();
                return (pkts, reopen);
            }
            let charm_mins = tag.as_deref().and_then(|tg| match tg { "BUA_1H" => Some(60_i64), "BUA_8H" => Some(480), "BUA_1M" => Some(43_200), _ => None });
            let rec = shop.and_then(|s| s.tabs.iter().flat_map(|t| &t.items).find(|i| i.temp_id == item_id));
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let res: Result<(), String> = (|| {
                let Some(it) = rec else { return Err("Không tìm thấy vật phẩm!".into()); };
                let cost = u64::try_from(it.cost.max(0)).unwrap_or(0).saturating_mul(u64::from(count));
                let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
                if type_shop == 3 {
                    let spec_tid = if it.icon_spec == 4028 {
                        TemplateId(457)
                    } else {
                        self.shared.templates.items.iter().find(|r| r.icon_id == it.icon_spec)
                            .map(|r| TemplateId(r.id as i16))
                            .ok_or_else(|| "Vật phẩm đổi không hợp lệ!".to_owned())?
                    };
                    match spec_tid.0 { 77 => inv.sub_gem(c32).map_err(|e| format!("{e}"))?, 861 => inv.sub_ruby(c32).map_err(|e| format!("{e}"))?, _ => { inv.sub_template_from_bag(spec_tid, c32).map_err(|e| format!("{e}"))?; } }
                } else { match it.type_sell { 1 => inv.sub_gem(c32), 3 => inv.sub_ruby(c32), _ => inv.sub_gold(cost) }.map_err(|e| format!("{e}"))?; }
                if let Some(mins) = charm_mins {
                    if !entry.snapshot.charms.extend_minutes(item_id.0, mins * i64::from(count), unix_now_ms()) { return Err("Bùa không hợp lệ!".into()); }
                } else {
                    match item_id.0 {
                        517 => { inv.expand_slots(ContainerKind::Bag).map_err(|e| format!("{e}"))?; }
                        518 => { inv.expand_slots(ContainerKind::Box).map_err(|e| format!("{e}"))?; }
                        _ => { inv.add_to_bag(ItemInstance { template_id: item_id, quantity: count, options: it.options.clone(), create_time_ms: unix_now_ms() }, catalog.meta_of(item_id).stackable).map_err(|e| format!("{e}"))?; }
                    }
                }
                Ok(())
            })();
            if res.is_ok() { sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot); }
            let mut pkts = if charm_mins.is_some() { vec![wallet_money_pkt(&entry.snapshot.wallet)] } else { vec![s2c_packets::inventory_bag(&entry.snapshot.items_bag), s2c_packets::inventory_box(&entry.snapshot.items_box), wallet_money_pkt(&entry.snapshot.wallet)] };
            let msg = match &res { Ok(()) => "Mua vật phẩm thành công!".to_owned(), Err(e) => format!("Không thể mua: {e}") };
            if let Ok(tb) = s2c_packets::thong_bao(&msg) { pkts.push(tb); }
            let reopen = (res.is_ok() && charm_mins.is_some()).then(|| tag.map(|tg| (shop.map_or(21, |s| i16::from(s.npc_id)), tg))).flatten();
            (pkts, reopen)
        });
        if let Some((pkts, reopen)) = out { for p in pkts { session.send(p).await?; } if let Some((npc_id, tg)) = reopen { self.handle_open_shop(session, sid, npc_id, &tg).await?; } }
        Ok(())
    }

    pub(super) async fn handle_shop_sell(&self, session: &SessionHandle, sid: u64, action: i8, where_slot: i8, index: i16) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        let Ok(idx) = usize::try_from(index) else { return Ok(()); };
        let catalog = DbItemCatalog(&self.shared.templates);
        if action == 0 {
            if let Some(text) = self.shared.clients.authenticated.get(&sid).and_then(|entry| {
                let it = if where_slot == 0 { entry.snapshot.items_body.get(idx)?.as_ref()? } else { entry.snapshot.items_bag.get(idx)?.as_ref()? };
                let (qty, meta) = (it.quantity, catalog.meta_of(it.template_id));
                let price = if it.template_id.0 == 457 { u64::from(qty) * 37_000_000 } else { (meta.base_gold_value / 4).max(1) * u64::from(qty) };
                let name = self.shared.templates.items.iter().find(|r| r.id == i32::from(it.template_id.0)).map_or("vật phẩm", |r| r.name.as_str());
                Some(format!("Bạn có muốn bán {qty} {name} với giá {price} vàng không?"))
            }) { session.send(s2c_packets::item_sale_confirm(where_slot, index, &text)?).await?; }
            return Ok(());
        }
        let mut in_trade = false;
        if let Some((payout, bag_pkt, money_pkt, body_out)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if is_in_trade(entry.snapshot.id) { in_trade = true; return None; }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let payout = if where_slot == 1 {
                let qty = inv.bag_item(idx).ok()?.quantity;
                let mut ledger = buyback_ledgers().entry(entry.snapshot.id).or_default();
                ledger.sell_from_bag(&mut inv, idx, qty, &catalog).ok()?
            } else if where_slot == 0 {
                let item = inv.body.get_mut(idx)?.take()?;
                let p = (catalog.meta_of(item.template_id).base_gold_value / 4).max(1) * u64::from(item.quantity);
                inv.add_gold_clamped(p);
                p
            } else { return None; };
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            let body_out = (where_slot == 0).then(|| {
                super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                let (eh, eb, el, ebag, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
                let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                (s2c_packets::inventory_body(eh, &entry.snapshot.items_body), s2c_packets::me_load_point(&entry.snapshot.stats), s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))), s2c_packets::send_flag_bag(pid, ebag), loc)
            });
            Some((payout, s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet), body_out))
        }) {
            session.send(bag_pkt).await?; session.send(money_pkt).await?;
            if let Some((body_pkt, pt_pkt, upd_body, flag_bag, loc)) = body_out {
                session.send(body_pkt).await?; session.send(pt_pkt).await?;
                self.broadcast_to_zone_peers(loc, None, &upd_body); self.broadcast_to_zone_peers(loc, None, &flag_bag);
            }
            session.send(s2c_packets::thong_bao(&format!("Đã bán vật phẩm nhận {payout} vàng!"))?).await?;
        } else if in_trade { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        Ok(())
    }

    pub(super) async fn handle_open_consign(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some((pid, bag_items)) = self.consign_bag_snapshot(sid) else { return Ok(()); };
        open_shop_state().insert(pid, "CONSIGN".to_owned());
        let pkt = {
            let market = self.shared.consign_shop.lock();
            let raw: Vec<Vec<ConsignListing>> = CONSIGN_TABS.iter().map(|&(_, opt_t)| consign_listings_for(&market, pid, opt_t)).collect();
            let mut wire: Vec<Vec<ShopItemWire<'_>>> = raw.iter().map(|lst| lst.iter().map(|l| {
                let (gold, gem) = listing_prices(l);
                ShopItemWire { template_id: l.item.template_id.0, buy_coin_gold: gold, buy_gold_gem: gem, power_require: 0, icon_spec: 0, buy_spec: 0, reason: "", quantity: l.item.quantity as i32, consign: Some((l.id as i16, if l.sold { 2_i8 } else { 1_i8 }, i8::from(l.seller_id == pid), "")), options: l.item.options.as_slice(), is_new: false, part_temp: None }
            }).collect()).collect();
            if let Some(tab4) = wire.get_mut(4) {
                tab4.extend(bag_items.iter().map(|(slot, it)| ShopItemWire { template_id: it.template_id.0, buy_coin_gold: -1, buy_gold_gem: -1, power_require: 0, icon_spec: 0, buy_spec: 0, reason: "", quantity: it.quantity as i32, consign: Some((*slot as i16, 0, 0, "")), options: it.options.as_slice(), is_new: false, part_temp: None }));
            }
            let tabs: Vec<ShopTabWire<'_>> = CONSIGN_TABS.iter().zip(&wire).map(|(&(name, _), items)| ShopTabWire { name, max_page: u8::try_from(items.len().max(1).div_ceil(20).min(127)).unwrap_or(1), items: &items[..items.len().min(20)] }).collect();
            s2c_packets::shop_open(2, &tabs)?
        };
        session.send(pkt).await
    }

    pub(super) async fn handle_consign_action(&self, session: &SessionHandle, sid: u64, action: ConsignAction) -> Result<(), ProtocolError> {
        if self.is_sid_in_trade(sid) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        if let ConsignAction::OpenTab(tab, page) = action {
            let Some(&(_, opt_t)) = usize::try_from(tab).ok().and_then(|i| CONSIGN_TABS.get(i)) else { return Ok(()); };
            let Some((pid, bag_items)) = self.consign_bag_snapshot(sid) else { return Ok(()); };
            let pkt = {
                let market = self.shared.consign_shop.lock();
                let listings = consign_listings_for(&market, pid, opt_t);
                let mut items: Vec<ConsignItemWire<'_>> = listings.iter().map(|l| {
                    let (gold, gem) = listing_prices(l);
                    ConsignItemWire { template_id: l.item.template_id.0, listing_id: l.id as i16, buy_coin_gold: gold, buy_gold_gem: gem, buy_type: if l.sold { 2_i8 } else { 1_i8 }, quantity: l.item.quantity as i32, is_me: i8::from(l.seller_id == pid), options: l.item.options.as_slice(), part_temp: None, seller_name: "" }
                }).collect();
                if tab == 4 { items.extend(bag_items.iter().map(|(slot, it)| ConsignItemWire { template_id: it.template_id.0, listing_id: *slot as i16, buy_coin_gold: -1, buy_gold_gem: -1, buy_type: 0, quantity: it.quantity as i32, is_me: 0, options: it.options.as_slice(), part_temp: None, seller_name: "" })); }
                let max_page = items.len().max(1).div_ceil(20).min(127) as i8;
                let curr_page = page.clamp(0, (max_page - 1).max(0));
                let start = usize::try_from(curr_page).unwrap_or(0).saturating_mul(20).min(items.len());
                s2c_packets::consign_tab_page(tab, max_page, curr_page, &items[start..(start + 20).min(items.len())])?
            };
            return session.send(pkt).await;
        }
        let mut in_trade = false;
        if let Some((res, bag_pkt, money_pkt)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if is_in_trade(entry.snapshot.id) { in_trade = true; return None; }
            let (pid, pwr, mut inv) = (entry.snapshot.id, entry.snapshot.stats.power, snapshot_to_economy_inv(&entry.snapshot));
            let res = {
                let mut market = self.shared.consign_shop.lock();
                match action {
                    ConsignAction::Create(tid, mtype, money, qty) => {
                        let cur = if mtype == 1 { ConsignCurrency::Gem } else { ConsignCurrency::Gold };
                        let slot = usize::try_from(tid.0).ok().filter(|&s| inv.bag_item(s).is_ok()).unwrap_or(usize::MAX);
                        market.list_item(&mut inv, pid, slot, cur, u64::from(money.max(0).cast_unsigned()), qty.max(1).cast_unsigned()).map(|_| ())
                    }
                    ConsignAction::ClaimOrCancel(1, lid) => market.cancel_listing(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned())),
                    ConsignAction::ClaimOrCancel(2, lid) => market.claim_proceeds(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned())).map(|_| ()),
                    ConsignAction::Buy(lid, _, _) => market.buy_item(&mut inv, pid, pwr, u64::from(lid.0.max(0).cast_unsigned())),
                    ConsignAction::UpToTop(lid) => market.promote_to_top(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned())),
                    _ => Ok(()),
                }
            };
            if res.is_ok() { sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot); }
            Some((res, s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)))
        }) {
            session.send(bag_pkt).await?; session.send(money_pkt).await?;
            if let Err(err) = res { session.send(s2c_packets::thong_bao(&format!("Ký gửi: {err}"))?).await?; }
            self.handle_open_consign(session, sid).await?;
        } else if in_trade { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        Ok(())
    }

    fn consign_bag_snapshot(&self, sid: u64) -> Option<(PlayerId, Vec<(usize, ItemInstance)>)> {
        let catalog = DbItemCatalog(&self.shared.templates);
        self.shared.clients.authenticated.get(&sid).map(|e| (
            e.snapshot.id,
            e.snapshot.items_bag.iter().enumerate()
                .filter_map(|(i, s)| s.as_ref().filter(|it| is_consignable_bag_item(it, &catalog)).map(|it| (i, it.clone())))
                .collect(),
        ))
    }

    pub(super) async fn handle_combine_preview(&self, session: &SessionHandle, sid: u64, bag_indices: &[i8]) -> Result<(), ProtocolError> {
        let out = self.shared.clients.authenticated.get(&sid).map(|entry| {
            let pid = entry.snapshot.id;
            if is_in_trade(pid) { return Err("Không thể thao tác khi đang giao dịch!"); }
            let Some((npc_id, op, _)) = pending_combine().get(&pid).map(|v| v.value().clone()) else {
                return Err("Chưa mở giao diện kết hợp vật phẩm!");
            };
            let slots: Vec<usize> = bag_indices.iter().filter_map(|&i| usize::try_from(i).ok()).collect();
            let wearing_full_destroyer = entry.snapshot.items_body.len() >= 5
                && entry.snapshot.items_body[..5].iter().all(|s| s.as_ref().is_some_and(|it| (650..=662).contains(&it.template_id.0)));
            let special_tid = match entry.snapshot.gender { Gender::TraiDat => 24, Gender::Xayda => 25, Gender::Namec => 26 };
            let (skill_point, skill_mastery) = entry.snapshot.skills.iter()
                .find(|sk| sk.template_id == special_tid)
                .map_or((0_u8, 0_u16), |sk| (u8::try_from(sk.point.max(0)).unwrap_or(0), u16::try_from(sk.curr_level.max(0)).unwrap_or(0)));
            let ctx = CombineContext {
                gender: entry.snapshot.gender, power: entry.snapshot.stats.power,
                wearing_full_destroyer, skill_point, skill_mastery, parity: self.shared.config.parity_mode,
            };
            let res = CombineService::preview(&snapshot_to_economy_inv(&entry.snapshot), op, &slots, &ctx, &DbItemCatalog(&self.shared.templates));
            pending_combine().insert(pid, (npc_id, op, slots));
            let pkt = match res {
                Ok(q) => {
                    npc_menu_index().insert(pid, const_npc::MENU_START_COMBINE);
                    let btns: &[&str] = if op == CombineOp::PhaLeHoaTrangBi {
                        &["Nâng cấp", "Đập 10 lần", "Đập 100 lần", "Từ chối"]
                    } else {
                        &["Nâng cấp", "Từ chối"]
                    };
                    s2c_packets::npc_open_menu(
                        npc_id,
                        &format!("Tỉ lệ: {}.{:02}%\nVàng: {} | Ngọc: {}", q.ui_rate_bps / 100, q.ui_rate_bps % 100, q.gold_cost, q.gem_cost),
                        btns,
                        None,
                    ).ok()
                }
                Err(err) => s2c_packets::npc_open_say(npc_id, &format!("Nguyên liệu chưa hợp lệ: {err}"), None).ok(),
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

fn listing_prices(l: &ConsignListing) -> (i32, i32) { let p = i32::try_from(l.price).unwrap_or(i32::MAX); (if l.currency == ConsignCurrency::Gold { p } else { -1 }, if l.currency == ConsignCurrency::Gem { p } else { -1 }) }
fn consign_listings_for(market: &nro_economy::ConsignShop, pid: PlayerId, tab: Option<ConsignTab>) -> Vec<ConsignListing> { match tab { Some(t) => market.listings_in_tab(t), None => market.seller_listings(pid) }.into_iter().cloned().collect() }
fn is_consignable_bag_item(it: &ItemInstance, catalog: &dyn ItemCatalog) -> bool { !it.options.iter().any(|o| o.id == 30) && (it.options.iter().any(|o| matches!(o.id, 86 | 87)) || matches!(catalog.meta_of(it.template_id).item_type, 6 | 14 | 15) || (14..=20).contains(&it.template_id.0)) }
