//! Secondary gameplay systems: Pets (`-107`, `-108`, `125`) and Radar (`127`).

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::{cmd_message, const_player};
use nro_core::{ClanId, CoreStats, Gender, PlayerId};
use nro_db::{
    json_columns::{PetSnapshot, SkillSlot},
    player_repo::PlayerSnapshot,
};
use nro_player::npoint::LIMIT_TIERS;
use nro_protocol::{Packet, PacketWriter, ProtocolError, RadarAction, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{
    ServerSessionHandler, effective_appearance,
    router_secondary_clan::{build_clan_info_pkt, clear_pending_clan_invites},
    router_secondary_clan_ops::clear_clan_join_requests,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

const PET_FUSION_COOLDOWN_MS: i64 = 5_000;

static PET_FUSION_COOLDOWN: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn pet_fusion_cooldown() -> &'static DashMap<PlayerId, i64> {
    PET_FUSION_COOLDOWN.get_or_init(DashMap::new)
}

const STARTER_PET_CLAIMED_FLAG: i32 = 1 << 30;

pub(super) fn clear_clan_invites(pid: PlayerId) {
    clear_pending_clan_invites(pid);
    clear_clan_join_requests(pid);
    pet_fusion_cooldown().remove(&pid);
}

impl ServerSessionHandler {
    pub(super) async fn send_login_secondary(&self, session: &SessionHandle, snap: &PlayerSnapshot) -> Result<(), ProtocolError> {
        if let Some(pet_pkt) = self.pkt_pet_detail(snap) { session.send(pet_pkt).await?; }
        if snap.intrinsic.intrinsic_id > 0 && let Some(intr) = self.shared.templates.intrinsics.iter().find(|r| r.id == i32::from(snap.intrinsic.intrinsic_id)) {
            session.send(s2c_packets::intrinsic_info(intr.icon as i16, &intr.name.replacen('#', &snap.intrinsic.param1.to_string(), 1))?).await?;
        }
        if snap.clan_id.0 >= 0 && let Ok(clan_opt) = self.shared.clans_repo.get_by_id(snap.clan_id).await {
            if let Some(clan) = clan_opt.as_ref().filter(|c| c.members.iter().any(|m| m.id == snap.id.0 as i32)) { session.send(build_clan_info_pkt(Some(clan), snap.id)?).await?; }
            else {
                for mut e in self.shared.clients.authenticated.iter_mut() { if e.snapshot.id == snap.id { e.snapshot.clan_id = ClanId(-1); } }
                session.send(build_clan_info_pkt(None, snap.id)?).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn grant_starter_pet(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if entry.snapshot.pet.is_some() { return Ok(None); }
            if (entry.snapshot.data_event.0[5] & STARTER_PET_CLAIMED_FLAG) != 0 {
                return Err("Bạn đã nhận Đệ Tử khởi đầu trước đó rồi!");
            }
            entry.snapshot.data_event.0[5] |= STARTER_PET_CLAIMED_FLAG;
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
            Ok(Some((s2c_packets::have_pet(true), self.pkt_pet_detail(&entry.snapshot))))
        });
        match res {
            Some(Ok(Some((hp_pkt, det_pkt)))) => {
                session.send(hp_pkt).await?; if let Some(dp) = det_pkt { session.send(dp).await?; }
                session.send(s2c_packets::thong_bao("Chúc mừng bạn đã nhận được Đệ Tử!")?).await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
            _ => {}
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
        if !is_fusion {
            if val == 5 {
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if entry.snapshot.gender != Gender::Namec {
                        return Err("Chỉ hành tinh Namec mới có thể thực hiện hợp thể vĩnh viễn!");
                    }
                    let Some(pet) = entry.snapshot.pet.as_ref() else {
                        return Err("Bạn chưa có đệ tử!");
                    };
                    if pet.stats.hp <= 0 {
                        return Err("Đệ tử đã chết, không thể hợp thể vĩnh viễn!");
                    }
                    if pet.fusion_type != const_player::NON_FUSION {
                        return Err("Vui lòng tách hợp thể trước khi hợp thể vĩnh viễn!");
                    }
                    if !pet.items_body.iter().all(Option::is_none) {
                        return Err("Vui lòng tháo hết trang bị của đệ tử trước khi hợp thể vĩnh viễn!");
                    }
                    entry.snapshot.data_event.0[5] |= STARTER_PET_CLAIMED_FLAG;
                    let lim_idx = usize::from(entry.snapshot.stats.limit_power).min(LIMIT_TIERS.len() - 1);
                    let max_add = LIMIT_TIERS[lim_idx].0.saturating_sub(entry.snapshot.stats.power).max(0);
                    let bonus = pet.stats.power.max(0).min(max_add);
                    entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(bonus);
                    entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(bonus);
                    entry.snapshot.pet = None;
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    Ok((s2c_packets::have_pet(false), s2c_packets::me_load_point(&entry.snapshot.stats)))
                });
                match res {
                    Some(Ok((have_pkt, pt_pkt))) => {
                        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                        session.send(have_pkt).await?;
                        session.send(pt_pkt).await?;
                        self.advance_task_on_power(session, sid).await?;
                        session.send(s2c_packets::thong_bao("Hợp thể vĩnh viễn thành công!")?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => {}
                }
                return Ok(());
            }
            let status_res = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut e| {
                let pet = e.snapshot.pet.as_mut()?;
                if pet.fusion_type != const_player::NON_FUSION {
                    return Some(Err("Không thể đổi trạng thái đệ tử khi đang hợp thể!"));
                }
                pet.status = val.clamp(0, 3);
                Some(Ok(self.pkt_pet_detail(&e.snapshot)))
            });
            match status_res {
                Some(Ok(Some(p))) => session.send(p).await?,
                Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                _ => {}
            }
            return Ok(());
        }

        let now_ms = unix_now_ms();
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let (pid, loc, gender) = (entry.snapshot.id, entry.snapshot.location, entry.snapshot.gender);
            let Some(pet_ref) = entry.snapshot.pet.as_ref() else {
                return Err("Bạn chưa có đệ tử để hợp thể!");
            };
            if pet_ref.stats.hp <= 0 {
                return Err("Đệ tử đã chết, không thể hợp thể!");
            }
            if pet_fusion_cooldown().get(&pid).is_some_and(|t| now_ms.saturating_sub(*t) < PET_FUSION_COOLDOWN_MS) {
                return Err("Vui lòng đợi 5 giây giữa các lần hợp thể và tách hợp thể!");
            }
            let entering_fusion = pet_ref.fusion_type == const_player::NON_FUSION;
            let target_mode = if entering_fusion {
                match val {
                    const_player::HOP_THE_PORATA => {
                        if !entry.snapshot.items_bag.iter().flatten().any(|it| it.template_id.0 == 454 && it.quantity > 0) {
                            return Err("Bạn cần có Bông tai Porata trong hành trang!");
                        }
                        const_player::HOP_THE_PORATA
                    }
                    const_player::HOP_THE_PORATA2 => {
                        if !entry.snapshot.items_bag.iter().flatten().any(|it| it.template_id.0 == 921 && it.quantity > 0) {
                            return Err("Bạn cần có Bông tai Porata cấp 2 trong hành trang!");
                        }
                        const_player::HOP_THE_PORATA2
                    }
                    const_player::HOP_THE_PORATA3 => {
                        if !entry.snapshot.items_bag.iter().flatten().any(|it| it.template_id.0 == 1819 && it.quantity > 0) {
                            return Err("Bạn cần có Bông tai Porata cấp 3 trong hành trang!");
                        }
                        const_player::HOP_THE_PORATA3
                    }
                    _ => const_player::LUONG_LONG_NHAT_THE,
                }
            } else {
                const_player::NON_FUSION
            };
            pet_fusion_cooldown().insert(pid, now_ms);
            if let Some(pet) = entry.snapshot.pet.as_mut() {
                pet.fusion_type = target_mode;
                pet.fusion_time_left_ms = if target_mode == const_player::LUONG_LONG_NHAT_THE { 600_000 } else { 0 };
                pet.status = if entering_fusion { 4 } else { 0 };
            }
            let wire_mode = if entering_fusion {
                match target_mode {
                    const_player::HOP_THE_PORATA | const_player::HOP_THE_PORATA2 | const_player::HOP_THE_PORATA3 => 6_i8,
                    other => other,
                }
            } else {
                1_i8
            };
            super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            let (eh, eb, el, _, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
            let (parts, upd_body) = if entering_fusion {
                let fp = match gender {
                    Gender::Namec => (391_i16, 392_i16, 393_i16),
                    Gender::TraiDat | Gender::Xayda => (380_i16, 381_i16, 382_i16),
                };
                (fp, s2c_packets::update_body(pid, Some((fp.0, fp.1, fp.2, 0))))
            } else {
                ((eh, eb, el), s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))))
            };
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, parts));
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            let mut w = PacketWriter::with_capacity(5);
            w.write_i8(wire_mode);
            w.write_i32(pid.0 as i32);
            Ok((w.finish(cmd_message::FUSION), upd_body, s2c_packets::me_load_point(&entry.snapshot.stats), self.pkt_pet_detail(&entry.snapshot), loc))
        });
        match outcome {
            Some(Ok((f_pkt, upd_body, pt_pkt, pet_pkt, loc))) => {
                self.broadcast_zone_or_self(loc, sid, &f_pkt);
                self.broadcast_zone_or_self(loc, sid, &upd_body);
                if let Some(pp) = pet_pkt { session.send(pp).await?; }
                session.send(pt_pkt).await
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await,
            None => Ok(()),
        }
    }

    pub(super) async fn handle_radar(&self, session: &SessionHandle, sid: u64, action: RadarAction) -> Result<(), ProtocolError> {
        match action {
            RadarAction::OpenList => {
                let owned = self.shared.clients.authenticated.get(&sid).map_or_default(|e| e.snapshot.radar_cards.clone());
                let cnt = self.shared.templates.radars.len().min(i16::MAX as usize);
                let mut w = PacketWriter::with_capacity(8 + cnt * 48); w.write_i8(0); w.write_i16(cnt as i16);
                for r in self.shared.templates.radars.iter().take(cnt) {
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
                    if !e.snapshot.radar_cards.iter().any(|c| c.id.0 == tid.0 && c.level.0 >= 0) { return None; }
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

    pub(crate) fn tick_timed_fusions(&self, delta_ms: i32) {
        let mut expired = Vec::new();
        for mut entry in self.shared.clients.authenticated.iter_mut() {
            let sid = *entry.key();
            let Some(pet) = entry.snapshot.pet.as_mut() else { continue; };
            if pet.fusion_type != const_player::LUONG_LONG_NHAT_THE { continue; }
            pet.fusion_time_left_ms = pet.fusion_time_left_ms.saturating_sub(delta_ms);
            if pet.fusion_time_left_ms > 0 { continue; }
            pet.fusion_type = const_player::NON_FUSION;
            pet.fusion_time_left_ms = 0;
            pet.status = 0;
            super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
            let (eh, eb, el, _, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            let mut w = PacketWriter::with_capacity(5);
            w.write_i8(1_i8);
            w.write_i32(pid.0 as i32);
            let upd_body = s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0)));
            let pt_pkt = s2c_packets::me_load_point(&entry.snapshot.stats);
            let pet_pkt = self.pkt_pet_detail(&entry.snapshot);
            expired.push((sid, loc, entry.handle.clone(), w.finish(cmd_message::FUSION), upd_body, pt_pkt, pet_pkt));
        }
        for (sid, loc, handle, f_pkt, upd_body, pt_pkt, pet_pkt) in expired {
            self.broadcast_zone_or_self(loc, sid, &f_pkt);
            self.broadcast_zone_or_self(loc, sid, &upd_body);
            if handle.sent_key() {
                if let Some(pp) = pet_pkt { let _ = handle.try_send(pp); }
                let _ = handle.try_send(pt_pkt);
            }
        }
    }
}
