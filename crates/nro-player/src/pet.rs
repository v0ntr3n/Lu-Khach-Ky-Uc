//! Disciple (`Pet` / `Mabu` / `Uub` / `KidBeer` / `KidJiren`), `NewPet`, and `Fusion` state machines.

use nro_consts as consts;
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, Location, MapId, PlayerId, ZoneId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::npoint::NPoint;
use crate::skills::{EffectSkillState, ManaUseType, SkillKind, SkillSlot};
use crate::{PlayerError, gender_index, rng_range};

#[path = "pet_ai.rs"]
mod pet_ai;
pub use pet_ai::NewPet;

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
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Display,
    EnumCount,
    EnumIter,
    FromRepr,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum PetKind {
    #[default]
    Normal = 0,
    Mabu = 1,
    Uub = 2,
    KidBeer = 3,
    KidJiren = 4,
    DivineHakai = 5,
}

impl PetKind {
    #[inline]
    pub const fn body_slot_count(self) -> usize {
        if matches!(self, Self::Uub | Self::KidBeer | Self::KidJiren) {
            9
        } else {
            7
        }
    }

    #[inline]
    pub const fn can_hakai(self) -> bool {
        matches!(self, Self::Uub | Self::KidJiren | Self::DivineHakai)
    }
}

/// Disciple command state machine (`Pet.status`).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Display,
    EnumCount,
    EnumIter,
    FromRepr,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum PetStatus {
    #[default]
    Follow = 0,
    Protect = 1,
    Attack = 2,
    GoHome = 3,
    Fusion = 4,
    PermanentAssimilate = 5,
}

/// Fusion modes (`ConstPlayer` fusion constants).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Display,
    EnumCount,
    EnumIter,
    FromRepr,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum FusionKind {
    #[default]
    None = 0,
    LuongLongNhatThe = 4,
    Porata1 = 6,
    Porata2 = 8,
    Porata3 = 9,
    Gogeta = 10,
}

impl FusionKind {
    #[inline]
    pub const fn is_porata(self) -> bool {
        matches!(self, Self::Porata1 | Self::Porata2 | Self::Porata3)
    }
}

/// Master-side fusion state (`nro.models.player.Fusion`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionState {
    pub kind: FusionKind,
    pub started_at_ms: u64,
    pub last_unfusion_ms: u64,
}

impl FusionState {
    #[inline]
    pub const fn should_auto_unfuse(&self, now_ms: u64) -> bool {
        matches!(self.kind, FusionKind::LuongLongNhatThe)
            && now_ms.saturating_sub(self.started_at_ms) >= FUSION_DANCE_DURATION_MS
    }
}

/// Candidate target snapshot visible to [`Pet::tick_ai`].
#[derive(Clone, Copy, Debug)]
pub struct PetTargetCandidate {
    pub id: i64,
    pub location: Location,
    pub hp: i32,
    pub is_mob: bool,
    pub is_boss: bool,
    pub has_ltdb: bool,
}

/// Deterministic AI decision emitted by [`Pet::tick_ai`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetAction {
    Idle,
    MoveTo(i16, i16),
    HakaiKill(PlayerId),
    CastSkill(SkillKind, Option<i64>, bool, Option<(i16, i16)>),
    AskPea,
    AssimilateIntoMaster(u64),
}

/// Complete Disciple entity (`nro.models.player.Pet`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pet {
    pub id: PlayerId,
    pub kind: PetKind,
    pub gender: Gender,
    pub status: PetStatus,
    pub is_transform: bool,
    pub location: Location,
    pub npoint: NPoint,
    pub items_body: SmallVec<[Option<ItemInstance>; 9]>,
    pub skills: SmallVec<[Option<SkillSlot>; 5]>,
    pub effects: EffectSkillState,
    pub timers_ms: (u64, u64, u64),
    pub home_dir: i8,
}

impl Pet {
    /// Spawns a new disciple matching `PetService.createNewPet` initial stat distributions.
    pub fn spawn(master_id: PlayerId, kind: PetKind, gender: Gender, rng: &mut GameRng) -> Self {
        let _ = consts::PER100;
        let (power, hpg, mpg, dameg): (i64, i64, i64, i64) = match kind {
            PetKind::Normal => (
                2_000,
                rng_range(rng, 40, 105) * 20,
                rng_range(rng, 40, 105) * 20,
                rng_range(rng, 20, 45),
            ),
            PetKind::Mabu => (
                1_500_000,
                rng_range(rng, 40, 105) * 20,
                rng_range(rng, 40, 105) * 20,
                rng_range(rng, 50, 120),
            ),
            _ => (40_000_000_000, 400_000, 400_000, 20_000),
        };
        let core = CoreStats {
            hpg,
            mpg,
            hp: hpg,
            mp: mpg,
            dameg,
            defg: rng_range(rng, 9, 50) as _,
            critg: rng_range(rng, 0, 2) as _,
            power: power as _,
            tiem_nang: 0 as _,
            limit_power: 0,
            stamina: 1000,
            max_stamina: 1000,
            ..CoreStats::default()
        };
        let s1 =
            [SkillKind::Dragon, SkillKind::Demon, SkillKind::Galick][rng_range(rng, 0, 2) as usize];
        let mut skills = SmallVec::from_elem(None, 5);
        skills[0] = Some(mk_pet_slot(s1, 500, 2, ManaUseType::Flat, 100, 0));
        let mut items_body = SmallVec::new();
        items_body.resize(kind.body_slot_count(), None);
        Self {
            id: PlayerId(-(master_id.0.abs().max(1))),
            kind,
            gender,
            status: PetStatus::Follow,
            is_transform: false,
            location: Location {
                map_id: MapId(21),
                zone_id: ZoneId(0),
                x: 200,
                y: 336,
            },
            npoint: NPoint::new(core),
            items_body,
            skills,
            effects: EffectSkillState::default(),
            timers_ms: (0, 0, 0),
            home_dir: -1,
        }
    }

    /// Unlocks Milestone Skills 2–5 (`150M`, `1.5B`, `20B`, `40B` power) (`Pet.updatePower`).
    pub fn check_skill_unlocks(&mut self, rng: &mut GameRng) {
        if self.skills.len() < 5 {
            self.skills.resize(5, None);
        }
        let p = self.npoint.core.power as i64;
        if self.skills[1].is_none() && p >= 150_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 33 {
                SkillKind::Kamejoko
            } else if r <= 66 {
                SkillKind::Masenko
            } else {
                SkillKind::Antomic
            };
            self.skills[1] = Some(mk_pet_slot(k, 1_000, 10, ManaUseType::Flat, 120, 0));
        }
        if self.skills[1].is_some() && self.skills[2].is_none() && p >= 1_500_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 30 {
                SkillKind::ThaiDuongHaSan
            } else if r <= 70 {
                SkillKind::TaiTaoNangLuong
            } else {
                SkillKind::Kaioken
            };
            self.skills[2] = Some(mk_pet_slot(k, 15_000, 10, ManaUseType::PercentMax, 130, 0));
        }
        if self.skills[2].is_some() && self.skills[3].is_none() && p >= 20_000_000_000 {
            let r = rng_range(rng, 1, 100);
            let k = if r <= 10 {
                SkillKind::BienKhi
            } else if r <= 80 {
                SkillKind::DeTrung
            } else {
                SkillKind::KhienNangLuong
            };
            self.skills[3] = Some(mk_pet_slot(k, 60_000, 15, ManaUseType::PercentMax, 100, 0));
        }
        if self.skills[3].is_some()
            && self.skills[4].is_none()
            && p >= 40_000_000_000
            && matches!(
                self.kind,
                PetKind::Uub | PetKind::KidBeer | PetKind::KidJiren
            )
        {
            let k = [
                SkillKind::SuperKame,
                SkillKind::MaPhongBa,
                SkillKind::LienHoanChuong,
            ][usize::from(gender_index(self.gender))];
            self.skills[4] = Some(mk_pet_slot(k, 30_000, 20, ManaUseType::PercentMax, 200, 1));
        }
    }

    /// Executes a command transition (`Pet.changeStatus` / `fusion`).
    pub fn change_status(
        &mut self,
        next: PetStatus,
        master_fusion: &mut FusionState,
        desired_fusion: FusionKind,
        now_ms: u64,
    ) -> Result<(), PlayerError> {
        if master_fusion.kind != FusionKind::None
            || (self.npoint.hp <= 0 && next == PetStatus::Fusion)
        {
            return Err(PlayerError::ActionBlocked);
        }
        if next == PetStatus::Fusion {
            if master_fusion.last_unfusion_ms > 0
                && now_ms.saturating_sub(master_fusion.last_unfusion_ms) < UNFUSION_COOLDOWN_MS
            {
                return Err(PlayerError::FusionCooldownActive);
            }
            master_fusion.kind = if desired_fusion == FusionKind::None {
                FusionKind::LuongLongNhatThe
            } else {
                desired_fusion
            };
            master_fusion.started_at_ms = now_ms;
        }
        self.status = next;
        Ok(())
    }

    /// Unfuses pet back into `Protect` mode (`Pet.unFusion`).
    pub fn unfuse(&mut self, master_fusion: &mut FusionState, now_ms: u64) {
        (
            master_fusion.kind,
            master_fusion.last_unfusion_ms,
            self.status,
        ) = (FusionKind::None, now_ms, PetStatus::Protect);
    }
}

const fn mk_pet_slot(
    kind: SkillKind,
    cd: u32,
    mana: u32,
    mana_type: ManaUseType,
    dmg: u16,
    curr: u16,
) -> SkillSlot {
    SkillSlot {
        kind,
        skill_id: kind as i16,
        point: 1,
        cool_down_ms: cd,
        mana_use: mana,
        mana_type,
        damage_pct: dmg,
        curr_level: curr,
        last_used_ms: 0,
    }
}
