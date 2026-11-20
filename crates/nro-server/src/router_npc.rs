//! NPC menu dialogue (`32`, `33`) and main `NpcMenuRouter` dispatch.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::const_npc;
use nro_core::PlayerId;
use nro_db::{json_columns::KolProgressData, player_repo::PlayerSnapshot};
use nro_economy::{NpcEffect, NpcKind, NpcMenuRouter, NpcPlayerCtx};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use tracing::info;

use super::{
    ServerSessionHandler, npc_menu_index, pending_combine, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static DAILY_CHECKIN_DAY: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn daily_checkin_day() -> &'static DashMap<PlayerId, i64> { DAILY_CHECKIN_DAY.get_or_init(DashMap::new) }

pub(super) fn is_wearing_full_destroyer(snap: &PlayerSnapshot) -> bool {
    snap.items_body.len() >= 5
        && snap.items_body[..5]
            .iter()
            .all(|slot| slot.as_ref().is_some_and(|it| (650..=662).contains(&it.template_id.0)))
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

    pub(super) fn money_pkt_for(&self, sid: u64) -> Option<Packet> {
        self.shared.clients.authenticated.get(&sid).map(|e| wallet_money_pkt(&e.snapshot.wallet))
    }
}
