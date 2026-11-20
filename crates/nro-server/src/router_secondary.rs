//! Secondary gameplay systems: Pets (`-107`, `-108`, `125`), Clans (`-46..-57`), and Radar (`127`).

use std::sync::OnceLock;

use chrono::Utc;
use dashmap::DashMap;
use nro_consts::{cmd_message, const_npc, const_player};
use nro_core::{ClanId, CoreStats, Gender, PlayerId};
use nro_db::{UnquotedLeaderInfo, json_columns::{PetSnapshot, SkillSlot}, player_repo::PlayerSnapshot, world_repos::{ClanMemberRecord, ClanRecord}};
use nro_protocol::{ClanInfoAction, ClanInviteAction, ClanMessageAction, Packet, PacketWriter, ProtocolError, RadarAction, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::Mutex;

use super::{ServerSessionHandler, con_meo_state, effective_appearance, wallet_money_pkt};
use crate::runtime::unix_now_ms;

static PENDING_CLAN_INVITES: OnceLock<DashMap<(PlayerId, ClanId), i64>> = OnceLock::new();
fn pending_clan_invites() -> &'static DashMap<(PlayerId, ClanId), i64> { PENDING_CLAN_INVITES.get_or_init(DashMap::new) }
static CLAN_OP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
fn clan_op_lock() -> &'static Mutex<()> { CLAN_OP_LOCK.get_or_init(|| Mutex::new(())) }
pub(super) fn clear_clan_invites(pid: PlayerId) { pending_clan_invites().retain(|k, _| k.0 != pid); }

impl ServerSessionHandler {
    pub(super) async fn send_login_secondary(&self, session: &SessionHandle, snap: &PlayerSnapshot) -> Result<(), ProtocolError> {
        if let Some(pet_pkt) = self.pkt_pet_detail(snap) { session.send(pet_pkt).await?; }
        if snap.intrinsic.intrinsic_id > 0 && let Some(intr) = self.shared.templates.intrinsics.iter().find(|r| r.id == i32::from(snap.intrinsic.intrinsic_id)) {
            session.send(s2c_packets::intrinsic_info(intr.icon as i16, &intr.name.replacen('#', &snap.intrinsic.param1.to_string(), 1))?).await?;
        }
        if snap.clan_id.0 >= 0 && let Ok(clans) = self.shared.clans_repo.load_all().await {
            if let Some(clan) = clans.iter().find(|c| c.id == snap.clan_id && c.members.iter().any(|m| m.id == snap.id.0 as i32)) { session.send(build_clan_info_pkt(Some(clan), snap.id)?).await?; }
            else {
                for mut e in self.shared.clients.authenticated.iter_mut() { if e.snapshot.id == snap.id { e.snapshot.clan_id = ClanId(-1); } }
                session.send(build_clan_info_pkt(None, snap.id)?).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn grant_starter_pet(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let pkts = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.pet.is_some() { return None; }
            let g = entry.snapshot.gender;
            let first_sk = match g { Gender::TraiDat => 0, Gender::Namec => 2, Gender::Xayda => 4 };
            let mut skills = vec![SkillSlot { template_id: first_sk, point: 1, last_time_use_ms: 0, curr_level: 0 }];
            skills.resize(4, SkillSlot { template_id: -1, point: 0, last_time_use_ms: 0, curr_level: 0 });
            entry.snapshot.pet = Some(PetSnapshot {
                type_pet: 0, gender: g, name: "$Đệ tử".to_owned(), fusion_type: 0, fusion_time_left_ms: 0, status: 0, items_body: vec![None; 7], skills,
                stats: CoreStats {
                    power: 2_000, tiem_nang: 2_000, stamina: 1_000, max_stamina: 1_000, hpg: 1_000, mpg: 1_000, dameg: 45, defg: 10, critg: 2,
                    hp: 1_000, mp: 1_000, hp_max: 1_000, mp_max: 1_000, dame: 45, def: 10, crit: 2, speed: 5, ..CoreStats::default()
                },
            });
            super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            Some((s2c_packets::have_pet(true), self.pkt_pet_detail(&entry.snapshot)))
        });
        if let Some((hp_pkt, det_pkt)) = pkts {
            session.send(hp_pkt).await?; if let Some(dp) = det_pkt { session.send(dp).await?; }
            session.send(s2c_packets::thong_bao("Chúc mừng bạn đã nhận được Đệ Tử!")?).await?;
        }
        Ok(())
    }

    #[must_use]
    pub(super) fn pkt_pet_detail(&self, snap: &PlayerSnapshot) -> Option<Packet> {
        let pet = snap.pet.as_ref()?;
        let mut w = PacketWriter::with_capacity(192);
        w.write_i8(2); w.write_i16(match (pet.type_pet, pet.gender) { (1, _) => 297, (_, Gender::TraiDat) => 303, (_, Gender::Namec) => 304, (_, Gender::Xayda) => 305 });
        let _ = s2c_packets::write_inventory_slots(&mut w, &pet.items_body);
        for v in [pet.stats.hp.max(1), pet.stats.hp_max.max(1_000), pet.stats.mp.max(1), pet.stats.mp_max.max(1_000), pet.stats.dame.max(10)] { w.write_i64(v); }
        let _ = w.write_utf(&pet.name); let _ = w.write_utf("Đệ tử");
        for v in [pet.stats.power.max(0), pet.stats.tiem_nang.max(0)] { w.write_i64(v); }
        w.write_i8(pet.status.clamp(0, 4)); w.write_i16(pet.stats.stamina.min(i16::MAX as u16) as i16);
        w.write_i16(pet.stats.max_stamina.clamp(1_000, i16::MAX as u16) as i16);
        w.write_i8(pet.stats.crit as i8); w.write_i64(i64::from(pet.stats.def.max(0)));
        let sk_n = pet.skills.len().min(5); w.write_i8(sk_n as i8);
        for sk in pet.skills.iter().take(sk_n) {
            if sk.template_id >= 0 && sk.point > 0 {
                let lv_id = self.shared.templates.skills.iter().find(|s| i32::from(s.id) == sk.template_id)
                    .and_then(|s| s.levels.iter().find(|l| l.point == sk.point).or_else(|| s.levels.first()))
                    .map_or(sk.template_id as i16, |l| l.id);
                w.write_i16(lv_id);
            } else { w.write_i16(-1); let _ = w.write_utf("Cần đạt sức mạnh để mở"); }
        }
        w.write_i8(0); w.write_i16(0); Some(w.finish(cmd_message::PET_INFO))
    }

    pub(super) async fn handle_pet_info(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        if let Some(p) = self.shared.clients.authenticated.get(&sid).map(|e| self.pkt_pet_detail(&e.snapshot).unwrap_or_else(|| s2c_packets::have_pet(false))) { session.send(p).await?; }
        Ok(())
    }

    pub(super) async fn handle_pet_status(&self, session: &SessionHandle, sid: u64, val: i8, is_fusion: bool) -> Result<(), ProtocolError> {
        if !is_fusion && val != 5 {
            if let Some(p) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut e| { e.snapshot.pet.as_mut()?.status = val.clamp(0, 4); self.pkt_pet_detail(&e.snapshot) }) { session.send(p).await?; }
            return Ok(());
        }
        if let Some((f_pkt, pt_pkt, loc)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
            let pet = entry.snapshot.pet.as_mut()?;
            pet.fusion_type = if pet.fusion_type == const_player::NON_FUSION { const_player::LUONG_LONG_NHAT_THE } else { const_player::NON_FUSION };
            let wire_mode = if pet.fusion_type == const_player::NON_FUSION { 1_i8 } else { pet.fusion_type };
            super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats)); }
            let mut w = PacketWriter::with_capacity(5); w.write_i8(wire_mode); w.write_i32(pid.0 as i32);
            Some((w.finish(cmd_message::FUSION), s2c_packets::me_load_point(&entry.snapshot.stats), loc))
        }) { self.broadcast_to_zone_peers(loc, None, &f_pkt); session.send(pt_pkt).await }
        else { session.send(s2c_packets::thong_bao("Bạn chưa có đệ tử để hợp thể!")?).await }
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
                    session.send(w.finish(cmd_message::CLAN_CREATE_INFO)).await?; session.send(build_clan_info_pkt(Some(clan), pid)?).await?;
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

    pub(super) async fn handle_clan_message(&self, session: &SessionHandle, sid: u64, act: &ClanMessageAction) -> Result<(), ProtocolError> {
        let Some((pid, name, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.name.clone(), e.snapshot.clan_id)) else { return Ok(()); };
        match act {
            ClanMessageAction::Chat(text) if cid.0 >= 0 => {
                let mut w = PacketWriter::with_capacity(24 + name.len() + text.len());
                w.write_i8(0); w.write_i32((unix_now_ms() & 0x7fff_ffff) as i32); w.write_i32(pid.0 as i32);
                w.write_utf(&name)?; w.write_i8(0); w.write_i32((unix_now_ms() / 1_000) as i32); w.write_utf(text)?; w.write_i8(0);
                let pkt = w.finish(cmd_message::CLAN_MESSAGE);
                for rec in &self.shared.clients.authenticated { if rec.value().snapshot.clan_id == cid { let _ = rec.value().handle.try_send(pkt.clone()); } }
            }
            ClanMessageAction::AskForJoin(_) => session.send(s2c_packets::thong_bao("Đã gửi yêu cầu xin vào bang hội!")?).await?,
            _ => {}
        }
        Ok(())
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
                if new_role == 0 && let Some(old) = clan.members.iter_mut().find(|m| m.id == pid.0 as i32) { old.role = 1; }
                if let Some(m) = clan.members.iter_mut().find(|m| m.id == target.0 as i32) { m.role = new_role; }
            }
            if self.shared.clans_repo.update_clan(clan).await.is_err() { drop(guard); return session.send(s2c_packets::thong_bao("Không thể cập nhật bang hội lúc này!")?).await; }
            if role < 0 && let Ok(kick_pkt) = build_clan_info_pkt(None, target) {
                for mut entry in self.shared.clients.authenticated.iter_mut() { if entry.snapshot.id == target { entry.snapshot.clan_id = ClanId(-1); let _ = entry.handle.try_send(kick_pkt.clone()); } }
            }
            drop(guard); session.send(build_clan_info_pkt(Some(clan), pid)?).await?;
        }
        Ok(())
    }

    async fn join_clan_by_id(&self, session: &SessionHandle, sid: u64, join_cid: i32) -> Result<(), ProtocolError> {
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
                    clear_clan_invites(pid); drop(guard); session.send(build_clan_info_pkt(Some(clan), pid)?).await?;
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
        if cid.0 >= 0 {
            let Ok(mut clans) = self.shared.clans_repo.load_all().await else { drop(guard); return session.send(s2c_packets::thong_bao("Không thể rời bang hội lúc này!")?).await; };
            if let Some(clan) = clans.iter_mut().find(|c| c.id == cid) {
                clan.members.retain(|m| m.id != pid.0 as i32);
                let db_res = if clan.members.is_empty() { self.shared.clans_repo.delete_clan(cid).await } else {
                    if !clan.members.iter().any(|m| m.role == 0) && let Some(next) = clan.members.iter_mut().min_by_key(|m| m.role) { next.role = 0; }
                    self.shared.clans_repo.update_clan(clan).await
                };
                if db_res.is_err() { drop(guard); return session.send(s2c_packets::thong_bao("Không thể rời bang hội lúc này!")?).await; }
            }
        }
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.clan_id = ClanId(-1); }
        drop(guard); session.send(build_clan_info_pkt(None, pid)?).await
    }

    pub(super) async fn handle_radar(&self, session: &SessionHandle, sid: u64, action: RadarAction) -> Result<(), ProtocolError> {
        match action {
            RadarAction::OpenList => {
                let owned = self.shared.clients.authenticated.get(&sid).map_or_default(|e| e.snapshot.radar_cards.clone());
                let cnt = self.shared.templates.radars.len().min(120);
                let mut w = PacketWriter::with_capacity(8 + cnt * 48); w.write_i8(0); w.write_i16(cnt as i16);
                for r in self.shared.templates.radars.iter().take(120) {
                    let rec = owned.iter().find(|c| c.id.0 == r.id);
                    for v in [r.id, r.icon_id] { w.write_i16(v); }
                    for v in [r.rank, rec.map_or(0, |c| c.amount.0), r.max, r.card_type] { w.write_i8(v); }
                    if r.card_type == 0 { w.write_i16(r.mob_id); } else { for p in [r.body.head, r.body.body, r.body.leg, r.body.bag] { w.write_i16(p); } }
                    w.write_utf(&r.name)?; w.write_utf(&r.info)?;
                    w.write_i8(rec.map_or(0, |c| c.level.0)); w.write_i8(rec.map_or(0, |c| c.used.0)); w.write_i8(r.options.len().min(8) as i8);
                    for o in r.options.iter().take(8) { w.write_i16(o.id as i16); w.write_i32(i32::from(o.param)); w.write_i8(o.active_card); }
                }
                session.send(w.finish(127)).await?;
            }
            RadarAction::ToggleCard(tid) => {
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    if !e.snapshot.radar_cards.iter().any(|c| c.id.0 == tid.0 && c.level.0 > 0) { return None; }
                    let mut new_use = 0_i8;
                    for c in &mut e.snapshot.radar_cards { if c.id.0 == tid.0 { c.used.0 = i8::from(c.used.0 == 0); new_use = c.used.0; } else { c.used.0 = 0; } }
                    super::recalculate_session_stats(sid, &mut e.snapshot, self.shared.config.parity_mode);
                    if let (Some(ek), Ok(tx)) = (e.entity_key, self.shared.world_router.resolve_sender(e.snapshot.location.map_id, e.snapshot.location.zone_id)) { let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, e.snapshot.stats)); }
                    Some((new_use, s2c_packets::me_load_point(&e.snapshot.stats)))
                });
                match res {
                    Some(Some((used, pt_pkt))) => { let mut w = PacketWriter::with_capacity(4); w.write_i8(1); w.write_i16(tid.0); w.write_i8(used); session.send(w.finish(127)).await?; session.send(pt_pkt).await?; }
                    Some(None) => session.send(s2c_packets::thong_bao("Bạn chưa mở khóa thẻ sưu tầm này!")?).await?,
                    None => {}
                }
            }
            RadarAction::Unknown(_) => {}
        }
        Ok(())
    }
}

fn new_clan_member(snap: &PlayerSnapshot, parts: (i16, i16, i16), role: i8) -> ClanMemberRecord {
    ClanMemberRecord {
        id: snap.id.0 as i32, name: snap.name.clone(), head: parts.0, body: parts.1, leg: parts.2, role, donate: 0,
        receive_donate: 0, member_point: 0, clan_point: 0, join_time: (unix_now_ms() / 1_000) as i32, ask_pea_time: 0, power: snap.stats.power,
    }
}

fn build_clan_info_pkt(clan: Option<&ClanRecord>, viewer: PlayerId) -> Result<Packet, ProtocolError> {
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
