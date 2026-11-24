//! World-war, Majin Buu, Namek Dragon Ball, and Super Divine Water engines + dungeon tests.

use nro_consts::{const_map, const_player};
use nro_core::{ClanId, GameRng, Location, MapId, PlayerId, ZoneId};
use serde::{Deserialize, Serialize};

use crate::WorldError;

/// Gold buff tiers in [`BlackBallWarEngine`] (`X3` 10M, `X5` 30M, `X7` 50M).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlackBallBuff {
    /// `X3` stats (`10,000,000` Gold).
    X3,
    /// `X5` stats (`30,000,000` Gold).
    X5,
    /// `X7` stats (`50,000,000` Gold).
    X7,
}

impl BlackBallBuff {
    /// Returns `(cost_gold, multiplier)`.
    #[must_use]
    pub const fn cost_and_multiplier(self) -> (u64, u8) {
        match self {
            Self::X3 => (10_000_000, 3),
            Self::X5 => (30_000_000, 5),
            Self::X7 => (50_000_000, 7),
        }
    }
}

/// 5. Black Ball War (`BlackBallWar` + `BlackBallWarService`, Maps `85..=91`, `20:00–21:00`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackBallWarEngine {
    pub star: u8,
    pub holder: Option<(PlayerId, ClanId, u64)>,
    pub next_pick_allowed_ms: u64,
    pub winning_clan: Option<(ClanId, u64)>,
}

impl BlackBallWarEngine {
    /// Initializes a zone state machine for `star` (`1..=7`).
    #[must_use]
    pub const fn new(star: u8) -> Self {
        Self {
            star,
            holder: None,
            next_pick_allowed_ms: 0,
            winning_clan: None,
        }
    }

    /// Picks up the Black Ball (`20:30–21:00` window + clan membership + `5s` drop lock), forcing Flag `8`.
    pub fn try_pick(
        &mut self,
        pid: PlayerId,
        clan: ClanId,
        hour: u8,
        min: u8,
        now_ms: u64,
    ) -> Result<i8, WorldError> {
        if clan.0 < 0
            || hour != 20
            || min < 30
            || self.holder.is_some()
            || self.winning_clan.is_some()
        {
            return Err(WorldError::DungeonRule(
                "Black Ball pickup requires clan membership, 20:30..21:00, and unheld ball",
            ));
        }
        if now_ms < self.next_pick_allowed_ms {
            return Err(WorldError::CooldownActive {
                remaining_ms: self.next_pick_allowed_ms - now_ms,
            });
        }
        self.holder = Some((pid, clan, now_ms));
        Ok(8)
    }

    /// Drops the ball and arms the `5,000 ms` pickup cooldown.
    pub fn drop_ball(&mut self, now_ms: u64) {
        self.holder = None;
        self.next_pick_allowed_ms = now_ms.saturating_add(5_000);
    }

    /// Awards the `22h` clan victory if the carrier holds continuously for `300,000 ms` (`5m`).
    pub fn check_five_minute_victory(&mut self, now_ms: u64) -> Option<ClanId> {
        if self.winning_clan.is_some() {
            return None;
        }
        let (_, clan, t0) = self.holder?;
        (now_ms.saturating_sub(t0) >= 300_000).then(|| {
            self.holder = None;
            self.winning_clan = Some((clan, now_ms.saturating_add(79_200_000)));
            clan
        })
    }
}

/// 6. Majin Buu 14h (`MajinBuu14H`, Maps `127–128`, Pool `7`) & 12h Spaceship Floor (`FightMabu`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajinBuu14hInstance {
    pub stomach_holds: [Option<PlayerId>; 4],
}

impl MajinBuu14hInstance {
    /// Canonical cocoon coordinates on Super Buu's stomach (`Map 128`).
    pub const HOLD_COORDS: [Location; 4] = [
        Location {
            map_id: MapId(128),
            zone_id: ZoneId(0),
            x: 196,
            y: 257,
        },
        Location {
            map_id: MapId(128),
            zone_id: ZoneId(0),
            x: 340,
            y: 256,
        },
        Location {
            map_id: MapId(128),
            zone_id: ZoneId(0),
            x: 412,
            y: 232,
        },
        Location {
            map_id: MapId(128),
            zone_id: ZoneId(0),
            x: 532,
            y: 257,
        },
    ];

    /// Traps an unshielded player eaten on Map `127` into a Map `128` cocoon.
    pub fn eat_player(&mut self, victim: PlayerId, has_shield: bool) -> Option<(usize, Location)> {
        if has_shield || self.stomach_holds.contains(&Some(victim)) {
            return None;
        }
        let idx = self.stomach_holds.iter().position(Option::is_none)?;
        self.stomach_holds[idx] = Some(victim);
        Some((idx, Self::HOLD_COORDS[idx]))
    }

    /// Releases `pid` from any occupied stomach cocoon slot.
    pub fn release_player(&mut self, pid: PlayerId) {
        for s in &mut self.stomach_holds {
            if *s == Some(pid) {
                *s = None;
            }
        }
    }
}

/// Majin Buu 12h (`12:00–13:00`) floor tracker (`114 -> 115 -> 117 -> 118 -> 119 -> 120`, `POINT_MAX = 10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajinBuu12hFloor {
    pub map_id: MapId,
    pub points: u8,
    pub c_flag: i8,
}

impl MajinBuu12hFloor {
    /// Ticks Babidi `1/100` hypnosis (`9 -> 10`) and Osin `1/50` cure (`10 -> 9`), descending at `10` points.
    pub fn tick_and_try_descend(&mut self, rng: &mut GameRng) -> Option<MapId> {
        if self.c_flag == 9 && rng.next_u64().is_multiple_of(100) {
            self.c_flag = 10;
        } else if self.c_flag == 10 && rng.next_u64().is_multiple_of(50) {
            self.c_flag = 9;
        }
        if self.points < 10 {
            return None;
        }
        let next = match i32::from(self.map_id.0) {
            114 => 115,
            115 => 117,
            117 => 118,
            118 => 119,
            119 => 120,
            _ => return None,
        };
        self.points = 0;
        self.map_id = MapId(next);
        Some(self.map_id)
    }
}

/// 7. Namek Dragon Balls (`NgocRongNamecService`, `353..=359`, Stone `362`) & Super Divine Water (`Map 146`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamekDragonBallTracker {
    pub holders: [Option<(PlayerId, ClanId, MapId, ZoneId)>; 7],
    pub stone_until_ms: u64,
}

impl NamekDragonBallTracker {
    /// Picks up star `0..7` (`1` ball/player max, forces `PK_ALL = 5`).
    pub fn pick_ball(
        &mut self,
        star: usize,
        pid: PlayerId,
        clan: ClanId,
        map: MapId,
        zone: ZoneId,
        now_ms: u64,
    ) -> Result<i8, WorldError> {
        if now_ms < self.stone_until_ms
            || star >= 7
            || self.holders[star].is_some()
            || self.holders.iter().flatten().any(|(p, _, _, _)| *p == pid)
        {
            return Err(WorldError::DungeonRule("cannot pick Namek Dragon Ball"));
        }
        self.holders[star] = Some((pid, clan, map, zone));
        Ok(const_player::PK_ALL)
    }

    /// Consumes all 7 balls held by `clan` on Map `7` (`LANG_MORI`) in `zone`, petrifying for `24h`.
    pub fn consume_for_porunga(
        &mut self,
        clan: ClanId,
        zone: ZoneId,
        now_ms: u64,
    ) -> Result<(), WorldError> {
        if clan.0 < 0
            || !self.holders.iter().all(|s| {
                matches!(s, Some((_, c, m, z)) if *c == clan && i32::from(m.0) == const_map::LANG_MORI && *z == zone)
            })
        {
            return Err(WorldError::DungeonRule(
                "Porunga requires all 7 Namek balls held by clan on Map 7",
            ));
        }
        self.holders = [None; 7];
        self.stone_until_ms = now_ms.saturating_add(86_400_000);
        Ok(())
    }

    /// Releases any Namek Dragon Ball currently held by `pid`.
    pub fn release_player(&mut self, pid: PlayerId) {
        for s in &mut self.holders {
            if matches!(s, Some((p, ..)) if *p == pid) {
                *s = None;
            }
        }
    }
}

/// Super Divine Water (`SuperDivineWaterService`, Map `146`, `15m` poison timer, 1 win/day).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperDivineWaterSession {
    pub deadline_ms: u64,
    pub pocolo_stats: (u64, u64),
}

impl SuperDivineWaterSession {
    /// Starts a `Tây Karin` solo session (`dame = min(pl_dame, 200M)`, `hp = min(pl_hp * 5, 2B)`).
    pub fn start(
        won_today: bool,
        pl_dame: u64,
        pl_hp_max: u64,
        now_ms: u64,
    ) -> Result<Self, WorldError> {
        if won_today {
            return Err(WorldError::DungeonRule(
                "already won SuperDivineWater today",
            ));
        }
        Ok(Self {
            deadline_ms: now_ms.saturating_add(900_000),
            pocolo_stats: (
                pl_dame.min(200_000_000),
                pl_hp_max.saturating_mul(5).min(2_000_000_000),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        COLLAPSE_DURATION_MS, DUNGEON_DURATION_MS, DestronGasInstance, SkillBeamKind,
        SnakeWayInstance,
    };
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_destron_gas_clamps_beams_to_one(raw in 2u64..5_000_000_000u64) {
            let mut dg = DestronGasInstance::open(ClanId(1), true, 200_000_000, 0, 50, 1_000).expect("open dg");
            prop_assert_eq!(dg.filter_damage(raw, SkillBeamKind::EnergyBeam), 1);
            dg.destroyer_machine_alive = false;
            prop_assert_eq!(dg.filter_damage(raw, SkillBeamKind::EnergyBeam), raw);
        }
    }

    #[test]
    fn black_ball_war_enforces_clan_2030_and_5min_victory() {
        let mut bb = BlackBallWarEngine::new(1);
        assert!(
            bb.try_pick(PlayerId(10), ClanId(-1), 20, 30, 1_000)
                .is_err()
        );
        assert!(bb.try_pick(PlayerId(10), ClanId(7), 20, 15, 1_000).is_err());
        assert_eq!(bb.try_pick(PlayerId(10), ClanId(7), 20, 30, 10_000), Ok(8));
        assert_eq!(bb.check_five_minute_victory(310_000), Some(ClanId(7)));
        assert!(
            bb.try_pick(PlayerId(11), ClanId(8), 20, 36, 320_000)
                .is_err()
        );
    }

    #[test]
    fn mabu_14h_and_namek_balls_guard_duplicates_clanless_and_release() {
        let mut mb = MajinBuu14hInstance::default();
        assert!(mb.eat_player(PlayerId(1), false).is_some());
        assert!(mb.eat_player(PlayerId(1), false).is_none());
        mb.release_player(PlayerId(1));
        assert!(mb.eat_player(PlayerId(1), false).is_some());
        let mut nb = NamekDragonBallTracker::default();
        for i in 0_usize..7 {
            assert!(
                nb.pick_ball(
                    i,
                    PlayerId(i64::try_from(i).unwrap_or(0) + 1),
                    ClanId(-1),
                    MapId(7),
                    ZoneId(0),
                    1_000,
                )
                .is_ok()
            );
        }
        assert!(
            nb.consume_for_porunga(ClanId(-1), ZoneId(0), 2_000)
                .is_err()
        );
        nb.release_player(PlayerId(1));
        assert!(nb.holders[0].is_none());
    }

    #[test]
    fn destron_gas_and_snake_way_evacuate_on_expiry_and_collapse() {
        let mut dg =
            DestronGasInstance::open(ClanId(1), true, 200_000_000, 0, 50, 10_000).expect("dg");
        assert_eq!(dg.check_evacuation(10_000 + DUNGEON_DURATION_MS - 1), None);
        assert_eq!(
            dg.check_evacuation(10_000 + DUNGEON_DURATION_MS)
                .map(|e| e.map_id),
            Some(MapId(0))
        );
        dg.on_final_boss_killed(20_000);
        assert_eq!(dg.phase, 3);
        assert_eq!(
            dg.check_evacuation(20_000 + COLLAPSE_DURATION_MS)
                .map(|e| e.map_id),
            Some(MapId(0))
        );
        let mut sw = SnakeWayInstance::open(ClanId(1), 50, 200_000_000, 10_000, 0).expect("sw");
        assert_eq!(sw.check_evacuation(10_000 + DUNGEON_DURATION_MS - 1), None);
        assert_eq!(
            sw.check_evacuation(10_000 + DUNGEON_DURATION_MS)
                .map(|e| e.map_id),
            Some(MapId(47))
        );
        sw.on_boss_killed(25_000);
        assert_eq!(
            sw.check_evacuation(25_000 + COLLAPSE_DURATION_MS)
                .map(|e| e.map_id),
            Some(MapId(47))
        );
    }
}
