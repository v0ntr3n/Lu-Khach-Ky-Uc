//! PvP challenges (`-59`) and PK flags (`-103`).

#[path = "router_combat_pvp_social.rs"]
mod router_combat_pvp_social;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{Location, PlayerId};
use nro_protocol::{FlagAction, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::router_combat::is_offline_map;
use super::{ServerSessionHandler, send_thong_bao, wallet_money_pkt};
use crate::runtime::unix_now_ms;

const DEFAULT_FLAGS: &[(i16, i16)] = &[
    (519, 2325),
    (520, 2326),
    (521, 2327),
    (522, 2328),
    (523, 2329),
    (524, 2330),
    (525, 2331),
    (526, 2332),
    (527, 2333),
];
const PVP_CHALLENGE_STAKE: i64 = 1_000;
const FLAG_CHANGE_COOLDOWN_MS: i64 = 60_000;

static ACTIVE_PVP: OnceLock<Mutex<HashMap<PlayerId, (PlayerId, i64)>>> = OnceLock::new();
pub(super) fn lock_pvp() -> MutexGuard<'static, HashMap<PlayerId, (PlayerId, i64)>> {
    ACTIVE_PVP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

static PENDING_PVP: OnceLock<DashMap<(PlayerId, PlayerId), (i64, i64)>> = OnceLock::new();
pub(super) fn pending_pvp() -> &'static DashMap<(PlayerId, PlayerId), (i64, i64)> {
    PENDING_PVP.get_or_init(DashMap::new)
}

static FLAG_COOLDOWN: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn flag_cooldown() -> &'static DashMap<PlayerId, i64> {
    FLAG_COOLDOWN.get_or_init(DashMap::new)
}
pub(super) fn clear_flag_cooldown(pid: PlayerId) {
    flag_cooldown().remove(&pid);
}

impl ServerSessionHandler {
    pub(super) fn broadcast_pk_type(
        &self,
        loc: Location,
        actor_sid: u64,
        pid: PlayerId,
        pk_type: i8,
    ) {
        let mut pk = s2c_packets::sub_command_writer(cmd_message::UPDATE_TYPE_PK, 5);
        pk.write_i32(pid.0 as i32);
        pk.write_i8(pk_type);
        self.broadcast_zone_or_self(loc, actor_sid, &pk.finish(cmd_message::SUB_COMMAND));
    }

    pub(super) fn forfeit_active_pvp(&self, pid: PlayerId, loc: Location, sid: u64) {
        let opp_opt = {
            let mut pvp = lock_pvp();
            let opp = pvp.remove(&pid);
            if let Some((opp_id, _)) = opp
                && pvp.get(&opp_id).is_some_and(|&(other, _)| other == pid)
            {
                pvp.remove(&opp_id);
            }
            opp
        };
        let Some((opp, stake)) = opp_opt else {
            return;
        };
        self.cleanup_trade_on_disconnect(opp);
        let opp_sid_opt = self.shared.clients.players.get(&opp).map(|s| *s.value());
        let opp_loc = opp_sid_opt
            .and_then(|os| {
                self.shared
                    .clients
                    .authenticated
                    .get(&os)
                    .map(|e| e.snapshot.location)
            })
            .unwrap_or(loc);
        self.broadcast_pk_type(loc, sid, pid, 0);
        self.broadcast_pk_type(opp_loc, opp_sid_opt.unwrap_or(sid), opp, 0);
        if stake > 0
            && let Some(opp_sid) = opp_sid_opt
            && let Some((transferred, loser_handle, loser_money_pkt)) = self
                .shared
                .clients
                .authenticated
                .get_mut(&sid)
                .map(|mut loser| {
                    let transferred = stake.min(loser.snapshot.wallet.gold.max(0));
                    loser.snapshot.wallet.gold = loser
                        .snapshot
                        .wallet
                        .gold
                        .saturating_sub(transferred)
                        .max(0);
                    (
                        transferred,
                        loser.handle.clone(),
                        wallet_money_pkt(&loser.snapshot.wallet),
                    )
                })
        {
            let winner_res =
                self.shared
                    .clients
                    .authenticated
                    .get_mut(&opp_sid)
                    .map(|mut winner| {
                        winner.snapshot.wallet.gold = winner
                            .snapshot
                            .wallet
                            .gold
                            .saturating_add(transferred)
                            .min(nro_db::player_repo::LIMIT_GOLD);
                        (
                            winner.handle.clone(),
                            wallet_money_pkt(&winner.snapshot.wallet),
                        )
                    });
            let _ = loser_handle.try_send(loser_money_pkt);
            if let Ok(tb) =
                s2c_packets::thong_bao(&format!("Bạn đã thua thách đấu và mất {transferred} vàng!"))
            {
                let _ = loser_handle.try_send(tb);
            }
            if let Some((winner_handle, winner_money_pkt)) = winner_res {
                let _ = winner_handle.try_send(winner_money_pkt);
                if let Ok(tb) = s2c_packets::thong_bao(&format!(
                    "Bạn đã thắng thách đấu và nhận được {transferred} vàng!"
                )) {
                    let _ = winner_handle.try_send(tb);
                }
            }
        }
    }

    pub(super) async fn handle_pvp_challenge(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: i8,
        pvp_type: i8,
        target_pid: PlayerId,
    ) -> Result<(), ProtocolError> {
        let Some((my_pid, my_name, loc, my_hp, my_gold)) =
            self.shared.clients.authenticated.get(&sid).map(|e| {
                (
                    e.snapshot.id,
                    e.snapshot.name.clone(),
                    e.snapshot.location,
                    e.snapshot.stats.hp,
                    e.snapshot.wallet.gold,
                )
            })
        else {
            return Ok(());
        };
        if is_offline_map(loc.map_id) || my_hp <= 0 || my_pid == target_pid {
            return Ok(());
        }
        let Some(target_sid) = self
            .shared
            .clients
            .players
            .get(&target_pid)
            .map(|s| *s.value())
        else {
            return session
                .send(s2c_packets::thong_bao("Người chơi không còn trực tuyến!")?)
                .await;
        };
        if matches!(action, 0 | 1)
            && (super::router_shop::is_in_trade(my_pid)
                || super::router_shop::is_in_trade(target_pid))
        {
            return send_thong_bao(session, "Không thể thách đấu khi đang giao dịch!");
        }
        if action == 0
            && let Some((tloc, t_hp, t_gold, t_handle)) =
                self.shared.clients.authenticated.get(&target_sid).map(|t| {
                    (
                        t.snapshot.location,
                        t.snapshot.stats.hp,
                        t.snapshot.wallet.gold,
                        t.handle.clone(),
                    )
                })
        {
            let in_pvp = {
                let pvp = lock_pvp();
                pvp.contains_key(&my_pid) || pvp.contains_key(&target_pid)
            };
            if t_hp <= 0 || tloc.map_id != loc.map_id || tloc.zone_id != loc.zone_id || in_pvp {
                return Ok(());
            }
            if my_gold < PVP_CHALLENGE_STAKE || t_gold < PVP_CHALLENGE_STAKE {
                return send_thong_bao(session, "Cả hai người chơi cần có đủ vàng để thách đấu!");
            }
            let now_ms = unix_now_ms();
            let pending = pending_pvp();
            if pending.len() >= 1_024 {
                pending.retain(|_, (exp, _)| *exp > now_ms);
                if pending.len() >= 2_048 {
                    return send_thong_bao(session, "Hệ thống đang bận, vui lòng thử lại sau!");
                }
            }
            pending.insert(
                (my_pid, target_pid),
                (now_ms.saturating_add(30_000), PVP_CHALLENGE_STAKE),
            );
            let mut pw = PacketWriter::with_capacity(24 + my_name.len());
            pw.write_i8(pvp_type);
            pw.write_i32(my_pid.0 as i32);
            pw.write_i32(PVP_CHALLENGE_STAKE as i32);
            pw.write_utf(&format!("{my_name} mời bạn thách đấu!"))?;
            let _ = t_handle.try_send(pw.finish(cmd_message::PLAYER_VS_PLAYER));
        } else if action == 1 {
            let now_ms = unix_now_ms();
            let target_info = self.shared.clients.authenticated.get(&target_sid).map(|t| {
                (
                    t.snapshot.stats.hp > 0
                        && t.snapshot.location.map_id == loc.map_id
                        && t.snapshot.location.zone_id == loc.zone_id,
                    t.snapshot.wallet.gold,
                )
            });
            let Some((_, (exp, stake))) = pending_pvp().remove(&(target_pid, my_pid)) else {
                return Ok(());
            };
            let Some((target_ok, target_gold)) = target_info else {
                return Ok(());
            };
            if !target_ok || exp < now_ms {
                return Ok(());
            }
            if my_gold < stake || target_gold < stake {
                return send_thong_bao(session, "Cả hai người chơi cần có đủ vàng để thách đấu!");
            }
            {
                let mut pvp = lock_pvp();
                if pvp.contains_key(&my_pid)
                    || pvp.contains_key(&target_pid)
                    || super::router_shop::is_in_trade(my_pid)
                    || super::router_shop::is_in_trade(target_pid)
                {
                    return Ok(());
                }
                pvp.insert(my_pid, (target_pid, stake));
                pvp.insert(target_pid, (my_pid, stake));
            }
            for pid in [my_pid, target_pid] {
                self.broadcast_pk_type(loc, sid, pid, 3);
            }
        }
        Ok(())
    }

    pub(super) async fn handle_change_flag(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: FlagAction,
    ) -> Result<(), ProtocolError> {
        let tpl_flags: Vec<(i16, i16)> = self
            .shared
            .templates
            .items
            .iter()
            .filter(|r| r.item_type == 28)
            .take(12)
            .map(|r| (r.id as i16, r.icon_id as i16))
            .collect();
        let flags: &[(i16, i16)] = if tpl_flags.is_empty() {
            DEFAULT_FLAGS
        } else {
            &tpl_flags
        };
        match action {
            FlagAction::OpenUi => {
                let mut w = PacketWriter::with_capacity(4 + flags.len() * 9);
                w.write_i8(0);
                w.write_i8(flags.len() as i8);
                for &(tid, _) in flags {
                    w.write_i16(tid);
                    w.write_i8(1);
                    w.write_i16(73);
                    w.write_i32(0);
                }
                session.send(w.finish(cmd_message::CHANGE_FLAG)).await?;
            }
            FlagAction::Choose(idx) => {
                let now_ms = unix_now_ms();
                let res = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .and_then(|mut entry| {
                        if entry.snapshot.stats.hp <= 0 {
                            return None;
                        }
                        let (pid, cf) = (entry.snapshot.id, idx.clamp(0, flags.len() as i8));
                        if entry.c_flag != cf
                            && let Some(last) = flag_cooldown().get(&pid).map(|v| *v.value())
                            && now_ms.saturating_sub(last) < FLAG_CHANGE_COOLDOWN_MS
                        {
                            let rem = ((FLAG_CHANGE_COOLDOWN_MS - now_ms.saturating_sub(last))
                                .saturating_add(999)
                                / 1_000)
                                .max(1);
                            return Some(Err(rem));
                        }
                        entry.c_flag = cf;
                        flag_cooldown().insert(pid, now_ms);
                        let mut w = PacketWriter::with_capacity(6);
                        w.write_i8(1);
                        w.write_i32(pid.0 as i32);
                        w.write_i8(cf);
                        Some(Ok((
                            w.finish(cmd_message::CHANGE_FLAG),
                            entry.snapshot.location,
                        )))
                    });
                match res {
                    Some(Ok((pkt, loc))) => self.broadcast_zone_or_self(loc, sid, &pkt),
                    Some(Err(rem)) => {
                        send_thong_bao(session, &format!("Vui lòng chờ {rem} giây để đổi cờ!"))?
                    }
                    None => {}
                }
            }
            FlagAction::RequestIcon(cf) => {
                let icon = usize::try_from(cf.saturating_sub(1))
                    .ok()
                    .and_then(|i| flags.get(i))
                    .map_or(2325, |&(_, ic)| ic);
                let mut w = PacketWriter::with_capacity(4);
                w.write_i8(2);
                w.write_i8(cf);
                w.write_i16(icon);
                session.send(w.finish(cmd_message::CHANGE_FLAG)).await?;
            }
            FlagAction::Unknown(_) => {}
        }
        Ok(())
    }
}
