#![forbid(unsafe_code)]
//! Embedded single-binary Windows/desktop launcher (`nro-server-gui`) running the authoritative
//! `nro-server` Tokio runtime on a background thread and the `gpui-kit` Admin Control Panel on
//! the main UI thread.

mod app;
mod inspector;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Context as _;
use clap::Parser;
use gpui_kit::*;
use nro_server::admin_control::AdminControl;
use nro_server::config::ServerConfig;
use nro_server::runtime::ServerRuntime;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::app::AdminApp;

/// Command-line flags for `nro-server-gui`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "nro-server-gui",
    version,
    about = "Ngọc Rồng Online (Lữ Khách Ký Ức) — Embedded Server + GPUI Kit Control Panel"
)]
struct GuiCliArgs {
    /// Path to `server.toml` configuration file.
    #[arg(short, long, value_name = "PATH", default_value = "server.toml")]
    config: PathBuf,

    /// Optional TCP bind override (default `0.0.0.0:14445`).
    #[arg(short, long, value_name = "ADDR")]
    bind: Option<SocketAddr>,

    /// Optional embedded `fjall` storage directory override.
    #[arg(long, value_name = "DIR")]
    storage_path: Option<PathBuf>,

    /// Optional `SRC/data` binary client asset directory override.
    #[arg(long, value_name = "ASSETS_DIR")]
    assets_dir: Option<PathBuf>,

    /// Default `tracing` filter directive when `RUST_LOG` is unset.
    #[arg(long, value_name = "DIRECTIVE", default_value = "info,nro_server=info")]
    log_filter: String,
}

fn init_tracing(default_directive: &str) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_directive))
        .context("invalid tracing filter directive")?;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .try_init();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = GuiCliArgs::parse();
    init_tracing(&cli.log_filter)?;

    let config = ServerConfig::load_with_overrides(
        &cli.config,
        cli.bind,
        cli.storage_path.as_deref(),
        cli.assets_dir.as_deref(),
        false,
    )
    .with_context(|| format!("failed loading configuration from {}", cli.config.display()))?;

    let shutdown_token = CancellationToken::new();
    let bg_shutdown = shutdown_token.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<anyhow::Result<AdminControl>>();

    let server_thread = thread::Builder::new()
        .name("nro-server-tokio".to_owned())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.into()));
                    return;
                }
            };
            let rt_handle = rt.handle().clone();
            rt.block_on(async move {
                match ServerRuntime::boot(config, bg_shutdown.clone()).await {
                    Ok(runtime) => {
                        let control = AdminControl::new(runtime.shared(), rt_handle, bg_shutdown.clone());
                        let _ = ready_tx.send(Ok(control));
                        bg_shutdown.cancelled().await;
                        info!("GUI requested shutdown; flushing online player snapshots to fjall");
                        if let Err(err) = runtime.shutdown_and_flush().await {
                            error!(error = %err, "error during server shutdown flush");
                        }
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                }
            });
        })
        .context("failed spawning background server runtime thread")?;

    let control = ready_rx
        .recv()
        .context("background server thread exited before ready")??;
    let close_control = control.clone();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let mut opts = WindowOptions::default();
            opts.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.0), px(820.0)),
                cx,
            )));
            let _ = gpui_kit::open_window(opts, cx, |window, cx| {
                cx.new(|cx| AdminApp::new(control, window, cx))
            });
        });

    close_control.trigger_shutdown();
    let _ = server_thread.join();
    Ok(())
}
