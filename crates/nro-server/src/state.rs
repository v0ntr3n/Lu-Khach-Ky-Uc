//! Concurrent server registries (`LeaderboardCache`, `IpRateLimiter`, `ClientRegistry`,
//! and `MaintenanceCoordinator`) replacing mutable statics in `ServerManager.java` and `Client.java`.

use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{AccountId, EntityKey, PlayerId, SessionKey};
use nro_db::player_repo::PlayerSnapshot;
use nro_protocol::{Packet, PacketWriter, SessionHandle};
use tokio::sync::RwLock;

/// Entry in one of the four server leaderboards (`TopSM`, `TopNV`, `TopSukien1`, `TopSukien2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderboardRow {
    /// 1-based leaderboard rank.
    pub rank: u32,
    /// Character display name.
    pub player_name: String,
    /// Ranked metric (`power`, `task_id`, `point_sukien1`, or `point_sukien2`).
    pub score: i64,
}

/// Distinct leaderboard slots managed by [`LeaderboardCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardSlot {
    /// Power ranking (`TopSM`).
    Power,
    /// Main task progression ranking (`TopNV`).
    Task,
    /// Primary seasonal event ranking (`TopSukien1`).
    SuKien1,
    /// Secondary seasonal event ranking (`TopSukien2` — isolated from `TopSukien1`!).
    SuKien2,
}

/// Dirty-flag leaderboard cache refreshed every `3000 ms`, fixing Java's `TopSukien2 -> TopSukien1` overwrite bug.
#[derive(Debug, Default)]
pub struct LeaderboardCache {
    top_power: RwLock<Vec<LeaderboardRow>>,
    top_task: RwLock<Vec<LeaderboardRow>>,
    top_sukien_1: RwLock<Vec<LeaderboardRow>>,
    top_sukien_2: RwLock<Vec<LeaderboardRow>>,
    dirty: [AtomicBool; 4],
}

impl LeaderboardCache {
    /// Marks all four leaderboards dirty on startup.
    pub fn mark_all_dirty(&self) {
        for flag in &self.dirty {
            flag.store(true, Ordering::Release);
        }
    }

    /// Marks a single [`LeaderboardSlot`] dirty.
    pub fn mark_dirty(&self, slot: LeaderboardSlot) {
        self.dirty[slot as usize].store(true, Ordering::Release);
    }

    /// Reads a snapshot of `slot`.
    pub async fn snapshot(&self, slot: LeaderboardSlot) -> Vec<LeaderboardRow> {
        match slot {
            LeaderboardSlot::Power => self.top_power.read().await.clone(),
            LeaderboardSlot::Task => self.top_task.read().await.clone(),
            LeaderboardSlot::SuKien1 => self.top_sukien_1.read().await.clone(),
            LeaderboardSlot::SuKien2 => self.top_sukien_2.read().await.clone(),
        }
    }

    /// Refreshes only dirty leaderboards via `fetcher`, keeping `SuKien1` and `SuKien2` strictly isolated.
    pub async fn refresh_dirty_with<F, Fut>(&self, mut fetcher: F) -> anyhow::Result<u8>
    where
        F: FnMut(LeaderboardSlot) -> Fut,
        Fut: Future<Output = anyhow::Result<Vec<LeaderboardRow>>>,
    {
        let mut count = 0_u8;
        for (idx, (slot, target)) in [
            (LeaderboardSlot::Power, &self.top_power),
            (LeaderboardSlot::Task, &self.top_task),
            (LeaderboardSlot::SuKien1, &self.top_sukien_1),
            (LeaderboardSlot::SuKien2, &self.top_sukien_2),
        ]
        .into_iter()
        .enumerate()
        {
            if self.dirty[idx].swap(false, Ordering::AcqRel) {
                match fetcher(slot).await {
                    Ok(rows) => {
                        *target.write().await = rows;
                        count = count.saturating_add(1);
                    }
                    Err(err) => {
                        self.dirty[idx].store(true, Ordering::Release);
                        return Err(err);
                    }
                }
            }
        }
        Ok(count)
    }
}

const ACCOUNT_CREATION_WINDOW: Duration = Duration::from_secs(60);
const MAX_ACCOUNT_CREATIONS_PER_WINDOW: usize = 5;
const ASSET_BURST_WINDOW: Duration = Duration::from_secs(10);
const MAX_ASSET_BURST_PER_WINDOW: usize = 30;
const MAX_ASSET_BURST_TRACKED_IPS: usize = 4_096;
const MAX_TRACKED_IPS: usize = 16_384;

/// Per-IP connection counter and login cooldown guard replacing `ServerManager.CLIENTS`.
#[derive(Debug)]
pub struct IpRateLimiter {
    buckets: DashMap<IpAddr, (usize, Option<Instant>)>,
    account_creations: DashMap<IpAddr, Vec<Instant>>,
    asset_bursts: DashMap<IpAddr, (Instant, u16)>,
    max_per_ip: usize,
    login_cooldown: Duration,
}

impl IpRateLimiter {
    /// Creates a new [`IpRateLimiter`].
    #[must_use]
    pub fn new(max_per_ip: usize, login_cooldown_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            buckets: DashMap::new(),
            account_creations: DashMap::new(),
            asset_bursts: DashMap::new(),
            max_per_ip,
            login_cooldown: Duration::from_millis(login_cooldown_ms),
        })
    }

    /// Increments the active connection count for `ip`, returning `false` if capped.
    pub fn try_acquire(&self, ip: IpAddr) -> bool {
        if self.buckets.len() >= MAX_TRACKED_IPS && !self.buckets.contains_key(&ip) {
            self.prune_idle();
            if self.buckets.len() >= MAX_TRACKED_IPS {
                return false;
            }
        }
        let mut entry = self.buckets.entry(ip).or_insert((0, None));
        if entry.0 >= self.max_per_ip {
            return false;
        }
        entry.0 += 1;
        true
    }

    /// Releases one active connection slot for `ip` and prunes idle entries once cooldown elapses.
    pub fn release(&self, ip: IpAddr) {
        let is_idle = self.buckets.get_mut(&ip).is_some_and(|mut entry| {
            entry.0 = entry.0.saturating_sub(1);
            entry.0 == 0
        });
        if is_idle {
            let now = Instant::now();
            self.buckets.remove_if(&ip, |_, (count, last_login)| {
                *count == 0
                    && last_login
                        .is_none_or(|prev| now.saturating_duration_since(prev) >= self.login_cooldown)
            });
        }
    }

    /// Enforces per-IP login burst cooldown (loopback is exempt for local multi-step handshakes).
    pub fn check_login_cooldown(&self, ip: IpAddr) -> Result<(), Duration> {
        if ip.is_loopback() {
            return Ok(());
        }
        if self.buckets.len() >= MAX_TRACKED_IPS && !self.buckets.contains_key(&ip) {
            self.prune_idle();
            if self.buckets.len() >= MAX_TRACKED_IPS {
                return Err(Duration::from_secs(1));
            }
        }
        let now = Instant::now();
        let mut entry = self.buckets.entry(ip).or_insert((0, None));
        if let Some(prev) = entry.1 {
            let elapsed = now.saturating_duration_since(prev);
            if elapsed < self.login_cooldown {
                return Err(self.login_cooldown.saturating_sub(elapsed));
            }
        }
        entry.1 = Some(now);
        Ok(())
    }

    /// Enforces per-IP account/guest creation quota (5 per 60s; never cleared by `clear_login_cooldown`).
    pub fn check_account_creation_limit(&self, ip: IpAddr) -> Result<(), Duration> {
        if ip.is_loopback() {
            return Ok(());
        }
        if self.account_creations.len() >= MAX_TRACKED_IPS && !self.account_creations.contains_key(&ip) {
            self.prune_idle();
            if self.account_creations.len() >= MAX_TRACKED_IPS {
                return Err(Duration::from_secs(1));
            }
        }
        let now = Instant::now();
        let mut entry = self.account_creations.entry(ip).or_default();
        entry.retain(|&ts| now.saturating_duration_since(ts) < ACCOUNT_CREATION_WINDOW);
        if entry.len() >= MAX_ACCOUNT_CREATIONS_PER_WINDOW {
            let wait = ACCOUNT_CREATION_WINDOW.saturating_sub(now.saturating_duration_since(entry[0]));
            return Err(wait.max(Duration::from_secs(1)));
        }
        entry.push(now);
        Ok(())
    }

    /// Clears the login burst cooldown for `ip` (superseded by per-session bypass on `SessionHandle`).
    #[allow(dead_code)]
    pub fn clear_login_cooldown(&self, ip: IpAddr) {
        if let Some(mut entry) = self.buckets.get_mut(&ip) {
            entry.1 = None;
        }
    }

    /// Enforces a bounded per-IP bulk/asset request burst limit (max 30 requests / 10s, loopback exempt).
    pub fn check_asset_burst(&self, ip: IpAddr) -> bool {
        if ip.is_loopback() {
            return true;
        }
        let now = Instant::now();
        if self.asset_bursts.len() >= MAX_ASSET_BURST_TRACKED_IPS && !self.asset_bursts.contains_key(&ip) {
            self.asset_bursts.retain(|_, (start, _)| now.saturating_duration_since(*start) < ASSET_BURST_WINDOW);
            if self.asset_bursts.len() >= MAX_ASSET_BURST_TRACKED_IPS {
                return false;
            }
        }
        let mut entry = self.asset_bursts.entry(ip).or_insert((now, 0));
        if now.saturating_duration_since(entry.0) >= ASSET_BURST_WINDOW {
            *entry = (now, 1);
            return true;
        }
        if usize::from(entry.1) >= MAX_ASSET_BURST_PER_WINDOW {
            return false;
        }
        entry.1 = entry.1.saturating_add(1);
        true
    }

    /// Prunes idle per-IP buckets whose connection count is zero and login cooldown has elapsed.
    pub fn prune_idle(&self) {
        let now = Instant::now();
        self.buckets.retain(|_, (count, last)| {
            *count > 0
                || last.is_some_and(|prev| now.saturating_duration_since(prev) < self.login_cooldown)
        });
        self.account_creations.retain(|_, timestamps| {
            timestamps.retain(|&ts| now.saturating_duration_since(ts) < ACCOUNT_CREATION_WINDOW);
            !timestamps.is_empty()
        });
        self.asset_bursts.retain(|_, (start, _)| now.saturating_duration_since(*start) < ASSET_BURST_WINDOW);
    }
}

/// Active authenticated player state tracked per connected session.
#[derive(Debug, Clone)]
pub struct ActivePlayerEntry {
    /// Connected session handle.
    pub handle: SessionHandle,
    /// Generational session key inside `ZoneActor`.
    pub session_key: SessionKey,
    /// Active zone entity key, if spawned in-world.
    pub entity_key: Option<EntityKey>,
    /// Database snapshot updated across gameplay actions for crash-safe persistence.
    pub snapshot: PlayerSnapshot,
    /// Whether account has admin privileges.
    pub is_admin: bool,
    /// Whether account is activated (`account.active != 0`).
    pub activated: bool,
    /// Currently selected skill template ID (`SKILL_SELECT`).
    pub selected_skill: i16,
    /// Timestamp (`ms`) of last Magic Pea consumption.
    pub last_pea_ms: i64,
    /// Current PK flag color (`CHANGE_FLAG`).
    pub c_flag: i8,
}

/// Concurrent session & player registry replacing `Client.java` and `ServerNotify.java`.
#[derive(Debug, Default)]
pub struct ClientRegistry {
    /// Connected sessions keyed by `SessionHandle::id()`.
    pub sessions: DashMap<u64, SessionHandle>,
    /// Authenticated players keyed by `SessionHandle::id()`.
    pub authenticated: DashMap<u64, ActivePlayerEntry>,
    /// Account-to-session index preventing dual-login dupes.
    pub accounts: DashMap<AccountId, u64>,
    /// Player-to-session index for whispers, trades, and kicks.
    pub players: DashMap<PlayerId, u64>,
}

impl ClientRegistry {
    /// Evicts any existing session on `account_id` and binds `entry`, returning the evicted entry if any.
    pub fn bind_authenticated(
        &self,
        session_id: u64,
        account_id: AccountId,
        entry: ActivePlayerEntry,
    ) -> Option<ActivePlayerEntry> {
        let mut evicted = None;
        if let Some(old_sid) = self.accounts.insert(account_id, session_id)
            && old_sid != session_id
        {
            evicted = self.authenticated.remove(&old_sid).map(|(_, e)| e);
            if let Some(old_handle) = self.sessions.get(&old_sid) {
                old_handle.disconnect();
            }
        }
        self.players.insert(entry.snapshot.id, session_id);
        self.authenticated.insert(session_id, entry);
        evicted
    }

    /// Broadcasts a pre-built [`Packet`] to all authenticated, post-handshake online players.
    #[allow(clippy::needless_pass_by_value)]
    pub fn broadcast_packet(&self, pkt: Packet) -> usize {
        let mut sent = 0_usize;
        for rec in &self.authenticated {
            let handle = &rec.value().handle;
            if handle.sent_key() && handle.try_send(pkt.clone()).is_ok() {
                sent += 1;
            }
        }
        sent
    }

    /// Broadcasts a scrolling `ServerNotify` (`cmd 94` / `SERVER_ALERT`) packet to all online players.
    pub fn broadcast_server_notify(&self, alert: &str) -> usize {
        let mut pw = PacketWriter::with_capacity(alert.len() + 4);
        if pw.write_utf(alert).is_err() {
            return 0;
        }
        self.broadcast_packet(pw.finish(cmd_message::SERVER_ALERT))
    }

    /// Kicks an online player by character name or numeric `PlayerId`.
    pub fn kick_by_target(&self, target: &str) -> bool {
        let needle = target.trim();
        let found = self
            .authenticated
            .iter()
            .find(|e| e.value().snapshot.name.eq_ignore_ascii_case(needle) || e.value().snapshot.id.0.to_string() == needle)
            .map(|e| *e.key());
        if let Some(sid) = found
            && let Some(h) = self.sessions.get(&sid)
        {
            h.disconnect();
            return true;
        }
        false
    }
}

/// Single-latch maintenance state machine fixing `ServerManager.java`'s duplicate `AutoMaintenance.start()`.
#[derive(Debug, Default)]
pub struct MaintenanceCoordinator {
    maintaining: AtomicBool,
    auto_scheduler_claimed: AtomicBool,
}

impl MaintenanceCoordinator {
    /// Returns `true` once maintenance countdown has locked logins.
    #[must_use]
    pub fn is_maintaining(&self) -> bool { self.maintaining.load(Ordering::Acquire) }
    /// Claims the single `AutoMaintenance` scheduler slot.
    #[must_use]
    pub fn try_claim_auto_scheduler(&self) -> bool {
        self.auto_scheduler_claimed.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
    /// Transitions into maintenance mode at most once.
    #[must_use]
    pub fn try_begin_maintenance(&self) -> bool {
        self.maintaining.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
    /// Explicitly toggles maintenance mode from the admin control panel.
    pub fn set_maintaining(&self, enabled: bool) {
        self.maintaining.store(enabled, Ordering::SeqCst);
    }
}
