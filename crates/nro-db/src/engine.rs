//! Safe embedded `fjall` 3.1 storage engine replacing `sqlx::MySqlPool`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64, Ordering};

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions, PersistMode};
use parking_lot::{Mutex, MutexGuard};
use tracing::{info, warn};

use crate::codec::{dec_i32, dec_i64, encode_val};
use crate::world_repos::WomensDayBuffs;
use crate::{CANONICAL_41_TABLES, DbError, DbResult};

const CAS_STRIPES: usize = 64;

/// Key in `templates` storing the compiled [`crate::templates::GameTemplates`] snapshot.
pub const TEMPLATES_SNAPSHOT_KEY: &[u8] = b"game_templates_v1";
/// Key in `meta` storing the [`crate::legacy_sql::ImportManifest`] record.
pub const IMPORT_MANIFEST_KEY: &[u8] = b"import_manifest_v1";
/// Key in `events` storing `'international_womens_day'` buff timers (`EventDAO.java`).
pub const WOMENS_DAY_EVENT_KEY: &[u8] = b"international_womens_day";

/// Legacy JSON literal preserved for `'international_womens_day'` wire parity checks.
pub const DEFAULT_WOMENS_DAY_JSON: &str =
    r#"{"damePrecent":0,"hpPrecent":0,"mpPrecent":0,"papPrecent":0}"#;

/// Tuning parameters for opening an embedded [`FjallEngine`] directory.
#[derive(Debug, Clone)]
pub struct FjallConfig {
    /// Local directory storing the `fjall` journal and LSM-tree keyspaces.
    pub path: PathBuf,
    /// Maximum per-keyspace write buffer (`Memtable`) size in bytes for hot keyspaces (default `4 MiB`).
    pub memtable_bytes: u64,
    /// Enable blob key-value separation on `players` for large multi-slot inventory payloads.
    pub kv_separate_players: bool,
    /// Fail boot with [`DbError::SchemaHealth`] if `templates` has not been imported yet.
    pub strict_schema_check: bool,
}

impl Default for FjallConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data/nro.fjall"),
            memtable_bytes: 4 * 1024 * 1024,
            kv_separate_players: true,
            strict_schema_check: true,
        }
    }
}

impl FjallConfig {
    /// Creates an embedded storage configuration rooted at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ..Self::default()
        }
    }
}

/// Summary produced by [`FjallEngine::check_schema_health`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaHealthReport {
    /// Number of canonical logical tables represented in the active store (`41` once imported).
    pub discovered_count: usize,
    /// Canonical table groups not yet imported into `templates`.
    pub missing_canonical: Vec<String>,
    /// Whether the supplemental `'international_womens_day'` event record exists in `events`.
    pub has_event_table: bool,
}

impl SchemaHealthReport {
    /// Returns `true` when static templates are populated and the `events` seed exists.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.missing_canonical.is_empty() && self.has_event_table
    }
}

/// Shared `fjall` handles and atomic high-water sequence counters.
pub struct FjallInner {
    pub db: Database,
    pub meta: Keyspace,
    pub templates: Keyspace,
    pub accounts: Keyspace,
    pub idx_account_user: Keyspace,
    pub players: Keyspace,
    pub idx_player_account: Keyspace,
    pub idx_player_name: Keyspace,
    pub idx_leaderboard: Keyspace,
    pub clans: Keyspace,
    pub consign_shop: Keyspace,
    pub super_rank: Keyspace,
    pub idx_super_rank: Keyspace,
    pub giftcodes: Keyspace,
    pub history_tx: Keyspace,
    pub events: Keyspace,
    pub portal_archive: Keyspace,
    next_account_id: AtomicI32,
    next_player_id: AtomicI64,
    next_clan_id: AtomicI32,
    next_tx_seq: AtomicU64,
    cas_stripes: [Mutex<()>; CAS_STRIPES],
}

/// Cheaply cloneable (`Arc`) handle to the embedded `fjall` storage engine.
#[derive(Clone)]
pub struct FjallEngine {
    inner: Arc<FjallInner>,
}

impl std::fmt::Debug for FjallEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallEngine")
            .field("keyspaces", &self.inner.db.keyspace_count())
            .finish()
    }
}

impl FjallEngine {
    /// Opens or creates the `fjall` database on a blocking worker thread and hydrates sequences.
    pub async fn open(cfg: &FjallConfig) -> DbResult<Self> {
        let owned = cfg.clone();
        tokio::task::spawn_blocking(move || Self::open_sync(&owned)).await?
    }

    /// Alias matching legacy `DbPool::connect` callers.
    pub async fn connect(cfg: &FjallConfig) -> DbResult<Self> {
        Self::open(cfg).await
    }

    /// Synchronous constructor used by CLI utilities (`nro-import-sql`) and tests.
    pub fn open_sync(cfg: &FjallConfig) -> DbResult<Self> {
        let db = Database::builder(&cfg.path).open()?;
        let hot_mem_bytes = cfg.memtable_bytes.max(1024 * 1024);
        let cold_mem_bytes = (hot_mem_bytes / 4).max(1024 * 1024);
        let hot_opts = || KeyspaceCreateOptions::default().max_memtable_size(hot_mem_bytes);
        let cold_opts = || KeyspaceCreateOptions::default().max_memtable_size(cold_mem_bytes);
        let player_opts = || {
            let base = KeyspaceCreateOptions::default().max_memtable_size(hot_mem_bytes);
            if cfg.kv_separate_players {
                base.with_kv_separation(Some(KvSeparationOptions::default()))
            } else {
                base
            }
        };

        let meta = db.keyspace("meta", cold_opts)?;
        let templates = db.keyspace("templates", cold_opts)?;
        let accounts = db.keyspace("accounts", cold_opts)?;
        let idx_account_user = db.keyspace("idx_account_user", cold_opts)?;
        let players = db.keyspace("players", player_opts)?;
        let idx_player_account = db.keyspace("idx_player_account", cold_opts)?;
        let idx_player_name = db.keyspace("idx_player_name", cold_opts)?;
        let idx_leaderboard = db.keyspace("idx_leaderboard", hot_opts)?;
        let clans = db.keyspace("clans", cold_opts)?;
        let consign_shop = db.keyspace("consign_shop", cold_opts)?;
        let super_rank = db.keyspace("super_rank", cold_opts)?;
        let idx_super_rank = db.keyspace("idx_super_rank", cold_opts)?;
        let giftcodes = db.keyspace("giftcodes", cold_opts)?;
        let history_tx = db.keyspace("history_tx", hot_opts)?;
        let events = db.keyspace("events", cold_opts)?;
        let portal_archive = db.keyspace("portal_archive", cold_opts)?;

        if !events.contains_key(WOMENS_DAY_EVENT_KEY)? {
            let default_buffs = WomensDayBuffs::default();
            events.insert(WOMENS_DAY_EVENT_KEY, encode_val("event", &default_buffs)?)?;
        }

        let max_acc = match accounts.last_key_value() {
            Some(g) => dec_i32(&g.key()?)?,
            None => 0,
        };
        let max_plr = match players.last_key_value() {
            Some(g) => dec_i64(&g.key()?)?,
            None => 0,
        };
        let max_clan = match clans.last_key_value() {
            Some(g) => dec_i32(&g.key()?)?,
            None => -1,
        };
        let max_tx_seq = match history_tx.last_key_value() {
            Some(g) => {
                let key = g.key()?;
                let key_bytes: &[u8] = &key;
                let seq_bytes: [u8; 8] =
                    key_bytes.get(8..16).and_then(|s| s.try_into().ok()).ok_or_else(|| {
                        DbError::InvalidColumnFormat {
                            column: "history_tx.key",
                            reason: format!("expected 16 bytes, got {}", key_bytes.len()),
                        }
                    })?;
                u64::from_be_bytes(seq_bytes)
            }
            None => 0,
        };

        let next_acc = max_acc.saturating_add(1).max(1);
        let next_plr = max_plr.saturating_add(1).max(1);
        let next_clan = max_clan.saturating_add(1).max(0);
        let next_tx = max_tx_seq.saturating_add(1).max(1);

        let this = Self {
            inner: Arc::new(FjallInner {
                db,
                meta,
                templates,
                accounts,
                idx_account_user,
                players,
                idx_player_account,
                idx_player_name,
                idx_leaderboard,
                clans,
                consign_shop,
                super_rank,
                idx_super_rank,
                giftcodes,
                history_tx,
                events,
                portal_archive,
                next_account_id: AtomicI32::new(next_acc),
                next_player_id: AtomicI64::new(next_plr),
                next_clan_id: AtomicI32::new(next_clan),
                next_tx_seq: AtomicU64::new(next_tx),
                cas_stripes: std::array::from_fn(|_| Mutex::new(())),
            }),
        };

        let report = this.check_schema_health_sync()?;
        if !report.missing_canonical.is_empty() {
            if cfg.strict_schema_check {
                return Err(DbError::SchemaHealth {
                    missing: report.missing_canonical,
                });
            }
            warn!(
                path = %Path::display(&cfg.path),
                "opened unseeded fjall store (`strict_schema_check = false`)"
            );
        } else {
            info!(
                path = %Path::display(&cfg.path),
                next_account_id = next_acc,
                next_player_id = next_plr,
                next_clan_id = next_clan,
                next_tx_seq = next_tx,
                "opened embedded fjall storage engine"
            );
        }
        Ok(this)
    }

    #[inline]
    #[must_use]
    pub fn ks(&self) -> &FjallInner {
        &self.inner
    }

    /// Acquires a synchronous striped lock for read-modify-write CAS operations.
    /// Never hold the guard across an `.await` point (`clippy::await_holding_lock = "deny"`).
    #[inline]
    pub fn lock_stripe(&self, seed: u64) -> MutexGuard<'_, ()> {
        self.inner.cas_stripes[(seed as usize) & (CAS_STRIPES - 1)].lock()
    }

    #[inline]
    pub fn alloc_account_id(&self) -> i32 {
        self.inner.next_account_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Allocates the next positive 32-bit wire-safe player ID (`1..=i32::MAX`).
    pub fn alloc_player_id(&self) -> DbResult<i64> {
        let max_wire_id = i64::from(i32::MAX);
        self.inner
            .next_player_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                (1..=max_wire_id).contains(&cur).then_some(cur.saturating_add(1))
            })
            .map_err(|cur| DbError::InvalidColumnFormat {
                column: "player.id",
                reason: format!("allocated player id {cur} outside wire range 1..={max_wire_id}"),
            })
    }

    #[inline]
    pub fn alloc_clan_id(&self) -> i32 {
        self.inner.next_clan_id.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub fn alloc_tx_seq(&self) -> u64 {
        self.inner.next_tx_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Advances high-water ID sequences after importing existing SQL rows.
    pub fn bump_sequences(&self, next_acc: i32, next_plr: i64, next_clan: i32) {
        self.inner.next_account_id.fetch_max(next_acc.max(1), Ordering::SeqCst);
        self.inner.next_player_id.fetch_max(next_plr.max(1), Ordering::SeqCst);
        self.inner.next_clan_id.fetch_max(next_clan.max(0), Ordering::SeqCst);
    }

    /// Seeds the `'international_womens_day'` record in `events` if absent.
    pub async fn ensure_event_table(&self) -> DbResult<()> {
        if !self.inner.events.contains_key(WOMENS_DAY_EVENT_KEY)? {
            let default_buffs = WomensDayBuffs::default();
            self.inner
                .events
                .insert(WOMENS_DAY_EVENT_KEY, encode_val("event", &default_buffs)?)?;
        }
        Ok(())
    }

    /// Synchronous schema health check inspecting `templates` and `events`.
    pub fn check_schema_health_sync(&self) -> DbResult<SchemaHealthReport> {
        let has_templates = self.inner.templates.contains_key(TEMPLATES_SNAPSHOT_KEY)?;
        let has_event_table = self.inner.events.contains_key(WOMENS_DAY_EVENT_KEY)?;
        let (discovered_count, missing_canonical) = if has_templates {
            (CANONICAL_41_TABLES.len(), Vec::new())
        } else {
            (0, CANONICAL_41_TABLES.iter().map(|&t| t.to_owned()).collect())
        };
        Ok(SchemaHealthReport {
            discovered_count,
            missing_canonical,
            has_event_table,
        })
    }

    /// Async wrapper around [`Self::check_schema_health_sync`].
    pub async fn check_schema_health(&self) -> DbResult<SchemaHealthReport> {
        self.check_schema_health_sync()
    }

    /// Synchronously flushes and `fsync`s the active `fjall` journal (`PersistMode::SyncAll`).
    pub fn persist_sync(&self) -> DbResult<()> {
        self.inner.db.persist(PersistMode::SyncAll).map_err(DbError::from)
    }

    /// Flushes and `fsync`s the active `fjall` journal on a blocking worker thread.
    pub async fn close(&self) -> DbResult<()> {
        let db = self.inner.db.clone();
        tokio::task::spawn_blocking(move || db.persist(PersistMode::SyncAll).map_err(DbError::from))
            .await?
    }
}
