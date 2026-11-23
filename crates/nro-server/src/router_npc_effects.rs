//! `NpcEffect` outcome handlers for [`ServerSessionHandler::handle_npc_menu`].

use nro_consts::const_npc;
use nro_core::{AccountId, MapId, PlayerId, ZoneId};
use nro_economy::{NpcEffect, NpcError, NpcKind};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::super::{
    ServerSessionHandler, npc_menu_index, pending_combine, recalculate_session_stats,
    wallet_money_pkt,
};
use super::{black_ball_buff, daily_checkin_day, phu_ho_mabu, reset_mabu_points};

impl ServerSessionHandler {
    pub(super) async fn handle_home_elder_starter_pet(
        &self,
        session: &SessionHandle,
        sid: u64,
        pid: PlayerId,
        npc_id: i16,
        select: Option<u8>,
    ) -> Result<(), ProtocolError> {
        match select {
            None => {
                npc_menu_index().insert(pid, (npc_id, const_npc::BASE_MENU));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        "Con muốn nhận Đệ Tử đồng hành không?",
                        &["Nói chuyện", "Nhận Đệ Tử", "Đóng"],
                        None,
                    )?)
                    .await
            }
            Some(0) => {
                session
                    .send(s2c_packets::npc_open_say(
                        npc_id,
                        "Con cố gắng học thành tài, đừng lo cho ta.",
                        None,
                    )?)
                    .await
            }
            Some(1) => {
                npc_menu_index().remove(&pid);
                self.grant_starter_pet(session, sid).await
            }
            Some(_) => {
                npc_menu_index().remove(&pid);
                if let Some(mp) = self.money_pkt_for(sid) {
                    session.send(mp).await?;
                }
                Ok(())
            }
        }
    }

    pub(super) async fn dispatch_npc_effect(
        &self,
        session: &SessionHandle,
        sid: u64,
        ids: (PlayerId, AccountId),
        npc_loc: (i16, NpcKind, MapId, ZoneId),
        select: Option<u8>,
        now_ms: i64,
        res: Result<NpcEffect, NpcError>,
        bag_changed: bool,
        pkts: [Packet; 4],
    ) -> Result<(), ProtocolError> {
        let (player_id, acc_id) = ids;
        let (npc_id, npc, map_id, cur_zone) = npc_loc;
        let [box_pkt, bag_pkt, money_pkt, pt_pkt] = pkts;

        if !matches!(
            res,
            Ok(NpcEffect::ShowMenu(_) | NpcEffect::OpenHomePlant(_))
        ) {
            npc_menu_index().remove(&player_id);
        }
        match res {
            Ok(NpcEffect::ShowMenu(prompt)) => {
                npc_menu_index().insert(player_id, (npc_id, prompt.menu_index));
                session
                    .send(s2c_packets::npc_open_menu(
                        npc_id,
                        prompt.say,
                        &prompt.options,
                        None,
                    )?)
                    .await?;
            }
            Ok(NpcEffect::ChangeMap(m, tx, ty)) => {
                if self
                    .switch_player_zone_with_ship(
                        session,
                        sid,
                        Some(m),
                        cur_zone,
                        Some((tx, ty)),
                        0,
                    )
                    .await?
                    && npc == NpcKind::Osin
                    && (114..=120).contains(&map_id.0)
                {
                    reset_mabu_points(player_id);
                }
            }
            Ok(NpcEffect::Tutorial(text)) => {
                if bag_changed {
                    session.send(bag_pkt).await?;
                }
                session
                    .send(s2c_packets::npc_open_say(npc_id, text, None)?)
                    .await?;
            }
            Ok(NpcEffect::OpenItemBox) => {
                session.send(box_pkt).await?;
                session.send(s2c_packets::inventory_box_open(0)).await?;
            }
            Ok(NpcEffect::OpenCombine(op)) => {
                pending_combine().insert(player_id, (npc_id, op, Vec::new()));
                session
                    .send(s2c_packets::combine_open(
                        &format!("Chọn trang bị để thực hiện: {op}"),
                        "Bà Hạt Mít",
                    ))
                    .await?;
            }
            Ok(NpcEffect::OpenMagicTree) => {
                let pkt = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut e| {
                        super::super::router_world::tick_magic_tree(&mut e.snapshot, now_ms);
                        super::super::router_world::build_magic_tree_menu_pkt(&e.snapshot)
                    })
                    .transpose()?;
                if let Some(pkt) = pkt {
                    session.send(pkt).await?;
                }
            }
            Ok(NpcEffect::OpenShop(tag)) => {
                self.handle_open_shop(session, sid, npc_id, tag).await?;
            }
            Ok(NpcEffect::OpenConsignShop) => self.handle_open_consign(session, sid).await?,
            Ok(NpcEffect::OpenInput(kind)) => self.handle_open_input(session, sid, kind).await?,
            Ok(NpcEffect::NoOp) => session.send(money_pkt).await?,
            Ok(NpcEffect::ClaimedDailyCheckIn) => {
                daily_checkin_day().retain(|_, day| *day >= now_ms / 86_400_000);
                for p in [bag_pkt, money_pkt, pt_pkt] {
                    session.send(p).await?;
                }
            }
            Ok(NpcEffect::UnlockedPowerLimit(true)) => {
                let pet_res = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut entry| {
                        if let Some(pet) = entry.snapshot.pet.as_mut()
                            && pet.stats.limit_power < 9
                        {
                            pet.stats.limit_power += 1;
                            Ok((
                                self.pkt_pet_detail(&entry.snapshot),
                                wallet_money_pkt(&entry.snapshot.wallet),
                            ))
                        } else {
                            entry.snapshot.wallet.gold = entry
                                .snapshot
                                .wallet
                                .gold
                                .saturating_add(50_000_000)
                                .min(nro_db::player_repo::LIMIT_GOLD);
                            Err(wallet_money_pkt(&entry.snapshot.wallet))
                        }
                    });
                match pet_res {
                    Some(Ok((pet_pkt, mp))) => {
                        if let Some(p) = pet_pkt {
                            session.send(p).await?;
                        }
                        session.send(mp).await?;
                    }
                    Some(Err(mp)) => {
                        session.send(mp).await?;
                        session
                            .send(s2c_packets::npc_open_say(
                                npc_id,
                                "Đệ tử đã đạt giới hạn sức mạnh tối đa!",
                                None,
                            )?)
                            .await?;
                    }
                    None => {}
                }
            }
            Ok(NpcEffect::UnlockedPowerLimit(false)) => {
                for p in [bag_pkt, money_pkt, pt_pkt] {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao("Đã mở giới hạn sức mạnh bản thân!")?)
                    .await?;
            }
            Ok(NpcEffect::PurchasedVip(tier)) => {
                self.handle_purchased_vip(session, sid, acc_id, npc_id, tier, now_ms)
                    .await?;
            }
            Ok(NpcEffect::ApplyGtpt) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
                    e.snapshot.item_time.gtpt_ms = now_ms.saturating_add(1_800_000);
                }
                for p in [bag_pkt, money_pkt, pt_pkt] {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao(
                        "Đã kích hoạt Giải trừ phép thuật (30 phút)!",
                    )?)
                    .await?;
            }
            Ok(NpcEffect::ApplyMabuBlessing) => {
                phu_ho_mabu().insert(player_id, true);
                let updated_pt =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .map(|mut entry| {
                            recalculate_session_stats(
                                sid,
                                &mut entry.snapshot,
                                self.shared.config.parity_mode,
                            );
                            entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                            entry.snapshot.stats.mp = entry.snapshot.stats.mp_max;
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
                            s2c_packets::me_load_point(&entry.snapshot.stats)
                        });
                for p in [bag_pkt, money_pkt, updated_pt.unwrap_or(pt_pkt)] {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao(
                        "Đã nhận Phù hộ Kaiô (+1.000.000 HP/KI, +10.000 Sức đánh)!",
                    )?)
                    .await?;
            }
            Ok(NpcEffect::ExtendKilisBuff) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
                    e.snapshot.item_time.kilis_ms = e
                        .snapshot
                        .item_time
                        .kilis_ms
                        .max(now_ms)
                        .saturating_add(600_000);
                }
                for p in [bag_pkt, money_pkt, pt_pkt] {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao(
                        "Đã gia hạn bùa Kilis thêm 10 phút!",
                    )?)
                    .await?;
            }
            Ok(NpcEffect::ApplyBlackBallBuff(hp_mult, dame_mult)) => {
                let (hm_u8, dm_u8) = (hp_mult.max(1), dame_mult.max(1));
                let updated_pt =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .map(|mut entry| {
                            let already = black_ball_buff()
                                .get(&player_id)
                                .is_some_and(|prev| prev.0 >= hm_u8 && prev.1 >= dm_u8);
                            if !already {
                                black_ball_buff().insert(player_id, (hm_u8, dm_u8));
                                recalculate_session_stats(
                                    sid,
                                    &mut entry.snapshot,
                                    self.shared.config.parity_mode,
                                );
                                if hm_u8 > 1 {
                                    entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                                }
                            }
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
                            s2c_packets::me_load_point(&entry.snapshot.stats)
                        });
                for p in [bag_pkt, money_pkt, updated_pt.unwrap_or(pt_pkt)] {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao(&format!(
                        "Đã kích hoạt Phù hộ Sao Đen (x{hp_mult} HP, x{dame_mult} Sức đánh)!"
                    ))?)
                    .await?;
            }
            Ok(NpcEffect::ClaimedKolStage(is_vip, _)) => {
                self.handle_kol_claim(session, sid, is_vip, [bag_pkt, pt_pkt])
                    .await?;
            }
            Ok(NpcEffect::CancelledSkillStudy) => {
                let cancel_res =
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .map(|mut entry| {
                            if entry.snapshot.learn_skill.0 > 0 {
                                let refund = i64::from(entry.snapshot.learn_skill.2.max(0)) / 2;
                                entry.snapshot.stats.tiem_nang =
                                    entry.snapshot.stats.tiem_nang.saturating_add(refund);
                                entry.snapshot.learn_skill = (-1, -1, 0);
                                Some((refund, s2c_packets::me_load_point(&entry.snapshot.stats)))
                            } else {
                                None
                            }
                        });
                match cancel_res {
                    Some(Some((refund, new_pt_pkt))) => {
                        session.send(new_pt_pkt).await?;
                        session
                            .send(s2c_packets::thong_bao(&format!(
                                "Đã hủy học kỹ năng và hoàn lại {refund} tiềm năng!"
                            ))?)
                            .await?;
                    }
                    _ => {
                        session
                            .send(s2c_packets::thong_bao("Bạn không đang học kỹ năng nào!")?)
                            .await?;
                    }
                }
            }
            Ok(NpcEffect::OpenHomePlant(d)) => {
                self.handle_open_home_plant(session, sid, player_id, npc_id, d, now_ms)
                    .await?;
            }
            Ok(NpcEffect::LaunchActivity("PRESS_SUGARCANE")) => {
                self.handle_press_sugarcane(session, sid, select == Some(1), now_ms)
                    .await?;
            }
            Ok(NpcEffect::LaunchActivity(_)) => {
                session
                    .send(s2c_packets::npc_open_say(
                        npc_id,
                        "Tính năng đang sẵn sàng phục vụ!",
                        None,
                    )?)
                    .await?;
            }
            Err(err) => {
                session
                    .send(s2c_packets::npc_open_say(npc_id, &err.to_string(), None)?)
                    .await?;
            }
        }
        Ok(())
    }
}
