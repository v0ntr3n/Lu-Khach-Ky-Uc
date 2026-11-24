//! Inbound packet router implementing [`nro_protocol::SessionEventHandler`] across
//! `DataGameAssets`, `nro-db`, `nro-world`, `nro-economy`, `nro-player`, and `nro-boss`.

#[path = "router_admin.rs"]
mod router_admin;
#[path = "router_auth.rs"]
mod router_auth;
#[path = "router_combat.rs"]
mod router_combat;
#[path = "router_combat_calc.rs"]
mod router_combat_calc;
#[path = "router_combat_pvp.rs"]
mod router_combat_pvp;
#[path = "router_dispatch.rs"]
mod router_dispatch;
#[path = "router_economy.rs"]
mod router_economy;
#[path = "router_economy_use.rs"]
mod router_economy_use;
#[path = "router_misc.rs"]
mod router_misc;
#[path = "router_npc.rs"]
mod router_npc;
#[path = "router_npc_extra.rs"]
mod router_npc_extra;
#[path = "router_npc_home.rs"]
mod router_npc_home;
#[path = "router_secondary.rs"]
mod router_secondary;
#[path = "router_secondary_clan.rs"]
mod router_secondary_clan;
#[path = "router_secondary_clan_ops.rs"]
mod router_secondary_clan_ops;
#[path = "router_shop.rs"]
mod router_shop;
#[path = "router_shop_trade.rs"]
mod router_shop_trade;
#[path = "router_skill.rs"]
mod router_skill;
#[path = "router_skill_effects.rs"]
mod router_skill_effects;
#[path = "router_stats.rs"]
mod router_stats;
#[path = "router_task.rs"]
mod router_task;
#[path = "router_world.rs"]
mod router_world;
#[path = "router_world_tree.rs"]
mod router_world_tree;

pub(crate) use router_combat_calc::apply_incoming_hit_to_player;
pub(crate) use router_skill_effects::apply_boss_debuff_to_player;
pub(super) use router_stats::{DbItemCatalog, send_dialog, send_thong_bao};
#[allow(unused_imports)]
pub(crate) use router_stats::{
    effective_appearance, effective_appearance as resolve_appearance, has_x2_pet_exp_buff,
    is_own_home_map, is_player_in_trade, pkt_fusion, recalculate_session_stats,
    recalculate_snapshot_stats, snapshot_to_economy_inv, sprite_restore_packets,
    sync_economy_inv_to_snapshot, unfuse_if_missing_porata, wallet_money_pkt,
};

use std::sync::{Arc, OnceLock};

use chrono::Utc;
use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{MapId, PlayerId};
use nro_economy::CombineOp;
use nro_protocol::{ClientPacket, Packet, ProtocolError, SessionEventHandler, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tracing::{info, warn};

use crate::runtime::{SharedServerState, unix_now_ms};

static NPC_MENU_INDEX: OnceLock<DashMap<PlayerId, (i16, i32)>> = OnceLock::new();
static TRADE_PARTNERS: OnceLock<DashMap<PlayerId, PlayerId>> = OnceLock::new();
static TRADE_GOLD_OFFERS: OnceLock<DashMap<(PlayerId, PlayerId), (u64, u64)>> = OnceLock::new();
static OPEN_SHOP_STATE: OnceLock<DashMap<PlayerId, String>> = OnceLock::new();
static PENDING_COMBINE: OnceLock<DashMap<PlayerId, (i16, CombineOp, Vec<usize>)>> = OnceLock::new();
static PENDING_INPUT: OnceLock<DashMap<PlayerId, &'static str>> = OnceLock::new();
static CAPSULE_DESTS: OnceLock<DashMap<PlayerId, (bool, Vec<MapId>)>> = OnceLock::new();
static CON_MEO_STATE: OnceLock<DashMap<PlayerId, i32>> = OnceLock::new();

pub(super) fn npc_menu_index() -> &'static DashMap<PlayerId, (i16, i32)> {
    NPC_MENU_INDEX.get_or_init(DashMap::new)
}
pub(super) fn trade_partners() -> &'static DashMap<PlayerId, PlayerId> {
    TRADE_PARTNERS.get_or_init(DashMap::new)
}
pub(super) fn trade_gold_offers() -> &'static DashMap<(PlayerId, PlayerId), (u64, u64)> {
    TRADE_GOLD_OFFERS.get_or_init(DashMap::new)
}
pub(super) fn open_shop_state() -> &'static DashMap<PlayerId, String> {
    OPEN_SHOP_STATE.get_or_init(DashMap::new)
}
pub(super) fn pending_combine() -> &'static DashMap<PlayerId, (i16, CombineOp, Vec<usize>)> {
    PENDING_COMBINE.get_or_init(DashMap::new)
}
pub(super) fn pending_input() -> &'static DashMap<PlayerId, &'static str> {
    PENDING_INPUT.get_or_init(DashMap::new)
}
pub(super) fn capsule_dests() -> &'static DashMap<PlayerId, (bool, Vec<MapId>)> {
    CAPSULE_DESTS.get_or_init(DashMap::new)
}
pub(super) fn con_meo_state() -> &'static DashMap<PlayerId, i32> {
    CON_MEO_STATE.get_or_init(DashMap::new)
}

pub(super) fn clear_transient_player_maps(pid: PlayerId) {
    router_npc::clear_npc_menu_state(pid);
    open_shop_state().remove(&pid);
    pending_combine().remove(&pid);
    pending_input().remove(&pid);
    capsule_dests().remove(&pid);
    con_meo_state().remove(&pid);
    router_skill::clear_skill_state(pid);
    let _ = router_skill::take_monkey_revert_pending(pid);
    router_skill_effects::clear_player_effects(pid);
    router_combat_pvp::clear_flag_cooldown(pid);
    router_secondary::clear_clan_invites(pid);
    router_shop::clear_buyback_ledger(pid);
    router_npc_extra::clear_lucky_round_mode(pid);
    router_npc::clear_npc_session_state(pid);
    router_world_tree::clear_magic_tree_menu_state(pid);
    router_world::clear_chat_global_cooldown(pid);
    crate::runtime::clear_pet_runtime_timers(pid);
}

/// Stateless [`SessionEventHandler`] bridging [`SessionHandle`] I/O tasks to authoritative subsystems.
pub struct ServerSessionHandler {
    shared: Arc<SharedServerState>,
}

impl ServerSessionHandler {
    #[must_use]
    pub const fn new(shared: Arc<SharedServerState>) -> Self {
        Self { shared }
    }
}

impl SessionEventHandler for ServerSessionHandler {
    async fn on_packet(
        &self,
        session: &SessionHandle,
        packet: ClientPacket,
    ) -> Result<(), ProtocolError> {
        if self
            .shared
            .assets
            .try_dispatch_packet(
                session,
                &packet,
                &self.shared.templates,
                &self.shared.config.server_name,
                &self.shared.config.advertised_host(),
                self.shared.config.port,
            )
            .await?
        {
            return Ok(());
        }
        match packet {
            ClientPacket::CheckControllerPing => session.send(Packet::empty(-120)).await,
            ClientPacket::CheckMapPing => session.send(Packet::empty(-121)).await,
            ClientPacket::Login {
                username,
                password,
                version: _,
                login_type,
            } => {
                self.handle_login(session, &username, &password, login_type)
                    .await
            }
            ClientPacket::Register {
                username,
                password,
                guest_user,
            } => {
                self.handle_register(session, &username, &password, guest_user.as_deref())
                    .await
            }
            ClientPacket::CreateChar(name, gender, hair) => {
                if session.player_id().0 != 0 {
                    send_dialog(session, "Tài khoản đã có nhân vật!")
                } else {
                    self.handle_create_char(session, &name, gender, hair).await
                }
            }
            ClientPacket::Login2GuestNotice => {
                if session.account_id().0 > 0 {
                    Ok(())
                } else {
                    self.handle_guest_login(session).await
                }
            }
            ClientPacket::UserInfoDisabled => send_dialog(
                session,
                "Vui lòng đăng ký tài khoản chính thức tại trang chủ!",
            ),
            ClientPacket::LoadRms(key) if session.player_id().0 == 0 => {
                session.send(s2c_packets::key_skill_rms(&key, &[])?).await
            }
            ClientPacket::ClientOk if session.account_id().0 > 0 && session.player_id().0 == 0 => {
                info!(
                    sid = session.id(),
                    account_id = session.account_id().0,
                    "ClientOk before character creation; sending CREATE_PLAYER"
                );
                session
                    .send(Packet::empty(cmd_message::CREATE_PLAYER))
                    .await
            }
            other if session.player_id().0 > 0 => self.dispatch_in_world(session, other).await,
            _ => Ok(()),
        }
    }

    async fn on_disconnect(&self, session: &SessionHandle) {
        let sid = session.id();
        router_economy::clear_session_buffs(sid);
        self.shared.rate_limiter.release(session.ip());
        if let Some((pid, loc)) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| (e.snapshot.id, e.snapshot.location))
            && self
                .shared
                .clients
                .players
                .get(&pid)
                .is_some_and(|s| *s == sid)
        {
            self.cleanup_combat_on_disconnect(pid, loc, sid);
        }
        let detached = {
            let guard = router_shop::trade_lifecycle_lock()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let active_info = self
                .shared
                .clients
                .authenticated
                .get(&sid)
                .map(|e| (e.snapshot.id, e.snapshot.location));
            let still_active = active_info.is_some_and(|(pid, _)| {
                self.shared
                    .clients
                    .players
                    .get(&pid)
                    .is_some_and(|s| *s == sid)
            });
            if let Some((pid, _)) = active_info {
                if still_active {
                    self.cleanup_trade_on_disconnect_unlocked(pid);
                }
                self.shared
                    .clients
                    .players
                    .remove_if(&pid, |_, s| *s == sid);
            }
            let removed = self
                .shared
                .clients
                .authenticated
                .remove(&sid)
                .map(|(_, entry)| (entry, still_active));
            drop(guard);
            removed
        };
        if let Some((entry, still_active)) = detached {
            let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
            if still_active {
                clear_transient_player_maps(pid);
                let now_ms = unix_now_ms();
                router_shop_trade::prune_expired_trade_cooldowns(now_ms);
                if let Some(acc) = entry.snapshot.account_id {
                    let _ = self
                        .shared
                        .accounts_repo
                        .update_logout_time(acc, Utc::now())
                        .await;
                }
                if let Ok(zone_tx) = self
                    .shared
                    .world_router
                    .resolve_sender(loc.map_id, loc.zone_id)
                {
                    let _ = zone_tx
                        .send(ZoneCommand::LeavePlayer { player_id: pid })
                        .await;
                }
                if let Err(err) = self
                    .shared
                    .players_repo
                    .save_player(
                        &entry.snapshot,
                        None,
                        self.shared.config.parity_mode,
                        now_ms,
                    )
                    .await
                {
                    warn!(player_id = pid.0, error = %err, "failed persisting player on disconnect");
                }
            }
            if let Some(acc) = entry.snapshot.account_id {
                self.shared
                    .clients
                    .accounts
                    .remove_if(&acc, |_, s| *s == sid);
            } else {
                self.shared.clients.accounts.retain(|_, s| *s != sid);
            }
        } else {
            self.shared.clients.accounts.retain(|_, s| *s != sid);
        }
        self.shared.clients.sessions.remove(&sid);
    }
}
