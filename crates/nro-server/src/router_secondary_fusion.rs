//! Pet status (`-108`), fusion/unfusion (`125`) with client echo suppression, Namekian HTVV, and timed fusion expiry.

use nro_consts::{cmd_message, const_player};
use nro_core::Gender;
use nro_player::npoint::LIMIT_TIERS;
use nro_protocol::{PacketWriter, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::super::{ServerSessionHandler, effective_appearance, recalculate_session_stats};
use super::{
    FUSION_CLIENT_ECHO_WINDOW_MS, PET_FUSION_COOLDOWN_MS, STARTER_PET_CLAIMED_FLAG,
    last_fusion_enter_ms, pet_unfusion_cooldown,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_pet_status(
        &self,
        session: &SessionHandle,
        sid: u64,
        val: i8,
        is_fusion_cmd: bool,
    ) -> Result<(), ProtocolError> {
        let is_fusion = is_fusion_cmd || val == 4;
        if !is_fusion {
            if val == 5 {
                let res = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut entry| {
                        if entry.snapshot.gender != Gender::Namec {
                            return Err(
                                "Chỉ hành tinh Namec mới có thể thực hiện hợp thể vĩnh viễn!",
                            );
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
                            return Err(
                                "Vui lòng tháo hết trang bị của đệ tử trước khi hợp thể vĩnh viễn!",
                            );
                        }
                        let (pid, pet_power) = (entry.snapshot.id, pet.stats.power.max(0));
                        entry.snapshot.data_event.0[5] |= STARTER_PET_CLAIMED_FLAG;
                        let lim_idx = usize::from(entry.snapshot.stats.limit_power)
                            .min(LIMIT_TIERS.len() - 1);
                        let max_add = LIMIT_TIERS[lim_idx]
                            .0
                            .saturating_sub(entry.snapshot.stats.power)
                            .max(0);
                        let bonus = pet_power.min(max_add);
                        entry.snapshot.stats.power =
                            entry.snapshot.stats.power.saturating_add(bonus);
                        entry.snapshot.stats.tiem_nang =
                            entry.snapshot.stats.tiem_nang.saturating_add(bonus);
                        entry.snapshot.pet = None;
                        recalculate_session_stats(
                            sid,
                            &mut entry.snapshot,
                            self.shared.config.parity_mode,
                        );
                        let loc = entry.snapshot.location;
                        if let (Some(ek), Ok(tx)) = (
                            entry.entity_key,
                            self.shared
                                .world_router
                                .resolve_sender(loc.map_id, loc.zone_id),
                        ) {
                            let _ = tx
                                .try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                        }
                        Ok((
                            pid,
                            s2c_packets::have_pet(false),
                            s2c_packets::me_load_point(&entry.snapshot.stats),
                        ))
                    });
                match res {
                    Some(Ok((pid, have_pkt, pt_pkt))) => {
                        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                        session.send(have_pkt).await?;
                        session.send(pt_pkt).await?;
                        crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                        self.advance_task_on_power(session, sid).await?;
                        session
                            .send(s2c_packets::thong_bao("Hợp thể vĩnh viễn thành công!")?)
                            .await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => {}
                }
                return Ok(());
            }

            let is_currently_fused = self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .and_then(|e| {
                    e.snapshot
                        .pet
                        .as_ref()
                        .map(|p| p.fusion_type != const_player::NON_FUSION)
                })
                .unwrap_or(false);
            if !is_currently_fused {
                let status_res =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .and_then(|mut e| {
                            let pid = e.snapshot.id;
                            let pet = e.snapshot.pet.as_mut()?;
                            pet.status = val.clamp(0, 3);
                            Some((pid, self.pkt_pet_detail(&e.snapshot)))
                        });
                if let Some((pid, maybe_pkt)) = status_res {
                    if let Some(p) = maybe_pkt {
                        session.send(p).await?;
                    }
                    crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                }
                return Ok(());
            }
        }

        let now_ms = unix_now_ms();
        let post_unfuse_status = if !is_fusion_cmd && (0..=3).contains(&val) {
            val
        } else {
            0
        };
        let outcome = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                let (pid, loc, gender) = (
                    entry.snapshot.id,
                    entry.snapshot.location,
                    entry.snapshot.gender,
                );
                let Some(pet_ref) = entry.snapshot.pet.as_ref() else {
                    return Err("Bạn chưa có đệ tử để hợp thể!");
                };
                let entering_fusion = pet_ref.fusion_type == const_player::NON_FUSION;
                if !entering_fusion
                    && is_fusion_cmd
                    && (val == 4 || val == 5)
                    && last_fusion_enter_ms()
                        .get(&pid)
                        .is_some_and(|t| now_ms.saturating_sub(*t) <= FUSION_CLIENT_ECHO_WINDOW_MS)
                {
                    return Ok(None);
                }
                let target_mode = if entering_fusion {
                    if pet_ref.stats.hp <= 0 {
                        return Err("Đệ tử đã chết, không thể hợp thể!");
                    }
                    if pet_unfusion_cooldown()
                        .get(&pid)
                        .is_some_and(|t| now_ms.saturating_sub(*t) < PET_FUSION_COOLDOWN_MS)
                    {
                        return Err("Vui lòng đợi 5 giây trước khi hợp thể lại!");
                    }
                    let has_bag_item = |tid: i16| {
                        entry
                            .snapshot
                            .items_bag
                            .iter()
                            .flatten()
                            .any(|it| it.template_id.0 == tid && it.quantity > 0)
                    };
                    match val {
                        const_player::HOP_THE_PORATA => {
                            if !has_bag_item(454) {
                                return Err("Bạn cần có Bông tai Porata trong hành trang!");
                            }
                            const_player::HOP_THE_PORATA
                        }
                        const_player::HOP_THE_PORATA2 => {
                            if !has_bag_item(921) {
                                return Err("Bạn cần có Bông tai Porata cấp 2 trong hành trang!");
                            }
                            const_player::HOP_THE_PORATA2
                        }
                        const_player::HOP_THE_PORATA3 => {
                            if !has_bag_item(1819) {
                                return Err("Bạn cần có Bông tai Porata cấp 3 trong hành trang!");
                            }
                            const_player::HOP_THE_PORATA3
                        }
                        _ => const_player::LUONG_LONG_NHAT_THE,
                    }
                } else {
                    const_player::NON_FUSION
                };
                if entering_fusion {
                    last_fusion_enter_ms().insert(pid, now_ms);
                } else {
                    last_fusion_enter_ms().remove(&pid);
                    pet_unfusion_cooldown().insert(pid, now_ms);
                }
                if let Some(pet) = entry.snapshot.pet.as_mut() {
                    pet.fusion_type = target_mode;
                    pet.fusion_time_left_ms = if target_mode == const_player::LUONG_LONG_NHAT_THE {
                        600_000
                    } else {
                        0
                    };
                    pet.status = if entering_fusion {
                        4
                    } else {
                        post_unfuse_status
                    };
                }
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut entry.snapshot);
                let wire_mode = if entering_fusion {
                    match target_mode {
                        const_player::HOP_THE_PORATA
                        | const_player::HOP_THE_PORATA2
                        | const_player::HOP_THE_PORATA3 => 6_i8,
                        other => other,
                    }
                } else {
                    1_i8
                };
                recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                let (eh, eb, el, _, is_disg) =
                    effective_appearance(&self.shared.templates, &entry.snapshot);
                let (parts, upd_body) = if entering_fusion {
                    let fp = match gender {
                        Gender::Namec => (391_i16, 392_i16, 393_i16),
                        Gender::TraiDat | Gender::Xayda => (380_i16, 381_i16, 382_i16),
                    };
                    (
                        fp,
                        s2c_packets::update_body(pid, Some((fp.0, fp.1, fp.2, 0))),
                    )
                } else {
                    (
                        (eh, eb, el),
                        s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))),
                    )
                };
                if let (Some(ek), Ok(tx)) = (
                    entry.entity_key,
                    self.shared
                        .world_router
                        .resolve_sender(loc.map_id, loc.zone_id),
                ) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, parts));
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                let mut w = PacketWriter::with_capacity(5);
                w.write_i8(wire_mode);
                w.write_i32(pid.0 as i32);
                Ok(Some((
                    pid,
                    w.finish(cmd_message::FUSION),
                    upd_body,
                    s2c_packets::me_load_point(&entry.snapshot.stats),
                    self.pkt_pet_detail(&entry.snapshot),
                    loc,
                )))
            });
        match outcome {
            Some(Ok(Some((pid, f_pkt, upd_body, pt_pkt, pet_pkt, loc)))) => {
                self.broadcast_zone_or_self(loc, sid, &f_pkt);
                self.broadcast_zone_or_self(loc, sid, &upd_body);
                if let Some(pp) = pet_pkt {
                    session.send(pp).await?;
                }
                session.send(pt_pkt).await?;
                crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                Ok(())
            }
            Some(Ok(None)) | None => Ok(()),
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await,
        }
    }

    pub(crate) async fn tick_timed_fusions(&self, delta_ms: i32) {
        let now_ms = unix_now_ms();
        let mut expired = Vec::new();
        for mut entry in self.shared.clients.authenticated.iter_mut() {
            let sid = *entry.key();
            let Some(pet) = entry.snapshot.pet.as_mut() else {
                continue;
            };
            if pet.fusion_type != const_player::LUONG_LONG_NHAT_THE {
                continue;
            }
            pet.fusion_time_left_ms = pet.fusion_time_left_ms.saturating_sub(delta_ms);
            if pet.fusion_time_left_ms > 0 {
                continue;
            }
            pet.fusion_type = const_player::NON_FUSION;
            pet.fusion_time_left_ms = 0;
            pet.status = 0;
            let pid = entry.snapshot.id;
            last_fusion_enter_ms().remove(&pid);
            pet_unfusion_cooldown().insert(pid, now_ms);
            crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut entry.snapshot);
            recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
            let loc = entry.snapshot.location;
            let (eh, eb, el, _, is_disg) =
                effective_appearance(&self.shared.templates, &entry.snapshot);
            if let (Some(ek), Ok(tx)) = (
                entry.entity_key,
                self.shared
                    .world_router
                    .resolve_sender(loc.map_id, loc.zone_id),
            ) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            let mut w = PacketWriter::with_capacity(5);
            w.write_i8(1_i8);
            w.write_i32(pid.0 as i32);
            let upd_body = s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0)));
            let pt_pkt = s2c_packets::me_load_point(&entry.snapshot.stats);
            let pet_pkt = self.pkt_pet_detail(&entry.snapshot);
            expired.push((
                pid,
                sid,
                loc,
                entry.handle.clone(),
                w.finish(cmd_message::FUSION),
                upd_body,
                pt_pkt,
                pet_pkt,
            ));
        }
        for (pid, sid, loc, handle, f_pkt, upd_body, pt_pkt, pet_pkt) in expired {
            self.broadcast_zone_or_self(loc, sid, &f_pkt);
            self.broadcast_zone_or_self(loc, sid, &upd_body);
            if handle.sent_key() {
                if let Some(pp) = pet_pkt {
                    let _ = handle.try_send(pp);
                }
                let _ = handle.try_send(pt_pkt);
            }
            crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
        }
    }
}
