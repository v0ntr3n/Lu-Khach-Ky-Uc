//! Clan management handlers: search, create/update, info, invite, join, leave, and remote role management.

use std::sync::OnceLock;

use chrono::Utc;
use dashmap::DashMap;
use nro_consts::{cmd_message, const_npc};
use nro_core::{ClanId, PlayerId};
use nro_db::{
    UnquotedLeaderInfo,
    player_repo::PlayerSnapshot,
    world_repos::{ClanMemberRecord, ClanRecord},
};
use nro_protocol::{
    ClanInfoAction, ClanInviteAction, Packet, PacketWriter, ProtocolError, SessionHandle,
};
use nro_world::s2c_packets;
use tokio::sync::Mutex;

use super::{
    ServerSessionHandler, con_meo_state, effective_appearance,
    router_secondary::clear_clan_invites, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static PENDING_CLAN_INVITES: OnceLock<DashMap<(PlayerId, ClanId), i64>> = OnceLock::new();
fn pending_clan_invites() -> &'static DashMap<(PlayerId, ClanId), i64> {
    PENDING_CLAN_INVITES.get_or_init(DashMap::new)
}

static CLAN_OP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
pub(super) fn clan_op_lock() -> &'static Mutex<()> {
    CLAN_OP_LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn clear_pending_clan_invites(pid: PlayerId) {
    pending_clan_invites().retain(|k, _| k.0 != pid);
}

impl ServerSessionHandler {
    pub(super) fn broadcast_clan_info(&self, clan: &ClanRecord) {
        for rec in &self.shared.clients.authenticated {
            if rec.value().snapshot.clan_id == clan.id
                && let Ok(pkt) = build_clan_info_pkt(Some(clan), rec.value().snapshot.id)
            {
                let _ = rec.value().handle.try_send(pkt);
            }
        }
    }

    pub(super) async fn handle_clan_create_info(&self, session: &SessionHandle, sid: u64, action: &ClanInfoAction) -> Result<(), ProtocolError> {
        match action {
            &ClanInfoAction::RequestEmblems(act) => {
                let cnt = self.shared.templates.flag_bags.len().min(60);
                let mut w = PacketWriter::with_capacity(4 + cnt * 20); w.write_i8(act); w.write_i16(cnt as i16);
                for (id, name, gold, gem, _, _) in self.shared.templates.flag_bags.iter().take(60) { w.write_i16(*id as i16); w.write_utf(name)?; w.write_i32(*gold); w.write_i32(*gem); }
                session.send(w.finish(cmd_message::CLAN_CREATE_INFO)).await?;
            }
            ClanInfoAction::Create(img_id, name) => {
                let trimmed = name.trim();
                if !(3..=30).contains(&trimmed.chars().count()) { return session.send(s2c_packets::thong_bao("Tên bang hội phải từ 3 đến 30 ký tự!")?).await; }
                let Some(&(_, _, gold_c, gem_c, _, _)) = self.shared.templates.flag_bags.iter().find(|f| f.0 == i32::from(*img_id)) else { return session.send(s2c_packets::thong_bao("Biểu tượng bang hội không hợp lệ!")?).await; };
                let (gold_cost, gem_cost) = (i64::from(gold_c.max(0)), gem_c.max(0));
                let guard = clan_op_lock().lock().await;
                let Ok(cid) = self.shared.clans_repo.next_clan_id().await else { drop(guard); return session.send(s2c_packets::thong_bao("Không thể tạo bang hội lúc này!")?).await; };
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    if super::router_shop::is_in_trade(e.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!"); }
                    if e.snapshot.clan_id.0 >= 0 { return Err("Bạn đã ở trong bang hội!"); }
                    if e.snapshot.wallet.gold < gold_cost || e.snapshot.wallet.gem < gem_cost { return Err("Không đủ vàng hoặc ngọc để tạo bang hội!"); }
                    e.snapshot.wallet.gold -= gold_cost; e.snapshot.wallet.gem -= gem_cost; e.snapshot.clan_id = cid;
                    let (h, b, l, _, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                    let clan = ClanRecord {
                        id: cid, name: trimmed.to_owned(), name_2: trimmed.to_owned(), slogan: String::new(), img_id: *img_id as i8,
                        power_point: e.snapshot.stats.power, max_member: 10, clan_point: 0, level: 1, members: vec![new_clan_member(&e.snapshot, (h, b, l), 0)],
                        create_time: Utc::now(), bdkb_level_done: 0, bdkb_duration_ms: 0, leader_info: UnquotedLeaderInfo::default(),
                    };
                    Ok((e.snapshot.id, clan, wallet_money_pkt(&e.snapshot.wallet)))
                });
                match res {
                    Some(Ok((pid, clan, mp))) => {
                        if self.shared.clans_repo.insert_clan(&clan).await.is_err() {
                            self.cleanup_trade_on_disconnect(pid);
                            if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.wallet.gold += gold_cost; e.snapshot.wallet.gem += gem_cost; e.snapshot.clan_id = ClanId(-1); }
                            drop(guard); return session.send(s2c_packets::thong_bao("Không thể tạo bang hội lúc này!")?).await;
                        }
                        clear_clan_invites(pid); drop(guard);
                        session.send(mp).await?; session.send(build_clan_info_pkt(Some(&clan), pid)?).await?;
                    }
                    Some(Err(msg)) => { drop(guard); session.send(s2c_packets::thong_bao(msg)?).await?; }
                    None => {}
                }
            }
            ClanInfoAction::Update(img_id, slogan) => {
                let slogan = slogan.trim(); if slogan.chars().count() > 100 { return session.send(s2c_packets::thong_bao("Khẩu hiệu bang hội tối đa 100 ký tự!")?).await; }
                let Some(&(_, _, gold_c, gem_c, _, _)) = self.shared.templates.flag_bags.iter().find(|f| f.0 == i32::from(*img_id)) else { return session.send(s2c_packets::thong_bao("Biểu tượng bang hội không hợp lệ!")?).await; };
                let guard = clan_op_lock().lock().await;
                if let Some((pid, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id))
                    && let Ok(mut clans) = self.shared.clans_repo.load_all().await && let Some(clan) = clans.iter_mut().find(|c| c.id == cid)
                {
                    if !clan.members.iter().any(|m| m.id == pid.0 as i32 && m.role <= 1) { drop(guard); return session.send(s2c_packets::thong_bao("Chỉ bang chủ hoặc phó bang mới có quyền cập nhật!")?).await; }
                    let (mut mp, mut charged) = (None, (0_i64, 0_i32));
                    if *img_id as i8 != clan.img_id {
                        let (gc, gmc) = (i64::from(gold_c.max(0)), gem_c.max(0));
                        let paid = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                            if super::router_shop::is_in_trade(e.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!"); }
                            if e.snapshot.wallet.gold < gc || e.snapshot.wallet.gem < gmc { return Err("Không đủ vàng hoặc ngọc để đổi biểu tượng!"); }
                            e.snapshot.wallet.gold -= gc; e.snapshot.wallet.gem -= gmc; Ok(wallet_money_pkt(&e.snapshot.wallet))
                        });
                        match paid { Some(Ok(p)) => { mp = Some(p); charged = (gc, gmc); } Some(Err(msg)) => { drop(guard); return session.send(s2c_packets::thong_bao(msg)?).await; } None => return Ok(()) }
                    }
                    clan.img_id = *img_id as i8; slogan.clone_into(&mut clan.slogan);
                    if self.shared.clans_repo.update_clan(clan).await.is_err() {
                        if charged != (0, 0) {
                            self.cleanup_trade_on_disconnect(pid);
                            if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.wallet.gold += charged.0; e.snapshot.wallet.gem += charged.1; }
                        }
                        drop(guard); return session.send(s2c_packets::thong_bao("Không thể cập nhật bang hội lúc này!")?).await;
                    }
                    drop(guard);
                    if let Some(p) = mp { session.send(p).await?; }
                    let mut w = PacketWriter::with_capacity(8 + slogan.len()); w.write_i8(4); w.write_i16(*img_id); w.write_utf(slogan)?;
                    session.send(w.finish(cmd_message::CLAN_CREATE_INFO)).await?;
                    self.broadcast_clan_info(clan);
                }
            }
            ClanInfoAction::Unknown(_) => {}
        }
        Ok(())
    }

    pub(super) async fn handle_clan_search(&self, session: &SessionHandle, _sid: u64, query: &str) -> Result<(), ProtocolError> {
        let q = query.trim().to_ascii_lowercase();
        let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
        let matched: Vec<_> = clans.iter().filter(|c| q.is_empty() || c.name.to_ascii_lowercase().contains(&q)).take(20).collect();
        let mut w = PacketWriter::with_capacity(4 + matched.len() * 48); w.write_i8(matched.len() as i8);
        for c in matched {
            let leader = c.members.iter().find(|m| m.role == 0).or_else(|| c.members.first()).map_or("", |m| m.name.as_str());
            w.write_i32(c.id.0); w.write_utf(&c.name)?; w.write_utf(&c.slogan)?; w.write_i16(i16::from(c.img_id.cast_unsigned()));
            w.write_utf(&c.power_point.to_string())?; w.write_utf(leader)?;
            w.write_u8(c.members.len() as u8); w.write_u8(c.max_member.max(10).cast_unsigned()); w.write_i32(c.create_time.timestamp() as i32);
        }
        session.send(w.finish(cmd_message::CLAN_SEARCH)).await
    }

    pub(super) async fn handle_clan_member_list(&self, session: &SessionHandle, _sid: u64, clan_id: i32) -> Result<(), ProtocolError> {
        let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
        let members = clans.iter().find(|c| c.id.0 == clan_id).map_or(&[][..], |c| c.members.as_slice());
        let mut w = PacketWriter::with_capacity(4 + members.len() * 40); w.write_i8(members.len().min(50) as i8);
        for m in members.iter().take(50) { write_clan_member(&mut w, m, false)?; }
        session.send(w.finish(cmd_message::CLAN_MEMBER)).await
    }

    pub(super) async fn handle_clan_info_req(&self, session: &SessionHandle, sid: u64, req_id: i16) -> Result<(), ProtocolError> {
        let Some((pid, my_cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else { return Ok(()); };
        let target = if req_id >= 0 { ClanId(i32::from(req_id)) } else { my_cid };
        let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
        session.send(build_clan_info_pkt(clans.iter().find(|c| c.id == target), pid)?).await
    }

    pub(super) async fn handle_clan_invite(&self, session: &SessionHandle, sid: u64, act: ClanInviteAction) -> Result<(), ProtocolError> {
        match act {
            ClanInviteAction::SendInvite(tpid) => {
                let Some((pid, name, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.clan_id)) else { return Ok(()); };
                if cid.0 < 0 { return Ok(()); }
                let clans = self.shared.clans_repo.load_all().await.unwrap_or_default();
                if !clans.iter().find(|c| c.id == cid).is_some_and(|c| c.members.iter().any(|m| m.id == pid.0 as i32 && m.role <= 1)) { return session.send(s2c_packets::thong_bao("Chỉ bang chủ hoặc phó bang mới có quyền mời!")?).await; }
                if let Some(th) = self.handle_for_pid(tpid) {
                    pending_clan_invites().insert((tpid, cid), unix_now_ms() + 60_000);
                    let mut w = PacketWriter::with_capacity(24 + name.len()); w.write_utf(&format!("{name} mời bạn vào bang hội!"))?; w.write_i32(cid.0); w.write_i32(1);
                    let _ = th.try_send(w.finish(cmd_message::CLAN_INVITE));
                }
            }
            ClanInviteAction::AcceptInvite(cid, _) => {
                let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
                let Some((_, exp)) = pending_clan_invites().remove(&(pid, ClanId(cid))) else { return session.send(s2c_packets::thong_bao("Lời mời vào bang không tồn tại hoặc đã hết hạn!")?).await; };
                if unix_now_ms() > exp { return session.send(s2c_packets::thong_bao("Lời mời vào bang đã hết hạn!")?).await; }
                self.join_clan_by_id(session, sid, cid).await?;
            }
            ClanInviteAction::DeclineInvite(cid, _) => if let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) { pending_clan_invites().remove(&(pid, ClanId(cid))); },
            ClanInviteAction::Unknown(_) => {}
        }
        Ok(())
    }

    pub(super) async fn handle_clan_remote(&self, session: &SessionHandle, sid: u64, target: PlayerId, role: i8) -> Result<(), ProtocolError> {
        let guard = clan_op_lock().lock().await;
        let Some((pid, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else { return Ok(()); };
        if cid.0 >= 0 && let Ok(mut clans) = self.shared.clans_repo.load_all().await && let Some(clan) = clans.iter_mut().find(|c| c.id == cid) {
            let Some(caller_role) = clan.members.iter().find(|m| m.id == pid.0 as i32).map(|m| m.role) else { return Ok(()); };
            let Some(target_role) = clan.members.iter().find(|m| m.id == target.0 as i32).map(|m| m.role) else { return Ok(()); };
            if role < 0 {
                if caller_role > 1 || target_role <= caller_role { drop(guard); return session.send(s2c_packets::thong_bao("Bạn không có quyền đuổi thành viên này!")?).await; }
                clan.members.retain(|m| m.id != target.0 as i32);
            } else {
                if caller_role != 0 { drop(guard); return session.send(s2c_packets::thong_bao("Chỉ bang chủ mới có quyền thay đổi chức vụ!")?).await; }
                if target == pid { return Ok(()); }
                let new_role = role.clamp(0, 2);
                if new_role == 0 {
                    if target_role != 1 {
                        drop(guard);
                        return session.send(s2c_packets::thong_bao("Chỉ có thể nhường chức Bang chủ cho Phó bang!")?).await;
                    }
                    if let Some(old) = clan.members.iter_mut().find(|m| m.id == pid.0 as i32) { old.role = 2; }
                }
                if let Some(m) = clan.members.iter_mut().find(|m| m.id == target.0 as i32) { m.role = new_role; }
            }
            if self.shared.clans_repo.update_clan(clan).await.is_err() { drop(guard); return session.send(s2c_packets::thong_bao("Không thể cập nhật bang hội lúc này!")?).await; }
            if role < 0 && let Ok(kick_pkt) = build_clan_info_pkt(None, target) {
                for mut entry in self.shared.clients.authenticated.iter_mut() { if entry.snapshot.id == target { entry.snapshot.clan_id = ClanId(-1); let _ = entry.handle.try_send(kick_pkt.clone()); } }
            }
            drop(guard);
            self.broadcast_clan_info(clan);
        }
        Ok(())
    }

    pub(super) async fn join_clan_by_id(&self, session: &SessionHandle, sid: u64, join_cid: i32) -> Result<(), ProtocolError> {
        let guard = clan_op_lock().lock().await;
        if self.shared.clients.authenticated.get(&sid).is_some_and(|e| e.snapshot.clan_id.0 >= 0) { drop(guard); return session.send(s2c_packets::thong_bao("Bạn đã ở trong bang hội!")?).await; }
        if let Ok(mut clans) = self.shared.clans_repo.load_all().await
            && let Some(clan) = clans.iter_mut().find(|c| c.id.0 == join_cid)
            && clan.members.len() < usize::from(clan.max_member.max(10).cast_unsigned())
            && let Some(res) = self.shared.clients.authenticated.get(&sid).map(|me| {
                if me.snapshot.clan_id.0 >= 0 { return Err("Bạn đã ở trong bang hội!"); }
                let (h, b, l, _, _) = effective_appearance(&self.shared.templates, &me.snapshot);
                if !clan.members.iter().any(|m| m.id == me.snapshot.id.0 as i32) { clan.members.push(new_clan_member(&me.snapshot, (h, b, l), 2)); }
                Ok(me.snapshot.id)
            })
        {
            match res {
                Ok(pid) => {
                    if self.shared.clans_repo.update_clan(clan).await.is_err() { drop(guard); return session.send(s2c_packets::thong_bao("Không thể tham gia bang hội lúc này!")?).await; }
                    if let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) { me.snapshot.clan_id = clan.id; }
                    clear_clan_invites(pid); drop(guard);
                    self.broadcast_clan_info(clan);
                }
                Err(msg) => { drop(guard); session.send(s2c_packets::thong_bao(msg)?).await?; }
            }
        }
        Ok(())
    }

    pub(super) async fn handle_clan_leave_menu(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        if let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) {
            con_meo_state().insert(pid, const_npc::CONFIRM_LEAVE_CLAN);
            session.send(s2c_packets::npc_open_menu(i16::from(const_npc::CON_MEO), "Bạn có chắc chắn muốn rời bang hội?", &["Đồng ý", "Từ chối"], None)?).await?;
        }
        Ok(())
    }

    pub(super) async fn execute_leave_clan(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let guard = clan_op_lock().lock().await;
        let Some((pid, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else { return Ok(()); };
        let mut updated_clan = None;
        if cid.0 >= 0 {
            let Ok(mut clans) = self.shared.clans_repo.load_all().await else { drop(guard); return session.send(s2c_packets::thong_bao("Không thể rời bang hội lúc này!")?).await; };
            if let Some(clan) = clans.iter_mut().find(|c| c.id == cid) {
                clan.members.retain(|m| m.id != pid.0 as i32);
                let db_res = if clan.members.is_empty() { self.shared.clans_repo.delete_clan(cid).await } else {
                    if !clan.members.iter().any(|m| m.role == 0) && let Some(next) = clan.members.iter_mut().min_by_key(|m| m.role) { next.role = 0; }
                    let r = self.shared.clans_repo.update_clan(clan).await;
                    if r.is_ok() { updated_clan = Some(clan.clone()); }
                    r
                };
                if db_res.is_err() { drop(guard); return session.send(s2c_packets::thong_bao("Không thể rời bang hội lúc này!")?).await; }
            }
        }
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.clan_id = ClanId(-1); }
        drop(guard);
        if let Some(ref clan) = updated_clan { self.broadcast_clan_info(clan); }
        session.send(build_clan_info_pkt(None, pid)?).await
    }
}

fn new_clan_member(snap: &PlayerSnapshot, parts: (i16, i16, i16), role: i8) -> ClanMemberRecord {
    ClanMemberRecord {
        id: snap.id.0 as i32, name: snap.name.clone(), head: parts.0, body: parts.1, leg: parts.2, role, donate: 0,
        receive_donate: 0, member_point: 0, clan_point: 0, join_time: (unix_now_ms() / 1_000) as i32, ask_pea_time: 0, power: snap.stats.power,
    }
}

pub(super) fn build_clan_info_pkt(clan: Option<&ClanRecord>, viewer: PlayerId) -> Result<Packet, ProtocolError> {
    let Some(c) = clan else { let mut w = PacketWriter::with_capacity(4); w.write_i32(-1); return Ok(w.finish(cmd_message::CLAN_INFO)); };
    let mut w = PacketWriter::with_capacity(64 + c.members.len() * 44);
    let leader = c.members.iter().find(|m| m.role == 0).or_else(|| c.members.first()).map_or("", |m| m.name.as_str());
    let my_role = c.members.iter().find(|m| m.id == viewer.0 as i32).map_or(2, |m| m.role);
    w.write_i32(c.id.0); w.write_utf(&c.name)?; w.write_utf(&c.slogan)?; w.write_i16(i16::from(c.img_id.cast_unsigned()));
    w.write_utf(&c.power_point.to_string())?; w.write_utf(leader)?;
    w.write_u8(c.members.len() as u8); w.write_u8(c.max_member.max(10).cast_unsigned());
    w.write_i8(my_role); w.write_i32(c.clan_point); w.write_i8(c.level.max(1));
    for m in &c.members { write_clan_member(&mut w, m, true)?; }
    w.write_u8(0); Ok(w.finish(cmd_message::CLAN_INFO))
}

fn write_clan_member(w: &mut PacketWriter, m: &ClanMemberRecord, include_cur_pt: bool) -> Result<(), ProtocolError> {
    w.write_i32(m.id); for v in [m.head, -1, m.leg, m.body] { w.write_i16(v); }
    w.write_utf(&m.name)?; w.write_i8(m.role); w.write_utf(&m.power.to_string())?;
    for v in [m.donate, m.receive_donate, m.clan_point] { w.write_i32(v); }
    if include_cur_pt { w.write_i32(m.member_point); }
    w.write_i32(m.join_time); Ok(())
}
