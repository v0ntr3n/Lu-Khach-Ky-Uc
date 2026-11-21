//! HomePlant (Trứng Mabư / Dưa Hấu), Shenron wishes, sugarcane pressing, and pending combine execution.

use nro_consts::{const_item, const_npc};
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, ItemOption, PlayerId, TemplateId};
use nro_db::{
    StrBool, StrNum,
    json_columns::{EggTimer, PetSnapshot, PlayerBadgeRecord, SkillSlot},
    player_repo::{LIMIT_GOLD, PlayerSnapshot},
};
use nro_economy::{CombineContext, CombineOp, CombineService, MabuEggState};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use smallvec::{SmallVec, smallvec};

use super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, npc_menu_index, pending_combine,
    router_npc::is_wearing_full_destroyer, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

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
        &self, session: &SessionHandle, sid: u64, player_id: PlayerId, npc_id: i16, is_dua_hau: bool, now_ms: i64,
    ) -> Result<(), ProtocolError> {
        let egg = self.shared.clients.authenticated.get(&sid)
            .and_then(|e| if is_dua_hau { e.snapshot.dua_hau_egg } else { e.snapshot.mabu_egg });
        if is_dua_hau {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session.send(s2c_packets::npc_open_say(npc_id, "Bạn chưa trồng Dưa Hấu nào!", None)?).await;
            };
            let rem_sec = egg.last_time_create_ms.saturating_add(egg.time_done_ms).saturating_sub(now_ms).max(0) / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CO_THE_THU_HOACH));
                session.send(s2c_packets::npc_open_menu(npc_id, "Dưa Hấu đã chín, bạn có thể thu hoạch!", &["Thu hoạch", "Đóng"], None)?).await
            } else {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CHUA_THE_THU_HOACH));
                session.send(s2c_packets::npc_open_menu(npc_id, &format!("Dưa Hấu đang lớn, còn {rem_sec} giây nữa mới chín."), &["Đóng"], None)?).await
            }
        } else {
            let Some(egg) = egg else {
                npc_menu_index().remove(&player_id);
                return session.send(s2c_packets::npc_open_say(npc_id, "Bạn chưa có Trứng Mabư nào!", None)?).await;
            };
            let rem_sec = egg.last_time_create_ms.saturating_add(egg.time_done_ms).saturating_sub(now_ms).max(0) / 1_000;
            if rem_sec == 0 {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CAN_OPEN_EGG));
                session.send(s2c_packets::npc_open_menu(npc_id, "Trứng Mabư đã sẵn sàng nở!", &["Nở trứng", "Đóng"], None)?).await
            } else {
                npc_menu_index().insert(player_id, (npc_id, const_npc::CAN_NOT_OPEN_EGG));
                session.send(s2c_packets::npc_open_menu(npc_id, &format!("Trứng Mabư còn {rem_sec} giây nữa mới nở."), &["Nở ngay (1 Tỷ vàng)", "Đóng"], None)?).await
            }
        }
    }

    pub(super) async fn handle_home_plant_confirm(
        &self, session: &SessionHandle, sid: u64, pid: PlayerId, npc_id: i16, cur_menu_idx: i32, select: u8,
    ) -> Result<(), ProtocolError> {
        npc_menu_index().remove(&pid);
        let now_ms = unix_now_ms();
        if npc_id == i16::from(const_npc::QUA_TRUNG) {
            if cur_menu_idx == const_npc::CAN_NOT_OPEN_EGG && select == 0 {
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!".to_owned()); }
                    let Some(egg) = entry.snapshot.mabu_egg else { return Err("Bạn không có Trứng Mabư nào!".to_owned()); };
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
                    let had_pet = entry.snapshot.pet.is_some();
                    if let Some(pet) = entry.snapshot.pet.as_mut() {
                        pet.type_pet = 1;
                        "$Mabư".clone_into(&mut pet.name);
                    } else {
                        let g = entry.snapshot.gender;
                        let first_sk = match g { Gender::TraiDat => 0, Gender::Namec => 2, Gender::Xayda => 4 };
                        let mut skills = vec![SkillSlot { template_id: first_sk, point: 1, last_time_use_ms: 0, curr_level: 0 }];
                        skills.resize(4, SkillSlot { template_id: -1, point: 0, last_time_use_ms: 0, curr_level: 0 });
                        entry.snapshot.pet = Some(PetSnapshot {
                            type_pet: 1, gender: g, name: "$Mabư".to_owned(), fusion_type: 0, fusion_time_left_ms: 0, status: 0, items_body: vec![None; 7], skills,
                            stats: CoreStats {
                                power: 2_000, tiem_nang: 2_000, stamina: 1_000, max_stamina: 1_000, hpg: 1_000, mpg: 1_000, dameg: 45, defg: 10, critg: 2,
                                hp: 1_000, mp: 1_000, hp_max: 1_000, mp_max: 1_000, dame: 45, def: 10, crit: 2, speed: 5, ..CoreStats::default()
                            },
                        });
                    }
                    entry.snapshot.mabu_egg = None;
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    Some(Ok((!had_pet, self.pkt_pet_detail(&entry.snapshot), s2c_packets::me_load_point(&entry.snapshot.stats))))
                });
                match hatched {
                    Some(Ok((is_new, pet_pkt, pt_pkt))) => {
                        if is_new { session.send(s2c_packets::have_pet(true)).await?; }
                        if let Some(p) = pet_pkt { session.send(p).await?; }
                        session.send(pt_pkt).await?;
                        session.send(s2c_packets::thong_bao("Chúc mừng! Trứng Mabư đã nở thành công!")?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => session.send(s2c_packets::thong_bao("Bạn không có Trứng Mabư nào!")?).await?,
                }
                return Ok(());
            }
        } else if npc_id == i16::from(const_npc::DUA_HAU) && cur_menu_idx == const_npc::CO_THE_THU_HOACH && select == 0 {
            let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!".to_owned()); }
                let Some(egg) = entry.snapshot.dua_hau_egg else { return Err("Bạn chưa trồng Dưa Hấu nào!".to_owned()); };
                if egg.last_time_create_ms.saturating_add(egg.time_done_ms) > now_ms {
                    return Err("Dưa Hấu chưa chín!".to_owned());
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                inv.add_to_bag(
                    ItemInstance { template_id: TemplateId(const_item::DUA_HAU as i16), quantity: 1, options: SmallVec::new(), create_time_ms: now_ms },
                    true,
                ).map_err(|_| "Hành trang đã đầy!".to_owned())?;
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

    pub(super) async fn handle_dragon_wish_confirm(
        &self, session: &SessionHandle, sid: u64, pid: PlayerId, npc_id: i16, cur_menu_idx: i32, select: u8,
    ) -> Result<(), ProtocolError> {
        if cur_menu_idx == const_npc::SHENRON_1_1 && select == 3 {
            npc_menu_index().insert(pid, (npc_id, const_npc::SHENRON_1_2));
            return session.send(s2c_packets::npc_open_menu(
                npc_id,
                "Hãy chọn điều ước khác:",
                &["Giàu có (+2 tỷ vàng)", "Giàu có (+10.000 ngọc)", "Tăng 200tr Sức mạnh & Tiềm năng", "Đóng"],
                None,
            )?).await;
        }
        npc_menu_index().remove(&pid);
        if (cur_menu_idx == const_npc::SHENRON_1_2 && select >= 3)
            || !matches!(cur_menu_idx, const_npc::SHENRON_1_1 | const_npc::SHENRON_1_2 | const_npc::SHENRON_NAMEK_CONFIRM)
        {
            return Ok(());
        }
        let is_namek = npc_id == i16::from(const_npc::RONG_THIENG_NAMEC);
        let now_ms = unix_now_ms();
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!"); }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let balls = if is_namek && (353..=359).all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1) {
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
                    let av = match entry.snapshot.gender { Gender::TraiDat => 227, Gender::Namec => 228, Gender::Xayda => 229 };
                    let opt_p = 10 + (rng.next_u64() % 6) as i32;
                    inv.add_to_bag(ItemInstance { template_id: TemplateId(av), quantity: 1, options: smallvec![ItemOption { id: 5, param: opt_p }], create_time_ms: now_ms }, false)
                        .map_err(|_| "Hành trang đã đầy!")?;
                }
                (const_npc::SHENRON_1_1 | const_npc::SHENRON_NAMEK_CONFIRM, 1) => {
                    let Some(gloves) = inv.body.get_mut(2).and_then(Option::as_mut) else {
                        return Err("Bạn chưa mang găng tay trên người!");
                    };
                    if gloves.options.iter().any(|o| o.id == 72 && o.param >= 7) {
                        return Err("Găng tay đã đạt cấp tối đa (+7)!");
                    }
                    if let Some(o) = gloves.options.iter_mut().find(|o| o.id == 72) { o.param += 1; } else { gloves.options.push(ItemOption { id: 72, param: 1 }); }
                    for o in &mut gloves.options {
                        if matches!(o.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) { o.param = o.param.saturating_add(o.param / 10); }
                    }
                    stats_dirty = true;
                }
                (const_npc::SHENRON_1_1 | const_npc::SHENRON_NAMEK_CONFIRM, 2) => {
                    let Some(pet) = entry.snapshot.pet.as_mut() else { return Err("Bạn chưa có đệ tử!"); };
                    if pet.skills.len() < 3 { pet.skills.resize(4, SkillSlot { template_id: -1, point: 0, last_time_use_ms: 0, curr_level: 0 }); }
                    pet.skills[1] = SkillSlot { template_id: [1, 3, 5][(rng.next_u64() % 3) as usize], point: 1, last_time_use_ms: 0, curr_level: 0 };
                    pet.skills[2] = SkillSlot { template_id: [6, 9, 12][(rng.next_u64() % 3) as usize], point: 1, last_time_use_ms: 0, curr_level: 0 };
                    if let Some(p) = self.pkt_pet_detail(&entry.snapshot) { extra_pkts.push(p); }
                }
                (const_npc::SHENRON_1_2, 0) => inv.gold = inv.gold.saturating_add(2_000_000_000).min(LIMIT_GOLD.max(0).cast_unsigned()),
                (const_npc::SHENRON_1_2, 1) => inv.gem = inv.gem.saturating_add(10_000),
                (const_npc::SHENRON_1_2, 2) => {
                    entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(200_000_000);
                    entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(200_000_000);
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    stats_dirty = true;
                }
                _ => return Err("Điều ước không hợp lệ!"),
            }
            for b in balls { inv.sub_template_from_bag(TemplateId(b), 1).map_err(|_| "Không đủ 7 viên Ngọc Rồng!")?; }
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            if stats_dirty {
                super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                let (eh, _, _, _, _) = effective_appearance(&self.shared.templates, &entry.snapshot);
                extra_pkts.push(s2c_packets::inventory_body(eh, &entry.snapshot.items_body));
                let loc = entry.snapshot.location;
                if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
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
                for p in pkts { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Điều ước của ngươi đã trở thành sự thật!")?).await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::npc_open_say(npc_id, msg, None)?).await?,
            None => {}
        }
        Ok(())
    }

    pub(super) async fn handle_press_sugarcane(&self, session: &SessionHandle, sid: u64, ten: bool, now_ms: i64) -> Result<(), ProtocolError> {
        let cups: u32 = if ten { 10 } else { 1 };
        let gold_cost: u64 = 5_000_000 * u64::from(cups);
        let res: Option<Result<(Packet, Packet), String>> = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!".to_owned()); }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            inv.sub_template_from_bag(TemplateId(1612), 5 * cups).map_err(|_| format!("Cần {} cây mía để ép {cups} ly nước mía!", 5 * cups))?;
            inv.sub_template_from_bag(TemplateId(1613), 2 * cups).map_err(|_| format!("Cần {} đá viên để ép {cups} ly nước mía!", 2 * cups))?;
            inv.sub_gold(gold_cost).map_err(|_| format!("Cần {gold_cost} vàng để ép {cups} ly nước mía!"))?;
            let mut rng = GameRng::from_seed(now_ms.cast_unsigned() ^ sid);
            for _ in 0..cups {
                let juice_id = 1614 + (rng.next_u64() % 3) as i16;
                inv.add_to_bag(ItemInstance { template_id: TemplateId(juice_id), quantity: 1, options: SmallVec::new(), create_time_ms: now_ms }, true)
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
            if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!".to_owned()); }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let (skill_point, skill_mastery) = special_skill_stats(&entry.snapshot);
            let ctx = CombineContext {
                gender: entry.snapshot.gender, power: entry.snapshot.stats.power,
                wearing_full_destroyer: is_wearing_full_destroyer(&entry.snapshot),
                skill_point, skill_mastery, parity: self.shared.config.parity_mode,
            };
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let exec_res = match batch_rolls {
                Some(rolls) => {
                    let Some(&slot) = slots.first() else { return Err("Chưa chọn trang bị cần pha lê hóa!".to_owned()); };
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
                            let prev = sk.point; sk.point = new_pt_i8; sk.curr_level = 0; prev <= 0
                        } else {
                            entry.snapshot.skills.push(SkillSlot { template_id: tid, point: new_pt_i8, last_time_use_ms: 0, curr_level: 0 });
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
                            entry.snapshot.badges.push(PlayerBadgeRecord { id_badges: StrNum(bid), time_of_use_badges_ms: StrNum(unix_now_ms()), is_use: StrBool(false) });
                        }
                        if let Ok(p) = s2c_packets::thong_bao(&format!("Chúc mừng! Bạn vừa mở khóa danh hiệu #{bid}!")) { extra_pkts.push(p); }
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
