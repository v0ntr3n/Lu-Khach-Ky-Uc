//! Deterministic `Pet::tick_ai` combat, Hakai, support-skill, and leash movement state machine.

use nro_core::{GameRng, Gender, Location, ParityMode, PlayerId};
use serde::{Deserialize, Serialize};

use super::{
    PET_ATTACK_RANGE_PX, PET_MELEE_RANGE_PX, PET_REVIVE_DELAY_MS, Pet, PetAction, PetStatus,
    PetTargetCandidate,
};
use crate::npoint::StatUpgradeKind;
use crate::skills::{SkillKind, SkillSlot};
use crate::{distance_2d, gender_index, map_raw, rng_range, rng_ratio};

impl Pet {
    /// Runs 1 tick of the `Pet` state machine (`Pet.update`), auto-allocating up to `20` stat points.
    pub fn tick_ai(
        &mut self,
        master_loc: Location,
        master_gender: Gender,
        master_afk: bool,
        candidates: &[PetTargetCandidate],
        now_ms: u64,
        parity: ParityMode,
        rng: &mut GameRng,
    ) -> PetAction {
        for _ in 0..20 {
            if let Some(k) = StatUpgradeKind::from_repr(rng_range(rng, 0, 4) as u8) {
                let _ = self.npoint.increase_point(k, 1, parity);
            }
        }
        self.check_skill_unlocks(rng);
        if self.npoint.hp <= 0 {
            if self.timers_ms.0 > 0
                && now_ms.saturating_sub(self.timers_ms.0) >= PET_REVIVE_DELAY_MS
            {
                (self.npoint.hp, self.npoint.mp, self.timers_ms.0) =
                    (self.npoint.hp_max, self.npoint.mp_max, 0);
            }
            return PetAction::Idle;
        }
        if self.effects.is_locked() || self.status == PetStatus::Fusion {
            return PetAction::Idle;
        }
        match self.status {
            PetStatus::Fusion => PetAction::Idle,
            PetStatus::Follow => self.follow_step(master_loc, 60),
            PetStatus::GoHome => {
                if matches!(map_raw(self.location.map_id), 21..=23)
                    && now_ms.saturating_sub(self.timers_ms.2) > 5_000
                {
                    let x = match (map_raw(self.location.map_id), self.home_dir < 0) {
                        (22, true) => 500,
                        (22, false) => 452,
                        (_, true) => 250,
                        _ => 200,
                    };
                    (
                        self.home_dir,
                        self.timers_ms.2,
                        self.location.x,
                        self.location.y,
                    ) = (-self.home_dir, now_ms, x, 336);
                    PetAction::MoveTo(x, 336)
                } else {
                    PetAction::Idle
                }
            }
            PetStatus::PermanentAssimilate => {
                if gender_index(master_gender) == 1 {
                    PetAction::AssimilateIntoMaster((self.npoint.core.power as i64).max(0) as u64)
                } else {
                    PetAction::Idle
                }
            }
            PetStatus::Protect | PetStatus::Attack => {
                self.tick_combat(master_loc, master_afk, candidates, now_ms, rng)
            }
        }
    }

    fn tick_combat(
        &mut self,
        master_loc: Location,
        master_afk: bool,
        candidates: &[PetTargetCandidate],
        now_ms: u64,
        rng: &mut GameRng,
    ) -> PetAction {
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
        if let Some(sup) = self.try_support_skills(candidates, now_ms) {
            return sup;
        }
        if master_afk {
            return PetAction::Idle;
        }
        let nearest = candidates
            .iter()
            .filter(|c| distance_2d(&self.location, &c.location) <= PET_ATTACK_RANGE_PX)
            .min_by_key(|c| (c.is_mob, distance_2d(&self.location, &c.location)));
        let Some(t) = nearest else {
            return self.follow_step(master_loc, 60);
        };
        if self.npoint.core.stamina == 0 {
            return self.request_pea(now_ms);
        }
        let idx = usize::from(
            distance_2d(&self.location, &t.location) > PET_MELEE_RANGE_PX
                && self.skills.get(1).copied().flatten().is_some(),
        );
        let Some(slot) = self.skills.get(idx).copied().flatten() else {
            return PetAction::Idle;
        };
        if !slot.is_off_cooldown(now_ms) {
            return PetAction::Idle;
        }
        if slot.mana_cost(self.npoint.mp, self.npoint.mp_max).is_none() {
            return self.request_pea(now_ms);
        }
        let move_to = if idx == 0 || (self.status == PetStatus::Attack && t.is_mob) {
            let j = if t.is_mob && self.status == PetStatus::Attack {
                20
            } else {
                60
            };
            Some((
                t.location.x.saturating_add(rng_range(rng, -j, j) as i16),
                t.location.y,
            ))
        } else {
            None
        };
        PetAction::CastSkill(slot.kind, Some(t.id), t.is_mob, move_to)
    }

    fn try_support_skills(
        &self,
        candidates: &[PetTargetCandidate],
        now_ms: u64,
    ) -> Option<PetAction> {
        let ready = |s: Option<SkillSlot>| {
            s.filter(|x| {
                x.is_off_cooldown(now_ms)
                    && x.mana_cost(self.npoint.mp, self.npoint.mp_max).is_some()
            })
        };
        if let Some(s3) = ready(self.skills.get(2).copied().flatten()) {
            match s3.kind {
                SkillKind::ThaiDuongHaSan => {
                    return Some(PetAction::CastSkill(s3.kind, None, false, None));
                }
                SkillKind::TaiTaoNangLuong
                    if self.npoint.curr_percent_hp() <= 20
                        || (i64::from(self.npoint.mp) * 100
                            / i64::from(self.npoint.mp_max.max(1)))
                            <= 20 =>
                {
                    return Some(PetAction::CastSkill(s3.kind, None, false, None));
                }
                SkillKind::Kaioken => {
                    if let Some(t) = candidates.first() {
                        return Some(PetAction::CastSkill(
                            s3.kind,
                            Some(t.id),
                            t.is_mob,
                            Some((t.location.x, t.location.y)),
                        ));
                    }
                }
                _ => {}
            }
        }
        if let Some(s4) = ready(self.skills.get(3).copied().flatten())
            && ((s4.kind == SkillKind::BienKhi && self.effects.monkey_level == 0)
                || (s4.kind == SkillKind::KhienNangLuong && self.effects.shield_until_ms == 0)
                || (s4.kind == SkillKind::DeTrung && self.effects.mob_me.is_none()))
        {
            return Some(PetAction::CastSkill(s4.kind, None, false, None));
        }
        ready(self.skills.get(4).copied().flatten())
            .zip(candidates.first())
            .map(|(s5, t)| PetAction::CastSkill(s5.kind, Some(t.id), t.is_mob, None))
    }

    fn follow_step(&mut self, master_loc: Location, leash: i32) -> PetAction {
        if distance_2d(&self.location, &master_loc) >= leash {
            let nx = if self.location.x < master_loc.x {
                master_loc.x - 50
            } else {
                master_loc.x + 50
            };
            (self.location.x, self.location.y) = (nx, master_loc.y);
            PetAction::MoveTo(nx, master_loc.y)
        } else {
            PetAction::Idle
        }
    }

    fn request_pea(&mut self, now_ms: u64) -> PetAction {
        if now_ms.saturating_sub(self.timers_ms.1) >= 10_000 {
            self.timers_ms.1 = now_ms;
            PetAction::AskPea
        } else {
            PetAction::Idle
        }
    }
}

/// Untargetable cosmetic/aura mini-pet equipped in `itemsBody[7]` (`NewPet.java`, `500M` HP/MP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPet {
    pub id: PlayerId,
    pub parts: (i16, i16, i16),
    pub location: Location,
}

impl NewPet {
    /// Resolves a slot-7 pet item ID (`892..=1789`) to `(head, body, leg)` (`Player.sendNewPet`).
    pub const fn parts_for_item(item_id: u16) -> Option<(i16, i16, i16)> {
        const MAP: [(u16, (i16, i16, i16)); 30] = [
            (892, (882, 883, 884)),
            (893, (885, 886, 887)),
            (908, (891, 892, 893)),
            (909, (894, 895, 896)),
            (910, (897, 898, 899)),
            (916, (925, 926, 927)),
            (917, (928, 929, 930)),
            (918, (931, 932, 933)),
            (919, (934, 935, 936)),
            (936, (718, 719, 720)),
            (942, (966, 967, 968)),
            (943, (969, 970, 971)),
            (944, (972, 973, 974)),
            (967, (1050, 1051, 1052)),
            (1008, (1074, 1075, 1076)),
            (1039, (1089, 1090, 1091)),
            (1040, (1092, 1093, 1094)),
            (1046, (-1, -1, -1)),
            (1107, (1155, 1156, 1157)),
            (1114, (1158, 1159, 1160)),
            (1188, (1183, 1184, 1185)),
            (1202, (1201, 1202, 1203)),
            (1203, (1201, 1202, 1203)),
            (1207, (1077, 1078, 1079)),
            (1668, (1550, 1551, 1552)),
            (1765, (1662, 1663, 1664)),
            (1766, (1665, 1666, 1667)),
            (1767, (1668, 1669, 1670)),
            (1768, (1671, 1672, 1673)),
            (1789, (1724, 1725, 1726)),
        ];
        if item_id >= 1769 && item_id <= 1771 {
            let b = 1674 + ((item_id - 1769) as i16) * 3;
            return Some((b, b + 1, b + 2));
        }
        let mut i = 0;
        while i < MAP.len() {
            if MAP[i].0 == item_id {
                return Some(MAP[i].1);
            }
            i += 1;
        }
        None
    }

    /// Leashes `NewPet` within `50` px of `master_loc` (`NewPet.followMaster`).
    pub fn follow_master(&mut self, master_loc: Location, rng: &mut GameRng) -> bool {
        if distance_2d(&self.location, &master_loc) >= 50 {
            let off = rng_range(rng, 0, 50) as i16;
            let x = if self.location.x < master_loc.x {
                master_loc.x - off
            } else {
                master_loc.x + off
            };
            self.location = Location {
                map_id: master_loc.map_id,
                zone_id: master_loc.zone_id,
                x,
                y: master_loc.y,
            };
            true
        } else {
            false
        }
    }
}
