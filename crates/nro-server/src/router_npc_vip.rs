//! `NpcMenuRouter` context evaluation, VIP package fulfillment, and KOL quest claims.

use dashmap::mapref::entry::Entry;
use nro_consts::const_npc;
use nro_core::{AccountId, ItemInstance, PlayerId, TemplateId, ZoneId};
use nro_economy::{NpcEffect, NpcError, NpcKind, NpcMenuRouter, NpcPlayerCtx};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::{
    ServerSessionHandler, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::{
    current_kol_progress, daily_checkin_day, is_wearing_full_destroyer, mabu_points_map,
    phu_ho_mabu,
};

type NpcEvalOut = (
    PlayerId,
    AccountId,
    ZoneId,
    Result<NpcEffect, NpcError>,
    bool,
    [Packet; 4],
);

impl ServerSessionHandler {
    pub(super) fn evaluate_npc_router_step(
        &self,
        sid: u64,
        npc: NpcKind,
        cur_menu_idx: i32,
        select: Option<u8>,
        is_clan_leader: bool,
        now_ms: i64,
    ) -> (Option<NpcEvalOut>, bool, bool) {
        let (mut in_trade, mut already_checked_in) = (false, false);
        let eval = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                let val = entry.value_mut();
                let player_id = val.snapshot.id;
                if select.is_some() && super::super::router_shop::is_in_trade(player_id) {
                    in_trade = true;
                    return None;
                }
                let mut checkin_prev: Option<Option<i64>> = None;
                if npc == NpcKind::BoMong
                    && select == Some(3)
                    && cur_menu_idx == const_npc::BASE_MENU
                {
                    let today = now_ms / 86_400_000;
                    match daily_checkin_day().entry(player_id) {
                        Entry::Occupied(occ) if *occ.get() == today => {
                            already_checked_in = true;
                            return None;
                        }
                        Entry::Occupied(mut occ) => {
                            checkin_prev = Some(Some(*occ.get()));
                            occ.insert(today);
                        }
                        Entry::Vacant(vac) => {
                            vac.insert(today);
                            checkin_prev = Some(None);
                        }
                    }
                }
                let acc_id = val.snapshot.account_id.unwrap_or_default();
                let vnd_balance = if npc == NpcKind::ToriBot && acc_id.0 > 0 {
                    self.shared
                        .accounts_repo
                        .get_by_id(acc_id)
                        .ok()
                        .flatten()
                        .map_or(0, |a| u32::try_from(a.vnd.max(0)).unwrap_or(0))
                } else {
                    0
                };
                let mut inv = snapshot_to_economy_inv(&val.snapshot);
                let res = {
                    let mut ctx = NpcPlayerCtx {
                        account_id: acc_id,
                        player_id,
                        location: val.snapshot.location,
                        gender: val.snapshot.gender,
                        c_flag: val.c_flag.max(0).cast_unsigned(),
                        activated: val.activated,
                        has_clan: val.snapshot.clan_id.0 >= 0,
                        is_clan_leader,
                        clan_days: 1,
                        has_pet: val.snapshot.pet.is_some(),
                        wearing_full_destroyer: is_wearing_full_destroyer(&val.snapshot),
                        is_use_gtpt: val.snapshot.item_time.gtpt_ms > now_ms,
                        is_phu_ho_mabu: phu_ho_mabu().get(&player_id).is_some_and(|v| *v.value()),
                        mabu_points: u16::try_from(
                            mabu_points_map().get(&player_id).map_or(0, |v| *v.value()),
                        )
                        .unwrap_or(u16::MAX),
                        vnd_balance,
                        vip_purchases: val.snapshot.vip_state.5.max(0) as u8,
                        kol_stage: val.snapshot.kol_progress.kol_quest_stage.clamp(1, 7) as u8,
                        kol_vip_stage: val.snapshot.kol_progress.kol_vip_quest_stage.clamp(1, 7)
                            as u8,
                        kol_progress: current_kol_progress(
                            &val.snapshot.kol_progress,
                            if npc == NpcKind::QuyLaoKame
                                && cur_menu_idx == const_npc::BASE_MENU
                                && select == Some(3)
                            {
                                val.snapshot.kol_progress.kol_vip_quest_stage
                            } else {
                                val.snapshot.kol_progress.kol_quest_stage
                            },
                        ),
                        stats: &mut val.snapshot.stats,
                        inventory: &mut inv,
                    };
                    match select {
                        None => NpcMenuRouter::open_base_menu(npc, &ctx),
                        Some(sel) => NpcMenuRouter::confirm_menu(npc, cur_menu_idx, sel, &mut ctx),
                    }
                };
                if res.is_err()
                    && let Some(prev) = checkin_prev
                {
                    match prev {
                        Some(d) => {
                            daily_checkin_day().insert(player_id, d);
                        }
                        None => {
                            daily_checkin_day().remove(&player_id);
                        }
                    }
                }
                let bag_changed = res.is_ok()
                    && !matches!(res, Ok(NpcEffect::PurchasedVip(_)))
                    && inv.bag != val.snapshot.items_bag;
                if res.is_ok() && !matches!(res, Ok(NpcEffect::PurchasedVip(_))) {
                    sync_economy_inv_to_snapshot(&inv, &mut val.snapshot);
                }
                Some((
                    player_id,
                    acc_id,
                    val.snapshot.location.zone_id,
                    res,
                    bag_changed,
                    [
                        s2c_packets::inventory_box(&val.snapshot.items_box),
                        s2c_packets::inventory_bag(&val.snapshot.items_bag),
                        wallet_money_pkt(&val.snapshot.wallet),
                        s2c_packets::me_load_point(&val.snapshot.stats),
                    ],
                ))
            });
        (eval, in_trade, already_checked_in)
    }

    pub(in crate::router) async fn handle_purchased_vip(
        &self,
        session: &SessionHandle,
        sid: u64,
        acc_id: AccountId,
        npc_id: i16,
        tier: u8,
        now_ms: i64,
    ) -> Result<(), ProtocolError> {
        let (cost, qty, box_tid) = (
            i32::from(tier) * 50_000,
            [200u32, 500, 700, 1_000][usize::from(tier.saturating_sub(1).min(3))],
            TemplateId(if tier == 4 { 1772 } else { 1254 }),
        );
        let mut vip_in_trade = false;
        let has_space = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| {
                if super::super::router_shop::is_in_trade(e.snapshot.id) {
                    vip_in_trade = true;
                    return false;
                }
                let mut dry = snapshot_to_economy_inv(&e.snapshot);
                dry.add_to_bag(ItemInstance::new(TemplateId(457), qty), true)
                    .is_ok()
                    && dry.add_to_bag(ItemInstance::new(box_tid, 1), false).is_ok()
            });
        if vip_in_trade {
            return session
                .send(s2c_packets::thong_bao(
                    "Không thể thao tác khi đang giao dịch!",
                )?)
                .await;
        }
        if !has_space {
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Hành trang không đủ ô trống để nhận gói VIP!",
                    None,
                )?)
                .await;
        }
        if acc_id.0 <= 0
            || !self
                .shared
                .accounts_repo
                .sub_vnd(acc_id, cost)
                .await
                .unwrap_or(false)
        {
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Không đủ VND để mua gói VIP!",
                    None,
                )?)
                .await;
        }

        let out = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                inv.add_to_bag(ItemInstance::new(TemplateId(457), qty), true)
                    .ok()?;
                inv.add_to_bag(ItemInstance::new(box_tid, 1), false).ok()?;
                entry.snapshot.vip_state.5 = entry.snapshot.vip_state.5.saturating_add(1).min(4);
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                Some((
                    entry.snapshot.clone(),
                    [
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                        wallet_money_pkt(&entry.snapshot.wallet),
                        s2c_packets::me_load_point(&entry.snapshot.stats),
                    ],
                ))
            });
        if let Some((snap, pkts)) = out {
            if let Err(err) = self
                .shared
                .players_repo
                .save_player(&snap, None, self.shared.config.parity_mode, now_ms)
                .await
            {
                tracing::warn!(player_id = snap.id.0, error = %err, "failed persisting player after VIP purchase");
            }
            for p in pkts {
                session.send(p).await?;
            }
            session
                .send(s2c_packets::thong_bao(&format!(
                    "Đã mua gói VIP {tier} thành công!"
                ))?)
                .await?;
        } else {
            tracing::warn!(
                sid,
                account_id = acc_id.0,
                cost,
                tier,
                "VIP delivery failed after VND deduction; refunding VND"
            );
            if let Err(err) = self
                .shared
                .accounts_repo
                .add_vnd(acc_id, i64::from(cost))
                .await
            {
                tracing::warn!(sid, account_id = acc_id.0, cost, error = %err, "failed refunding VND after VIP delivery failure");
            }
            let _ = session.try_send(s2c_packets::npc_open_say(
                npc_id,
                "Hành trang không đủ ô trống để nhận gói VIP, đã hoàn lại VND!",
                None,
            )?);
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_kol_claim(
        &self,
        session: &SessionHandle,
        sid: u64,
        is_vip: bool,
        pkts: [Packet; 2],
    ) -> Result<(), ProtocolError> {
        let [bag_pkt, pt_pkt] = pkts;
        let claim_res = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return Err("Không thể thao tác khi đang giao dịch!");
                }
                let stage_ref = if is_vip {
                    &mut entry.snapshot.kol_progress.kol_vip_quest_stage
                } else {
                    &mut entry.snapshot.kol_progress.kol_quest_stage
                };
                if *stage_ref > 7 {
                    return Err("Bạn đã nhận hết thưởng nhiệm vụ KOL!");
                }
                let cur_stage = (*stage_ref).max(1);
                *stage_ref = (cur_stage + 1).min(8);
                if is_vip {
                    entry.snapshot.wallet.gem = entry.snapshot.wallet.gem.saturating_add(50);
                } else {
                    entry.snapshot.wallet.gold = entry
                        .snapshot
                        .wallet
                        .gold
                        .saturating_add(10_000_000)
                        .min(nro_db::player_repo::LIMIT_GOLD);
                }
                Ok((cur_stage, wallet_money_pkt(&entry.snapshot.wallet)))
            });
        match claim_res {
            Some(Ok((claimed, mp))) => {
                for p in [bag_pkt, mp, pt_pkt] {
                    session.send(p).await?;
                }
                let reward = if is_vip {
                    "+50 Ngọc"
                } else {
                    "+10.000.000 Vàng"
                };
                session
                    .send(s2c_packets::thong_bao(&format!(
                        "Đã nhận thưởng nhiệm vụ KOL{} mốc {claimed} ({reward})!",
                        if is_vip { " VIP" } else { "" }
                    ))?)
                    .await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
            None => {}
        }
        Ok(())
    }
}
