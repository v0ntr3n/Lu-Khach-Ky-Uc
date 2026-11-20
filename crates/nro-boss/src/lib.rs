//! Safe, deterministic, data-driven Boss, BigBoss, Mob, and RewardService engine.
//!
//! Ports all 176 legacy Java classes (`159` in `models/boss/`, `3` in
//! `models/boss_con_duong_ran_doc/`, `5` in `models/mob/`, and `9` in `models/mob_bigboss/`)
//! plus `RewardService.java` (`1,762` lines) into `#![forbid(unsafe_code)]` Rust (`1.100.0`,
//! `edition = "2024"`).

#![forbid(unsafe_code)]

pub mod big_boss;
pub mod catalog;
pub mod clothes;
pub mod combat_ai;
pub mod lifecycle;
pub mod mechanics;
pub mod mob;
pub mod rewards;
pub mod spawner;

#[cfg(test)]
mod tests;

pub use big_boss::{BigBossKind, BigBossState, MobEffectState, MobMapKind, MobMeState, MobState};
pub use catalog::builtin_boss_catalog;
pub use clothes::{ACTIVATION_SETS, ClothesStatProfile};
pub use combat_ai::run_boss_combat_tick;
pub use lifecycle::{AppearType, BossStatus, LifecycleState, StageConfig, WaveRole};
pub use mechanics::{BossArchetype, DamageOutcome, DamageRule, DungeonKind, DungeonParams};
pub use nro_consts as consts;
pub use nro_core::{
    BossTypeId, ClanId, CoreStats, GameRng, Gender, ItemInstance, ItemOption, Location, MapId,
    ParityMode, PlayerId, TemplateId, ZoneId,
};
pub use rewards::{BossLootProfile, MapRegions, PlayerDropCtx, RewardEngine};
pub use spawner::{BossDefinition, BossGroupManager, BossRuntime};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumIter, EnumString, FromRepr};
use thiserror::Error;

/// Domain errors emitted by boss configuration loading, inventory checks, and legacy bug traps.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BossError {
    /// A `BossDefinition` contained zero stage configurations.
    #[error("boss definition {0:?} must declare at least one stage")]
    EmptyStages(BossTypeId),
    /// Player lacked the required empty bag slot for direct NPC/event reward delivery.
    #[error("at least one empty bag slot is required")]
    EmptyBagSlotRequired,
    /// Strict Java parity reproduced `Util.nextInt(20, 10)` negative bound exception.
    #[error("legacy Java RNG bound crash: next_int({from}, {to}) has from > to")]
    LegacyBoundsCrash { from: i32, to: i32 },
    /// Requested boss archetype ID was not registered in the catalog.
    #[error("unknown boss definition {0:?}")]
    UnknownBossDefinition(BossTypeId),
    /// JSON configuration deserialization or serialization failed.
    #[error("boss catalog JSON error: {0}")]
    JsonConfig(String),
}

bitflags! {
    /// Behavioral bitflags controlling spawn placement, announcements, pets, and despawn timers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct BossFlags: u16 {
        /// Broadcasts global server announcement (`ServerNotify`) upon map entry.
        const NOTIFY_GLOBAL         = 1 << 0;
        /// Disables spawning in zones `0` and `1` (`BlackGoku`, `Cumber`, `Broly`).
        const ZONE_01_DISABLED      = 1 << 1;
        /// Restricts spawning exclusively to `zone_id == 0` (`AnTrom`).
        const ZONE_ZERO_ONLY        = 1 << 2;
        /// Restores full MP on every tick (`The23rdMartialArtCongress`, `DeathOrAliveArena`).
        const IMMORTAL_MP           = 1 << 3;
        /// Marks entity as a player clone (`NhanBan`, `Rival`).
        const IS_COPY               = 1 << 4;
        /// Spawns an accompanying AI normal pet on `JoinMap` (`SuperBroly`).
        const HAS_NORMAL_PET        = 1 << 5;
        /// Leaves map after `900s` (or `300s`) only when the zone has zero players.
        const EMPTY_ZONE_DESPAWN    = 1 << 6;
        /// Legacy Nappa/Ginyu bug: leaves map after `900s` unconditionally unless fixed.
        const LEGACY_UNCOND_DESPAWN = 1 << 7;
        /// Clears stun automatically on every tick (`ThienXinHang`, `ThienXinHangClone`).
        const STUN_IMMUNE           = 1 << 8;
        /// Disposes the runtime instance permanently upon `LeaveMap` (`SuperBroly`, dungeons).
        const DISPOSE_ON_LEAVE      = 1 << 9;
    }
}

/// All 16 legacy `Boss_Manager` subsystems unified under [`BossGroupManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumIter, EnumString, Serialize, Deserialize)]
pub enum ManagerGroup {
    /// Core world bosses (`BossManager.java`).
    WorldBoss,
    /// `BrolyManager` (`30x` Broly + dynamic `SuperBroly`).
    Broly,
    /// `SkillSummonedManager` (`DeathBeam 1..5`).
    SkillSummoned,
    /// `YardartManager` (`18` patrol clones across Maps `131..=133`).
    Yardrat,
    /// `GoldenFriezaManager` (`21:00..=21:59` ICT).
    GoldenFrieza21h,
    /// `FinalBossManager` (`12:00` Babidi spaceship).
    MajinBuu12h,
    /// `MajinBuu14HManager` (`14:00` Map `127/128`).
    MajinBuu14h,
    /// `RedRibbonHQManager` (`Doanh Trại`).
    RedRibbonDungeon,
    /// `TreasureUnderSeaManager` (`Bản Đồ Kho Báu`).
    TreasureMapDungeon,
    /// `SnakeWayManager` (`Con Đường Rắn Độc`).
    SnakeWayDungeon,
    /// `GasDestroyManager` (`Khí Gas Hủy Diệt`).
    DestronGasDungeon,
    /// `OtherBossManager` (DHVT, Hạt Mít, Training, SuperRank, Commeson, Piccolo).
    OtherInstanced,
    /// `HalloweenEventManager` (`BiMa`, `Doi`, `MaTroi` — wired!).
    HalloweenEvent,
    /// `ChristmasEventManager` (`OngGiaNoel` — wired!).
    ChristmasEvent,
    /// `HungVuongEventManager` (`SonTinh`, `ThuyTinh` — wired!).
    HungVuongEvent,
    /// `TrungThuEventManager` + `TetEventManager` (`KhiDot`, `NguyetThan`, `NhatThan`, `LanCon` — wired!).
    TrungThuAndTetEvent,
}

/// Seasonal festival switches enabling the 5 event boss groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeasonalEvent {
    /// Halloween festival.
    Halloween,
    /// Christmas festival.
    Christmas,
    /// Hùng Vương Commemoration Day.
    HungVuong,
    /// Mid-Autumn Festival (`Trung Thu`).
    TrungThu,
    /// Lunar New Year (`Tết Nguyên Đán`).
    Tet,
}

/// Time-of-day or festival gate controlling `BossStatus::Rest -> Respawn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeWindowGate {
    /// Always eligible once `rest_seconds` elapses.
    #[default]
    Always,
    /// `21:00..=21:59` ICT (`GoldenFrieza`).
    Hour21Ict,
    /// `12:00` Mabu spaceship window.
    Mabu12hWindow,
    /// `14:00` Mabu raid window.
    Mabu14hWindow,
    /// Enabled seasonal festival gate.
    Seasonal(SeasonalEvent),
}

/// Player/Boss PK stance mirroring `ConstPlayer.NON_PK`, `PK_PVP`, and `PK_ALL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Display, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum PkMode {
    /// Peaceful / untargetable (`ConstPlayer.NON_PK = 0`).
    #[default]
    NonPk = 0,
    /// 1v1 arena duel (`ConstPlayer.PK_PVP = 3`).
    PkPvp = 3,
    /// Hostile to all non-boss players in zone (`ConstPlayer.PK_ALL = 5`).
    PkAll = 5,
}

/// Canonical skill identifiers mirroring `nro.models.skill.Skill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum SkillKind {
    /// Melee Dragon (`0`), Kamejoko (`1`), Demon (`2`), Masenko (`3`), Galick (`4`), Antomic (`5`).
    Dragon = 0,
    /// Kamejoko Ki beam (`1`).
    Kamejoko = 1,
    /// Melee Demon strike (`2`).
    Demon = 2,
    /// Masenko Ki beam (`3`).
    Masenko = 3,
    /// Melee Galick strike (`4`).
    Galick = 4,
    /// Antomic Ki beam (`5`).
    Antomic = 5,
    /// Solar Flare AoE blind (`6`).
    ThaiDuongHaSan = 6,
    /// Namekian heal (`7`).
    TriThuong = 7,
    /// Energy regeneration (`8`).
    TaiTaoNangLuong = 8,
    /// Kaioken strike (`9`).
    Kaioken = 9,
    /// Spirit Bomb (`10` — required finisher for Stage 4 `KidBu`).
    QuaCauKenhKhi = 10,
    /// Special Beam Cannon (`11`).
    Makankosappo = 11,
    /// Spawn `MobMe` egg familiar (`12`).
    DeTrung = 12,
    /// Great Ape Oozaru transformation (`13`).
    BienKhi = 13,
    /// Self-destruct explosion (`14` — bypasses Broly `1%` cap, blocked by `Drabura`).
    TuSat = 14,
    /// Multi-hit barrage (`17`).
    LienHoan = 17,
    /// Turn target into chocolate (`18`).
    Socola = 18,
    /// Energy shield (`19`).
    KhienNangLuong = 19,
    /// Instant transmission (`20`).
    DichChuyenTucThoi = 20,
    /// Whistle max-HP buff (`21`).
    HuytSao = 21,
    /// Hypnosis sleep (`22`).
    ThoiMien = 22,
    /// Telekinetic bind (`23`).
    Troi = 23,
    /// Super Kamehameha (`24`).
    SuperKame = 24,
    /// Continuous energy volley (`25`).
    LienHoanChuong = 25,
    /// Evil Containment Wave jar DoT (`26`).
    MaPhongBa = 26,
}

impl SkillKind {
    /// Returns `true` for the three primary Ki-blast skills absorbed by Android 19 & Dr. Kore.
    #[must_use]
    pub const fn is_ki_blast(self) -> bool {
        matches!(self, Self::Kamejoko | Self::Masenko | Self::Antomic)
    }

    /// Returns `true` for melee punches that deal `2x` damage to Babidi's spaceship guardians.
    #[must_use]
    pub const fn is_melee(self) -> bool {
        matches!(self, Self::Dragon | Self::Demon | Self::Galick | Self::Kaioken)
    }

    /// Effective cast range in pixels matching `Boss.getRangeCanAttackWithSkillSelect()`.
    #[must_use]
    pub const fn attack_range(self) -> i32 {
        match self {
            Self::Dragon | Self::Demon | Self::Galick | Self::LienHoan | Self::Kaioken => 100,
            Self::DichChuyenTucThoi => 400,
            _ => 300,
        }
    }
}

/// Snapshot of an attacking or nearby player supplied to pure boss/mob evaluation functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerView {
    /// Unique player entity identifier.
    pub id: PlayerId,
    /// Optional clan membership for dungeon scaling and `GauTuongCuop` capsule splits.
    pub clan_id: Option<ClanId>,
    /// Player planet gender (`Earth`, `Namek`, `Saiyan`).
    pub gender: Gender,
    /// Spatial coordinates within the zone.
    pub location: Location,
    /// Combat stats (`hp`, `hp_max`, `dame`, `def`, `crit`, `dodge`).
    pub stats: CoreStats,
    /// Power level (`nPoint.power`).
    pub power: i64,
    /// Active faction flag (`cFlag`).
    pub flag: u8,
    /// Current gold carried (`inventory.gold`) for `AnTrom` pickpocketing.
    pub gold: u64,
    /// Whether the player already owns a Pet (`SuperBroly` reward gate).
    pub has_pet: bool,
    /// Whether the player is carrying a Namek Dragon Ball (`idNRNM != -1`).
    pub carrying_namek_ball: bool,
    /// Active quest ID `(task_id, sub_task_id)`.
    pub active_task: (u16, u8),
    /// Current Whis training leaderboard top level.
    pub whis_top_level: u32,
    /// Last chat message spoken in zone (used by `LanCon` `"thang"` taming).
    pub last_chat: Option<String>,
}

/// Ground item drop descriptor produced by boss, big-boss, or mob death/phase transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundDrop {
    /// Item instance (`template_id`, `quantity`, `options`).
    pub item: ItemInstance,
    /// Ground drop coordinates.
    pub location: Location,
    /// Exclusive owner (`None` represents public loot `playerId == -1`).
    pub owner: Option<PlayerId>,
}

/// Pure, side-effect-free engine events emitted during boss, mob, and big-boss ticks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineEffect {
    /// Boss or mob spoke overhead chat line.
    Chat { speaker: PlayerId, text: String },
    /// Boss switched PK mode (`NonPk`, `PkPvp`, `PkAll`).
    ChangePk { entity: PlayerId, mode: PkMode },
    /// Spawned a ground `ItemMap` drop.
    DropItem(GroundDrop),
    /// Requested dynamic boss spawn (`SuperBroly`, `Hatchiyack`, `Drabura3`, clones).
    SpawnDynamicBoss { boss_type: BossTypeId, map_id: MapId, zone_id: ZoneId, location: Location, scaled_stats: Option<(i64, i64)> },
    /// Inflicted damage or piercing lethal strike on a player.
    DamagePlayer { target: PlayerId, damage: i64, piercing: bool },
    /// Applied timed crowd-control or event disguise to a player.
    DebuffPlayer { target: PlayerId, kind: PlayerDebuffKind, duration_ms: u32 },
    /// Teleported a player to a new map/location (`Mabu2H` swallow to Map `128`).
    TeleportPlayer { target: PlayerId, map_id: MapId, zone_id: ZoneId, location: Location },
    /// Stole gold from a player (`AnTrom`).
    StealGold { target: PlayerId, amount: u64 },
    /// Awarded power/potential (`TNSM`) to a player (`TaoPaiPai`, `ToSuKaio`, `Mob`).
    GrantTnsm { target: PlayerId, amount: i64 },
    /// Awarded seasonal/boss event points (`+5` points).
    GrantEventPoints { target: PlayerId, points: u32 },
    /// Created a normal pet for a petless killer (`SuperBroly`).
    GrantNormalPet { target: PlayerId },
    /// Distributed Clan Capsules from `GauTuongCuop` (`(player_id, clan_id, capsules)`).
    DistributeClanCapsules(SmallVec<[(PlayerId, ClanId, u32); 8]>),
    /// Awarded `MayDoSucManh` punching-machine milestone points.
    AwardMayDamPoints { target: PlayerId, points: u32 },
    /// Killed all regular mobs in zone (`GoldenFrieza` 21h entry).
    WipeZoneMobs { map_id: MapId, zone_id: ZoneId },
    /// Unlocked Super Divine Water (`player.winSTT = true` from `Pocolo`).
    UnlockSuperDivineWater { target: PlayerId },
}

/// Timed crowd-control and status ailments applied by bosses to players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerDebuffKind {
    /// Solar Flare / stun (`THAI_DUONG_HA_SAN`, `ChanXu`, `SAIBAMEN`, `CADICH`).
    Stun,
    /// Dabura `22,000ms` stone petrification.
    PetrifyStone,
    /// Majin Buu `30,000ms` chocolate polymorph.
    Chocolate,
    /// Captain Ginyu body-change (`TDT`, `TDT_NM`).
    BodyChange,
    /// Pui Pui Floor 3 gravity slow (`5,000ms`).
    GravitySlow,
    /// Halloween `1,800,000ms` costume curse (`2 = BiMa, 3 = Doi, 4 = MaTroi`).
    HalloweenCurse(u8),
    /// `MatTroi` solar burn (`30s`) or `Virut` viral infection (`300s`).
    Contagion { lethal_on_expire: bool },
}

/// Computes integer Euclidean distance matching `Util.getDistance(x1, y1, x2, y2)`.
#[must_use]
pub fn loc_distance(a: Location, b: Location) -> i32 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    ((dx * dx + dy * dy) as f64).sqrt() as i32
}
