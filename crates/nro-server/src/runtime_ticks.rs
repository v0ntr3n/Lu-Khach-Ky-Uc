//! Background maintenance, boss/leaderboard/autosave tick loops, and admin console.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike, Utc};
use nro_boss::spawner::WorldTickContext;
use nro_boss::{EngineEffect, PlayerDebuffKind, PlayerView, SeasonalEvent};
use nro_consts::{BOSS_TICK_MS, LEADERBOARD_REFRESH_MS, LOTTERY_TICK_MS, TOURNAMENT_TICK_MS, cmd_message, const_event};
use nro_core::{GameRng, ItemOption};
use nro_db::player_repo::LeaderboardIndexKind;
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};
use smallvec::SmallVec;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, info, warn};

use super::{LeaderboardRow, LeaderboardSlot, SharedServerState, unix_now_ms};
use crate::router::ServerSessionHandler;

pub(super) fn spawn_auto_maintenance(tracker: &TaskTracker, shared: Arc<SharedServerState>, shutdown: CancellationToken) {
    if !shared.config.auto_maintenance.enabled || !shared.maintenance.try_claim_auto_scheduler() { return; }
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

pub(super) fn spawn_background_ticks(tracker: &TaskTracker, shared: Arc<SharedServerState>, shutdown: CancellationToken) {
    tracker.spawn(async move {
        let mut boss_iv = tokio::time::interval(Duration::from_millis(BOSS_TICK_MS));
        let mut lb_iv = tokio::time::interval(Duration::from_millis(LEADERBOARD_REFRESH_MS));
        let mut lot_iv = tokio::time::interval(Duration::from_millis(LOTTERY_TICK_MS.max(TOURNAMENT_TICK_MS)));
        let mut regen_iv = tokio::time::interval(Duration::from_secs(30));
        let mut autosave_iv = tokio::time::interval(Duration::from_secs(60));
        for iv in [&mut boss_iv, &mut lb_iv, &mut lot_iv, &mut regen_iv, &mut autosave_iv] { iv.set_missed_tick_behavior(MissedTickBehavior::Skip); }
        let mut rng = GameRng::from_seed(0x2026_B055);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
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
                    };
                    let effects = shared.bosses.lock().tick_all(&ctx, &mut rng, unix_now_ms() as u64);
                    if !effects.is_empty() { shared.dispatch_boss_effects(effects).await; }
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
                                rank: (idx + 1) as u32, player_name: e.player_name, score: e.score,
                            }).collect())
                        }
                    }).await;
                }
                _ = lot_iv.tick() => {
                    for s in &shared.clients.sessions { s.value().tick_login_timeout(); }
                    let handler = ServerSessionHandler::new(Arc::clone(&shared));
                    handler.tick_timed_fusions(LOTTERY_TICK_MS.max(TOURNAMENT_TICK_MS) as i32);
                    handler.tick_expired_buffs().await;
                }
                _ = regen_iv.tick() => tick_passive_regen(&shared),
                _ = autosave_iv.tick() => {
                    shared.rate_limiter.prune_idle();
                    let sids: Vec<u64> = shared.clients.authenticated.iter().map(|e| *e.key()).collect();
                    if !sids.is_empty() {
                        let mut saved = 0_usize;
                        for sid in &sids {
                            let Some((snap, now_ms)) = shared.clients.authenticated.get(sid).map(|e| (e.snapshot.clone(), unix_now_ms())) else {
                                continue;
                            };
                            if let Err(err) = shared.players_repo.save_player(&snap, None, shared.config.parity_mode, now_ms).await {
                                warn!(player_id = snap.id.0, error = %err, "periodic autosave failed for player");
                            } else { saved += 1; }
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
        if stats.hp <= 0 { continue; }
        let (hp_max, mp_max, max_stamina) = (stats.hp_max.max(1), stats.mp_max.max(1), stats.max_stamina);
        let new_hp = if stats.hp < hp_max { stats.hp.saturating_add((hp_max / 20).max(1)).min(hp_max) } else { stats.hp };
        let new_mp = if stats.mp < mp_max { stats.mp.saturating_add((mp_max / 20).max(1)).min(mp_max) } else { stats.mp };
        let new_stamina = if max_stamina > 0 && stats.stamina < max_stamina {
            stats.stamina.saturating_add(5).min(max_stamina)
        } else { stats.stamina };
        let (hp_changed, mp_changed, stamina_changed) = (new_hp != stats.hp, new_mp != stats.mp, new_stamina != stats.stamina);
        if !hp_changed && !mp_changed && !stamina_changed { continue; }
        stats.hp = new_hp;
        stats.mp = new_mp;
        stats.stamina = new_stamina;
        if entry.handle.sent_key() {
            if hp_changed { let _ = entry.handle.try_send(s2c_packets::me_load_hp(new_hp)); }
            if mp_changed { let _ = entry.handle.try_send(s2c_packets::me_load_mp(new_mp)); }
            if stamina_changed {
                let _ = entry.handle.try_send(s2c_packets::stamina(new_stamina.min(i16::MAX as u16) as i16));
            }
        }
        let loc = entry.snapshot.location;
        if let (Some(ek), Ok(ztx)) = (entry.entity_key, shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
            let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
        }
    }
}

impl SharedServerState {
    fn resolve_entity_zone(&self, entity: nro_core::PlayerId) -> Option<(nro_core::MapId, nro_core::ZoneId)> {
        self.bosses.lock().instances().get(&entity.0).map(|b| (b.map_id, b.zone_id)).or_else(|| {
            let sid = self.clients.players.get(&entity).map(|s| *s.value())?;
            self.clients.authenticated.get(&sid).map(|e| (e.snapshot.location.map_id, e.snapshot.location.zone_id))
        })
    }

    fn broadcast_zone_pkt(&self, map_id: nro_core::MapId, zone_id: nro_core::ZoneId, pkt: &nro_protocol::Packet) {
        for rec in &self.clients.authenticated {
            let loc = rec.value().snapshot.location;
            if loc.map_id == map_id && loc.zone_id == zone_id && rec.value().handle.sent_key() {
                let _ = rec.value().handle.try_send(pkt.clone());
            }
        }
    }

    fn notify_online_player(&self, target: nro_core::PlayerId, text: &str) {
        if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
            && let Some(entry) = self.clients.authenticated.get(&sid)
            && entry.handle.sent_key()
            && let Ok(pkt) = s2c_packets::thong_bao(text)
        { let _ = entry.handle.try_send(pkt); }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn dispatch_boss_effects(self: &Arc<Self>, effects: Vec<EngineEffect>) {
        for effect in effects {
            match effect {
                EngineEffect::SpawnDynamicBoss { boss_type, map_id, zone_id, location, scaled_stats } => {
                    info!(boss_type = boss_type.0, map_id = map_id.0, zone_id = zone_id.0, x = location.x, y = location.y, scaled = scaled_stats.is_some(), "boss spawned");
                    if !matches!(boss_type.0, 46..=50 | 96) {
                        let map_label = self.templates.maps.iter().find(|m| m.id == map_id)
                            .map_or_else(|| format!("Map {}", map_id.0), |m| m.name.clone());
                        self.clients.broadcast_server_notify(&format!("BOSS #{} vừa xuất hiện tại {map_label} khu {}", boss_type.0, zone_id.0));
                    }
                }
                EngineEffect::DamagePlayer { target, damage, piercing } => {
                    let now_ms = unix_now_ms();
                    let target_sid = self.clients.players.get(&target).map(|s| *s.value());
                    let (mut died_info, mut took_damage, mut hit_broadcast) = (None, false, None);
                    if let Some(sid) = target_sid && let Some(mut entry) = self.clients.authenticated.get_mut(&sid) {
                        let prev_hp = entry.snapshot.stats.hp;
                        let e = &mut *entry;
                        let (out, _) = crate::router::apply_incoming_hit_to_player(sid, target, &mut e.snapshot.stats, &e.snapshot.items_body, damage.max(0), None, (0, 0, 0), piercing, now_ms);
                        let final_dmg = if out.dodged { 0 } else { i64::from(out.damage_taken.max(0)) };
                        took_damage = final_dmg > 0;
                        if entry.handle.sent_key() { let _ = entry.handle.try_send(s2c_packets::me_load_point(&entry.snapshot.stats)); }
                        let loc = entry.snapshot.location;
                        if !ServerSessionHandler::is_offline_map_id(loc.map_id) { hit_broadcast = Some((loc, entry.snapshot.stats.hp, final_dmg)); }
                        if let (Some(ek), Ok(ztx)) = (entry.entity_key, self.world_router.resolve_sender(loc.map_id, loc.zone_id)) { let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats)); }
                        if prev_hp > 0 && entry.snapshot.stats.hp == 0 { died_info = Some((sid, entry.handle.clone(), loc, entry.snapshot.stats.power)); }
                        debug!(player_id = target.0, damage, final_dmg, piercing, hp = entry.snapshot.stats.hp, "applied boss damage effect");
                    }
                    if let Some((loc, hp_after, dmg)) = hit_broadcast { self.broadcast_zone_pkt(loc.map_id, loc.zone_id, &s2c_packets::have_attack_player(target, hp_after, dmg, false, -1)); }
                    if took_damage { ServerSessionHandler::clear_ttnl_for_player(target); }
                    if let Some((victim_sid, victim_handle, victim_loc, victim_power)) = died_info {
                        ServerSessionHandler::new(Arc::clone(self)).handle_player_death_cleanup(target, victim_loc, victim_sid);
                        let (me_die_pkt, peer_die_pkt) = s2c_packets::char_die_packets(target, 0, victim_loc, victim_power);
                        if victim_handle.sent_key() { let _ = victim_handle.try_send(me_die_pkt); }
                        if !ServerSessionHandler::is_offline_map_id(victim_loc.map_id) {
                            for rec in &self.clients.authenticated {
                                if *rec.key() != victim_sid && rec.value().snapshot.location.map_id == victim_loc.map_id
                                    && rec.value().snapshot.location.zone_id == victim_loc.zone_id && rec.value().handle.sent_key()
                                { let _ = rec.value().handle.try_send(peer_die_pkt.clone()); }
                            }
                        }
                    }
                }
                EngineEffect::DropItem(drop) => {
                    if let Ok(ztx) = self.world_router.resolve_sender(drop.location.map_id, drop.location.zone_id) {
                        let kind = match drop.item.template_id.0 {
                            14..=20 => GroundDropKind::EarthDragonBall,
                            353..=359 | 362 => GroundDropKind::NamekBall,
                            372..=378 => GroundDropKind::BlackBall,
                            _ => GroundDropKind::Standard,
                        };
                        let options: SmallVec<[ItemOption; 4]> = drop.item.options.iter().copied().collect();
                        let _ = ztx.try_send(ZoneCommand::DropItemWithOptions(
                            drop.item.template_id, drop.item.quantity, drop.location, drop.owner, kind, options,
                        ));
                    }
                }
                EngineEffect::GrantTnsm { target, amount } => {
                    let gain = amount.max(0);
                    if gain > 0
                        && let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(gain);
                        entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(gain);
                        if entry.handle.sent_key() { let _ = entry.handle.try_send(s2c_packets::me_load_point(&entry.snapshot.stats)); }
                        self.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    }
                }
                EngineEffect::GrantEventPoints { target, points } => {
                    let delta = i32::try_from(points).unwrap_or(i32::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    { entry.snapshot.event_point = entry.snapshot.event_point.saturating_add(delta); }
                }
                EngineEffect::StealGold { target, amount } => {
                    let stolen = i64::try_from(amount).unwrap_or(i64::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.wallet.gold = entry.snapshot.wallet.gold.saturating_sub(stolen).max(0);
                        if entry.handle.sent_key() {
                            let w = &entry.snapshot.wallet;
                            let pkt = s2c_packets::send_money(w.gold.max(0).cast_unsigned(), u64::from(w.gem.max(0).cast_unsigned()), u64::from(w.ruby.max(0).cast_unsigned()));
                            let _ = entry.handle.try_send(pkt);
                        }
                    }
                }
                EngineEffect::WipeZoneMobs { map_id, zone_id } => {
                    if let Ok(ztx) = self.world_router.resolve_sender(map_id, zone_id) { let _ = ztx.try_send(ZoneCommand::WipeMobs); }
                    info!(map_id = map_id.0, zone_id = zone_id.0, "boss wiped zone mobs");
                }
                EngineEffect::ChangePk { entity, mode } => {
                    if let Some((map_id, zone_id)) = self.resolve_entity_zone(entity) && !ServerSessionHandler::is_offline_map_id(map_id) {
                        let mut pk = s2c_packets::sub_command_writer(cmd_message::UPDATE_TYPE_PK, 5);
                        pk.write_i32(entity.0 as i32); pk.write_i8((mode as u8).cast_signed());
                        self.broadcast_zone_pkt(map_id, zone_id, &pk.finish(cmd_message::SUB_COMMAND));
                    }
                }
                EngineEffect::GrantNormalPet { target } => {
                    if let Some((sid, handle)) = self.clients.players.get(&target).map(|s| *s.value())
                        .and_then(|sid| self.clients.authenticated.get(&sid).map(|e| (sid, e.handle.clone())))
                    {
                        let shared = Arc::clone(self);
                        tokio::spawn(async move {
                            if let Err(err) = ServerSessionHandler::new(shared).grant_starter_pet_for_effect(&handle, sid).await {
                                warn!(player_id = target.0, error = %err, "boss grant normal pet failed");
                            }
                        });
                    }
                }
                EngineEffect::AwardMayDamPoints { target, points } => {
                    let delta = i32::try_from(points).unwrap_or(i32::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    { entry.snapshot.point_maydam = entry.snapshot.point_maydam.saturating_add(delta); }
                    self.notify_online_player(target, &format!("Bạn nhận được +{points} điểm Máy Đấm!"));
                }
                EngineEffect::UnlockSuperDivineWater { target } => {
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    { entry.snapshot.sieu_than_thuy.0 = true; }
                    self.notify_online_player(target, "Bạn đã mở khóa Siêu Thần Thủy!");
                }
                EngineEffect::DebuffPlayer { target, kind, duration_ms } => {
                    let applied = self.clients.players.get(&target).map(|s| *s.value()).and_then(|sid| {
                        self.clients.authenticated.get(&sid).map(|e| crate::router::apply_boss_debuff_to_player(target, kind, u64::from(duration_ms), &e.snapshot.items_body, unix_now_ms()))
                    }).unwrap_or(true);
                    if !applied { self.notify_online_player(target, "Bạn đã kháng hiệu ứng choáng!"); continue; }
                    let secs = duration_ms.div_ceil(1_000).max(1);
                    let label = match kind {
                        PlayerDebuffKind::Stun => "bị choáng",
                        PlayerDebuffKind::PetrifyStone => "bị hóa đá",
                        PlayerDebuffKind::Chocolate => "bị biến thành Sô-cô-la",
                        PlayerDebuffKind::BodyChange => "bị hoán đổi cơ thể",
                        PlayerDebuffKind::GravitySlow => "bị làm chậm trọng lực",
                        PlayerDebuffKind::HalloweenCurse(_) => "bị dính lời nguyền Halloween",
                        PlayerDebuffKind::Contagion { lethal_on_expire: true } => "bị nhiễm độc chí mạng",
                        PlayerDebuffKind::Contagion { lethal_on_expire: false } => "bị thiêu đốt",
                    };
                    self.notify_online_player(target, &format!("Bạn đã {label} trong {secs} giây!"));
                }
                EngineEffect::Chat { speaker, text } => {
                    if let Some((map_id, zone_id)) = self.resolve_entity_zone(speaker)
                        && !ServerSessionHandler::is_offline_map_id(map_id)
                        && let Ok(pkt) = s2c_packets::chat_map(speaker, &text)
                    { self.broadcast_zone_pkt(map_id, zone_id, &pkt); }
                    else { debug!(speaker = speaker.0, %text, "boss chat"); }
                }
                EngineEffect::TeleportPlayer { target, map_id, zone_id, location } => {
                    if let Some((sid, handle)) = self.clients.players.get(&target).map(|s| *s.value())
                        .and_then(|sid| self.clients.authenticated.get(&sid).map(|e| (sid, e.handle.clone())))
                    {
                        let shared = Arc::clone(self);
                        tokio::spawn(async move {
                            if let Err(err) = ServerSessionHandler::new(shared).teleport_player(&handle, sid, map_id, zone_id, (location.x, location.y)).await {
                                warn!(player_id = target.0, map_id = map_id.0, zone_id = zone_id.0, error = %err, "boss teleport player failed");
                            }
                        });
                    }
                }
                EngineEffect::DistributeClanCapsules(grants) => {
                    for (target, _clan_id, capsules) in grants {
                        self.notify_online_player(target, &format!("Bang hội nhận được +{capsules} Capsule Bang!"));
                    }
                }
            }
        }
    }
}

pub(super) fn spawn_admin_console(tracker: &TaskTracker, shared: Arc<SharedServerState>, shutdown: CancellationToken) {
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
