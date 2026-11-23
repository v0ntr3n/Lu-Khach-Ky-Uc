//! `fjall` store bootstrap, consignment market serialization, and TCP/zone-event consumer loops.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use nro_core::{ItemInstance, PlayerId};
use nro_db::legacy_sql::{ImportOptions, import_mysqldump_file};
use nro_db::pool::DbPool;
use nro_db::world_repos::ConsignItemRecord;
use nro_economy::{ConsignCurrency, ConsignListing, ConsignShop, ConsignTab};
use nro_protocol::{SessionConfig, spawn_tcp_session};
use nro_world::ZoneEvent;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use super::{SharedServerState, runtime_pet};
use crate::config::ServerConfig;
use crate::router::ServerSessionHandler;

#[derive(serde::Serialize, serde::Deserialize)]
struct ConsignShopSerde {
    next_id: u64,
    listings: Vec<ConsignListing>,
}

pub(super) fn consign_shop_from_records(mut records: Vec<ConsignItemRecord>) -> ConsignShop {
    records.sort_by_key(|r| (r.is_up_top == 0, r.id));
    let mut next_id = 0_u64;
    let listings = records
        .into_iter()
        .map(|r| {
            let id = u64::from(r.id.max(0).cast_unsigned());
            next_id = next_id.max(id);
            let tab = match r.tab {
                0 => ConsignTab::AoQuan,
                1 => ConsignTab::GangTay,
                2 => ConsignTab::PhuKien,
                4 => ConsignTab::SellerManage,
                _ => ConsignTab::LinhTinh,
            };
            let (currency, price) = if r.gem > 0 {
                (ConsignCurrency::Gem, u64::from(r.gem.cast_unsigned()))
            } else {
                (
                    ConsignCurrency::Gold,
                    u64::from(r.gold.max(0).cast_unsigned()),
                )
            };
            ConsignListing {
                id,
                seller_id: PlayerId(i64::from(r.player_id)),
                tab,
                currency,
                price,
                sold: r.is_buy,
                item: ItemInstance {
                    template_id: r.item_id,
                    quantity: r.quantity.max(1).cast_unsigned(),
                    options: r.options,
                    create_time_ms: 0,
                },
            }
        })
        .collect();
    toml::to_string(&ConsignShopSerde { next_id, listings })
        .ok()
        .and_then(|raw| toml::from_str::<ConsignShop>(&raw).ok())
        .unwrap_or_default()
}

pub(super) fn consign_shop_to_records(
    shop: &ConsignShop,
) -> Result<Vec<ConsignItemRecord>, String> {
    let raw = toml::to_string(shop).map_err(|e| format!("consign_shop serialize failed: {e}"))?;
    let view: ConsignShopSerde =
        toml::from_str(&raw).map_err(|e| format!("consign_shop deserialize failed: {e}"))?;
    let mut is_top = vec![0_i8; view.listings.len()];
    let mut min_later_id = u64::MAX;
    for (idx, l) in view.listings.iter().enumerate().rev() {
        if l.id > min_later_id {
            is_top[idx] = 1;
        }
        min_later_id = min_later_id.min(l.id);
    }
    Ok(view
        .listings
        .into_iter()
        .zip(is_top)
        .map(|(l, up_top)| ConsignItemRecord {
            id: i32::try_from(l.id).unwrap_or(i32::MAX),
            player_id: i32::try_from(l.seller_id.0).unwrap_or(0),
            tab: l.tab as i8,
            item_id: l.item.template_id,
            gold: if l.currency == ConsignCurrency::Gold {
                i32::try_from(l.price).unwrap_or(i32::MAX)
            } else {
                -1
            },
            gem: if l.currency == ConsignCurrency::Gem {
                i32::try_from(l.price).unwrap_or(i32::MAX)
            } else {
                -1
            },
            quantity: i32::try_from(l.item.quantity).unwrap_or(i32::MAX),
            options: l.item.options,
            is_up_top: up_top,
            is_buy: l.sold,
        })
        .collect())
}

pub(super) async fn open_or_bootstrap_store(config: &ServerConfig) -> anyhow::Result<DbPool> {
    open_or_bootstrap_store_in(config, Path::new(".")).await
}

pub(crate) async fn open_or_bootstrap_store_in(
    config: &ServerConfig,
    base_dir: &Path,
) -> anyhow::Result<DbPool> {
    let mut fjall_cfg = config.storage.to_fjall_config();
    let strict = fjall_cfg.strict_schema_check;
    fjall_cfg.strict_schema_check = false;
    let pool = DbPool::open(&fjall_cfg)
        .await
        .context("embedded fjall init failed")?;
    if pool.check_schema_health().await?.is_healthy() {
        return Ok(pool);
    }

    if let Some(sql_path) =
        discover_bootstrap_sql_in(base_dir, config.storage.bootstrap_sql_path.as_deref())
    {
        info!(sql = %sql_path.display(), "unseeded fjall store detected; auto-importing SQL dump");
        let (p, path, parity) = (pool.clone(), sql_path.clone(), config.parity_mode);
        tokio::task::spawn_blocking(move || {
            import_mysqldump_file(
                &p,
                &path,
                ImportOptions {
                    parity_mode: parity,
                    upgrade_passwords: false,
                },
            )
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
    let target = if expected.is_absolute() {
        expected.to_path_buf()
    } else {
        resolved_base.join(expected)
    };
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
        let direct = if cfg.is_absolute() {
            cfg.to_path_buf()
        } else {
            base.join(cfg)
        };
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
        .map(|e| e.path())
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

pub(super) fn spawn_zone_event_consumer(
    tracker: &TaskTracker,
    shared: Arc<SharedServerState>,
    mut rx: mpsc::Receiver<ZoneEvent>,
    shutdown: CancellationToken,
) {
    tracker.spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                ev = rx.recv() => match ev {
                    Some(ev) => runtime_pet::handle_zone_event(&shared, ev).await,
                    None => break,
                },
            }
        }
    });
}

pub(super) fn spawn_accept_loop(
    tracker: &TaskTracker,
    session_tracker: TaskTracker,
    listener: TcpListener,
    shared: Arc<SharedServerState>,
    shutdown: CancellationToken,
) {
    let next_sid = Arc::new(AtomicU64::new(1));
    let handler = Arc::new(ServerSessionHandler::new(Arc::clone(&shared)));
    tracker.spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                accepted = listener.accept() => {
                    let Ok((stream, peer_addr)) = accepted else { continue };
                    if shared.clients.sessions.len() >= shared.config.rate_limit.max_online_players.saturating_mul(2) {
                        debug!(active = shared.clients.sessions.len(), "global pre-auth connection cap exceeded");
                        continue;
                    }
                    if !shared.rate_limiter.try_acquire(peer_addr.ip()) {
                        debug!(ip = %peer_addr.ip(), "per-IP connection limit exceeded");
                        continue;
                    }
                    let sid = next_sid.fetch_add(1, Ordering::Relaxed);
                    match spawn_tcp_session(sid, stream, peer_addr, SessionConfig::default(), Arc::clone(&handler)) {
                        Ok((handle, join)) => {
                            shared.clients.sessions.insert(sid, handle.clone());
                            if !handle.is_connected() { shared.clients.sessions.remove(&sid); }
                            session_tracker.spawn(async move { let _ = join.await; });
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
