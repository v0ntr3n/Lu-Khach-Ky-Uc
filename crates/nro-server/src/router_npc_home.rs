//! HomePlant (`Trứng Mabư` / `Dưa Hấu`) menu and hatch/harvest handlers.

#[path = "router_npc_home_combine.rs"]
mod router_npc_home_combine;
#[path = "router_npc_home_wish.rs"]
mod router_npc_home_wish;

use nro_consts::{const_item, const_npc};
use nro_core::{ItemInstance, PlayerId, TemplateId};
use nro_db::json_columns::EggTimer;
use nro_economy::MabuEggState;
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use smallvec::SmallVec;

use super::{
    ServerSessionHandler, npc_menu_index, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use crate::data_game_tpl;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(super) async fn handle_open_home_plant(
        &self,
        session: &SessionHandle,
        sid: u64,
        player_id: PlayerId,
        npc_id: i16,
        is_dua_hau: bool,
        now_ms: i64,
    ) -> Result<(), ProtocolError> {
        let egg = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            if is_dua_hau {
                e.snapshot.dua_hau_egg
            } else {
                e.snapshot.mabu_egg
            }
        });
        if is_dua_hau {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session
                    .send(s2c_packets::npc_open_say(
                        npc_id,
                        "Bạn chưa trồng Dưa Hấu nào!",
                        None,
                    )?)
                    .await;
            };
            let rem_sec = egg
                .last_time_create_ms
                .saturating_add(egg.time_done_ms)
                .saturating_sub(now_ms)
                .max(0)
                / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CO_THE_THU_HOACH));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        "Dưa Hấu đã chín, bạn có thể thu hoạch!",
                        &["Thu hoạch", "Đóng"],
                        None,
                    )?)
                    .await
            } else {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CHUA_THE_THU_HOACH));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        &format!("Dưa Hấu đang lớn, còn {rem_sec} giây nữa mới chín."),
                        &["Đóng"],
                        None,
                    )?)
                    .await
            }
        } else {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session
                    .send(s2c_packets::npc_open_say(
                        npc_id,
                        "Bạn chưa có Trứng Mabư nào!",
                        None,
                    )?)
                    .await;
            };
            let rem_sec = egg
                .last_time_create_ms
                .saturating_add(egg.time_done_ms)
                .saturating_sub(now_ms)
                .max(0)
                / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CAN_OPEN_EGG));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        "Trứng Mabư đã sẵn sàng nở!",
                        &["Nở trứng", "Đóng"],
                        None,
                    )?)
                    .await
            } else {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CAN_NOT_OPEN_EGG));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        &format!("Trứng Mabư còn {rem_sec} giây nữa mới nở."),
                        &["Nở ngay (1 Tỷ vàng)", "Đóng"],
                        None,
                    )?)
                    .await
            }
        }
    }

    pub(super) async fn handle_home_plant_confirm(
        &self,
        session: &SessionHandle,
        sid: u64,
        pid: PlayerId,
        npc_id: i16,
        cur_menu_idx: i32,
        select: u8,
    ) -> Result<(), ProtocolError> {
        npc_menu_index().remove(&pid);
        let now_ms = unix_now_ms();
        if npc_id == i16::from(const_npc::QUA_TRUNG) {
            if cur_menu_idx == const_npc::CAN_NOT_OPEN_EGG && select == 0 {
                let res = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut entry| {
                        if super::router_shop::is_in_trade(entry.snapshot.id) {
                            return Err("Không thể thao tác khi đang giao dịch!".to_owned());
                        }
                        let Some(egg) = entry.snapshot.mabu_egg else {
                            return Err("Bạn không có Trứng Mabư nào!".to_owned());
                        };
                        let mut egg_state = MabuEggState {
                            ready_at_ms: egg.last_time_create_ms.saturating_add(egg.time_done_ms),
                        };
                        let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                        egg_state
                            .fast_hatch(&mut inv, now_ms)
                            .map_err(|err| format!("Không thể nở nhanh: {err}"))?;
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        entry.snapshot.mabu_egg = Some(EggTimer {
                            last_time_create_ms: now_ms,
                            time_done_ms: 0,
                        });
                        Ok((
                            wallet_money_pkt(&entry.snapshot.wallet),
                            crate::router_pet::build_home_egg_pkts(&entry.snapshot, now_ms),
                        ))
                    });
                match res {
                    Some(Ok((mp, egg_pkts))) => {
                        session.send(mp).await?;
                        for ep in egg_pkts {
                            session.send(ep).await?;
                        }
                        session
                            .send(s2c_packets::thong_bao("Trứng Mabư đã sẵn sàng nở!")?)
                            .await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
                    None => {}
                }
                return Ok(());
            }
            if cur_menu_idx == const_npc::CAN_OPEN_EGG && select == 0 {
                let hatched =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .and_then(|mut entry| {
                            let egg = entry.snapshot.mabu_egg?;
                            if egg.last_time_create_ms.saturating_add(egg.time_done_ms) > now_ms {
                                return Some(Err("Trứng Mabư chưa sẵn sàng nở!"));
                            }
                            if entry
                                .snapshot
                                .pet
                                .as_ref()
                                .is_some_and(|p| p.fusion_type != 0)
                            {
                                return Some(Err("Vui lòng tách hợp thể trước khi nở Trứng Mabư!"));
                            }
                            let had_pet = entry.snapshot.pet.is_some();
                            let mut fresh =
                                crate::router_pet::make_fresh_pet(1, entry.snapshot.gender);
                            if let Some(prev) = entry.snapshot.pet.take() {
                                for (i, slot) in prev.items_body.into_iter().enumerate() {
                                    if let Some(it) = slot {
                                        while fresh.items_body.len() <= i {
                                            fresh.items_body.push(None);
                                        }
                                        fresh.items_body[i] = Some(it);
                                    }
                                }
                            }
                            entry.snapshot.pet = Some(fresh);
                            entry.snapshot.mabu_egg = None;
                            crate::router_pet::recalculate_pet_in_snapshot(
                                &self.shared,
                                &mut entry.snapshot,
                            );
                            super::recalculate_session_stats(
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
                                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(
                                    ek,
                                    entry.snapshot.stats,
                                ));
                            }
                            Some(Ok((
                                !had_pet,
                                entry.snapshot.clone(),
                                entry.entity_key,
                                self.pkt_pet_detail(&entry.snapshot),
                                s2c_packets::me_load_point(&entry.snapshot.stats),
                            )))
                        });
                match hatched {
                    Some(Ok((is_new, snap, ek, pet_pkt, pt_pkt))) => {
                        if is_new {
                            session.send(s2c_packets::have_pet(true)).await?;
                        }
                        if let Some(p) = pet_pkt {
                            session.send(p).await?;
                        }
                        session.send(pt_pkt).await?;
                        if matches!(snap.location.map_id.0, 21..=23) {
                            session
                                .send(data_game_tpl::pkt_map_info(
                                    &self.shared.assets,
                                    &self.shared.templates,
                                    &snap,
                                ))
                                .await?;
                            session
                                .send(super::router_world::build_magic_tree_load(&snap)?)
                                .await?;
                            for ep in crate::router_pet::build_home_egg_pkts(&snap, now_ms) {
                                session.send(ep).await?;
                            }
                            if let (Some(ek), Ok(tx)) = (
                                ek,
                                self.shared
                                    .world_router
                                    .resolve_sender(snap.location.map_id, snap.location.zone_id),
                            ) {
                                let _ = tx.try_send(ZoneCommand::SyncZoneState(ek));
                            }
                        }
                        crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                        session
                            .send(s2c_packets::thong_bao(
                                "Chúc mừng! Trứng Mabư đã nở thành công!",
                            )?)
                            .await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => {
                        session
                            .send(s2c_packets::thong_bao("Bạn không có Trứng Mabư nào!")?)
                            .await?;
                    }
                }
                return Ok(());
            }
        } else if npc_id == i16::from(const_npc::DUA_HAU)
            && cur_menu_idx == const_npc::CO_THE_THU_HOACH
            && select == 0
        {
            let res = self
                .shared
                .clients
                .authenticated
                .get_mut(&sid)
                .map(|mut entry| {
                    if super::router_shop::is_in_trade(entry.snapshot.id) {
                        return Err("Không thể thao tác khi đang giao dịch!".to_owned());
                    }
                    let Some(egg) = entry.snapshot.dua_hau_egg else {
                        return Err("Bạn chưa trồng Dưa Hấu nào!".to_owned());
                    };
                    if egg.last_time_create_ms.saturating_add(egg.time_done_ms) > now_ms {
                        return Err("Dưa Hấu chưa chín!".to_owned());
                    }
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    inv.add_to_bag(
                        ItemInstance {
                            template_id: TemplateId(const_item::DUA_HAU as i16),
                            quantity: 1,
                            options: SmallVec::new(),
                            create_time_ms: now_ms,
                        },
                        true,
                    )
                    .map_err(|_| "Hành trang đã đầy!".to_owned())?;
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    entry.snapshot.dua_hau_egg = None;
                    Ok((
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        entry.snapshot.clone(),
                        entry.entity_key,
                    ))
                });
            match res {
                Some(Ok((bag_pkt, snap, ek))) => {
                    session.send(bag_pkt).await?;
                    if matches!(snap.location.map_id.0, 21..=23) {
                        session
                            .send(data_game_tpl::pkt_map_info(
                                &self.shared.assets,
                                &self.shared.templates,
                                &snap,
                            ))
                            .await?;
                        session
                            .send(super::router_world::build_magic_tree_load(&snap)?)
                            .await?;
                        for ep in crate::router_pet::build_home_egg_pkts(&snap, now_ms) {
                            session.send(ep).await?;
                        }
                        if let (Some(ek), Ok(tx)) = (
                            ek,
                            self.shared
                                .world_router
                                .resolve_sender(snap.location.map_id, snap.location.zone_id),
                        ) {
                            let _ = tx.try_send(ZoneCommand::SyncZoneState(ek));
                        }
                        crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                    }
                    session
                        .send(s2c_packets::thong_bao("Đã thu hoạch Dưa Hấu thành công!")?)
                        .await?;
                }
                Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
                None => {}
            }
            return Ok(());
        }
        if let Some(mp) = self.money_pkt_for(sid) {
            session.send(mp).await?;
        }
        Ok(())
    }
}
