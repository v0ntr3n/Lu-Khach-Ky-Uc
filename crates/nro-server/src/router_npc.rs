//! NPC menu dialogue (`32`, `33`), `CON_MEO` confirms (`112`), Lucky Round (`-127`), and Achievements (`-76`).

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::{cmd_message, const_item, const_npc};
use nro_core::{ClanId, GameRng, Gender, ItemInstance, PlayerId, TemplateId};
use nro_db::{
    StrBool, StrNum,
    json_columns::{EggTimer, KolProgressData, PlayerBadgeRecord, SkillSlot},
    player_repo::PlayerSnapshot,
};
use nro_economy::{
    CombineContext, CombineOp, CombineService, LuckyRoundMode, LuckyRoundService, MabuEggState,
    NpcEffect, NpcKind, NpcMenuRouter, NpcPlayerCtx,
};
use nro_player::progression::{INTRINSIC_GOLD_MILLIONS, INTRINSIC_MIN_POWER};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use smallvec::SmallVec;
use tracing::info;

use super::{
    DbItemCatalog, ServerSessionHandler, con_meo_state, npc_menu_index, pending_combine,
    snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static DAILY_CHECKIN_DAY: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn daily_checkin_day() -> &'static DashMap<PlayerId, i64> { DAILY_CHECKIN_DAY.get_or_init(DashMap::new) }

fn is_wearing_full_destroyer(snap: &PlayerSnapshot) -> bool {
    snap.items_body.len() >= 5
        && snap.items_body[..5]
            .iter()
            .all(|slot| slot.as_ref().is_some_and(|it| (650..=662).contains(&it.template_id.0)))
}

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

fn current_kol_progress(kol: &KolProgressData) -> u32 {
    let raw: i64 = match kol.kol_quest_stage {
        2 => i64::from(kol.destron_gas_70_completion_count),
        3 => i64::from(kol.martial_arts_tournament_wins),
        4 => i64::from(kol.daily_super_hard_quest_completion_count),
        5 => i64::from(kol.boss_baby_defeat_participation_count),
        6 | 7 => kol.monster_kill_count_auto_train,
        _ => i64::from(kol.destron_gas_70_completion_count)
            .saturating_add(i64::from(kol.martial_arts_tournament_wins))
            .saturating_add(i64::from(kol.daily_super_hard_quest_completion_count))
            .saturating_add(i64::from(kol.boss_baby_defeat_participation_count))
            .saturating_add(kol.monster_kill_count_auto_train),
    };
    u32::try_from(raw.max(0)).unwrap_or(u32::MAX)
}

impl ServerSessionHandler {
    pub(super) async fn handle_npc_menu(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: Option<u8>) -> Result<(), ProtocolError> {
        if npc_id == i16::from(const_npc::CON_MEO) {
            if let Some(sel) = select { return self.handle_con_meo_confirm(session, sid, sel).await; }
            return session.send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?).await;
        }
        if select.is_none() && let Some(task_say) = self.advance_task_on_npc(session, sid, npc_id).await? {
            return session.send(s2c_packets::npc_open_say(npc_id, &task_say, None)?).await;
        }
        let Some((pid, clan_id, has_pet, cur_menu_idx)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            let pid = e.snapshot.id;
            (pid, e.snapshot.clan_id, e.snapshot.pet.is_some(), npc_menu_index().get(&pid).map_or(0, |v| *v.value()))
        }) else { return Ok(()); };

        if let Some(sel) = select && cur_menu_idx == const_npc::MENU_START_COMBINE {
            return self.execute_pending_combine(session, sid, npc_id, sel).await;
        }
        if let Some(sel) = select
            && (npc_id == i16::from(const_npc::QUA_TRUNG) || npc_id == i16::from(const_npc::DUA_HAU))
        {
            return self.handle_home_plant_confirm(session, sid, pid, npc_id, cur_menu_idx, sel).await;
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
        if npc_id == i16::from(const_npc::BO_MONG) && select == Some(3) && cur_menu_idx == const_npc::BASE_MENU {
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

        let is_clan_leader = if clan_id.0 >= 0 {
            self.shared.clans_repo.load_all().await.ok().is_some_and(|clans| {
                clans.iter().any(|c| c.id == clan_id && c.members.iter().any(|m| m.id == pid.0 as i32 && m.role == 0))
            })
        } else {
            false
        };
        let now_ms = unix_now_ms();

        let eval = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let val = entry.value_mut();
            let player_id = val.snapshot.id;
            let wearing_full_destroyer = is_wearing_full_destroyer(&val.snapshot);
            let kol_progress = current_kol_progress(&val.snapshot.kol_progress);
            let is_use_gtpt = val.snapshot.item_time.gtpt_ms > now_ms;
            let mut inv = snapshot_to_economy_inv(&val.snapshot);
            let res = {
                let mut ctx = NpcPlayerCtx {
                    account_id: val.snapshot.account_id.unwrap_or_default(), player_id,
                    location: val.snapshot.location, gender: val.snapshot.gender,
                    c_flag: val.c_flag.max(0).cast_unsigned(), activated: val.activated,
                    has_clan: val.snapshot.clan_id.0 >= 0, is_clan_leader, clan_days: 1,
                    has_pet: val.snapshot.pet.is_some(), wearing_full_destroyer,
                    is_use_gtpt, is_phu_ho_mabu: false, mabu_points: 0, vnd_balance: 0,
                    vip_purchases: val.snapshot.vip_state.5.max(0) as u8,
                    kol_stage: val.snapshot.kol_progress.kol_quest_stage.clamp(1, 7) as u8,
                    kol_vip_stage: val.snapshot.kol_progress.kol_vip_quest_stage.clamp(1, 7) as u8,
                    kol_progress, stats: &mut val.snapshot.stats, inventory: &mut inv,
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

        if !matches!(res, Ok(NpcEffect::ShowMenu(_) | NpcEffect::OpenHomePlant(_))) {
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
            Ok(NpcEffect::OpenMagicTree) => {
                if let Some(pkt) = self
                    .shared
                    .clients
                    .authenticated
                    .get(&sid)
                    .map(|e| super::router_world::build_magic_tree_menu_pkt(&e.snapshot))
                    .transpose()?
                {
                    session.send(pkt).await?;
                }
            }
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
            Ok(NpcEffect::UnlockedPowerLimit(false)) => {
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã mở giới hạn sức mạnh bản thân!")?).await?;
            }
            Ok(NpcEffect::PurchasedVip(tier)) => {
                if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                    entry.snapshot.vip_state.5 = entry.snapshot.vip_state.5.saturating_add(1).min(4);
                }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao(&format!("Đã mua gói VIP {tier} thành công!"))?).await?;
            }
            Ok(NpcEffect::ApplyGtpt) => {
                if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                    entry.snapshot.item_time.gtpt_ms = now_ms.saturating_add(1_800_000);
                }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã kích hoạt Giải trừ phép thuật (30 phút)!")?).await?;
            }
            Ok(NpcEffect::ApplyMabuBlessing) => {
                let updated_pt = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    entry.snapshot.stats.hp_max = entry.snapshot.stats.hp_max.saturating_add(1_000_000);
                    entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                    entry.snapshot.stats.mp_max = entry.snapshot.stats.mp_max.saturating_add(1_000_000);
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp_max;
                    entry.snapshot.stats.dame = entry.snapshot.stats.dame.saturating_add(10_000);
                    s2c_packets::me_load_point(&entry.snapshot.stats)
                });
                session.send(bag_pkt).await?;
                session.send(money_pkt).await?;
                session.send(updated_pt.unwrap_or(pt_pkt)).await?;
                session.send(s2c_packets::thong_bao("Đã nhận Phù hộ Kaiô (+1.000.000 HP/KI, +10.000 Sức đánh)!")?).await?;
            }
            Ok(NpcEffect::ExtendKilisBuff) => {
                if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                    entry.snapshot.item_time.kilis_ms = entry.snapshot.item_time.kilis_ms.max(now_ms).saturating_add(600_000);
                }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã gia hạn bùa Kilis thêm 10 phút!")?).await?;
            }
            Ok(NpcEffect::ApplyBlackBallBuff(hp_mult, dame_mult)) => {
                let updated_pt = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let hm = i64::from(hp_mult.max(1));
                    let dm = i64::from(dame_mult.max(1));
                    if hm > 1 {
                        entry.snapshot.stats.hp_max = entry.snapshot.stats.hp_max.saturating_mul(hm);
                        entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                    }
                    if dm > 1 {
                        entry.snapshot.stats.dame = entry.snapshot.stats.dame.saturating_mul(dm);
                    }
                    s2c_packets::me_load_point(&entry.snapshot.stats)
                });
                session.send(bag_pkt).await?;
                session.send(money_pkt).await?;
                session.send(updated_pt.unwrap_or(pt_pkt)).await?;
                session.send(s2c_packets::thong_bao(&format!("Đã kích hoạt Phù hộ Sao Đen (x{hp_mult} HP, x{dame_mult} Sức đánh)!"))?).await?;
            }
            Ok(NpcEffect::ClaimedKolStage(is_vip, stage)) => {
                if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                    let next_stage = i32::from(stage).saturating_add(1).min(8);
                    if is_vip {
                        entry.snapshot.kol_progress.kol_vip_quest_stage = next_stage;
                    } else {
                        entry.snapshot.kol_progress.kol_quest_stage = next_stage;
                    }
                }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao(&format!(
                    "Đã nhận thưởng nhiệm vụ KOL{} mốc {stage}!",
                    if is_vip { " VIP" } else { "" }
                ))?).await?;
            }
            Ok(NpcEffect::CancelledSkillStudy) => {
                let cancel_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if entry.snapshot.learn_skill.0 > 0 {
                        let refund = i64::from(entry.snapshot.learn_skill.2.max(0)) / 2;
                        entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(refund);
                        entry.snapshot.learn_skill = (-1, -1, 0);
                        Some((refund, s2c_packets::me_load_point(&entry.snapshot.stats)))
                    } else {
                        None
                    }
                });
                match cancel_res {
                    Some(Some((refund, new_pt_pkt))) => {
                        session.send(new_pt_pkt).await?;
                        session.send(s2c_packets::thong_bao(&format!("Đã hủy học kỹ năng và hoàn lại {refund} tiềm năng!"))?).await?;
                    }
                    _ => {
                        session.send(s2c_packets::thong_bao("Bạn không đang học kỹ năng nào!")?).await?;
                    }
                }
            }
            Ok(NpcEffect::OpenHomePlant(is_dua_hau)) => {
                self.handle_open_home_plant(session, sid, player_id, npc_id, is_dua_hau, now_ms).await?;
            }
            Ok(NpcEffect::LaunchActivity("PRESS_SUGARCANE")) => {
                self.handle_press_sugarcane(session, sid, select == Some(1), now_ms).await?;
            }
            Ok(NpcEffect::LaunchActivity(_)) => {
                session.send(s2c_packets::npc_open_say(npc_id, "Tính năng đang sẵn sàng phục vụ!", None)?).await?;
            }
            Err(err) => session.send(s2c_packets::npc_open_say(npc_id, &err.to_string(), None)?).await?,
        }
        Ok(())
    }

    async fn handle_open_home_plant(
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

    async fn handle_home_plant_confirm(
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

    async fn handle_press_sugarcane(&self, session: &SessionHandle, sid: u64, ten: bool, now_ms: i64) -> Result<(), ProtocolError> {
        let cups: u32 = if ten { 10 } else { 1 };
        let gold_cost: u64 = 5_000_000 * u64::from(cups);
        let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
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

    async fn execute_pending_combine(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: u8) -> Result<(), ProtocolError> {
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

    pub(super) async fn handle_intrinsic_open(&self, session: &SessionHandle, sid: u64, sub: i8) -> Result<(), ProtocolError> {
        if sub == 1 { return session.send(self.build_intrinsic_catalog_pkt()?).await; }
        let Some((pid, intr)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.intrinsic)) else { return Ok(()); };
        if let Some(rec) = self.shared.templates.intrinsics.iter().find(|r| r.id == i32::from(intr.intrinsic_id)) {
            session.send(s2c_packets::intrinsic_info(rec.icon as i16, &rec.name.replacen('#', &intr.param1.to_string(), 1))?).await?;
        } else {
            session.send(s2c_packets::intrinsic_info(0, "Chưa mở nội tại")?).await?;
        }
        let open_m = INTRINSIC_GOLD_MILLIONS[usize::from(intr.count_open.clamp(0, 7) as u8)];
        let gold_lbl = format!("Mở Nội Tại\n({open_m}Tr Vàng)");
        con_meo_state().insert(pid, const_npc::INTRINSIC);
        session.send(s2c_packets::npc_open_menu(
            i16::from(const_npc::CON_MEO), "Bạn muốn thực hiện thao tác Nội Tại nào?",
            &["Xem tất cả\nNội Tại", &gold_lbl, "Mở VIP\n(100 Ngọc)", "Đóng"], None,
        )?).await
    }

    async fn execute_dissolve_clan(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some((pid, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else {
            return Ok(());
        };
        if cid.0 < 0 {
            return session.send(s2c_packets::thong_bao("Bạn không ở trong bang hội nào!")?).await;
        }
        let Ok(clans) = self.shared.clans_repo.load_all().await else {
            return session.send(s2c_packets::thong_bao("Không thể giải tán bang hội lúc này!")?).await;
        };
        let is_leader = clans
            .iter()
            .find(|c| c.id == cid)
            .is_some_and(|c| c.members.iter().any(|m| m.id == pid.0 as i32 && m.role == 0));
        if !is_leader {
            return session.send(s2c_packets::thong_bao("Chỉ bang chủ mới có thể giải tán bang hội!")?).await;
        }
        if self.shared.clans_repo.delete_clan(cid).await.is_err() {
            return session.send(s2c_packets::thong_bao("Không thể giải tán bang hội lúc này!")?).await;
        }
        for mut entry in self.shared.clients.authenticated.iter_mut() {
            if entry.snapshot.clan_id == cid {
                entry.snapshot.clan_id = ClanId(-1);
            }
        }
        let mut w = PacketWriter::with_capacity(4);
        w.write_i32(-1);
        session.send(w.finish(cmd_message::CLAN_INFO)).await?;
        session.send(s2c_packets::thong_bao("Đã giải tán bang hội!")?).await
    }

    pub(super) async fn handle_con_meo_confirm(&self, session: &SessionHandle, sid: u64, select: u8) -> Result<(), ProtocolError> {
        let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
        let mode = con_meo_state().remove(&pid).map_or(const_npc::INTRINSIC, |(_, v)| v);
        if mode == const_npc::CONFIRM_LEAVE_CLAN {
            return if select == 0 { self.execute_leave_clan(session, sid).await } else { Ok(()) };
        }
        if mode == const_npc::CONFIRM_DISSOLUTION_CLAN {
            return if select == 0 { self.execute_dissolve_clan(session, sid).await } else { Ok(()) };
        }
        if !matches!(mode, const_npc::INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC_VIP) {
            return Ok(());
        }
        match select {
            0 => session.send(self.build_intrinsic_catalog_pkt()?).await?,
            1 | 2 => {
                let is_vip = select == 2;
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if entry.snapshot.stats.power < INTRINSIC_MIN_POWER {
                        return Err(format!("Cần ít nhất {INTRINSIC_MIN_POWER} sức mạnh để mở Nội Tại!"));
                    }
                    let g_i16 = i16::from(i8::from(entry.snapshot.gender));
                    let pool: Vec<_> = self.shared.templates.intrinsics.iter().filter(|r| r.gender == g_i16 || r.gender == 3).collect();
                    if pool.is_empty() { return Err("Chưa có dữ liệu Nội Tại!".to_owned()); }
                    if is_vip {
                        if entry.snapshot.wallet.gem < 100 { return Err("Cần 100 ngọc xanh để mở Nội Tại VIP!".to_owned()); }
                        entry.snapshot.wallet.gem -= 100;
                        entry.snapshot.intrinsic.count_open = 0;
                    } else {
                        let open_idx = usize::from(entry.snapshot.intrinsic.count_open.clamp(0, 7) as u8);
                        let gold_cost = INTRINSIC_GOLD_MILLIONS[open_idx] * 1_000_000;
                        let gold_cost_i64 = gold_cost as i64;
                        if entry.snapshot.wallet.gold < gold_cost_i64 {
                            return Err(format!("Cần {gold_cost} vàng để mở Nội Tại!"));
                        }
                        entry.snapshot.wallet.gold -= gold_cost_i64;
                        if entry.snapshot.intrinsic.count_open < 7 {
                            entry.snapshot.intrinsic.count_open += 1;
                        }
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
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
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

    pub(super) async fn handle_lucky_round(&self, session: &SessionHandle, sid: u64, ty: Option<i8>, opt_cnt: Option<i8>) -> Result<(), ProtocolError> {
        let is_vip = ty == Some(2);
        let Some(cnt) = opt_cnt else {
            let mut w = PacketWriter::with_capacity(24);
            w.write_i8(0);
            w.write_i8(7);
            for b in 419_i16..=425 { w.write_i16(b); }
            w.write_i8(if is_vip { 2 } else { 1 });
            w.write_i32(if is_vip { 250_000_000 } else { 4 });
            w.write_i16(if is_vip { -1 } else { 821 });
            return session.send(w.finish(cmd_message::LUCKY_ROUND)).await;
        };
        let mode = if is_vip { LuckyRoundMode::VipGold } else { LuckyRoundMode::StandardGem };
        let n = cnt.clamp(1, 7).cast_unsigned();
        let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            if inv.empty_bag_slots() < usize::from(n) { return Err("Hành trang đã đầy!".to_owned()); }
            let prev_len = inv.lucky_round_box.len();
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let spun = LuckyRoundService::spin(&mut inv, mode, n, &mut rng)
                .map_err(|err| format!("Vòng quay: {err}"))?;
            let mut icons = Vec::with_capacity(usize::from(spun));
            for item in inv.lucky_round_box.drain(prev_len..).flatten() {
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
