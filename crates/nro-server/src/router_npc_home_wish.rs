//! Shenron dragon wishes and sugarcane pressing (`PRESS_SUGARCANE`).

use nro_consts::const_npc;
use nro_core::{GameRng, Gender, ItemInstance, ItemOption, PlayerId, TemplateId};
use nro_db::{json_columns::SkillSlot, player_repo::LIMIT_GOLD};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use smallvec::{SmallVec, smallvec};

use super::super::{
    ServerSessionHandler, effective_appearance, npc_menu_index, recalculate_session_stats,
    snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_dragon_wish_confirm(
        &self,
        session: &SessionHandle,
        sid: u64,
        pid: PlayerId,
        npc_id: i16,
        cur_menu_idx: i32,
        select: u8,
    ) -> Result<(), ProtocolError> {
        if cur_menu_idx == const_npc::SHENRON_1_1 && select == 3 {
            npc_menu_index().insert(pid, (npc_id, const_npc::SHENRON_1_2));
            return session
                .send(s2c_packets::npc_open_menu(
                    npc_id,
                    "Hãy chọn điều ước khác:",
                    &[
                        "Giàu có (+2 tỷ vàng)",
                        "Giàu có (+10.000 ngọc)",
                        "Tăng 200tr Sức mạnh & Tiềm năng",
                        "Đóng",
                    ],
                    None,
                )?)
                .await;
        }
        npc_menu_index().remove(&pid);
        if (cur_menu_idx == const_npc::SHENRON_1_2 && select >= 3)
            || !matches!(
                cur_menu_idx,
                const_npc::SHENRON_1_1 | const_npc::SHENRON_1_2 | const_npc::SHENRON_NAMEK_CONFIRM
            )
        {
            return Ok(());
        }
        let is_namek = npc_id == i16::from(const_npc::RONG_THIENG_NAMEC);
        let now_ms = unix_now_ms();
        let outcome = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return Err("Không thể thao tác khi đang giao dịch!");
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let balls = if is_namek
                    && (353..=359).all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1)
                {
                    353_i16..=359_i16
                } else if (14..=20).all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1) {
                    14_i16..=20_i16
                } else {
                    return Err("Cần đủ 7 viên Ngọc Rồng để thực hiện điều ước!");
                };
                let mut rng = GameRng::from_seed(now_ms.cast_unsigned() ^ sid);
                let mut extra_pkts: Vec<Packet> = Vec::new();
                let mut stats_dirty = false;
                match (cur_menu_idx, select) {
                    (const_npc::SHENRON_1_1 | const_npc::SHENRON_NAMEK_CONFIRM, 0) => {
                        let av = match entry.snapshot.gender {
                            Gender::TraiDat => 227,
                            Gender::Namec => 228,
                            Gender::Xayda => 229,
                        };
                        let opt_p = 10 + (rng.next_u64() % 6) as i32;
                        inv.add_to_bag(
                            ItemInstance {
                                template_id: TemplateId(av),
                                quantity: 1,
                                options: smallvec![ItemOption {
                                    id: 5,
                                    param: opt_p
                                }],
                                create_time_ms: now_ms,
                            },
                            false,
                        )
                        .map_err(|_| "Hành trang đã đầy!")?;
                    }
                    (const_npc::SHENRON_1_1 | const_npc::SHENRON_NAMEK_CONFIRM, 1) => {
                        let Some(gloves) = inv.body.get_mut(2).and_then(Option::as_mut) else {
                            return Err("Bạn chưa mang găng tay trên người!");
                        };
                        if gloves.options.iter().any(|o| o.id == 72 && o.param >= 7) {
                            return Err("Găng tay đã đạt cấp tối đa (+7)!");
                        }
                        if let Some(o) = gloves.options.iter_mut().find(|o| o.id == 72) {
                            o.param += 1;
                        } else {
                            gloves.options.push(ItemOption { id: 72, param: 1 });
                        }
                        for o in &mut gloves.options {
                            if matches!(o.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) {
                                o.param = o.param.saturating_add(o.param / 10);
                            }
                        }
                        stats_dirty = true;
                    }
                    (const_npc::SHENRON_1_1 | const_npc::SHENRON_NAMEK_CONFIRM, 2) => {
                        let Some(pet) = entry.snapshot.pet.as_mut() else {
                            return Err("Bạn chưa có đệ tử!");
                        };
                        if pet.skills.len() < 3 {
                            pet.skills.resize(
                                4,
                                SkillSlot {
                                    template_id: -1,
                                    point: 0,
                                    last_time_use_ms: 0,
                                    curr_level: 0,
                                },
                            );
                        }
                        pet.skills[1] = SkillSlot {
                            template_id: [1, 3, 5][(rng.next_u64() % 3) as usize],
                            point: 1,
                            last_time_use_ms: 0,
                            curr_level: 0,
                        };
                        pet.skills[2] = SkillSlot {
                            template_id: [6, 7, 8][(rng.next_u64() % 3) as usize],
                            point: 1,
                            last_time_use_ms: 0,
                            curr_level: 0,
                        };
                        crate::router_pet::recalculate_pet_in_snapshot(
                            &self.shared,
                            &mut entry.snapshot,
                        );
                        if let Some(p) = self.pkt_pet_detail(&entry.snapshot) {
                            extra_pkts.push(p);
                        }
                    }
                    (const_npc::SHENRON_1_2, 0) => {
                        inv.gold = inv
                            .gold
                            .saturating_add(2_000_000_000)
                            .min(LIMIT_GOLD.max(0).cast_unsigned());
                    }
                    (const_npc::SHENRON_1_2, 1) => inv.gem = inv.gem.saturating_add(10_000),
                    (const_npc::SHENRON_1_2, 2) => {
                        entry.snapshot.stats.power =
                            entry.snapshot.stats.power.saturating_add(200_000_000);
                        entry.snapshot.stats.tiem_nang =
                            entry.snapshot.stats.tiem_nang.saturating_add(200_000_000);
                        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                        stats_dirty = true;
                    }
                    _ => return Err("Điều ước không hợp lệ!"),
                }
                for b in balls {
                    inv.sub_template_from_bag(TemplateId(b), 1)
                        .map_err(|_| "Không đủ 7 viên Ngọc Rồng!")?;
                }
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                if stats_dirty {
                    recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                    let (eh, _, _, _, _) =
                        effective_appearance(&self.shared.templates, &entry.snapshot);
                    extra_pkts.push(s2c_packets::inventory_body(eh, &entry.snapshot.items_body));
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (
                        entry.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                }
                let mut pkts = vec![
                    s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    wallet_money_pkt(&entry.snapshot.wallet),
                    s2c_packets::me_load_point(&entry.snapshot.stats),
                ];
                pkts.extend(extra_pkts);
                Ok(pkts)
            });
        match outcome {
            Some(Ok(pkts)) => {
                for p in pkts {
                    session.send(p).await?;
                }
                session
                    .send(s2c_packets::thong_bao(
                        "Điều ước của ngươi đã trở thành sự thật!",
                    )?)
                    .await?;
            }
            Some(Err(msg)) => {
                session
                    .send(s2c_packets::npc_open_say(npc_id, msg, None)?)
                    .await?;
            }
            None => {}
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_press_sugarcane(
        &self,
        session: &SessionHandle,
        sid: u64,
        ten: bool,
        now_ms: i64,
    ) -> Result<(), ProtocolError> {
        let cups: u32 = if ten { 10 } else { 1 };
        let gold_cost: u64 = 5_000_000 * u64::from(cups);
        let res: Option<Result<(Packet, Packet), String>> = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return Err("Không thể thao tác khi đang giao dịch!".to_owned());
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                inv.sub_template_from_bag(TemplateId(1612), 5 * cups)
                    .map_err(|_| format!("Cần {} cây mía để ép {cups} ly nước mía!", 5 * cups))?;
                inv.sub_template_from_bag(TemplateId(1613), 2 * cups)
                    .map_err(|_| format!("Cần {} đá viên để ép {cups} ly nước mía!", 2 * cups))?;
                inv.sub_gold(gold_cost)
                    .map_err(|_| format!("Cần {gold_cost} vàng để ép {cups} ly nước mía!"))?;
                let mut rng = GameRng::from_seed(now_ms.cast_unsigned() ^ sid);
                for _ in 0..cups {
                    let juice_id = 1614 + (rng.next_u64() % 3) as i16;
                    inv.add_to_bag(
                        ItemInstance {
                            template_id: TemplateId(juice_id),
                            quantity: 1,
                            options: SmallVec::new(),
                            create_time_ms: now_ms,
                        },
                        true,
                    )
                    .map_err(|_| "Hành trang đã đầy!".to_owned())?;
                }
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                Ok((
                    s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    wallet_money_pkt(&entry.snapshot.wallet),
                ))
            });
        match res {
            Some(Ok((bag_pkt, money_pkt))) => {
                session.send(bag_pkt).await?;
                session.send(money_pkt).await?;
                session
                    .send(s2c_packets::thong_bao(&format!(
                        "Đã ép thành công {cups} ly nước mía!"
                    ))?)
                    .await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
            None => {}
        }
        Ok(())
    }
}
