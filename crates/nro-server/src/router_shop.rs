//! Shops (`-44`, `6`, `7`), consignment (`-100`), giftcode input (`-125`), combine preview (`-81`), and trade (`-86`).

use std::sync::OnceLock;

use dashmap::{DashMap, mapref::entry::Entry};
use nro_consts::{cmd_message, const_npc};
use nro_core::{ItemInstance, ItemOption, PlayerId, TemplateId};
use nro_db::world_repos::GiftCodeRepo;
use nro_economy::{BuyBackLedger, CombineContext, CombineOp, CombineService, ConsignCurrency, ConsignListing, ConsignTab, ContainerKind, GiftCodeDef, GiftCodeService, ItemCatalog, ItemMeta, TradeError, TradeParticipant, TradeSession};
use nro_protocol::{ConsignAction, Packet, ProtocolError, SessionHandle, TradeAction};
use nro_world::{ZoneCommand, s2c_packets::{self, ConsignItemWire, InputFieldWire, ShopItemWire, ShopTabWire}};

use super::{DbItemCatalog, ServerSessionHandler, effective_appearance, npc_menu_index, open_shop_state, pending_combine, pending_input, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, trade_gold_offers, trade_partners, wallet_money_pkt};
use crate::runtime::unix_now_ms;

const CONSIGN_TABS: [(&str, Option<ConsignTab>); 5] = [
    ("Trang bị\nÁo Quần", Some(ConsignTab::AoQuan)), ("Phụ kiện\nGăng Tay", Some(ConsignTab::GangTay)),
    ("Phụ kiện\nĐặc biệt", Some(ConsignTab::PhuKien)), ("Linh tinh", Some(ConsignTab::LinhTinh)), ("Quản lý\nKý gửi", None),
];

fn pending_trade_invites() -> &'static DashMap<(PlayerId, PlayerId), i64> { static M: OnceLock<DashMap<(PlayerId, PlayerId), i64>> = OnceLock::new(); M.get_or_init(DashMap::new) }
pub(super) fn trade_lifecycle_lock() -> &'static std::sync::Mutex<()> { static L: OnceLock<std::sync::Mutex<()>> = OnceLock::new(); L.get_or_init(|| std::sync::Mutex::new(())) }
pub(super) fn is_in_trade(pid: PlayerId) -> bool { trade_partners().contains_key(&pid) }

impl ServerSessionHandler {
    fn is_sid_in_trade(&self, sid: u64) -> bool { self.shared.clients.authenticated.get(&sid).is_some_and(|e| is_in_trade(e.snapshot.id)) }

    pub(super) async fn handle_open_shop(&self, session: &SessionHandle, sid: u64, npc_id: i16, tag: &str) -> Result<(), ProtocolError> {
        let Some((pid, gender, charms)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.gender, e.snapshot.charms)) else { return Ok(()); };
        let Some(shop) = self.shared.templates.shops.iter().find(|s| s.tag_name.as_deref() == Some(tag)).or_else(|| self.shared.templates.shops.iter().find(|s| i16::from(s.npc_id) == npc_id))
        else { return session.send(s2c_packets::npc_open_say(npc_id, &format!("Cửa hàng {tag} đang mở!"), None)?).await; };
        let resolved = shop.tag_name.clone().unwrap_or_else(|| tag.to_owned());
        let is_charm = matches!(resolved.as_str(), "BUA_1H" | "BUA_8H" | "BUA_1M");
        open_shop_state().insert(pid, resolved);
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
            let charm_mins = tag.as_deref().and_then(|tg| match tg { "BUA_1H" => Some(60_i64), "BUA_8H" => Some(480), "BUA_1M" => Some(43_200), _ => None });
            let rec = shop.and_then(|s| s.tabs.iter().flat_map(|t| &t.items).find(|i| i.temp_id == item_id));
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let res: Result<(), String> = (|| {
                let Some(it) = rec else { return Err("Không tìm thấy vật phẩm!".into()); };
                let cost = u64::try_from(it.cost.max(0)).unwrap_or(0).saturating_mul(u64::from(count));
                let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
                if type_shop == 3 {
                    let spec_tid = if it.icon_spec == 4028 { TemplateId(457) } else { self.shared.templates.items.iter().find(|r| r.icon_id == it.icon_spec).map_or(TemplateId(457), |r| TemplateId(r.id as i16)) };
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
            (pkts, (res.is_ok() && charm_mins.is_some()).then_some(tag).flatten())
        });
        if let Some((pkts, reopen)) = out { for p in pkts { session.send(p).await?; } if let Some(tg) = reopen { self.handle_open_shop(session, sid, 21, &tg).await?; } }
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
            let (mut inv, mut ledger) = (snapshot_to_economy_inv(&entry.snapshot), BuyBackLedger::default());
            let payout = if where_slot == 1 { let qty = inv.bag_item(idx).ok()?.quantity; ledger.sell_from_bag(&mut inv, idx, qty, &catalog).ok()? }
                else if where_slot == 0 { let item = inv.body.get_mut(idx)?.take()?; let p = (catalog.meta_of(item.template_id).base_gold_value / 4).max(1) * u64::from(item.quantity); inv.add_gold_clamped(p); p }
                else { return None; };
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
                        let slot = usize::try_from(tid.0).ok().filter(|&s| inv.bag_item(s).is_ok()).or_else(|| inv.find_bag_slot(tid)).unwrap_or(usize::MAX);
                        market.list_item(&mut inv, pid, slot, cur, u64::from(money.max(0).cast_unsigned()), qty.max(1).cast_unsigned()).map(|_| ())
                    }
                    ConsignAction::ClaimOrCancel(1, lid) => market.cancel_listing(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned())),
                    ConsignAction::ClaimOrCancel(_, lid) => market.claim_proceeds(&mut inv, pid, u64::from(lid.0.max(0).cast_unsigned())).map(|_| ()),
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

    fn consign_bag_snapshot(&self, sid: u64) -> Option<(PlayerId, Vec<(usize, ItemInstance)>)> { self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.items_bag.iter().enumerate().filter_map(|(i, s)| s.as_ref().filter(|it| is_consignable_bag_item(it)).map(|it| (i, it.clone()))).collect())) }

    pub(super) async fn handle_open_input(&self, session: &SessionHandle, sid: u64, kind: &'static str) -> Result<(), ProtocolError> {
        if let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) { pending_input().insert(pid, kind); }
        session.send(s2c_packets::client_input_open("Nhập mã quà tặng (Giftcode)", &[InputFieldWire { label: "Mã Giftcode", input_type: 1 }])?).await
    }

    pub(super) async fn handle_client_input(&self, session: &SessionHandle, sid: u64, fields: &[String]) -> Result<(), ProtocolError> {
        let Some(code) = fields.first().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else { return send_thong_bao(session, "Vui lòng nhập nội dung hợp lệ!"); };
        let repo = GiftCodeRepo::new(self.shared.pool.clone());
        let Some(rec) = repo.load_all().await.unwrap_or_default().into_iter().find(|r| r.code == code) else { return send_thong_bao(session, "Mã Giftcode không tồn tại!"); };
        let (rec_id, now_ms) = (rec.id, unix_now_ms());
        let count_left = if rec.count_left < 0 { u32::MAX } else { rec.count_left.cast_unsigned() };
        let mut def = GiftCodeDef { code: rec.code, count_left, expires_at_ms: rec.expired.timestamp_millis(), items: rec.items.iter().map(|gi| (gi.id as i16, gi.quantity.max(1).cast_unsigned(), gi.options.iter().map(|o| ItemOption { id: o.id.max(0) as u16, param: o.param }).collect())).collect(), redeemed_by: Vec::new() };
        let need_slots = def.items.iter().filter(|&&(id, _, _)| id >= 0).count();
        let pre = self.shared.clients.authenticated.get(&sid).map(|entry| {
            let pid = entry.snapshot.id;
            if is_in_trade(pid) { return Err("Không thể thao tác khi đang giao dịch!"); }
            pending_input().remove(&pid);
            if entry.snapshot.redeemed_giftcodes.contains(&code) { return Err("Bạn đã sử dụng mã Giftcode này rồi!"); }
            if now_ms > def.expires_at_ms || def.count_left == 0 { return Err("Mã quà tặng đã hết hạn hoặc hết lượt sử dụng!"); }
            if entry.snapshot.items_bag.iter().filter(|s| s.is_none()).count() < need_slots { return Err("Hành trang không đủ ô trống!"); }
            Ok(())
        });
        match pre { Some(Ok(())) => {} Some(Err(msg)) => return send_thong_bao(session, msg), None => return Ok(()) }
        if !repo.redeem_decrement(rec_id).await.unwrap_or(false) { return send_thong_bao(session, "Mã quà tặng đã hết lượt sử dụng!"); }
        match self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let pid = entry.snapshot.id;
            if is_in_trade(pid) { return Err("Không thể thao tác khi đang giao dịch!".to_owned()); }
            if entry.snapshot.redeemed_giftcodes.contains(&code) { return Err("Bạn đã sử dụng mã Giftcode này rồi!".to_owned()); }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            GiftCodeService::redeem(&mut def, &mut inv, pid, now_ms).map_err(|e| format!("{e}"))?;
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            entry.snapshot.redeemed_giftcodes.push(code);
            Ok((s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)))
        }) {
            Some(Ok((bag_pkt, money_pkt))) => { session.send(bag_pkt).await?; session.send(money_pkt).await?; send_thong_bao(session, "Nhận quà Giftcode thành công!")?; }
            Some(Err(msg)) => send_thong_bao(session, &msg)?, None => {}
        }
        Ok(())
    }

    pub(super) async fn handle_combine_preview(&self, session: &SessionHandle, sid: u64, bag_indices: &[i8]) -> Result<(), ProtocolError> {
        if let Some(p) = self.shared.clients.authenticated.get(&sid).and_then(|entry| {
            let pid = entry.snapshot.id;
            let (npc_id, op, _) = pending_combine().get(&pid).map_or((21_i16, CombineOp::PhaLeHoaTrangBi, Vec::new()), |v| v.value().clone());
            let slots: Vec<usize> = bag_indices.iter().filter_map(|&i| usize::try_from(i).ok()).collect();
            let ctx = CombineContext { gender: entry.snapshot.gender, power: entry.snapshot.stats.power, wearing_full_destroyer: false, skill_point: 0, skill_mastery: 0, parity: self.shared.config.parity_mode };
            let res = CombineService::preview(&snapshot_to_economy_inv(&entry.snapshot), op, &slots, &ctx, &DbItemCatalog(&self.shared.templates));
            pending_combine().insert(pid, (npc_id, op, slots));
            match res {
                Ok(q) => { npc_menu_index().insert(pid, const_npc::MENU_START_COMBINE); s2c_packets::npc_open_menu(npc_id, &format!("Tỉ lệ: {}.{}%\nVàng: {} | Ngọc: {}", q.ui_rate_bps / 100, q.ui_rate_bps % 100, q.gold_cost, q.gem_cost), &["Nâng cấp", "Từ chối"], None).ok() }
                Err(err) => s2c_packets::npc_open_say(npc_id, &format!("Nguyên liệu chưa hợp lệ: {err}"), None).ok(),
            }
        }) { session.send(p).await?; }
        Ok(())
    }

    pub(super) async fn handle_trade(&self, session: &SessionHandle, sid: u64, action: TradeAction) -> Result<(), ProtocolError> {
        let Some((my_pid, my_zone)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, (e.snapshot.location.map_id, e.snapshot.location.zone_id))) else { return Ok(()); };
        let now_ms = unix_now_ms();
        match action {
            TradeAction::InviteOrAccept(0, Some(tpid)) if tpid != my_pid => {
                if is_in_trade(my_pid) || is_in_trade(tpid) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
                let target = self.shared.clients.players.get(&tpid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| ((e.snapshot.location.map_id, e.snapshot.location.zone_id), e.handle.clone())));
                if let Some((tz, th)) = target && tz == my_zone {
                    pending_trade_invites().insert((my_pid, tpid), now_ms + 30_000);
                    let _ = th.try_send(s2c_packets::trade_invite_notify(0, my_pid));
                }
            }
            TradeAction::InviteOrAccept(1, Some(tpid)) if tpid != my_pid => {
                let (p1, p2, _) = trade_pair_key(my_pid, tpid);
                if is_in_trade(p1) || is_in_trade(p2) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
                let busy = {
                    let guard = trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let is_busy = if !pending_trade_invites().remove(&(tpid, my_pid)).is_some_and(|(_, exp)| exp >= now_ms) { false }
                    else if reserve_trade_pair(p1, p2) {
                        let fetch = |pid: PlayerId| self.shared.clients.players.get(&pid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| (snapshot_to_economy_inv(&e.snapshot), e.handle.clone(), (e.snapshot.location.map_id, e.snapshot.location.zone_id))));
                        let opened = if let (Some((inv1, h1, z1)), Some((inv2, h2, z2))) = (fetch(p1), fetch(p2)) && z1 == z2 {
                            let (tp1, tp2) = (TradeParticipant { player_id: p1, activated: true, security_locked: false, last_trade_ms: 0 }, TradeParticipant { player_id: p2, activated: true, security_locked: false, last_trade_ms: 0 });
                            if let Ok(ts) = TradeSession::open(tp1, inv1, tp2, inv2, now_ms) {
                                self.shared.active_trades.insert((p1, p2), ts); trade_gold_offers().insert((p1, p2), (0, 0));
                                let _ = h1.try_send(s2c_packets::trade_invite_notify(1, p2)); let _ = h2.try_send(s2c_packets::trade_invite_notify(1, p1)); true
                            } else { false }
                        } else { false };
                        if !opened { trade_partners().remove_if(&p1, |_, p| *p == p2); trade_partners().remove_if(&p2, |_, p| *p == p1); }
                        false
                    } else { true };
                    drop(guard); is_busy
                };
                if busy { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
            }
            TradeAction::AddItem(bag_idx, qty) => if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                let (mut rejected, mut timed_out) = (false, false);
                if let Some(mut ts) = self.shared.active_trades.get_mut(&(k1, k2)) {
                    if bag_idx == -1 {
                        let g = u64::from(qty.max(0).cast_unsigned());
                        match ts.set_gold_offer(is_p1, g, now_ms) { Ok(()) => if let Some(mut go) = trade_gold_offers().get_mut(&(k1, k2)) { if is_p1 { go.0 = g; } else { go.1 = g; } } Err(TradeError::SessionTimedOut) => timed_out = true, Err(_) => {} }
                    } else if let Ok(s) = usize::try_from(bag_idx) {
                        match ts.add_item_offer(is_p1, s, qty.max(0).cast_unsigned(), &DbItemCatalog(&self.shared.templates), now_ms) { Ok(()) => {} Err(TradeError::SessionTimedOut) => timed_out = true, Err(_) => rejected = true }
                    } else { rejected = true; }
                }
                if timed_out { self.cleanup_trade_on_disconnect(my_pid); }
                else if rejected { session.send(s2c_packets::trade_remove_rejected_item(bag_idx)).await?; session.send(s2c_packets::thong_bao("Vật phẩm này không thể giao dịch!")?).await?; }
            },
            TradeAction::RemoveItem(bag_idx) => if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                if let Some(mut ts) = self.shared.active_trades.get_mut(&(k1, k2)) && let Ok(slot) = usize::try_from(bag_idx) { let _ = ts.remove_item_offer(is_p1, slot); }
            },
            TradeAction::StateChange(5) => if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                let lock_res = self.shared.active_trades.get_mut(&(k1, k2)).map(|mut ts| ts.lock(is_p1, now_ms).map(|_| {
                    let ((o1, o2), (g1, g2)) = (ts.escrow_offers(), trade_gold_offers().get(&(k1, k2)).map_or((0, 0), |v| *v.value()));
                    if is_p1 { s2c_packets::trade_lock_offer(g1, o1) } else { s2c_packets::trade_lock_offer(g2, o2) }
                }));
                match lock_res {
                    Some(Ok(pkt)) => if let Some(ph) = self.handle_for_pid(partner) { let _ = ph.try_send(pkt); }
                    Some(Err(TradeError::SessionTimedOut)) => self.cleanup_trade_on_disconnect(my_pid), _ => {}
                }
            },
            TradeAction::StateChange(7) => {
                let guard = trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                    let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                    let acc = self.shared.active_trades.get_mut(&(k1, k2)).map(|mut ts| ts.accept(is_p1, &DbItemCatalog(&self.shared.templates), now_ms));
                    match acc {
                        Some(Ok(None)) => if let Some(ph) = self.handle_for_pid(partner) { let _ = ph.try_send(Packet::empty(cmd_message::TRADE_ACCEPT)); },
                        Some(Ok(Some((out1, out2)))) => if let Some((_, ts)) = self.shared.active_trades.remove(&(k1, k2)) {
                            trade_gold_offers().remove(&(k1, k2));
                            let is_online = |p: PlayerId| self.shared.clients.players.get(&p).is_some_and(|s| self.shared.clients.authenticated.contains_key(s.value()));
                            if is_online(k1) && is_online(k2) { self.apply_trade_inv(k1, &out1); self.apply_trade_inv(k2, &out2); }
                            else { let (b1, b2) = ts.cancel(); self.apply_trade_inv(k1, &b1); self.apply_trade_inv(k2, &b2); }
                            trade_partners().remove_if(&k1, |_, p| *p == k2); trade_partners().remove_if(&k2, |_, p| *p == k1);
                        }
                        Some(Err(_)) => self.cleanup_trade_on_disconnect_unlocked(my_pid), None => {}
                    }
                }
                drop(guard);
            }
            TradeAction::StateChange(3) => self.cleanup_trade_on_disconnect(my_pid),
            _ => {}
        }
        Ok(())
    }

    pub(super) fn cleanup_trade_on_disconnect_unlocked(&self, pid: PlayerId) {
        pending_trade_invites().retain(|k, _| k.0 != pid && k.1 != pid);
        let Some(partner) = trade_partners().get(&pid).map(|v| *v.value()) else { return; };
        let (k1, k2, _) = trade_pair_key(pid, partner);
        trade_gold_offers().remove(&(k1, k2));
        if let Some((_, ts)) = self.shared.active_trades.remove(&(k1, k2)) {
            let (b1, b2) = ts.cancel(); self.apply_trade_inv(k1, &b1); self.apply_trade_inv(k2, &b2);
        } else { for p in [pid, partner] { if let Some(h) = self.handle_for_pid(p) { let _ = h.try_send(s2c_packets::trade_close()); } } }
        trade_partners().remove_if(&k1, |_, p| *p == k2); trade_partners().remove_if(&k2, |_, p| *p == k1);
    }

    pub(super) fn cleanup_trade_on_disconnect(&self, pid: PlayerId) {
        let guard = trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        self.cleanup_trade_on_disconnect_unlocked(pid); drop(guard);
    }

    fn apply_trade_inv(&self, pid: PlayerId, inv: &nro_economy::Inventory) {
        let Some(sid) = self.shared.clients.players.get(&pid).map(|s| *s.value()) else { return; };
        if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
            sync_economy_inv_to_snapshot(inv, &mut entry.snapshot);
            for p in [s2c_packets::trade_close(), s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)] { let _ = entry.handle.try_send(p); }
        }
    }

    pub(super) fn handle_for_pid(&self, pid: PlayerId) -> Option<SessionHandle> { self.shared.clients.players.get(&pid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| e.handle.clone())) }
}

fn reserve_trade_pair(lo: PlayerId, hi: PlayerId) -> bool {
    let Entry::Vacant(v) = trade_partners().entry(lo) else { return false; }; drop(v.insert(hi));
    let ok = match trade_partners().entry(hi) { Entry::Vacant(v2) => { drop(v2.insert(lo)); true } Entry::Occupied(_) => false };
    if !ok { trade_partners().remove_if(&lo, |_, p| *p == hi); }
    ok
}
fn listing_prices(l: &ConsignListing) -> (i32, i32) { let p = i32::try_from(l.price).unwrap_or(i32::MAX); (if l.currency == ConsignCurrency::Gold { p } else { -1 }, if l.currency == ConsignCurrency::Gem { p } else { -1 }) }
fn consign_listings_for(market: &nro_economy::ConsignShop, pid: PlayerId, tab: Option<ConsignTab>) -> Vec<ConsignListing> { match tab { Some(t) => market.listings_in_tab(t), None => market.seller_listings(pid) }.into_iter().cloned().collect() }
fn is_consignable_bag_item(it: &ItemInstance) -> bool { !it.options.iter().any(|o| o.id == 30) && (it.options.iter().any(|o| matches!(o.id, 86 | 87)) || matches!(ItemMeta::classify_builtin(it.template_id).item_type, 6 | 14 | 15) || (14..=20).contains(&it.template_id.0)) }
const fn trade_pair_key(a: PlayerId, b: PlayerId) -> (PlayerId, PlayerId, bool) { if a.0 <= b.0 { (a, b, true) } else { (b, a, false) } }
