//! NPC menu dialogue (`32`, `33`) and main `NpcMenuRouter` dispatch.

#[path = "router_npc_effects.rs"]
mod router_npc_effects;
#[path = "router_npc_vip.rs"]
mod router_npc_vip;

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::const_npc;
use nro_core::{CoreStats, MapId, PlayerId};
use nro_db::{json_columns::KolProgressData, player_repo::PlayerSnapshot};
use nro_economy::NpcKind;
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use tracing::info;

use super::{ServerSessionHandler, npc_menu_index, wallet_money_pkt};
use crate::runtime::unix_now_ms;

static DAILY_CHECKIN_DAY: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
pub(self) fn daily_checkin_day() -> &'static DashMap<PlayerId, i64> {
    DAILY_CHECKIN_DAY.get_or_init(DashMap::new)
}
static PHU_HO_MABU: OnceLock<DashMap<PlayerId, bool>> = OnceLock::new();
pub(self) fn phu_ho_mabu() -> &'static DashMap<PlayerId, bool> {
    PHU_HO_MABU.get_or_init(DashMap::new)
}
static BLACK_BALL_BUFF: OnceLock<DashMap<PlayerId, (u8, u8)>> = OnceLock::new();
pub(self) fn black_ball_buff() -> &'static DashMap<PlayerId, (u8, u8)> {
    BLACK_BALL_BUFF.get_or_init(DashMap::new)
}
static MABU_POINTS: OnceLock<DashMap<PlayerId, u32>> = OnceLock::new();
pub(self) fn mabu_points_map() -> &'static DashMap<PlayerId, u32> {
    MABU_POINTS.get_or_init(DashMap::new)
}

pub(super) fn reset_mabu_points(pid: PlayerId) {
    mabu_points_map().remove(&pid);
}
pub(super) fn clear_npc_session_state(pid: PlayerId) {
    phu_ho_mabu().remove(&pid);
    black_ball_buff().remove(&pid);
    mabu_points_map().remove(&pid);
}
pub(super) fn clear_npc_menu_state(pid: PlayerId) {
    npc_menu_index().remove(&pid);
}
pub(super) fn clear_map_scoped_buffs(pid: PlayerId, from_map: MapId, to_map: MapId) -> bool {
    let mut cleared = false;
    if (127..=128).contains(&from_map.0) && !(127..=128).contains(&to_map.0) {
        cleared |= phu_ho_mabu().remove(&pid).is_some();
    }
    if (85..=91).contains(&from_map.0) && !(85..=91).contains(&to_map.0) {
        cleared |= black_ball_buff().remove(&pid).is_some();
    }
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
        if hm > 1 {
            stats.hp_max = stats.hp_max.saturating_mul(hm);
        }
        if dm > 1 {
            stats.dame = stats.dame.saturating_mul(dm);
        }
    }
}

pub(super) fn is_wearing_full_destroyer(snap: &PlayerSnapshot) -> bool {
    snap.items_body.len() >= 5
        && snap.items_body[..5].iter().all(|s| {
            s.as_ref()
                .is_some_and(|it| (650..=662).contains(&it.template_id.0))
        })
}

pub(self) fn current_kol_progress(kol: &KolProgressData, stage: i32) -> u32 {
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
    pub(super) async fn handle_npc_menu(
        &self,
        session: &SessionHandle,
        sid: u64,
        npc_id: i16,
        select: Option<i8>,
    ) -> Result<(), ProtocolError> {
        let Some((pid, map_id, alive, clan_id, can_claim_starter_pet, stored_menu)) =
            self.shared.clients.authenticated.get(&sid).map(|e| {
                let pid = e.snapshot.id;
                let can_claim = e.snapshot.pet.is_none()
                    && (e.snapshot.data_event.0[5]
                        & super::router_secondary::STARTER_PET_CLAIMED_FLAG)
                        == 0;
                (
                    pid,
                    e.snapshot.location.map_id,
                    e.snapshot.stats.hp > 0,
                    e.snapshot.clan_id,
                    can_claim,
                    npc_menu_index().get(&pid).map(|v| *v.value()),
                )
            })
        else {
            return Ok(());
        };
        let select = match select {
            Some(s) => {
                let Ok(u) = u8::try_from(s) else {
                    clear_npc_menu_state(pid);
                    super::con_meo_state().remove(&pid);
                    return Ok(());
                };
                Some(u)
            }
            None => None,
        };
        if !alive {
            clear_npc_menu_state(pid);
            return session
                .send(s2c_packets::thong_bao("Không thể thao tác khi đã chết!")?)
                .await;
        }
        if super::router_shop::is_in_trade(pid) {
            return session
                .send(s2c_packets::thong_bao(
                    "Không thể thao tác khi đang giao dịch!",
                )?)
                .await;
        }
        let is_virtual = matches!(
            npc_id,
            x if x == i16::from(const_npc::CON_MEO)
                || x == i16::from(const_npc::RONG_THIENG)
                || x == i16::from(const_npc::RONG_THIENG_NAMEC)
                || x == i16::from(const_npc::QUA_TRUNG)
                || x == i16::from(const_npc::DUA_HAU)
                || x == i16::from(const_npc::LY_TIEU_NUONG)
        );
        let npc_valid = npc_id >= 0
            && (is_virtual
                || self
                    .shared
                    .templates
                    .maps
                    .iter()
                    .find(|m| m.id == map_id)
                    .map_or_else(
                        || NpcKind::from_repr(npc_id).is_some(),
                        |map| {
                            map.npcs.iter().any(|&(tid, _, _)| {
                                i16::from(tid) == npc_id || i16::from(tid.cast_unsigned()) == npc_id
                            })
                        },
                    ));
        if !npc_valid {
            clear_npc_menu_state(pid);
            return session
                .send(s2c_packets::thong_bao(
                    "NPC không tồn tại trong khu vực này!",
                )?)
                .await;
        }
        if npc_id == i16::from(const_npc::CON_MEO) {
            if let Some(sel) = select {
                return self.handle_con_meo_confirm(session, sid, sel).await;
            }
            return session
                .send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?)
                .await;
        }
        if select.is_none()
            && let Some(task_say) = self.advance_task_on_npc(session, sid, npc_id).await?
        {
            return session
                .send(s2c_packets::npc_open_say(npc_id, &task_say, None)?)
                .await;
        }
        let cur_menu_idx = if select.is_some() {
            let Some((stored_npc, idx)) = stored_menu else {
                return Ok(());
            };
            if stored_npc != npc_id {
                return Ok(());
            }
            idx
        } else {
            0
        };
        if let Some(sel) = select
            && cur_menu_idx == const_npc::MENU_START_COMBINE
        {
            return self
                .execute_pending_combine(session, sid, npc_id, sel)
                .await;
        }
        if let Some(sel) = select
            && (npc_id == i16::from(const_npc::QUA_TRUNG)
                || npc_id == i16::from(const_npc::DUA_HAU))
        {
            return self
                .handle_home_plant_confirm(session, sid, pid, npc_id, cur_menu_idx, sel)
                .await;
        }
        if npc_id == i16::from(const_npc::RONG_THIENG)
            || npc_id == i16::from(const_npc::RONG_THIENG_NAMEC)
        {
            if let Some(sel) = select {
                return self
                    .handle_dragon_wish_confirm(session, sid, pid, npc_id, cur_menu_idx, sel)
                    .await;
            }
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Hãy nói ra điều ước của ngươi...",
                    None,
                )?)
                .await;
        }
        if matches!(npc_id, 0..=2) && can_claim_starter_pet {
            return self
                .handle_home_elder_starter_pet(session, sid, pid, npc_id, select)
                .await;
        }
        if npc_id == i16::from(const_npc::BO_MONG)
            && select == Some(1)
            && cur_menu_idx == const_npc::BASE_MENU
        {
            return self.handle_achievement(session, sid, None).await;
        }
        if cur_menu_idx == const_npc::OPEN_POWER_PET
            && select == Some(0)
            && self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .is_some_and(|e| {
                    e.snapshot
                        .pet
                        .as_ref()
                        .is_none_or(|p| p.stats.limit_power >= 9)
                })
        {
            npc_menu_index().remove(&pid);
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Đệ tử đã đạt giới hạn sức mạnh tối đa!",
                    None,
                )?)
                .await;
        }
        if matches!(npc_id, 18..=20) && select == Some(3) && cur_menu_idx == const_npc::BASE_MENU {
            return self.handle_lucky_round(session, sid, None, None).await;
        }
        let Some(npc) = NpcKind::from_repr(npc_id) else {
            info!(
                sid,
                npc_id, "unknown npc_id requested; sending fallback greeting"
            );
            return session
                .send(s2c_packets::npc_open_say(npc_id, "Xin chào!", None)?)
                .await;
        };

        let is_clan_leader = clan_id.0 >= 0
            && self
                .shared
                .clans_repo
                .get_by_id(clan_id)
                .await
                .ok()
                .flatten()
                .is_some_and(|c| {
                    c.members
                        .iter()
                        .any(|m| m.id == pid.0 as i32 && m.role == 0)
                });
        let now_ms = unix_now_ms();
        let (eval, in_trade, already_checked_in) =
            self.evaluate_npc_router_step(sid, npc, cur_menu_idx, select, is_clan_leader, now_ms);
        if in_trade {
            return session
                .send(s2c_packets::thong_bao(
                    "Không thể thao tác khi đang giao dịch!",
                )?)
                .await;
        }
        if already_checked_in {
            npc_menu_index().remove(&pid);
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Hôm nay bạn đã điểm danh rồi!",
                    None,
                )?)
                .await;
        }
        let Some((player_id, acc_id, cur_zone, res, bag_changed, pkts)) = eval else {
            return Ok(());
        };
        self.dispatch_npc_effect(
            session,
            sid,
            (player_id, acc_id),
            (npc_id, npc, map_id, cur_zone),
            select,
            now_ms,
            res,
            bag_changed,
            pkts,
        )
        .await
    }

    pub(super) fn money_pkt_for(&self, sid: u64) -> Option<Packet> {
        self.shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| wallet_money_pkt(&e.snapshot.wallet))
    }
}
