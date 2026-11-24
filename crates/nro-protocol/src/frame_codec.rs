//! Wire S2C/C2S frame encoders, decoders, and round-trip frame tests.

use bytes::{BufMut, BytesMut};

use super::{MAX_EXTENDED_PAYLOAD, MAX_STANDARD_PAYLOAD, Packet, is_extended_s2c_cmd};
use crate::ProtocolError;
use crate::cipher::{HANDSHAKE_CMD, XorReadHalf, XorWriteHalf};

/// Encodes an S2C [`Packet`] out-of-place into `dst` (`MessageSendCollect.doSendMessage`).
pub fn encode_s2c_frame(
    packet: &Packet,
    cipher: &mut XorWriteHalf,
    dst: &mut BytesMut,
) -> Result<(), ProtocolError> {
    let size = packet.payload.len();
    let extended = is_extended_s2c_cmd(packet.cmd);
    let max = if extended {
        MAX_EXTENDED_PAYLOAD
    } else {
        MAX_STANDARD_PAYLOAD
    };
    if size > max {
        return Err(ProtocolError::PayloadTooLarge {
            cmd: packet.cmd,
            size,
            max,
        });
    }
    dst.reserve(4 + size);
    let raw_cmd = packet.cmd.to_be_bytes()[0];
    dst.put_u8(if cipher.sent_key() {
        cipher.write_key(raw_cmd)
    } else {
        raw_cmd
    });
    if extended {
        let le = (size as u32).to_le_bytes();
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
        let [hi, lo] = (size as u16).to_be_bytes();
        let h = if cipher.sent_key() {
            [cipher.write_key(hi), cipher.write_key(lo)]
        } else {
            [hi, lo]
        };
        dst.put_slice(&h);
    }
    cipher.encrypt_into(&packet.payload, dst);
    Ok(())
}

/// Encodes a C2S [`Packet`] out-of-place into `dst` (always 1-byte `cmd` + 2-byte BE `u16` length).
pub fn encode_c2s_frame(
    packet: &Packet,
    cipher: &mut XorWriteHalf,
    dst: &mut BytesMut,
) -> Result<(), ProtocolError> {
    let size = packet.payload.len();
    let sz16 = u16::try_from(size).map_err(|_| ProtocolError::PayloadTooLarge {
        cmd: packet.cmd,
        size,
        max: MAX_STANDARD_PAYLOAD,
    })?;
    let [hi, lo] = sz16.to_be_bytes();
    let raw_cmd = packet.cmd.to_be_bytes()[0];
    dst.reserve(3 + size);
    let hdr = if cipher.sent_key() {
        [
            cipher.write_key(raw_cmd),
            cipher.write_key(hi),
            cipher.write_key(lo),
        ]
    } else {
        [raw_cmd, hi, lo]
    };
    dst.put_slice(&hdr);
    cipher.encrypt_into(&packet.payload, dst);
    Ok(())
}

/// Non-blocking incremental C2S frame decoder (`MessageSendCollect.readMessage`).
pub fn decode_c2s_frame(
    src: &mut BytesMut,
    cipher: &mut XorReadHalf,
) -> Result<Option<Packet>, ProtocolError> {
    if src.len() < 3 {
        return Ok(None);
    }
    if !cipher.sent_key() {
        let cmd = i8::from_be_bytes([src[0]]);
        if cmd != HANDSHAKE_CMD {
            return Err(ProtocolError::InvalidHandshakeOpcode { actual: cmd });
        }
    }
    let size = usize::from(u16::from_be_bytes([
        cipher.peek_decrypted(src[1], 1),
        cipher.peek_decrypted(src[2], 2),
    ]));
    if src.len() < 3 + size {
        return Ok(None);
    }
    let raw = src.split_to(3 + size);
    let cmd_u8 = if cipher.sent_key() {
        let c = cipher.read_key(raw[0]);
        let _ = (cipher.read_key(raw[1]), cipher.read_key(raw[2]));
        c
    } else {
        raw[0]
    };
    let mut payload = BytesMut::with_capacity(size);
    cipher.decrypt_into(&raw[3..], &mut payload);
    Ok(Some(Packet::new(
        i8::from_be_bytes([cmd_u8]),
        payload.freeze(),
    )))
}

/// Non-blocking incremental S2C frame decoder supporting both 16-bit BE and 24-bit LE `- 128` frames.
pub fn decode_s2c_frame(
    src: &mut BytesMut,
    cipher: &mut XorReadHalf,
) -> Result<Option<Packet>, ProtocolError> {
    if src.len() < 3 {
        return Ok(None);
    }
    let cmd = i8::from_be_bytes([cipher.peek_decrypted(src[0], 0)]);
    let extended = is_extended_s2c_cmd(cmd);
    let hdr_len = if extended { 4 } else { 3 };
    if src.len() < hdr_len {
        return Ok(None);
    }
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
        usize::from(u16::from_be_bytes([
            cipher.peek_decrypted(src[1], 1),
            cipher.peek_decrypted(src[2], 2),
        ]))
    };
    if size > MAX_EXTENDED_PAYLOAD {
        return Err(ProtocolError::PayloadTooLarge {
            cmd,
            size,
            max: MAX_EXTENDED_PAYLOAD,
        });
    }
    if src.len() < hdr_len + size {
        return Ok(None);
    }
    let raw = src.split_to(hdr_len + size);
    if cipher.sent_key() {
        let _ = cipher.read_key(raw[0]);
        let shift = if extended { 128 } else { 0 };
        for &b in &raw[1..hdr_len] {
            let _ = cipher.read_key(b.wrapping_add(shift));
        }
    }
    let mut payload = BytesMut::with_capacity(size);
    cipher.decrypt_into(&raw[hdr_len..], &mut payload);
    Ok(Some(Packet::new(cmd, payload.freeze())))
}

#[cfg(test)]
mod tests {
    use super::super::PacketWriter;
    use super::*;
    use crate::cipher::XorSessionCipher;

    #[test]
    fn extended_opcodes_and_4mib_cap_enforced() {
        assert!(EXTENDED_S2C_OPCODES.contains(&12));
        assert_eq!(MAX_EXTENDED_PAYLOAD, 4 * 1024 * 1024);
        let mut cipher = XorSessionCipher::default_nro();
        let oversized = (MAX_EXTENDED_PAYLOAD + 1) as u32;
        let le = oversized.to_le_bytes();
        let mut wire = BytesMut::from(
            &[
                12_u8,
                le[0].wrapping_sub(128),
                le[1].wrapping_sub(128),
                le[2].wrapping_sub(128),
            ][..],
        );
        assert_eq!(
            decode_s2c_frame(&mut wire, &mut cipher.read),
            Err(ProtocolError::PayloadTooLarge {
                cmd: 12,
                size: MAX_EXTENDED_PAYLOAD + 1,
                max: MAX_EXTENDED_PAYLOAD
            })
        );
    }

    #[test]
    fn pre_handshake_c2s_rejects_non_handshake_cmd() {
        let mut cipher = XorSessionCipher::default_nro();
        let mut bad = BytesMut::from(&[44_u8, 0, 0][..]);
        assert_eq!(
            decode_c2s_frame(&mut bad, &mut cipher.read),
            Err(ProtocolError::InvalidHandshakeOpcode { actual: 44 })
        );
        let mut ok = BytesMut::from(&[HANDSHAKE_CMD.to_be_bytes()[0], 0, 0][..]);
        assert_eq!(
            decode_c2s_frame(&mut ok, &mut cipher.read),
            Ok(Some(Packet::empty(HANDSHAKE_CMD)))
        );
    }

    #[test]
    fn unencrypted_extended_frame_does_not_advance_cipher_cursor() {
        let mut cipher = XorSessionCipher::default_nro();
        let pkt = Packet::new(12, vec![1u8, 2, 3, 4]);
        let mut wire = BytesMut::new();
        encode_s2c_frame(&pkt, &mut cipher.write, &mut wire).expect("encode unencrypted extended");
        assert_eq!(cipher.write.cursor(), 0);
        let decoded =
            decode_s2c_frame(&mut wire, &mut cipher.read).expect("decode unencrypted extended");
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
            Err(ProtocolError::Utf8StringTooLong {
                byte_len: MAX_UTF_BYTES + 1
            })
        );
    }
}
