//! Background maintenance, boss/leaderboard/autosave tick loops, and admin console.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike};
use nro_boss::EngineEffect;
use nro_boss::spawner::WorldTickContext;
use nro_consts::{BOSS_TICK_MS, LEADERBOARD_REFRESH_MS, LOTTERY_TICK_MS, TOURNAMENT_TICK_MS};
use nro_core::GameRng;
use nro_db::player_repo::LeaderboardIndexKind;
use nro_world::{ZoneCommand, s2c_packets};
use smallvec::SmallVec;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, info, warn};

use super::{LeaderboardRow, LeaderboardSlot, SharedServerState, unix_now_ms};
use crate::router::ServerSessionHandler;

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
                    let now = Local::now();
                    let s = shared.config.auto_maintenance;
                    if now.hour() == u32::from(s.hour) && now.minute() == u32::from(s.minute) {
                        if shared.maintenance.try_begin_maintenance() {
                            shared.clients.broadcast_server_notify("Máy chủ bảo trì định kỳ sau ít giây!");
                            tokio::time::sleep(Duration::from_secs(u64::from(s.countdown_secs))).await;
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
    tracker.spawn(async move {
        let mut boss_iv = tokio::time::interval(Duration::from_millis(BOSS_TICK_MS));
        let mut lb_iv = tokio::time::interval(Duration::from_millis(LEADERBOARD_REFRESH_MS));
        let mut lot_iv = tokio::time::interval(Duration::from_millis(LOTTERY_TICK_MS.max(TOURNAMENT_TICK_MS)));
        let mut regen_iv = tokio::time::interval(Duration::from_secs(30));
        regen_iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut autosave_iv = tokio::time::interval(Duration::from_secs(60));
        autosave_iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut rng = GameRng::from_seed(0x2026_B055);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                _ = boss_iv.tick() => {
                    let mut zone_players: BTreeMap<(u16, u16), usize> = BTreeMap::new();
                    for entry in &shared.clients.authenticated {
                        let loc = entry.value().snapshot.location;
                        *zone_players.entry((loc.map_id.0.cast_unsigned(), loc.zone_id.0)).or_insert(0) += 1;
                    }
                    let now_hour = Local::now().hour();
                    let ctx = WorldTickContext {
                        ict_hour: now_hour as u8,
                        mabu_12h_open: now_hour == 12,
                        mabu_14h_open: now_hour == 14,
                        active_seasons: SmallVec::new(),
                        zone_players,
                        zone_rosters: BTreeMap::new(),
                    };
                    let effects = shared.bosses.lock().tick_all(&ctx, &mut rng, unix_now_ms() as u64);
                    if !effects.is_empty() {
                        dispatch_boss_effects(&shared, effects);
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
                            Ok(entries
                                .into_iter()
                                .enumerate()
                                .map(|(idx, e)| LeaderboardRow {
                                    rank: (idx + 1) as u32,
                                    player_name: e.player_name,
                                    score: e.score,
                                })
                                .collect())
                        }
                    }).await;
                }
                _ = lot_iv.tick() => {
                    for s in &shared.clients.sessions { s.value().tick_login_timeout(); }
                }
                _ = regen_iv.tick() => {
                    tick_passive_regen(&shared);
                }
                _ = autosave_iv.tick() => {
                    shared.rate_limiter.prune_idle();
                    let now_ms = unix_now_ms();
                    let sids: Vec<u64> = shared
                        .clients
                        .authenticated
                        .iter()
                        .map(|e| *e.key())
                        .collect();
                    if !sids.is_empty() {
                        let mut saved = 0_usize;
                        for sid in &sids {
                            let Some(snap) = shared.clients.authenticated.get(sid).map(|e| e.snapshot.clone()) else {
                                continue;
                            };
                            if let Err(err) = shared
                                .players_repo
                                .save_player(&snap, None, shared.config.parity_mode, now_ms)
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
}

fn tick_passive_regen(shared: &SharedServerState) {
    for mut entry in shared.clients.authenticated.iter_mut() {
        let stats = &mut entry.snapshot.stats;
        if stats.hp <= 0 {
            continue;
        }
        let hp_max = stats.hp_max.max(1);
        let mp_max = stats.mp_max.max(1);
        let max_stamina = stats.max_stamina;
        let hp_gain = (hp_max / 20).max(1);
        let mp_gain = (mp_max / 20).max(1);
        let stamina_gain: u16 = 5;

        let new_hp = if stats.hp < hp_max {
            stats.hp.saturating_add(hp_gain).min(hp_max)
        } else {
            stats.hp
        };
        let new_mp = if stats.mp < mp_max {
            stats.mp.saturating_add(mp_gain).min(mp_max)
        } else {
            stats.mp
        };
        let new_stamina = if max_stamina > 0 && stats.stamina < max_stamina {
            stats.stamina.saturating_add(stamina_gain).min(max_stamina)
        } else {
            stats.stamina
        };

        let hp_changed = new_hp != stats.hp;
        let mp_changed = new_mp != stats.mp;
        let stamina_changed = new_stamina != stats.stamina;
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
                let _ = entry.handle.try_send(s2c_packets::stamina(new_stamina.min(i16::MAX as u16) as i16));
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

fn dispatch_boss_effects(shared: &Arc<SharedServerState>, effects: Vec<EngineEffect>) {
    for effect in effects {
        match effect {
            EngineEffect::SpawnDynamicBoss { boss_type, map_id, zone_id, location, scaled_stats } => {
                let map_label = shared
                    .templates
                    .maps
                    .iter()
                    .find(|m| m.id == map_id)
                    .map_or_else(|| format!("Map {}", map_id.0), |m| m.name.clone());
                info!(
                    boss_type = boss_type.0,
                    map_id = map_id.0,
                    zone_id = zone_id.0,
                    x = location.x,
                    y = location.y,
                    scaled = scaled_stats.is_some(),
                    "boss spawned"
                );
                shared.clients.broadcast_server_notify(&format!(
                    "BOSS #{} vừa xuất hiện tại {map_label} khu {}",
                    boss_type.0, zone_id.0
                ));
            }
            EngineEffect::DamagePlayer { target, damage, piercing } => {
                let target_sid = shared.clients.players.get(&target).map(|s| *s.value());
                let mut died_info = None;
                let mut took_damage = false;
                if let Some(sid) = target_sid
                    && let Some(mut entry) = shared.clients.authenticated.get_mut(&sid)
                {
                    let prev_hp = entry.snapshot.stats.hp;
                    let dmg = damage.max(0);
                    entry.snapshot.stats.hp = prev_hp.saturating_sub(dmg).max(0);
                    took_damage = dmg > 0;
                    if entry.handle.sent_key() {
                        let _ = entry.handle.try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
                    }
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(ztx)) = (
                        entry.entity_key,
                        shared.world_router.resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    if prev_hp > 0 && entry.snapshot.stats.hp == 0 {
                        died_info = Some((sid, entry.handle.clone(), loc, entry.snapshot.stats.power));
                    }
                    debug!(player_id = target.0, damage, piercing, hp = entry.snapshot.stats.hp, "applied boss damage effect");
                }
                if took_damage {
                    ServerSessionHandler::clear_ttnl_for_player(target);
                }
                if let Some((victim_sid, victim_handle, victim_loc, victim_power)) = died_info {
                    ServerSessionHandler::new(Arc::clone(shared))
                        .handle_player_death_cleanup(target, victim_loc, victim_sid);
                    let (me_die_pkt, peer_die_pkt) =
                        s2c_packets::char_die_packets(target, 0, victim_loc, victim_power);
                    if victim_handle.sent_key() {
                        let _ = victim_handle.try_send(me_die_pkt);
                    }
                    for rec in &shared.clients.authenticated {
                        if *rec.key() != victim_sid
                            && rec.value().snapshot.location.map_id == victim_loc.map_id
                            && rec.value().snapshot.location.zone_id == victim_loc.zone_id
                            && rec.value().handle.sent_key()
                        {
                            let _ = rec.value().handle.try_send(peer_die_pkt.clone());
                        }
                    }
                }
            }
            EngineEffect::GrantTnsm { target, amount } => {
                let gain = amount.max(0);
                let target_sid = shared.clients.players.get(&target).map(|s| *s.value());
                if gain > 0
                    && let Some(sid) = target_sid
                    && let Some(mut entry) = shared.clients.authenticated.get_mut(&sid)
                {
                    entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(gain);
                    entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(gain);
                    if entry.handle.sent_key() {
                        let _ = entry.handle.try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
                    }
                    shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                }
            }
            EngineEffect::GrantEventPoints { target, points } => {
                let delta = i32::try_from(points).unwrap_or(i32::MAX);
                let target_sid = shared.clients.players.get(&target).map(|s| *s.value());
                if let Some(sid) = target_sid
                    && let Some(mut entry) = shared.clients.authenticated.get_mut(&sid)
                {
                    entry.snapshot.event_point = entry.snapshot.event_point.saturating_add(delta);
                }
            }
            EngineEffect::StealGold { target, amount } => {
                let stolen = i64::try_from(amount).unwrap_or(i64::MAX);
                let target_sid = shared.clients.players.get(&target).map(|s| *s.value());
                if let Some(sid) = target_sid
                    && let Some(mut entry) = shared.clients.authenticated.get_mut(&sid)
                {
                    entry.snapshot.wallet.gold = entry.snapshot.wallet.gold.saturating_sub(stolen).max(0);
                }
            }
            EngineEffect::WipeZoneMobs { map_id, zone_id } => {
                info!(map_id = map_id.0, zone_id = zone_id.0, "boss wiped zone mobs");
            }
            EngineEffect::Chat { speaker, text } => {
                debug!(speaker = speaker.0, %text, "boss chat");
            }
            other => {
                debug!(?other, "boss engine effect emitted");
            }
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
