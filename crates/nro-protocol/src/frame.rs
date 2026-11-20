//! Asymmetric wire packet framing (`MessageSendCollect.java`) and Java `DataInputStream` /
//! `DataOutputStream` big-endian + Modified UTF-8 binary readers and writers (`Message.java`).

use bytes::{BufMut, Bytes, BytesMut};

use crate::ProtocolError;
use crate::cipher::{XorReadHalf, XorWriteHalf};

/// Maximum payload size (`65,535` bytes) for standard 16-bit framed packets.
pub const MAX_STANDARD_PAYLOAD: usize = 0xFFFF;
/// Maximum payload size (`16,777,215` bytes) for S2C 24-bit extended asset packets.
pub const MAX_EXTENDED_PAYLOAD: usize = 0x00FF_FFFF;
/// Maximum encoded byte length (`32,767` bytes, `i16::MAX`) for C#-compatible `readUTF` / `writeUTF` strings.
pub const MAX_UTF_BYTES: usize = i16::MAX as usize;
/// Exact 8 S2C opcodes framed with 3-byte little-endian `- 128` headers (`MessageSendCollect.java:63`, plus `12` `cmdExtraBig`).
pub const EXTENDED_S2C_OPCODES: [i8; 8] = [-32, -66, -74, 11, -67, -87, 66, 12];

/// Returns `true` if an S2C packet with `cmd` uses the 24-bit little-endian `- 128` header.
#[inline]
#[must_use]
pub const fn is_extended_s2c_cmd(cmd: i8) -> bool {
    matches!(cmd, -32 | -66 | -74 | 11 | -67 | -87 | 66 | 12)
}

/// Immutable, reference-counted protocol packet (`Message.java` equivalent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Signed 8-bit command opcode (`Message.command`).
    pub cmd: i8,
    /// Unencrypted payload buffer shared cheaply across broadcast recipients.
    pub payload: Bytes,
}

impl Packet {
    /// Constructs a packet from `cmd` and any `Bytes`-convertible payload buffer.
    #[inline] #[must_use] pub fn new(cmd: i8, payload: impl Into<Bytes>) -> Self { Self { cmd, payload: payload.into() } }
    /// Constructs a packet with an empty (`0`-byte) payload.
    #[inline] #[must_use] pub const fn empty(cmd: i8) -> Self { Self { cmd, payload: Bytes::new() } }
    /// Borrows the payload as a big-endian [`PacketReader`].
    #[inline] #[must_use] pub fn reader(&self) -> PacketReader<'_> { PacketReader::new(&self.payload) }
}

/// Zero-copy big-endian reader compatible with `java.io.DataInputStream` (`Message.reader()`).
#[derive(Debug, Clone, Copy)]
pub struct PacketReader<'a> {
    buf: &'a [u8],
}

impl<'a> PacketReader<'a> {
    /// Wraps a raw decrypted payload slice.
    #[inline] #[must_use] pub const fn new(buf: &'a [u8]) -> Self { Self { buf } }
    /// Returns the number of unread bytes remaining in the slice.
    #[inline] #[must_use] pub const fn remaining(&self) -> usize { self.buf.len() }
    /// Returns `true` when all payload bytes have been consumed.
    #[inline] #[must_use] pub const fn is_empty(&self) -> bool { self.buf.is_empty() }

    /// Reads an exact sub-slice of `len` bytes (`DataInputStream.readFully`).
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        if self.buf.len() < len {
            return Err(ProtocolError::UnexpectedEof { needed: len, remaining: self.buf.len() });
        }
        let (head, tail) = self.buf.split_at(len);
        self.buf = tail;
        Ok(head)
    }

    /// Reads a 1-byte boolean (`!= 0`, `DataInputStream.readBoolean`).
    #[inline] pub fn read_bool(&mut self) -> Result<bool, ProtocolError> { Ok(self.read_u8()? != 0) }
    /// Reads a signed 8-bit integer (`DataInputStream.readByte`).
    #[inline] pub fn read_i8(&mut self) -> Result<i8, ProtocolError> { Ok(i8::from_be_bytes([self.read_bytes(1)?[0]])) }
    /// Reads an unsigned 8-bit integer (`DataInputStream.readUnsignedByte`).
    #[inline] pub fn read_u8(&mut self) -> Result<u8, ProtocolError> { Ok(self.read_bytes(1)?[0]) }
    /// Reads a big-endian signed 16-bit integer (`DataInputStream.readShort`).
    #[inline] pub fn read_i16(&mut self) -> Result<i16, ProtocolError> { let b = self.read_bytes(2)?; Ok(i16::from_be_bytes([b[0], b[1]])) }
    /// Reads a big-endian unsigned 16-bit integer (`DataInputStream.readUnsignedShort`).
    #[inline] pub fn read_u16(&mut self) -> Result<u16, ProtocolError> { let b = self.read_bytes(2)?; Ok(u16::from_be_bytes([b[0], b[1]])) }
    /// Reads a big-endian signed 32-bit integer (`DataInputStream.readInt`).
    #[inline] pub fn read_i32(&mut self) -> Result<i32, ProtocolError> { let b = self.read_bytes(4)?; Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]])) }
    /// Reads a big-endian signed 64-bit integer (`DataInputStream.readLong`).
    #[inline] pub fn read_i64(&mut self) -> Result<i64, ProtocolError> { let b = self.read_bytes(8)?; Ok(i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])) }
    /// Reads an IEEE-754 big-endian 32-bit float (`DataInputStream.readFloat`).
    #[inline] pub fn read_f32(&mut self) -> Result<f32, ProtocolError> { let b = self.read_bytes(4)?; Ok(f32::from_be_bytes([b[0], b[1], b[2], b[3]])) }
    /// Reads an IEEE-754 big-endian 64-bit float (`DataInputStream.readDouble`).
    #[inline] pub fn read_f64(&mut self) -> Result<f64, ProtocolError> { let b = self.read_bytes(8)?; Ok(f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])) }

    /// Reads a `u16` BE length-prefixed Java Modified UTF-8 string (`DataInputStream.readUTF`).
    pub fn read_utf(&mut self) -> Result<String, ProtocolError> {
        let len = usize::from(self.read_u16()?);
        let raw = self.read_bytes(len)?;
        if !raw.contains(&0xC0) && !raw.contains(&0xED) && let Ok(s) = std::str::from_utf8(raw) {
            return Ok(s.to_owned());
        }
        decode_modified_utf8(raw)
    }
}

/// Big-endian buffer builder compatible with `java.io.DataOutputStream` (`Message.writer()`).
#[derive(Debug, Default, Clone)]
pub struct PacketWriter {
    buf: BytesMut,
}

impl PacketWriter {
    /// Creates an empty writer with default initial capacity (`64` bytes).
    #[inline] #[must_use] pub fn new() -> Self { Self { buf: BytesMut::with_capacity(64) } }
    /// Creates an empty writer pre-allocated for `capacity` payload bytes.
    #[inline] #[must_use] pub fn with_capacity(capacity: usize) -> Self { Self { buf: BytesMut::with_capacity(capacity) } }
    /// Returns the current payload length in bytes.
    #[inline] #[must_use] pub fn len(&self) -> usize { self.buf.len() }
    /// Returns `true` if no payload bytes have been written.
    #[inline] #[must_use] pub fn is_empty(&self) -> bool { self.buf.is_empty() }
    /// Writes a 1-byte boolean (`1` or `0`).
    #[inline] pub fn write_bool(&mut self, v: bool) { self.buf.put_u8(u8::from(v)); }
    /// Writes a signed 8-bit integer (`writeByte`).
    #[inline] pub fn write_i8(&mut self, v: i8) { self.buf.put_i8(v); }
    /// Writes an unsigned 8-bit integer (`writeByte`).
    #[inline] pub fn write_u8(&mut self, v: u8) { self.buf.put_u8(v); }
    /// Writes a big-endian signed 16-bit integer (`writeShort`).
    #[inline] pub fn write_i16(&mut self, v: i16) { self.buf.put_i16(v); }
    /// Writes a big-endian unsigned 16-bit integer (`writeShort`).
    #[inline] pub fn write_u16(&mut self, v: u16) { self.buf.put_u16(v); }
    /// Writes a big-endian signed 32-bit integer (`writeInt`).
    #[inline] pub fn write_i32(&mut self, v: i32) { self.buf.put_i32(v); }
    /// Writes a big-endian signed 64-bit integer (`writeLong`).
    #[inline] pub fn write_i64(&mut self, v: i64) { self.buf.put_i64(v); }
    /// Writes an IEEE-754 big-endian 32-bit float (`writeFloat`).
    #[inline] pub fn write_f32(&mut self, v: f32) { self.buf.put_f32(v); }
    /// Writes an IEEE-754 big-endian 64-bit float (`writeDouble`).
    #[inline] pub fn write_f64(&mut self, v: f64) { self.buf.put_f64(v); }
    /// Appends raw unframed bytes (`write(byte[])`).
    #[inline] pub fn write_bytes(&mut self, data: &[u8]) { self.buf.extend_from_slice(data); }

    /// Writes a signed 16-bit (`i16`) BE length-prefixed Java Modified UTF-8 string (`DataOutputStream.writeUTF`),
    /// enforcing C# `myReader.readUTF()`'s `32,767`-byte (`i16::MAX`) ceiling ([`MAX_UTF_BYTES`]).
    pub fn write_utf(&mut self, s: &str) -> Result<(), ProtocolError> {
        if !s.bytes().any(|b| b == 0 || b >= 0xF0) {
            let len_i16 = i16::try_from(s.len()).map_err(|_| ProtocolError::Utf8StringTooLong { byte_len: s.len() })?;
            self.buf.put_i16(len_i16);
            self.buf.extend_from_slice(s.as_bytes());
            return Ok(());
        }
        let encoded = encode_modified_utf8(s);
        let len_i16 = i16::try_from(encoded.len()).map_err(|_| ProtocolError::Utf8StringTooLong { byte_len: encoded.len() })?;
        self.buf.put_i16(len_i16);
        self.buf.extend_from_slice(&encoded);
        Ok(())
    }

    /// Finalizes the buffer into an immutable [`Packet`] with opcode `cmd`.
    #[inline] #[must_use] pub fn finish(self, cmd: i8) -> Packet { Packet { cmd, payload: self.buf.freeze() } }
}

fn encode_modified_utf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 4);
    for cu in s.encode_utf16() {
        let [hi, lo] = cu.to_be_bytes();
        if cu == 0 {
            out.extend_from_slice(&[0xC0, 0x80]);
        } else if cu <= 0x007F {
            out.push(lo);
        } else if cu <= 0x07FF {
            out.extend_from_slice(&[0xC0 | ((hi << 2) | (lo >> 6)), 0x80 | (lo & 0x3F)]);
        } else {
            out.extend_from_slice(&[0xE0 | (hi >> 4), 0x80 | (((hi & 0x0F) << 2) | (lo >> 6)), 0x80 | (lo & 0x3F)]);
        }
    }
    out
}

fn decode_modified_utf8(bytes: &[u8]) -> Result<String, ProtocolError> {
    let mut utf16 = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 & 0x80 == 0 {
            utf16.push(u16::from(b0));
            i += 1;
        } else if (b0 & 0xE0) == 0xC0 {
            if i + 1 >= bytes.len() || (bytes[i + 1] & 0xC0) != 0x80 { return Err(ProtocolError::InvalidModifiedUtf8); }
            utf16.push((u16::from(b0 & 0x1F) << 6) | u16::from(bytes[i + 1] & 0x3F));
            i += 2;
        } else if (b0 & 0xF0) == 0xE0 {
            if i + 2 >= bytes.len() || (bytes[i + 1] & 0xC0) != 0x80 || (bytes[i + 2] & 0xC0) != 0x80 { return Err(ProtocolError::InvalidModifiedUtf8); }
            utf16.push((u16::from(b0 & 0x0F) << 12) | (u16::from(bytes[i + 1] & 0x3F) << 6) | u16::from(bytes[i + 2] & 0x3F));
            i += 3;
        } else {
            return Err(ProtocolError::InvalidModifiedUtf8);
        }
    }
    String::from_utf16(&utf16).map_err(|_| ProtocolError::InvalidModifiedUtf8)
}

/// Encodes an S2C [`Packet`] out-of-place into `dst` (`MessageSendCollect.doSendMessage`).
pub fn encode_s2c_frame(packet: &Packet, cipher: &mut XorWriteHalf, dst: &mut BytesMut) -> Result<(), ProtocolError> {
    let size = packet.payload.len();
    let extended = is_extended_s2c_cmd(packet.cmd);
    let max = if extended { MAX_EXTENDED_PAYLOAD } else { MAX_STANDARD_PAYLOAD };
    if size > max { return Err(ProtocolError::PayloadTooLarge { cmd: packet.cmd, size, max }); }
    dst.reserve(4 + size);
    let raw_cmd = packet.cmd.to_be_bytes()[0];
    dst.put_u8(if cipher.sent_key() { cipher.write_key(raw_cmd) } else { raw_cmd });
    if extended {
        let le = u32::try_from(size).expect("size <= 0xFF_FFFF").to_le_bytes();
        let h = if cipher.sent_key() {
            [
                cipher.write_key(le[0]).wrapping_sub(128),
                cipher.write_key(le[1]).wrapping_sub(128),
                cipher.write_key(le[2]).wrapping_sub(128),
            ]
        } else {
            [
                le[0].wrapping_sub(128),
                le[1].wrapping_sub(128),
                le[2].wrapping_sub(128),
            ]
        };
        dst.put_slice(&h);
    } else {
        let [hi, lo] = u16::try_from(size).expect("size <= 0xFFFF").to_be_bytes();
        let h = if cipher.sent_key() { [cipher.write_key(hi), cipher.write_key(lo)] } else { [hi, lo] };
        dst.put_slice(&h);
    }
    cipher.encrypt_into(&packet.payload, dst);
    Ok(())
}

/// Encodes a C2S [`Packet`] out-of-place into `dst` (always 1-byte `cmd` + 2-byte BE `u16` length).
pub fn encode_c2s_frame(packet: &Packet, cipher: &mut XorWriteHalf, dst: &mut BytesMut) -> Result<(), ProtocolError> {
    let size = packet.payload.len();
    let sz16 = u16::try_from(size).map_err(|_| ProtocolError::PayloadTooLarge { cmd: packet.cmd, size, max: MAX_STANDARD_PAYLOAD })?;
    let [hi, lo] = sz16.to_be_bytes();
    let raw_cmd = packet.cmd.to_be_bytes()[0];
    dst.reserve(3 + size);
    let hdr = if cipher.sent_key() { [cipher.write_key(raw_cmd), cipher.write_key(hi), cipher.write_key(lo)] } else { [raw_cmd, hi, lo] };
    dst.put_slice(&hdr);
    cipher.encrypt_into(&packet.payload, dst);
    Ok(())
}

/// Non-blocking incremental C2S frame decoder (`MessageSendCollect.readMessage`).
pub fn decode_c2s_frame(src: &mut BytesMut, cipher: &mut XorReadHalf) -> Result<Option<Packet>, ProtocolError> {
    if src.len() < 3 { return Ok(None); }
    let size = usize::from(u16::from_be_bytes([cipher.peek_decrypted(src[1], 1), cipher.peek_decrypted(src[2], 2)]));
    if src.len() < 3 + size { return Ok(None); }
    let raw = src.split_to(3 + size);
    let cmd_u8 = if cipher.sent_key() {
        let c = cipher.read_key(raw[0]);
        let _ = (cipher.read_key(raw[1]), cipher.read_key(raw[2]));
        c
    } else { raw[0] };
    let mut payload = BytesMut::with_capacity(size);
    cipher.decrypt_into(&raw[3..], &mut payload);
    Ok(Some(Packet::new(i8::from_be_bytes([cmd_u8]), payload.freeze())))
}

/// Non-blocking incremental S2C frame decoder supporting both 16-bit BE and 24-bit LE `- 128` frames.
pub fn decode_s2c_frame(src: &mut BytesMut, cipher: &mut XorReadHalf) -> Result<Option<Packet>, ProtocolError> {
    if src.len() < 3 { return Ok(None); }
    let cmd = i8::from_be_bytes([cipher.peek_decrypted(src[0], 0)]);
    let extended = is_extended_s2c_cmd(cmd);
    let hdr_len = if extended { 4 } else { 3 };
    if src.len() < hdr_len { return Ok(None); }
    let size = if extended {
        if cipher.sent_key() {
            let k = cipher.key();
            let lo = usize::from(k[(cipher.cursor() + 1) % k.len()] ^ src[1].wrapping_add(128));
            let mid = usize::from(k[(cipher.cursor() + 2) % k.len()] ^ src[2].wrapping_add(128));
            let hi = usize::from(k[(cipher.cursor() + 3) % k.len()] ^ src[3].wrapping_add(128));
            lo | (mid << 8) | (hi << 16)
        } else {
            let lo = usize::from(src[1].wrapping_add(128));
            let mid = usize::from(src[2].wrapping_add(128));
            let hi = usize::from(src[3].wrapping_add(128));
            lo | (mid << 8) | (hi << 16)
        }
    } else {
        usize::from(u16::from_be_bytes([cipher.peek_decrypted(src[1], 1), cipher.peek_decrypted(src[2], 2)]))
    };
    if src.len() < hdr_len + size { return Ok(None); }
    let raw = src.split_to(hdr_len + size);
    if cipher.sent_key() {
        let _ = cipher.read_key(raw[0]);
        let shift = if extended { 128 } else { 0 };
        for &b in &raw[1..hdr_len] { let _ = cipher.read_key(b.wrapping_add(shift)); }
    }
    let mut payload = BytesMut::with_capacity(size);
    cipher.decrypt_into(&raw[hdr_len..], &mut payload);
    Ok(Some(Packet::new(cmd, payload.freeze())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cipher::XorSessionCipher;

    #[test]
    fn extended_opcodes_include_cmd_extra_big_12() {
        assert!(EXTENDED_S2C_OPCODES.contains(&12));
        assert!(is_extended_s2c_cmd(12));
    }

    #[test]
    fn unencrypted_extended_frame_does_not_advance_cipher_cursor() {
        let mut cipher = XorSessionCipher::default_nro();
        let pkt = Packet::new(12, vec![1u8, 2, 3, 4]);
        let mut wire = BytesMut::new();
        encode_s2c_frame(&pkt, &mut cipher.write, &mut wire).expect("encode unencrypted extended");
        assert_eq!(cipher.write.cursor(), 0);
        let decoded = decode_s2c_frame(&mut wire, &mut cipher.read).expect("decode unencrypted extended");
        assert_eq!(decoded, Some(pkt));
        assert_eq!(cipher.read.cursor(), 0);
    }

    #[test]
    fn write_utf_enforces_csharp_short_max_length() {
        let mut pw = PacketWriter::new();
        let ok_str = "a".repeat(MAX_UTF_BYTES);
        pw.write_utf(&ok_str).expect("32767 bytes fits i16::MAX");
        let too_long = "a".repeat(MAX_UTF_BYTES + 1);
        assert_eq!(
            pw.write_utf(&too_long),
            Err(ProtocolError::Utf8StringTooLong { byte_len: MAX_UTF_BYTES + 1 })
        );
    }
}
