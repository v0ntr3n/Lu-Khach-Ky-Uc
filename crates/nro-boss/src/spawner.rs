//! Unified tick-driven `BossGroupManager` replacing all 16 legacy `Boss_Manager` thread loops.
//!
//! Consolidates `BossManager`, `BrolyManager`, `SkillSummonedManager`, `YardartManager`,
//! `GoldenFriezaManager`, `FinalBossManager`, `MajinBuu14HManager`, `RedRibbonHQManager`,
//! `TreasureUnderSeaManager`, `SnakeWayManager`, `GasDestroyManager`, `OtherBossManager`, and
//! the 5 previously unwired seasonal event managers (`Halloween`, `Christmas`, `HungVuong`,
//! `TrungThu`, `Tet`) into a deterministic single-threaded scheduler.

pub use crate::{ManagerGroup, SeasonalEvent, TimeWindowGate};

use crate::{
    AppearType, BossArchetype, BossError, BossFlags, BossLootProfile, BossStatus, BossTypeId,
    CoreStats, DamageOutcome, DamageRule, DungeonKind, DungeonParams, EngineEffect, GameRng,
    GroundDrop, ItemInstance, ItemOption, LifecycleState, Location, MapId, ParityMode, PkMode,
    PlayerId, PlayerView, RewardEngine, SkillKind, StageConfig, TemplateId, WaveRole, ZoneId,
    builtin_boss_catalog, combat_ai::run_boss_combat_tick,
    lifecycle::{LifecycleAction, reconcile_wave_deadlocks},
    mechanics::{evaluate_boss_damage, scale_dungeon_boss},
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::BTreeMap;

/// Serializable data-driven boss definition loadable from JSON or TOML catalogs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossDefinition {
    /// Canonical positive archetype ID (`1..=200`).
    pub boss_type: BossTypeId,
    /// Legacy negative `BossID` constant.
    pub legacy_boss_id: i32,
    /// Owning scheduler subsystem among the 16 manager groups.
    pub group: ManagerGroup,
    /// Time or seasonal spawn window gate.
    pub time_gate: TimeWindowGate,
    /// Behavioral bitflags.
    pub flags: BossFlags,
    /// Primary spawn mode (`DefaultAppear`, `AppearWithAnother`, `CallByAnother`).
    pub appear_type: AppearType,
    /// Wave coordination role.
    pub wave_role: WaveRole,
    /// Incoming damage mitigation rule.
    pub damage_rule: DamageRule,
    /// Archetype combat & reactive behavior.
    pub archetype: BossArchetype,
    /// Death loot table profile.
    pub loot_profile: BossLootProfile,
    /// Candidate map IDs selected on `JoinMap`.
    pub spawn_maps: SmallVec<[MapId; 8]>,
    /// Multi-stage configurations (`len >= 1`).
    pub stages: Vec<StageConfig>,
    /// Co-spawned `AppearWithAnother` companion `BossTypeId`s.
    pub companions: SmallVec<[BossTypeId; 6]>,
    /// Mid-fight `CallByAnother` child `BossTypeId`s (`Android13`, `XenCon`, `DeathBeam`).
    pub summoned_children: SmallVec<[BossTypeId; 8]>,
}

/// Live runtime boss instance managed inside [`BossGroupManager`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossRuntime {
    /// Unique negative runtime entity ID (`PlayerId`).
    pub entity_id: PlayerId,
    /// Shared wave group identifier.
    pub wave_group_id: u64,
    /// Backing static definition ID.
    pub boss_type: BossTypeId,
    /// Assigned map ID.
    pub map_id: MapId,
    /// Assigned zone ID.
    pub zone_id: ZoneId,
    /// Spatial coordinates.
    pub location: Location,
    /// Current and maximum combat stats.
    pub stats: CoreStats,
    /// Whether `KHIEN_NANG_LUONG` energy shield is currently up.
    pub shield_active: bool,
    /// Lifecycle state machine.
    pub lifecycle: LifecycleState,
    /// Mutable archetype runtime state.
    pub archetype: BossArchetype,
    /// Active damage mitigation rule.
    pub damage_rule: DamageRule,
    /// Active loot profile.
    pub loot_profile: BossLootProfile,
    /// Behavioral flags.
    pub flags: BossFlags,
    /// Last attacker who dealt lethal damage (`playerReward`).
    pub last_killer: Option<PlayerView>,
}

/// World clock and zone population snapshot passed into [`BossGroupManager::tick_all`].
#[derive(Debug, Clone, Default)]
pub struct WorldTickContext {
    /// Current `Asia/Ho_Chi_Minh` hour (`0..=23`).
    pub ict_hour: u8,
    /// Whether 12:00 Majin Buu spaceship raid is open.
    pub mabu_12h_open: bool,
    /// Whether 14:00 Majin Buu raid is open.
    pub mabu_14h_open: bool,
    /// Currently active seasonal festivals.
    pub active_seasons: SmallVec<[SeasonalEvent; 5]>,
    /// Player count per `(MapId, ZoneId)`.
    pub zone_players: BTreeMap<(u16, u16), usize>,
    /// Active player views per `(MapId, ZoneId)` for combat target selection.
    pub zone_rosters: BTreeMap<(u16, u16), Vec<PlayerView>>,
}

impl WorldTickContext {
    fn gate_open(&self, gate: TimeWindowGate, group: ManagerGroup, parity: ParityMode) -> bool {
        if parity == ParityMode::Legacy && matches!(group, ManagerGroup::HalloweenEvent | ManagerGroup::ChristmasEvent | ManagerGroup::HungVuongEvent | ManagerGroup::TrungThuAndTetEvent) { return false; }
        match gate {
            TimeWindowGate::Always => true,
            TimeWindowGate::Hour21Ict => self.ict_hour == 21,
            TimeWindowGate::Mabu12hWindow => self.mabu_12h_open,
            TimeWindowGate::Mabu14hWindow => self.mabu_14h_open,
            TimeWindowGate::Seasonal(ev) => self.active_seasons.contains(&ev),
        }
    }
}

/// Single deterministic scheduler managing all 16 boss groups, waves, and dungeon instances.
#[derive(Debug, Clone)]
pub struct BossGroupManager {
    parity: ParityMode,
    catalog: BTreeMap<BossTypeId, BossDefinition>,
    instances: BTreeMap<i64, BossRuntime>,
    next_entity_seq: i64,
    next_wave_group: u64,
}

impl BossGroupManager {
    /// Creates an empty `BossGroupManager` under `parity`.
    #[must_use]
    pub fn new(parity: ParityMode) -> Self { Self { parity, catalog: BTreeMap::new(), instances: BTreeMap::new(), next_entity_seq: -1_000, next_wave_group: 1 } }
    /// Initializes the manager pre-populated with [`builtin_boss_catalog`] and spawns world waves.
    pub fn with_builtin_catalog(parity: ParityMode, now_ms: u64) -> Result<Self, BossError> {
        let mut mgr = Self::new(parity); mgr.register_definitions(builtin_boss_catalog())?; mgr.bootstrap_persistent_waves(now_ms)?; Ok(mgr)
    }
    /// Loads `BossDefinition` records from a JSON array string (`serde_json`).
    pub fn from_json_str(json: &str, parity: ParityMode, now_ms: u64) -> Result<Self, BossError> {
        let defs: Vec<BossDefinition> = serde_json::from_str(json).map_err(|e| BossError::JsonConfig(e.to_string()))?;
        let mut mgr = Self::new(parity); mgr.register_definitions(defs)?; mgr.bootstrap_persistent_waves(now_ms)?; Ok(mgr)
    }
    /// Exports all registered `BossDefinition` records to pretty JSON.
    pub fn to_json_string(&self) -> Result<String, BossError> {
        let vec: Vec<&BossDefinition> = self.catalog.values().collect(); serde_json::to_string_pretty(&vec).map_err(|e| BossError::JsonConfig(e.to_string()))
    }

    /// Registers a batch of `BossDefinition` records after validating non-empty stages.
    pub fn register_definitions(&mut self, defs: Vec<BossDefinition>) -> Result<(), BossError> {
        for def in defs {
            if def.stages.is_empty() || def.stages.iter().any(|s| s.hp_pool.is_empty()) { return Err(BossError::EmptyStages(def.boss_type)); }
            self.catalog.insert(def.boss_type, def);
        }
        Ok(())
    }

    /// Returns immutable access to all active/resting runtime boss instances.
    #[must_use] pub const fn instances(&self) -> &BTreeMap<i64, BossRuntime> { &self.instances }

    /// Returns the number of registered `BossDefinition` entries in the catalog.
    #[must_use] pub fn catalog_len(&self) -> usize { self.catalog.len() }

    /// Bootstraps persistent world, raid, Yardrat, Broly (`30x`), and seasonal waves.
    pub fn bootstrap_persistent_waves(&mut self, now_ms: u64) -> Result<(), BossError> {
        let roots: Vec<BossTypeId> = self.catalog.values().filter(|d| d.appear_type == AppearType::DefaultAppear && !matches!(d.group, ManagerGroup::RedRibbonDungeon | ManagerGroup::TreasureMapDungeon | ManagerGroup::SnakeWayDungeon | ManagerGroup::DestronGasDungeon | ManagerGroup::OtherInstanced)).map(|d| d.boss_type).collect();
        for bt in roots {
            let flags = self.catalog[&bt].flags;
            let zid = if !flags.contains(BossFlags::ZONE_ZERO_ONLY) && flags.contains(BossFlags::ZONE_01_DISABLED) { ZoneId(2) } else { ZoneId(0) };
            for _ in 0..if self.catalog[&bt].group == ManagerGroup::Broly { 30 } else { 1 } { self.spawn_wave(bt, None, zid, now_ms)?; }
        }
        Ok(())
    }

    /// Spawns a parent `BossDefinition` together with its `companions` & `summoned_children`.
    pub fn spawn_wave(&mut self, root: BossTypeId, ovr: Option<(i64, i64)>, zid: ZoneId, now_ms: u64) -> Result<PlayerId, BossError> {
        let def = self.catalog.get(&root).cloned().ok_or(BossError::UnknownBossDefinition(root))?;
        let wid = self.next_wave_group;
        self.next_wave_group += 1;
        let pid = self.alloc_instance(&def, wid, None, ovr, zid, now_ms);
        for &comp in def.companions.iter().chain(def.summoned_children.iter()) {
            if let Some(cdef) = self.catalog.get(&comp).cloned() { self.alloc_instance(&cdef, wid, Some(pid), None, zid, now_ms); }
        }
        Ok(pid)
    }

    /// Spawns a dynamically scaled clan dungeon boss (`Doanh Trại`, `BĐKB`, `Khí Gas`, `CDRĐ`).
    pub fn spawn_dungeon_boss(&mut self, bt: BossTypeId, dungeon: DungeonKind, params: DungeonParams, zid: ZoneId, now_ms: u64) -> Result<PlayerId, BossError> {
        let pid = self.spawn_wave(bt, Some(scale_dungeon_boss(dungeon, params)), zid, now_ms)?;
        if let Some(inst) = self.instances.get_mut(&pid.0) {
            if let DungeonKind::DestronGas { is_hatchiyack } = dungeon {
                inst.damage_rule = DamageRule::KhiGasScaled { level: params.level, is_hatchiyack };
                inst.loot_profile = BossLootProfile::DestronGas { level: params.level, is_hatchiyack };
            }
            inst.lifecycle.transition_to(BossStatus::JoinMap, now_ms);
        }
        Ok(pid)
    }

    /// Advances all 16 boss groups deterministically for one server tick at `now_ms`.
    #[allow(clippy::too_many_lines)]
    pub fn tick_all(&mut self, world: &WorldTickContext, rng: &mut GameRng, now_ms: u64) -> Vec<EngineEffect> {
        let mut effects = Vec::new();
        let ids: Vec<i64> = self.instances.keys().copied().collect();
        let (mut wake_waves, mut to_dispose) = (SmallVec::<[(u64, MapId, ZoneId); 8]>::new(), SmallVec::<[i64; 8]>::new());
        let (mut super_brolys, mut dyn_spawns, mut clear_clones) = (SmallVec::<[(MapId, ZoneId, Location); 4]>::new(), SmallVec::<[(u64, BossTypeId, MapId, ZoneId, Location, Option<(i64, i64)>); 8]>::new(), SmallVec::<[(MapId, ZoneId); 4]>::new());

        for raw_id in ids {
            let Some((bt, m_id, z_id)) = self.instances.get(&raw_id).map(|b| (b.boss_type, b.map_id, b.zone_id)) else { continue };
            let Some(def) = self.catalog.get(&bt).cloned() else { continue };
            let roster = world.zone_rosters.get(&(m_id.0 as u16, z_id.0)).map_or(&[][..], Vec::as_slice);
            let p_count = world.zone_players.get(&(m_id.0 as u16, z_id.0)).copied().unwrap_or(roster.len());
            let gate_ok = world.gate_open(def.time_gate, def.group, self.parity);

            let inst = self.instances.get_mut(&raw_id).expect("checked");
            if inst.flags.contains(BossFlags::IMMORTAL_MP) { inst.stats.mp = inst.stats.mp_max; }
            if inst.lifecycle.status == BossStatus::Afk && let BossArchetype::TetLanCon { tamed_owner: Some(owner) } = inst.archetype {
                let expired = inst.lifecycle.afk_revive_at_ms.is_some_and(|t| now_ms >= t);
                let owner_gone = world.zone_rosters.get(&(m_id.0 as u16, z_id.0)).is_some_and(|r| !r.iter().any(|p| p.id == owner));
                if expired || owner_gone { inst.lifecycle.afk_revive_at_ms = None; inst.lifecycle.transition_to(BossStatus::LeaveMap, now_ms); continue; }
            }
            match inst.lifecycle.step(inst.entity_id, &def.stages, inst.flags, p_count, gate_ok, self.parity, rng, now_ms, &mut effects) {
                LifecycleAction::InitStageStats | LifecycleAction::TransformNextStage => {
                    if inst.lifecycle.current_stage == 0 { inst.archetype = def.archetype.clone(); inst.shield_active = false; inst.last_killer = None; }
                    let stg = &def.stages[inst.lifecycle.current_stage];
                    let (hp, dame) = if matches!(inst.archetype, BossArchetype::Broly { is_super: false }) {
                        let h = i64::from(rng.next_int(500, 100_000)); (h, (h / 100).max(10))
                    } else {
                        let h = if stg.hp_pool.len() > 1 { stg.hp_pool[rng.next_int(0, (stg.hp_pool.len() - 1) as i32) as usize] } else { stg.hp_pool.first().copied().unwrap_or(1) };
                        (h, stg.base_dame)
                    };
                    inst.stats.hp_max = hp; inst.stats.hp = hp; inst.stats.dame = dame;
                    if inst.lifecycle.current_stage > 0 { inst.lifecycle.transition_to(BossStatus::ChatStart, now_ms); }
                }
                LifecycleAction::EnterZone { wake_companions } => {
                    if inst.lifecycle.parent_id.is_none() && !def.spawn_maps.is_empty() && inst.lifecycle.current_stage == 0 {
                        inst.map_id = def.spawn_maps[rng.next_int(0, (def.spawn_maps.len() - 1) as i32) as usize];
                    }
                    inst.location.map_id = inst.map_id; inst.location.zone_id = inst.zone_id;
                    if matches!(inst.archetype, BossArchetype::GoldenFrieza21h) { effects.push(EngineEffect::WipeZoneMobs { map_id: inst.map_id, zone_id: inst.zone_id }); }
                    if wake_companions { wake_waves.push((inst.wave_group_id, inst.map_id, inst.zone_id)); }
                }
                LifecycleAction::RunCombatTick => {
                    let lifespan = match inst.archetype { BossArchetype::DeathBeamProjectile { lifespan_ms } => Some(lifespan_ms), BossArchetype::DhvtArena { clone_lifespan_ms, .. } => clone_lifespan_ms, _ => None };
                    if lifespan.is_some_and(|ms| now_ms.saturating_sub(inst.lifecycle.joined_map_ms) >= u64::from(ms)) {
                        inst.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                    } else {
                        let eff_start = effects.len();
                        run_boss_combat_tick(inst.entity_id, inst.map_id, inst.zone_id, &mut inst.location, &mut inst.stats, &mut inst.archetype, roster, self.parity, rng, &mut effects);
                        for eff in &effects[eff_start..] {
                            if let &EngineEffect::SpawnDynamicBoss { boss_type, map_id, zone_id, location, scaled_stats } = eff {
                                dyn_spawns.push((inst.wave_group_id, boss_type, map_id, zone_id, location, scaled_stats));
                            }
                        }
                    }
                }
                LifecycleAction::ProcessDeath => {
                    inst.location.map_id = inst.map_id; inst.location.zone_id = inst.zone_id;
                    if let Some(ref killer) = inst.last_killer.clone() {
                        let stolen = if let BossArchetype::ThiefAnTrom { stolen_gold } = inst.archetype { stolen_gold } else { 0 };
                        RewardEngine::roll_boss_drops(inst.loot_profile, inst.location, killer, p_count, stolen, rng, &mut effects);
                    }
                    if matches!(inst.archetype, BossArchetype::MajinBuu12h { revive_afk_60s: true, .. }) { inst.lifecycle.afk_revive_at_ms = Some(now_ms.saturating_add(60_000)); inst.lifecycle.transition_to(BossStatus::Afk, now_ms); }
                }
                LifecycleAction::ReviveFromAfk => inst.stats.hp = inst.stats.hp_max,
                LifecycleAction::ExitZoneAndRest => {
                    if matches!(inst.archetype, BossArchetype::Broly { is_super: false }) && (self.parity == ParityMode::Legacy || inst.stats.hp_max >= 1_000_000) {
                        super_brolys.push((inst.map_id, inst.zone_id, inst.location));
                    }
                    if inst.boss_type == BossTypeId(95) { clear_clones.push((inst.map_id, inst.zone_id)); }
                    if inst.flags.contains(BossFlags::DISPOSE_ON_LEAVE) { to_dispose.push(raw_id); }
                }
                LifecycleAction::None => {}
            }
        }
        for (wid, map_id, zone_id) in wake_waves {
            for inst in self.instances.values_mut().filter(|b| b.wave_group_id == wid && b.lifecycle.appear_type == AppearType::AppearWithAnother && b.lifecycle.status == BossStatus::Rest) {
                inst.map_id = map_id; inst.zone_id = zone_id; inst.location.map_id = map_id; inst.location.zone_id = zone_id;
                inst.lifecycle.transition_to(BossStatus::Respawn, now_ms);
            }
        }
        for id in to_dispose { self.instances.remove(&id); }
        for (map_id, zone_id) in clear_clones {
            for c in self.instances.values_mut().filter(|b| b.boss_type == BossTypeId(96) && b.map_id == map_id && b.zone_id == zone_id && !matches!(b.lifecycle.status, BossStatus::Rest | BossStatus::LeaveMap)) { c.lifecycle.transition_to(BossStatus::LeaveMap, now_ms); }
        }
        for (map_id, zone_id, mut loc) in super_brolys {
            if let Ok(pid) = self.spawn_wave(BossTypeId(2), None, zone_id, now_ms) && let Some(sb) = self.instances.get_mut(&pid.0) {
                let hp = i64::from(rng.next_int(1_500_000, 16_070_777));
                loc.map_id = map_id; loc.zone_id = zone_id;
                sb.map_id = map_id; sb.zone_id = zone_id; sb.location = loc; sb.stats.hp_max = hp; sb.stats.hp = hp; sb.stats.dame = (hp / 10).min(2_000_000);
                sb.lifecycle.transition_to(BossStatus::ChatStart, now_ms);
            }
        }
        for (wid, bt, map_id, zone_id, mut loc, scaled) in dyn_spawns {
            loc.map_id = map_id; loc.zone_id = zone_id;
            if (46..=50).contains(&bt.0) {
                for c in self.instances.values_mut().filter(|b| b.wave_group_id == wid && b.boss_type == bt && b.lifecycle.appear_type == AppearType::CallByAnother && b.lifecycle.status == BossStatus::Rest) {
                    c.map_id = map_id; c.zone_id = zone_id; c.location = loc; c.lifecycle.transition_to(BossStatus::Respawn, now_ms);
                }
            } else if bt == BossTypeId(96) && self.instances.values().filter(|b| b.boss_type == bt && b.map_id == map_id && b.zone_id == zone_id).count() < 4
                && let Ok(pid) = self.spawn_wave(bt, scaled, zone_id, now_ms) && let Some(c) = self.instances.get_mut(&pid.0)
            {
                c.map_id = map_id; c.zone_id = zone_id; c.location = loc; c.flags |= BossFlags::DISPOSE_ON_LEAVE;
                c.lifecycle.transition_to(BossStatus::ChatStart, now_ms);
            }
        }
        self.reconcile_all_waves(now_ms, &mut effects);
        effects
    }

    /// Applies an attacker's hit to `target_boss`, handling `SuperBu` forwarding & wave summons.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_hit(
        &mut self, target_boss: PlayerId, attacker: &PlayerView, skill: SkillKind, raw_damage: i64,
        piercing: bool, bulons_alive: bool, rng: &mut GameRng, now_ms: u64, effects: &mut Vec<EngineEffect>,
    ) -> Result<DamageOutcome, BossError> {
        let actual_id = if let Some(b) = self.instances.get(&target_boss.0) && matches!(b.archetype, BossArchetype::MajinBuu14h { is_belly_super_bu: true }) {
            self.instances.values().find(|o| o.zone_id == b.zone_id && matches!(o.archetype, BossArchetype::MajinBuu14h { is_belly_super_bu: false })).map_or(target_boss, |o| o.entity_id)
        } else { target_boss };
        let wave_id = self.instances.get(&actual_id.0).map(|b| b.wave_group_id).ok_or(BossError::UnknownBossDefinition(BossTypeId(0)))?;
        let sibling_alive = self.instances.values().any(|o| o.wave_group_id == wave_id && o.entity_id != actual_id && o.stats.hp > 0 && matches!(o.archetype, BossArchetype::AndroidTrio { member: 14 | 15, .. }));
        let inst = self.instances.get_mut(&actual_id.0).expect("verified");
        if inst.lifecycle.status != BossStatus::Active || inst.lifecycle.pk_mode == PkMode::NonPk {
            return Ok(DamageOutcome { applied_damage: 0, healed_hp: 0, killed: false, yardrat_drop_proc: false });
        }
        let prev_called = matches!(inst.archetype, BossArchetype::AndroidTrio { called_13: true, .. } | BossArchetype::SuperCellSwarm { spawned_jrs: true });
        let outcome = evaluate_boss_damage(inst.entity_id, inst.lifecycle.current_stage, &mut inst.stats, &mut inst.shield_active, &inst.damage_rule, &mut inst.archetype, Some(attacker), skill, raw_damage, piercing, bulons_alive, sibling_alive, self.parity, rng, effects);
        let now_called = matches!(inst.archetype, BossArchetype::AndroidTrio { called_13: true, .. } | BossArchetype::SuperCellSwarm { spawned_jrs: true });
        let (caller_map, caller_zone) = (inst.map_id, inst.zone_id);
        let is_android_caller = matches!(inst.archetype, BossArchetype::AndroidTrio { member: 14 | 15, .. });
        inst.location.map_id = caller_map; inst.location.zone_id = caller_zone;
        let loc = inst.location;
        if outcome.yardrat_drop_proc {
            let mut bk = ItemInstance::new(TemplateId(590), 1); bk.options.push(ItemOption::new(31, 1));
            effects.push(EngineEffect::DropItem(GroundDrop { item: bk, location: attacker.location, owner: Some(attacker.id) }));
        }
        if matches!(inst.archetype, BossArchetype::TetLanCon { tamed_owner: Some(_) }) { inst.lifecycle.pk_mode = PkMode::NonPk; inst.lifecycle.afk_revive_at_ms = Some(now_ms.saturating_add(300_000)); inst.lifecycle.transition_to(BossStatus::Afk, now_ms); }
        if outcome.killed {
            let killed_type = inst.boss_type;
            inst.last_killer = Some(attacker.clone());
            inst.lifecycle.transition_to(BossStatus::Die, now_ms);
            for o in self.instances.values_mut() {
                if o.wave_group_id == wave_id && o.entity_id != actual_id && let BossArchetype::RivalFlagDuel { afk_reward_item, .. } = o.archetype {
                    o.stats.hp = 0; o.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                    RewardEngine::roll_boss_drops(BossLootProfile::SeasonalItem { item_id: afk_reward_item, with_costume_opts: true }, loc, attacker, 1, 0, rng, effects);
                } else if killed_type == BossTypeId(95) && o.boss_type == BossTypeId(96) && o.map_id == caller_map && o.zone_id == caller_zone && !matches!(o.lifecycle.status, BossStatus::Rest | BossStatus::LeaveMap) {
                    o.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                }
            }
        }
        if !prev_called && now_called {
            for o in self.instances.values_mut().filter(|b| b.wave_group_id == wave_id) {
                if o.lifecycle.appear_type == AppearType::CallByAnother && o.lifecycle.status == BossStatus::Rest {
                    o.map_id = caller_map; o.zone_id = caller_zone; o.location.map_id = caller_map; o.location.zone_id = caller_zone;
                    o.lifecycle.transition_to(BossStatus::Respawn, now_ms);
                } else if is_android_caller && let BossArchetype::AndroidTrio { member: 14 | 15, called_13 } = &mut o.archetype {
                    *called_13 = true; o.stats.hp = o.stats.hp_max;
                }
            }
        }
        Ok(outcome)
    }

    fn alloc_instance(&mut self, def: &BossDefinition, wid: u64, parent: Option<PlayerId>, ovr: Option<(i64, i64)>, zid: ZoneId, now_ms: u64) -> PlayerId {
        let eid = PlayerId(self.next_entity_seq);
        self.next_entity_seq -= 1;
        let (dame, hp) = ovr.unwrap_or((def.stages[0].base_dame, def.stages[0].hp_pool.first().copied().unwrap_or(1)));
        let mut lc = LifecycleState::new(def.appear_type, def.wave_role.clone(), now_ms);
        lc.parent_id = parent;
        let map_id = def.spawn_maps.first().copied().unwrap_or(MapId(0));
        let mut loc = if let BossArchetype::YardratPatrol { point_a, .. } = def.archetype { point_a } else { Location::new(300, 360) };
        loc.map_id = map_id; loc.zone_id = zid;
        self.instances.insert(eid.0, BossRuntime {
            entity_id: eid, wave_group_id: wid, boss_type: def.boss_type, map_id,
            zone_id: zid, location: loc, stats: CoreStats { hp, hp_max: hp, mp: 1_000_000, mp_max: 1_000_000, dame, def: 0, crit: 10, dodge: 10, ..CoreStats::default() },
            shield_active: false, lifecycle: lc, archetype: def.archetype.clone(), damage_rule: def.damage_rule.clone(), loot_profile: def.loot_profile,
            flags: def.flags, last_killer: None,
        });
        eid
    }

    fn reconcile_all_waves(&mut self, now_ms: u64, effects: &mut Vec<EngineEffect>) {
        let mut wids: SmallVec<[u64; 16]> = self.instances.values().map(|b| b.wave_group_id).collect();
        wids.sort_unstable(); wids.dedup();
        for wid in wids {
            let keys: SmallVec<[i64; 12]> = self.instances.iter().filter(|(_, b)| b.wave_group_id == wid).map(|(&k, _)| k).collect();
            if keys.len() <= 1 { continue; }
            let mut snap: SmallVec<[(PlayerId, BossTypeId, LifecycleState, bool); 12]> = keys.iter().map(|k| { let b = &self.instances[k]; (b.entity_id, b.boss_type, b.lifecycle.clone(), b.stats.hp > 0 && b.lifecycle.status != BossStatus::Rest) }).collect();
            {
                let mut refs: SmallVec<[(PlayerId, BossTypeId, &mut LifecycleState, bool); 12]> = snap.iter_mut().map(|(id, bt, lc, a)| (*id, *bt, lc, *a)).collect();
                reconcile_wave_deadlocks(&mut refs, now_ms, effects);
            }
            for (k, (_, _, lc, _)) in keys.into_iter().zip(snap) { if let Some(inst) = self.instances.get_mut(&k) { inst.lifecycle = lc; } }
        }
    }
}
