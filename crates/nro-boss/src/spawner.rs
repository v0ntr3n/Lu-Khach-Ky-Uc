//! Unified tick-driven `BossGroupManager` replacing all 16 legacy `Boss_Manager` thread loops.
//!
//! Consolidates `BossManager`, `BrolyManager`, `SkillSummonedManager`, `YardartManager`,
//! `GoldenFriezaManager`, `FinalBossManager`, `MajinBuu14HManager`, `RedRibbonHQManager`,
//! `TreasureUnderSeaManager`, `SnakeWayManager`, `GasDestroyManager`, `OtherBossManager`, and
//! the 5 previously unwired seasonal event managers (`Halloween`, `Christmas`, `HungVuong`,
//! `TrungThu`, `Tet`) into a deterministic single-threaded scheduler.

#[path = "spawner_combat.rs"]
mod spawner_combat;
#[path = "spawner_waves.rs"]
mod spawner_waves;

pub use crate::{ManagerGroup, SeasonalEvent, TimeWindowGate};

use crate::{
    AppearType, BossArchetype, BossError, BossFlags, BossLootProfile, BossStatus, BossTypeId,
    CoreStats, DamageRule, EngineEffect, GameRng, LifecycleState, Location, MapId, ParityMode,
    PlayerId, PlayerView, StageConfig, WaveRole, ZoneId, builtin_boss_catalog,
    combat_ai::run_boss_combat_tick, lifecycle::LifecycleAction,
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
    /// Assigned zone ID (`>= 1`; bosses never spawn in Khu 0).
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
    /// Live zone count per `MapId` (`>= 2` on world maps) for non-zero boss zone selection.
    pub map_zones: BTreeMap<u16, u16>,
}

impl WorldTickContext {
    fn gate_open(&self, gate: TimeWindowGate, group: ManagerGroup, parity: ParityMode) -> bool {
        if parity == ParityMode::Legacy
            && matches!(
                group,
                ManagerGroup::HalloweenEvent
                    | ManagerGroup::ChristmasEvent
                    | ManagerGroup::HungVuongEvent
                    | ManagerGroup::TrungThuAndTetEvent
            )
        {
            return false;
        }
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
    pub(super) parity: ParityMode,
    pub(super) catalog: BTreeMap<BossTypeId, BossDefinition>,
    pub(super) instances: BTreeMap<i64, BossRuntime>,
    pub(super) next_entity_seq: i64,
    pub(super) next_wave_group: u64,
}

impl BossGroupManager {
    /// Minimum allowed spawn zone for a boss (`ZoneId(1)` or `ZoneId(2)` — never `ZoneId(0)`).
    #[must_use]
    pub const fn min_boss_zone(flags: BossFlags) -> ZoneId {
        if flags.contains(BossFlags::ZONE_01_DISABLED) {
            ZoneId(2)
        } else {
            ZoneId(1)
        }
    }

    /// Creates an empty `BossGroupManager` under `parity`.
    #[must_use]
    pub fn new(parity: ParityMode) -> Self {
        Self {
            parity,
            catalog: BTreeMap::new(),
            instances: BTreeMap::new(),
            next_entity_seq: -1_000,
            next_wave_group: 1,
        }
    }

    /// Initializes the manager pre-populated with [`builtin_boss_catalog`] and spawns world waves.
    pub fn with_builtin_catalog(parity: ParityMode, now_ms: u64) -> Result<Self, BossError> {
        let mut mgr = Self::new(parity);
        mgr.register_definitions(builtin_boss_catalog())?;
        mgr.bootstrap_persistent_waves(now_ms)?;
        Ok(mgr)
    }

    /// Loads `BossDefinition` records from a JSON array string (`serde_json`).
    pub fn from_json_str(json: &str, parity: ParityMode, now_ms: u64) -> Result<Self, BossError> {
        let defs: Vec<BossDefinition> =
            serde_json::from_str(json).map_err(|e| BossError::JsonConfig(e.to_string()))?;
        let mut mgr = Self::new(parity);
        mgr.register_definitions(defs)?;
        mgr.bootstrap_persistent_waves(now_ms)?;
        Ok(mgr)
    }

    /// Exports all registered `BossDefinition` records to pretty JSON.
    pub fn to_json_string(&self) -> Result<String, BossError> {
        let vec: Vec<&BossDefinition> = self.catalog.values().collect();
        serde_json::to_string_pretty(&vec).map_err(|e| BossError::JsonConfig(e.to_string()))
    }

    /// Registers a batch of `BossDefinition` records after validating non-empty stages.
    pub fn register_definitions(&mut self, defs: Vec<BossDefinition>) -> Result<(), BossError> {
        for def in defs {
            if def.stages.is_empty() || def.stages.iter().any(|s| s.hp_pool.is_empty()) {
                return Err(BossError::EmptyStages(def.boss_type));
            }
            self.catalog.insert(def.boss_type, def);
        }
        Ok(())
    }

    /// Returns immutable access to all active/resting runtime boss instances.
    #[must_use]
    pub const fn instances(&self) -> &BTreeMap<i64, BossRuntime> {
        &self.instances
    }

    /// Returns the number of registered `BossDefinition` entries in the catalog.
    #[must_use]
    pub fn catalog_len(&self) -> usize {
        self.catalog.len()
    }

    /// Advances all 16 boss groups deterministically for one server tick at `now_ms`.
    #[allow(clippy::too_many_lines)]
    pub fn tick_all(
        &mut self,
        world: &WorldTickContext,
        rng: &mut GameRng,
        now_ms: u64,
    ) -> Vec<EngineEffect> {
        let mut effects = Vec::new();
        let ids: Vec<i64> = self.instances.keys().copied().collect();
        let mut post = spawner_waves::TickPostQueues::default();

        for raw_id in ids {
            let Some((bt, m_id, z_id)) = self
                .instances
                .get(&raw_id)
                .map(|b| (b.boss_type, b.map_id, b.zone_id))
            else {
                continue;
            };
            let Some(def) = self.catalog.get(&bt).cloned() else {
                continue;
            };
            let zkey = (m_id.0.cast_unsigned(), z_id.0);
            let roster = world.zone_rosters.get(&zkey).map_or(&[][..], Vec::as_slice);
            let p_count = world
                .zone_players
                .get(&zkey)
                .copied()
                .unwrap_or(roster.len());
            let gate_ok = world.gate_open(def.time_gate, def.group, self.parity);

            let Some(inst) = self.instances.get_mut(&raw_id) else {
                continue;
            };
            if inst.flags.contains(BossFlags::IMMORTAL_MP) {
                inst.stats.mp = inst.stats.mp_max;
            }
            if spawner_waves::check_tet_lan_con_leave(inst, world, zkey, now_ms) {
                continue;
            }
            match inst.lifecycle.step(
                inst.entity_id,
                &def.stages,
                inst.flags,
                p_count,
                gate_ok,
                self.parity,
                rng,
                now_ms,
                &mut effects,
            ) {
                LifecycleAction::InitStageStats | LifecycleAction::TransformNextStage => {
                    spawner_waves::init_stage_stats(inst, &def, rng, now_ms);
                }
                LifecycleAction::EnterZone { wake_companions } => {
                    spawner_waves::enter_zone(
                        inst,
                        &def,
                        world,
                        rng,
                        wake_companions,
                        &mut post,
                        &mut effects,
                    );
                }
                LifecycleAction::RunCombatTick => {
                    let lifespan = match inst.archetype {
                        BossArchetype::DeathBeamProjectile { lifespan_ms } => Some(lifespan_ms),
                        BossArchetype::DhvtArena {
                            clone_lifespan_ms, ..
                        } => clone_lifespan_ms,
                        _ => None,
                    };
                    if lifespan.is_some_and(|ms| {
                        now_ms.saturating_sub(inst.lifecycle.joined_map_ms) >= u64::from(ms)
                    }) {
                        inst.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                    } else {
                        let eff_start = effects.len();
                        run_boss_combat_tick(
                            inst.entity_id,
                            inst.map_id,
                            inst.zone_id,
                            &mut inst.location,
                            &mut inst.stats,
                            &mut inst.archetype,
                            roster,
                            self.parity,
                            rng,
                            &mut effects,
                        );
                        post.collect_dynamic_spawns(inst.wave_group_id, &effects[eff_start..]);
                    }
                }
                LifecycleAction::ProcessDeath => {
                    spawner_waves::process_death(inst, p_count, rng, now_ms, &mut effects);
                }
                LifecycleAction::ReviveFromAfk => inst.stats.hp = inst.stats.hp_max,
                LifecycleAction::ExitZoneAndRest => {
                    post.record_exit(inst, raw_id, self.parity);
                }
                LifecycleAction::None => {}
            }
        }
        self.apply_post_queues(post, rng, now_ms);
        self.reconcile_all_waves(now_ms, &mut effects);
        effects
    }
}
