#![forbid(unsafe_code)]
//! Authoritative in-process admin control facade for `nro-server` and `nro-admin-ui`.

use std::collections::HashMap;
use std::sync::Arc;

use nro_core::{AccountId, ItemInstance, TemplateId};
use nro_db::player_repo::PlayerSnapshot;
use nro_world::s2c_packets;
use smallvec::SmallVec;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::router::{self, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt};
use crate::runtime::{SharedServerState, unix_now_ms};

const MAX_LIST_ROWS: usize = 100;

/// High-level runtime metrics shown in the admin control panel header.
#[derive(Debug, Clone)]
pub struct ServerSummary {
    pub online_players: usize, pub total_sessions: usize, pub maintaining: bool,
    pub uptime_secs: u64, pub bind_addr: String,
}

/// Unified account + character row for the admin user table.
#[derive(Debug, Clone)]
pub struct AdminUserRow {
    pub account_id: i32, pub username: String, pub player_id: Option<i64>, pub player_name: String,
    pub online: bool, pub ban: bool, pub active: bool, pub is_admin: bool,
    pub map_id: i16, pub zone_id: u16, pub power: i64, pub gold: i64,
    pub gem: i32, pub ruby: i32, pub vnd: i32,
}

/// Thread-safe handle allowing an embedded GUI to inspect and control live server state.
#[derive(Clone)]
pub struct AdminControl {
    shared: Arc<SharedServerState>,
    rt: tokio::runtime::Handle,
    shutdown: CancellationToken,
}

impl AdminControl {
    /// Creates a new [`AdminControl`] bound to a running [`SharedServerState`].
    #[must_use]
    pub const fn new(
        shared: Arc<SharedServerState>,
        rt: tokio::runtime::Handle,
        shutdown: CancellationToken,
    ) -> Self {
        Self { shared, rt, shutdown }
    }

    /// Returns current server health & online counts.
    #[must_use]
    pub fn summary(&self) -> ServerSummary {
        ServerSummary {
            online_players: self.shared.clients.authenticated.len(),
            total_sessions: self.shared.clients.sessions.len(),
            maintaining: self.shared.maintenance.is_maintaining(),
            uptime_secs: self.shared.started_at.elapsed().as_secs(),
            bind_addr: format!("{}:{}", self.shared.config.bind_ip, self.shared.config.port),
        }
    }

    /// Lists up to [`MAX_LIST_ROWS`] accounts (online first), bounded to avoid full offline player scans.
    #[must_use]
    pub fn list_users(&self, query: &str, online_only: bool) -> Vec<AdminUserRow> {
        let needle = query.trim().to_ascii_lowercase();
        let mut online_by_acc: HashMap<i32, PlayerSnapshot> = HashMap::new();
        for entry in &self.shared.clients.authenticated {
            if let Some(aid) = entry.value().snapshot.account_id {
                online_by_acc.insert(aid.0, entry.value().snapshot.clone());
            }
        }
        let mut rows = Vec::with_capacity(MAX_LIST_ROWS);
        if online_only {
            for (aid, snap) in online_by_acc {
                let acc = self.shared.accounts_repo.get_by_id(AccountId(aid)).ok().flatten();
                let (username, ban, active, is_admin, vnd) = match acc {
                    Some(a) => (a.username, a.ban, a.active != 0, a.is_admin, a.vnd),
                    None => (format!("acc_{aid}"), false, false, false, 0),
                };
                if !matches_query(&needle, aid, &username, Some(snap.id.0), &snap.name) {
                    continue;
                }
                rows.push(row_from_snap(aid, username, Some(snap), true, ban, active, is_admin, vnd, 0));
            }
            rows.sort_by_key(|r| r.account_id);
            rows.truncate(MAX_LIST_ROWS);
            return rows;
        }

        let mut accounts = self.shared.accounts_repo.list_all().unwrap_or_default();
        accounts.sort_by(|a, b| {
            let ao = online_by_acc.contains_key(&a.id);
            let bo = online_by_acc.contains_key(&b.id);
            bo.cmp(&ao).then_with(|| a.id.cmp(&b.id))
        });

        for acc in accounts {
            if rows.len() >= MAX_LIST_ROWS {
                break;
            }
            let live = online_by_acc.remove(&acc.id);
            let online = live.is_some();
            if !online && !needle.is_empty() && !matches_query(&needle, acc.id, &acc.username, None, "") {
                continue;
            }
            let snap = live.or_else(|| {
                self.rt
                    .block_on(self.shared.players_repo.load_by_account(AccountId(acc.id), self.shared.config.parity_mode))
                    .ok()
                    .flatten()
            });
            let (pid, pname) = match &snap {
                Some(s) => (Some(s.id.0), s.name.as_str()),
                None => (None, ""),
            };
            if !matches_query(&needle, acc.id, &acc.username, pid, pname) {
                continue;
            }
            rows.push(row_from_snap(
                acc.id,
                acc.username,
                snap,
                online,
                acc.ban,
                acc.active != 0,
                acc.is_admin,
                acc.vnd,
                acc.vang,
            ));
        }
        rows
    }

    /// Disconnects the live session for `account_id` if currently online.
    pub fn kick_user(&self, account_id: i32) -> Result<String, String> {
        let Some(sid) = self.shared.clients.accounts.get(&AccountId(account_id)).map(|v| *v.value()) else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };
        if let Some(handle) = self.shared.clients.sessions.get(&sid) {
            handle.disconnect();
            info!(account_id, "admin UI kicked online account");
            return Ok(format!("Đã kick tài khoản #{account_id}."));
        }
        Err(format!("Không tìm thấy phiên kết nối cho tài khoản #{account_id}."))
    }

    /// Updates `ban` flag in `fjall` and kicks the account immediately when banning.
    pub fn set_ban(&self, account_id: i32, ban: bool) -> Result<String, String> {
        let updated = self
            .shared
            .accounts_repo
            .set_ban(AccountId(account_id), ban)
            .map_err(|err| format!("Lỗi DB khi cập nhật ban: {err}"))?;
        if !updated {
            return Err(format!("Không tìm thấy tài khoản #{account_id}."));
        }
        if ban {
            let _ = self.kick_user(account_id);
        }
        info!(account_id, ban, "admin UI updated account ban status");
        Ok(if ban {
            format!("Đã khóa (ban) tài khoản #{account_id}.")
        } else {
            format!("Đã mở khóa (unban) tài khoản #{account_id}.")
        })
    }

    /// Updates member activation (`active`) in `fjall` and in live session state.
    pub fn set_active(&self, account_id: i32, active: bool) -> Result<String, String> {
        let updated = self
            .shared
            .accounts_repo
            .set_active(AccountId(account_id), active)
            .map_err(|err| format!("Lỗi DB khi cập nhật kích hoạt: {err}"))?;
        if !updated {
            return Err(format!("Không tìm thấy tài khoản #{account_id}."));
        }
        if let Some(sid) = self.shared.clients.accounts.get(&AccountId(account_id)).map(|v| *v.value())
            && let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid)
        {
            entry.activated = active;
        }
        info!(account_id, active, "admin UI updated account activation");
        Ok(if active {
            format!("Đã kích hoạt thành viên cho tài khoản #{account_id}.")
        } else {
            format!("Đã hủy kích hoạt thành viên cho tài khoản #{account_id}.")
        })
    }

    /// Adjusts account VND balance in `fjall`.
    pub fn add_vnd(&self, account_id: i32, delta: i64) -> Result<String, String> {
        if delta == 0 {
            return Err("Số VND cần cộng/trừ phải khác 0.".to_owned());
        }
        if delta > 0 {
            self.rt
                .block_on(self.shared.accounts_repo.add_vnd(AccountId(account_id), delta))
                .map_err(|err| format!("Lỗi cộng VND: {err}"))?;
            Ok(format!("Đã cộng +{delta} VND cho tài khoản #{account_id}."))
        } else {
            let amount = i32::try_from(delta.unsigned_abs()).unwrap_or(i32::MAX);
            let ok = self
                .rt
                .block_on(self.shared.accounts_repo.sub_vnd(AccountId(account_id), amount))
                .map_err(|err| format!("Lỗi trừ VND: {err}"))?;
            if ok {
                Ok(format!("Đã trừ {amount} VND của tài khoản #{account_id}."))
            } else {
                Err(format!("Tài khoản #{account_id} không đủ VND để trừ {amount}."))
            }
        }
    }

    /// Grants or deducts gold/gem/ruby for an online or offline character, rejecting mid-trade mutations.
    pub fn grant_currency(
        &self,
        account_id: i32,
        gold_delta: i64,
        gem_delta: i32,
        ruby_delta: i32,
    ) -> Result<String, String> {
        if gold_delta == 0 && gem_delta == 0 && ruby_delta == 0 {
            return Err("Nhập ít nhất 1 giá trị Vàng / Ngọc / Hồng ngọc khác 0.".to_owned());
        }
        if let Some(sid) = self.shared.clients.accounts.get(&AccountId(account_id)).map(|v| *v.value()) {
            let (handle, money_pkt, notify_pkt) = {
                let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) else {
                    return Err("Không tìm thấy phiên người chơi.".to_owned());
                };
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                apply_wallet_deltas(&mut entry.snapshot, gold_delta, gem_delta, ruby_delta);
                let handle = entry.handle.clone();
                let money_pkt = wallet_money_pkt(&entry.snapshot.wallet);
                let notify_pkt = s2c_packets::thong_bao(&format!(
                    "Admin cập nhật ví: Vàng {gold_delta:+}, Ngọc {gem_delta:+}, Hồng ngọc {ruby_delta:+}"
                ));
                (handle, money_pkt, notify_pkt)
            };
            let _ = handle.try_send(money_pkt);
            if let Ok(notify) = notify_pkt {
                let _ = handle.try_send(notify);
            }
            return Ok(format!("Đã cập nhật tiền online cho tài khoản #{account_id}."));
        }
        let mut snap = self
            .rt
            .block_on(self.shared.players_repo.load_by_account(AccountId(account_id), self.shared.config.parity_mode))
            .map_err(|err| format!("Lỗi đọc nhân vật: {err}"))?
            .ok_or_else(|| format!("Tài khoản #{account_id} chưa tạo nhân vật."))?;
        apply_wallet_deltas(&mut snap, gold_delta, gem_delta, ruby_delta);
        self.rt
            .block_on(self.shared.players_repo.save_player(&snap, None, self.shared.config.parity_mode, unix_now_ms()))
            .map_err(|err| format!("Lỗi lưu nhân vật: {err}"))?;
        Ok(format!("Đã cập nhật tiền offline cho tài khoản #{account_id}."))
    }

    /// Gives `qty` of `template_id` into the player's bag (online or offline).
    pub fn give_item(&self, account_id: i32, template_id: i16, qty: u32) -> Result<String, String> {
        if template_id < 0 || qty == 0 {
            return Err("Template ID phải >= 0 và số lượng phải > 0.".to_owned());
        }
        let item = ItemInstance {
            template_id: TemplateId(template_id),
            quantity: qty,
            options: SmallVec::new(),
            create_time_ms: unix_now_ms(),
        };
        if let Some(sid) = self.shared.clients.accounts.get(&AccountId(account_id)).map(|v| *v.value()) {
            let (handle, bag_pkt, notify_pkt) = {
                let mut entry = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .ok_or_else(|| "Không tìm thấy phiên người chơi.".to_owned())?;
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                inv.add_to_bag(item, true).map_err(|err| format!("Hành trang đầy hoặc lỗi thêm đồ: {err}"))?;
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                let handle = entry.handle.clone();
                let bag_pkt = s2c_packets::inventory_bag(&entry.snapshot.items_bag);
                let notify_pkt = s2c_packets::thong_bao(&format!("Admin vừa tặng bạn x{qty} vật phẩm #{template_id}!"));
                (handle, bag_pkt, notify_pkt)
            };
            let _ = handle.try_send(bag_pkt);
            if let Ok(msg) = notify_pkt {
                let _ = handle.try_send(msg);
            }
            return Ok(format!("Đã tặng x{qty} item #{template_id} (online) cho tài khoản #{account_id}."));
        }
        let mut snap = self
            .rt
            .block_on(self.shared.players_repo.load_by_account(AccountId(account_id), self.shared.config.parity_mode))
            .map_err(|err| format!("Lỗi đọc nhân vật: {err}"))?
            .ok_or_else(|| format!("Tài khoản #{account_id} chưa tạo nhân vật."))?;
        let mut inv = snapshot_to_economy_inv(&snap);
        inv.add_to_bag(item, true).map_err(|err| format!("Hành trang đầy hoặc lỗi thêm đồ: {err}"))?;
        sync_economy_inv_to_snapshot(&inv, &mut snap);
        self.rt
            .block_on(self.shared.players_repo.save_player(&snap, None, self.shared.config.parity_mode, unix_now_ms()))
            .map_err(|err| format!("Lỗi lưu nhân vật: {err}"))?;
        Ok(format!("Đã tặng x{qty} item #{template_id} (offline) cho tài khoản #{account_id}."))
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
        format!("{} (đã thông báo tới {sent} người chơi)", if enabled { "Đã BẬT bảo trì" } else { "Đã TẮT bảo trì" })
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

    /// Signals the background server runtime to flush all players and shut down cleanly.
    pub fn trigger_shutdown(&self) {
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
    online: bool,
    ban: bool,
    active: bool,
    is_admin: bool,
    vnd: i32,
    fallback_gold: i64,
) -> AdminUserRow {
    let (player_id, player_name, map_id, zone_id, power, gold, gem, ruby) = match snap {
        Some(s) => (Some(s.id.0), s.name, s.location.map_id.0, s.location.zone_id.0, s.stats.power, s.wallet.gold, s.wallet.gem, s.wallet.ruby),
        None => (None, "(Chưa tạo NV)".to_owned(), 0, 0, 0, fallback_gold, 0, 0),
    };
    AdminUserRow { account_id, username, player_id, player_name, online, ban, active, is_admin, map_id, zone_id, power, gold, gem, ruby, vnd }
}

fn apply_wallet_deltas(snap: &mut PlayerSnapshot, gold_delta: i64, gem_delta: i32, ruby_delta: i32) {
    snap.wallet.gold = snap.wallet.gold.saturating_add(gold_delta).clamp(0, 2_000_000_000_000);
    snap.wallet.gem = snap.wallet.gem.saturating_add(gem_delta).max(0);
    snap.wallet.ruby = snap.wallet.ruby.saturating_add(ruby_delta).max(0);
}
