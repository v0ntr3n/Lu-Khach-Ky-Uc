//! Non-blocking split Tokio TCP session driver (`Session.java`, `MySession.java`,
//! `Collector.java`, `Sender.java`, `MyKeyHandler.java`) with bounded `mpsc` backpressure.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, AtomicU16, Ordering};
use std::time::Duration;

use bytes::BytesMut;
use nro_core::{AccountId, PlayerId};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace, warn};

use crate::ProtocolError;
use crate::c2s::ClientPacket;
use crate::cipher::{DEFAULT_SESSION_KEY, HANDSHAKE_CMD, XorReadHalf, XorSessionCipher, XorWriteHalf};
use crate::frame::{Packet, PacketWriter, decode_c2s_frame, encode_s2c_frame};

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
    fn on_packet(&self, session: &SessionHandle, packet: ClientPacket) -> impl Future<Output = Result<(), ProtocolError>> + Send;
    /// Invoked **at most once** when the session terminates for any reason.
    fn on_disconnect(&self, session: &SessionHandle) -> impl Future<Output = ()> + Send;
}

#[derive(Debug)]
enum OutboundCommand {
    HandshakeAndVersionRes { vs_res: i32 },
    Packet(Packet),
}

#[derive(Debug)]
struct SessionShared {
    id: u64,
    peer_addr: SocketAddr,
    connected: AtomicBool,
    sent_key: AtomicBool,
    finish_update: AtomicBool,
    res_streamed: AtomicBool,
    client_ok_sent: AtomicBool,
    client_type_set: AtomicBool,
    auth_cooldown_bypass: AtomicBool,
    client_version: AtomicI32,
    zoom_level: AtomicU8,
    time_wait: AtomicU16,
    account_id: AtomicI32,
    player_id: AtomicI64,
    cancel: CancellationToken,
}

/// Cheaply cloneable, thread-safe handle to an active Tokio client session (`MySession` state).
#[derive(Debug, Clone)]
pub struct SessionHandle {
    shared: Arc<SessionShared>,
    outbound_tx: mpsc::Sender<OutboundCommand>,
}

impl SessionHandle {
    /// Returns the unique monotonic session ID (`Session.getID()`).
    #[inline] #[must_use] pub fn id(&self) -> u64 { self.shared.id }
    /// Returns the remote peer socket address.
    #[inline] #[must_use] pub fn peer_addr(&self) -> SocketAddr { self.shared.peer_addr }
    /// Returns the remote peer IP address (`Session.getIP()`).
    #[inline] #[must_use] pub fn ip(&self) -> IpAddr { self.shared.peer_addr.ip() }
    /// Returns `true` while the session remains open (`Session.isConnected()`).
    #[inline] #[must_use] pub fn is_connected(&self) -> bool { self.shared.connected.load(Ordering::Acquire) }
    /// Returns `true` once the `-27` key exchange has completed (`Session.sentKey()`).
    #[inline] #[must_use] pub fn sent_key(&self) -> bool { self.shared.sent_key.load(Ordering::Acquire) }
    /// Returns the numeric client version parsed from `-29` sub `2` (e.g. `231`).
    #[inline] #[must_use] pub fn client_version(&self) -> i32 { self.shared.client_version.load(Ordering::Acquire) }
    /// Returns the client sprite zoom level (`1..=4`).
    #[inline] #[must_use] pub fn zoom_level(&self) -> u8 { self.shared.zoom_level.load(Ordering::Acquire) }
    /// Returns the authenticated `AccountId` (`0` if unauthenticated).
    #[inline] #[must_use] pub fn account_id(&self) -> AccountId { AccountId(self.shared.account_id.load(Ordering::Acquire)) }
    /// Returns the active `PlayerId` (`0` if not yet in-world).
    #[inline] #[must_use] pub fn player_id(&self) -> PlayerId { PlayerId(self.shared.player_id.load(Ordering::Acquire)) }
    /// Marks `-38` (`FINISH_UPDATE`) complete on this session.
    #[inline] pub fn set_finish_update(&self, done: bool) { self.shared.finish_update.store(done, Ordering::Release); }
    /// Atomically claims one-shot `-74` resource streaming on this session.
    #[inline] #[must_use] pub fn try_mark_res_streamed(&self) -> bool { !self.shared.res_streamed.swap(true, Ordering::AcqRel) }
    /// Atomically claims one-shot in-world `ClientOk` (`-28` sub `13`) initialization on this session.
    #[inline] #[must_use] pub fn try_mark_client_ok_sent(&self) -> bool { !self.shared.client_ok_sent.swap(true, Ordering::AcqRel) }
    /// Grants a one-shot login cooldown bypass after registration or guest creation.
    #[inline] pub fn grant_auth_cooldown_bypass(&self) { self.shared.auth_cooldown_bypass.store(true, Ordering::Release); }
    /// Consumes a pending one-shot login cooldown bypass on this session.
    #[inline] #[must_use] pub fn take_auth_cooldown_bypass(&self) -> bool { self.shared.auth_cooldown_bypass.swap(false, Ordering::AcqRel) }
    /// Returns a clone of the session's cancellation token for cooperative task shutdown.
    #[inline] #[must_use] pub fn cancel_token(&self) -> CancellationToken { self.shared.cancel.clone() }

    /// Marks the session as authenticated and records `account_id` + `player_id`.
    /// Clears `timeWait` once in-world (`player_id != 0`), or grants a 300s grace window for char creation.
    pub fn mark_authenticated(&self, account_id: AccountId, player_id: PlayerId) {
        let grace_ticks = if player_id.0 == 0 { 300 } else { 0 };
        self.shared.time_wait.store(grace_ticks, Ordering::Release);
        self.shared.account_id.store(account_id.0, Ordering::Release);
        self.shared.player_id.store(player_id.0, Ordering::Release);
    }

    /// Decrements `timeWait` by 1 tick (`Client.update`, 1 Hz). Disconnects and returns `true` on expiry.
    pub fn tick_login_timeout(&self) -> bool {
        let prev = self.shared.time_wait.try_update(Ordering::AcqRel, Ordering::Acquire, |v| (v > 0).then(|| v - 1));
        if prev == Ok(1) {
            self.disconnect();
            return true;
        }
        false
    }

    /// Enqueues an outbound [`Packet`], awaiting bounded channel capacity (`backpressure`).
    pub async fn send(&self, packet: Packet) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::SessionClosed);
        }
        self.outbound_tx.send(OutboundCommand::Packet(packet)).await.map_err(|_| ProtocolError::SessionClosed)
    }

    /// Attempts to enqueue an outbound [`Packet`] without blocking zone/world broadcast loops.
    pub fn try_send(&self, packet: Packet) -> Result<(), ProtocolError> {
        if !self.is_connected() {
            return Err(ProtocolError::SessionClosed);
        }
        self.outbound_tx.try_send(OutboundCommand::Packet(packet)).map_err(|err| match err {
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

/// Configures `TCP_NODELAY = true` (`Network.java:129`) and spawns split Tokio session tasks.
pub fn spawn_tcp_session<H: SessionEventHandler>(
    id: u64, stream: TcpStream, peer_addr: SocketAddr, config: SessionConfig, handler: Arc<H>,
) -> Result<(SessionHandle, JoinHandle<()>), ProtocolError> {
    let _ = stream.set_nodelay(true);
    drive_session_stream(id, stream, peer_addr, config, handler)
}

/// Splits any `AsyncRead + AsyncWrite` transport into bounded reader and writer Tokio tasks.
pub fn drive_session_stream<S, H>(
    id: u64, stream: S, peer_addr: SocketAddr, config: SessionConfig, handler: Arc<H>,
) -> Result<(SessionHandle, JoinHandle<()>), ProtocolError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: SessionEventHandler,
{
    let (read_cipher, write_cipher) = XorSessionCipher::new(&config.session_key)?.split();
    let (outbound_tx, outbound_rx) = mpsc::channel(config.outbound_capacity.max(1));
    let shared = Arc::new(SessionShared {
        id, peer_addr,
        connected: AtomicBool::new(true),
        sent_key: AtomicBool::new(false),
        finish_update: AtomicBool::new(false),
        res_streamed: AtomicBool::new(false),
        client_ok_sent: AtomicBool::new(false),
        client_type_set: AtomicBool::new(false),
        auth_cooldown_bypass: AtomicBool::new(false),
        client_version: AtomicI32::new(0),
        zoom_level: AtomicU8::new(2),
        time_wait: AtomicU16::new(config.login_timeout_ticks),
        account_id: AtomicI32::new(0),
        player_id: AtomicI64::new(0),
        cancel: CancellationToken::new(),
    });
    let handle = SessionHandle { shared, outbound_tx };
    let sup = handle.clone();
    let join = tokio::spawn(async move {
        let (reader, writer) = tokio::io::split(stream);
        let w_cancel = sup.shared.cancel.clone();
        let w_task = tokio::spawn(run_writer_loop(writer, write_cipher, outbound_rx, w_cancel, config.write_timeout));
        if let Err(err) = run_reader_loop(reader, read_cipher, &sup, &config, handler.as_ref()).await {
            debug!(session_id = id, %peer_addr, error = %err, "session reader terminated");
        }
        sup.disconnect();
        let _ = w_task.await;
        handler.on_disconnect(&sup).await;
    });
    Ok((handle, join))
}

async fn run_reader_loop<R: AsyncRead + Unpin, H: SessionEventHandler>(
    mut reader: R, mut cipher: XorReadHalf, handle: &SessionHandle, config: &SessionConfig, handler: &H,
) -> Result<(), ProtocolError> {
    let mut buf = BytesMut::with_capacity(config.read_buffer_capacity);
    loop {
        while let Some(raw) = decode_c2s_frame(&mut buf, &mut cipher)? {
            trace!(session_id = handle.id(), cmd = raw.cmd, len = raw.payload.len(), "c2s frame");
            if !cipher.sent_key() {
                if raw.cmd == HANDSHAKE_CMD {
                    cipher.set_sent_key(true);
                    handle.shared.sent_key.store(true, Ordering::Release);
                    let msg = OutboundCommand::HandshakeAndVersionRes { vs_res: config.initial_vs_res };
                    handle.outbound_tx.send(msg).await.map_err(|_| ProtocolError::SessionClosed)?;
                } else {
                    debug!(session_id = handle.id(), cmd = raw.cmd, "dropping pre-handshake packet");
                }
                continue;
            }
            let pkt = ClientPacket::decode(raw.cmd, &mut raw.reader(), handle.client_version())?;
            if let ClientPacket::SetClientType(info) = &pkt {
                if !handle.shared.client_type_set.swap(true, Ordering::AcqRel) {
                    handle.shared.zoom_level.store(info.zoom_level, Ordering::Release);
                    handle.shared.client_version.store(info.parsed_version, Ordering::Release);
                }
            } else if matches!(pkt, ClientPacket::FinishUpdate) {
                handle.set_finish_update(true);
            }
            handler.on_packet(handle, pkt).await?;
        }
        if buf.is_empty() && buf.capacity() > 64 * 1024 {
            buf = BytesMut::with_capacity(8 * 1024);
        }
        let n = tokio::select! {
            biased;
            () = handle.shared.cancel.cancelled() => return Ok(()),
            res = reader.read_buf(&mut buf) => res?,
        };
        if n == 0 { return Ok(()); }
    }
}

async fn run_writer_loop<W: AsyncWrite + Unpin>(
    mut writer: W, mut cipher: XorWriteHalf, mut rx: mpsc::Receiver<OutboundCommand>, cancel: CancellationToken, timeout: Duration,
) {
    let mut wire = BytesMut::with_capacity(4096);
    'writer: loop {
        let maybe_cmd = tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            cmd = rx.recv() => cmd,
        };
        let Some(first) = maybe_cmd else { break };
        wire.clear();
        if wire.capacity() > 64 * 1024 {
            wire = BytesMut::with_capacity(4096);
        }
        if let Err(err) = encode_outbound_cmd(first, &mut cipher, &mut wire) {
            warn!(error = %err, "failed encoding outbound frame");
            cancel.cancel();
            break;
        }
        while let Ok(next) = rx.try_recv() {
            if wire.len() >= 64 * 1024 {
                let flushed = tokio::select! {
                    biased;
                    () = cancel.cancelled() => break 'writer,
                    res = flush_with_timeout(&mut writer, &wire, timeout) => res.is_ok(),
                };
                if !flushed {
                    cancel.cancel();
                    break 'writer;
                }
                wire.clear();
                if wire.capacity() > 64 * 1024 {
                    wire = BytesMut::with_capacity(4096);
                }
            }
            if let Err(err) = encode_outbound_cmd(next, &mut cipher, &mut wire) {
                warn!(error = %err, "failed encoding coalesced frame");
                cancel.cancel();
                break 'writer;
            }
        }
        let flushed = tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            res = flush_with_timeout(&mut writer, &wire, timeout) => res.is_ok(),
        };
        if !flushed {
            cancel.cancel();
            break;
        }
        wire.clear();
        if wire.capacity() > 64 * 1024 {
            wire = BytesMut::with_capacity(4096);
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), writer.shutdown()).await;
}

fn encode_outbound_cmd(cmd: OutboundCommand, cipher: &mut XorWriteHalf, wire: &mut BytesMut) -> Result<(), ProtocolError> {
    match cmd {
        OutboundCommand::HandshakeAndVersionRes { vs_res } => {
            cipher.write_handshake_frame(wire)?;
            let mut pw = PacketWriter::with_capacity(5);
            pw.write_i8(0);
            pw.write_i32(vs_res);
            encode_s2c_frame(&pw.finish(-74), cipher, wire)?;
        }
        OutboundCommand::Packet(pkt) => encode_s2c_frame(&pkt, cipher, wire)?,
    }
    Ok(())
}

async fn flush_with_timeout<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8], timeout: Duration) -> Result<(), ProtocolError> {
    tokio::time::timeout(timeout, async { writer.write_all(bytes).await?; writer.flush().await })
        .await
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
    Ok(())
}
