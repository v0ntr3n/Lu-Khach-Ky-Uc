//! Layered TOML and environment configuration loader replacing `Manager.java` properties.

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};
use nro_consts::{DEFAULT_SERVER_PORT, const_event};
use nro_core::ParityMode;
use nro_db::engine::FjallConfig;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Top-level configuration for `nro-server`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Realm display name shown in login banners and `ServerNotify`.
    pub server_name: String,
    /// Public realm hostname or IP advertised in `-29` `sendLinkIP` packets.
    pub domain: String,
    /// Local IP interface to bind the TCP listener onto.
    pub bind_ip: IpAddr,
    /// Local TCP port to listen on (TeaMobi default: `14445`).
    pub port: u16,
    /// Active seasonal event ID (`const_event::KHONG_CO_SU_KIEN = 0`, `1..=6` seasonal).
    pub active_seasonal_event_id: u8,
    /// Arithmetic/wire execution mode (`Fixed` vs `Legacy`).
    pub parity_mode: ParityMode,
    /// Per-IP connection and login rate-limiting thresholds.
    pub rate_limit: RateLimitConfig,
    /// Daily scheduled auto-maintenance configuration.
    pub auto_maintenance: AutoMaintenanceConfig,
    /// Server-wide experience and item drop rate multipliers.
    pub rates: RateMultipliers,
    /// Embedded `fjall` LSM storage and `SRC/data` binary asset parameters.
    #[serde(alias = "database")]
    pub storage: StorageConfig,
}

/// Per-IP connection limits and authentication cooldown settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    pub login_wait_secs: u32,
    pub login_cooldown_ms: u64,
    pub max_connections_per_ip: usize,
    pub max_online_players: usize,
}

/// Daily scheduled maintenance window replacing `AutoMaintenance.java`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoMaintenanceConfig {
    pub enabled: bool,
    pub hour: u8,
    pub minute: u8,
    pub countdown_secs: u32,
}

/// Global gameplay progression multipliers replacing `Manager.RATE_EXP_SERVER`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct RateMultipliers {
    pub exp_multiplier: u8,
    pub drop_multiplier: u8,
}

/// Embedded `fjall` storage directory, `SRC/data` client asset path, and LSM tuning parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Local directory path storing `fjall` journals and keyspace SSTs.
    pub path: PathBuf,
    /// Directory containing binary client assets (`update_data`, `icon`, `effect`, `effdata`, `map`, `res`).
    pub assets_dir: PathBuf,
    /// Per-keyspace write buffer (`Memtable`) cap in MiB (`>= 1`).
    pub memtable_mb: u64,
    /// Store large `players` values in `fjall` blob files via key-value separation.
    pub kv_separate_players: bool,
    /// Fail boot if `templates` has not been populated yet (unless `bootstrap_sql_path` exists).
    pub strict_schema_check: bool,
    /// Optional SQL dump path auto-imported on first boot when `templates` is empty.
    pub bootstrap_sql_path: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        let _ = const_event::SU_KIEN_TET;
        Self {
            server_name: String::from("Ngọc Rồng Teamobi2026"),
            domain: String::from("127.0.0.1"),
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_SERVER_PORT,
            active_seasonal_event_id: 0,
            parity_mode: ParityMode::Fixed,
            rate_limit: RateLimitConfig::default(),
            auto_maintenance: AutoMaintenanceConfig::default(),
            rates: RateMultipliers::default(),
            storage: StorageConfig::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            login_wait_secs: 0,
            login_cooldown_ms: 2_000,
            max_connections_per_ip: 12,
            max_online_players: 2_000,
        }
    }
}

impl Default for AutoMaintenanceConfig {
    fn default() -> Self {
        Self { enabled: true, hour: 5, minute: 0, countdown_secs: 60 }
    }
}

impl Default for RateMultipliers {
    fn default() -> Self {
        Self { exp_multiplier: 2, drop_multiplier: 1 }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data/nro.fjall"),
            assets_dir: PathBuf::from("SRC/data"),
            memtable_mb: 16,
            kv_separate_players: true,
            strict_schema_check: true,
            bootstrap_sql_path: Some(PathBuf::from("database team2026.sql")),
        }
    }
}

impl StorageConfig {
    /// Converts this configuration section into [`FjallConfig`].
    #[must_use]
    pub fn to_fjall_config(&self) -> FjallConfig {
        FjallConfig {
            path: self.path.clone(),
            memtable_bytes: self.memtable_mb.saturating_mul(1024 * 1024),
            kv_separate_players: self.kv_separate_players,
            strict_schema_check: self.strict_schema_check,
        }
    }
}

impl ServerConfig {
    /// Returns the combined TCP [`SocketAddr`] (`bind_ip:port`).
    #[must_use]
    pub fn bind_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_ip, self.port)
    }

    /// Hostname or IP advertised in `-29` `sendLinkIP` packets (`127.0.0.1` when unspecified).
    #[must_use]
    pub fn advertised_host(&self) -> String {
        if !self.domain.trim().is_empty() && self.domain != "nro.teamobi2026.vn" {
            return self.domain.trim().to_owned();
        }
        if self.bind_ip.is_unspecified() {
            String::from("127.0.0.1")
        } else {
            self.bind_ip.to_string()
        }
    }

    /// Parses a TOML string into [`ServerConfig`].
    pub fn from_toml_str(raw: &str) -> anyhow::Result<Self> {
        toml::from_str(raw).context("failed to deserialize ServerConfig TOML")
    }

    /// Loads `path` (falling back to defaults when `allow_missing` is `true`), layers env + CLI, and validates.
    pub fn load_with_overrides(
        path: &Path,
        bind_override: Option<SocketAddr>,
        storage_path_override: Option<&Path>,
        assets_dir_override: Option<&Path>,
        allow_missing: bool,
    ) -> anyhow::Result<Self> {
        let mut cfg = match fs::read_to_string(path) {
            Ok(raw) => {
                info!(path = %path.display(), "loaded server configuration file");
                Self::from_toml_str(&raw)?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound && allow_missing => Self::default(),
            Err(err) => return Err(err).with_context(|| format!("unable to read {}", path.display())),
        };
        cfg.apply_env_with(|key| std::env::var(key).ok())?;
        if let Some(addr) = bind_override {
            cfg.bind_ip = addr.ip();
            cfg.port = addr.port();
        }
        if let Some(storage_dir) = storage_path_override {
            cfg.storage.path = storage_dir.to_path_buf();
        }
        if let Some(assets_dir) = assets_dir_override {
            cfg.storage.assets_dir = assets_dir.to_path_buf();
        }
        cfg.validate()?;
        Ok(cfg)
    }

    /// Applies `NRO_*` environment variable overrides.
    pub fn apply_env_with<F: Fn(&str) -> Option<String>>(&mut self, lookup: F) -> anyhow::Result<()> {
        if let Some(v) = lookup("NRO_SERVER_NAME") { self.server_name = v; }
        if let Some(v) = lookup("NRO_DOMAIN") { self.domain = v; }
        if let Some(v) = lookup("NRO_BIND_IP") { self.bind_ip = v.parse().context("invalid NRO_BIND_IP")?; }
        if let Some(v) = lookup("NRO_PORT") { self.port = v.parse().context("invalid NRO_PORT")?; }
        if let Some(v) = lookup("NRO_EVENT_ID") { self.active_seasonal_event_id = v.parse().context("invalid NRO_EVENT_ID")?; }
        if let Some(v) = lookup("NRO_EXP_MULTIPLIER") { self.rates.exp_multiplier = v.parse().context("invalid NRO_EXP_MULTIPLIER")?; }
        if let Some(v) = lookup("NRO_DROP_MULTIPLIER") { self.rates.drop_multiplier = v.parse().context("invalid NRO_DROP_MULTIPLIER")?; }
        if let Some(v) = lookup("NRO_PARITY_MODE") { self.parity_mode = parse_parity_mode(&v)?; }
        if let Some(v) = lookup("NRO_STORAGE_PATH") { self.storage.path = PathBuf::from(v); }
        if let Some(v) = lookup("NRO_ASSETS_DIR") { self.storage.assets_dir = PathBuf::from(v); }
        if let Some(v) = lookup("NRO_BOOTSTRAP_SQL") {
            self.storage.bootstrap_sql_path = (!v.trim().is_empty()).then(|| PathBuf::from(v));
        }
        Ok(())
    }

    /// Validates range and consistency invariants across all configuration sections.
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.server_name.trim().is_empty(), "server_name cannot be empty");
        ensure!(self.port > 0, "port must be non-zero");
        ensure!(self.auto_maintenance.hour <= 23, "auto_maintenance.hour must be 0..=23");
        ensure!(self.auto_maintenance.minute <= 59, "auto_maintenance.minute must be 0..=59");
        ensure!(self.rates.exp_multiplier >= 1, "rates.exp_multiplier must be >= 1");
        ensure!(self.rates.drop_multiplier >= 1, "rates.drop_multiplier must be >= 1");
        ensure!(!self.storage.path.as_os_str().is_empty(), "storage.path cannot be empty");
        ensure!(!self.storage.assets_dir.as_os_str().is_empty(), "storage.assets_dir cannot be empty");
        ensure!(self.storage.memtable_mb >= 1, "storage.memtable_mb must be >= 1");
        Ok(())
    }
}

fn parse_parity_mode(raw: &str) -> anyhow::Result<ParityMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "fixed" | "hardened" | "safe_rust" => Ok(ParityMode::Fixed),
        "legacy" | "java" | "java_parity" => Ok(ParityMode::Legacy),
        other => bail!("unsupported NRO_PARITY_MODE '{other}'"),
    }
}
