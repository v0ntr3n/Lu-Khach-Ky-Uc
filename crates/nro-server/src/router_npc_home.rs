//! HomePlant (Trứng Mabư / Dưa Hấu), sugarcane pressing, and pending combine execution.

use nro_consts::{const_item, const_npc};
use nro_core::{GameRng, Gender, ItemInstance, PlayerId, TemplateId};
use nro_db::{
    StrBool, StrNum,
    json_columns::{EggTimer, PlayerBadgeRecord, SkillSlot},
    player_repo::PlayerSnapshot,
};
use nro_economy::{CombineContext, CombineOp, CombineService, MabuEggState};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use smallvec::SmallVec;

use super::{
    DbItemCatalog, ServerSessionHandler, npc_menu_index, pending_combine,
    router_npc::is_wearing_full_destroyer, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

const fn special_skill_template_id(gender: Gender) -> i32 {
    match gender {
        Gender::TraiDat => 24,
        Gender::Namec => 26,
        Gender::Xayda => 25,
    }
}

fn special_skill_stats(snap: &PlayerSnapshot) -> (u8, u16) {
    let tid = special_skill_template_id(snap.gender);
    snap.skills
        .iter()
        .find(|sk| sk.template_id == tid)
        .map_or((0, 0), |sk| (sk.point.max(0) as u8, sk.curr_level.max(0) as u16))
}

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
        let egg = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .and_then(|e| if is_dua_hau { e.snapshot.dua_hau_egg } else { e.snapshot.mabu_egg });
        if is_dua_hau {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session.send(s2c_packets::npc_open_say(npc_id, "Bạn chưa trồng Dưa Hấu nào!", None)?).await;
            };
            let rem_sec = egg.last_time_create_ms.saturating_add(egg.time_done_ms).saturating_sub(now_ms).max(0) / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, const_npc::CO_THE_THU_HOACH);
                session.send(s2c_packets::npc_open_menu(npc_id, "Dưa Hấu đã chín, bạn có thể thu hoạch!", &["Thu hoạch", "Đóng"], None)?).await
            } else {
                npc_menu_index().insert(player_id, const_npc::CHUA_THE_THU_HOACH);
                session.send(s2c_packets::npc_open_menu(npc_id, &format!("Dưa Hấu đang lớn, còn {rem_sec} giây nữa mới chín."), &["Đóng"], None)?).await
            }
        } else {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session.send(s2c_packets::npc_open_say(npc_id, "Bạn chưa có Trứng Mabư nào!", None)?).await;
            };
            let rem_sec = egg.last_time_create_ms.saturating_add(egg.time_done_ms).saturating_sub(now_ms).max(0) / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, const_npc::CAN_OPEN_EGG);
                session.send(s2c_packets::npc_open_menu(npc_id, "Trứng Mabư đã sẵn sàng nở!", &["Nở trứng", "Đóng"], None)?).await
            } else {
                npc_menu_index().insert(player_id, const_npc::CAN_NOT_OPEN_EGG);
                session.send(s2c_packets::npc_open_menu(npc_id, &format!("Trứng Mabư còn {rem_sec} giây nữa mới nở."), &["Nở ngay (1 Tỷ vàng)", "Đóng"], None)?).await
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
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let Some(egg) = entry.snapshot.mabu_egg else {
                        return Err("Bạn không có Trứng Mabư nào!".to_owned());
                    };
                    let mut egg_state = MabuEggState { ready_at_ms: egg.last_time_create_ms.saturating_add(egg.time_done_ms) };
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    egg_state.fast_hatch(&mut inv, now_ms).map_err(|err| format!("Không thể nở nhanh: {err}"))?;
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    entry.snapshot.mabu_egg = Some(EggTimer { last_time_create_ms: now_ms, time_done_ms: 0 });
                    Ok(wallet_money_pkt(&entry.snapshot.wallet))
                });
                match res {
                    Some(Ok(mp)) => {
                        session.send(mp).await?;
                        session.send(s2c_packets::thong_bao("Trứng Mabư đã sẵn sàng nở!")?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
                    None => {}
                }
                return Ok(());
            }
            if cur_menu_idx == const_npc::CAN_OPEN_EGG && select == 0 {
                let hatched = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                    let egg = entry.snapshot.mabu_egg?;
                    if egg.last_time_create_ms.saturating_add(egg.time_done_ms) > now_ms {
                        return Some(Err("Trứng Mabư chưa sẵn sàng nở!"));
                    }
                    entry.snapshot.mabu_egg = None;
                    Some(Ok(()))
                });
                match hatched {
                    Some(Ok(())) => session.send(s2c_packets::thong_bao("Chúc mừng! Trứng Mabư đã nở thành công!")?).await?,
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => session.send(s2c_packets::thong_bao("Bạn không có Trứng Mabư nào!")?).await?,
                }
                return Ok(());
            }
        } else if npc_id == i16::from(const_npc::DUA_HAU)
            && cur_menu_idx == const_npc::CO_THE_THU_HOACH
            && select == 0
        {
            let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
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
                Ok(s2c_packets::inventory_bag(&entry.snapshot.items_bag))
            });
            match res {
                Some(Ok(bag_pkt)) => {
                    session.send(bag_pkt).await?;
                    session.send(s2c_packets::thong_bao("Đã thu hoạch Dưa Hấu thành công!")?).await?;
                }
                Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
                None => {}
            }
            return Ok(());
        }
        if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; }
        Ok(())
    }

    pub(super) async fn handle_press_sugarcane(&self, session: &SessionHandle, sid: u64, ten: bool, now_ms: i64) -> Result<(), ProtocolError> {
        let cups: u32 = if ten { 10 } else { 1 };
        let gold_cost: u64 = 5_000_000 * u64::from(cups);
        let res: Option<Result<(Packet, Packet), String>> = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            inv.sub_gold(gold_cost).map_err(|_| format!("Cần {gold_cost} vàng để ép {cups} ly nước mía!"))?;
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
            Ok((s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)))
        });
        match res {
            Some(Ok((bag_pkt, money_pkt))) => {
                session.send(bag_pkt).await?;
                session.send(money_pkt).await?;
                session.send(s2c_packets::thong_bao(&format!("Đã ép thành công {cups} ly nước mía!"))?).await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
            None => {}
        }
        Ok(())
    }

    pub(super) async fn execute_pending_combine(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: u8) -> Result<(), ProtocolError> {
        let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
        npc_menu_index().remove(&pid);
        if super::router_shop::is_in_trade(pid) {
            return session.send(s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!")?).await;
        }
        let Some((comb_npc, op, slots)) = pending_combine().get(&pid).map(|v| v.value().clone()) else {
            if select != 0 {
                if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; }
                return Ok(());
            }
            return session.send(s2c_packets::npc_open_say(npc_id, "Chưa chọn vật phẩm nâng cấp!", None)?).await;
        };
        let batch_rolls: Option<u8> = match (op, select) {
            (_, 0) => None,
            (CombineOp::PhaLeHoaTrangBi, 1) => Some(10),
            (CombineOp::PhaLeHoaTrangBi, 2) => Some(100),
            _ => {
                if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; }
                return Ok(());
            }
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) {
                return Err("Không thể thao tác khi đang giao dịch!".to_owned());
            }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let (skill_point, skill_mastery) = special_skill_stats(&entry.snapshot);
            let ctx = CombineContext {
                gender: entry.snapshot.gender,
                power: entry.snapshot.stats.power,
                wearing_full_destroyer: is_wearing_full_destroyer(&entry.snapshot),
                skill_point,
                skill_mastery,
                parity: self.shared.config.parity_mode,
            };
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let exec_res = match batch_rolls {
                Some(rolls) => {
                    let Some(&slot) = slots.first() else {
                        return Err("Chưa chọn trang bị cần pha lê hóa!".to_owned());
                    };
                    CombineService::execute_vip_batch(&mut inv, slot, rolls, &ctx, &catalog, &mut rng)
                }
                None => CombineService::execute(&mut inv, op, &slots, &ctx, &catalog, &mut rng),
            };
            match exec_res {
                Ok(out) => {
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    let mut extra_pkts: Vec<Packet> = Vec::new();
                    if let Some(new_pt) = out.new_skill_point {
                        let tid = special_skill_template_id(entry.snapshot.gender);
                        let new_pt_i8 = new_pt.min(9) as i8;
                        let was_new = if let Some(sk) = entry.snapshot.skills.iter_mut().find(|s| s.template_id == tid) {
                            let prev = sk.point;
                            sk.point = new_pt_i8;
                            sk.curr_level = 0;
                            prev <= 0
                        } else {
                            entry.snapshot.skills.push(SkillSlot {
                                template_id: tid,
                                point: new_pt_i8,
                                last_time_use_ms: 0,
                                curr_level: 0,
                            });
                            true
                        };
                        if let Some(st) = self.shared.templates.skills.iter().find(|s| i32::from(s.id) == tid)
                            && let Some(lv) = st.levels.iter().find(|l| l.point == new_pt_i8).or_else(|| st.levels.first())
                        {
                            extra_pkts.push(if was_new { s2c_packets::me_add_skill(lv.id) } else { s2c_packets::me_update_skill(lv.id) });
                        }
                    }
                    if let Some(badge_id) = out.badge_awarded {
                        let bid = i32::from(badge_id);
                        if !entry.snapshot.badges.iter().any(|b| b.id_badges.0 == bid) {
                            entry.snapshot.badges.push(PlayerBadgeRecord {
                                id_badges: StrNum(bid),
                                time_of_use_badges_ms: StrNum(unix_now_ms()),
                                is_use: StrBool(false),
                            });
                        }
                        if let Ok(p) = s2c_packets::thong_bao(&format!("Chúc mừng! Bạn vừa mở khóa danh hiệu #{bid}!")) {
                            extra_pkts.push(p);
                        }
                    }
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    let kept: Vec<i8> = slots.iter().copied()
                        .filter(|&s| entry.snapshot.items_bag.get(s).and_then(Option::as_ref).is_some())
                        .filter_map(|s| i8::try_from(s).ok()).collect();
                    let mut pkts = vec![
                        s2c_packets::combine_result(if out.success { 2 } else { 3 }),
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        wallet_money_pkt(&entry.snapshot.wallet),
                        s2c_packets::combine_items(&kept),
                    ];
                    pkts.extend(extra_pkts);
                    Ok(pkts)
                }
                Err(err) => Err(format!("Không thể kết hợp: {err}")),
            }
        });
        match outcome {
            Some(Ok(pkts)) => { for p in pkts { session.send(p).await?; } }
            Some(Err(msg)) => session.send(s2c_packets::npc_open_say(comb_npc, &msg, None)?).await?,
            None => {}
        }
        Ok(())
    }
}
