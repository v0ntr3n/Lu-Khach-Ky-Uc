#![forbid(unsafe_code)]
//! Top-level binary entrypoint for the Ngọc Rồng Online (Teamobi2026) Safe Rust server.
//!
//! Replaces legacy Java singletons (`ServerManager`, `Client`, `Maintenance`, `AutoMaintenance`,
//! `ServerNotify`, `Manager`, and `DataGame`) with structured Tokio supervision backed by
//! embedded `fjall` LSM-tree storage (`nro-db`) and direct `SRC/data` binary asset streaming.

use nro_server::{config, data_game, pgo_bench, runtime};

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
    use std::fs;
    use super::*;
    use super::data_game::DataGameAssets;
    use nro_core::{AccountId, Gender, MapId, PlayerId};
    use nro_db::player_repo::{NewPlayerSeed, PlayerSnapshot};
    use nro_db::templates::{GameTemplates, ItemTemplateRecord, MapTemplateRecord, SkillTemplateRecord};
    use nro_protocol::is_extended_s2c_cmd;

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

    #[test]
    fn data_game_streams_all_asset_and_template_opcodes_cleanly() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("nro_assets_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        for sub in ["update_data", "res/x2", "icon/x2", "effdata", "effect/x2", "img_by_name/x2", "item_bg_temp/x2", "mob/x2", "map/tile_map_data", "map/item_bg_map_data", "map/eff_map"] {
            fs::create_dir_all(tmp.join(sub))?;
        }
        for part in ["dart", "arrow", "effect", "image", "part", "skill"] {
            fs::write(tmp.join("update_data").join(part), b"\x01\x02\x03")?;
        }
        fs::write(tmp.join("res/x2/0"), b"RES0")?;
        fs::write(tmp.join("icon/x2/4028.png"), b"\x89PNG_ICON")?;
        fs::write(tmp.join("effdata/DataEffect_60"), b"EFF60")?;
        fs::write(tmp.join("effect/x2/ImgEffect_60.png"), b"PNG60")?;
        fs::write(tmp.join("img_by_name/x2/mount_12_0.png"), b"MOUNT")?;
        fs::write(tmp.join("item_bg_temp/x2/5.png"), b"BG5")?;
        fs::write(tmp.join("mob/x2/1"), b"MOB1")?;
        fs::write(tmp.join("map/tile_set_info"), b"TILESET")?;
        fs::write(tmp.join("map/tile_map_data/21"), b"TILEMAP21")?;
        fs::write(tmp.join("map/item_bg_map_data/21"), b"\x00\x00")?;
        fs::write(tmp.join("map/eff_map/21"), b"\x00\x00")?;

        let assets = DataGameAssets::open(&tmp);
        let mut tpl = GameTemplates::default();
        tpl.images_by_name.insert("mount_12_0".to_owned(), 4);
        tpl.flag_bags.push((1, "Cờ Đỏ".to_owned(), 1000, 0, 2001, vec![2002, 2003]));
        tpl.skills.push(SkillTemplateRecord {
            nclass_id: 0, id: 0, name: "Đấm Dragon".to_owned(), max_point: 7,
            mana_use_type: 0, skill_type: 1, icon_id: 539, dam_info: "100%".to_owned(), slot: 0, levels: Vec::new(),
        });
        tpl.maps.push(MapTemplateRecord {
            id: MapId(21), name: "Nhà Gôhan".to_owned(), zones: 1, max_player: 15,
            map_type: 0, planet_id: 0, bg_type: 0, tile_id: 1, bg_id: 0,
            waypoints: Vec::new(), mobs: Vec::new(), npcs: Vec::new(),
        });
        for idx in 0..900_i32 {
            tpl.items.push(ItemTemplateRecord {
                id: idx, item_type: 0, gender: 0, name: format!("Item {idx}"),
                description: Some("Mô tả".to_owned()), level: 1, icon_id: 390, part: 1,
                is_up_to_up: false, power_require: 100, gold: 0, gem: 0, head: -1, body: -1, leg: -1,
            });
        }

        assert_eq!(assets.pkt_size_res(2).cmd, -74);
        let res_pack = assets.pkts_res_pack(2);
        assert_eq!(res_pack.len(), 2);
        assert!(is_extended_s2c_cmd(res_pack[0].cmd));
        assert_eq!(assets.pkt_update_data().expect("-87").cmd, -87);
        assert_eq!(assets.pkt_icon(2, 4028).expect("-67").cmd, -67);
        assert_eq!(assets.pkt_effect_template(2, 231, 60, 60).expect("-66").cmd, -66);
        assert_eq!(assets.pkt_img_by_name(&tpl, 2, "mount_12_0").cmd, 66);
        assert_eq!(assets.pkt_img_by_name(&tpl, 2, "../etc/passwd").reader().remaining(), 2 + 13 + 1 + 4);
        assert_eq!(assets.pkt_item_bg_template(2, 5).expect("-32").cmd, -32);
        assert_eq!(assets.pkt_mob_template(2, 1).expect("11").cmd, 11);
        assert_eq!(assets.pkt_tile_set_info().expect("-82").cmd, -82);
        assert_eq!(assets.pkt_map_temp(21).expect("-28:10").cmd, -28);
        assert_eq!(DataGameAssets::pkt_link_ip("NRO", "127.0.0.1", 14445).cmd, -29);
        let item_pkts = data_game_tpl::pkts_update_item(&tpl);
        assert_eq!(item_pkts.len(), 3);
        assert_eq!(item_pkts[2].cmd, 12);
        assert!(is_extended_s2c_cmd(item_pkts[2].cmd));
        assert_eq!(data_game_tpl::pkt_flag_bag(&tpl, 1, true).expect("-62").cmd, -62);
        assert_eq!(data_game_tpl::pkt_flag_bag(&tpl, 1, false).expect("-63").cmd, -63);

        let snap = PlayerSnapshot::from_seed(PlayerId(1), &NewPlayerSeed {
            account_id: AccountId(1), name: "goku01".to_owned(), gender: Gender::TraiDat, hair: 0, test_mode: false, now_ms: 0,
        });
        let world_pkts = data_game_tpl::pkts_client_ok(&assets, &tpl, &snap);
        assert_eq!(
            [world_pkts[0].cmd, world_pkts[1].cmd, world_pkts[2].cmd, world_pkts[3].cmd],
            [-30, -64, 40, -24]
        );

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }

    #[tokio::test]
    async fn bootstrap_discovers_nested_sql_and_formats_clear_missing_hint() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("nro_boot_sql_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;

        let mut cfg = ServerConfig::default();
        cfg.storage.path = tmp.join("data/nro.fjall");
        cfg.storage.bootstrap_sql_path = Some(PathBuf::from("database team2026.sql"));

        let err = crate::runtime::open_or_bootstrap_store_in(&cfg, &tmp)
            .await
            .expect_err("empty store without SQL should return clear hint");
        let msg = format!("{err:#}");
        assert!(msg.contains("is empty and no SQL seed dump was found"), "unexpected error: {msg}");
        assert!(!msg.contains("achievement_template"), "raw table list leaked: {msg}");

        let nested_zip = tmp.join("teamobi2026_databases (1)");
        fs::create_dir_all(&nested_zip)?;
        let sql_file = nested_zip.join("database team2026.sql");
        fs::write(&sql_file, b"-- sql stub")?;
        let discovered = crate::runtime::discover_bootstrap_sql_in(
            &tmp,
            cfg.storage.bootstrap_sql_path.as_deref(),
        );
        assert_eq!(discovered, Some(sql_file));

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
