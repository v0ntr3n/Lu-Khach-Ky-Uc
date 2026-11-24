//! Synchronous session eviction (`evict_online_session`, `evict_account_sessions`) and `ZoneActor` ingress helper.

use chrono::Utc;
use nro_core::{AccountId, Location};
use nro_db::player_repo::PlayerSnapshot;
use nro_protocol::Packet;
use nro_world::{PlayerIngress, ZoneCommand};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use super::super::super::{clear_transient_player_maps, router_economy, router_shop};
use super::super::ServerSessionHandler;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    /// Synchronously detaches `old_sid`, cleans combat/trade/zone state, and persists its snapshot to `fjall`.
    pub(in crate::router) async fn evict_online_session(&self, old_sid: u64) {
        router_economy::clear_session_buffs(old_sid);
        if let Some((pid, loc)) = self
            .shared
            .clients
            .authenticated
            .get(&old_sid)
            .map(|e| (e.snapshot.id, e.snapshot.location))
        {
            self.cleanup_combat_on_disconnect(pid, loc, old_sid);
        }
        let evicted = {
            let guard = router_shop::trade_lifecycle_lock()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(pid) = self
                .shared
                .clients
                .authenticated
                .get(&old_sid)
                .map(|e| e.snapshot.id)
            {
                self.cleanup_trade_on_disconnect_unlocked(pid);
            }
            let removed = self.shared.clients.take_session_for_eviction(old_sid);
            drop(guard);
            removed
        };
        if let Some(entry) = evicted {
            let (pid, loc, now_ms) = (entry.snapshot.id, entry.snapshot.location, unix_now_ms());
            clear_transient_player_maps(pid);
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
                warn!(player_id = pid.0, error = %err, "failed saving evicted player snapshot");
            }
            if let Some(acc) = entry.snapshot.account_id {
                let _ = self
                    .shared
                    .accounts_repo
                    .update_logout_time(acc, Utc::now())
                    .await;
            }
        }
    }

    /// Synchronously evicts and persists all sessions attached to `account_id` except `except_sid`.
    pub(in crate::router) async fn evict_account_sessions(
        &self,
        account_id: AccountId,
        except_sid: u64,
    ) {
        let mut sids = Vec::new();
        if let Some(sid) = self.shared.clients.accounts.get(&account_id).map(|s| *s)
            && sid != except_sid
        {
            sids.push(sid);
        }
        for rec in &self.shared.clients.authenticated {
            let sid = *rec.key();
            if sid != except_sid
                && rec.value().snapshot.account_id == Some(account_id)
                && !sids.contains(&sid)
            {
                sids.push(sid);
            }
        }
        for old_sid in sids {
            self.evict_online_session(old_sid).await;
        }
    }

    pub(super) async fn try_enter_zone(
        &self,
        account_id: AccountId,
        session_key: nro_core::SessionKey,
        snap: &PlayerSnapshot,
        loc: Location,
        parts: (i16, i16, i16),
        outbound_tx: mpsc::Sender<Packet>,
    ) -> Option<nro_core::EntityKey> {
        let zone_tx = self
            .shared
            .world_router
            .resolve_sender(loc.map_id, loc.zone_id)
            .ok()?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let ingress = PlayerIngress {
            session: (account_id, session_key),
            identity: (snap.id, (snap.clan_id.0 >= 0).then_some(snap.clan_id)),
            gender: snap.gender,
            name: snap.name.clone(),
            loc,
            stats: snap.stats,
            parts,
            ship_type: 0,
            outbound_tx: Some(outbound_tx),
        };
        if zone_tx
            .send(ZoneCommand::EnterPlayer(Box::new(ingress), reply_tx))
            .await
            .is_ok()
            && let Ok(Ok(ek)) = reply_rx.await
        {
            return Some(ek);
        }
        None
    }
}
