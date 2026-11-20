//! Disciple (`Pet` / `Mabu` / `Uub` / `KidBeer` / `KidJiren`), `NewPet`, and `Fusion` state machines.
//!
//! Ports `Pet.java`, `NewPet.java`, `Fusion.java`, and `PetService.java`, including `Follow`,
//! `Protect` (with `1/5` Hakai proc), `Attack`, `GoHome`, `Fusion` (Fusion Dance + Potara Lv1–3
//! + Gogeta), and Namekian permanent assimilation (`HTVV`).

use nro_consts as consts;
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, Location, MapId, ParityMode, PlayerId, ZoneId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::npoint::{NPoint, StatUpgradeKind};
use crate::skills::{EffectSkillState, ManaUseType, SkillKind, SkillSlot};
use crate::{PlayerError, distance_2d, gender_index, map_raw, rng_range, rng_ratio};

/// Maximum target acquisition distance for `Pet` (`Pet.ARANGE_CAN_ATTACK = 300`).
pub const PET_ATTACK_RANGE_PX: i32 = 300;
/// Melee switch distance (`Pet.ARANGE_ATT_SKILL1 = 50`).
pub const PET_MELEE_RANGE_PX: i32 = 50;
/// Timed Fusion Dance duration (`Fusion.TIME_FUSION = 600_000` ms = 10 minutes).
pub const FUSION_DANCE_DURATION_MS: u64 = 600_000;
/// Cooldown after unfusing before fusing again (`Pet.TIME_WAIT_AFTER_UNFUSION = 5_000` ms).
pub const UNFUSION_COOLDOWN_MS: u64 = 5_000;
/// Automatic pet revival timer (`120_000` ms).
pub const PET_REVIVE_DELAY_MS: u64 = 120_000;

/// Disciple archetypes (`Pet.typePet` in `PetService.java`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum PetKind {
    /// Normal disciple (`0`: `2,000` initial power, `7` body slots).
    #[default]
    Normal = 0,
    /// Majin Buu (`1`: `1,500,000` power, `7` body slots, `+0%` Porata bonus).
    Mabu = 1,
    /// Uub (`2`: `40B` power, `9` body slots, `+20%` Porata stats, Hakai).
    Uub = 2,
    /// Kid Beerus (`3`: `40B` power, `9` body slots, `+20%` Porata stats / `+44%` dame in legacy Java).
    KidBeer = 3,
    /// Kid Jiren (`4`: `40B` power, `9` body slots, `+20%` Porata stats, Hakai).
    KidJiren = 4,
    /// Extended Hakai variant (`5`).
    DivineHakai = 5,
}

impl PetKind {
    /// Returns body equipment capacity (`9` for `Uub`/`KidBeer`/`KidJiren`, else `7`).
    #[inline] pub const fn body_slot_count(self) -> usize { if matches!(self, Self::Uub | Self::KidBeer | Self::KidJiren) { 9 } else { 7 } }
    /// Returns `true` if this pet can proc the `1/5` Hakai instant-kill in `Protect` mode.
    #[inline] pub const fn can_hakai(self) -> bool { matches!(self, Self::Uub | Self::KidJiren | Self::DivineHakai) }
}

/// Disciple command state machine (`Pet.status`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum PetStatus {
    /// Passively follows master within `60` px (`0`).
    #[default]
    Follow = 0,
    /// Defends master within `300` px and can trigger Hakai (`1`).
    Protect = 1,
    /// Aggressively chases targets up to `1,000` px from master (`2`).
    Attack = 2,
    /// Walks back and forth inside master's house (`map 21/22/23`) (`3`).
    GoHome = 3,
    /// Merged into master via Fusion Dance or Potara (`4`).
    Fusion = 4,
    /// Namekian permanent assimilation (`HTVV = 5`), converting pet power into master `tiem_nang`.
    PermanentAssimilate = 5,
}

/// Fusion modes (`ConstPlayer` fusion constants).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum FusionKind {
    /// Not fused (`0`).
    #[default]
    None = 0,
    /// Timed Fusion Dance (`Lưỡng Long Nhất Thể = 4`, `600,000` ms).
    LuongLongNhatThe = 4,
    /// Potara Earring Tier 1 (`Bông Tai Porata = 6`).
    Porata1 = 6,
    /// Potara Earring Tier 2 (`Bông Tai Cấp 2 = 8`, item `921`).
    Porata2 = 8,
    /// Potara Earring Tier 3 (`Bông Tai Cấp 3 = 9`, item `1819`).
    Porata3 = 9,
    /// Gogeta fusion (`10`).
    Gogeta = 10,
}

impl FusionKind {
    /// Returns `true` for Potara Tiers 1–3 (`6, 8, 9`).
    #[inline] pub const fn is_porata(self) -> bool { matches!(self, Self::Porata1 | Self::Porata2 | Self::Porata3) }
}

/// Master-side fusion state (`nro.models.player.Fusion`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionState {
    /// Current fusion type.
    pub kind: FusionKind,
    /// Timestamp (`ms`) when current fusion started.
    pub started_at_ms: u64,
    /// Timestamp (`ms`) when last unfusion occurred (`5,000` ms cooldown).
    pub last_unfusion_ms: u64,
}

impl FusionState {
    /// Checks whether `LuongLongNhatThe` has expired after `600,000` ms (`Fusion.update`).
    #[inline]
    pub const fn should_auto_unfuse(&self, now_ms: u64) -> bool {
        matches!(self.kind, FusionKind::LuongLongNhatThe) && now_ms.saturating_sub(self.started_at_ms) >= FUSION_DANCE_DURATION_MS
    }
}

/// Candidate target snapshot visible to [`Pet::tick_ai`].
#[derive(Clone, Copy, Debug)]
pub struct PetTargetCandidate {
    /// Entity ID.
    pub id: i64,
    /// Position in current zone.
    pub location: Location,
    /// Current HP (`< 1_000_000_000` check for Hakai).
    pub hp: i32,
    /// `true` if mob (`false` if player/boss).
    pub is_mob: bool,
    /// Whether player target is a Boss.
    pub is_boss: bool,
    /// Whether target has 5pc `Linh Thủy Đánh Bạc` immunity (`islinhthuydanhbac`).
    pub has_ltdb: bool,
}

/// Deterministic AI decision emitted by [`Pet::tick_ai`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetAction {
    /// Stay idle or wander around master.
    Idle,
    /// Move to `(x, y)`.
    MoveTo(i16, i16),
    /// Execute `Hakai` instant kill on player `id`.
    HakaiKill(PlayerId),
    /// Cast `(skill, target_id, is_mob, optional_move_to)`.
    CastSkill(SkillKind, Option<i64>, bool, Option<(i16, i16)>),
    /// Request a magic pea (`Đậu thần`) from master (`10,000` ms cooldown).
    AskPea,
    /// Permanently assimilate into Namekian master, granting `power_to_tn` potential.
    AssimilateIntoMaster(u64),
}

/// Complete Disciple entity (`nro.models.player.Pet`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pet {
    /// Disciple negative `PlayerId` (`-master_id`).
    pub id: PlayerId,
    /// Archetype (`Normal`, `Mabu`, `Uub`, `KidBeer`, `KidJiren`).
    pub kind: PetKind,
    /// Planet class (`Gender`).
    pub gender: Gender,
    /// Current AI state (`Follow`, `Protect`, `Attack`, `GoHome`, `Fusion`, `PermanentAssimilate`).
    pub status: PetStatus,
    /// Visual transformation toggle.
    pub is_transform: bool,
    /// Spatial location.
    pub location: Location,
    /// Stat calculator & current HP/MP/Stamina.
    pub npoint: NPoint,
    /// Equipped items (`7` or `9` slots).
    pub items_body: SmallVec<[Option<ItemInstance>; 9]>,
    /// Learned skill slots (`0..5`).
    pub skills: SmallVec<[Option<SkillSlot>; 5]>,
    /// Active CC/buff timers.
    pub effects: EffectSkillState,
    /// Timestamps `(died_at_ms, last_ask_pea_ms, last_home_pace_ms)`.
    pub timers_ms: (u64, u64, u64),
    /// Home pacing direction (`-1` or `1`).
    pub home_dir: i8,
}

impl Pet {
    /// Spawns a new disciple matching `PetService.createNewPet` initial stat distributions.
    pub fn spawn(master_id: PlayerId, kind: PetKind, gender: Gender, rng: &mut GameRng) -> Self {
        let _ = consts::PER100;
        let (power, hpg, mpg, dameg): (i64, i64, i64, i64) = match kind {
            PetKind::Normal => (2_000, rng_range(rng, 40, 105) * 20, rng_range(rng, 40, 105) * 20, rng_range(rng, 20, 45)),
            PetKind::Mabu => (1_500_000, rng_range(rng, 40, 105) * 20, rng_range(rng, 40, 105) * 20, rng_range(rng, 50, 120)),
            _ => (40_000_000_000, 400_000, 400_000, 20_000),
        };
        let core = CoreStats {
            hpg, mpg, hp: hpg, mp: mpg, dameg, defg: rng_range(rng, 9, 50) as _, critg: rng_range(rng, 0, 2) as _,
            power: power as _, tiem_nang: 0 as _, limit_power: 0, stamina: 1000, max_stamina: 1000, ..CoreStats::default() };
        let s1 = [SkillKind::Dragon, SkillKind::Demon, SkillKind::Galick][rng_range(rng, 0, 2) as usize];
        let mut skills = SmallVec::from_elem(None, 5);
        skills[0] = Some(mk_pet_slot(s1, 500, 2, ManaUseType::Flat, 100, 0));
        let mut items_body = SmallVec::new();
        items_body.resize(kind.body_slot_count(), None);
        Self {
            id: PlayerId(-(master_id.0.abs().max(1))), kind, gender, status: PetStatus::Follow, is_transform: false,
            location: Location { map_id: MapId(21), zone_id: ZoneId(0), x: 200, y: 336 },
            npoint: NPoint::new(core), items_body, skills, effects: EffectSkillState::default(),
            timers_ms: (0, 0, 0), home_dir: -1,
        }
    }

    /// Unlocks Milestone Skills 2–5 (`150M`, `1.5B`, `20B`, `40B` power) (`Pet.updatePower`).
    pub fn check_skill_unlocks(&mut self, rng: &mut GameRng) {
        let p = self.npoint.core.power as i64;
        if self.skills[1].is_none() && p >= 150_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 33 { SkillKind::Kamejoko } else if r <= 66 { SkillKind::Masenko } else { SkillKind::Antomic };
            self.skills[1] = Some(mk_pet_slot(k, 1_000, 10, ManaUseType::Flat, 120, 0));
        }
        if self.skills[1].is_some() && self.skills[2].is_none() && p >= 1_500_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 30 { SkillKind::ThaiDuongHaSan } else if r <= 70 { SkillKind::TaiTaoNangLuong } else { SkillKind::Kaioken };
            self.skills[2] = Some(mk_pet_slot(k, 15_000, 10, ManaUseType::PercentMax, 130, 0));
        }
        if self.skills[2].is_some() && self.skills[3].is_none() && p >= 20_000_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 10 { SkillKind::BienKhi } else if r <= 80 { SkillKind::DeTrung } else { SkillKind::KhienNangLuong };
            self.skills[3] = Some(mk_pet_slot(k, 60_000, 15, ManaUseType::PercentMax, 100, 0));
        }
        if self.skills[3].is_some() && self.skills[4].is_none() && p >= 40_000_000_000 && matches!(self.kind, PetKind::Uub | PetKind::KidBeer | PetKind::KidJiren) {
            let k = [SkillKind::SuperKame, SkillKind::MaPhongBa, SkillKind::LienHoanChuong][usize::from(gender_index(self.gender))];
            self.skills[4] = Some(mk_pet_slot(k, 30_000, 20, ManaUseType::PercentMax, 200, 1));
        }
    }

    /// Executes a command transition (`Pet.changeStatus` / `fusion`).
    pub fn change_status(&mut self, next: PetStatus, master_fusion: &mut FusionState, desired_fusion: FusionKind, now_ms: u64) -> Result<(), PlayerError> {
        if master_fusion.kind != FusionKind::None || (self.npoint.hp <= 0 && next == PetStatus::Fusion) {
            return Err(PlayerError::ActionBlocked);
        }
        if next == PetStatus::Fusion {
            if now_ms.saturating_sub(master_fusion.last_unfusion_ms) < UNFUSION_COOLDOWN_MS { return Err(PlayerError::FusionCooldownActive); }
            master_fusion.kind = if desired_fusion == FusionKind::None { FusionKind::LuongLongNhatThe } else { desired_fusion };
            master_fusion.started_at_ms = now_ms;
        }
        self.status = next;
        Ok(())
    }

    /// Unfuses pet back into `Protect` mode (`Pet.unFusion`).
    pub fn unfuse(&mut self, master_fusion: &mut FusionState, now_ms: u64) {
        (master_fusion.kind, master_fusion.last_unfusion_ms, self.status) = (FusionKind::None, now_ms, PetStatus::Protect);
    }

    /// Runs 1 tick of the `Pet` state machine (`Pet.update`), auto-allocating up to `20` stat points.
    pub fn tick_ai(&mut self, master_loc: Location, master_gender: Gender, master_afk: bool, candidates: &[PetTargetCandidate], now_ms: u64, parity: ParityMode, rng: &mut GameRng) -> PetAction {
        for _ in 0..20 {
            if let Some(k) = StatUpgradeKind::from_repr(rng_range(rng, 0, 4) as u8) { let _ = self.npoint.increase_point(k, 1, parity); }
        }
        self.check_skill_unlocks(rng);
        if self.npoint.hp <= 0 {
            if self.timers_ms.0 > 0 && now_ms.saturating_sub(self.timers_ms.0) >= PET_REVIVE_DELAY_MS {
                (self.npoint.hp, self.npoint.mp, self.timers_ms.0) = (self.npoint.hp_max, self.npoint.mp_max, 0);
            }
            return PetAction::Idle;
        }
        if self.effects.is_locked() || self.status == PetStatus::Fusion { return PetAction::Idle; }
        match self.status {
            PetStatus::Fusion => PetAction::Idle,
            PetStatus::Follow => self.follow_step(master_loc, 60),
            PetStatus::GoHome => {
                if matches!(map_raw(self.location.map_id), 21..=23) && now_ms.saturating_sub(self.timers_ms.2) > 5_000 {
                    let x = match (map_raw(self.location.map_id), self.home_dir < 0) { (22, true) => 500, (22, false) => 452, (_, true) => 250, _ => 200 };
                    (self.home_dir, self.timers_ms.2, self.location.x, self.location.y) = (-self.home_dir, now_ms, x, 336);
                    PetAction::MoveTo(x, 336)
                } else { PetAction::Idle }
            }
            PetStatus::PermanentAssimilate => if gender_index(master_gender) == 1 {
                PetAction::AssimilateIntoMaster((self.npoint.core.power as i64).max(0) as u64)
            } else { PetAction::Idle },
            PetStatus::Protect | PetStatus::Attack => {
                if !master_afk
                    && self.status == PetStatus::Protect
                    && self.kind.can_hakai()
                    && let Some(t) = candidates
                        .iter()
                        .filter(|c| distance_2d(&self.location, &c.location) <= PET_ATTACK_RANGE_PX)
                        .min_by_key(|c| (c.is_mob, distance_2d(&self.location, &c.location)))
                    && !t.is_mob
                    && !t.is_boss
                    && !t.has_ltdb
                    && t.hp < 1_000_000_000
                    && rng_ratio(rng, 1, 5)
                {
                    return PetAction::HakaiKill(PlayerId(t.id));
                }
                if let Some(sup) = self.try_support_skills(candidates, now_ms) { return sup; }
                if master_afk { return PetAction::Idle; }
                let nearest = candidates.iter().filter(|c| distance_2d(&self.location, &c.location) <= PET_ATTACK_RANGE_PX).min_by_key(|c| (c.is_mob, distance_2d(&self.location, &c.location)));
                let Some(t) = nearest else { return self.follow_step(master_loc, 60); };
                if self.npoint.core.stamina == 0 { return self.request_pea(now_ms); }
                let idx = usize::from(distance_2d(&self.location, &t.location) > PET_MELEE_RANGE_PX && self.skills[1].is_some());
                let Some(slot) = self.skills[idx] else { return PetAction::Idle; };
                if !slot.is_off_cooldown(now_ms) { return PetAction::Idle; }
                if slot.mana_cost(self.npoint.mp, self.npoint.mp_max).is_none() { return self.request_pea(now_ms); }
                let move_to = if idx == 0 || (self.status == PetStatus::Attack && t.is_mob) {
                    let j = if t.is_mob && self.status == PetStatus::Attack { 20 } else { 60 };
                    Some((t.location.x.saturating_add(rng_range(rng, -j, j) as i16), t.location.y))
                } else { None };
                PetAction::CastSkill(slot.kind, Some(t.id), t.is_mob, move_to)
            }
        }
    }

    fn try_support_skills(&self, candidates: &[PetTargetCandidate], now_ms: u64) -> Option<PetAction> {
        let ready = |s: Option<SkillSlot>| s.filter(|x| x.is_off_cooldown(now_ms) && x.mana_cost(self.npoint.mp, self.npoint.mp_max).is_some());
        if let Some(s3) = ready(self.skills[2]) {
            match s3.kind {
                SkillKind::ThaiDuongHaSan => return Some(PetAction::CastSkill(s3.kind, None, false, None)),
                SkillKind::TaiTaoNangLuong if self.npoint.curr_percent_hp() <= 20 || (self.npoint.mp * 100 / self.npoint.mp_max.max(1)) <= 20 => {
                    return Some(PetAction::CastSkill(s3.kind, None, false, None));
                }
                SkillKind::Kaioken => if let Some(t) = candidates.first() {
                    return Some(PetAction::CastSkill(s3.kind, Some(t.id), t.is_mob, Some((t.location.x, t.location.y))));
                },
                _ => {}
            }
        }
        if let Some(s4) = ready(self.skills[3]) && ((s4.kind == SkillKind::BienKhi && self.effects.monkey_level == 0) || (s4.kind == SkillKind::KhienNangLuong && self.effects.shield_until_ms == 0) || (s4.kind == SkillKind::DeTrung && self.effects.mob_me.is_none())) {
            return Some(PetAction::CastSkill(s4.kind, None, false, None));
        }
        ready(self.skills[4]).zip(candidates.first()).map(|(s5, t)| PetAction::CastSkill(s5.kind, Some(t.id), t.is_mob, None))
    }

    fn follow_step(&mut self, master_loc: Location, leash: i32) -> PetAction {
        if distance_2d(&self.location, &master_loc) >= leash {
            let nx = if self.location.x < master_loc.x { master_loc.x - 50 } else { master_loc.x + 50 };
            (self.location.x, self.location.y) = (nx, master_loc.y);
            PetAction::MoveTo(nx, master_loc.y)
        } else { PetAction::Idle }
    }

    fn request_pea(&mut self, now_ms: u64) -> PetAction {
        if now_ms.saturating_sub(self.timers_ms.1) >= 10_000 { self.timers_ms.1 = now_ms; PetAction::AskPea } else { PetAction::Idle }
    }
}

const fn mk_pet_slot(kind: SkillKind, cd: u32, mana: u32, mana_type: ManaUseType, dmg: u16, curr: u16) -> SkillSlot {
    SkillSlot { kind, skill_id: kind as i16, point: 1, cool_down_ms: cd, mana_use: mana, mana_type, damage_pct: dmg, curr_level: curr, last_used_ms: 0 }
}

/// Untargetable cosmetic/aura mini-pet equipped in `itemsBody[7]` (`NewPet.java`, `500M` HP/MP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPet {
    /// Negative entity ID (`<= -310_720_020`).
    pub id: PlayerId,
    /// Sprite parts `(head, body, leg)`.
    pub parts: (i16, i16, i16),
    /// Current position (`50` px leash from master).
    pub location: Location,
}

impl NewPet {
    /// Resolves a slot-7 pet item ID (`892..=1789`) to `(head, body, leg)` (`Player.sendNewPet`).
    pub const fn parts_for_item(item_id: u16) -> Option<(i16, i16, i16)> {
        const MAP: [(u16, (i16, i16, i16)); 30] = [
            (892, (882, 883, 884)), (893, (885, 886, 887)), (908, (891, 892, 893)), (909, (894, 895, 896)),
            (910, (897, 898, 899)), (916, (925, 926, 927)), (917, (928, 929, 930)), (918, (931, 932, 933)),
            (919, (934, 935, 936)), (936, (718, 719, 720)), (942, (966, 967, 968)), (943, (969, 970, 971)),
            (944, (972, 973, 974)), (967, (1050, 1051, 1052)), (1008, (1074, 1075, 1076)), (1039, (1089, 1090, 1091)),
            (1040, (1092, 1093, 1094)), (1046, (-1, -1, -1)), (1107, (1155, 1156, 1157)), (1114, (1158, 1159, 1160)),
            (1188, (1183, 1184, 1185)), (1202, (1201, 1202, 1203)), (1203, (1201, 1202, 1203)), (1207, (1077, 1078, 1079)),
            (1668, (1550, 1551, 1552)), (1765, (1662, 1663, 1764)), (1766, (1665, 1666, 1667)), (1767, (1668, 1669, 1670)),
            (1768, (1671, 1672, 1673)), (1789, (1724, 1725, 1726)),
        ];
        if item_id >= 1769 && item_id <= 1771 {
            let b = 1674 + ((item_id - 1769) as i16) * 3;
            return Some((b, b + 1, b + 2));
        }
        let mut i = 0;
        while i < MAP.len() { if MAP[i].0 == item_id { return Some(MAP[i].1); } i += 1; }
        None
    }

    /// Leashes `NewPet` within `50` px of `master_loc` (`NewPet.followMaster`).
    pub fn follow_master(&mut self, master_loc: Location, rng: &mut GameRng) -> bool {
        if distance_2d(&self.location, &master_loc) >= 50 {
            let off = rng_range(rng, 0, 50) as i16;
            self.location = Location { map_id: master_loc.map_id, zone_id: master_loc.zone_id, x: if self.location.x < master_loc.x { master_loc.x - off } else { master_loc.x + off }, y: master_loc.y };
            true
        } else { false }
    }
}
