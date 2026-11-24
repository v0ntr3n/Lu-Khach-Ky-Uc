//! Wave bootstrapping, non-zero zone selection, and tick post-queues for [`BossGroupManager`].

use super::{BossDefinition, BossGroupManager, BossRuntime, ManagerGroup, WorldTickContext};
use crate::{
    AppearType, BossArchetype, BossError, BossFlags, BossLootProfile, BossStatus, BossTypeId,
    CoreStats, DamageRule, DungeonKind, DungeonParams, EngineEffect, GameRng, LifecycleState,
    Location, MapId, ParityMode, PlayerId, RewardEngine, ZoneId, mechanics::scale_dungeon_boss,
};
use smallvec::SmallVec;

#[derive(Default)]
pub(super) struct TickPostQueues {
    pub(super) wake_waves: SmallVec<[(u64, MapId, ZoneId); 8]>,
    pub(super) to_dispose: SmallVec<[i64; 8]>,
    pub(super) super_brolys: SmallVec<[(MapId, ZoneId, Location); 4]>,
    pub(super) dyn_spawns:
        SmallVec<[(u64, BossTypeId, MapId, ZoneId, Location, Option<(i64, i64)>); 8]>,
    pub(super) clear_clones: SmallVec<[(MapId, ZoneId); 4]>,
}

impl TickPostQueues {
    pub(super) fn collect_dynamic_spawns(&mut self, wid: u64, new_effects: &[EngineEffect]) {
        for eff in new_effects {
            if let &EngineEffect::SpawnDynamicBoss {
                boss_type,
                map_id,
                zone_id,
                location,
                scaled_stats,
            } = eff
            {
                let safe_z = ZoneId(zone_id.0.max(1));
                self.dyn_spawns
                    .push((wid, boss_type, map_id, safe_z, location, scaled_stats));
            }
        }
    }

    pub(super) fn record_exit(&mut self, inst: &BossRuntime, raw_id: i64, parity: ParityMode) {
        if matches!(inst.archetype, BossArchetype::Broly { is_super: false })
            && (parity == ParityMode::Legacy || inst.stats.hp_max >= 1_000_000)
        {
            self.super_brolys
                .push((inst.map_id, ZoneId(inst.zone_id.0.max(1)), inst.location));
        }
        if inst.boss_type == BossTypeId(95) {
            self.clear_clones.push((inst.map_id, inst.zone_id));
        }
        if inst.flags.contains(BossFlags::DISPOSE_ON_LEAVE) {
            self.to_dispose.push(raw_id);
        }
    }
}

pub(super) fn check_tet_lan_con_leave(
    inst: &mut BossRuntime,
    world: &WorldTickContext,
    zkey: (u16, u16),
    now_ms: u64,
) -> bool {
    if inst.lifecycle.status == BossStatus::Afk
        && let BossArchetype::TetLanCon {
            tamed_owner: Some(owner),
        } = inst.archetype
    {
        let expired = inst.lifecycle.afk_revive_at_ms.is_some_and(|t| now_ms >= t);
        let owner_gone = world
            .zone_rosters
            .get(&zkey)
            .is_some_and(|r| !r.iter().any(|p| p.id == owner));
        if expired || owner_gone {
            inst.lifecycle.afk_revive_at_ms = None;
            inst.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
            return true;
        }
    }
    false
}

pub(super) fn process_death(
    inst: &mut BossRuntime,
    p_count: usize,
    rng: &mut GameRng,
    now_ms: u64,
    effects: &mut Vec<EngineEffect>,
) {
    inst.location.map_id = inst.map_id;
    inst.location.zone_id = inst.zone_id;
    if let Some(ref killer) = inst.last_killer.clone() {
        let stolen = if let BossArchetype::ThiefAnTrom { stolen_gold } = inst.archetype {
            stolen_gold
        } else {
            0
        };
        RewardEngine::roll_boss_drops(
            inst.loot_profile,
            inst.location,
            killer,
            p_count,
            stolen,
            rng,
            effects,
        );
    }
    if matches!(
        inst.archetype,
        BossArchetype::MajinBuu12h {
            revive_afk_60s: true,
            ..
        }
    ) {
        inst.lifecycle.afk_revive_at_ms = Some(now_ms.saturating_add(60_000));
        inst.lifecycle.transition_to(BossStatus::Afk, now_ms);
    }
}

pub(super) fn init_stage_stats(
    inst: &mut BossRuntime,
    def: &BossDefinition,
    rng: &mut GameRng,
    now_ms: u64,
) {
    if inst.lifecycle.current_stage == 0 {
        inst.archetype = def.archetype.clone();
        inst.shield_active = false;
        inst.last_killer = None;
    }
    let stg = &def.stages[inst.lifecycle.current_stage];
    let (hp, dame) = if matches!(inst.archetype, BossArchetype::Broly { is_super: false }) {
        let h = i64::from(rng.next_int(500, 100_000));
        (h, (h / 100).max(10))
    } else {
        let h = if stg.hp_pool.len() > 1 {
            stg.hp_pool[rng.next_int(0, (stg.hp_pool.len() - 1) as i32) as usize]
        } else {
            stg.hp_pool.first().copied().unwrap_or(1)
        };
        (h, stg.base_dame)
    };
    inst.stats.hp_max = hp;
    inst.stats.hp = hp;
    inst.stats.dame = dame;
    if inst.lifecycle.current_stage > 0 {
        inst.lifecycle.transition_to(BossStatus::ChatStart, now_ms);
    }
}

pub(super) fn enter_zone(
    inst: &mut BossRuntime,
    def: &BossDefinition,
    world: &WorldTickContext,
    rng: &mut GameRng,
    wake_companions: bool,
    post: &mut TickPostQueues,
    effects: &mut Vec<EngineEffect>,
) {
    let min_z = BossGroupManager::min_boss_zone(inst.flags).0;
    if inst.lifecycle.parent_id.is_none() && inst.lifecycle.current_stage == 0 {
        if !def.spawn_maps.is_empty() {
            inst.map_id =
                def.spawn_maps[rng.next_int(0, (def.spawn_maps.len() - 1) as i32) as usize];
        }
        if let Some(&max_z) = world.map_zones.get(&inst.map_id.0.cast_unsigned()) {
            let hi = max_z.saturating_sub(1).max(1);
            let lo = min_z.min(hi).max(1);
            inst.zone_id = ZoneId(rng.next_int(i32::from(lo), i32::from(hi)) as u16);
        } else {
            inst.zone_id = ZoneId(inst.zone_id.0.max(min_z));
        }
    } else {
        inst.zone_id = ZoneId(inst.zone_id.0.max(1));
    }
    inst.location.map_id = inst.map_id;
    inst.location.zone_id = inst.zone_id;
    if matches!(inst.archetype, BossArchetype::GoldenFrieza21h) {
        effects.push(EngineEffect::WipeZoneMobs {
            map_id: inst.map_id,
            zone_id: inst.zone_id,
        });
    }
    if wake_companions {
        post.wake_waves
            .push((inst.wave_group_id, inst.map_id, inst.zone_id));
    }
}

impl BossGroupManager {
    /// Bootstraps persistent world, raid, Yardrat, Broly (`30x`), and seasonal waves.
    pub fn bootstrap_persistent_waves(&mut self, now_ms: u64) -> Result<(), BossError> {
        let roots: Vec<BossTypeId> = self
            .catalog
            .values()
            .filter(|d| {
                d.appear_type == AppearType::DefaultAppear
                    && !matches!(
                        d.group,
                        ManagerGroup::RedRibbonDungeon
                            | ManagerGroup::TreasureMapDungeon
                            | ManagerGroup::SnakeWayDungeon
                            | ManagerGroup::DestronGasDungeon
                            | ManagerGroup::OtherInstanced
                    )
            })
            .map(|d| d.boss_type)
            .collect();
        for bt in roots {
            let zid = Self::min_boss_zone(self.catalog[&bt].flags);
            let count = if self.catalog[&bt].group == ManagerGroup::Broly {
                30
            } else {
                1
            };
            for _ in 0..count {
                self.spawn_wave(bt, None, zid, now_ms)?;
            }
        }
        Ok(())
    }

    /// Spawns a parent `BossDefinition` together with its `companions` & `summoned_children`.
    pub fn spawn_wave(
        &mut self,
        root: BossTypeId,
        ovr: Option<(i64, i64)>,
        zid: ZoneId,
        now_ms: u64,
    ) -> Result<PlayerId, BossError> {
        let def = self
            .catalog
            .get(&root)
            .cloned()
            .ok_or(BossError::UnknownBossDefinition(root))?;
        let wid = self.next_wave_group;
        self.next_wave_group += 1;
        let safe_zid = ZoneId(zid.0.max(Self::min_boss_zone(def.flags).0));
        let pid = self.alloc_instance(&def, wid, None, ovr, safe_zid, now_ms);
        for &comp in def.companions.iter().chain(def.summoned_children.iter()) {
            if let Some(cdef) = self.catalog.get(&comp).cloned() {
                self.alloc_instance(&cdef, wid, Some(pid), None, safe_zid, now_ms);
            }
        }
        Ok(pid)
    }

    /// Spawns a dynamically scaled clan dungeon boss (`Doanh Trại`, `BĐKB`, `Khí Gas`, `CDRĐ`).
    pub fn spawn_dungeon_boss(
        &mut self,
        bt: BossTypeId,
        dungeon: DungeonKind,
        params: DungeonParams,
        zid: ZoneId,
        now_ms: u64,
    ) -> Result<PlayerId, BossError> {
        let pid = self.spawn_wave(bt, Some(scale_dungeon_boss(dungeon, params)), zid, now_ms)?;
        if let Some(inst) = self.instances.get_mut(&pid.0) {
            if let DungeonKind::DestronGas { is_hatchiyack } = dungeon {
                inst.damage_rule = DamageRule::KhiGasScaled {
                    level: params.level,
                    is_hatchiyack,
                };
                inst.loot_profile = BossLootProfile::DestronGas {
                    level: params.level,
                    is_hatchiyack,
                };
            }
            inst.lifecycle.transition_to(BossStatus::JoinMap, now_ms);
        }
        Ok(pid)
    }

    pub(super) fn alloc_instance(
        &mut self,
        def: &BossDefinition,
        wid: u64,
        parent: Option<PlayerId>,
        ovr: Option<(i64, i64)>,
        zid: ZoneId,
        now_ms: u64,
    ) -> PlayerId {
        let eid = PlayerId(self.next_entity_seq);
        self.next_entity_seq -= 1;
        let safe_zid = ZoneId(zid.0.max(Self::min_boss_zone(def.flags).0));
        let (dame, hp) = ovr.unwrap_or((
            def.stages[0].base_dame,
            def.stages[0].hp_pool.first().copied().unwrap_or(1),
        ));
        let mut lc = LifecycleState::new(def.appear_type, def.wave_role.clone(), now_ms);
        lc.parent_id = parent;
        let map_id = def.spawn_maps.first().copied().unwrap_or(MapId(0));
        let mut loc = if let BossArchetype::YardratPatrol { point_a, .. } = def.archetype {
            point_a
        } else {
            Location::new(300, 360)
        };
        loc.map_id = map_id;
        loc.zone_id = safe_zid;
        self.instances.insert(
            eid.0,
            BossRuntime {
                entity_id: eid,
                wave_group_id: wid,
                boss_type: def.boss_type,
                map_id,
                zone_id: safe_zid,
                location: loc,
                stats: CoreStats {
                    hp,
                    hp_max: hp,
                    mp: 1_000_000,
                    mp_max: 1_000_000,
                    dame,
                    def: 0,
                    crit: 10,
                    dodge: 10,
                    ..CoreStats::default()
                },
                shield_active: false,
                lifecycle: lc,
                archetype: def.archetype.clone(),
                damage_rule: def.damage_rule.clone(),
                loot_profile: def.loot_profile,
                flags: def.flags,
                last_killer: None,
            },
        );
        eid
    }
}
