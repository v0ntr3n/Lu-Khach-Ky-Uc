//! Non-blocking split Tokio TCP session driver (`Session.java`, `MySession.java`,
//! `Collector.java`, `Sender.java`, `MyKeyHandler.java`) with bounded `mpsc` backpressure.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, AtomicU16, Ordering};
use std::time::Duration;

use nro_core::{AccountId, PlayerId};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::ProtocolError;
use crate::c2s::ClientPacket;
use crate::cipher::DEFAULT_SESSION_KEY;
use crate::frame::Packet;

#[path = "session_io.rs"]
mod session_io;
pub use session_io::{drive_session_stream, spawn_tcp_session};

/// Per-session tuning parameters for framing, key exchange, timeouts, and backpressure.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Bounded outbound `mpsc` queue depth (replaces unbounded `Sender` deque).
    pub outbound_capacity: usize,
    /// Initial socket read buffer capacity in bytes.
    pub read_buffer_capacity: usize,
    /// Unauthenticated idle countdown ticks (`MySession.timeWait = 100` @ 1 Hz = 100s).
    pub login_timeout_ticks: u16,
    /// Session XOR key (`b"NRO"` by default, or 7-byte random alphanumeric key).
    pub session_key: Vec<u8>,
    /// Resource version (`DataGame.vsRes = 1`) sent via `-74` sub `0` right after `-27`.
    pub initial_vs_res: i32,
    /// Per-write socket flush timeout protecting bounded queues against dead peers.
    pub write_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            outbound_capacity: 256,
            read_buffer_capacity: 8 * 1024,
            login_timeout_ticks: 100,
            session_key: DEFAULT_SESSION_KEY.to_vec(),
            initial_vs_res: 1,
            write_timeout: Duration::from_secs(15),
        }
    }
}

/// Asynchronous application-layer callback trait replacing `IMessageHandler` and `ISessionAcceptHandler`.
pub trait SessionEventHandler: Send + Sync + 'static {
    /// Invoked sequentially for every decoded C2S packet after the `-27` handshake.
    fn on_packet(
        &self,
        session: &SessionHandle,
        packet: ClientPacket,
    ) -> impl Future<Output = Result<(), ProtocolError>> + Send;
    /// Invoked **at most once** when the session terminates for any reason.
    fn on_disconnect(&self, session: &SessionHandle) -> impl Future<Output = ()> + Send;
}

#[derive(Debug)]
pub(super) enum OutboundCommand {
    HandshakeAndVersionRes { vs_res: i32 },
    Packet(Packet),
}

#[derive(Debug)]
pub(super) struct SessionShared {
    pub(super) id: u64,
    pub(super) peer_addr: SocketAddr,
    pub(super) connected: AtomicBool,
    pub(super) sent_key: AtomicBool,
    pub(super) finish_update: AtomicBool,
    pub(super) res_streamed: AtomicBool,
    pub(super) client_ok_sent: AtomicBool,
    pub(super) client_type_set: AtomicBool,
    pub(super) auth_cooldown_bypass: AtomicBool,
    pub(super) client_version: AtomicI32,
    pub(super) zoom_level: AtomicU8,
    pub(super) time_wait: AtomicU16,
    pub(super) account_id: AtomicI32,
    pub(super) player_id: AtomicI64,
    pub(super) cancel: CancellationToken,
}

/// Cheaply cloneable, thread-safe handle to an active Tokio client session (`MySession` state).
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub(super) shared: Arc<SessionShared>,
    pub(super) outbound_tx: mpsc::Sender<OutboundCommand>,
}

impl SessionHandle {
    /// Returns the unique monotonic session ID (`Session.getID()`).
    #[inline]
    #[must_use]
    pub fn id(&self) -> u64 {
        self.shared.id
    }
    /// Returns the remote peer socket address.
    #[inline]
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.shared.peer_addr
    }
    /// Returns the remote peer IP address (`Session.getIP()`).
    #[inline]
    #[must_use]
    pub fn ip(&self) -> IpAddr {
        self.shared.peer_addr.ip()
    }
    /// Returns `true` while the session remains open (`Session.isConnected()`).
    #[inline]
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::Acquire)
    }
    /// Returns `true` once the `-27` key exchange has completed (`Session.sentKey()`).
    #[inline]
    #[must_use]
    pub fn sent_key(&self) -> bool {
        self.shared.sent_key.load(Ordering::Acquire)
    }
    /// Returns the numeric client version parsed from `-29` sub `2` (e.g. `231`).
    #[inline]
    #[must_use]
    pub fn client_version(&self) -> i32 {
        self.shared.client_version.load(Ordering::Acquire)
    }
    /// Returns the client sprite zoom level (`1..=4`).
    #[inline]
    #[must_use]
    pub fn zoom_level(&self) -> u8 {
        self.shared.zoom_level.load(Ordering::Acquire)
    }
    /// Returns the authenticated `AccountId` (`0` if unauthenticated).
    #[inline]
    #[must_use]
    pub fn account_id(&self) -> AccountId {
        AccountId(self.shared.account_id.load(Ordering::Acquire))
    }
    /// Returns the active `PlayerId` (`0` if not yet in-world).
    #[inline]
    #[must_use]
    pub fn player_id(&self) -> PlayerId {
        PlayerId(self.shared.player_id.load(Ordering::Acquire))
    }
    /// Marks `-38` (`FINISH_UPDATE`) complete on this session.
    #[inline]
    pub fn set_finish_update(&self, done: bool) {
        self.shared.finish_update.store(done, Ordering::Release);
    }
    /// Atomically claims one-shot `-74` resource streaming on this session.
    #[inline]
    #[must_use]
    pub fn try_mark_res_streamed(&self) -> bool {
        !self.shared.res_streamed.swap(true, Ordering::AcqRel)
    }
    /// Atomically claims one-shot in-world `ClientOk` (`-28` sub `13`) initialization on this session.
    #[inline]
    #[must_use]
    pub fn try_mark_client_ok_sent(&self) -> bool {
        !self.shared.client_ok_sent.swap(true, Ordering::AcqRel)
    }
    /// Grants a one-shot login cooldown bypass after registration or guest creation.
    #[inline]
    pub fn grant_auth_cooldown_bypass(&self) {
        self.shared
            .auth_cooldown_bypass
            .store(true, Ordering::Release);
    }
    /// Consumes a pending one-shot login cooldown bypass on this session.
    #[inline]
    #[must_use]
    pub fn take_auth_cooldown_bypass(&self) -> bool {
        self.shared
            .auth_cooldown_bypass
            .swap(false, Ordering::AcqRel)
    }
    /// Returns a clone of the session's cancellation token for cooperative task shutdown.
    #[inline]
    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.shared.cancel.clone()
    }

    /// Marks the session as authenticated and records `account_id` + `player_id`.
    /// Clears `timeWait` once in-world (`player_id != 0`), or grants a 300s grace window for char creation.
    pub fn mark_authenticated(&self, account_id: AccountId, player_id: PlayerId) {
        let grace_ticks = if player_id.0 == 0 { 300 } else { 0 };
        self.shared.time_wait.store(grace_ticks, Ordering::Release);
        self.shared
            .account_id
            .store(account_id.0, Ordering::Release);
        self.shared.player_id.store(player_id.0, Ordering::Release);
    }

    /// Decrements `timeWait` by 1 tick (`Client.update`, 1 Hz). Disconnects and returns `true` on expiry.
    pub fn tick_login_timeout(&self) -> bool {
        let prev = self
            .shared
            .time_wait
            .try_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                (v > 0).then(|| v - 1)
            });
        if prev == Ok(1) {
            self.disconnect();
            return true;
        }
        false
    }

    /// Enqueues an outbound [`Packet`], awaiting bounded channel capacity (`backpressure`) or cancellation.
    pub async fn send(&self, packet: Packet) -> Result<(), ProtocolError> {
        if !self.is_connected() || self.shared.cancel.is_cancelled() {
            return Err(ProtocolError::SessionClosed);
        }
        tokio::select! {
            biased;
            () = self.shared.cancel.cancelled() => Err(ProtocolError::SessionClosed),
            res = self.outbound_tx.send(OutboundCommand::Packet(packet)) => {
                res.map_err(|_| ProtocolError::SessionClosed)
            }
        }
    }

    /// Attempts to enqueue an outbound [`Packet`] without blocking zone/world broadcast loops.
    pub fn try_send(&self, packet: Packet) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::SessionClosed);
        }
        self.outbound_tx
            .try_send(OutboundCommand::Packet(packet))
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => ProtocolError::OutboundQueueFull,
                mpsc::error::TrySendError::Closed(_) => ProtocolError::SessionClosed,
            })
    }

    /// Initiates idempotent, non-blocking session shutdown (`Session.disconnect()`).
    pub fn disconnect(&self) {
        if self.shared.connected.swap(false, Ordering::AcqRel) {
            self.shared.cancel.cancel();
        }
    }
}
