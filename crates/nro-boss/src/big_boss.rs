//! Multi-phase AoE `BigBoss` engine (`models/mob_bigboss/` — all 9 classes) + `Mob` re-exports.
//!
//! Implements `Hirudegarn`, `GauTuongCuop`, `VuaBachTuoc`, `RobotBaoVe`, `MayDoSucManh`,
//! `GaChinCua`, `NguaChinLmao`, `VoiChinNga`, and `Piano`, alongside re-exported [`MobState`].

pub use crate::mob::{MobEffectState, MobMapKind, MobMeState, MobState};

use crate::{
    ClanId, EngineEffect, GameRng, GroundDrop, ItemInstance, ItemOption, Location, ParityMode,
    PlayerId, PlayerView, TemplateId, loc_distance,
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::BTreeMap;
use strum::{Display, EnumIter, EnumString};

/// All 9 multi-phase / AoE `BigBoss` classes in `models/mob_bigboss/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumIter, EnumString, Serialize, Deserialize)]
pub enum BigBossKind {
    /// `Hirudegarn` (`temp_id = 70`): 3 evolutions, `1%` current-HP cap, `30x32k` gold shower.
    Hirudegarn,
    /// `GauTuongCuop` (`temp_id = 77`): `5%` DR, `300ms` AoE, `50` Clan Capsule contribution split.
    GauTuongCuop,
    /// `VuaBachTuoc` (`temp_id = 71`): `440 < x < 950` corridor gate, actions `3/4/5`.
    VuaBachTuoc,
    /// `RobotBaoVe` (`temp_id = 72`): `1,000ms` distance-tiered x-teleport strike.
    RobotBaoVe,
    /// `MayDoSucManh` (`temp_id = 117`): `1,000ms` full-HP reset, `+10` pts per `10M` damage.
    MayDoSucManh,
    /// `GaChinCua` (`temp_id = 83`): Hùng Vương Nine-Spur Rooster (`<50` / `<100` / jump).
    GaChinCua,
    /// `NguaChinLmao` (`temp_id = 84`): Hùng Vương Nine-Mane Horse (`<50` / `<100` / jump).
    NguaChinLmao,
    /// `VoiChinNga` (`temp_id = 82`): Hùng Vương Nine-Tusk Elephant (`<50` / `<100` / jump).
    VoiChinNga,
    /// `Piano` (`temp_id = 85`): `<50`/`<100` single strike + `<150` multi-player AoE (`13, 14`).
    Piano,
}

/// Stateful multi-phase AoE encounter covering all 9 `models/mob_bigboss/` classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BigBossState {
    /// Specific big-boss archetype.
    pub kind: BigBossKind,
    /// Current HP.
    pub hp: i64,
    /// Maximum HP.
    pub max_hp: i64,
    /// Base outgoing damage (`GauTuongCuop` overrides to `19,999,999`).
    pub base_dame: i64,
    /// Evolution stage (`0..=3` for `Hirudegarn`).
    pub phase_lv: u8,
    /// Last wire action code (`0..=23`).
    pub last_action: u8,
    /// Current spatial coordinates.
    pub location: Location,
    /// Timestamp (ms) of last attack.
    pub last_attack_ms: u64,
    /// Timestamp (ms) when `hp` reached `0`.
    pub died_at_ms: u64,
    /// Timestamp (ms) of last `1,000ms` full-HP reset (`MayDoSucManh`).
    pub last_full_heal_ms: u64,
    /// Per-player cumulative damage ledger (`GauTuongCuop` & `MayDoSucManh`).
    pub damage_ledger: BTreeMap<i64, (Option<ClanId>, u64)>,
    /// Per-player awarded `10M` milestones (`MayDoSucManh`).
    pub milestones: BTreeMap<i64, u32>,
}

impl BigBossState {
    /// Creates a new `BigBossState` at full HP.
    #[must_use]
    pub fn new(kind: BigBossKind, max_hp: i64, base_dame: i64, location: Location) -> Self {
        Self {
            kind,
            hp: max_hp,
            max_hp,
            base_dame,
            phase_lv: 0,
            last_action: 0,
            location,
            last_attack_ms: 0,
            died_at_ms: 0,
            last_full_heal_ms: 0,
            damage_ledger: BTreeMap::new(),
            milestones: BTreeMap::new(),
        }
    }

    /// Processes an incoming hit and emits milestone or death-split rewards.
    pub fn injured(
        &mut self,
        attacker: &PlayerView,
        raw_damage: i64,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) -> i64 {
        if self.hp <= 0 || raw_damage <= 0 {
            return 0;
        }
        let dmg = match self.kind {
            BigBossKind::Hirudegarn => (self.hp / 100).max(1),
            BigBossKind::GauTuongCuop => ((raw_damage / 20).max(1)).min(self.hp),
            _ => raw_damage.min(self.hp),
        };
        self.hp -= dmg;
        match self.kind {
            BigBossKind::GauTuongCuop => {
                let entry = self.damage_ledger.entry(attacker.id.0).or_insert((attacker.clan_id, 0));
                entry.0 = attacker.clan_id;
                entry.1 = entry.1.saturating_add(dmg as u64);
                if self.hp == 0 {
                    self.died_at_ms = now_ms;
                    self.distribute_gau_capsules(effects);
                    self.damage_ledger.clear();
                }
            }
            BigBossKind::MayDoSucManh => {
                let entry = self.damage_ledger.entry(attacker.id.0).or_insert((attacker.clan_id, 0));
                entry.1 = entry.1.saturating_add(dmg as u64);
                let cur_ms = (entry.1 / 10_000_000) as u32;
                let prev_ms = self.milestones.get(&attacker.id.0).copied().unwrap_or(0);
                if cur_ms > prev_ms {
                    self.milestones.insert(attacker.id.0, cur_ms);
                    effects.push(EngineEffect::AwardMayDamPoints { target: attacker.id, points: (cur_ms - prev_ms) * 10 });
                }
                if self.hp == 0 {
                    self.died_at_ms = now_ms;
                    self.damage_ledger.clear();
                    self.milestones.clear();
                }
            }
            _ if self.hp == 0 => self.died_at_ms = now_ms,
            _ => {}
        }
        dmg
    }

    /// Advances phase transitions (`Hirudegarn`), `1s` heal (`MayDoSucManh`), and AoE attacks.
    pub fn tick(
        &mut self,
        players: &[PlayerView],
        golden_frieza_21h: bool,
        parity: ParityMode,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) {
        if golden_frieza_21h {
            self.hp = 0;
            return;
        }
        if self.kind == BigBossKind::MayDoSucManh {
            if self.hp > 0 && now_ms.saturating_sub(self.last_full_heal_ms) >= 1_000 {
                self.last_full_heal_ms = now_ms;
                self.hp = self.max_hp;
            }
            return;
        }
        if self.hp <= 0 {
            if self.kind == BigBossKind::Hirudegarn {
                self.tick_hirudegarn_dead(parity, rng, now_ms, effects);
            }
            return;
        }
        if players.is_empty() {
            return;
        }
        let cd = match self.kind {
            BigBossKind::GauTuongCuop => 300,
            BigBossKind::RobotBaoVe | BigBossKind::VuaBachTuoc => 1_000,
            _ => 3_000,
        };
        if now_ms.saturating_sub(self.last_attack_ms) < cd {
            return;
        }
        self.last_attack_ms = now_ms;
        self.execute_attack(players, rng, effects);
    }

    fn tick_hirudegarn_dead(
        &mut self,
        parity: ParityMode,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) {
        let elapsed = now_ms.saturating_sub(self.died_at_ms);
        if self.phase_lv <= 2 && elapsed > 5_000 {
            for i in 0..30_i16 {
                let offset = if i % 2 == 0 { -5 * ((i / 2) % 11) } else { 5 * (((i / 2) % 10) + 1) };
                effects.push(EngineEffect::DropItem(GroundDrop {
                    item: ItemInstance::new(TemplateId(190), 32_000),
                    location: Location::new(self.location.x + offset, self.location.y),
                    owner: None,
                }));
            }
            if rng.is_true(20, 100) {
                effects.push(EngineEffect::DropItem(GroundDrop { item: ItemInstance::new(TemplateId(568), 1), location: self.location, owner: None }));
            }
            if rng.is_true(20, 100) {
                let mut kilis = ItemInstance::new(TemplateId(1795), 1);
                kilis.options.push(ItemOption::new(250, 0));
                effects.push(EngineEffect::DropItem(GroundDrop { item: kilis, location: self.location, owner: None }));
            }
            if rng.is_true(1, 40) {
                let id = [555_i16, 557, 559][rng.next_int(0, 2) as usize];
                effects.push(EngineEffect::DropItem(GroundDrop { item: ItemInstance::new(TemplateId(id), 1), location: self.location, owner: None }));
            }
            self.phase_lv += 1;
            if self.phase_lv <= 2 {
                self.last_action = if self.phase_lv == 1 { 6 } else { 5 };
                self.hp = self.max_hp;
            } else {
                self.last_action = 9;
                self.location = Location::new(-1_000, -1_000);
            }
        } else if self.phase_lv == 3 {
            let respawn_ms = if parity == ParityMode::Legacy { 15_000 } else { 600_000 };
            if elapsed > respawn_ms {
                self.phase_lv = 0;
                self.last_action = 0;
                self.location = Location::new(rng.next_int(100, 900) as i16, 360);
                self.hp = self.max_hp;
            }
        }
    }

    fn execute_attack(&mut self, players: &[PlayerView], rng: &mut GameRng, effects: &mut Vec<EngineEffect>) {
        let pick = &players[rng.next_int(0, (players.len() - 1) as i32) as usize];
        match self.kind {
            BigBossKind::Hirudegarn => {
                let pool: &[u8] = if self.phase_lv >= 2 { &[1, 2] } else { &[1, 2, 3, 7] };
                self.last_action = if self.last_action == 7 { 0 } else { pool[rng.next_int(0, (pool.len() - 1) as i32) as usize] };
                match self.last_action {
                    1 => {
                        self.location.x = pick.location.x;
                        effects.push(EngineEffect::DamagePlayer { target: pick.id, damage: self.base_dame, piercing: false });
                    }
                    3 => self.location.x = pick.location.x,
                    0 | 2 | 4 => {
                        for pl in players {
                            effects.push(EngineEffect::DamagePlayer { target: pl.id, damage: self.base_dame, piercing: false });
                        }
                    }
                    _ => {}
                }
            }
            BigBossKind::GauTuongCuop => {
                let act = rng.next_int(11, 15) as u8;
                let radius = match act { 11 => 50, 12 => 100, 13 | 14 => 150, _ => 200 };
                let mut hit = false;
                for pl in players {
                    if loc_distance(self.location, pl.location) < radius {
                        hit = true;
                        effects.push(EngineEffect::DamagePlayer { target: pl.id, damage: 19_999_999, piercing: false });
                    }
                }
                if hit {
                    self.last_action = act;
                } else {
                    self.last_action = 10;
                    self.location = Location::new(pick.location.x + rng.next_int(-10, 10) as i16, pick.location.y);
                }
            }
            BigBossKind::VuaBachTuoc => {
                if let Some(pl) = players.iter().find(|p| p.location.x > 440 && p.location.x < 950) {
                    let d = loc_distance(self.location, pl.location);
                    self.last_action = if d <= 100 { 3 } else if d <= 200 { 4 } else { self.location.x = pl.location.x; 5 };
                    effects.push(EngineEffect::DamagePlayer { target: pl.id, damage: self.base_dame, piercing: false });
                }
            }
            BigBossKind::RobotBaoVe => {
                let pl = &players[0];
                let d = loc_distance(self.location, pl.location);
                self.last_action = if d <= 100 { 0 } else if d <= 200 { 1 } else { 2 };
                self.location.x = pl.location.x;
                effects.push(EngineEffect::DamagePlayer { target: pl.id, damage: self.base_dame, piercing: false });
            }
            BigBossKind::Piano | BigBossKind::GaChinCua | BigBossKind::NguaChinLmao | BigBossKind::VoiChinNga => {
                let act = rng.next_int(11, if self.kind == BigBossKind::Piano { 14 } else { 12 }) as u8;
                let radius = match act { 11 => 50, 12 => 100, _ => 150 };
                let mut hits = 0_usize;
                for pl in players {
                    if loc_distance(self.location, pl.location) < radius {
                        hits += 1;
                        effects.push(EngineEffect::DamagePlayer { target: pl.id, damage: self.base_dame, piercing: false });
                        if act < 13 {
                            break;
                        }
                    }
                }
                if hits > 0 {
                    self.last_action = act;
                } else {
                    self.last_action = 10;
                    self.location = Location::new(pick.location.x + rng.next_int(-10, 10) as i16, pick.location.y);
                }
            }
            BigBossKind::MayDoSucManh => {}
        }
    }

    fn distribute_gau_capsules(&self, effects: &mut Vec<EngineEffect>) {
        let total_dmg: u64 = self.damage_ledger.values().map(|(_, d)| *d).sum();
        if total_dmg == 0 {
            return;
        }
        let mut grants: SmallVec<[(PlayerId, ClanId, u32); 8]> = SmallVec::new();
        let mut distributed = 0_u32;
        let mut top_idx: Option<(usize, u64)> = None;
        for (&pid, &(clan_opt, dmg)) in &self.damage_ledger {
            let Some(clan_id) = clan_opt else { continue };
            let share = (((dmg as u128 * 50) / (total_dmg as u128)) as u32).max(1);
            let idx = grants.len();
            grants.push((PlayerId(pid), clan_id, share));
            distributed += share;
            if top_idx.is_none_or(|(_, best)| dmg > best) {
                top_idx = Some((idx, dmg));
            }
        }
        if distributed < 50 && let Some((idx, _)) = top_idx {
            grants[idx].2 += 50 - distributed;
        }
        if !grants.is_empty() {
            effects.push(EngineEffect::DistributeClanCapsules(grants));
        }
    }
}
