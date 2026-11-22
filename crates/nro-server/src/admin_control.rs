#![forbid(unsafe_code)]
//! Authoritative in-process admin control facade for `nro-server` and `nro-admin-ui`.

#[path = "admin_control_extra.rs"]
mod admin_control_extra;
#[path = "admin_control_ops.rs"]
mod admin_control_ops;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nro_core::AccountId;
use nro_db::player_repo::PlayerSnapshot;
use nro_db::templates::MapTemplateRecord;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::runtime::SharedServerState;

const MAX_LIST_ROWS: usize = 100;

/// High-level runtime metrics shown in the admin control panel header.
#[derive(Debug, Clone)]
pub struct ServerSummary {
    pub online_players: usize,
    pub total_sessions: usize,
    pub maintaining: bool,
    pub uptime_secs: u64,
    pub bind_addr: String,
}

/// Lightweight item template summary for admin item search.
#[derive(Debug, Clone)]
pub struct AdminItemInfo {
    pub id: i16,
    pub name: String,
    pub stackable: bool,
}

/// Unified account + character row for the admin user table.
#[derive(Debug, Clone)]
pub struct AdminUserRow {
    pub account_id: i32,
    pub username: String,
    pub player_id: Option<i64>,
    pub player_name: String,
    pub online: bool,
    pub ban: bool,
    pub active: bool,
    pub is_admin: bool,
    pub map_id: i16,
    pub map_name: String,
    pub zone_id: u16,
    pub power: i64,
    pub gold: i64,
    pub gem: i32,
    pub ruby: i32,
    pub vnd: i32,
}

/// Thread-safe handle allowing an embedded GUI to inspect and control live server state.
#[derive(Clone)]
pub struct AdminControl {
    pub(super) shared: Arc<SharedServerState>,
    pub(super) rt: tokio::runtime::Handle,
    shutdown: CancellationToken,
    bind_addr: Arc<str>,
    shutdown_triggered: Arc<AtomicBool>,
}

impl AdminControl {
    /// Creates a new [`AdminControl`] bound to a running [`SharedServerState`].
    #[must_use]
    pub fn new(
        shared: Arc<SharedServerState>,
        rt: tokio::runtime::Handle,
        shutdown: CancellationToken,
    ) -> Self {
        let bind_addr: Arc<str> =
            Arc::from(format!("{}:{}", shared.config.bind_ip, shared.config.port));
        Self {
            shared,
            rt,
            shutdown,
            bind_addr,
            shutdown_triggered: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the cached `ip:port` bind address without touching runtime registries.
    #[must_use]
    pub fn bind_addr_cached(&self) -> &str {
        &self.bind_addr
    }

    /// Returns the cached `ip:port` bind address (lock-free alias for [`Self::bind_addr_cached`]).
    #[must_use]
    pub fn bind_addr(&self) -> &str {
        &self.bind_addr
    }

    /// Returns current server health & online counts.
    #[must_use]
    pub fn summary(&self) -> ServerSummary {
        ServerSummary {
            online_players: self.shared.clients.authenticated.len(),
            total_sessions: self.shared.clients.sessions.len(),
            maintaining: self.shared.maintenance.is_maintaining(),
            uptime_secs: self.shared.started_at.elapsed().as_secs(),
            bind_addr: self.bind_addr.to_string(),
        }
    }

    /// Lists up to `limit` online users from in-memory session state with zero DB I/O.
    #[must_use]
    pub fn list_online_users(&self, query: &str, limit: usize) -> Vec<AdminUserRow> {
        if limit == 0 {
            return Vec::new();
        }
        let needle = query.trim().to_ascii_lowercase();
        let mut rows = Vec::with_capacity(limit.min(MAX_LIST_ROWS));
        for entry in &self.shared.clients.authenticated {
            let e = entry.value();
            let Some(aid) = e.snapshot.account_id else {
                continue;
            };
            let username = format!("acc_{}", aid.0);
            if !matches_query(
                &needle,
                aid.0,
                &username,
                Some(e.snapshot.id.0),
                &e.snapshot.name,
            ) {
                continue;
            }
            rows.push(row_from_snap(
                aid.0,
                username,
                Some(e.snapshot.clone()),
                &self.shared.templates.maps,
                true,
                false,
                e.activated,
                e.is_admin,
                0,
                0,
            ));
        }
        rows.sort_by_key(|r| r.account_id);
        rows.truncate(limit);
        rows
    }

    /// Searches up to `limit` online and offline accounts by username, character name, or IDs.
    pub fn search_users(&self, query: &str, limit: usize) -> Result<Vec<AdminUserRow>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let needle = query.trim().to_ascii_lowercase();
        let mut online_by_acc: HashMap<i32, PlayerSnapshot> = HashMap::new();
        for entry in &self.shared.clients.authenticated {
            if let Some(aid) = entry.value().snapshot.account_id {
                online_by_acc.insert(aid.0, entry.value().snapshot.clone());
            }
        }
        let mut accounts = self
            .shared
            .accounts_repo
            .list_all()
            .map_err(|err| format!("Lỗi DB khi tải danh sách tài khoản: {err}"))?;
        accounts.sort_by(|a, b| {
            let ao = online_by_acc.contains_key(&a.id);
            let bo = online_by_acc.contains_key(&b.id);
            bo.cmp(&ao).then_with(|| a.id.cmp(&b.id))
        });

        let mut rows = Vec::with_capacity(limit.min(MAX_LIST_ROWS));
        for acc in accounts {
            if rows.len() >= limit {
                break;
            }
            let live = online_by_acc.remove(&acc.id);
            let online = live.is_some();
            let snap = live.or_else(|| {
                self.rt
                    .block_on(
                        self.shared
                            .players_repo
                            .load_by_account(AccountId(acc.id), self.shared.config.parity_mode),
                    )
                    .ok()
                    .flatten()
            });
            let (pid, pname) = snap
                .as_ref()
                .map_or((None, ""), |s| (Some(s.id.0), s.name.as_str()));
            if !matches_query(&needle, acc.id, &acc.username, pid, pname) {
                continue;
            }
            rows.push(row_from_snap(
                acc.id,
                acc.username,
                snap,
                &self.shared.templates.maps,
                online,
                acc.ban,
                acc.active != 0,
                acc.is_admin,
                acc.vnd,
                acc.vang,
            ));
        }
        Ok(rows)
    }

    /// Lists up to [`MAX_LIST_ROWS`] accounts (online first), keeping `nro-admin-ui` compatible.
    #[must_use]
    pub fn list_users(&self, query: &str, online_only: bool) -> Vec<AdminUserRow> {
        if online_only {
            self.list_online_users(query, MAX_LIST_ROWS)
        } else {
            self.search_users(query, MAX_LIST_ROWS).unwrap_or_default()
        }
    }

    /// Toggles maintenance mode and notifies all connected players.
    #[must_use]
    pub fn set_maintenance(&self, enabled: bool) -> String {
        self.shared.maintenance.set_maintaining(enabled);
        let notice = if enabled {
            "Máy chủ đã bật chế độ bảo trì, tạm khóa đăng nhập mới!"
        } else {
            "Máy chủ đã kết thúc bảo trì, mở lại đăng nhập bình thường!"
        };
        let sent = self.shared.clients.broadcast_server_notify(notice);
        info!(enabled, sent, "admin UI toggled maintenance mode");
        let label = if enabled {
            "Đã BẬT bảo trì"
        } else {
            "Đã TẮT bảo trì"
        };
        format!("{label} (đã thông báo tới {sent} người chơi)")
    }

    /// Broadcasts a server-wide alert banner (`SERVER_ALERT`) to all online players.
    pub fn broadcast(&self, message: &str) -> Result<String, String> {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return Err("Nội dung thông báo không được để trống.".to_owned());
        }
        let sent = self.shared.clients.broadcast_server_notify(trimmed);
        info!(sent, message = trimmed, "admin UI sent global broadcast");
        Ok(format!("Đã gửi thông báo tới {sent} người chơi online."))
    }

    /// Signals the background server runtime to flush all players and shut down cleanly (idempotent).
    pub fn trigger_shutdown(&self) {
        if self.shutdown_triggered.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.shared.maintenance.try_begin_maintenance();
        self.shared
            .clients
            .broadcast_server_notify("Máy chủ đang tắt từ bảng điều khiển, đang lưu dữ liệu...");
        self.shutdown.cancel();
    }
}

fn matches_query(needle: &str, acc_id: i32, username: &str, pid: Option<i64>, pname: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    acc_id.to_string().contains(needle)
        || pid.is_some_and(|p| p.to_string().contains(needle))
        || username.to_ascii_lowercase().contains(needle)
        || pname.to_ascii_lowercase().contains(needle)
}

fn row_from_snap(
    account_id: i32,
    username: String,
    snap: Option<PlayerSnapshot>,
    maps: &[MapTemplateRecord],
    online: bool,
    ban: bool,
    active: bool,
    is_admin: bool,
    vnd: i32,
    fallback_gold: i64,
) -> AdminUserRow {
    let (player_id, player_name, map_id, map_name, zone_id, power, gold, gem, ruby) = snap
        .map_or_else(
            || {
                (
                    None,
                    "(Chưa tạo NV)".to_owned(),
                    0,
                    "-".to_owned(),
                    0,
                    0,
                    fallback_gold,
                    0,
                    0,
                )
            },
            |s| {
                let map_name = maps.iter().find(|m| m.id == s.location.map_id).map_or_else(
                    || format!("Map {}", s.location.map_id.0),
                    |m| m.name.clone(),
                );
                (
                    Some(s.id.0),
                    s.name,
                    s.location.map_id.0,
                    map_name,
                    s.location.zone_id.0,
                    s.stats.power,
                    s.wallet.gold,
                    s.wallet.gem,
                    s.wallet.ruby,
                )
            },
        );
    AdminUserRow {
        account_id,
        username,
        player_id,
        player_name,
        online,
        ban,
        active,
        is_admin,
        map_id,
        map_name,
        zone_id,
        power,
        gold,
        gem,
        ruby,
        vnd,
    }
}
