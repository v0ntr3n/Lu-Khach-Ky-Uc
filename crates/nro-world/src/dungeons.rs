//! Instance lifecycles, schedules, boss scaling, and world-war mechanics for all 6 `phoban`
//! dungeons (`BanDoKhoBau`, `RedRibbonHQ`, `SnakeWay`, `DestronGas`, `BlackBallWar`,
//! `MajinBuu14H`) plus `NgocRongNamecService`, `MajinBuuService` (12h), and `SuperDivineWaterService`.

#[path = "dungeons_war.rs"]
mod dungeons_war;
pub use dungeons_war::*;

use nro_consts::{boss_i_d, const_item};
use nro_core::{ClanId, GameRng, Gender, MapId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::WorldError;

/// Standard 30-minute dungeon combat window (`1,800,000 ms`).
pub const DUNGEON_DURATION_MS: u64 = 1_800_000;

/// Standard 60-second collapse evacuation timer after final boss defeat (`60,000 ms`).
pub const COLLAPSE_DURATION_MS: u64 = 60_000;

/// Evacuated destination coordinate when a dungeon expires or collapses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DungeonExit {
    /// Destination map ID.
    pub map_id: MapId,
    /// Destination X coordinate.
    pub x: i16,
}

/// Attack classification filtered by `Cỗ Máy Hủy Diệt` (`mob 76`) inside [`DestronGasInstance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillBeamKind {
    /// Melee or non-beam skill (normal damage).
    NonBeam,
    /// Kamejoko, Masenko, Antomic, or Liên Hoàn (clamped to `1` while mob `76` lives).
    EnergyBeam,
}

/// 1. Undersea Treasure (`BanDoKhoBau` + `TreasureUnderSeaService`, Maps `135..=138`, Pool `50`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanDoKhoBauInstance {
    pub clan_id: ClanId,
    pub level: u8,
    pub opened_at_ms: u64,
    pub collapse_at_ms: Option<u64>,
}

impl BanDoKhoBauInstance {
    /// Validates `level in 1..=110`, `< 3` daily runs/player, and `>= 1` Treasure Map (`611`).
    pub fn open(
        clan_id: ClanId,
        level: u8,
        runs_today: u8,
        map_611_qty: u32,
        now_ms: u64,
    ) -> Result<Self, WorldError> {
        if !(1..=110).contains(&level) || runs_today >= 3 {
            return Err(WorldError::DungeonRule(
                "BanDoKhoBau requires level 1..=110 and < 3 runs today",
            ));
        }
        if map_611_qty == 0 {
            return Err(WorldError::InsufficientItem {
                template_id: TemplateId(611),
                required: 1,
                available: 0,
            });
        }
        Ok(Self {
            clan_id,
            level,
            opened_at_ms: now_ms,
            collapse_at_ms: None,
        })
    }

    #[must_use]
    pub fn spike_trap_damage(&self) -> u64 {
        u64::from(self.level) * 100_000
    }

    #[must_use]
    pub fn boss_stats(&self) -> (i32, u64, u64) {
        let lv = u64::from(self.level);
        (
            boss_i_d::TRUNG_UY_XANH_LO,
            (200_000 * lv).min(200_000_000),
            (20_000_000 * lv).min(2_000_000_000),
        )
    }

    pub fn on_boss_killed(&mut self, now_ms: u64) {
        self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS));
    }

    #[must_use]
    pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit {
            map_id: MapId(5),
            x: 1038,
        })
    }
}

/// 2. Red Ribbon HQ (`RedRibbonHQ` + `RedRibbonHQService`, Maps `53..=62`, Pool `50`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedRibbonHqInstance {
    pub clan_id: ClanId,
    pub clan_totals: (u64, u64),
    pub opened_at_ms: u64,
    pub pick_ball_until_ms: Option<u64>,
    pub trung_uy_thep_alive: bool,
}

impl RedRibbonHqInstance {
    /// Validates `1/day` clan entry and `>= 24h` (`86,400,000 ms`) member seniority.
    pub fn open(
        clan_id: ClanId,
        ran_today: bool,
        member_age_ms: u64,
        dame: u64,
        hp: u64,
        now_ms: u64,
    ) -> Result<Self, WorldError> {
        if ran_today || member_age_ms < 86_400_000 {
            return Err(WorldError::DungeonRule(
                "RedRibbonHQ requires 1/day clan quota and >= 24h seniority",
            ));
        }
        Ok(Self {
            clan_id,
            clan_totals: (dame.max(1_000), hp.max(10_000)),
            opened_at_ms: now_ms,
            pick_ball_until_ms: None,
            trung_uy_thep_alive: true,
        })
    }

    #[must_use]
    pub fn stage_boss_stats(&self, stage: u8) -> (i32, u64, u64) {
        let (id, pct) = match stage {
            0 => (boss_i_d::TRUNG_UY_TRANG, 100u64),
            1 => (boss_i_d::TRUNG_UY_XANH_LO, 110),
            2 => (boss_i_d::TRUNG_UY_THEP, 115),
            3 => (boss_i_d::NINJA_AO_TIM, 120),
            _ => (boss_i_d::ROBOT_VE_SI, 130),
        };
        (
            id,
            self.clan_totals.1.saturating_mul(pct) / 1_000,
            self.clan_totals.0.saturating_mul(pct * 2),
        )
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn roll_zone_dragon_balls(rng: &mut GameRng) -> SmallVec<[TemplateId; 8]> {
        (0..(3 + (rng.next_u64() % 5)))
            .map(|_| {
                let id = if rng.next_u64().is_multiple_of(500) {
                    const_item::NGOC_RONG_1_SAO + (rng.next_u64() % 5) as i32
                } else {
                    const_item::NGOC_RONG_5_SAO + (rng.next_u64() % 3) as i32
                };
                TemplateId(id as i16)
            })
            .collect()
    }

    #[must_use]
    pub fn check_evacuation(&self, now_ms: u64, gender: Gender) -> Option<DungeonExit> {
        let end = self
            .pick_ball_until_ms
            .unwrap_or_else(|| self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS));
        let home = match gender {
            Gender::TraiDat => 21,
            Gender::Namec => 22,
            Gender::Xayda => 23,
        };
        (now_ms >= end).then_some(DungeonExit {
            map_id: MapId(home),
            x: 350,
        })
    }
}

/// 3. Snake Way (`SnakeWay` + `SnakeWayService`, Maps `141..=144`, Pool `50`, 7-day cooldown).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnakeWayInstance {
    pub clan_id: ClanId,
    pub level_cdrd: u8,
    pub opened_at_ms: u64,
    pub collapse_at_ms: Option<u64>,
    pub all_mobs_cleared: bool,
}

impl SnakeWayInstance {
    /// Validates `level in 1..=110`, `>= 48h` seniority, and `7-day` (`604,800,000 ms`) player cooldown.
    pub fn open(
        clan_id: ClanId,
        level: u8,
        member_age_ms: u64,
        now_ms: u64,
        last_join_ms: u64,
    ) -> Result<Self, WorldError> {
        if !(1..=110).contains(&level) || member_age_ms < 172_800_000 {
            return Err(WorldError::DungeonRule(
                "SnakeWay requires level 1..=110 and >= 48h clan seniority",
            ));
        }
        let elapsed = now_ms.saturating_sub(last_join_ms);
        if last_join_ms > 0 && elapsed < 604_800_000 {
            return Err(WorldError::CooldownActive {
                remaining_ms: 604_800_000 - elapsed,
            });
        }
        Ok(Self {
            clan_id,
            level_cdrd: level,
            opened_at_ms: now_ms,
            collapse_at_ms: None,
            all_mobs_cleared: false,
        })
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn leaderboard_record(&self, finished_at_ms: u64) -> (u8, u32) {
        (
            self.level_cdrd,
            (finished_at_ms.saturating_sub(self.opened_at_ms) / 1_000).min(u64::from(u32::MAX))
                as u32,
        )
    }

    pub fn on_boss_killed(&mut self, now_ms: u64) {
        self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS));
    }

    #[must_use]
    pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit {
            map_id: MapId(47),
            x: 350,
        })
    }
}

/// 4. Destron Gas (`DestronGas` + `DestronGasService`, Maps `147, 148, 149, 151, 152`, Pool `50`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestronGasInstance {
    pub clan_id: ClanId,
    pub level: u8,
    pub opened_at_ms: u64,
    pub collapse_at_ms: Option<u64>,
    pub destroyer_machine_alive: bool,
    pub phase: u8,
}

impl DestronGasInstance {
    /// Validates Clan Leader permission, `>= 48h` seniority, `< 3` daily clan runs, and `1..=110` level.
    pub fn open(
        clan_id: ClanId,
        is_leader: bool,
        age_ms: u64,
        runs_today: u8,
        level: u8,
        now_ms: u64,
    ) -> Result<Self, WorldError> {
        if !is_leader || age_ms < 172_800_000 || !(1..=110).contains(&level) || runs_today >= 3 {
            return Err(WorldError::DungeonRule(
                "DestronGas requires Clan Leader, >=48h seniority, <3 runs, lv 1..=110",
            ));
        }
        Ok(Self {
            clan_id,
            level,
            opened_at_ms: now_ms,
            collapse_at_ms: None,
            destroyer_machine_alive: true,
            phase: 0,
        })
    }

    #[must_use]
    pub const fn filter_damage(&self, raw_damage: u64, skill: SkillBeamKind) -> u64 {
        if self.destroyer_machine_alive && matches!(skill, SkillBeamKind::EnergyBeam) {
            1
        } else {
            raw_damage
        }
    }

    pub fn on_final_boss_killed(&mut self, now_ms: u64) {
        self.phase = 3;
        self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS));
    }

    #[must_use]
    pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit {
            map_id: MapId(0),
            x: 350,
        })
    }
}
