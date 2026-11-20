//! NPC menu dialogue (`32`, `33`), `CON_MEO` confirms (`112`), Lucky Round (`-127`), and Achievements (`-76`).

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::{cmd_message, const_npc};
use nro_core::{GameRng, PlayerId};
use nro_economy::{
    CombineContext, CombineService, LuckyRoundMode, LuckyRoundService, NpcEffect, NpcKind,
    NpcMenuRouter, NpcPlayerCtx,
};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use tracing::info;

use super::{
    DbItemCatalog, ServerSessionHandler, con_meo_state, npc_menu_index, pending_combine,
    snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static DAILY_CHECKIN_DAY: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn daily_checkin_day() -> &'static DashMap<PlayerId, i64> { DAILY_CHECKIN_DAY.get_or_init(DashMap::new) }

impl ServerSessionHandler {
    pub(super) async fn handle_npc_menu(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: Option<u8>) -> Result<(), ProtocolError> {
        if npc_id == i16::from(const_npc::CON_MEO) {
            if let Some(sel) = select { return self.handle_con_meo_confirm(session, sid, sel).await; }
            return session.send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?).await;
        }
        if select.is_none() && let Some(task_say) = self.advance_task_on_npc(session, sid, npc_id).await? {
            return session.send(s2c_packets::npc_open_say(npc_id, &task_say, None)?).await;
        }
        let Some((pid, has_pet, cur_menu_idx)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            let pid = e.snapshot.id;
            (pid, e.snapshot.pet.is_some(), npc_menu_index().get(&pid).map_or(0, |v| *v.value()))
        }) else { return Ok(()); };

        if let Some(sel) = select && cur_menu_idx == const_npc::MENU_START_COMBINE {
            return self.execute_pending_combine(session, sid, npc_id, sel).await;
        }
        if matches!(npc_id, 0..=2) && !has_pet {
            match select {
                None => {
                    npc_menu_index().insert(pid, const_npc::BASE_MENU);
                    return session.send(s2c_packets::npc_open_menu(npc_id, "Con muốn nhận Đệ Tử đồng hành không?", &["Nói chuyện", "Nhận Đệ Tử", "Đóng"], None)?).await;
                }
                Some(0) => return session.send(s2c_packets::npc_open_say(npc_id, "Con cố gắng học thành tài, đừng lo cho ta.", None)?).await,
                Some(1) => {
                    npc_menu_index().remove(&pid);
                    return self.grant_starter_pet(session, sid).await;
                }
                Some(_) => {
                    npc_menu_index().remove(&pid);
                    if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; }
                    return Ok(());
                }
            }
        }
        if npc_id == i16::from(const_npc::BO_MONG) && select == Some(1) && cur_menu_idx == const_npc::BASE_MENU {
            return self.handle_achievement(session, sid, None).await;
        }
        if npc_id == i16::from(const_npc::BO_MONG) && select == Some(3) {
            let today = unix_now_ms() / 86_400_000;
            if daily_checkin_day().get(&pid).is_some_and(|d| *d.value() == today) {
                npc_menu_index().remove(&pid);
                return session.send(s2c_packets::npc_open_say(npc_id, "Hôm nay bạn đã điểm danh rồi!", None)?).await;
            }
        }
        if cur_menu_idx == const_npc::OPEN_POWER_PET && select == Some(0)
            && self.shared.clients.authenticated.get(&sid).is_some_and(|e| e.snapshot.pet.as_ref().is_none_or(|p| p.stats.limit_power >= 9))
        {
            npc_menu_index().remove(&pid);
            return session.send(s2c_packets::npc_open_say(npc_id, "Đệ tử đã đạt giới hạn sức mạnh tối đa!", None)?).await;
        }
        if matches!(npc_id, 18..=20) && select == Some(3) && cur_menu_idx == const_npc::BASE_MENU {
            return self.handle_lucky_round(session, sid, None, None).await;
        }
        let Some(npc) = NpcKind::from_repr(npc_id) else {
            info!(sid, npc_id, "unknown npc_id requested; sending fallback greeting");
            return session.send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?).await;
        };

        let eval = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let val = entry.value_mut();
            let player_id = val.snapshot.id;
            let mut inv = snapshot_to_economy_inv(&val.snapshot);
            let res = {
                let mut ctx = NpcPlayerCtx {
                    account_id: val.snapshot.account_id.unwrap_or_default(), player_id,
                    location: val.snapshot.location, gender: val.snapshot.gender,
                    c_flag: val.c_flag.max(0).cast_unsigned(), activated: val.activated,
                    has_clan: val.snapshot.clan_id.0 >= 0, is_clan_leader: false, clan_days: 1,
                    has_pet: val.snapshot.pet.is_some(), wearing_full_destroyer: false,
                    is_use_gtpt: false, is_phu_ho_mabu: false, mabu_points: 0, vnd_balance: 0, vip_purchases: 0,
                    kol_stage: val.snapshot.kol_progress.kol_quest_stage.clamp(1, 7) as u8,
                    kol_vip_stage: val.snapshot.kol_progress.kol_vip_quest_stage.clamp(1, 7) as u8,
                    kol_progress: 0, stats: &mut val.snapshot.stats, inventory: &mut inv,
                };
                match select {
                    None => NpcMenuRouter::open_base_menu(npc, &ctx),
                    Some(sel) => NpcMenuRouter::confirm_menu(npc, cur_menu_idx, sel, &mut ctx),
                }
            };
            let bag_changed = res.is_ok() && inv.bag != val.snapshot.items_bag;
            if res.is_ok() { sync_economy_inv_to_snapshot(&inv, &mut val.snapshot); }
            (
                player_id, val.snapshot.location.zone_id, res, bag_changed,
                s2c_packets::inventory_box(&val.snapshot.items_box),
                s2c_packets::inventory_bag(&val.snapshot.items_bag),
                wallet_money_pkt(&val.snapshot.wallet),
                s2c_packets::me_load_point(&val.snapshot.stats),
            )
        });
        let Some((player_id, cur_zone, res, bag_changed, box_pkt, bag_pkt, money_pkt, pt_pkt)) = eval else { return Ok(()); };

        if !matches!(res, Ok(NpcEffect::ShowMenu(_))) {
            npc_menu_index().remove(&player_id);
        }
        match res {
            Ok(NpcEffect::ShowMenu(prompt)) => {
                npc_menu_index().insert(player_id, prompt.menu_index);
                session.send(s2c_packets::npc_open_menu(npc_id, prompt.say, &prompt.options, None)?).await?;
            }
            Ok(NpcEffect::ChangeMap(target_map, tx, ty)) => {
                self.switch_player_zone(session, sid, Some(target_map), cur_zone, Some((tx, ty))).await?;
            }
            Ok(NpcEffect::Tutorial(text)) => {
                if bag_changed { session.send(bag_pkt).await?; }
                session.send(s2c_packets::npc_open_say(npc_id, text, None)?).await?;
            }
            Ok(NpcEffect::OpenItemBox) => {
                session.send(box_pkt).await?;
                session.send(s2c_packets::inventory_box_open(0)).await?;
            }
            Ok(NpcEffect::OpenCombine(op)) => {
                pending_combine().insert(player_id, (npc_id, op, Vec::new()));
                session.send(s2c_packets::combine_open(&format!("Chọn trang bị để thực hiện: {op}"), "Bà Hạt Mít")).await?;
            }
            Ok(NpcEffect::OpenMagicTree) => session.send(s2c_packets::magic_tree_menu(&["Thu hoạch", "Nâng cấp"])?).await?,
            Ok(NpcEffect::OpenShop(tag)) => self.handle_open_shop(session, sid, npc_id, tag).await?,
            Ok(NpcEffect::OpenConsignShop) => self.handle_open_consign(session, sid).await?,
            Ok(NpcEffect::OpenInput(kind)) => self.handle_open_input(session, sid, kind).await?,
            Ok(NpcEffect::NoOp) => session.send(money_pkt).await?,
            Ok(NpcEffect::ClaimedDailyCheckIn) => {
                daily_checkin_day().insert(player_id, unix_now_ms() / 86_400_000);
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
            }
            Ok(NpcEffect::UnlockedPowerLimit(true)) => {
                let pet_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if let Some(pet) = entry.snapshot.pet.as_mut() && pet.stats.limit_power < 9 {
                        pet.stats.limit_power += 1;
                        Ok((self.pkt_pet_detail(&entry.snapshot), wallet_money_pkt(&entry.snapshot.wallet)))
                    } else {
                        entry.snapshot.wallet.gold = entry.snapshot.wallet.gold.saturating_add(50_000_000);
                        Err(wallet_money_pkt(&entry.snapshot.wallet))
                    }
                });
                match pet_res {
                    Some(Ok((pet_pkt, mp))) => {
                        if let Some(p) = pet_pkt { session.send(p).await?; }
                        session.send(mp).await?;
                    }
                    Some(Err(mp)) => {
                        session.send(mp).await?;
                        session.send(s2c_packets::npc_open_say(npc_id, "Đệ tử đã đạt giới hạn sức mạnh tối đa!", None)?).await?;
                    }
                    None => {}
                }
            }
            Ok(
                NpcEffect::UnlockedPowerLimit(false) | NpcEffect::PurchasedVip(_)
                | NpcEffect::ApplyGtpt | NpcEffect::ApplyMabuBlessing | NpcEffect::ExtendKilisBuff
                | NpcEffect::ApplyBlackBallBuff(_, _) | NpcEffect::ClaimedKolStage(_, _) | NpcEffect::CancelledSkillStudy,
            ) => {
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
            }
            Ok(NpcEffect::OpenHomePlant(_) | NpcEffect::LaunchActivity(_)) => {
                session.send(s2c_packets::npc_open_say(npc_id, "Tính năng đang sẵn sàng phục vụ!", None)?).await?;
            }
            Err(err) => session.send(s2c_packets::npc_open_say(npc_id, &err.to_string(), None)?).await?,
        }
        Ok(())
    }

    async fn execute_pending_combine(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: u8) -> Result<(), ProtocolError> {
        let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
        npc_menu_index().remove(&pid);
        if select != 0 {
            if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; }
            return Ok(());
        }
        let Some((comb_npc, op, slots)) = pending_combine().get(&pid).map(|v| v.value().clone()) else {
            return session.send(s2c_packets::npc_open_say(npc_id, "Chưa chọn vật phẩm nâng cấp!", None)?).await;
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            let ctx = CombineContext {
                gender: entry.snapshot.gender, power: entry.snapshot.stats.power, wearing_full_destroyer: false,
                skill_point: 0, skill_mastery: 0, parity: self.shared.config.parity_mode,
            };
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            match CombineService::execute(&mut inv, op, &slots, &ctx, &catalog, &mut rng) {
                Ok(out) => {
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    let kept: Vec<i8> = slots.iter().copied()
                        .filter(|&s| entry.snapshot.items_bag.get(s).and_then(Option::as_ref).is_some())
                        .filter_map(|s| i8::try_from(s).ok()).collect();
                    Ok([
                        s2c_packets::combine_result(if out.success { 2 } else { 3 }),
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        wallet_money_pkt(&entry.snapshot.wallet),
                        s2c_packets::combine_items(&kept),
                    ])
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

    pub(super) async fn handle_intrinsic_open(&self, session: &SessionHandle, sid: u64, sub: i8) -> Result<(), ProtocolError> {
        if sub == 1 { return session.send(self.build_intrinsic_catalog_pkt()?).await; }
        let Some((pid, intr)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.intrinsic)) else { return Ok(()); };
        if let Some(rec) = self.shared.templates.intrinsics.iter().find(|r| r.id == i32::from(intr.intrinsic_id)) {
            session.send(s2c_packets::intrinsic_info(rec.icon as i16, &rec.name.replacen('#', &intr.param1.to_string(), 1))?).await?;
        } else {
            session.send(s2c_packets::intrinsic_info(0, "Chưa mở nội tại")?).await?;
        }
        con_meo_state().insert(pid, const_npc::INTRINSIC);
        session.send(s2c_packets::npc_open_menu(
            i16::from(const_npc::CON_MEO), "Bạn muốn thực hiện thao tác Nội Tại nào?",
            &["Xem tất cả\nNội Tại", "Mở Nội Tại\n(10Tr Vàng)", "Mở VIP\n(100 Ngọc)", "Đóng"], None,
        )?).await
    }

    pub(super) async fn handle_con_meo_confirm(&self, session: &SessionHandle, sid: u64, select: u8) -> Result<(), ProtocolError> {
        let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
        let mode = con_meo_state().remove(&pid).map_or(const_npc::INTRINSIC, |(_, v)| v);
        if mode == const_npc::CONFIRM_LEAVE_CLAN {
            return if select == 0 { self.execute_leave_clan(session, sid).await } else { Ok(()) };
        }
        match select {
            0 => session.send(self.build_intrinsic_catalog_pkt()?).await?,
            1 | 2 => {
                let is_vip = select == 2;
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let g_i16 = i16::from(i8::from(entry.snapshot.gender));
                    let pool: Vec<_> = self.shared.templates.intrinsics.iter().filter(|r| r.gender == g_i16 || r.gender == 3).collect();
                    if pool.is_empty() { return Err("Chưa có dữ liệu Nội Tại!"); }
                    if is_vip {
                        if entry.snapshot.wallet.gem < 100 { return Err("Cần 100 ngọc xanh để mở Nội Tại VIP!"); }
                        entry.snapshot.wallet.gem -= 100;
                    } else {
                        if entry.snapshot.wallet.gold < 10_000_000 { return Err("Cần 10,000,000 vàng để mở Nội Tại!"); }
                        entry.snapshot.wallet.gold -= 10_000_000;
                    }
                    let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
                    let chosen = pool[rng.next_range(0, (pool.len() - 1) as i32) as usize];
                    let p1 = rng.next_range(chosen.param_from_1, chosen.param_to_1.max(chosen.param_from_1)) as i16;
                    entry.snapshot.intrinsic.intrinsic_id = chosen.id as i8;
                    entry.snapshot.intrinsic.param1 = p1;
                    let info = chosen.name.replacen('#', &p1.to_string(), 1);
                    Ok((chosen.icon as i16, info, wallet_money_pkt(&entry.snapshot.wallet)))
                });
                match res {
                    Some(Ok((icon, info, money_pkt))) => {
                        session.send(s2c_packets::intrinsic_info(icon, &info)?).await?;
                        session.send(money_pkt).await?;
                        session.send(s2c_packets::thong_bao(&format!("Đã mở Nội Tại: {info}"))?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => {}
                }
            }
            _ => { if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; } }
        }
        Ok(())
    }

    fn build_intrinsic_catalog_pkt(&self) -> Result<Packet, ProtocolError> {
        let mut w = PacketWriter::with_capacity(1024);
        w.write_i8(1);
        let tabs = [(0_i16, "Trái Đất\nNội Tại"), (1, "Namếc\nNội Tại"), (2, "Xayda\nNội Tại")];
        w.write_i8(tabs.len() as i8);
        for (g, title) in tabs {
            w.write_utf(title)?;
            let cnt = self.shared.templates.intrinsics.iter().filter(|r| r.gender == g).take(60).count();
            w.write_i8(cnt as i8);
            for r in self.shared.templates.intrinsics.iter().filter(|r| r.gender == g).take(60) {
                w.write_i16(r.icon as i16);
                let (lo, hi) = (r.param_from_1, r.param_to_1);
                w.write_utf(&r.name.replacen('#', &format!("{lo}..{hi}"), 1))?;
            }
        }
        Ok(w.finish(cmd_message::SPEACIAL_SKILL))
    }

    pub(super) async fn handle_lucky_round(&self, session: &SessionHandle, sid: u64, _ty: Option<i8>, opt_cnt: Option<i8>) -> Result<(), ProtocolError> {
        let Some(cnt) = opt_cnt else {
            let mut w = PacketWriter::with_capacity(24);
            w.write_i8(0);
            w.write_i8(7);
            for b in 419_i16..=425 { w.write_i16(b); }
            w.write_i8(1);
            w.write_i32(4);
            w.write_i16(821);
            return session.send(w.finish(cmd_message::LUCKY_ROUND)).await;
        };
        let n = cnt.clamp(1, 7).cast_unsigned();
        let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            if inv.empty_bag_slots() < usize::from(n) { return Err("Hành trang đã đầy!".to_owned()); }
            let prev_len = inv.lucky_round_box.len();
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let spun = LuckyRoundService::spin(&mut inv, LuckyRoundMode::StandardGem, n, &mut rng)
                .map_err(|err| format!("Vòng quay: {err}"))?;
            let mut icons = Vec::with_capacity(usize::from(spun));
            let drained: Vec<_> = inv.lucky_round_box.drain(prev_len..).flatten().collect();
            for item in drained {
                let tpl = self.shared.templates.items.iter().find(|t| t.id == i32::from(item.template_id.0));
                let icon = tpl.map_or(420_i16, |t| t.icon_id as i16);
                inv.add_to_bag(item, tpl.is_some_and(|t| t.is_up_to_up)).map_err(|_| "Hành trang đã đầy!".to_owned())?;
                icons.push(icon);
            }
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            let mut w = PacketWriter::with_capacity(2 + icons.len() * 2);
            w.write_i8(1);
            w.write_i8(icons.len() as i8);
            for icon in icons { w.write_i16(icon); }
            Ok([w.finish(cmd_message::LUCKY_ROUND), s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)])
        });
        match res {
            Some(Ok(pkts)) => { for p in pkts { session.send(p).await?; } }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
            None => {}
        }
        Ok(())
    }

    pub(super) async fn handle_achievement(&self, session: &SessionHandle, sid: u64, claim_idx: Option<i8>) -> Result<(), ProtocolError> {
        if let Some(raw_idx) = claim_idx {
            let Ok(idx) = usize::try_from(raw_idx) else { return Ok(()); };
            if let Some((ack, money)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                let &(ach_id, _, _, ruby_rw, req) = self.shared.templates.achievements.get(idx)?;
                while entry.snapshot.achievements.len() <= idx { entry.snapshot.achievements.push((0, false)); }
                let met = if matches!(ach_id, 1 | 2 | 17) {
                    entry.snapshot.stats.power >= req
                } else {
                    entry.snapshot.achievements.get(idx).map_or(0, |a| a.0) >= req
                };
                if entry.snapshot.achievements[idx].1 || !met { return None; }
                entry.snapshot.achievements[idx].1 = true;
                entry.snapshot.wallet.ruby = entry.snapshot.wallet.ruby.saturating_add(ruby_rw.max(1));
                let mut w = PacketWriter::with_capacity(2);
                w.write_i8(1);
                w.write_u8(idx as u8);
                Some((w.finish(cmd_message::ARCHIVEMENT), wallet_money_pkt(&entry.snapshot.wallet)))
            }) {
                session.send(ack).await?;
                session.send(money).await?;
            }
            return Ok(());
        }
        let Some((pwr, achs)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.stats.power, e.snapshot.achievements.clone())) else { return Ok(()); };
        let count = self.shared.templates.achievements.len().min(50);
        if count == 0 { return session.send(s2c_packets::thong_bao("Chưa có dữ liệu thành tích!")?).await; }
        let mut w = PacketWriter::with_capacity(4 + count * 36);
        w.write_i8(0);
        w.write_i8(count as i8);
        for (i, (ach_id, i1, i2, ruby, req)) in self.shared.templates.achievements.iter().take(50).enumerate() {
            let rec = achs.get(i).copied().unwrap_or((0, false));
            let met = if matches!(*ach_id, 1 | 2 | 17) { pwr >= *req } else { rec.0 >= *req };
            w.write_utf(i1)?;
            w.write_utf(i2)?;
            w.write_i16((*ruby).clamp(0, i32::from(i16::MAX)) as i16);
            w.write_bool(met);
            w.write_bool(rec.1);
        }
        session.send(w.finish(cmd_message::ARCHIVEMENT)).await
    }

    pub(super) fn money_pkt_for(&self, sid: u64) -> Option<Packet> {
        self.shared.clients.authenticated.get(&sid).map(|e| wallet_money_pkt(&e.snapshot.wallet))
    }
}
