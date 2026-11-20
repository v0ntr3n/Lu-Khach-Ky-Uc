//! Client input (`-125`), giftcodes, admin item grants, and player-to-player trade (`-86`).

use std::sync::OnceLock;

use dashmap::{DashMap, mapref::entry::Entry};
use nro_consts::cmd_message;
use nro_core::{ItemInstance, ItemOption, PlayerId, TemplateId};
use nro_db::world_repos::GiftCodeRepo;
use nro_economy::{GiftCodeDef, GiftCodeService, TradeError, TradeParticipant, TradeSession};
use nro_protocol::{Packet, ProtocolError, SessionHandle, TradeAction};
use nro_world::s2c_packets::{self, InputFieldWire};

use super::{DbItemCatalog, ServerSessionHandler, pending_input, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, trade_gold_offers, trade_partners, wallet_money_pkt};
use crate::runtime::unix_now_ms;

fn pending_trade_invites() -> &'static DashMap<(PlayerId, PlayerId), i64> { static M: OnceLock<DashMap<(PlayerId, PlayerId), i64>> = OnceLock::new(); M.get_or_init(DashMap::new) }
fn last_trade_ms() -> &'static DashMap<PlayerId, i64> { static M: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new(); M.get_or_init(DashMap::new) }
pub(super) fn trade_lifecycle_lock() -> &'static std::sync::Mutex<()> { static L: OnceLock<std::sync::Mutex<()>> = OnceLock::new(); L.get_or_init(|| std::sync::Mutex::new(())) }
pub(super) fn is_in_trade(pid: PlayerId) -> bool { trade_partners().contains_key(&pid) }

impl ServerSessionHandler {
    pub(super) async fn handle_open_input(&self, session: &SessionHandle, sid: u64, kind: &'static str) -> Result<(), ProtocolError> {
        if let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) { pending_input().insert(pid, kind); }
        let pkt = match kind {
            "GIFT_CODE" | "giftcode" => s2c_packets::client_input_open("Nhập mã quà tặng (Giftcode)", &[InputFieldWire { label: "Mã Giftcode", input_type: 1 }])?,
            "admin_give_item" | "ADMIN_GIVE_ITEM" => s2c_packets::client_input_open("Tặng vật phẩm cho người chơi", &[
                InputFieldWire { label: "Tên người chơi", input_type: 1 },
                InputFieldWire { label: "ID vật phẩm", input_type: 0 },
                InputFieldWire { label: "Số lượng", input_type: 0 },
            ])?,
            "admin_get_item" | "ADMIN_GET_ITEM" => s2c_packets::client_input_open("Nhận vật phẩm (Admin)", &[
                InputFieldWire { label: "ID vật phẩm", input_type: 0 },
                InputFieldWire { label: "Số lượng", input_type: 0 },
            ])?,
            _ => s2c_packets::client_input_open("Nhập thông tin", &[InputFieldWire { label: "Nội dung", input_type: 1 }])?,
        };
        session.send(pkt).await
    }

    pub(super) async fn handle_client_input(&self, session: &SessionHandle, sid: u64, fields: &[String]) -> Result<(), ProtocolError> {
        let Some((pid, is_admin)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.is_admin)) else { return Ok(()); };
        if is_in_trade(pid) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        let kind = pending_input().remove(&pid).map(|(_, k)| k);
        match kind {
            Some("GIFT_CODE" | "giftcode") => self.handle_giftcode_input(session, sid, fields).await,
            Some("admin_get_item" | "ADMIN_GET_ITEM") => {
                if !is_admin { return send_thong_bao(session, "Bạn không có quyền quản trị viên!"); }
                let Ok(tid) = fields.first().map_or("", |s| s.trim()).parse::<i16>() else { return send_thong_bao(session, "ID vật phẩm không hợp lệ!"); };
                let qty = fields.get(1).and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(1).max(1);
                self.admin_grant_item_to_sid(session, sid, sid, TemplateId(tid), qty).await
            }
            Some("admin_give_item" | "ADMIN_GIVE_ITEM") => {
                if !is_admin { return send_thong_bao(session, "Bạn không có quyền quản trị viên!"); }
                let target_name = fields.first().map_or("", |s| s.trim());
                let Ok(tid) = fields.get(1).map_or("", |s| s.trim()).parse::<i16>() else { return send_thong_bao(session, "ID vật phẩm không hợp lệ!"); };
                let qty = fields.get(2).and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(1).max(1);
                let target_sid = self.shared.clients.authenticated.iter()
                    .find(|e| e.snapshot.name.eq_ignore_ascii_case(target_name))
                    .map(|e| *e.key());
                let Some(tsid) = target_sid else { return send_thong_bao(session, "Không tìm thấy người chơi trực tuyến!"); };
                self.admin_grant_item_to_sid(session, sid, tsid, TemplateId(tid), qty).await
            }
            Some(other) => send_thong_bao(session, &format!("Loại biểu mẫu '{other}' chưa được hỗ trợ!")),
            None => send_thong_bao(session, "Không có yêu cầu nhập liệu nào đang chờ!"),
        }
    }

    async fn admin_grant_item_to_sid(&self, admin_session: &SessionHandle, admin_sid: u64, target_sid: u64, item_id: TemplateId, qty: u32) -> Result<(), ProtocolError> {
        let Some(tpl_it) = self.shared.templates.items.iter().find(|i| i.id == i32::from(item_id.0)) else {
            return send_thong_bao(admin_session, "ID vật phẩm không tồn tại!");
        };
        let res = self.shared.clients.authenticated.get_mut(&target_sid).map(|mut e| {
            if is_in_trade(e.snapshot.id) { return Err("Người chơi đang giao dịch!"); }
            let (mut inv, now) = (snapshot_to_economy_inv(&e.snapshot), unix_now_ms());
            let ok = if tpl_it.is_up_to_up {
                inv.add_to_bag(ItemInstance::with_options(item_id, qty, Vec::new(), now), true).is_ok()
            } else {
                (0..qty.min(20)).any(|_| inv.add_to_bag(ItemInstance::with_options(item_id, 1, Vec::new(), now), false).is_ok())
            };
            if !ok { return Err("Hành trang đã đầy!"); }
            sync_economy_inv_to_snapshot(&inv, &mut e.snapshot);
            let _ = e.handle.try_send(s2c_packets::inventory_bag(&e.snapshot.items_bag));
            if target_sid != admin_sid && let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn nhận được {} x{qty} từ Admin!", tpl_it.name)) {
                let _ = e.handle.try_send(tb);
            }
            Ok(e.snapshot.name.clone())
        });
        match res {
            Some(Ok(tname)) => send_thong_bao(admin_session, &format!("Đã trao {} x{qty} cho {tname}!", tpl_it.name)),
            Some(Err(msg)) => send_thong_bao(admin_session, msg),
            None => send_thong_bao(admin_session, "Không tìm thấy người chơi trực tuyến!"),
        }
    }

    async fn handle_giftcode_input(&self, session: &SessionHandle, sid: u64, fields: &[String]) -> Result<(), ProtocolError> {
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
                } else {
                    return send_thong_bao(session, "Người chơi không trực tuyến hoặc không ở cùng khu vực!");
                }
            }
            TradeAction::InviteOrAccept(1, Some(tpid)) if tpid != my_pid => {
                let (p1, p2, _) = trade_pair_key(my_pid, tpid);
                if is_in_trade(p1) || is_in_trade(p2) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
                let err_msg = {
                    let guard = trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let msg = if !pending_trade_invites().remove(&(tpid, my_pid)).is_some_and(|(_, exp)| exp >= now_ms) { None }
                    else if reserve_trade_pair(p1, p2) {
                        let fetch = |pid: PlayerId| self.shared.clients.players.get(&pid).and_then(|s| self.shared.clients.authenticated.get(s.value()).map(|e| (snapshot_to_economy_inv(&e.snapshot), e.handle.clone(), (e.snapshot.location.map_id, e.snapshot.location.zone_id), e.activated)));
                        let open_res = if let (Some((inv1, h1, z1, act1)), Some((inv2, h2, z2, act2))) = (fetch(p1), fetch(p2)) && z1 == z2 {
                            let lt1 = last_trade_ms().get(&p1).map_or(0, |v| *v.value());
                            let lt2 = last_trade_ms().get(&p2).map_or(0, |v| *v.value());
                            let (tp1, tp2) = (
                                TradeParticipant { player_id: p1, activated: act1, security_locked: false, last_trade_ms: lt1 },
                                TradeParticipant { player_id: p2, activated: act2, security_locked: false, last_trade_ms: lt2 },
                            );
                            match TradeSession::open(tp1, inv1, tp2, inv2, now_ms) {
                                Ok(ts) => {
                                    self.shared.active_trades.insert((p1, p2), ts); trade_gold_offers().insert((p1, p2), (0, 0));
                                    let _ = h1.try_send(s2c_packets::trade_invite_notify(1, p2)); let _ = h2.try_send(s2c_packets::trade_invite_notify(1, p1));
                                    Ok(())
                                }
                                Err(err) => Err(match err {
                                    TradeError::AccountNotActivated => "Tài khoản chưa kích hoạt thành viên, không thể giao dịch!".to_owned(),
                                    TradeError::SecurityLockActive => "Tài khoản đang bật bảo vệ, không thể giao dịch!".to_owned(),
                                    TradeError::CooldownActive(rem) => format!("Vui lòng chờ {} giây nữa mới có thể giao dịch tiếp!", ((rem + 999) / 1_000).max(1)),
                                    other => format!("Không thể giao dịch: {other}"),
                                }),
                            }
                        } else {
                            Err("Người chơi không trực tuyến hoặc không ở cùng khu vực!".to_owned())
                        };
                        if open_res.is_err() { trade_partners().remove_if(&p1, |_, p| *p == p2); trade_partners().remove_if(&p2, |_, p| *p == p1); }
                        open_res.err()
                    } else { Some("Không thể thao tác khi đang giao dịch!".to_owned()) };
                    drop(guard); msg
                };
                if let Some(msg) = err_msg { return send_thong_bao(session, &msg); }
            }
            TradeAction::AddItem(bag_idx, qty) => if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                let (mut rejected, mut timed_out, mut gold_err) = (false, false, None);
                if let Some(mut ts) = self.shared.active_trades.get_mut(&(k1, k2)) {
                    if bag_idx == -1 {
                        let g = u64::from(qty.max(0).cast_unsigned());
                        match ts.set_gold_offer(is_p1, g, now_ms) {
                            Ok(()) => if let Some(mut go) = trade_gold_offers().get_mut(&(k1, k2)) { if is_p1 { go.0 = g; } else { go.1 = g; } }
                            Err(TradeError::SessionTimedOut) => timed_out = true,
                            Err(TradeError::InvalidGoldOffer { offered, balance }) => {
                                gold_err = Some(format!("Số vàng giao dịch không hợp lệ ({offered}, hiện có {balance})!"));
                            }
                            Err(_) => {}
                        }
                    } else if let Ok(s) = usize::try_from(bag_idx) {
                        match ts.add_item_offer(is_p1, s, qty.max(0).cast_unsigned(), &DbItemCatalog(&self.shared.templates), now_ms) { Ok(()) => {} Err(TradeError::SessionTimedOut) => timed_out = true, Err(_) => rejected = true }
                    } else { rejected = true; }
                }
                if timed_out { self.cleanup_trade_on_disconnect(my_pid); }
                else if let Some(msg) = gold_err { send_thong_bao(session, &msg)?; }
                else if rejected { session.send(s2c_packets::trade_remove_rejected_item(bag_idx)).await?; session.send(s2c_packets::thong_bao("Vật phẩm này không thể giao dịch!")?).await?; }
            },
            TradeAction::RemoveItem(bag_idx) => if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                let timed_out = self.shared.active_trades.get_mut(&(k1, k2)).is_some_and(|mut ts| {
                    usize::try_from(bag_idx).is_ok_and(|slot| matches!(ts.remove_item_offer(is_p1, slot, now_ms), Err(TradeError::SessionTimedOut)))
                });
                if timed_out { self.cleanup_trade_on_disconnect(my_pid); }
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
                let (notify_msg, broadcast_msg) = {
                    let guard = trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let (mut notify, mut bcast) = (None, None);
                    if let Some(partner) = trade_partners().get(&my_pid).map(|v| *v.value()) {
                        let (k1, k2, is_p1) = trade_pair_key(my_pid, partner);
                        let acc = self.shared.active_trades.get_mut(&(k1, k2)).map(|mut ts| ts.accept(is_p1, &DbItemCatalog(&self.shared.templates), now_ms));
                        match acc {
                            Some(Ok(None)) => if let Some(ph) = self.handle_for_pid(partner) { let _ = ph.try_send(Packet::empty(cmd_message::TRADE_ACCEPT)); },
                            Some(Ok(Some((out1, out2)))) => if let Some((_, ts)) = self.shared.active_trades.remove(&(k1, k2)) {
                                trade_gold_offers().remove(&(k1, k2));
                                let is_online = |p: PlayerId| self.shared.clients.players.get(&p).is_some_and(|s| self.shared.clients.authenticated.contains_key(s.value()));
                                if is_online(k1) && is_online(k2) {
                                    self.apply_trade_inv(k1, &out1); self.apply_trade_inv(k2, &out2);
                                    last_trade_ms().insert(k1, now_ms); last_trade_ms().insert(k2, now_ms);
                                    bcast = Some((self.handle_for_pid(k1), self.handle_for_pid(k2), "Giao dịch thành công!".to_owned()));
                                } else {
                                    let (b1, b2) = ts.cancel(); self.apply_trade_inv(k1, &b1); self.apply_trade_inv(k2, &b2);
                                    bcast = Some((self.handle_for_pid(k1), self.handle_for_pid(k2), "Giao dịch bị hủy do người chơi mất kết nối!".to_owned()));
                                }
                                trade_partners().remove_if(&k1, |_, p| *p == k2); trade_partners().remove_if(&k2, |_, p| *p == k1);
                            }
                            Some(Err(TradeError::InvalidState(_))) => {
                                notify = Some("Đối phương chưa khóa giao dịch!".to_owned());
                            }
                            Some(Err(err)) => {
                                let (h1, h2) = (self.handle_for_pid(k1), self.handle_for_pid(k2));
                                self.cleanup_trade_on_disconnect_unlocked(my_pid);
                                let reason = match err {
                                    TradeError::SessionTimedOut => "Giao dịch bị hủy do quá thời gian!".to_owned(),
                                    TradeError::InsufficientRecipientBagSpace { .. } => "Giao dịch thất bại: hành trang đối phương không đủ ô trống!".to_owned(),
                                    TradeError::GoldCapOverflow => "Giao dịch thất bại: vượt quá giới hạn vàng!".to_owned(),
                                    other => format!("Giao dịch thất bại: {other}"),
                                };
                                bcast = Some((h1, h2, reason));
                            }
                            None => {}
                        }
                    }
                    drop(guard);
                    (notify, bcast)
                };
                if let Some(msg) = notify_msg { send_thong_bao(session, &msg)?; }
                if let Some((h1, h2, msg)) = broadcast_msg && let Ok(tb) = s2c_packets::thong_bao(&msg) {
                    if let Some(h) = h1 { let _ = h.try_send(tb.clone()); }
                    if let Some(h) = h2 { let _ = h.try_send(tb); }
                }
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
const fn trade_pair_key(a: PlayerId, b: PlayerId) -> (PlayerId, PlayerId, bool) { if a.0 <= b.0 { (a, b, true) } else { (b, a, false) } }
