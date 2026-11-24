//! Authoritative server runtime replacing `ServerManager.java` and `Manager.java`.

#[path = "runtime_pet.rs"]
mod runtime_pet;
#[path = "runtime_store.rs"]
mod runtime_store;
#[path = "runtime_ticks.rs"]
mod runtime_ticks;

pub(crate) use runtime_pet::{clear_dead_pet_timer, clear_pet_runtime_timers};
pub use runtime_store::unix_now_ms;
#[cfg(test)]
pub(crate) use runtime_store::{discover_bootstrap_sql_in, open_or_bootstrap_store_in};

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::Utc;
use dashmap::DashMap;
use nro_boss::spawner::BossGroupManager;
use nro_core::{GameRng, Location, PlayerId, ZoneId};
use nro_db::account_repo::AccountRepo;
use nro_db::player_repo::PlayerRepo;
use nro_db::pool::DbPool;
use nro_db::templates::GameTemplates;
use nro_db::world_repos::{ClanRepo, ConsignShopRepo, SuperRankRepo};
use nro_economy::{ConsignShop, TradeSession};
use nro_world::{
    ChonAiDayPool, WorldRouter, ZoneActor, ZoneCommand, ZoneConfig, ZoneEvent, ZoneMobSeed,
};
use parking_lot::Mutex;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

use crate::config::ServerConfig;
use crate::data_game::DataGameAssets;
pub use crate::state::{
    ActivePlayerEntry, ClientRegistry, IpRateLimiter, LeaderboardCache, LeaderboardRow,
    LeaderboardSlot, MaintenanceCoordinator,
};
use runtime_store::{
    consign_shop_from_records, consign_shop_to_records, open_or_bootstrap_store, spawn_accept_loop,
    spawn_zone_event_consumer,
};

/// Shared server resources passed to session routers and background supervisors.
pub struct SharedServerState {
    pub config: ServerConfig,
    pub pool: DbPool,
    pub assets: DataGameAssets,
    pub accounts_repo: AccountRepo,
    pub players_repo: PlayerRepo,
    pub clans_repo: ClanRepo,
    pub consign_repo: ConsignShopRepo,
    #[allow(dead_code)]
    pub super_rank_repo: SuperRankRepo,
    pub templates: Arc<GameTemplates>,
    pub rate_limiter: Arc<IpRateLimiter>,
    pub clients: ClientRegistry,
    pub leaderboards: LeaderboardCache,
    pub maintenance: MaintenanceCoordinator,
    pub world_router: WorldRouter,
    pub bosses: Mutex<BossGroupManager>,
    pub consign_shop: Mutex<ConsignShop>,
    pub active_trades: DashMap<(PlayerId, PlayerId), TradeSession>,
    #[allow(dead_code)]
    pub chon_ai_day: Mutex<ChonAiDayPool>,
    pub started_at: Instant,
}

impl SharedServerState {
    /// Persists the in-memory consignment market snapshot to `fjall`.
    pub(crate) async fn persist_consign_shop(&self) {
        let records = match consign_shop_to_records(&self.consign_shop.lock()) {
            Ok(records) => records,
            Err(err) => {
                warn!(error = %err, "failed serializing consign_shop; aborting persistence");
                return;
            }
        };
        if let Err(err) = self.consign_repo.save_all(&records).await {
            warn!(error = %err, "failed persisting consign_shop");
        }
    }
}

/// Running `nro-server` supervisor owning `TaskTracker` and `CancellationToken`.
pub struct ServerRuntime {
    shared: Arc<SharedServerState>,
    tracker: TaskTracker,
    session_tracker: TaskTracker,
    shutdown: CancellationToken,
}

impl ServerRuntime {
    /// Opens embedded `fjall`, loads `SRC/data` binary assets, spawns `ZoneActor`s, and binds TCP `14445`.
    pub async fn boot(config: ServerConfig, shutdown: CancellationToken) -> anyhow::Result<Self> {
        let pool = open_or_bootstrap_store(&config).await?;
        pool.ensure_event_table()
            .await
            .context("failed to seed `events` keyspace")?;
        let templates = Arc::new(
            GameTemplates::load_all(&pool)
                .await
                .context("failed to load GameTemplates")?,
        );
        let assets = DataGameAssets::open(&config.storage.assets_dir);
        let tracker = TaskTracker::new();
        let session_tracker = TaskTracker::new();
        let world_router = WorldRouter::new();
        let (zone_event_tx, zone_event_rx) = mpsc::channel::<ZoneEvent>(1024);

        for map in &templates.maps {
            let wp_w = map
                .waypoints
                .iter()
                .map(|w| w.3.saturating_add(48))
                .max()
                .unwrap_or(1_200);
            let map_width = assets.map_pixel_width(map.id.0).max(wp_w);
            for z in 0..u16::from(map.zones.max(1)) {
                let (zone_id, seed) = (
                    ZoneId(z),
                    (u64::from(map.id.0.cast_unsigned()) << 16) | u64::from(z),
                );
                let cfg = ZoneConfig {
                    id: (map.id, zone_id),
                    max_players: map.max_player.max(15),
                    map_width,
                    parity: config.parity_mode,
                };
                let (actor, tx) = ZoneActor::new(
                    cfg,
                    Arc::clone(world_router.map()),
                    Vec::new(),
                    GameRng::from_seed(seed),
                );
                let actor = actor.with_event_tx(zone_event_tx.clone());
                if !map.mobs.is_empty() {
                    let seeds: Vec<ZoneMobSeed> = map
                        .mobs
                        .iter()
                        .take(127)
                        .map(|&(tid, lvl, hp, mx, my)| {
                            let max_hp = if hp > 0 {
                                hp as u64
                            } else {
                                templates
                                    .mobs
                                    .iter()
                                    .find(|m| m.id == i32::from(tid.cast_unsigned()))
                                    .map_or(100, |m| m.hp.max(100) as u64)
                            };
                            ZoneMobSeed {
                                temp_id: i16::from(tid.cast_unsigned()),
                                level: lvl.max(1) as u8,
                                max_hp,
                                loc: Location {
                                    map_id: map.id,
                                    zone_id,
                                    x: mx,
                                    y: my,
                                },
                            }
                        })
                        .collect();
                    let _ = tx.try_send(ZoneCommand::SeedMobs(seeds));
                }
                world_router.register_zone(map.id, zone_id, tx);
                tracker.spawn(actor.run());
            }
        }

        let now_ms = unix_now_ms() as u64;
        let bosses = BossGroupManager::with_builtin_catalog(config.parity_mode, now_ms)
            .context("boss catalog init failed")?;
        let leaderboards = LeaderboardCache::default();
        leaderboards.mark_all_dirty();
        let consign_repo = ConsignShopRepo::new(pool.clone());
        let consign_shop = match consign_repo.load_all().await {
            Ok(records) => consign_shop_from_records(records),
            Err(err) => {
                warn!(error = %err, "failed loading consign_shop; starting empty");
                ConsignShop::default()
            }
        };

        let shared = Arc::new(SharedServerState {
            rate_limiter: IpRateLimiter::new(
                config.rate_limit.max_connections_per_ip,
                config.rate_limit.login_cooldown_ms,
            ),
            accounts_repo: AccountRepo::new(pool.clone()),
            players_repo: PlayerRepo::new(pool.clone()),
            clans_repo: ClanRepo::new(pool.clone()),
            consign_repo,
            super_rank_repo: SuperRankRepo::new(pool.clone()),
            clients: ClientRegistry::default(),
            leaderboards,
            maintenance: MaintenanceCoordinator::default(),
            world_router,
            bosses: Mutex::new(bosses),
            consign_shop: Mutex::new(consign_shop),
            active_trades: DashMap::new(),
            chon_ai_day: Mutex::new(ChonAiDayPool::default()),
            assets,
            templates,
            pool,
            config,
            started_at: Instant::now(),
        });

        let listener = TcpListener::bind(shared.config.bind_socket_addr()).await?;
        info!(
            realm = %shared.config.server_name,
            zones = shared.world_router.zone_count(),
            assets_dir = %shared.assets.root().display(),
            has_update_data = shared.assets.has_update_data(),
            res_x2_files = shared.assets.res_pack_count(2),
            "nro-server online (embedded fjall + DataGame asset streaming)"
        );
        if !shared.assets.has_update_data() {
            warn!(assets_dir = %shared.assets.root().display(), "MISSING binary client assets (`update_data/{{dart,arrow,effect,image,part,skill}}`)! Copy `SRC/data` into `teamobi2026/SRC/data` (or set `storage.assets_dir` in `server.toml`) or clients will hang on 'Xin cho' and render black backgrounds.");
        }

        spawn_zone_event_consumer(
            &tracker,
            Arc::clone(&shared),
            zone_event_rx,
            shutdown.clone(),
        );
        spawn_accept_loop(
            &tracker,
            session_tracker.clone(),
            listener,
            Arc::clone(&shared),
            shutdown.clone(),
        );
        runtime_ticks::spawn_auto_maintenance(&tracker, Arc::clone(&shared), shutdown.clone());
        runtime_ticks::spawn_background_ticks(&tracker, Arc::clone(&shared), shutdown.clone());
        runtime_ticks::spawn_admin_console(&tracker, Arc::clone(&shared), shutdown.clone());
        Ok(Self {
            shared,
            tracker,
            session_tracker,
            shutdown,
        })
    }

    /// Returns a cloned handle to the shared server state.
    #[must_use]
    pub fn shared(&self) -> Arc<SharedServerState> {
        Arc::clone(&self.shared)
    }

    /// Locks new logins, broadcasts a `ServerNotify`, and cancels the root shutdown token.
    pub async fn trigger_immediate_shutdown(&self, reason: &str) {
        let _ = self.shared.maintenance.try_begin_maintenance();
        self.shared
            .clients
            .broadcast_server_notify(&format!("Máy chủ tạm dừng ({reason}), đang lưu dữ liệu..."));
        self.shutdown.cancel();
    }

    /// Flushes all authenticated player snapshots to `fjall`, stops `ZoneActor`s, and `fsync`s the journal.
    pub async fn shutdown_and_flush(self) -> anyhow::Result<()> {
        self.shutdown.cancel();
        let disconnect_all = || {
            for h in self
                .shared
                .clients
                .sessions
                .iter()
                .map(|e| e.value().clone())
                .collect::<Vec<_>>()
            {
                h.disconnect();
            }
        };
        disconnect_all();
        for tx in self
            .shared
            .world_router
            .map()
            .iter()
            .map(|r| r.value().clone())
            .collect::<Vec<_>>()
        {
            let _ =
                tokio::time::timeout(Duration::from_secs(1), tx.send(ZoneCommand::Shutdown)).await;
        }
        self.tracker.close();
        let _ = tokio::time::timeout(Duration::from_secs(10), self.tracker.wait()).await;
        disconnect_all();
        self.session_tracker.close();
        let _ = tokio::time::timeout(Duration::from_secs(10), self.session_tracker.wait()).await;
        let now_ms = unix_now_ms();
        let sids: Vec<u64> = self
            .shared
            .clients
            .authenticated
            .iter()
            .map(|e| *e.key())
            .collect();
        for sid in &sids {
            let Some(snap) = self
                .shared
                .clients
                .authenticated
                .get(sid)
                .map(|e| e.snapshot.clone())
            else {
                continue;
            };
            if let Err(err) = self
                .shared
                .players_repo
                .save_player(&snap, None, self.shared.config.parity_mode, now_ms)
                .await
            {
                warn!(player_id = snap.id.0, error = %err, "failed saving player during shutdown");
            }
            if let Some(acc_id) = snap.account_id {
                let _ = self
                    .shared
                    .accounts_repo
                    .update_logout_time(acc_id, Utc::now())
                    .await;
            }
        }
        self.shared.persist_consign_shop().await;
        self.shared
            .pool
            .close()
            .await
            .context("failed syncing fjall journal on shutdown")?;
        Ok(())
    }
}
