//! Shared domain identifiers, entity keys, item/stat primitives, parity modes, and safe PRNG
//! across the Teamobi2026 Ngọc Rồng Online Safe Rust server workspace.

#![forbid(unsafe_code)]

use std::fmt;
use std::ops::Neg;

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

slotmap::new_key_type! {
    /// Generational key addressing an entity inside a `ZoneActor` `SlotMap`.
    pub struct EntityKey;
    /// Generational key addressing a connected client session.
    pub struct SessionKey;
}

/// Database account identifier (`account.id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct AccountId(pub i32);

/// Character or boss entity identifier (`player.id`; negative values denote bosses/clones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct PlayerId(pub i64);

/// Clan identifier (`clan.id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ClanId(pub i32);

/// Map template identifier (`map_template.id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct MapId(pub i16);

/// Zone instance index within a map (`0..zone_count`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ZoneId(pub u16);

/// Item, skill, mob, or effect template identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct TemplateId(pub i16);

/// Boss archetype identifier (`BossID.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct BossTypeId(pub i32);

impl fmt::Display for AccountId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for PlayerId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for ClanId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for MapId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for ZoneId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for TemplateId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl fmt::Display for BossTypeId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }

impl From<i32> for AccountId { #[inline] fn from(v: i32) -> Self { Self(v) } }
impl From<i32> for PlayerId { #[inline] fn from(v: i32) -> Self { Self(i64::from(v)) } }
impl From<i64> for PlayerId { #[inline] fn from(v: i64) -> Self { Self(v) } }
impl Neg for PlayerId { type Output = Self; #[inline] fn neg(self) -> Self { Self(-self.0) } }
impl From<i32> for ClanId { #[inline] fn from(v: i32) -> Self { Self(v) } }
impl From<u8> for MapId { #[inline] fn from(v: u8) -> Self { Self(i16::from(v)) } }
impl From<i16> for MapId { #[inline] fn from(v: i16) -> Self { Self(v) } }
impl From<u8> for ZoneId { #[inline] fn from(v: u8) -> Self { Self(u16::from(v)) } }
impl From<u16> for ZoneId { #[inline] fn from(v: u16) -> Self { Self(v) } }
impl From<i16> for TemplateId { #[inline] fn from(v: i16) -> Self { Self(v) } }
impl From<i32> for BossTypeId { #[inline] fn from(v: i32) -> Self { Self(v) } }

/// Playable character planet/lineage (`ConstPlayer.TRAI_DAT / NAMEC / XAYDA`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::FromRepr)]
#[repr(u8)]
pub enum Gender {
    /// Earthling (`0` - Trái Đất).
    #[default]
    TraiDat = 0,
    /// Namekian (`1` - Namek).
    Namec = 1,
    /// Saiyan (`2` - Xayda).
    Xayda = 2,
}

impl Gender {
    /// Converts a raw byte into [`Gender`], defaulting out-of-range values to [`Gender::TraiDat`].
    #[must_use]
    pub const fn from_i8_lossy(v: i8) -> Self {
        match v {
            1 => Self::Namec,
            2 => Self::Xayda,
            _ => Self::TraiDat,
        }
    }
}

impl TryFrom<i8> for Gender {
    type Error = CoreError;
    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::TraiDat),
            1 => Ok(Self::Namec),
            2 => Ok(Self::Xayda),
            _ => Err(CoreError::InvalidParameter("gender must be 0, 1, or 2")),
        }
    }
}

impl From<Gender> for i8 { #[inline] fn from(g: Gender) -> Self { g as i8 } }
impl From<Gender> for u8 { #[inline] fn from(g: Gender) -> Self { g as u8 } }
impl From<Gender> for i16 { #[inline] fn from(g: Gender) -> Self { g as i16 } }

/// Controls whether formulas use hardened bug-fixed math (`Fixed`) or reproduce legacy Java quirks (`Legacy`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParityMode {
    /// Uses 64-bit saturating arithmetic, fixed array indices, and hardened dupe/payout guards.
    #[default]
    Fixed,
    /// Reproduces upstream Java integer truncations and wire quirks for differential A/B tests.
    Legacy,
}

/// Single modifier attached to an [`ItemInstance`] (`Item.ItemOption` in Java).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemOption {
    /// Option template ID (`item_option_template.id`).
    pub id: u16,
    /// Option magnitude (`param`).
    pub param: i32,
}

impl ItemOption {
    /// Constructs an `(id, param)` item option pair.
    #[inline]
    #[must_use]
    pub const fn new(id: u16, param: i32) -> Self {
        Self { id, param }
    }
}

/// Concrete item stack in body, bag, box, or ground drop (`Item.java`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemInstance {
    /// Template ID (`item_template.id`).
    pub template_id: TemplateId,
    /// Stack quantity (`>= 1` for valid items).
    pub quantity: u32,
    /// Inline small-vector of item options (zero heap allocs for `<= 4` options).
    pub options: SmallVec<[ItemOption; 4]>,
    /// Unix epoch creation timestamp in milliseconds (`createTime`).
    pub create_time_ms: i64,
}

impl ItemInstance {
    /// Constructs a clean item stack with empty options and `create_time_ms = 0`.
    #[inline]
    #[must_use]
    pub fn new(template_id: TemplateId, quantity: u32) -> Self {
        Self {
            template_id,
            quantity,
            options: SmallVec::new(),
            create_time_ms: 0,
        }
    }

    /// Constructs an item stack with explicit options and creation timestamp.
    #[must_use]
    pub fn with_options(
        template_id: TemplateId,
        quantity: u32,
        options: impl IntoIterator<Item = ItemOption>,
        create_time_ms: i64,
    ) -> Self {
        Self {
            template_id,
            quantity,
            options: options.into_iter().collect(),
            create_time_ms,
        }
    }

    /// Returns `true` if option `30` (`Không thể giao dịch`) is present or template is `570` (`Rương Gỗ`).
    #[must_use]
    pub fn is_untradeable(&self) -> bool {
        self.template_id.0 == 570 || self.options.iter().any(|o| o.id == 30)
    }
}

/// World coordinates (`Location.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Location {
    /// Map ID.
    pub map_id: MapId,
    /// Zone instance index.
    pub zone_id: ZoneId,
    /// Horizontal pixel coordinate.
    pub x: i16,
    /// Vertical pixel coordinate.
    pub y: i16,
}

impl Location {
    /// Constructs a [`Location`] at `(x, y)` with default `MapId(0)` and `ZoneId(0)`.
    #[inline]
    #[must_use]
    pub const fn new(x: i16, y: i16) -> Self {
        Self { map_id: MapId(0), zone_id: ZoneId(0), x, y }
    }
}

/// Unified persisted & effective combat attributes (`NPoint.java` + `data_point` JSON array).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreStats {
    /// Power cap tier (`0..=9`).
    pub limit_power: u8,
    /// Total power (`sức mạnh`).
    pub power: i64,
    /// Unspent potential (`tiềm năng`).
    pub tiem_nang: i64,
    /// Current stamina (`stamina`).
    pub stamina: u16,
    /// Maximum stamina (`maxStamina`).
    pub max_stamina: u16,
    /// Base HP (`hpg`).
    pub hpg: i64,
    /// Base Ki/MP (`mpg`).
    pub mpg: i64,
    /// Base attack damage (`dameg`).
    pub dameg: i64,
    /// Base defense (`defg`).
    pub defg: i32,
    /// Base critical chance % (`critg`).
    pub critg: u8,
    /// Permanent Shenron critical bonus (`critdragon`).
    pub critdragon: i8,
    /// Current HP (`hp`).
    pub hp: i64,
    /// Effective maximum HP (`hpMax`).
    pub hp_max: i64,
    /// Current Ki/MP (`mp`).
    pub mp: i64,
    /// Effective maximum Ki/MP (`mpMax`).
    pub mp_max: i64,
    /// Effective attack damage (`dame`).
    pub dame: i64,
    /// Effective flat defense (`def`).
    pub def: i32,
    /// Effective critical chance % (`crit`).
    pub crit: u8,
    /// Dodge chance % (`tlNeDon`).
    pub dodge: u8,
    /// Movement speed (`speed`, default `8`).
    pub speed: u8,
}

impl Default for CoreStats {
    fn default() -> Self {
        Self {
            limit_power: 0,
            power: 2_000,
            tiem_nang: 2_000,
            stamina: 1_000,
            max_stamina: 1_000,
            hpg: 200,
            mpg: 100,
            dameg: 15,
            defg: 0,
            critg: 0,
            critdragon: 0,
            hp: 200,
            hp_max: 200,
            mp: 100,
            mp_max: 100,
            dame: 15,
            def: 0,
            crit: 0,
            dodge: 0,
            speed: 8,
        }
    }
}

/// Deterministic, crash-safe PRNG wrapper replacing `nro.models.utils.Util` random helpers.
///
/// Normalizes inverted bounds such as `Util.nextInt(20, 10)` in `RewardService.rewardCapsuleTet`
/// so malformed legacy ranges never panic at runtime.
#[derive(Debug, Clone)]
pub struct GameRng {
    inner: ChaCha8Rng,
}

impl GameRng {
    /// Creates a deterministic `ChaCha8Rng` seeded from `seed`.
    #[inline]
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self { inner: ChaCha8Rng::seed_from_u64(seed) }
    }

    /// Alias of [`Self::from_seed`] used by economy/shop test harnesses.
    #[inline]
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self::from_seed(seed)
    }

    /// Alias of [`Self::from_seed`].
    #[inline]
    #[must_use]
    pub fn from_seed_u64(seed: u64) -> Self {
        Self::from_seed(seed)
    }

    /// Raw 64-bit random draw.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.inner.random()
    }

    /// Raw 32-bit random draw.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.inner.random()
    }

    /// Inclusive `i32` draw (`Util.nextInt(a, b)`), safely ordering `(min, max)` if `a > b`.
    #[inline]
    pub fn next_range(&mut self, a: i32, b: i32) -> i32 {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.inner.random_range(lo..=hi)
    }

    /// Alias of [`Self::next_range`] matching `Util.nextInt(a, b)`.
    #[inline]
    pub fn next_int(&mut self, a: i32, b: i32) -> i32 {
        self.next_range(a, b)
    }

    /// Inclusive `i64` draw (`[a, b]`), safely ordering `(min, max)` if `a > b`.
    #[inline]
    pub fn next_i64(&mut self, a: i64, b: i64) -> i64 {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.inner.random_range(lo..=hi)
    }

    /// Returns `true` with probability `numerator / denominator` (`Util.isTrue`).
    #[inline]
    pub fn is_true(&mut self, numerator: u32, denominator: u32) -> bool {
        if denominator == 0 || numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }
        self.inner.random_range(0..denominator) < numerator
    }

    /// Alias of [`Self::is_true`].
    #[inline]
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        self.is_true(numerator, denominator)
    }
}

/// Foundational domain error shared across crates.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// Invalid identifier or parameter boundary.
    #[error("invalid domain parameter: {0}")]
    InvalidParameter(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_rng_handles_inverted_bounds_without_panicking() {
        let mut rng = GameRng::from_seed(2026);
        let v = rng.next_int(20, 10);
        assert!((10..=20).contains(&v));
        assert_eq!(Gender::try_from(1_i8), Ok(Gender::Namec));
        assert_eq!(-PlayerId(42), PlayerId(-42));
    }
}
