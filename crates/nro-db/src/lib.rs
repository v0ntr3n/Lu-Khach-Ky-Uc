//! Safe embedded `fjall` persistence, static game-data registry, legacy JSON wire codecs,
//! and one-time `database team2026.sql` streaming importer (`nro-db`).
//!
//! Ports the 41-table MariaDB/MySQL schema (`database team2026.sql` & `SRC/sql/nro1.sql`),
//! all 7 legacy DAO classes (`MrBlue`, `PlayerDAO`, `ShopDAO`, `EventDAO`, `HistoryTransactionDAO`,
//! `SuperRankDAO`, `TraningDAO`), and `Manager.loadDatabase` static template loaders into 100% safe
//! Rust (`#![forbid(unsafe_code)]`) targeting Rust `1.100.0` (`edition = "2024"`, `resolver = "3"`).

#![forbid(unsafe_code)]

pub mod account_repo;
pub mod codec;
pub mod engine;
pub mod json_columns;
pub mod json_ext;
pub mod legacy_sql;
pub mod player_model;
pub mod player_repo;
pub mod pool;
pub mod templates;
pub mod world_repos;

#[cfg(test)]
mod json_tests;

use std::fmt;
use std::str::FromStr;

use nro_core::{Gender, ParityMode};
use serde::de::{self, DeserializeOwned, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical crate-wide result alias returning [`DbError`].
pub type DbResult<T> = Result<T, DbError>;

/// Exact 41 table names defined in `database team2026.sql` and `SRC/sql/nro1.sql`.
pub const CANONICAL_41_TABLES: [&str; 41] = [
    "account", "achievement_template", "adminpanel", "array_head_2_frames", "bank_transfers",
    "bg_item_template", "clan", "clan_task_template", "comments", "data_badges", "flag_bag",
    "giftcode", "head_avatar", "history_transaction", "img_by_name", "intrinsic",
    "item_option_template", "item_shop", "item_shop_option", "item_template", "map_template",
    "mob_template", "napthe", "notify", "npc_template", "part", "payments", "phongchat", "player",
    "posts", "radar", "settings", "shop", "shop_ky_gui", "side_task_template", "skill_template",
    "super_rank", "tab_shop", "task_badges_template", "task_main_template", "task_sub_template",
];

/// Supplemental runtime table referenced by `EventDAO.java` but omitted from both SQL dumps.
pub const SUPPLEMENTAL_EVENT_TABLE: &str = "event";

/// Unified error hierarchy across `fjall` storage, binary/JSON codecs, SQL import, and auth.
#[derive(Debug, Error)]
pub enum DbError {
    /// Underlying embedded `fjall` LSM-tree storage error.
    #[error("fjall storage error: {0}")]
    Storage(#[from] fjall::Error),
    /// Filesystem I/O failure while reading SQL dumps or storage directories.
    #[error("storage I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// `postcard` binary serialization or deserialization error.
    #[error("binary codec error on `{entity}`: {reason}")]
    BinaryCodec { entity: &'static str, reason: String },
    /// Unique secondary index constraint violation (`username`, `player.name`, `player.account_id`).
    #[error("duplicate unique key on `{entity}`: `{key}`")]
    DuplicateKey { entity: &'static str, key: String },
    /// JSON serialization or deserialization failure on a legacy text column.
    #[error("column `{column}` json codec error: {source}")]
    JsonColumn {
        column: &'static str,
        #[source]
        source: serde_json::Error,
    },
    /// Malformed positional array length, binary key slice, or SQL dump syntax error.
    #[error("column `{column}` invalid format: {reason}")]
    InvalidColumnFormat { column: &'static str, reason: String },
    /// Password hashing or PHC format error inside Argon2id processing.
    #[error("password hash error: {0}")]
    PasswordHash(String),
    /// Blocking background task panicked or was cancelled.
    #[error("blocking worker join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// Required `fjall` keyspaces or static templates have not been initialized yet.
    #[error("database schema health check failed; missing items: {missing:?}")]
    SchemaHealth { missing: Vec<String> },
    /// Invalid gender discriminant stored in SQL or seed data.
    #[error("invalid player gender discriminant: {0}")]
    InvalidGender(i32),
    /// Account credentials did not match `accounts` records.
    #[error("invalid username or password")]
    InvalidCredentials,
    /// Account has `ban = true` in `accounts`.
    #[error("account `{username}` is banned")]
    AccountBanned { username: String },
    /// Account must wait before logging in again (`Manager.SECOND_WAIT_LOGIN`).
    #[error("login cooldown active; wait {wait_seconds}s")]
    LoginCooldown { wait_seconds: u32 },
    /// Requested entity record was not found.
    #[error("{entity} `{id}` not found")]
    NotFound { entity: &'static str, id: String },
}

/// Returns `true` when operating in strict bug-for-bug Java DAO compatibility mode (`ParityMode::Legacy`).
#[must_use]
pub const fn is_legacy_parity(mode: ParityMode) -> bool {
    matches!(mode, ParityMode::Legacy)
}

/// Converts a SQL integer discriminant (`0..=2`) into [`nro_core::Gender`].
pub fn gender_from_sql(raw: i32) -> DbResult<Gender> {
    let byte = i8::try_from(raw).map_err(|_| DbError::InvalidGender(raw))?;
    Gender::try_from(byte).map_err(|_| DbError::InvalidGender(raw))
}

/// Converts [`nro_core::Gender`] into its canonical SQL discriminant (`0`, `1`, or `2`).
#[must_use]
pub fn gender_to_sql(gender: Gender) -> i8 {
    i8::from(gender)
}

/// Wraps an inner value `T` that `org.json.simple` double-encoded via `outer.add(inner.toJSONString())`.
///
/// Uses direct binary encoding for non-human-readable codecs (`postcard`) and escaped JSON strings
/// for human-readable codecs (`serde_json`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JsonStr<T>(pub T);

impl<T: Serialize> Serialize for JsonStr<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let encoded = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&encoded)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for JsonStr<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            return T::deserialize(deserializer).map(Self);
        }
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::String(raw) => {
                serde_json::from_str::<T>(raw.trim()).map(Self).map_err(de::Error::custom)
            }
            other => serde_json::from_value::<T>(other).map(Self).map_err(de::Error::custom),
        }
    }
}

/// Wraps a numeric primitive serialized as a quoted string in JSON (`Card`, `BadgesData`, etc.)
/// while encoding directly as a compact primitive in binary formats (`postcard`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct StrNum<T>(pub T);

impl<T: fmt::Display + Serialize> Serialize for StrNum<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.0.to_string())
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de, T> Deserialize<'de> for StrNum<T>
where
    T: Deserialize<'de> + FromStr + TryFrom<i64> + TryFrom<u64>,
    <T as FromStr>::Err: fmt::Display,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            return T::deserialize(deserializer).map(Self);
        }
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::String(s) => s.trim().parse::<T>().map(Self).map_err(de::Error::custom),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    T::try_from(i).map(Self).map_err(|_| de::Error::custom("i64 overflow"))
                } else if let Some(u) = n.as_u64() {
                    T::try_from(u).map(Self).map_err(|_| de::Error::custom("u64 overflow"))
                } else {
                    Err(de::Error::custom("expected integer"))
                }
            }
            _ => Err(de::Error::custom("expected stringified or numeric integer")),
        }
    }
}

/// Wraps a `bool` serialized as `"true"` / `"false"` in JSON and a 1-byte `bool` in `postcard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StrBool(pub bool);

impl Serialize for StrBool {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(if self.0 { "true" } else { "false" })
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for StrBool {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            return bool::deserialize(deserializer).map(Self);
        }
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Bool(b) => Ok(Self(b)),
            serde_json::Value::Number(n) => Ok(Self(n.as_i64().unwrap_or(0) != 0)),
            serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(Self(true)),
                "false" | "0" | "" => Ok(Self(false)),
                other => Err(de::Error::custom(format!("invalid boolean `{other}`"))),
            },
            _ => Err(de::Error::custom("expected boolean")),
        }
    }
}

/// Parses a JSON vector while tolerating empty/`"null"` strings and trailing commas before `]`
/// (such as `array_head_2_frames` row `id = 46`: `"[1705,1706,1707,]"` in `database team2026.sql`).
pub fn parse_lenient_json_vec<T: DeserializeOwned>(column: &'static str, raw: &str) -> DbResult<Vec<T>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return Ok(Vec::new());
    }
    if let Ok(parsed) = serde_json::from_str::<Vec<T>>(trimmed) {
        return Ok(parsed);
    }
    let normalized = trimmed.replace(",]", "]").replace(", ]", "]");
    serde_json::from_str::<Vec<T>>(&normalized).map_err(|source| DbError::JsonColumn { column, source })
}

/// Variable-length positional JSON array reader matching Java `String.valueOf(JSONArray.get(i))`.
#[derive(Debug, Clone, Default)]
pub struct PositionalArray(Vec<serde_json::Value>);

impl PositionalArray {
    pub fn parse(column: &'static str, raw: &str) -> DbResult<Self> {
        parse_lenient_json_vec(column, raw).map(Self)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn i64_or(&self, idx: usize, default: i64) -> i64 {
        match self.0.get(idx) {
            Some(serde_json::Value::Number(n)) => n
                .as_i64()
                .or_else(|| n.as_u64().map(|u| u.min(i64::MAX as u64) as i64))
                .or_else(|| {
                    n.as_f64()
                        .filter(|f| f.is_finite())
                        .map(|f| f.clamp(i64::MIN as f64, i64::MAX as f64) as i64)
                })
                .unwrap_or(default),
            Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(default),
            _ => default,
        }
    }

    #[must_use]
    pub fn i32_or(&self, idx: usize, default: i32) -> i32 {
        self.i64_or(idx, i64::from(default)).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    #[must_use]
    pub fn bool_or(&self, idx: usize, default: bool) -> bool {
        match self.0.get(idx) {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
            Some(serde_json::Value::String(s)) => matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1"),
            _ => default,
        }
    }

    #[must_use]
    pub fn opt_string(&self, idx: usize) -> Option<String> {
        match self.0.get(idx) {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

/// Unquoted bracketed 5-tuple stored in `clan.thongTinLeader`: `"[id,name,head,body,leg]"`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnquotedLeaderInfo {
    pub id: i32,
    pub name: String,
    pub head: i16,
    pub body: i16,
    pub leg: i16,
}

impl UnquotedLeaderInfo {
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        let clean: String = self.name.chars().filter(|&c| !matches!(c, ',' | '[' | ']')).collect();
        format!("[{},{},{},{},{}]", self.id, clean, self.head, self.body, self.leg)
    }

    #[must_use]
    pub fn from_wire_str(raw: &str) -> Self {
        let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
        let p: Vec<&str> = inner.split(',').collect();
        if p.len() < 5 {
            return Self::default();
        }
        let n = p.len();
        Self {
            id: p[0].trim().parse().unwrap_or(0),
            name: p[1..n - 3].join(",").trim_matches('"').to_owned(),
            head: p[n - 3].trim().parse().unwrap_or(0),
            body: p[n - 2].trim().parse().unwrap_or(0),
            leg: p[n - 1].trim().parse().unwrap_or(0),
        }
    }
}

/// Unquoted bracketed 4-tuple stored in `player.thanhTichBang`, `thanhTichKhiGas`, `thanhTichCDRD`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnquotedDungeonRecord {
    pub clan_name: String,
    pub level_done: i32,
    pub duration_ms: i64,
    pub updated_at_ms: i64,
}

impl UnquotedDungeonRecord {
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        if self.level_done <= 0 && self.clan_name.is_empty() {
            "[0,0,0,0]".to_owned()
        } else {
            let safe: String = self.clan_name.chars().filter(|&c| !matches!(c, ',' | '[' | ']')).collect();
            format!("[{},{},{},{}]", safe, self.level_done, self.duration_ms, self.updated_at_ms)
        }
    }

    #[must_use]
    pub fn from_wire_str(raw: &str) -> Self {
        let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
        let p: Vec<&str> = inner.splitn(4, ',').collect();
        if p.len() < 4 {
            return Self::default();
        }
        Self {
            clan_name: p[0].trim_matches('"').to_owned(),
            level_done: p[1].trim().parse().unwrap_or(0),
            duration_ms: p[2].trim().parse().unwrap_or(0),
            updated_at_ms: p[3].trim().parse().unwrap_or(0),
        }
    }
}

pub use account_repo::{AccountRepo, AccountRow, AuthenticatedAccount};
pub use engine::{FjallConfig, FjallEngine, SchemaHealthReport};
pub use legacy_sql::{ImportManifest, ImportOptions, import_mysqldump_file, import_mysqldump_str};
pub use player_repo::{
    LeaderboardEntryRecord, LeaderboardIndexKind, NewPlayerSeed, PlayerRepo, PlayerRow,
    PlayerSnapshot,
};
pub use pool::{DbPool, DbPoolConfig};
pub use templates::GameTemplates;
pub use world_repos::{
    ClanRepo, ConsignShopRepo, EventRepo, GiftCodeRepo, HistoryTransactionRepo, SuperRankRepo,
};
