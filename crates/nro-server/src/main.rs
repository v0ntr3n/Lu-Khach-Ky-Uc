#![forbid(unsafe_code)]
//! Top-level binary entrypoint for the Ngọc Rồng Online (Teamobi2026) Safe Rust server.
//!
//! Replaces legacy Java singletons (`ServerManager`, `Client`, `Maintenance`, `AutoMaintenance`,
//! `ServerNotify`, `Manager`, and `DataGame`) with structured Tokio supervision backed by
//! embedded `fjall` LSM-tree storage (`nro-db`) and direct `SRC/data` binary asset streaming.

use nro_server::{config, pgo_bench, runtime};

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use nro_db::engine::FjallEngine;
use nro_db::legacy_sql::{ImportOptions, import_mysqldump_file, verify_fjall_store};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use config::ServerConfig;
use runtime::ServerRuntime;

/// Command-line flags for `nro-server`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "nro-server",
    version,
    about = "Ngọc Rồng Online (Teamobi2026) Safe Rust Authoritative Server (Embedded fjall + DataGame)"
)]
pub struct CliArgs {
    /// Path to `server.toml` configuration file.
    #[arg(short, long, value_name = "PATH", default_value = "server.toml")]
    pub config: PathBuf,

    /// Optional TCP bind override (default `0.0.0.0:14445`).
    #[arg(short, long, value_name = "ADDR")]
    pub bind: Option<SocketAddr>,

    /// Optional embedded `fjall` storage directory override (default `data/nro.fjall`).
    #[arg(long, value_name = "DIR")]
    pub storage_path: Option<PathBuf>,

    /// Optional `SRC/data` binary client asset directory override (default `SRC/data`).
    #[arg(long, value_name = "ASSETS_DIR")]
    pub assets_dir: Option<PathBuf>,

    /// Import a MariaDB/phpMyAdmin `.sql` dump (`database team2026.sql`) into `storage.path` before booting.
    #[arg(long, value_name = "SQL_FILE")]
    pub import_sql: Option<PathBuf>,

    /// Exit immediately after completing `--import-sql` and verifying store health.
    #[arg(long, default_value_t = false)]
    pub import_only: bool,

    /// Execute the deterministic 4-stage headless LLVM PGO replay workload and exit.
    #[arg(long, default_value_t = false)]
    pub benchmark_replay: bool,

    /// Iteration scale factor for `--benchmark-replay` stages.
    #[arg(long, value_name = "SCALE", default_value_t = 1_000)]
    pub replay_scale: u32,

    /// Default `tracing` filter directive when `RUST_LOG` is unset.
    #[arg(long, value_name = "DIRECTIVE", default_value = "info,nro_server=debug")]
    pub log_filter: String,
}

fn init_tracing(default_directive: &str) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_directive))
        .context("invalid tracing filter directive")?;

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .try_init()
        .map_err(|err| anyhow::anyhow!("failed to initialize tracing subscriber: {err}"))?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = CliArgs::parse();
    init_tracing(&cli.log_filter)?;

    let config = ServerConfig::load_with_overrides(
        &cli.config,
        cli.bind,
        cli.storage_path.as_deref(),
        cli.assets_dir.as_deref(),
        cli.benchmark_replay || cli.import_only,
    )
    .with_context(|| format!("failed to load configuration from {}", cli.config.display()))?;

    if cli.benchmark_replay {
        info!(
            scale = cli.replay_scale,
            parity = ?config.parity_mode,
            "starting headless 4-stage LLVM PGO benchmark replay"
        );
        let summary = pgo_bench::run_four_stage_replay(&config, cli.replay_scale)?;
        info!(
            combined_digest = format_args!("{:#018x}", summary.combined_digest),
            total_ops = summary.total_operations(),
            elapsed_ms = summary.total_elapsed().as_secs_f64() * 1_000.0,
            "completed 4-stage LLVM PGO benchmark replay cleanly"
        );
        return Ok(());
    }

    if let Some(sql_file) = &cli.import_sql {
        let mut init_cfg = config.storage.to_fjall_config();
        init_cfg.strict_schema_check = false;
        let engine = FjallEngine::open(&init_cfg).await.context("failed opening fjall for SQL import")?;
        let sql_path = sql_file.clone();
        let parity = config.parity_mode;
        let eng_clone = engine.clone();
        let manifest = tokio::task::spawn_blocking(move || {
            import_mysqldump_file(&eng_clone, &sql_path, ImportOptions { parity_mode: parity, upgrade_passwords: false })
        })
        .await??;
        let verified = verify_fjall_store(&engine).await?;
        info!(
            rows = manifest.total_rows,
            items = verified.item_templates,
            maps = verified.map_templates,
            super_rank_max = verified.super_rank_highest,
            "imported legacy SQL dump into embedded fjall"
        );
        engine.close().await?;
        if cli.import_only {
            return Ok(());
        }
    }

    let shutdown_token = CancellationToken::new();
    let runtime = ServerRuntime::boot(config, shutdown_token.clone()).await?;

    tokio::select! {
        ctrl_c_res = tokio::signal::ctrl_c() => {
            match ctrl_c_res {
                Ok(()) => info!("received Ctrl-C; initiating graceful player state flush"),
                Err(err) => warn!(error = %err, "Ctrl-C listener closed; shutting down"),
            }
            runtime.trigger_immediate_shutdown("SIGINT / Ctrl-C").await;
        }
        () = shutdown_token.cancelled() => {
            info!("maintenance coordinator triggered shutdown; flushing online state");
        }
    }

    runtime.shutdown_and_flush().await?;
    info!("nro-server exited cleanly with all online player snapshots persisted to fjall");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_defaults_assets_import_and_pgo_flags() -> anyhow::Result<()> {
        let imp = CliArgs::try_parse_from([
            "nro-server",
            "--storage-path", "tmp/nro.fjall",
            "--assets-dir", "SRC/data",
            "--import-sql", "database team2026.sql",
            "--import-only",
        ])?;
        assert_eq!(imp.assets_dir, Some(PathBuf::from("SRC/data")));
        assert!(imp.import_only);
        Ok(())
    }

}
