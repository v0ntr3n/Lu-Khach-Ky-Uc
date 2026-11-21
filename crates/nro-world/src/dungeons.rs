//! Instance lifecycles, schedules, boss scaling, and world-war mechanics for all 6 `phoban`
//! dungeons (`BanDoKhoBau`, `RedRibbonHQ`, `SnakeWay`, `DestronGas`, `BlackBallWar`,
//! `MajinBuu14H`) plus `NgocRongNamecService`, `MajinBuuService` (12h), and `SuperDivineWaterService`.

use nro_consts::{boss_i_d, const_item, const_map, const_player};
use nro_core::{ClanId, GameRng, Gender, Location, MapId, PlayerId, TemplateId, ZoneId};
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
    /// Owning clan ID.
    pub clan_id: ClanId,
    /// Difficulty level (`1..=110`).
    pub level: u8,
    /// Open timestamp (`ms`).
    pub opened_at_ms: u64,
    /// `60s` collapse timestamp set upon `Trung Úy Xanh Lơ` defeat on Map `137`.
    pub collapse_at_ms: Option<u64>,
}

impl BanDoKhoBauInstance {
    /// Validates `level in 1..=110`, `< 3` daily runs/player, and `>= 1` Treasure Map (`611`).
    /// # Errors
    /// Returns [`WorldError`] on invalid level, exhausted daily quota, or missing item `611`.
    pub fn open(clan_id: ClanId, level: u8, runs_today: u8, map_611_qty: u32, now_ms: u64) -> Result<Self, WorldError> {
        if !(1..=110).contains(&level) || runs_today >= 3 { return Err(WorldError::DungeonRule("BanDoKhoBau requires level 1..=110 and < 3 runs today")); }
        if map_611_qty == 0 { return Err(WorldError::InsufficientItem { template_id: TemplateId(611), required: 1, available: 0 }); }
        Ok(Self { clan_id, level, opened_at_ms: now_ms, collapse_at_ms: None })
    }
    /// Base spike floor damage on Map `135` (`level * 100,000` per second).
    #[must_use] pub fn spike_trap_damage(&self) -> u64 { u64::from(self.level) * 100_000 }
    /// Computes `(boss_id, dame, hp)` for `Trung Úy Xanh Lơ` on Map `137`.
    #[must_use] pub fn boss_stats(&self) -> (i32, u64, u64) {
        let lv = u64::from(self.level);
        (boss_i_d::TRUNG_UY_XANH_LO, (200_000 * lv).min(200_000_000), (20_000_000 * lv).min(2_000_000_000))
    }
    /// Starts the `60s` collapse timer upon boss defeat.
    pub fn on_boss_killed(&mut self, now_ms: u64) { self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS)); }
    /// Returns `Some(Map 5, x = 1038)` once the 30m limit or 60s collapse elapses.
    #[must_use] pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit { map_id: MapId(5), x: 1038 })
    }
}

/// 2. Red Ribbon HQ (`RedRibbonHQ` + `RedRibbonHQService`, Maps `53..=62`, Pool `50`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedRibbonHqInstance {
    /// Owning clan ID.
    pub clan_id: ClanId,
    /// Sum of online clanmates' `(total_dame, total_hp)` at open.
    pub clan_totals: (u64, u64),
    /// Open timestamp (`ms`).
    pub opened_at_ms: u64,
    /// `5m` (`300,000 ms`) Dragon Ball pickup deadline once all 5 boss stages clear.
    pub pick_ball_until_ms: Option<u64>,
    /// True while `Trung Úy Thép` lives (Bulon respawns every `10s`).
    pub trung_uy_thep_alive: bool,
}

impl RedRibbonHqInstance {
    /// Validates `1/day` clan entry and `>= 24h` (`86,400,000 ms`) member seniority.
    /// # Errors
    /// Returns [`WorldError`] if clan already entered today or member joined `< 24h` ago.
    pub fn open(clan_id: ClanId, ran_today: bool, member_age_ms: u64, dame: u64, hp: u64, now_ms: u64) -> Result<Self, WorldError> {
        if ran_today || member_age_ms < 86_400_000 { return Err(WorldError::DungeonRule("RedRibbonHQ requires 1/day clan quota and >= 24h seniority")); }
        Ok(Self { clan_id, clan_totals: (dame.max(1_000), hp.max(10_000)), opened_at_ms: now_ms, pick_ball_until_ms: None, trung_uy_thep_alive: true })
    }
    /// Computes `(boss_id, dame, hp)` for stages `0..=4` (`1.0x, 1.1x, 1.15x, 1.2x, 1.3x`).
    #[must_use] pub fn stage_boss_stats(&self, stage: u8) -> (i32, u64, u64) {
        let (id, pct) = match stage {
            0 => (boss_i_d::TRUNG_UY_TRANG, 100u64), 1 => (boss_i_d::TRUNG_UY_XANH_LO, 110),
            2 => (boss_i_d::TRUNG_UY_THEP, 115), 3 => (boss_i_d::NINJA_AO_TIM, 120), _ => (boss_i_d::ROBOT_VE_SI, 130),
        };
        (id, self.clan_totals.1.saturating_mul(pct) / 1_000, self.clan_totals.0.saturating_mul(pct * 2))
    }
    /// Rolls `3..=7` Earth Dragon Balls per zone (`1/500` `14..=18`, else `18..=20`).
    #[allow(clippy::cast_possible_truncation)]
    pub fn roll_zone_dragon_balls(rng: &mut GameRng) -> SmallVec<[TemplateId; 8]> {
        (0..(3 + (rng.next_u64() % 5))).map(|_| {
            let id = if rng.next_u64().is_multiple_of(500) { const_item::NGOC_RONG_1_SAO + (rng.next_u64() % 5) as i32 }
            else { const_item::NGOC_RONG_5_SAO + (rng.next_u64() % 3) as i32 };
            TemplateId(id as i16)
        }).collect()
    }
    /// Evacuates to home house (`21 + gender`) when combat (`30m`) or pickup (`5m`) ends.
    #[must_use] pub fn check_evacuation(&self, now_ms: u64, gender: Gender) -> Option<DungeonExit> {
        let end = self.pick_ball_until_ms.unwrap_or_else(|| self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS));
        let home = match gender { Gender::TraiDat => 21, Gender::Namec => 22, Gender::Xayda => 23 };
        (now_ms >= end).then_some(DungeonExit { map_id: MapId(home), x: 350 })
    }
}

/// 3. Snake Way (`SnakeWay` + `SnakeWayService`, Maps `141..=144`, Pool `50`, 7-day cooldown).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnakeWayInstance {
    /// Owning clan ID.
    pub clan_id: ClanId,
    /// Difficulty level (`1..=110`), preserved into `level_cdrd_done` (fixing `TopConDuongRanDoc` bug).
    pub level_cdrd: u8,
    /// Open timestamp (`ms`).
    pub opened_at_ms: u64,
    /// `60s` post-clear collapse timestamp set upon final boss defeat.
    pub collapse_at_ms: Option<u64>,
    /// Unlocks Karin (`Map 47`) Yardrat teleport to boss Map `144` (`6x Saibamen`, `Nadic`, `Cadich`).
    pub all_mobs_cleared: bool,
}

impl SnakeWayInstance {
    /// Validates `level in 1..=110`, `>= 48h` seniority, and `7-day` (`604,800,000 ms`) player cooldown.
    /// # Errors
    /// Returns [`WorldError`] on invalid level, seniority, or active 7-day cooldown.
    pub fn open(clan_id: ClanId, level: u8, member_age_ms: u64, now_ms: u64, last_join_ms: u64) -> Result<Self, WorldError> {
        if !(1..=110).contains(&level) || member_age_ms < 172_800_000 { return Err(WorldError::DungeonRule("SnakeWay requires level 1..=110 and >= 48h clan seniority")); }
        let elapsed = now_ms.saturating_sub(last_join_ms);
        if last_join_ms > 0 && elapsed < 604_800_000 { return Err(WorldError::CooldownActive { remaining_ms: 604_800_000 - elapsed }); }
        Ok(Self { clan_id, level_cdrd: level, opened_at_ms: now_ms, collapse_at_ms: None, all_mobs_cleared: false })
    }
    /// Returns `(level_cdrd_done, duration_secs)` for `TopConDuongRanDoc`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn leaderboard_record(&self, finished_at_ms: u64) -> (u8, u32) { (self.level_cdrd, (finished_at_ms.saturating_sub(self.opened_at_ms) / 1_000).min(u64::from(u32::MAX)) as u32) }
    /// Starts the `60s` post-clear collapse timer upon final boss defeat.
    pub fn on_boss_killed(&mut self, now_ms: u64) { self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS)); }
    /// Returns `Some(Map 47)` once the `30m` limit or `60s` post-clear collapse elapses.
    #[must_use] pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit { map_id: MapId(47), x: 350 })
    }
}

/// 4. Destron Gas (`DestronGas` + `DestronGasService`, Maps `147, 148, 149, 151, 152`, Pool `50`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestronGasInstance {
    /// Owning clan ID.
    pub clan_id: ClanId,
    /// Difficulty (`1..=110`).
    pub level: u8,
    /// Open timestamp (`ms`).
    pub opened_at_ms: u64,
    /// `60s` explosion evacuation timestamp set upon `Hatchiyack` defeat.
    pub collapse_at_ms: Option<u64>,
    /// True while `Cỗ Máy Hủy Diệt` (`mob 76`) lives.
    pub destroyer_machine_alive: bool,
    /// Boss progression (`0` mobs, `1` `DrLychee`, `2` `Hatchiyack`, `3` `60s` explosion -> Map `0`).
    pub phase: u8,
}

impl DestronGasInstance {
    /// Validates Clan Leader permission, `>= 48h` seniority, `< 3` daily clan runs, and `1..=110` level.
    /// # Errors
    /// Returns [`WorldError`] if non-leader or quota/level check fails.
    pub fn open(clan_id: ClanId, is_leader: bool, age_ms: u64, runs_today: u8, level: u8, now_ms: u64) -> Result<Self, WorldError> {
        if !is_leader || age_ms < 172_800_000 || !(1..=110).contains(&level) || runs_today >= 3 { return Err(WorldError::DungeonRule("DestronGas requires Clan Leader, >=48h seniority, <3 runs, lv 1..=110")); }
        Ok(Self { clan_id, level, opened_at_ms: now_ms, collapse_at_ms: None, destroyer_machine_alive: true, phase: 0 })
    }
    /// Clamps energy beams (`Kamejoko`, `Masenko`, `Antomic`, `Liên Hoàn`) to `1` while mob `76` lives.
    #[must_use] pub const fn filter_damage(&self, raw_damage: u64, skill: SkillBeamKind) -> u64 { if self.destroyer_machine_alive && matches!(skill, SkillBeamKind::EnergyBeam) { 1 } else { raw_damage } }
    /// Advances to phase `3` and starts the `60s` explosion evacuation timer upon `Hatchiyack` defeat.
    pub fn on_final_boss_killed(&mut self, now_ms: u64) { self.phase = 3; self.collapse_at_ms = Some(now_ms.saturating_add(COLLAPSE_DURATION_MS)); }
    /// Returns `Some(Map 0)` once the `30m` limit or `60s` collapse elapses.
    #[must_use] pub fn check_evacuation(&self, now_ms: u64) -> Option<DungeonExit> {
        let hard = self.opened_at_ms.saturating_add(DUNGEON_DURATION_MS);
        (now_ms >= self.collapse_at_ms.map_or(hard, |c| c.min(hard))).then_some(DungeonExit { map_id: MapId(0), x: 350 })
    }
}

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
    #[must_use] pub const fn cost_and_multiplier(self) -> (u64, u8) {
        match self { Self::X3 => (10_000_000, 3), Self::X5 => (30_000_000, 5), Self::X7 => (50_000_000, 7) }
    }
}

/// 5. Black Ball War (`BlackBallWar` + `BlackBallWarService`, Maps `85..=91`, `20:00–21:00`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackBallWarEngine {
    /// Star tier (`1..=7`, Map `84 + star`, Item `371 + star`).
    pub star: u8,
    /// Active carrier `(player_id, clan_id, picked_at_ms)`.
    pub holder: Option<(PlayerId, ClanId, u64)>,
    /// `5,000 ms` drop pickup lock timestamp.
    pub next_pick_allowed_ms: u64,
    /// Winning clan & `22h` (`79,200,000 ms`) `RewardBlackBall` expiry.
    pub winning_clan: Option<(ClanId, u64)>,
}

impl BlackBallWarEngine {
    /// Initializes a zone state machine for `star` (`1..=7`).
    #[must_use] pub const fn new(star: u8) -> Self { Self { star, holder: None, next_pick_allowed_ms: 0, winning_clan: None } }
    /// Picks up the Black Ball (`20:30–21:00` window + `5s` drop lock), forcing Flag `8`.
    /// # Errors
    /// Returns [`WorldError`] if before `20:30`, already held, or within the `5s` drop lock.
    pub fn try_pick(&mut self, pid: PlayerId, clan: ClanId, hour: u8, min: u8, now_ms: u64) -> Result<i8, WorldError> {
        if hour != 20 || min < 30 || self.holder.is_some() || self.winning_clan.is_some() { return Err(WorldError::DungeonRule("Black Ball pickup requires 20:30..21:00 and unheld ball")); }
        if now_ms < self.next_pick_allowed_ms { return Err(WorldError::CooldownActive { remaining_ms: self.next_pick_allowed_ms - now_ms }); }
        self.holder = Some((pid, clan, now_ms));
        Ok(8)
    }
    /// Drops the ball and arms the `5,000 ms` pickup cooldown.
    pub fn drop_ball(&mut self, now_ms: u64) { self.holder = None; self.next_pick_allowed_ms = now_ms.saturating_add(5_000); }
    /// Awards the `22h` clan victory if the carrier holds continuously for `300,000 ms` (`5m`).
    pub fn check_five_minute_victory(&mut self, now_ms: u64) -> Option<ClanId> {
        if self.winning_clan.is_some() { return None; }
        let (_, clan, t0) = self.holder?;
        (now_ms.saturating_sub(t0) >= 300_000).then(|| { self.holder = None; self.winning_clan = Some((clan, now_ms.saturating_add(79_200_000))); clan })
    }
}

/// 6. Majin Buu 14h (`MajinBuu14H`, Maps `127–128`, Pool `7`) & 12h Spaceship Floor (`FightMabu`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajinBuu14hInstance {
    /// Four `MaBuHold` cocoons on Map `128`.
    pub stomach_holds: [Option<PlayerId>; 4],
}

impl MajinBuu14hInstance {
    /// Canonical cocoon coordinates on Super Buu's stomach (`Map 128`).
    pub const HOLD_COORDS: [Location; 4] = [
        Location { map_id: MapId(128), zone_id: ZoneId(0), x: 196, y: 257 }, Location { map_id: MapId(128), zone_id: ZoneId(0), x: 340, y: 256 }, Location { map_id: MapId(128), zone_id: ZoneId(0), x: 412, y: 232 }, Location { map_id: MapId(128), zone_id: ZoneId(0), x: 532, y: 257 },
    ];
    /// Traps an unshielded player eaten on Map `127` into a Map `128` cocoon.
    pub fn eat_player(&mut self, victim: PlayerId, has_shield: bool) -> Option<(usize, Location)> {
        if has_shield || self.stomach_holds.contains(&Some(victim)) { return None; }
        let idx = self.stomach_holds.iter().position(Option::is_none)?;
        self.stomach_holds[idx] = Some(victim);
        Some((idx, Self::HOLD_COORDS[idx]))
    }
    /// Releases `pid` from any occupied stomach cocoon slot.
    pub fn release_player(&mut self, pid: PlayerId) { for s in &mut self.stomach_holds { if *s == Some(pid) { *s = None; } } }
}

/// Majin Buu 12h (`12:00–13:00`) floor tracker (`114 -> 115 -> 117 -> 118 -> 119 -> 120`, `POINT_MAX = 10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajinBuu12hFloor {
    /// Current floor map ID.
    pub map_id: MapId,
    /// `FightMabu` gauge (`0..=10`).
    pub points: u8,
    /// Faction flag (`9` Osin, `10` Babidi).
    pub c_flag: i8,
}

impl MajinBuu12hFloor {
    /// Ticks Babidi `1/100` hypnosis (`9 -> 10`) and Osin `1/50` cure (`10 -> 9`), descending at `10` points.
    pub fn tick_and_try_descend(&mut self, rng: &mut GameRng) -> Option<MapId> {
        if self.c_flag == 9 && rng.next_u64().is_multiple_of(100) { self.c_flag = 10; }
        else if self.c_flag == 10 && rng.next_u64().is_multiple_of(50) { self.c_flag = 9; }
        if self.points < 10 { return None; }
        let next = match i32::from(self.map_id.0) { 114 => 115, 115 => 117, 117 => 118, 118 => 119, 119 => 120, _ => return None };
        self.points = 0;
        self.map_id = MapId(next);
        Some(self.map_id)
    }
}

/// 7. Namek Dragon Balls (`NgocRongNamecService`, `353..=359`, Stone `362`) & Super Divine Water (`Map 146`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamekDragonBallTracker {
    /// Active carrier `(player_id, clan_id, map_id, zone_id)` for stars `0..7`.
    pub holders: [Option<(PlayerId, ClanId, MapId, ZoneId)>; 7],
    /// Petrified stone expiration (`24h = 86,400,000 ms`).
    pub stone_until_ms: u64,
}

impl NamekDragonBallTracker {
    /// Picks up star `0..7` (`1` ball/player max, forces `PK_ALL = 5`).
    /// # Errors
    /// Returns [`WorldError`] if stone-locked or player already holds a Namek ball.
    pub fn pick_ball(&mut self, star: usize, pid: PlayerId, clan: ClanId, map: MapId, zone: ZoneId, now_ms: u64) -> Result<i8, WorldError> {
        if now_ms < self.stone_until_ms || star >= 7 || self.holders[star].is_some() || self.holders.iter().flatten().any(|(p, _, _, _)| *p == pid) {
            return Err(WorldError::DungeonRule("cannot pick Namek Dragon Ball"));
        }
        self.holders[star] = Some((pid, clan, map, zone));
        Ok(const_player::PK_ALL)
    }
    /// Consumes all 7 balls held by `clan` on Map `7` (`LANG_MORI`) in `zone`, petrifying for `24h`.
    /// # Errors
    /// Returns [`WorldError`] if all 7 balls are not gathered by clanmates on Map `7` in `zone`.
    pub fn consume_for_porunga(&mut self, clan: ClanId, zone: ZoneId, now_ms: u64) -> Result<(), WorldError> {
        if clan.0 < 0 || !self.holders.iter().all(|s| matches!(s, Some((_, c, m, z)) if *c == clan && i32::from(m.0) == const_map::LANG_MORI && *z == zone)) {
            return Err(WorldError::DungeonRule("Porunga requires all 7 Namek balls held by clan on Map 7"));
        }
        self.holders = [None; 7];
        self.stone_until_ms = now_ms.saturating_add(86_400_000);
        Ok(())
    }
    /// Releases any Namek Dragon Ball currently held by `pid`.
    pub fn release_player(&mut self, pid: PlayerId) { for s in &mut self.holders { if matches!(s, Some((p, ..)) if *p == pid) { *s = None; } } }
}

/// Super Divine Water (`SuperDivineWaterService`, Map `146`, `15m` poison timer, 1 win/day).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperDivineWaterSession {
    /// Poison expiration (`now_ms + 900_000 ms`).
    pub deadline_ms: u64,
    /// Scaled `Pocolo` boss `(dame, hp)`.
    pub pocolo_stats: (u64, u64),
}

impl SuperDivineWaterSession {
    /// Starts a `Tây Karin` solo session (`dame = min(pl_dame, 200M)`, `hp = min(pl_hp * 5, 2B)`).
    /// # Errors
    /// Returns [`WorldError`] if already won today.
    pub fn start(won_today: bool, pl_dame: u64, pl_hp_max: u64, now_ms: u64) -> Result<Self, WorldError> {
        if won_today { return Err(WorldError::DungeonRule("already won SuperDivineWater today")); }
        Ok(Self { deadline_ms: now_ms.saturating_add(900_000), pocolo_stats: (pl_dame.min(200_000_000), pl_hp_max.saturating_mul(5).min(2_000_000_000)) })
    }
}

#[cfg(test)]
mod tests {
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
    fn black_ball_war_enforces_2030_and_5min_victory() {
        let mut bb = BlackBallWarEngine::new(1);
        assert!(bb.try_pick(PlayerId(10), ClanId(7), 20, 15, 1_000).is_err());
        assert_eq!(bb.try_pick(PlayerId(10), ClanId(7), 20, 30, 10_000), Ok(8));
        assert_eq!(bb.check_five_minute_victory(310_000), Some(ClanId(7)));
        assert!(bb.try_pick(PlayerId(11), ClanId(8), 20, 36, 320_000).is_err());
    }
    #[test]
    fn mabu_14h_and_namek_balls_guard_duplicates_clanless_and_release() {
        let mut mb = MajinBuu14hInstance::default();
        assert!(mb.eat_player(PlayerId(1), false).is_some());
        assert!(mb.eat_player(PlayerId(1), false).is_none());
        mb.release_player(PlayerId(1));
        assert!(mb.eat_player(PlayerId(1), false).is_some());
        let mut nb = NamekDragonBallTracker::default();
        for i in 0_usize..7 { assert!(nb.pick_ball(i, PlayerId(i64::try_from(i).unwrap_or(0) + 1), ClanId(-1), MapId(7), ZoneId(0), 1_000).is_ok()); }
        assert!(nb.consume_for_porunga(ClanId(-1), ZoneId(0), 2_000).is_err());
        nb.release_player(PlayerId(1));
        assert!(nb.holders[0].is_none());
    }
    #[test]
    fn destron_gas_and_snake_way_evacuate_on_expiry_and_collapse() {
        let mut dg = DestronGasInstance::open(ClanId(1), true, 200_000_000, 0, 50, 10_000).expect("dg");
        assert_eq!(dg.check_evacuation(10_000 + DUNGEON_DURATION_MS - 1), None);
        assert_eq!(dg.check_evacuation(10_000 + DUNGEON_DURATION_MS).map(|e| e.map_id), Some(MapId(0)));
        dg.on_final_boss_killed(20_000);
        assert_eq!(dg.phase, 3);
        assert_eq!(dg.check_evacuation(20_000 + COLLAPSE_DURATION_MS).map(|e| e.map_id), Some(MapId(0)));
        let mut sw = SnakeWayInstance::open(ClanId(1), 50, 200_000_000, 10_000, 0).expect("sw");
        assert_eq!(sw.check_evacuation(10_000 + DUNGEON_DURATION_MS - 1), None);
        assert_eq!(sw.check_evacuation(10_000 + DUNGEON_DURATION_MS).map(|e| e.map_id), Some(MapId(47)));
        sw.on_boss_killed(25_000);
        assert_eq!(sw.check_evacuation(25_000 + COLLAPSE_DURATION_MS).map(|e| e.map_id), Some(MapId(47)));
    }
}
