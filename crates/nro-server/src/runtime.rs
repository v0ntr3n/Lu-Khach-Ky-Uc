//! Authoritative server runtime replacing `ServerManager.java` and `Manager.java`.

#[path = "runtime_ticks.rs"]
mod runtime_ticks;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use chrono::Utc;
use dashmap::DashMap;
use nro_boss::spawner::BossGroupManager;
use nro_core::{GameRng, Location, PlayerId, ZoneId};
use nro_db::account_repo::AccountRepo;
use nro_db::legacy_sql::{ImportOptions, import_mysqldump_file};
use nro_db::player_repo::PlayerRepo;
use nro_db::pool::DbPool;
use nro_db::templates::GameTemplates;
use nro_db::world_repos::{ClanRepo, ConsignShopRepo, SuperRankRepo};
use nro_economy::{ConsignShop, TradeSession};
use nro_protocol::{SessionConfig, spawn_tcp_session};
use nro_world::{ChonAiDayPool, WorldRouter, ZoneActor, ZoneCommand, ZoneConfig, ZoneMobSeed};
use parking_lot::Mutex;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::config::ServerConfig;
use crate::data_game::DataGameAssets;
use crate::router::ServerSessionHandler;
pub use crate::state::{
    ActivePlayerEntry, ClientRegistry, IpRateLimiter, LeaderboardCache, LeaderboardRow,
    LeaderboardSlot, MaintenanceCoordinator,
};

/// Shared server resources passed to session routers and background supervisors.
pub struct SharedServerState {
    pub config: ServerConfig,
    pub pool: DbPool,
    pub assets: DataGameAssets,
    pub accounts_repo: AccountRepo,
    pub players_repo: PlayerRepo,
    pub clans_repo: ClanRepo,
    #[allow(dead_code)]
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

/// Running `nro-server` supervisor owning `TaskTracker` and `CancellationToken`.
pub struct ServerRuntime {
    shared: Arc<SharedServerState>,
    tracker: TaskTracker,
    shutdown: CancellationToken,
}

impl ServerRuntime {
    /// Opens embedded `fjall`, loads `SRC/data` binary assets, spawns `ZoneActor`s, and binds TCP `14445`.
    pub async fn boot(config: ServerConfig, shutdown: CancellationToken) -> anyhow::Result<Self> {
        let pool = open_or_bootstrap_store(&config).await?;
        pool.ensure_event_table().await.context("failed to seed `events` keyspace")?;
        let templates = Arc::new(GameTemplates::load_all(&pool).await.context("failed to load GameTemplates")?);
        let assets = DataGameAssets::open(&config.storage.assets_dir);
        let tracker = TaskTracker::new();
        let world_router = WorldRouter::new();

        for map in &templates.maps {
            let wp_w = map.waypoints.iter().map(|w| w.3.saturating_add(48)).max().unwrap_or(1_200);
            let map_width = assets.map_pixel_width(map.id.0).max(wp_w);
            for z in 0..u16::from(map.zones.max(1)) {
                let (zone_id, seed) = (ZoneId(z), (u64::from(map.id.0.cast_unsigned()) << 16) | u64::from(z));
                let cfg = ZoneConfig { id: (map.id, zone_id), max_players: map.max_player.max(15), map_width, parity: config.parity_mode };
                let (actor, tx) = ZoneActor::new(cfg, Arc::clone(world_router.map()), Vec::new(), GameRng::from_seed(seed));
                if !map.mobs.is_empty() {
                    let seeds: Vec<ZoneMobSeed> = map
                        .mobs
                        .iter()
                        .take(127)
                        .map(|&(tid, lvl, hp, mx, my)| {
                            let temp_id = i16::from(tid.cast_unsigned());
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
                                temp_id,
                                level: lvl.max(1) as u8,
                                max_hp,
                                loc: Location { map_id: map.id, zone_id, x: mx, y: my },
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
        let bosses = BossGroupManager::with_builtin_catalog(config.parity_mode, now_ms).context("boss catalog init failed")?;
        let leaderboards = LeaderboardCache::default();
        leaderboards.mark_all_dirty();

        let shared = Arc::new(SharedServerState {
            rate_limiter: IpRateLimiter::new(config.rate_limit.max_connections_per_ip, config.rate_limit.login_cooldown_ms),
            accounts_repo: AccountRepo::new(pool.clone()),
            players_repo: PlayerRepo::new(pool.clone()),
            clans_repo: ClanRepo::new(pool.clone()),
            consign_repo: ConsignShopRepo::new(pool.clone()),
            super_rank_repo: SuperRankRepo::new(pool.clone()),
            clients: ClientRegistry::default(),
            leaderboards,
            maintenance: MaintenanceCoordinator::default(),
            world_router,
            bosses: Mutex::new(bosses),
            consign_shop: Mutex::new(ConsignShop::default()),
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
            warn!(
                assets_dir = %shared.assets.root().display(),
                "MISSING binary client assets (`update_data/{{dart,arrow,effect,image,part,skill}}`)! Copy `SRC/data` into `teamobi2026/SRC/data` (or set `storage.assets_dir` in `server.toml`) or clients will hang on 'Xin cho' and render black backgrounds."
            );
        }

        spawn_accept_loop(&tracker, listener, Arc::clone(&shared), shutdown.clone());
        runtime_ticks::spawn_auto_maintenance(&tracker, Arc::clone(&shared), shutdown.clone());
        runtime_ticks::spawn_background_ticks(&tracker, Arc::clone(&shared), shutdown.clone());
        runtime_ticks::spawn_admin_console(&tracker, Arc::clone(&shared), shutdown.clone());
        Ok(Self { shared, tracker, shutdown })
    }

    /// Locks new logins, broadcasts a `ServerNotify`, and cancels the root shutdown token.
    pub async fn trigger_immediate_shutdown(&self, reason: &str) {
        let _ = self.shared.maintenance.try_begin_maintenance();
        self.shared.clients.broadcast_server_notify(&format!("Máy chủ tạm dừng ({reason}), đang lưu dữ liệu..."));
        self.shutdown.cancel();
    }

    /// Flushes all authenticated player snapshots to `fjall`, stops `ZoneActor`s, and `fsync`s the journal.
    pub async fn shutdown_and_flush(self) -> anyhow::Result<()> {
        self.shutdown.cancel();
        let now_ms = unix_now_ms();
        let sids: Vec<u64> = self.shared.clients.authenticated.iter().map(|e| *e.key()).collect();
        for sid in &sids {
            let Some(snap) = self.shared.clients.authenticated.get(sid).map(|e| e.snapshot.clone()) else {
                continue;
            };
            if let Err(err) = self.shared.players_repo.save_player(&snap, None, self.shared.config.parity_mode, now_ms).await {
                warn!(player_id = snap.id.0, error = %err, "failed saving player during shutdown");
            }
            if let Some(acc_id) = snap.account_id {
                let _ = self.shared.accounts_repo.update_logout_time(acc_id, Utc::now()).await;
            }
        }
        for zone_ref in self.shared.world_router.map().iter() {
            let _ = zone_ref.value().try_send(ZoneCommand::Shutdown);
        }
        self.tracker.close();
        let _ = tokio::time::timeout(Duration::from_secs(10), self.tracker.wait()).await;
        self.shared.pool.close().await.context("failed syncing fjall journal on shutdown")?;
        Ok(())
    }
}

async fn open_or_bootstrap_store(config: &ServerConfig) -> anyhow::Result<DbPool> {
    open_or_bootstrap_store_in(config, Path::new(".")).await
}

pub(crate) async fn open_or_bootstrap_store_in(
    config: &ServerConfig,
    base_dir: &Path,
) -> anyhow::Result<DbPool> {
    let mut fjall_cfg = config.storage.to_fjall_config();
    let strict = fjall_cfg.strict_schema_check;
    fjall_cfg.strict_schema_check = false;
    let pool = DbPool::open(&fjall_cfg).await.context("embedded fjall init failed")?;
    if pool.check_schema_health().await?.is_healthy() {
        return Ok(pool);
    }

    if let Some(sql_path) = discover_bootstrap_sql_in(base_dir, config.storage.bootstrap_sql_path.as_deref()) {
        info!(sql = %sql_path.display(), "unseeded fjall store detected; auto-importing SQL dump");
        let p = pool.clone();
        let path = sql_path.clone();
        let parity = config.parity_mode;
        tokio::task::spawn_blocking(move || {
            import_mysqldump_file(&p, &path, ImportOptions { parity_mode: parity, upgrade_passwords: false })
        })
        .await??;
        let post = pool.check_schema_health().await?;
        if strict && !post.is_healthy() {
            anyhow::bail!(
                "fjall store remained incomplete after importing '{}' (missing: {:?})",
                sql_path.display(),
                post.missing_canonical
            );
        }
        return Ok(pool);
    }

    if !strict {
        return Ok(pool);
    }

    let _ = pool.close().await;
    drop(pool);
    let resolved_base = fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
    let expected = config
        .storage
        .bootstrap_sql_path
        .as_deref()
        .unwrap_or_else(|| Path::new("database team2026.sql"));
    let target = if expected.is_absolute() { expected.to_path_buf() } else { resolved_base.join(expected) };
    anyhow::bail!(
        "embedded fjall store at '{}' is empty and no SQL seed dump was found.\n  \
         -> Place 'database team2026.sql' at '{}'\n  \
         -> (also searched nested folders such as 'teamobi2026_databases/' and 'Copy of Teamobi2026/'),\n  \
         -> or run: nro-server --import-sql \"path/to/database team2026.sql\"",
        config.storage.path.display(),
        target.display(),
    )
}

pub(crate) fn discover_bootstrap_sql_in(base: &Path, configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(cfg) = configured {
        let direct = if cfg.is_absolute() { cfg.to_path_buf() } else { base.join(cfg) };
        if direct.is_file() {
            return Some(direct);
        }
    }
    for rel in [
        "database team2026.sql",
        "teamobi2026_databases/database team2026.sql",
        "Copy of Teamobi2026/database team2026.sql",
        "Teamobi2026/database team2026.sql",
        "SRC/sql/nro1.sql",
        "teamobi2026_databases/SRC/sql/nro1.sql",
        "Copy of Teamobi2026/SRC/sql/nro1.sql",
        "Teamobi2026/SRC/sql/nro1.sql",
    ] {
        let candidate = base.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let Ok(entries) = fs::read_dir(base) else {
        return None;
    };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(fs::DirEntry::path)
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    for dir in subdirs {
        for sub in ["database team2026.sql", "SRC/sql/nro1.sql"] {
            let candidate = dir.join(sub);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn spawn_accept_loop(tracker: &TaskTracker, listener: TcpListener, shared: Arc<SharedServerState>, shutdown: CancellationToken) {
    let next_sid = Arc::new(AtomicU64::new(1));
    let handler = Arc::new(ServerSessionHandler::new(Arc::clone(&shared)));
    tracker.spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                accepted = listener.accept() => {
                    let Ok((stream, peer_addr)) = accepted else { continue };
                    if !shared.rate_limiter.try_acquire(peer_addr.ip()) {
                        debug!(ip = %peer_addr.ip(), "per-IP connection limit exceeded");
                        continue;
                    }
                    let sid = next_sid.fetch_add(1, Ordering::Relaxed);
                    match spawn_tcp_session(sid, stream, peer_addr, SessionConfig::default(), Arc::clone(&handler)) {
                        Ok((handle, _join)) => {
                            shared.clients.sessions.insert(sid, handle.clone());
                            if !handle.is_connected() {
                                shared.clients.sessions.remove(&sid);
                            }
                        }
                        Err(err) => {
                            shared.rate_limiter.release(peer_addr.ip());
                            warn!(error = %err, "failed spawning TCP session");
                        }
                    }
                }
            }
        }
    });
}

/// Current Unix epoch timestamp in milliseconds.
#[must_use]
pub fn unix_now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}
