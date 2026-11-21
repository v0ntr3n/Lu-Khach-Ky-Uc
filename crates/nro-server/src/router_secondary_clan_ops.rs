//! Clan messaging, pea donation, and join-request approval handlers.

use std::sync::{
    OnceLock,
    atomic::{AtomicI32, Ordering},
};

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{ClanId, PlayerId};
use nro_protocol::{ClanMessageAction, Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{
    ServerSessionHandler, router_secondary_clan::clan_op_lock, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot,
};
use crate::runtime::unix_now_ms;

const PEA_IDS: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 352, 523, 595];
const ASK_PEA_COOLDOWN_MS: i64 = 300_000;
const CLAN_REQUEST_TTL_MS: i64 = 15 * 60_000;
const MAX_JOIN_REQUESTS_PER_PLAYER: usize = 5;

#[inline]
const fn clan_msg_time_sec(now_ms: i64) -> i32 {
    ((now_ms / 1_000) - 1_000_000_000) as i32
}

#[derive(Clone)]
struct ClanPeaRequest {
    clan_id: ClanId,
    requester_id: PlayerId,
    requester_name: String,
    requester_role: i8,
    received: u8,
    max_donate: u8,
    created_sec: i32,
    created_ms: i64,
}

#[derive(Clone)]
struct ClanJoinRequest {
    clan_id: ClanId,
    applicant_id: PlayerId,
    applicant_name: String,
    created_ms: i64,
}

static CLAN_PEA_REQUESTS: OnceLock<DashMap<i32, ClanPeaRequest>> = OnceLock::new();
fn clan_pea_requests() -> &'static DashMap<i32, ClanPeaRequest> {
    CLAN_PEA_REQUESTS.get_or_init(DashMap::new)
}

static CLAN_JOIN_REQUESTS: OnceLock<DashMap<i32, ClanJoinRequest>> = OnceLock::new();
fn clan_join_requests() -> &'static DashMap<i32, ClanJoinRequest> {
    CLAN_JOIN_REQUESTS.get_or_init(DashMap::new)
}

static CLAN_MSG_SEQ: AtomicI32 = AtomicI32::new(1);

fn next_clan_msg_id() -> i32 {
    let id = CLAN_MSG_SEQ.fetch_add(1, Ordering::Relaxed);
    if id <= 0 {
        CLAN_MSG_SEQ.store(2, Ordering::Relaxed);
        1
    } else {
        id
    }
}

pub(super) fn clear_clan_join_requests(pid: PlayerId) {
    clan_join_requests().retain(|_, r| r.applicant_id != pid);
    clan_pea_requests().retain(|_, r| r.requester_id != pid);
}

pub(super) fn clear_clan_pea_requests_for_clan(cid: ClanId) {
    clan_pea_requests().retain(|_, r| r.clan_id != cid);
    clan_join_requests().retain(|_, r| r.clan_id != cid);
}

impl ServerSessionHandler {
    pub(super) async fn handle_clan_message(&self, session: &SessionHandle, sid: u64, act: &ClanMessageAction) -> Result<(), ProtocolError> {
        let Some((pid, name, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.clan_id)) else { return Ok(()); };
        match act {
            ClanMessageAction::Chat(text) if cid.0 >= 0 => {
                let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
                let role = clans.iter().find(|c| c.id == cid)
                    .and_then(|c| c.members.iter().find(|m| m.id == pid.0 as i32))
                    .map_or(2, |m| m.role);
                let mut w = PacketWriter::with_capacity(24 + name.len() + text.len());
                w.write_i8(0); w.write_i32(next_clan_msg_id()); w.write_i32(pid.0 as i32);
                w.write_utf(&name)?; w.write_i8(role); w.write_i32(clan_msg_time_sec(unix_now_ms())); w.write_utf(text)?; w.write_i8(0);
                let pkt = w.finish(cmd_message::CLAN_MESSAGE);
                for rec in &self.shared.clients.authenticated { if rec.value().snapshot.clan_id == cid { let _ = rec.value().handle.try_send(pkt.clone()); } }
            }
            ClanMessageAction::AskForPea if cid.0 >= 0 => {
                let guard = clan_op_lock().lock().await;
                let now_ms = unix_now_ms();
                let Ok(mut clans) = self.shared.clans_repo.load_all().await else {
                    drop(guard);
                    return session.send(s2c_packets::thong_bao("Không thể xin đậu lúc này!")?).await;
                };
                let Some(clan) = clans.iter_mut().find(|c| c.id == cid) else { return Ok(()); };
                let Some(member) = clan.members.iter_mut().find(|m| m.id == pid.0 as i32) else { return Ok(()); };
                if member.ask_pea_time > 0 && now_ms.saturating_sub(member.ask_pea_time) < ASK_PEA_COOLDOWN_MS {
                    let wait_sec = (ASK_PEA_COOLDOWN_MS - now_ms.saturating_sub(member.ask_pea_time)).max(1).cast_unsigned().div_ceil(1_000);
                    drop(guard);
                    return session.send(s2c_packets::thong_bao(&format!("Vui lòng đợi {wait_sec} giây nữa mới có thể xin đậu!"))?).await;
                }
                member.ask_pea_time = now_ms;
                let role = member.role;
                if self.shared.clans_repo.update_clan(clan).await.is_err() {
                    drop(guard);
                    return session.send(s2c_packets::thong_bao("Không thể gửi yêu cầu xin đậu lúc này!")?).await;
                }
                let msg_id = next_clan_msg_id();
                let req = ClanPeaRequest {
                    clan_id: cid,
                    requester_id: pid,
                    requester_name: name,
                    requester_role: role,
                    received: 0,
                    max_donate: 5,
                    created_sec: clan_msg_time_sec(now_ms),
                    created_ms: now_ms,
                };
                clan_pea_requests().retain(|_, r| r.requester_id != pid && now_ms.saturating_sub(r.created_ms) < CLAN_REQUEST_TTL_MS);
                clan_pea_requests().insert(msg_id, req.clone());
                drop(guard);
                let pkt = build_clan_pea_msg_pkt(msg_id, &req, 1)?;
                for rec in &self.shared.clients.authenticated {
                    if rec.value().snapshot.clan_id == cid {
                        let _ = rec.value().handle.try_send(pkt.clone());
                    }
                }
            }
            ClanMessageAction::AskForJoin(target_cid) => {
                if cid.0 >= 0 {
                    return session.send(s2c_packets::thong_bao("Bạn đã ở trong bang hội!")?).await;
                }
                let target = ClanId(*target_cid);
                let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
                let Some(clan) = clans.iter().find(|c| c.id == target) else {
                    return session.send(s2c_packets::thong_bao("Bang hội không tồn tại!")?).await;
                };
                if clan.members.len() >= usize::from(clan.max_member.max(10).cast_unsigned()) {
                    return session.send(s2c_packets::thong_bao("Bang hội đã đủ số lượng thành viên!")?).await;
                }
                let now_ms = unix_now_ms();
                let joins = clan_join_requests();
                joins.retain(|_, r| now_ms.saturating_sub(r.created_ms) < CLAN_REQUEST_TTL_MS && !(r.applicant_id == pid && r.clan_id == target));
                if joins.iter().filter(|r| r.applicant_id == pid).count() >= MAX_JOIN_REQUESTS_PER_PLAYER {
                    return session.send(s2c_packets::thong_bao("Bạn đã gửi tối đa 5 yêu cầu xin vào bang hội!")?).await;
                }
                let msg_id = next_clan_msg_id();
                let created_sec = clan_msg_time_sec(now_ms);
                joins.insert(msg_id, ClanJoinRequest { clan_id: target, applicant_id: pid, applicant_name: name.clone(), created_ms: now_ms });
                let mut w = PacketWriter::with_capacity(20 + name.len());
                w.write_i8(2);
                w.write_i32(msg_id);
                w.write_i32(pid.0 as i32);
                w.write_utf(&name)?;
                w.write_i8(2);
                w.write_i32(created_sec);
                let join_pkt = w.finish(cmd_message::CLAN_MESSAGE);
                let notice_pkt = s2c_packets::thong_bao(&format!("{name} xin gia nhập bang hội!"))?;
                let officer_ids: Vec<i32> = clan.members.iter().filter(|m| m.role <= 1).map(|m| m.id).collect();
                for rec in &self.shared.clients.authenticated {
                    if rec.value().snapshot.clan_id == target && officer_ids.contains(&(rec.value().snapshot.id.0 as i32)) {
                        let _ = rec.value().handle.try_send(join_pkt.clone());
                        let _ = rec.value().handle.try_send(notice_pkt.clone());
                    }
                }
                session.send(s2c_packets::thong_bao("Đã gửi yêu cầu xin vào bang hội!")?).await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) async fn handle_clan_donate(&self, session: &SessionHandle, sid: u64, msg_id: Option<i32>) -> Result<(), ProtocolError> {
        let guard = clan_op_lock().lock().await;
        let Some((donor_pid, donor_name, donor_cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.clan_id)) else { return Ok(()); };
        if donor_cid.0 < 0 { drop(guard); return session.send(s2c_packets::thong_bao("Bạn chưa gia nhập bang hội!")?).await; }
        let resolved_id = match msg_id {
            Some(id) => Some(id),
            None => clan_pea_requests().iter()
                .filter(|r| r.clan_id == donor_cid && r.requester_id != donor_pid && r.received < r.max_donate)
                .map(|r| *r.key())
                .max(),
        };
        let Some(target_msg_id) = resolved_id else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Không tìm thấy yêu cầu xin đậu hợp lệ!")?).await;
        };
        let Some(req_snap) = clan_pea_requests().get(&target_msg_id).map(|r| r.value().clone()) else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Yêu cầu xin đậu không tồn tại hoặc đã kết thúc!")?).await;
        };
        if req_snap.clan_id != donor_cid {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Yêu cầu xin đậu không thuộc bang hội của bạn!")?).await;
        }
        if req_snap.requester_id == donor_pid {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Không thể tự cho đậu bản thân!")?).await;
        }
        if req_snap.received >= req_snap.max_donate {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Yêu cầu xin đậu này đã nhận đủ!")?).await;
        }
        let Ok(mut clans) = self.shared.clans_repo.load_all().await else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Không thể cho đậu lúc này!")?).await;
        };
        let Some(clan) = clans.iter_mut().find(|c| c.id == donor_cid) else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Bang hội không tồn tại!")?).await;
        };
        if !clan.members.iter().any(|m| m.id == donor_pid.0 as i32) {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Bạn không còn trong bang hội!")?).await;
        }
        if !clan.members.iter().any(|m| m.id == req_snap.requester_id.0 as i32) {
            clan_pea_requests().remove(&target_msg_id);
            drop(guard);
            return session.send(s2c_packets::thong_bao("Người xin đậu không còn trong bang hội!")?).await;
        }
        let probe_pea = self.shared.clients.authenticated.get(&sid).map(|entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) {
                return Err("Không thể thao tác khi đang giao dịch!");
            }
            let Some(mut item) = entry.snapshot.items_bag.iter().flatten().find(|it| PEA_IDS.contains(&it.template_id.0)).cloned() else {
                return Err("Bạn không có đậu thần trong hành trang!");
            };
            item.quantity = 1;
            Ok(item)
        });
        let probe_item = match probe_pea {
            Some(Ok(it)) => it,
            Some(Err(msg)) => { drop(guard); return session.send(s2c_packets::thong_bao(msg)?).await; }
            None => return Ok(()),
        };
        let Some(recv_sid) = self.shared.clients.players.get(&req_snap.requester_id).map(|s| *s) else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Người xin đậu hiện đang ngoại tuyến!")?).await;
        };
        let recv_probe = self.shared.clients.authenticated.get(&recv_sid).map(|re| {
            if super::router_shop::is_in_trade(re.snapshot.id) {
                Err("Người nhận hiện đang giao dịch!")
            } else if snapshot_to_economy_inv(&re.snapshot).add_to_bag(probe_item, true).is_ok() {
                Ok(())
            } else {
                Err("Hành trang của người nhận đã đầy!")
            }
        });
        match recv_probe {
            Some(Ok(())) => {}
            Some(Err(msg)) => { drop(guard); return session.send(s2c_packets::thong_bao(msg)?).await; }
            None => { drop(guard); return session.send(s2c_packets::thong_bao("Người xin đậu hiện đang ngoại tuyến!")?).await; }
        }
        if let Some(d) = clan.members.iter_mut().find(|m| m.id == donor_pid.0 as i32) { d.donate = d.donate.saturating_add(1); }
        if let Some(r) = clan.members.iter_mut().find(|m| m.id == req_snap.requester_id.0 as i32) { r.receive_donate = r.receive_donate.saturating_add(1); }
        if self.shared.clans_repo.update_clan(clan).await.is_err() {
            drop(guard); return session.send(s2c_packets::thong_bao("Không thể cho đậu lúc này!")?).await;
        }
        let transfer = {
            let _trade_guard = super::router_shop::trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let deduct_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!"); }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let Some(slot) = inv.bag.iter().position(|s| s.as_ref().is_some_and(|it| PEA_IDS.contains(&it.template_id.0))) else { return Err("Bạn không có đậu thần trong hành trang!"); };
                let Ok(pea_item) = inv.sub_from_bag_slot(slot, 1) else { return Err("Không thể lấy đậu thần từ hành trang!"); };
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                Ok((pea_item, s2c_packets::inventory_bag(&entry.snapshot.items_bag)))
            });
            match deduct_res {
                Some(Ok((pea_item, donor_bag_pkt))) => {
                    let recv_notify = self.shared.clients.authenticated.get_mut(&recv_sid).and_then(|mut re| {
                        if super::router_shop::is_in_trade(re.snapshot.id) { return None; }
                        let mut r_inv = snapshot_to_economy_inv(&re.snapshot);
                        r_inv.add_to_bag(pea_item.clone(), true).ok()?;
                        sync_economy_inv_to_snapshot(&r_inv, &mut re.snapshot);
                        Some((re.handle.clone(), s2c_packets::inventory_bag(&re.snapshot.items_bag)))
                    });
                    if let Some((r_handle, r_bag)) = recv_notify {
                        Some(Ok((donor_bag_pkt, r_handle, r_bag)))
                    } else {
                        if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                            let _ = inv.add_to_bag(pea_item, true);
                            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        }
                        Some(Err("Không thể trao đậu thần cho người nhận, đã hoàn lại vào hành trang!"))
                    }
                }
                Some(Err(msg)) => Some(Err(msg)),
                None => None,
            }
        };
        let (donor_bag_pkt, r_handle, r_bag) = match transfer {
            Some(Ok(v)) => v,
            other => {
                if let Some(d) = clan.members.iter_mut().find(|m| m.id == donor_pid.0 as i32) { d.donate = d.donate.saturating_sub(1); }
                if let Some(r) = clan.members.iter_mut().find(|m| m.id == req_snap.requester_id.0 as i32) { r.receive_donate = r.receive_donate.saturating_sub(1); }
                let _ = self.shared.clans_repo.update_clan(clan).await;
                drop(guard);
                if let Some(Err(msg)) = other { session.send(s2c_packets::thong_bao(msg)?).await?; }
                return Ok(());
            }
        };
        let updated_req = if let Some(mut r) = clan_pea_requests().get_mut(&target_msg_id) {
            r.received = r.received.saturating_add(1);
            r.value().clone()
        } else {
            let mut r = req_snap;
            r.received = r.received.saturating_add(1);
            r
        };
        if updated_req.received >= updated_req.max_donate {
            clan_pea_requests().remove(&target_msg_id);
        }
        drop(guard);
        let msg_pkt = build_clan_pea_msg_pkt(target_msg_id, &updated_req, 0)?;
        for rec in &self.shared.clients.authenticated {
            if rec.value().snapshot.clan_id == donor_cid {
                let _ = rec.value().handle.try_send(msg_pkt.clone());
            }
        }
        let _ = r_handle.try_send(r_bag);
        if let Ok(tb) = s2c_packets::thong_bao(&format!("Bạn vừa nhận được 1 đậu thần từ {donor_name}!")) {
            let _ = r_handle.try_send(tb);
        }
        session.send(donor_bag_pkt).await?;
        session.send(s2c_packets::thong_bao(&format!("Bạn đã cho {} 1 đậu thần!", updated_req.requester_name))?).await
    }

    pub(super) async fn handle_clan_join_decision(&self, session: &SessionHandle, sid: u64, msg_id: i32, decision: i8) -> Result<(), ProtocolError> {
        let guard = clan_op_lock().lock().await;
        let Some((caller_pid, caller_cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else { return Ok(()); };
        if caller_cid.0 < 0 { return Ok(()); }
        let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
        if !clans.iter().find(|c| c.id == caller_cid).is_some_and(|c| c.members.iter().any(|m| m.id == caller_pid.0 as i32 && m.role <= 1)) {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Chỉ bang chủ hoặc phó bang mới có quyền duyệt thành viên!")?).await;
        }
        let Some((_, req)) = clan_join_requests().remove_if(&msg_id, |_, r| r.clan_id == caller_cid) else {
            drop(guard);
            return session.send(s2c_packets::thong_bao("Yêu cầu xin vào bang không tồn tại hoặc đã được xử lý!")?).await;
        };
        drop(guard);
        if decision == 0 {
            let applicant = self.shared.clients.players.get(&req.applicant_id)
                .map(|s| *s)
                .and_then(|asid| self.shared.clients.authenticated.get(&asid).map(|e| (asid, e.handle.clone())));
            if let Some((asid, a_handle)) = applicant {
                if self.join_clan_by_id(&a_handle, asid, caller_cid.0).await? {
                    if let Ok(tb) = s2c_packets::thong_bao("Yêu cầu gia nhập bang hội của bạn đã được chấp nhận!") {
                        let _ = a_handle.try_send(tb);
                    }
                    session.send(s2c_packets::thong_bao(&format!("Đã duyệt {} vào bang hội!", req.applicant_name))?).await
                } else {
                    session.send(s2c_packets::thong_bao("Không thể duyệt người chơi vào bang hội!")?).await
                }
            } else {
                session.send(s2c_packets::thong_bao("Người chơi đã ngoại tuyến, không thể duyệt vào bang lúc này!")?).await
            }
        } else {
            if let Some(a_handle) = self.handle_for_pid(req.applicant_id)
                && let Ok(tb) = s2c_packets::thong_bao("Yêu cầu gia nhập bang hội của bạn đã bị từ chối!")
            {
                let _ = a_handle.try_send(tb);
            }
            session.send(s2c_packets::thong_bao(&format!("Đã từ chối yêu cầu vào bang của {}!", req.applicant_name))?).await
        }
    }
}

fn build_clan_pea_msg_pkt(msg_id: i32, req: &ClanPeaRequest, is_new: i8) -> Result<Packet, ProtocolError> {
    let mut w = PacketWriter::with_capacity(24 + req.requester_name.len());
    w.write_i8(1);
    w.write_i32(msg_id);
    w.write_i32(req.requester_id.0 as i32);
    w.write_utf(&req.requester_name)?;
    w.write_i8(req.requester_role);
    w.write_i32(req.created_sec);
    w.write_i8(req.received as i8);
    w.write_i8(req.max_donate as i8);
    w.write_i8(is_new);
    Ok(w.finish(cmd_message::CLAN_MESSAGE))
}
