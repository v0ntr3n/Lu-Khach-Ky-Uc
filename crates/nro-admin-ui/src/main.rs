#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![forbid(unsafe_code)]
//! Embedded single-binary Windows/desktop launcher (`nro-server-gui`) running the authoritative
//! `nro-server` Tokio runtime on a background thread and the `gpui-kit` Admin Control Panel on
//! the main UI thread.

mod app;
mod app_ops;
mod inspector;
mod inspector_cards;
mod inspector_pet;
mod log_buffer;
mod log_view;
mod server_tools;
mod views;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

use crate::app::{AdminApp, BootState};
use crate::log_buffer::LogBuffer;

/// Command-line flags for `nro-server-gui`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "nro-server-gui",
    version,
    about = "Ngọc Rồng Online (Lữ Khách Ký Ức) — Embedded Server + GPUI Kit Control Panel"
)]
pub(crate) struct GuiCliArgs {
    /// Path to `server.toml` configuration file.
    #[arg(short, long, value_name = "PATH", default_value = "server.toml")]
    pub(crate) config: PathBuf,

    /// Optional TCP bind override (default `0.0.0.0:14445`).
    #[arg(short, long, value_name = "ADDR")]
    pub(crate) bind: Option<SocketAddr>,

    /// Optional embedded `fjall` storage directory override.
    #[arg(long, value_name = "DIR")]
    pub(crate) storage_path: Option<PathBuf>,

    /// Optional `SRC/data` binary client asset directory override.
    #[arg(long, value_name = "ASSETS_DIR")]
    pub(crate) assets_dir: Option<PathBuf>,

    /// Default `tracing` filter directive when `RUST_LOG` is unset.
    #[arg(long, value_name = "DIRECTIVE", default_value = "info,nro_server=info")]
    pub(crate) log_filter: String,
}

fn init_tracing(default_directive: &str, log_buffer: &LogBuffer) {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_directive))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_ansi(false)
        .with_writer(log_buffer.clone())
        .try_init();
}

fn spawn_server_thread(
    cli: GuiCliArgs,
    boot_state: Arc<Mutex<BootState>>,
    shutdown_token: CancellationToken,
) -> thread::JoinHandle<()> {
    let bg_shutdown = shutdown_token;
    thread::Builder::new()
        .name("nro-server-tokio".to_owned())
        .spawn(move || {
            let boot_res = (|| -> anyhow::Result<()> {
                let config = ServerConfig::load_with_overrides(
                    &cli.config,
                    cli.bind,
                    cli.storage_path.as_deref(),
                    cli.assets_dir.as_deref(),
                    false,
                )
                .with_context(|| {
                    let cwd = std::env::current_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| ".".to_owned());
                    format!(
                        "Không đọc được cấu hình '{}' (thư mục hiện tại: {cwd}). Hãy đặt nro-server-gui.exe vào thư mục gốc repo Lu-Khach-Ky-Uc (cùng chỗ với server.toml) hoặc chạy start_gui.bat.",
                        cli.config.display()
                    )
                })?;

                let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
                let rt_handle = rt.handle().clone();
                let ready_state = Arc::clone(&boot_state);
                rt.block_on(async move {
                    let runtime = ServerRuntime::boot(config, bg_shutdown.clone()).await?;
                    let control = AdminControl::new(runtime.shared(), rt_handle, bg_shutdown.clone());
                    if let Ok(mut guard) = ready_state.lock() {
                        *guard = BootState::Ready(control);
                    }
                    bg_shutdown.cancelled().await;
                    info!("GUI requested shutdown; flushing online player snapshots to fjall");
                    if let Err(err) = runtime.shutdown_and_flush().await {
                        error!(error = %err, "error during server shutdown flush");
                    }
                    Ok(())
                })
            })();

            if let Err(err) = boot_res {
                error!(error = %err, "failed booting embedded nro-server");
                if let Ok(mut guard) = boot_state.lock() {
                    *guard = BootState::Failed(format!("{err:#}"));
                }
            }
        })
        .unwrap_or_else(|err| {
            eprintln!("failed spawning server thread: {err}");
            thread::spawn(|| {})
        })
}

fn main() {
    let cli = GuiCliArgs::parse();
    let log_buffer = LogBuffer::new();
    init_tracing(&cli.log_filter, &log_buffer);

    let boot_state = Arc::new(Mutex::new(BootState::Booting));
    let shutdown_token = CancellationToken::new();
    let server_thread = spawn_server_thread(cli, Arc::clone(&boot_state), shutdown_token.clone());

    let ui_state = Arc::clone(&boot_state);
    let ui_logs = log_buffer;
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.activate(true);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.0), px(820.0)), cx)),
                focus: true,
                show: true,
                ..Default::default()
            };
            if let Err(err) = gpui_kit::open_window(window_options, cx, |window, cx| {
                cx.new(|cx| AdminApp::new(ui_state, ui_logs, window, cx))
            }) {
                error!(error = %err, "failed to open GPUI window");
                eprintln!("[nro-server-gui] Failed to open GPUI window: {err:#}");
                cx.quit();
            }
        });

    if !shutdown_token.is_cancelled() {
        if let Ok(guard) = boot_state.lock()
            && let BootState::Ready(control) = &*guard
        {
            control.trigger_shutdown();
        } else {
            shutdown_token.cancel();
        }
    }
    let _ = server_thread.join();
}
