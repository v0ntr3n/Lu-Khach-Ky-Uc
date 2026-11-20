//! Order-preserving big-endian key encoders and `postcard` binary value helpers.
//!
//! Signed integer keys (`i32`, `i64`) flip their MSB (`^ 0x8000_...`) prior to big-endian byte
//! serialization so lexicographical byte order matches signed numerical order across negative IDs,
//! zero, and positive IDs.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{DbError, DbResult};

/// Encodes a signed `i32` into 4 big-endian bytes whose unsigned byte order matches `i32::cmp`.
#[inline]
#[must_use]
pub const fn enc_i32(v: i32) -> [u8; 4] {
    (v.cast_unsigned() ^ 0x8000_0000).to_be_bytes()
}

/// Decodes a 4-byte sign-flipped big-endian key back into `i32`.
#[inline]
pub fn dec_i32(bytes: &[u8]) -> DbResult<i32> {
    let arr: [u8; 4] = bytes.try_into().map_err(|_| DbError::InvalidColumnFormat {
        column: "key_i32",
        reason: format!("expected 4 bytes, got {}", bytes.len()),
    })?;
    Ok((u32::from_be_bytes(arr) ^ 0x8000_0000).cast_signed())
}

/// Encodes a signed `i64` into 8 big-endian bytes whose unsigned byte order matches `i64::cmp`.
#[inline]
#[must_use]
pub const fn enc_i64(v: i64) -> [u8; 8] {
    (v.cast_unsigned() ^ 0x8000_0000_0000_0000).to_be_bytes()
}

/// Decodes an 8-byte sign-flipped big-endian key back into `i64`.
#[inline]
pub fn dec_i64(bytes: &[u8]) -> DbResult<i64> {
    let arr: [u8; 8] = bytes.try_into().map_err(|_| DbError::InvalidColumnFormat {
        column: "key_i64",
        reason: format!("expected 8 bytes, got {}", bytes.len()),
    })?;
    Ok((u64::from_be_bytes(arr) ^ 0x8000_0000_0000_0000).cast_signed())
}

/// Normalizes a username or character name for MySQL `utf8mb4_general_ci`-compatible unique lookup.
#[inline]
#[must_use]
pub fn canonical_lookup_key(raw: &str) -> Vec<u8> {
    raw.trim().to_lowercase().into_bytes()
}

/// Composite 12-byte key `[enc_i32(rank), enc_i64(player_id)]` for `idx_super_rank`.
#[inline]
#[must_use]
pub fn enc_super_rank_key(rank: i32, player_id: i64) -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..4].copy_from_slice(&enc_i32(rank));
    out[4..].copy_from_slice(&enc_i64(player_id));
    out
}

/// Composite 17-byte descending leaderboard key `[slot_u8, !sort_score_be, enc_i64(player_id)]`.
///
/// Bitwise-inverting `sort_score` places the highest scores first under forward prefix iteration.
#[inline]
#[must_use]
pub fn enc_leaderboard_key(slot: u8, sort_score: u64, player_id: i64) -> [u8; 17] {
    let mut out = [0_u8; 17];
    out[0] = slot;
    out[1..9].copy_from_slice(&(!sort_score).to_be_bytes());
    out[9..].copy_from_slice(&enc_i64(player_id));
    out
}

/// Composite 16-byte chronological trade-audit key `[enc_i64(time_tran_ms), seq_u64_be]`.
#[inline]
#[must_use]
pub fn enc_history_tx_key(time_tran_ms: i64, seq: u64) -> [u8; 16] {
    let mut out = [0_u8; 16];
    out[..8].copy_from_slice(&enc_i64(time_tran_ms));
    out[8..].copy_from_slice(&seq.to_be_bytes());
    out
}

/// Serializes `value` into a compact `postcard` byte vector.
#[inline]
pub fn encode_val<T: Serialize>(entity: &'static str, value: &T) -> DbResult<Vec<u8>> {
    postcard::to_allocvec(value).map_err(|e| DbError::BinaryCodec {
        entity,
        reason: e.to_string(),
    })
}

/// Deserializes `T` from a `postcard` byte slice.
#[inline]
pub fn decode_val<T: DeserializeOwned>(entity: &'static str, bytes: &[u8]) -> DbResult<T> {
    postcard::from_bytes(bytes).map_err(|e| DbError::BinaryCodec {
        entity,
        reason: e.to_string(),
    })
}
