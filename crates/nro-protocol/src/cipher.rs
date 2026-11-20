//! Stateful rolling XOR stream cipher (`MessageSendCollect.java`) and `-27`
//! (`KeyHandler.java`) delta-XOR session key handshake codec.

use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};

use crate::ProtocolError;

/// Handshake opcode (`GET_SESSION_ID = -27` / `0xE5`) intercepted prior to XOR activation.
pub const HANDSHAKE_CMD: i8 = -27;

/// Default static 3-byte session key (`"NRO"` = `[0x4E, 0x52, 0x4F]`) from `Session.java:21`.
pub const DEFAULT_SESSION_KEY: &[u8] = b"NRO";

fn validate_key(slice: &[u8]) -> Result<Arc<[u8]>, ProtocolError> {
    if slice.is_empty() {
        return Err(ProtocolError::EmptySessionKey);
    }
    if u8::try_from(slice.len()).is_err() {
        return Err(ProtocolError::SessionKeyTooLong { len: slice.len() });
    }
    Ok(Arc::from(slice))
}

/// Inbound (Client-to-Server) rolling XOR decryption half (`MessageSendCollect.curR`).
#[derive(Debug, Clone)]
pub struct XorReadHalf {
    key: Arc<[u8]>,
    cur_r: usize,
    sent_key: bool,
}

impl XorReadHalf {
    /// Creates a new inbound cipher half initialized at `cur_r = 0` with `sent_key = false`.
    pub fn new(key: impl AsRef<[u8]>) -> Result<Self, ProtocolError> {
        Ok(Self { key: validate_key(key.as_ref())?, cur_r: 0, sent_key: false })
    }
    /// Returns whether the `-27` key handshake has completed on this inbound stream.
    #[inline] #[must_use] pub const fn sent_key(&self) -> bool { self.sent_key }
    /// Marks the session key as active (`sent_key = true`) or resets it on disconnect.
    #[inline] pub fn set_sent_key(&mut self, active: bool) { self.sent_key = active; }
    /// Returns the current inbound rolling key index (`0..key.len()`).
    #[inline] #[must_use] pub const fn cursor(&self) -> usize { self.cur_r }
    /// Returns the underlying session key bytes.
    #[inline] #[must_use] pub fn key(&self) -> &[u8] { &self.key }

    /// Decrypts a single byte and advances `cur_r` (`MessageSendCollect.readKey`).
    #[inline]
    pub fn read_key(&mut self, byte: u8) -> u8 {
        let out = self.key[self.cur_r] ^ byte;
        self.cur_r += 1;
        if self.cur_r >= self.key.len() {
            self.cur_r %= self.key.len();
        }
        out
    }

    /// Peeks a decrypted byte at `cur_r + offset` **without** advancing `cur_r`.
    #[inline]
    #[must_use]
    pub fn peek_decrypted(&self, byte: u8, offset: usize) -> u8 {
        if !self.sent_key {
            return byte;
        }
        let idx = (self.cur_r + offset) % self.key.len();
        self.key[idx] ^ byte
    }

    /// Decrypts `src` out-of-place into `dst` when `sent_key` is active, or copies verbatim.
    pub fn decrypt_into(&mut self, src: &[u8], dst: &mut BytesMut) {
        dst.reserve(src.len());
        if self.sent_key {
            for &b in src {
                dst.put_u8(self.read_key(b));
            }
        } else {
            dst.extend_from_slice(src);
        }
    }
}

/// Outbound (Server-to-Client) rolling XOR encryption half (`MessageSendCollect.curW`).
#[derive(Debug, Clone)]
pub struct XorWriteHalf {
    key: Arc<[u8]>,
    cur_w: usize,
    sent_key: bool,
}

impl XorWriteHalf {
    /// Creates a new outbound cipher half initialized at `cur_w = 0` with `sent_key = false`.
    pub fn new(key: impl AsRef<[u8]>) -> Result<Self, ProtocolError> {
        Ok(Self { key: validate_key(key.as_ref())?, cur_w: 0, sent_key: false })
    }
    /// Returns whether outbound XOR encryption is active (`sent_key == true`).
    #[inline] #[must_use] pub const fn sent_key(&self) -> bool { self.sent_key }
    /// Enables or disables outbound XOR encryption.
    #[inline] pub fn set_sent_key(&mut self, active: bool) { self.sent_key = active; }
    /// Returns the current outbound rolling key index (`0..key.len()`).
    #[inline] #[must_use] pub const fn cursor(&self) -> usize { self.cur_w }
    /// Returns the underlying session key bytes.
    #[inline] #[must_use] pub fn key(&self) -> &[u8] { &self.key }

    /// Encrypts a single byte and advances `cur_w` (`MessageSendCollect.writeKey`).
    #[inline]
    pub fn write_key(&mut self, byte: u8) -> u8 {
        let out = self.key[self.cur_w] ^ byte;
        self.cur_w += 1;
        if self.cur_w >= self.key.len() {
            self.cur_w %= self.key.len();
        }
        out
    }

    /// Encrypts `src` **out-of-place** into `dst` without mutating `src`.
    pub fn encrypt_into(&mut self, src: &[u8], dst: &mut BytesMut) {
        dst.reserve(src.len());
        if self.sent_key {
            for &b in src {
                dst.put_u8(self.write_key(b));
            }
        } else {
            dst.extend_from_slice(src);
        }
    }

    /// Writes the unencrypted `-27` handshake frame into `dst` and sets `sent_key = true`.
    pub fn write_handshake_frame(&mut self, dst: &mut BytesMut) -> Result<(), ProtocolError> {
        let payload = build_handshake_payload(&self.key)?;
        let len_u16 = u16::try_from(payload.len()).expect("handshake payload <= 263 bytes");
        dst.reserve(3 + payload.len());
        dst.put_i8(HANDSHAKE_CMD);
        dst.put_u16(len_u16);
        dst.extend_from_slice(&payload);
        self.sent_key = true;
        Ok(())
    }
}

/// Combined bidirectional session cipher pairing [`XorReadHalf`] and [`XorWriteHalf`].
#[derive(Debug, Clone)]
pub struct XorSessionCipher {
    /// Inbound C2S rolling decryption state.
    pub read: XorReadHalf,
    /// Outbound S2C rolling encryption state.
    pub write: XorWriteHalf,
}

impl XorSessionCipher {
    /// Constructs a bidirectional cipher using a custom non-empty session key (`1..=255` bytes).
    pub fn new(key: impl AsRef<[u8]>) -> Result<Self, ProtocolError> {
        let shared = validate_key(key.as_ref())?;
        Ok(Self {
            read: XorReadHalf { key: Arc::clone(&shared), cur_r: 0, sent_key: false },
            write: XorWriteHalf { key: shared, cur_w: 0, sent_key: false },
        })
    }
    /// Constructs a bidirectional cipher using the default `"NRO"` session key.
    #[must_use]
    pub fn default_nro() -> Self {
        Self::new(DEFAULT_SESSION_KEY).expect("DEFAULT_SESSION_KEY is non-empty")
    }
    /// Activates or deactivates rolling XOR on both read and write directions simultaneously.
    #[inline]
    pub fn set_sent_key(&mut self, active: bool) {
        self.read.set_sent_key(active);
        self.write.set_sent_key(active);
    }
    /// Splits the cipher into independent read and write halves for lock-free Tokio I/O tasks.
    #[inline] #[must_use] pub fn split(self) -> (XorReadHalf, XorWriteHalf) { (self.read, self.write) }

    /// Encrypts `src` out-of-place into a newly allocated `Bytes` buffer using the write half.
    pub fn encrypt_out_of_place(&mut self, src: &[u8]) -> Bytes {
        let mut out = BytesMut::with_capacity(src.len());
        self.write.encrypt_into(src, &mut out);
        out.freeze()
    }
}

/// Encodes `key` into the `-27` (`GET_SESSION_ID`) delta-XOR payload (`KeyHandler.java:13-17`),
/// appending trailing `IP2 = ""` (`0u16`), `PORT2 = 0i32`, and `isConnect2 = 0u8` (7 zero bytes)
/// required by C# `Session_ME.getKey()`.
pub fn build_handshake_payload(key: &[u8]) -> Result<Bytes, ProtocolError> {
    if key.is_empty() {
        return Err(ProtocolError::EmptySessionKey);
    }
    let len_u8 = u8::try_from(key.len()).map_err(|_| ProtocolError::SessionKeyTooLong { len: key.len() })?;
    let mut buf = BytesMut::with_capacity(1 + key.len() + 7);
    buf.put_u8(len_u8);
    buf.put_u8(key[0]);
    for i in 1..key.len() {
        buf.put_u8(key[i] ^ key[i - 1]);
    }
    // Trailing IP2 = "" (0u16), PORT2 = 0i32, isConnect2 = 0u8
    buf.extend_from_slice(&[0u8; 7]);
    Ok(buf.freeze())
}

/// Parses a `-27` (`GET_SESSION_ID`) delta-XOR payload back into the raw session key bytes.
pub fn parse_handshake_payload(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if payload.is_empty() {
        return Err(ProtocolError::UnexpectedEof { needed: 1, remaining: 0 });
    }
    let key_len = usize::from(payload[0]);
    if key_len == 0 {
        return Err(ProtocolError::EmptySessionKey);
    }
    if payload.len() < 1 + key_len {
        return Err(ProtocolError::UnexpectedEof { needed: 1 + key_len, remaining: payload.len() });
    }
    let mut key = Vec::with_capacity(key_len);
    key.push(payload[1]);
    for i in 1..key_len {
        let prev = key[i - 1];
        key.push(payload[1 + i] ^ prev);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_payload_appends_ip2_port2_and_is_connect2() {
        let payload = build_handshake_payload(DEFAULT_SESSION_KEY).expect("build handshake");
        assert_eq!(payload.len(), 1 + DEFAULT_SESSION_KEY.len() + 7);
        assert_eq!(&payload[1 + DEFAULT_SESSION_KEY.len()..], &[0u8; 7]);
        assert_eq!(parse_handshake_payload(&payload).expect("parse handshake"), DEFAULT_SESSION_KEY);
    }
}
