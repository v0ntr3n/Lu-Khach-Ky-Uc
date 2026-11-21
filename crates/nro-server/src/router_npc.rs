//! NPC menu dialogue (`32`, `33`) and main `NpcMenuRouter` dispatch.

use std::sync::OnceLock;

use dashmap::{DashMap, mapref::entry::Entry};
use nro_consts::const_npc;
use nro_core::{CoreStats, ItemInstance, MapId, PlayerId, TemplateId};
use nro_db::{json_columns::KolProgressData, player_repo::PlayerSnapshot};
use nro_economy::{NpcEffect, NpcKind, NpcMenuRouter, NpcPlayerCtx};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tracing::info;

use super::{
    ServerSessionHandler, npc_menu_index, pending_combine, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static DAILY_CHECKIN_DAY: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn daily_checkin_day() -> &'static DashMap<PlayerId, i64> { DAILY_CHECKIN_DAY.get_or_init(DashMap::new) }
static PHU_HO_MABU: OnceLock<DashMap<PlayerId, bool>> = OnceLock::new();
fn phu_ho_mabu() -> &'static DashMap<PlayerId, bool> { PHU_HO_MABU.get_or_init(DashMap::new) }
static BLACK_BALL_BUFF: OnceLock<DashMap<PlayerId, (u8, u8)>> = OnceLock::new();
fn black_ball_buff() -> &'static DashMap<PlayerId, (u8, u8)> { BLACK_BALL_BUFF.get_or_init(DashMap::new) }
static MABU_POINTS: OnceLock<DashMap<PlayerId, u32>> = OnceLock::new();
fn mabu_points_map() -> &'static DashMap<PlayerId, u32> { MABU_POINTS.get_or_init(DashMap::new) }

pub(super) fn reset_mabu_points(pid: PlayerId) { mabu_points_map().remove(&pid); }
pub(super) fn clear_npc_session_state(pid: PlayerId) { phu_ho_mabu().remove(&pid); black_ball_buff().remove(&pid); mabu_points_map().remove(&pid); }
pub(super) fn clear_npc_menu_state(pid: PlayerId) { npc_menu_index().remove(&pid); }
pub(super) fn clear_map_scoped_buffs(pid: PlayerId, from_map: MapId, to_map: MapId) -> bool {
    let mut cleared = false;
    if (127..=128).contains(&from_map.0) && !(127..=128).contains(&to_map.0) { cleared |= phu_ho_mabu().remove(&pid).is_some(); }
    if (85..=91).contains(&from_map.0) && !(85..=91).contains(&to_map.0) { cleared |= black_ball_buff().remove(&pid).is_some(); }
    cleared
}

pub(super) fn apply_npc_buffs_before_clamp(pid: PlayerId, stats: &mut CoreStats) {
    if phu_ho_mabu().get(&pid).is_some_and(|v| *v.value()) {
        stats.hp_max = stats.hp_max.saturating_add(1_000_000);
        stats.mp_max = stats.mp_max.saturating_add(1_000_000);
        stats.dame = stats.dame.saturating_add(10_000);
    }
    if let Some(bb) = black_ball_buff().get(&pid) {
        let (hm, dm) = (i64::from(bb.0.max(1)), i64::from(bb.1.max(1)));
        if hm > 1 { stats.hp_max = stats.hp_max.saturating_mul(hm); }
        if dm > 1 { stats.dame = stats.dame.saturating_mul(dm); }
    }
}

pub(super) fn is_wearing_full_destroyer(snap: &PlayerSnapshot) -> bool {
    snap.items_body.len() >= 5 && snap.items_body[..5].iter().all(|s| s.as_ref().is_some_and(|it| (650..=662).contains(&it.template_id.0)))
}

fn current_kol_progress(kol: &KolProgressData, stage: i32) -> u32 {
    let raw: i64 = match stage {
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
    pub(super) async fn handle_npc_menu(&self, session: &SessionHandle, sid: u64, npc_id: i16, select: Option<i8>) -> Result<(), ProtocolError> {
        let Some((pid, map_id, alive, clan_id, has_pet, stored_menu)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            let pid = e.snapshot.id;
            (pid, e.snapshot.location.map_id, e.snapshot.stats.hp > 0, e.snapshot.clan_id, e.snapshot.pet.is_some(), npc_menu_index().get(&pid).map(|v| *v.value()))
        }) else { return Ok(()); };
        let select = match select {
            Some(s) => { let Ok(u) = u8::try_from(s) else { clear_npc_menu_state(pid); super::con_meo_state().remove(&pid); return Ok(()); }; Some(u) }
            None => None,
        };
        if !alive { clear_npc_menu_state(pid); return session.send(s2c_packets::thong_bao("Không thể thao tác khi đã chết!")?).await; }
        if super::router_shop::is_in_trade(pid) { return session.send(s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!")?).await; }
        let is_virtual = npc_id == i16::from(const_npc::CON_MEO) || npc_id == i16::from(const_npc::RONG_THIENG)
            || npc_id == i16::from(const_npc::RONG_THIENG_NAMEC) || npc_id == i16::from(const_npc::QUA_TRUNG)
            || npc_id == i16::from(const_npc::DUA_HAU) || npc_id == i16::from(const_npc::LY_TIEU_NUONG);
        let npc_valid = npc_id >= 0 && (is_virtual || self.shared.templates.maps.iter().find(|m| m.id == map_id).map_or_else(
            || NpcKind::from_repr(npc_id).is_some(),
            |map| map.npcs.iter().any(|&(tid, _, _)| i16::from(tid) == npc_id || i16::from(tid.cast_unsigned()) == npc_id),
        ));
        if !npc_valid { clear_npc_menu_state(pid); return session.send(s2c_packets::thong_bao("NPC không tồn tại trong khu vực này!")?).await; }
        if npc_id == i16::from(const_npc::CON_MEO) {
            if let Some(sel) = select { return self.handle_con_meo_confirm(session, sid, sel).await; }
            return session.send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?).await;
        }
        if select.is_none() && let Some(task_say) = self.advance_task_on_npc(session, sid, npc_id).await? {
            return session.send(s2c_packets::npc_open_say(npc_id, &task_say, None)?).await;
        }
        let cur_menu_idx = if select.is_some() {
            let Some((stored_npc, idx)) = stored_menu else { return Ok(()); };
            if stored_npc != npc_id { return Ok(()); }
            idx
        } else { 0 };
        if let Some(sel) = select && cur_menu_idx == const_npc::MENU_START_COMBINE {
            return self.execute_pending_combine(session, sid, npc_id, sel).await;
        }
        if let Some(sel) = select && (npc_id == i16::from(const_npc::QUA_TRUNG) || npc_id == i16::from(const_npc::DUA_HAU)) {
            return self.handle_home_plant_confirm(session, sid, pid, npc_id, cur_menu_idx, sel).await;
        }
        if npc_id == i16::from(const_npc::RONG_THIENG) || npc_id == i16::from(const_npc::RONG_THIENG_NAMEC) {
            if let Some(sel) = select { return self.handle_dragon_wish_confirm(session, sid, pid, npc_id, cur_menu_idx, sel).await; }
            return session.send(s2c_packets::npc_open_say(npc_id, "Hãy nói ra điều ước của ngươi...", None)?).await;
        }
        if matches!(npc_id, 0..=2) && !has_pet {
            match select {
                None => {
                    npc_menu_index().insert(pid, (npc_id, const_npc::BASE_MENU));
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
        if npc_id == i16::from(const_npc::BO_MONG) && select == Some(1) && cur_menu_idx == const_npc::BASE_MENU { return self.handle_achievement(session, sid, None).await; }
        if cur_menu_idx == const_npc::OPEN_POWER_PET && select == Some(0) && self.shared.clients.authenticated.get(&sid).is_some_and(|e| e.snapshot.pet.as_ref().is_none_or(|p| p.stats.limit_power >= 9)) {
            npc_menu_index().remove(&pid); return session.send(s2c_packets::npc_open_say(npc_id, "Đệ tử đã đạt giới hạn sức mạnh tối đa!", None)?).await;
        }
        if matches!(npc_id, 18..=20) && select == Some(3) && cur_menu_idx == const_npc::BASE_MENU { return self.handle_lucky_round(session, sid, None, None).await; }
        let Some(npc) = NpcKind::from_repr(npc_id) else {
            info!(sid, npc_id, "unknown npc_id requested; sending fallback greeting");
            return session.send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?).await;
        };

        let is_clan_leader = clan_id.0 >= 0 && self.shared.clans_repo.get_by_id(clan_id).await.ok().flatten()
            .is_some_and(|c| c.members.iter().any(|m| m.id == pid.0 as i32 && m.role == 0));
        let (now_ms, mut in_trade, mut already_checked_in) = (unix_now_ms(), false, false);
        let eval = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            let val = entry.value_mut();
            let player_id = val.snapshot.id;
            if select.is_some() && super::router_shop::is_in_trade(player_id) { in_trade = true; return None; }
            let mut checkin_prev: Option<Option<i64>> = None;
            if npc == NpcKind::BoMong && select == Some(3) && cur_menu_idx == const_npc::BASE_MENU {
                let today = now_ms / 86_400_000;
                match daily_checkin_day().entry(player_id) {
                    Entry::Occupied(occ) if *occ.get() == today => { already_checked_in = true; return None; }
                    Entry::Occupied(mut occ) => { checkin_prev = Some(Some(*occ.get())); occ.insert(today); }
                    Entry::Vacant(vac) => { vac.insert(today); checkin_prev = Some(None); }
                }
            }
            let acc_id = val.snapshot.account_id.unwrap_or_default();
            let vnd_balance = if npc == NpcKind::ToriBot && acc_id.0 > 0 {
                self.shared.accounts_repo.get_by_id(acc_id).ok().flatten().map_or(0, |a| u32::try_from(a.vnd.max(0)).unwrap_or(0))
            } else { 0 };
            let mut inv = snapshot_to_economy_inv(&val.snapshot);
            let res = {
                let mut ctx = NpcPlayerCtx {
                    account_id: acc_id, player_id, location: val.snapshot.location, gender: val.snapshot.gender,
                    c_flag: val.c_flag.max(0).cast_unsigned(), activated: val.activated,
                    has_clan: val.snapshot.clan_id.0 >= 0, is_clan_leader, clan_days: 1,
                    has_pet: val.snapshot.pet.is_some(), wearing_full_destroyer: is_wearing_full_destroyer(&val.snapshot),
                    is_use_gtpt: val.snapshot.item_time.gtpt_ms > now_ms,
                    is_phu_ho_mabu: phu_ho_mabu().get(&player_id).is_some_and(|v| *v.value()),
                    mabu_points: u16::try_from(mabu_points_map().get(&player_id).map_or(0, |v| *v.value())).unwrap_or(u16::MAX),
                    vnd_balance, vip_purchases: val.snapshot.vip_state.5.max(0) as u8,
                    kol_stage: val.snapshot.kol_progress.kol_quest_stage.clamp(1, 7) as u8,
                    kol_vip_stage: val.snapshot.kol_progress.kol_vip_quest_stage.clamp(1, 7) as u8,
                    kol_progress: current_kol_progress(&val.snapshot.kol_progress, if npc == NpcKind::QuyLaoKame && cur_menu_idx == const_npc::BASE_MENU && select == Some(3) { val.snapshot.kol_progress.kol_vip_quest_stage } else { val.snapshot.kol_progress.kol_quest_stage }),
                    stats: &mut val.snapshot.stats, inventory: &mut inv,
                };
                match select {
                    None => NpcMenuRouter::open_base_menu(npc, &ctx),
                    Some(sel) => NpcMenuRouter::confirm_menu(npc, cur_menu_idx, sel, &mut ctx),
                }
            };
            if res.is_err() && let Some(prev) = checkin_prev {
                match prev { Some(d) => { daily_checkin_day().insert(player_id, d); } None => { daily_checkin_day().remove(&player_id); } }
            }
            let bag_changed = res.is_ok() && !matches!(res, Ok(NpcEffect::PurchasedVip(_))) && inv.bag != val.snapshot.items_bag;
            if res.is_ok() && !matches!(res, Ok(NpcEffect::PurchasedVip(_))) { sync_economy_inv_to_snapshot(&inv, &mut val.snapshot); }
            Some((
                player_id, acc_id, val.snapshot.location.zone_id, res, bag_changed,
                s2c_packets::inventory_box(&val.snapshot.items_box),
                s2c_packets::inventory_bag(&val.snapshot.items_bag),
                wallet_money_pkt(&val.snapshot.wallet),
                s2c_packets::me_load_point(&val.snapshot.stats),
            ))
        });
        if in_trade { return session.send(s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!")?).await; }
        if already_checked_in { npc_menu_index().remove(&pid); return session.send(s2c_packets::npc_open_say(npc_id, "Hôm nay bạn đã điểm danh rồi!", None)?).await; }
        let Some((player_id, acc_id, cur_zone, res, bag_changed, box_pkt, bag_pkt, money_pkt, pt_pkt)) = eval else { return Ok(()); };

        if !matches!(res, Ok(NpcEffect::ShowMenu(_) | NpcEffect::OpenHomePlant(_))) {
            npc_menu_index().remove(&player_id);
        }
        match res {
            Ok(NpcEffect::ShowMenu(prompt)) => {
                npc_menu_index().insert(player_id, (npc_id, prompt.menu_index));
                session.send(s2c_packets::npc_open_menu(npc_id, prompt.say, &prompt.options, None)?).await?;
            }
            Ok(NpcEffect::ChangeMap(m, tx, ty)) => { if self.switch_player_zone_with_ship(session, sid, Some(m), cur_zone, Some((tx, ty)), 0).await? && npc == NpcKind::Osin && (114..=120).contains(&map_id.0) { reset_mabu_points(player_id); } }
            Ok(NpcEffect::Tutorial(text)) => { if bag_changed { session.send(bag_pkt).await?; } session.send(s2c_packets::npc_open_say(npc_id, text, None)?).await?; }
            Ok(NpcEffect::OpenItemBox) => { session.send(box_pkt).await?; session.send(s2c_packets::inventory_box_open(0)).await?; }
            Ok(NpcEffect::OpenCombine(op)) => {
                pending_combine().insert(player_id, (npc_id, op, Vec::new()));
                session.send(s2c_packets::combine_open(&format!("Chọn trang bị để thực hiện: {op}"), "Bà Hạt Mít")).await?;
            }
            Ok(NpcEffect::OpenMagicTree) => {
                let pkt = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    super::router_world::tick_magic_tree(&mut e.snapshot, now_ms);
                    super::router_world::build_magic_tree_menu_pkt(&e.snapshot)
                }).transpose()?;
                if let Some(pkt) = pkt { session.send(pkt).await?; }
            }
            Ok(NpcEffect::OpenShop(tag)) => self.handle_open_shop(session, sid, npc_id, tag).await?,
            Ok(NpcEffect::OpenConsignShop) => self.handle_open_consign(session, sid).await?,
            Ok(NpcEffect::OpenInput(kind)) => self.handle_open_input(session, sid, kind).await?,
            Ok(NpcEffect::NoOp) => session.send(money_pkt).await?,
            Ok(NpcEffect::ClaimedDailyCheckIn) => {
                daily_checkin_day().retain(|_, day| *day >= now_ms / 86_400_000);
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
            }
            Ok(NpcEffect::UnlockedPowerLimit(true)) => {
                let pet_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if let Some(pet) = entry.snapshot.pet.as_mut() && pet.stats.limit_power < 9 {
                        pet.stats.limit_power += 1;
                        Ok((self.pkt_pet_detail(&entry.snapshot), wallet_money_pkt(&entry.snapshot.wallet)))
                    } else {
                        entry.snapshot.wallet.gold = entry.snapshot.wallet.gold.saturating_add(50_000_000).min(nro_db::player_repo::LIMIT_GOLD);
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
                let (cost, qty, box_tid) = (i32::from(tier) * 50_000, [200u32, 500, 700, 1_000][usize::from(tier.saturating_sub(1).min(3))], TemplateId(if tier == 4 { 1772 } else { 1254 }));
                let mut vip_in_trade = false;
                let has_space = self.shared.clients.authenticated.get(&sid).is_some_and(|e| {
                    if super::router_shop::is_in_trade(e.snapshot.id) { vip_in_trade = true; return false; }
                    let mut dry = snapshot_to_economy_inv(&e.snapshot);
                    dry.add_to_bag(ItemInstance::new(TemplateId(457), qty), true).is_ok() && dry.add_to_bag(ItemInstance::new(box_tid, 1), false).is_ok()
                });
                if vip_in_trade { session.send(s2c_packets::thong_bao("Không thể thao tác khi đang giao dịch!")?).await?; }
                else if !has_space { session.send(s2c_packets::npc_open_say(npc_id, "Hành trang không đủ ô trống để nhận gói VIP!", None)?).await?; }
                else if acc_id.0 <= 0 || !self.shared.accounts_repo.sub_vnd(acc_id, cost).await.unwrap_or(false) { session.send(s2c_packets::npc_open_say(npc_id, "Không đủ VND để mua gói VIP!", None)?).await?; }
                else {
                    let out = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                        if super::router_shop::is_in_trade(entry.snapshot.id) { return None; }
                        let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                        inv.add_to_bag(ItemInstance::new(TemplateId(457), qty), true).ok()?;
                        inv.add_to_bag(ItemInstance::new(box_tid, 1), false).ok()?;
                        entry.snapshot.vip_state.5 = entry.snapshot.vip_state.5.saturating_add(1).min(4);
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        Some((entry.snapshot.clone(), [s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet), s2c_packets::me_load_point(&entry.snapshot.stats)]))
                    });
                    if let Some((snap, pkts)) = out {
                        if let Err(err) = self.shared.players_repo.save_player(&snap, None, self.shared.config.parity_mode, now_ms).await { tracing::warn!(player_id = snap.id.0, error = %err, "failed persisting player after VIP purchase"); }
                        for p in pkts { session.send(p).await?; }
                        session.send(s2c_packets::thong_bao(&format!("Đã mua gói VIP {tier} thành công!"))?).await?;
                    } else {
                        tracing::warn!(sid, account_id = acc_id.0, cost, tier, "VIP delivery failed after VND deduction; refunding VND");
                        if let Err(err) = self.shared.accounts_repo.add_vnd(acc_id, i64::from(cost)).await { tracing::warn!(sid, account_id = acc_id.0, cost, error = %err, "failed refunding VND after VIP delivery failure"); }
                        let _ = session.try_send(s2c_packets::npc_open_say(npc_id, "Hành trang không đủ ô trống để nhận gói VIP, đã hoàn lại VND!", None)?);
                    }
                }
            }
            Ok(NpcEffect::ApplyGtpt) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.item_time.gtpt_ms = now_ms.saturating_add(1_800_000); }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã kích hoạt Giải trừ phép thuật (30 phút)!")?).await?;
            }
            Ok(NpcEffect::ApplyMabuBlessing) => {
                phu_ho_mabu().insert(player_id, true);
                let updated_pt = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    entry.snapshot.stats.hp = entry.snapshot.stats.hp_max;
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp_max;
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    s2c_packets::me_load_point(&entry.snapshot.stats)
                });
                for p in [bag_pkt, money_pkt, updated_pt.unwrap_or(pt_pkt)] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã nhận Phù hộ Kaiô (+1.000.000 HP/KI, +10.000 Sức đánh)!")?).await?;
            }
            Ok(NpcEffect::ExtendKilisBuff) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.item_time.kilis_ms = e.snapshot.item_time.kilis_ms.max(now_ms).saturating_add(600_000); }
                for p in [bag_pkt, money_pkt, pt_pkt] { session.send(p).await?; }
                session.send(s2c_packets::thong_bao("Đã gia hạn bùa Kilis thêm 10 phút!")?).await?;
            }
            Ok(NpcEffect::ApplyBlackBallBuff(hp_mult, dame_mult)) => {
                let (hm_u8, dm_u8) = (hp_mult.max(1), dame_mult.max(1));
                let updated_pt = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let already_applied = black_ball_buff()
                        .get(&player_id)
                        .is_some_and(|prev| prev.0 >= hm_u8 && prev.1 >= dm_u8);
                    if !already_applied {
                        black_ball_buff().insert(player_id, (hm_u8, dm_u8));
                        super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                        if hm_u8 > 1 { entry.snapshot.stats.hp = entry.snapshot.stats.hp_max; }
                    }
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    s2c_packets::me_load_point(&entry.snapshot.stats)
                });
                session.send(bag_pkt).await?;
                session.send(money_pkt).await?;
                session.send(updated_pt.unwrap_or(pt_pkt)).await?;
                session.send(s2c_packets::thong_bao(&format!("Đã kích hoạt Phù hộ Sao Đen (x{hp_mult} HP, x{dame_mult} Sức đánh)!"))?).await?;
            }
            Ok(NpcEffect::ClaimedKolStage(is_vip, _)) => {
                let claim_res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if super::router_shop::is_in_trade(entry.snapshot.id) { return Err("Không thể thao tác khi đang giao dịch!"); }
                    let stage_ref = if is_vip { &mut entry.snapshot.kol_progress.kol_vip_quest_stage } else { &mut entry.snapshot.kol_progress.kol_quest_stage };
                    if *stage_ref > 7 { return Err("Bạn đã nhận hết thưởng nhiệm vụ KOL!"); }
                    let cur_stage = (*stage_ref).max(1);
                    *stage_ref = (cur_stage + 1).min(8);
                    if is_vip { entry.snapshot.wallet.gem = entry.snapshot.wallet.gem.saturating_add(50); }
                    else { entry.snapshot.wallet.gold = entry.snapshot.wallet.gold.saturating_add(10_000_000).min(nro_db::player_repo::LIMIT_GOLD); }
                    Ok((cur_stage, wallet_money_pkt(&entry.snapshot.wallet)))
                });
                match claim_res {
                    Some(Ok((claimed, mp))) => {
                        for p in [bag_pkt, mp, pt_pkt] { session.send(p).await?; }
                        let reward = if is_vip { "+50 Ngọc" } else { "+10.000.000 Vàng" };
                        session.send(s2c_packets::thong_bao(&format!("Đã nhận thưởng nhiệm vụ KOL{} mốc {claimed} ({reward})!", if is_vip { " VIP" } else { "" }))?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
                    None => {}
                }
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
            Ok(NpcEffect::OpenHomePlant(d)) => self.handle_open_home_plant(session, sid, player_id, npc_id, d, now_ms).await?,
            Ok(NpcEffect::LaunchActivity("PRESS_SUGARCANE")) => self.handle_press_sugarcane(session, sid, select == Some(1), now_ms).await?,
            Ok(NpcEffect::LaunchActivity(_)) => session.send(s2c_packets::npc_open_say(npc_id, "Tính năng đang sẵn sàng phục vụ!", None)?).await?,
            Err(err) => session.send(s2c_packets::npc_open_say(npc_id, &err.to_string(), None)?).await?,
        }
        Ok(())
    }

    pub(super) fn money_pkt_for(&self, sid: u64) -> Option<Packet> {
        self.shared.clients.authenticated.get(&sid).map(|e| wallet_money_pkt(&e.snapshot.wallet))
    }
}
