#![forbid(unsafe_code)]
//! Standalone CLI importer converting `database team2026.sql` into an embedded `fjall` directory.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use nro_core::ParityMode;
use nro_db::engine::{FjallConfig, FjallEngine};
use nro_db::legacy_sql::{ImportOptions, import_mysqldump_file, verify_fjall_store};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliParity {
    Fixed,
    Legacy,
}

impl From<CliParity> for ParityMode {
    fn from(val: CliParity) -> Self {
        match val {
            CliParity::Fixed => Self::Fixed,
            CliParity::Legacy => Self::Legacy,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "nro-import-sql",
    version,
    about = "Imports `database team2026.sql` into an embedded Teamobi2026 `fjall` keyspace directory"
)]
struct Cli {
    /// Path to `database team2026.sql` or `SRC/sql/nro1.sql`.
    #[arg(long, value_name = "SQL_FILE")]
    sql: PathBuf,

    /// Output `fjall` directory path.
    #[arg(long, value_name = "FJALL_DIR", default_value = "data/nro.fjall")]
    out: PathBuf,

    /// Parity mode applied while decoding legacy `player` JSON arrays.
    #[arg(long, value_enum, default_value_t = CliParity::Fixed)]
    parity_mode: CliParity,

    /// Eagerly upgrade plaintext `account.password` values to Argon2id PHC during import.
    #[arg(long, default_value_t = false)]
    upgrade_passwords: bool,

    /// Verify `GameTemplates`, keyspaces, and `SuperRank` index ordering after import.
    #[arg(long, default_value_t = true)]
    verify: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();

    let cli = Cli::parse();
    let started = Instant::now();

    let mut cfg = FjallConfig::new(&cli.out);
    cfg.strict_schema_check = false;
    let engine = FjallEngine::open(&cfg).await?;

    let manifest = import_mysqldump_file(
        &engine,
        &cli.sql,
        ImportOptions {
            parity_mode: cli.parity_mode.into(),
            upgrade_passwords: cli.upgrade_passwords,
        },
    )?;

    info!(
        elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
        total_rows = manifest.total_rows,
        populated_tables = manifest.table_counts.len(),
        out = %cli.out.display(),
        "imported SQL dump cleanly"
    );

    if cli.verify {
        let report = verify_fjall_store(&engine).await?;
        info!(
            items = report.item_templates,
            maps = report.map_templates,
            shops = report.shop_templates,
            accounts = report.accounts_count,
            players = report.players_count,
            super_rank_highest = report.super_rank_highest,
            "verified embedded fjall store integrity"
        );
    }

    engine.close().await?;
    Ok(())
}
