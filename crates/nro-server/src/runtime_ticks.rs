//! Background maintenance, boss/leaderboard/autosave tick loops, and admin console.

#[path = "runtime_boss_fx.rs"]
mod runtime_boss_fx;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike, Utc};
use nro_boss::spawner::WorldTickContext;
use nro_boss::{PlayerView, SeasonalEvent};
use nro_consts::{
    BOSS_TICK_MS, LEADERBOARD_REFRESH_MS, LOTTERY_TICK_MS, TOURNAMENT_TICK_MS, const_event,
};
use nro_core::GameRng;
use nro_db::player_repo::LeaderboardIndexKind;
use nro_world::{ZoneCommand, s2c_packets};
use smallvec::SmallVec;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, info, warn};

use super::{LeaderboardRow, LeaderboardSlot, SharedServerState, unix_now_ms};
use crate::router::ServerSessionHandler;

impl SharedServerState {
    pub(self) fn resolve_entity_zone(
        &self,
        entity: nro_core::PlayerId,
    ) -> Option<(nro_core::MapId, nro_core::ZoneId)> {
        if let Some(zone) = self
            .bosses
            .lock()
            .instances()
            .get(&entity.0)
            .map(|b| (b.map_id, b.zone_id))
        {
            return Some(zone);
        }
        let sid = self.clients.players.get(&entity).map(|s| *s.value())?;
        self.clients
            .authenticated
            .get(&sid)
            .map(|e| (e.snapshot.location.map_id, e.snapshot.location.zone_id))
    }

    pub(self) fn broadcast_zone_pkt(
        &self,
        map_id: nro_core::MapId,
        zone_id: nro_core::ZoneId,
        pkt: &nro_protocol::Packet,
    ) {
        for rec in &self.clients.authenticated {
            let loc = rec.value().snapshot.location;
            if loc.map_id == map_id && loc.zone_id == zone_id && rec.value().handle.sent_key() {
                let _ = rec.value().handle.try_send(pkt.clone());
            }
        }
    }

    pub(self) fn notify_online_player(&self, target: nro_core::PlayerId, text: &str) {
        if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
            && let Some(entry) = self.clients.authenticated.get(&sid)
            && entry.handle.sent_key()
            && let Ok(pkt) = s2c_packets::thong_bao(text)
        {
            let _ = entry.handle.try_send(pkt);
        }
    }
}

pub(super) fn spawn_auto_maintenance(
    tracker: &TaskTracker,
    shared: Arc<SharedServerState>,
    shutdown: CancellationToken,
) {
    if !shared.config.auto_maintenance.enabled || !shared.maintenance.try_claim_auto_scheduler() {
        return;
    }
    tracker.spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                _ = tick.tick() => {
                    let (now, s) = (Local::now(), shared.config.auto_maintenance);
                    if now.hour() == u32::from(s.hour) && now.minute() == u32::from(s.minute) {
                        if shared.maintenance.try_begin_maintenance() {
                            shared.clients.broadcast_server_notify("Máy chủ bảo trì định kỳ sau ít giây!");
                            tokio::select! {
                                () = shutdown.cancelled() => break,
                                () = tokio::time::sleep(Duration::from_secs(u64::from(s.countdown_secs))) => {}
                            }
                            shutdown.cancel();
                        }
                        break;
                    }
                }
            }
        }
    });
}

pub(super) fn spawn_background_ticks(
    tracker: &TaskTracker,
    shared: Arc<SharedServerState>,
    shutdown: CancellationToken,
) {
    let (autosave_shared, autosave_shutdown) = (Arc::clone(&shared), shutdown.clone());
    tracker.spawn(async move {
        let mut autosave_iv = tokio::time::interval(Duration::from_secs(60));
        autosave_iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                () = autosave_shutdown.cancelled() => break,
                _ = autosave_iv.tick() => {
                    autosave_shared.rate_limiter.prune_idle();
                    let sids: Vec<u64> = autosave_shared
                        .clients
                        .authenticated
                        .iter()
                        .map(|e| *e.key())
                        .collect();
                    if !sids.is_empty() {
                        let mut saved = 0_usize;
                        for sid in &sids {
                            let Some((snap, now_ms)) = autosave_shared
                                .clients
                                .authenticated
                                .get(sid)
                                .map(|e| (e.snapshot.clone(), unix_now_ms()))
                            else {
                                continue;
                            };
                            if let Err(err) = autosave_shared
                                .players_repo
                                .save_player(&snap, None, autosave_shared.config.parity_mode, now_ms)
                                .await
                            {
                                warn!(player_id = snap.id.0, error = %err, "periodic autosave failed for player");
                            } else {
                                saved += 1;
                            }
                        }
                        debug!(saved, total = sids.len(), "completed periodic player autosave");
                    }
                }
            }
        }
    });

    tracker.spawn(async move {
        let mut boss_iv = tokio::time::interval(Duration::from_millis(BOSS_TICK_MS));
        let mut lb_iv = tokio::time::interval(Duration::from_millis(LEADERBOARD_REFRESH_MS));
        let mut lot_iv = tokio::time::interval(Duration::from_millis(
            LOTTERY_TICK_MS.max(TOURNAMENT_TICK_MS),
        ));
        let mut regen_iv = tokio::time::interval(Duration::from_secs(30));
        let mut magnet_iv = tokio::time::interval(Duration::from_millis(500));
        for iv in [
            &mut boss_iv,
            &mut lb_iv,
            &mut lot_iv,
            &mut regen_iv,
            &mut magnet_iv,
        ] {
            iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
        }
        let map_zones: BTreeMap<u16, u16> = shared
            .templates
            .maps
            .iter()
            .filter(|m| m.id.0 >= 0)
            .map(|m| {
                let zc = if ServerSessionHandler::is_offline_map_id(m.id) || m.map_type == 1 {
                    u16::from(m.zones.max(1))
                } else {
                    u16::from(m.zones.max(2))
                };
                (m.id.0.cast_unsigned(), zc)
            })
            .collect();
        let mut rng = GameRng::from_seed(0x2026_B055);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                _ = magnet_iv.tick() => {
                    ServerSessionHandler::new(Arc::clone(&shared))
                        .sweep_magnet_pickups()
                        .await;
                }
                _ = boss_iv.tick() => {
                    let mut zone_players: BTreeMap<(u16, u16), usize> = BTreeMap::new();
                    let mut zone_rosters: BTreeMap<(u16, u16), Vec<PlayerView>> = BTreeMap::new();
                    for entry in &shared.clients.authenticated {
                        let loc = entry.value().snapshot.location;
                        let zkey = (loc.map_id.0.cast_unsigned(), loc.zone_id.0);
                        *zone_players.entry(zkey).or_insert(0) += 1;
                        if entry.value().snapshot.stats.hp > 0 {
                            zone_rosters.entry(zkey).or_default().push(
                                ServerSessionHandler::active_entry_to_player_view(entry.value()),
                            );
                        }
                    }
                    let now_hour = (Utc::now() + chrono::Duration::hours(7)).hour();
                    let mut active_seasons = SmallVec::new();
                    if let Some(season) = match i8::try_from(shared.config.active_seasonal_event_id) {
                        Ok(const_event::SU_KIEN_HALLOWEEN) => Some(SeasonalEvent::Halloween),
                        Ok(const_event::SU_KIEN_NOEL) => Some(SeasonalEvent::Christmas),
                        Ok(const_event::SU_KIEN_TET) => Some(SeasonalEvent::Tet),
                        Ok(const_event::SU_KIEN_HUNG_VUONG) => Some(SeasonalEvent::HungVuong),
                        Ok(const_event::SU_KIEN_TRUNG_THU) => Some(SeasonalEvent::TrungThu),
                        _ => None,
                    } {
                        active_seasons.push(season);
                    }
                    let ctx = WorldTickContext {
                        ict_hour: now_hour as u8,
                        mabu_12h_open: now_hour == 12,
                        mabu_14h_open: now_hour == 14,
                        active_seasons,
                        zone_players,
                        zone_rosters,
                        map_zones: map_zones.clone(),
                    };
                    let effects = shared.bosses.lock().tick_all(&ctx, &mut rng, unix_now_ms() as u64);
                    if !effects.is_empty() {
                        shared.dispatch_boss_effects(effects).await;
                    }
                }
                _ = lb_iv.tick() => {
                    let repo = shared.players_repo.clone();
                    let _ = shared.leaderboards.refresh_dirty_with(|slot| {
                        let repo = repo.clone();
                        async move {
                            let kind = match slot {
                                LeaderboardSlot::Power => LeaderboardIndexKind::Power,
                                LeaderboardSlot::Task => LeaderboardIndexKind::Task,
                                LeaderboardSlot::SuKien1 => LeaderboardIndexKind::SuKien1,
                                LeaderboardSlot::SuKien2 => LeaderboardIndexKind::SuKien2,
                            };
                            let entries = repo.top_leaderboard(kind, 100)?;
                            Ok(entries.into_iter().enumerate().map(|(idx, e)| LeaderboardRow {
                                rank: (idx + 1) as u32,
                                player_name: e.player_name,
                                score: e.score,
                            }).collect())
                        }
                    }).await;
                }
                _ = lot_iv.tick() => {
                    for s in &shared.clients.sessions {
                        s.value().tick_login_timeout();
                    }
                    let handler = ServerSessionHandler::new(Arc::clone(&shared));
                    handler.tick_timed_fusions(LOTTERY_TICK_MS.max(TOURNAMENT_TICK_MS) as i32).await;
                    handler.tick_expired_buffs().await;
                }
                _ = regen_iv.tick() => {
                    tick_passive_regen(&shared);
                    super::runtime_pet::tick_passive_pet_regen(&shared).await;
                }
            }
        }
    });
}

fn tick_passive_regen(shared: &SharedServerState) {
    for mut entry in shared.clients.authenticated.iter_mut() {
        let stats = &mut entry.snapshot.stats;
        if stats.hp <= 0 {
            continue;
        }
        let (hp_max, mp_max, max_stamina) =
            (stats.hp_max.max(1), stats.mp_max.max(1), stats.max_stamina);
        let new_hp = if stats.hp < hp_max {
            stats.hp.saturating_add((hp_max / 20).max(1)).min(hp_max)
        } else {
            stats.hp
        };
        let new_mp = if stats.mp < mp_max {
            stats.mp.saturating_add((mp_max / 20).max(1)).min(mp_max)
        } else {
            stats.mp
        };
        let new_stamina = if max_stamina > 0 && stats.stamina < max_stamina {
            stats.stamina.saturating_add(5).min(max_stamina)
        } else {
            stats.stamina
        };
        let (hp_changed, mp_changed, stamina_changed) = (
            new_hp != stats.hp,
            new_mp != stats.mp,
            new_stamina != stats.stamina,
        );
        if !hp_changed && !mp_changed && !stamina_changed {
            continue;
        }
        stats.hp = new_hp;
        stats.mp = new_mp;
        stats.stamina = new_stamina;
        if entry.handle.sent_key() {
            if hp_changed {
                let _ = entry.handle.try_send(s2c_packets::me_load_hp(new_hp));
            }
            if mp_changed {
                let _ = entry.handle.try_send(s2c_packets::me_load_mp(new_mp));
            }
            if stamina_changed {
                let _ = entry
                    .handle
                    .try_send(s2c_packets::stamina(new_stamina.min(i16::MAX as u16) as i16));
            }
        }
        let loc = entry.snapshot.location;
        if let (Some(ek), Ok(ztx)) = (
            entry.entity_key,
            shared.world_router.resolve_sender(loc.map_id, loc.zone_id),
        ) {
            let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
        }
    }
}

pub(super) fn spawn_admin_console(
    tracker: &TaskTracker,
    shared: Arc<SharedServerState>,
    shutdown: CancellationToken,
) {
    tracker.spawn(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                res = lines.next_line() => {
                    let Ok(Some(line)) = res else { break };
                    let cmd = line.trim();
                    if cmd == "stats" {
                        info!(sessions = shared.clients.sessions.len(), players = shared.clients.authenticated.len(), zones = shared.world_router.zone_count(), uptime_s = shared.started_at.elapsed().as_secs(), "nro-server stats");
                    } else if cmd.starts_with("baotri") {
                        let _ = shared.maintenance.try_begin_maintenance();
                        shutdown.cancel();
                    } else if let Some(target) = cmd.strip_prefix("kick ") {
                        info!(target = target.trim(), kicked = shared.clients.kick_by_target(target), "admin kick");
                    } else if !cmd.is_empty() {
                        error!(%cmd, "unknown operator command (expected stats, baotri, kick <name>)");
                    }
                }
            }
        }
    });
}
