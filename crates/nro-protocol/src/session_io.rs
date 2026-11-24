//! Split Tokio reader and writer transport loops for [`SessionHandle`].

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, AtomicU16, Ordering};
use std::time::Duration;

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace, warn};

use super::{OutboundCommand, SessionConfig, SessionEventHandler, SessionHandle, SessionShared};
use crate::ProtocolError;
use crate::c2s::ClientPacket;
use crate::cipher::{HANDSHAKE_CMD, XorReadHalf, XorSessionCipher, XorWriteHalf};
use crate::frame::{PacketWriter, decode_c2s_frame, encode_s2c_frame};

/// Configures `TCP_NODELAY = true` (`Network.java:129`) and spawns split Tokio session tasks.
pub fn spawn_tcp_session<H: SessionEventHandler>(
    id: u64,
    stream: TcpStream,
    peer_addr: SocketAddr,
    config: SessionConfig,
    handler: Arc<H>,
) -> Result<(SessionHandle, JoinHandle<()>), ProtocolError> {
    let _ = stream.set_nodelay(true);
    drive_session_stream(id, stream, peer_addr, config, handler)
}

/// Splits any `AsyncRead + AsyncWrite` transport into bounded reader and writer Tokio tasks.
pub fn drive_session_stream<S, H>(
    id: u64,
    stream: S,
    peer_addr: SocketAddr,
    config: SessionConfig,
    handler: Arc<H>,
) -> Result<(SessionHandle, JoinHandle<()>), ProtocolError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: SessionEventHandler,
{
    let (read_cipher, write_cipher) = XorSessionCipher::new(&config.session_key)?.split();
    let (outbound_tx, outbound_rx) = mpsc::channel(config.outbound_capacity.max(1));
    let shared = Arc::new(SessionShared {
        id,
        peer_addr,
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
    let handle = SessionHandle {
        shared,
        outbound_tx,
    };
    let sup = handle.clone();
    let join = tokio::spawn(async move {
        let (reader, writer) = tokio::io::split(stream);
        let w_shared = Arc::clone(&sup.shared);
        let w_task = tokio::spawn(run_writer_loop(
            writer,
            write_cipher,
            outbound_rx,
            w_shared,
            config.write_timeout,
        ));
        if let Err(err) =
            run_reader_loop(reader, read_cipher, &sup, &config, handler.as_ref()).await
        {
            debug!(session_id = id, %peer_addr, error = %err, "session reader terminated");
        }
        sup.disconnect();
        let _ = w_task.await;
        handler.on_disconnect(&sup).await;
    });
    Ok((handle, join))
}

async fn run_reader_loop<R: AsyncRead + Unpin, H: SessionEventHandler>(
    mut reader: R,
    mut cipher: XorReadHalf,
    handle: &SessionHandle,
    config: &SessionConfig,
    handler: &H,
) -> Result<(), ProtocolError> {
    let mut buf = BytesMut::with_capacity(config.read_buffer_capacity);
    loop {
        while let Some(raw) = decode_c2s_frame(&mut buf, &mut cipher)? {
            if handle.shared.cancel.is_cancelled() {
                return Ok(());
            }
            trace!(
                session_id = handle.id(),
                cmd = raw.cmd,
                len = raw.payload.len(),
                "c2s frame"
            );
            if !cipher.sent_key() {
                if raw.cmd == HANDSHAKE_CMD {
                    let msg = OutboundCommand::HandshakeAndVersionRes {
                        vs_res: config.initial_vs_res,
                    };
                    tokio::select! {
                        biased;
                        () = handle.shared.cancel.cancelled() => return Ok(()),
                        res = handle.outbound_tx.send(msg) => res.map_err(|_| ProtocolError::SessionClosed)?,
                    }
                    cipher.set_sent_key(true);
                    handle.shared.sent_key.store(true, Ordering::Release);
                } else {
                    return Err(ProtocolError::InvalidHandshakeOpcode { actual: raw.cmd });
                }
                continue;
            }
            let pkt = ClientPacket::decode(raw.cmd, &mut raw.reader(), handle.client_version())?;
            if let ClientPacket::SetClientType(info) = &pkt {
                if !handle.shared.client_type_set.swap(true, Ordering::AcqRel) {
                    handle
                        .shared
                        .zoom_level
                        .store(info.zoom_level, Ordering::Release);
                    handle
                        .shared
                        .client_version
                        .store(info.parsed_version, Ordering::Release);
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
        if n == 0 {
            return Ok(());
        }
    }
}

async fn run_writer_loop<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut cipher: XorWriteHalf,
    mut rx: mpsc::Receiver<OutboundCommand>,
    shared: Arc<SessionShared>,
    timeout: Duration,
) {
    let cancel = &shared.cancel;
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
    shared.connected.store(false, Ordering::Release);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer.shutdown()).await;
}

fn encode_outbound_cmd(
    cmd: OutboundCommand,
    cipher: &mut XorWriteHalf,
    wire: &mut BytesMut,
) -> Result<(), ProtocolError> {
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

async fn flush_with_timeout<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    timeout: Duration,
) -> Result<(), ProtocolError> {
    tokio::time::timeout(timeout, async {
        writer.write_all(bytes).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
    Ok(())
}
