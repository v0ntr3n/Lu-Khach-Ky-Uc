//! Regular `Mob`, `MobPoint`, `MobEffectSkill`, and `MobMe` mechanics (`models/mob/`).
//!
//! Reproduces 1-shot full-HP protection, Destron Gas `CO_MAY_HUY_DIET (76)` beam shielding,
//! `3,000ms` world respawns, `10,000ms` Doanh Trại Bulon respawns, `30,000ms` `10%` regen,
//! Mafuba jar DoT, `MobMe` `5%` hit caps, and Map Cadic (`>= 165`) Kilis option `250` procs.

use crate::{
    EngineEffect, GameRng, Location, MapId, ParityMode, PlayerId, PlayerView, SkillKind,
    TemplateId, loc_distance,
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Map ruleset governing `Mob` respawn cadence in `Mob.update()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MobMapKind {
    /// Open-world map (`3,000ms` respawn + `30,000ms` `10%` HP recovery).
    #[default]
    NormalWorld,
    /// Doanh Trại (`10,000ms` respawn ONLY for `BULON` `temp_id == 22` while `is_tut_alive`).
    RedRibbonDungeon,
    /// Non-respawning instanced dungeon (`BĐKB`, `CDRĐ`, `Khí Gas`, `Tây Karin`).
    NoRespawnDungeon,
}

/// Crowd-control and Mafuba (`MA_PHONG_BA`) jar DoT state mirroring `MobEffectSkill.java`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobEffectState {
    /// Timestamp (ms) until which stun/blind/hypnosis/bind disables mob attacks.
    pub disabled_until_ms: u64,
    /// Timestamp (ms) until which chocolate polymorph lasts.
    pub chocolate_until_ms: u64,
    /// Active Mafuba containment jar `(caster_hp_max, skill_point, jar_multiplier, end_ms, last_tick_ms)`.
    pub mafuba_jar: Option<(i64, u8, u8, u64, u64)>,
}

impl MobEffectState {
    /// Returns `true` if stun, blind, hypnosis, or bind prevents the mob from attacking.
    #[must_use]
    pub const fn is_incapacitated(&self, now_ms: u64) -> bool {
        now_ms < self.disabled_until_ms
    }

    /// Computes non-lethal Mafuba jar DoT (`500ms` cadence) if active.
    pub fn tick_mafuba(&mut self, current_hp: i64, now_ms: u64) -> i64 {
        let Some((caster_hp_max, point, mult, end_ms, ref mut last_tick)) = self.mafuba_jar else {
            return 0;
        };
        if now_ms >= end_ms || current_hp <= 1 {
            self.mafuba_jar = None;
            return 0;
        }
        if now_ms.saturating_sub(*last_tick) >= 500 {
            *last_tick = now_ms;
            let raw = caster_hp_max.saturating_mul(i64::from(point)).saturating_mul(i64::from(mult)) / 100;
            return raw.min(current_hp - 1).max(0);
        }
        0
    }
}

/// Player-summoned egg familiar (`Skill.DE_TRUNG`, `MobMe.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobMeState {
    /// Summoning master player ID.
    pub owner: PlayerId,
    /// Current HP (`injured` caps incoming hits at `max_hp / 20`).
    pub hp: i64,
    /// Maximum HP.
    pub max_hp: i64,
    /// Base attack damage.
    pub dame: i64,
    /// Expiration timestamp (ignored when `pikkoro_set_5pc` is `true`).
    pub expire_at_ms: u64,
    /// Whether the owner wears all 5 pieces of the Pikkoro Daimao SKH set.
    pub pikkoro_set_5pc: bool,
}

impl MobMeState {
    /// Applies incoming hit capped at `max_hp / 20` (`5%`), returning `(damage_taken, died)`.
    pub fn injured(&mut self, raw_damage: i64) -> (i64, bool) {
        if self.hp <= 0 || raw_damage <= 0 {
            return (0, self.hp <= 0);
        }
        let dmg = raw_damage.min((self.max_hp / 20).max(1)).min(self.hp);
        self.hp -= dmg;
        (dmg, self.hp <= 0)
    }

    /// Returns `true` if the familiar expired on `now_ms`.
    #[must_use]
    pub const fn is_expired(&self, now_ms: u64) -> bool {
        !self.pikkoro_set_5pc && now_ms >= self.expire_at_ms
    }
}

/// Regular map monster entity mirroring `nro.models.mob.Mob` and `MobPoint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobState {
    /// Zone-local slot identifier (`13` and `14` represent Bulon bodyguards in Doanh Trại).
    pub slot_id: u8,
    /// Mob template ID (`0 = MOC_NHAN`, `22 = BULON`, `76 = CO_MAY_HUY_DIET`, `118 = CADIC_M`).
    pub temp_id: TemplateId,
    /// Map ID hosting this mob.
    pub map_id: MapId,
    /// Map respawn ruleset.
    pub map_kind: MobMapKind,
    /// Monster level (`Mob.level`).
    pub level: u8,
    /// Super-monster flag (`lvMob > 0`).
    pub lv_mob: u8,
    /// Current HP (`<= 0` means dead).
    pub hp: i64,
    /// Maximum HP (`MobPoint.maxHp`).
    pub max_hp: i64,
    /// Explicit base damage (`0` falls back to `p_dame` % of `max_hp`).
    pub dame: i64,
    /// Percentage damage scalar (`Mob.pDame`).
    pub p_dame: u8,
    /// Spawn coordinates.
    pub location: Location,
    /// Crowd-control and Mafuba jar state.
    pub effects: MobEffectState,
    /// Recent attackers tracked for `300px` `1,000ms` retaliation aggro.
    pub aggro_targets: SmallVec<[PlayerId; 4]>,
    /// Timestamp (ms) of death (`lastTimeDie`).
    pub died_at_ms: u64,
    /// Timestamp (ms) of last `30,000ms` `10%` regeneration (`lastTimePhucHoi`).
    pub last_regen_ms: u64,
    /// Timestamp (ms) of last attack (`1,500ms` for `CADIC_M = 118`, else `2,000ms`).
    pub last_attack_ms: u64,
}

impl MobState {
    /// Computes randomized outgoing mob hit matching `MobPoint.getDameAttack()`.
    pub fn roll_attack_damage(&self, rng: &mut GameRng) -> i64 {
        if self.dame > 0 {
            let j = i32::try_from(self.dame / 100).unwrap_or(i32::MAX);
            self.dame.saturating_add(i64::from(rng.next_int(-j, j))).max(1)
        } else {
            let p = i64::from(rng.next_int(i32::from(self.p_dame.saturating_sub(1)), i32::from(self.p_dame.saturating_add(1))));
            let lv_j = i32::from(self.level) * 10;
            (self.max_hp.saturating_mul(p) / 100 + i64::from(rng.next_int(-lv_j, lv_j))).max(1)
        }
    }

    /// Processes incoming player hit (`Mob.injured`) including 1-shot protection, Destron Gas
    /// `CO_MAY_HUY_DIET` shield, `SieuQuai` cap, TNSM grant, and Map Cadic Kilis proc.
    #[allow(clippy::too_many_arguments)]
    pub fn injured(
        &mut self,
        attacker: &PlayerView,
        skill: SkillKind,
        raw_damage: i64,
        die_when_hp_full: bool,
        destron_machine_alive: bool,
        has_oai_hung_charm: bool,
        has_kilis_buff: bool,
        parity: ParityMode,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) -> (i64, bool, bool) {
        if self.hp <= 0 || raw_damage <= 0 {
            return (0, false, false);
        }
        let mut dmg = raw_damage.min(self.hp);
        if !die_when_hp_full && self.map_id.0 != 164 {
            if self.hp == self.max_hp && dmg >= self.hp {
                dmg = (self.hp - 1).max(1);
            }
            if matches!(self.temp_id.0, 0 | 103) {
                dmg = dmg.min((self.max_hp / 10).max(1));
            }
        }
        if destron_machine_alive && (skill.is_ki_blast() || skill == SkillKind::LienHoan) {
            dmg = 1;
        }
        if !die_when_hp_full && self.lv_mob > 0 && self.map_kind == MobMapKind::NormalWorld && !has_oai_hung_charm {
            dmg = if parity == ParityMode::Legacy && self.max_hp > 20_000_000 {
                0
            } else {
                dmg.min((self.max_hp / 10).max(1))
            };
        }
        dmg = dmg.min(2_147_483_647).min(self.hp);
        self.hp -= dmg;
        if !self.aggro_targets.contains(&attacker.id) && self.aggro_targets.len() < 4 {
            self.aggro_targets.push(attacker.id);
        }
        let died = self.hp <= 0;
        if died {
            self.died_at_ms = now_ms;
            self.last_regen_ms = now_ms;
            self.aggro_targets.clear();
        }
        let tnsm = if self.temp_id.0 == 0 { 1 } else { (dmg + (self.max_hp / 2_000)).max(1) };
        effects.push(EngineEffect::GrantTnsm { target: attacker.id, amount: tnsm });
        let kilis = self.map_id.0 >= 165 && rng.is_true(if has_kilis_buff { 10 } else { 1 }, 333);
        (dmg, died, kilis)
    }

    /// Executes respawn (`3s` world / `10s` Bulon), `30s` regen, Mafuba DoT, and aggro attacks.
    pub fn tick(
        &mut self,
        players: &[PlayerView],
        golden_frieza_21h: bool,
        is_tut_alive: bool,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) {
        if golden_frieza_21h {
            if self.hp > 0 {
                self.hp = 0;
                self.died_at_ms = now_ms;
                self.aggro_targets.clear();
            }
            return;
        }
        if self.hp <= 0 {
            let respawn = match self.map_kind {
                MobMapKind::NormalWorld => now_ms.saturating_sub(self.died_at_ms) >= 3_000,
                MobMapKind::RedRibbonDungeon => {
                    self.temp_id.0 == 22 && is_tut_alive && now_ms.saturating_sub(self.died_at_ms) >= 10_000
                }
                MobMapKind::NoRespawnDungeon => false,
            };
            if respawn {
                self.hp = self.max_hp;
                self.last_regen_ms = now_ms;
            }
            return;
        }
        self.aggro_targets.retain(|id| players.iter().any(|p| p.id == *id && p.stats.hp > 0));
        self.hp -= self.effects.tick_mafuba(self.hp, now_ms);
        if self.map_kind == MobMapKind::NormalWorld && now_ms.saturating_sub(self.last_regen_ms) >= 30_000 {
            self.last_regen_ms = now_ms;
            self.hp = (self.hp + (self.max_hp / 10).max(1)).min(self.max_hp);
        }
        if self.effects.is_incapacitated(now_ms) || matches!(self.temp_id.0, 0 | 76 | 103 | 117) {
            return;
        }
        let cd = if self.temp_id.0 == 118 { 1_500 } else { 2_000 };
        if now_ms.saturating_sub(self.last_attack_ms) < cd {
            return;
        }
        if let Some(target) = players.iter().find(|p| {
            p.stats.hp > 0
                && ((self.aggro_targets.contains(&p.id) && loc_distance(self.location, p.location) <= 300)
                    || (self.temp_id.0 > 18 && loc_distance(self.location, p.location) <= 100))
        }) {
            self.last_attack_ms = now_ms;
            effects.push(EngineEffect::DamagePlayer {
                target: target.id,
                damage: self.roll_attack_damage(rng),
                piercing: false,
            });
        }
    }
}
