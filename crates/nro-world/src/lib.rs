//! Safe, deterministic world runtime (`nro-world`) ported from `Teamobi2026/SRC/src/nro`.
//!
//! # Architecture Overview
//! - [`s2c_packets`]: Complete server-to-client binary [`nro_protocol::Packet`] builders for
//!   `Service.java`, `ChangeMapService.java`, `Zone.java`, `Mob.java`, `ChatGlobalService.java`,
//!   `SubMenuService.java`, and `NpcService.java`.
//! - [`zone_actor`]: Isolated single-threaded [`zone_actor::ZoneActor`] ticking every `50 ms`
//!   over cache-friendly `slotmap::SlotMap` + Struct-of-Arrays (SoA) slices, with deadlock-free
//!   two-phase `ChangeMap` handoffs across `DashMap<(MapId, ZoneId), mpsc::Sender<ZoneCommand>>`.
//! - [`dungeons`]: State machines and schedules for all 6 `phoban` dungeons (`BanDoKhoBau`,
//!   `BlackBallWar`, `DestronGas`, `MajinBuu14H`, `RedRibbonHQ`, `SnakeWay`) plus Namek Dragon
//!   Balls (`NgocRongNamecService`), Majin Buu 12h, and Super Divine Water (`TayKarin`).
//! - [`pvp_tournaments`]: Open-world PvP (`ThachDau` with fixed single loser deduction,
//!   `LuyenTap`, `TraThu`, `PKCommeson`) and all 4 Martial Arts Arenas (`DHVT23`,
//!   `DeathOrAliveArena`, `WorldMartialArts`, and `SuperRank`).
//! - [`shenron_events`]: Earth, Namek, and Ice Shenron wish engines (completing `SHENRON_1_3`),
//!   seasonal crafting (`NoiBanh` with hardened stack-quantity checks, `VuaHung`, `XeNuocMia`),
//!   and stake-weighted `ChonAiDay` / `ConSoMayMan` lottery engines.

#![forbid(unsafe_code)]

pub mod dungeons;
pub mod pvp_tournaments;
pub mod s2c_packets;
pub mod shenron_events;
pub mod zone_actor;

use std::sync::Arc;

use dashmap::DashMap;
use nro_consts::{const_map, const_task};
use nro_core::{EntityKey, MapId, TemplateId, ZoneId};
use nro_protocol::PacketError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, warn};

pub use dungeons::{
    BanDoKhoBauInstance, BlackBallBuff, BlackBallWarEngine, DestronGasInstance, DungeonExit,
    MajinBuu12hFloor, MajinBuu14hInstance, NamekDragonBallTracker, RedRibbonHqInstance,
    SkillBeamKind, SnakeWayInstance, SuperDivineWaterSession,
};
pub use pvp_tournaments::{
    DeathOrAliveSession, Dhvt23Session, PvpMode, PvpResolution, PvpSession, SuperRankArena,
    SuperRankOutcome, TournamentTier, WorldMartialArtsBracket,
};
pub use shenron_events::{
    CakeKind, ChonAiDayPool, ConSoMayManRound, DragonSummonSession, EarthShenronMenu,
    IceShenronWish, NamekPorungaWish, SeasonalCrafter, ShenronEffect, VuaHungGiftKind,
};
pub use zone_actor::{
    EntityKind, GroundDropKind, GroundItem, PlayerIngress, TrapVolume, ZoneActor, ZoneCommand,
    ZoneConfig, ZoneEvent, ZoneMobSeed, ZoneSoaArena,
};

/// Domain errors across map routing, zone actors, dungeons, PvP arenas, Shenron, and events.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorldError {
    /// Underlying binary packet serialization failed.
    #[error("packet serialization error: {0}")]
    Protocol(#[from] PacketError),
    /// Requested `(MapId, ZoneId)` is not registered in the [`WorldRouter`].
    #[error("target zone ({map_id:?}, {zone_id:?}) does not exist")]
    ZoneNotFound {
        /// Destination map identifier.
        map_id: MapId,
        /// Destination zone identifier.
        zone_id: ZoneId,
    },
    /// Destination zone has reached its player capacity (`maxPlayer`).
    #[error("zone ({map_id:?}, {zone_id:?}) is full (limit {max_players})")]
    ZoneFull {
        /// Destination map identifier.
        map_id: MapId,
        /// Destination zone identifier.
        zone_id: ZoneId,
        /// Configured player cap for the zone.
        max_players: u8,
    },
    /// Actor mailbox for the target zone has closed.
    #[error("zone ({map_id:?}, {zone_id:?}) command channel closed")]
    ZoneChannelClosed {
        /// Destination map identifier.
        map_id: MapId,
        /// Destination zone identifier.
        zone_id: ZoneId,
    },
    /// Referenced [`EntityKey`] is no longer present in the zone arena.
    #[error("entity {0:?} not found in zone")]
    EntityNotFound(EntityKey),
    /// Progression or state rule blocked entry into `map_id` (`ChangeMapService.checkMapCanJoin`).
    #[error("cannot enter map {map_id:?}: {reason}")]
    MapGateLocked {
        /// Blocked map identifier.
        map_id: MapId,
        /// Human-readable gate rejection reason.
        reason: &'static str,
    },
    /// Action rejected due to active per-player or global cooldown.
    #[error("cooldown active: {remaining_ms} ms remaining")]
    CooldownActive {
        /// Remaining wait duration in milliseconds.
        remaining_ms: u64,
    },
    /// Dungeon entry or progression precondition failed.
    #[error("dungeon rule violation: {0}")]
    DungeonRule(&'static str),
    /// PvP or Martial Arts Tournament precondition failed.
    #[error("arena rule violation: {0}")]
    ArenaRule(&'static str),
    /// Insufficient gold, green gems, or rubies for an operation.
    #[error("insufficient {kind}: need {required}, have {available}")]
    InsufficientCurrency {
        /// Currency name (`"gold"`, `"gem"`, or `"ruby"`).
        kind: &'static str,
        /// Required balance.
        required: u64,
        /// Current player balance.
        available: u64,
    },
    /// Missing or insufficient stack quantity for a required item template.
    #[error("insufficient item {template_id:?}: need {required}, have {available}")]
    InsufficientItem {
        /// Required item template ID.
        template_id: TemplateId,
        /// Required stack count.
        required: u32,
        /// Currently held stack count.
        available: u32,
    },
    /// Invalid or unsatisfied Shenron wish selection.
    #[error("shenron wish error: {0}")]
    InvalidWish(&'static str),
    /// Lottery or mini-game rule violation.
    #[error("lottery rule violation: {0}")]
    LotteryRule(&'static str),
}

/// Player progression snapshot evaluated by [`WorldRouter::validate_map_entry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapGateContext {
    /// Current main quest task index (`TaskMain.id << 1 | index` in `ConstTask`).
    pub task_id: i32,
    /// Current character combat power (`NPoint.power`).
    pub power: i64,
    /// Whether the character owns an active companion pet (`Player.pet != null`).
    pub has_pet: bool,
    /// Whether the character carries `Nhẫn Thời Không` (item `992`).
    pub has_time_ring_992: bool,
    /// Whether the character currently holds a Namek Dragon Ball (`353..=359`).
    pub holding_namek_ball: bool,
    /// Whether the player has admin/VIP zone-cooldown bypass privileges.
    pub is_admin: bool,
}

/// Concurrent lock-sharded directory mapping `(MapId, ZoneId)` to live [`ZoneActor`] mailboxes.
#[derive(Clone, Debug, Default)]
pub struct WorldRouter {
    zones: Arc<DashMap<(MapId, ZoneId), mpsc::Sender<ZoneCommand>>>,
}

impl WorldRouter {
    /// Creates an empty [`WorldRouter`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            zones: Arc::new(DashMap::new()),
        }
    }

    /// Borrows the underlying `DashMap` handle for zero-lock-across-await lookups.
    #[must_use]
    pub fn map(&self) -> &Arc<DashMap<(MapId, ZoneId), mpsc::Sender<ZoneCommand>>> {
        &self.zones
    }

    /// Registers or replaces the mailbox sender for `(map_id, zone_id)`.
    pub fn register_zone(&self, map_id: MapId, zone_id: ZoneId, tx: mpsc::Sender<ZoneCommand>) {
        debug!(?map_id, ?zone_id, "registered zone actor mailbox");
        self.zones.insert((map_id, zone_id), tx);
    }

    /// Removes `(map_id, zone_id)` from the router directory.
    pub fn unregister_zone(&self, map_id: MapId, zone_id: ZoneId) {
        self.zones.remove(&(map_id, zone_id));
    }

    /// Returns the total number of active zones currently registered.
    #[must_use]
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }

    /// Clones the mailbox `Sender` for `(map_id, zone_id)` and immediately releases the shard lock.
    ///
    /// # Errors
    /// Returns [`WorldError::ZoneNotFound`] if the `(map_id, zone_id)` pair is absent.
    pub fn resolve_sender(
        &self,
        map_id: MapId,
        zone_id: ZoneId,
    ) -> Result<mpsc::Sender<ZoneCommand>, WorldError> {
        self.zones
            .get(&(map_id, zone_id))
            .map(|entry| entry.value().clone())
            .ok_or(WorldError::ZoneNotFound { map_id, zone_id })
    }

    /// Enforces `ChangeMapService.checkMapCanJoin` story, power, and item progression gates.
    ///
    /// # Errors
    /// Returns [`WorldError::MapGateLocked`] when the player has not met map requirements.
    pub fn validate_map_entry(dest_map: MapId, ctx: MapGateContext) -> Result<(), WorldError> {
        if ctx.is_admin {
            return Ok(());
        }
        let raw_map = i32::from(dest_map.0);
        match raw_map {
            92..=100 | 102 | 103 if ctx.task_id < const_task::TASK_24_0 => {
                warn!(raw_map, task_id = ctx.task_id, "blocked future map entry");
                Err(WorldError::MapGateLocked {
                    map_id: dest_map,
                    reason: "requires Future saga task >= TASK_24_0",
                })
            }
            105..=109 if ctx.task_id < const_task::TASK_29_0 => Err(WorldError::MapGateLocked {
                map_id: dest_map,
                reason: "requires Cold Planet task >= TASK_29_0",
            }),
            110 if ctx.task_id < const_task::TASK_30_0 => Err(WorldError::MapGateLocked {
                map_id: dest_map,
                reason: "requires Cold Planet task >= TASK_30_0",
            }),
            131..=133 if ctx.task_id < const_task::TASK_25_0 => Err(WorldError::MapGateLocked {
                map_id: dest_map,
                reason: "requires Yardrat task >= TASK_25_0",
            }),
            const_map::HANH_TINH_BILL if !ctx.has_pet => Err(WorldError::MapGateLocked {
                map_id: dest_map,
                reason: "requires an active companion pet for Beerus Planet",
            }),
            const_map::HANH_TINH_NGUC_TU if !ctx.has_pet || ctx.power < 40_000_000_000 => {
                Err(WorldError::MapGateLocked {
                    map_id: dest_map,
                    reason: "requires pet and >= 40,000,000,000 power for Prison Planet",
                })
            }
            const_map::KHU_HANG_DONG..=const_map::LANG_PLANT_NGUYEN_THUY
                if !ctx.has_time_ring_992 || ctx.task_id < const_task::TASK_30_0 =>
            {
                Err(WorldError::MapGateLocked {
                    map_id: dest_map,
                    reason: "requires Time Ring (992) and task >= TASK_30_0",
                })
            }
            _ => Ok(()),
        }
    }

    /// Validates manual UI zone switching (`ChangeMapService.changeZone`), enforcing the 10-second
    /// cooldown and blocking manual switches in instanced dungeons, Black Ball War, or Mabu maps.
    ///
    /// # Errors
    /// Returns [`WorldError`] if switching is prohibited on `current_map` or on cooldown.
    pub fn validate_manual_zone_switch(
        current_map: MapId,
        ctx: MapGateContext,
        now_ms: u64,
        last_zone_switch_ms: u64,
    ) -> Result<(), WorldError> {
        if ctx.is_admin {
            return Ok(());
        }
        if ctx.holding_namek_ball {
            return Err(WorldError::MapGateLocked {
                map_id: current_map,
                reason: "cannot switch zones while carrying a Namek Dragon Ball",
            });
        }
        let m = i32::from(current_map.0);
        let blocked = matches!(
            m,
            21..=23
                | 39..=41
                | 45..=50
                | 51
                | 53..=62
                | 85..=91
                | 112..=120
                | 127..=129
                | 135..=138
                | 141..=144
                | 146..=152
        );
        if blocked {
            return Err(WorldError::MapGateLocked {
                map_id: current_map,
                reason: "zone switching is disabled on this map",
            });
        }
        let elapsed = now_ms.saturating_sub(last_zone_switch_ms);
        if elapsed < 10_000 {
            return Err(WorldError::CooldownActive {
                remaining_ms: 10_000 - elapsed,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_prison_planet_and_zone_switch_gates() {
        let base = MapGateContext {
            task_id: const_task::TASK_30_0,
            power: 10_000_000_000,
            has_pet: true,
            has_time_ring_992: false,
            holding_namek_ball: false,
            is_admin: false,
        };
        assert!(WorldRouter::validate_map_entry(MapId(155), base).is_err());
        let strong = MapGateContext {
            power: 45_000_000_000,
            ..base
        };
        assert!(WorldRouter::validate_map_entry(MapId(155), strong).is_ok());
        assert!(WorldRouter::validate_manual_zone_switch(MapId(85), strong, 20_000, 0).is_err());
        assert!(WorldRouter::validate_manual_zone_switch(MapId(39), strong, 20_000, 0).is_err());
        assert!(WorldRouter::validate_manual_zone_switch(MapId(47), strong, 20_000, 0).is_err());
        assert_eq!(
            WorldRouter::validate_manual_zone_switch(MapId(5), strong, 14_000, 10_000),
            Err(WorldError::CooldownActive {
                remaining_ms: 6_000
            })
        );
    }
}
