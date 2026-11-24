//! Post-tick wave wakeups, hit resolution, and deadlock reconciliation for [`BossGroupManager`].

use super::{BossGroupManager, spawner_waves::TickPostQueues};
use crate::{
    AppearType, BossArchetype, BossError, BossFlags, BossLootProfile, BossStatus, BossTypeId,
    DamageOutcome, EngineEffect, GameRng, GroundDrop, ItemInstance, ItemOption, LifecycleState,
    PkMode, PlayerId, PlayerView, RewardEngine, SkillKind, TemplateId, ZoneId,
    lifecycle::reconcile_wave_deadlocks, mechanics::evaluate_boss_damage,
};
use smallvec::SmallVec;

impl BossGroupManager {
    pub(super) fn apply_post_queues(
        &mut self,
        post: TickPostQueues,
        rng: &mut GameRng,
        now_ms: u64,
    ) {
        for (wid, map_id, zone_id) in post.wake_waves {
            let safe_z = ZoneId(zone_id.0.max(1));
            for inst in self.instances.values_mut().filter(|b| {
                b.wave_group_id == wid
                    && b.lifecycle.appear_type == AppearType::AppearWithAnother
                    && b.lifecycle.status == BossStatus::Rest
            }) {
                inst.map_id = map_id;
                inst.zone_id = safe_z;
                inst.location.map_id = map_id;
                inst.location.zone_id = safe_z;
                inst.lifecycle.transition_to(BossStatus::Respawn, now_ms);
            }
        }
        for id in post.to_dispose {
            self.instances.remove(&id);
        }
        for (map_id, zone_id) in post.clear_clones {
            for c in self.instances.values_mut().filter(|b| {
                b.boss_type == BossTypeId(96)
                    && b.map_id == map_id
                    && b.zone_id == zone_id
                    && !matches!(b.lifecycle.status, BossStatus::Rest | BossStatus::LeaveMap)
            }) {
                c.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
            }
        }
        for (map_id, zone_id, mut loc) in post.super_brolys {
            let safe_z = ZoneId(zone_id.0.max(1));
            if let Ok(pid) = self.spawn_wave(BossTypeId(2), None, safe_z, now_ms)
                && let Some(sb) = self.instances.get_mut(&pid.0)
            {
                let hp = i64::from(rng.next_int(1_500_000, 16_070_777));
                loc.map_id = map_id;
                loc.zone_id = safe_z;
                sb.map_id = map_id;
                sb.zone_id = safe_z;
                sb.location = loc;
                sb.stats.hp_max = hp;
                sb.stats.hp = hp;
                sb.stats.dame = (hp / 10).min(2_000_000);
                sb.lifecycle.transition_to(BossStatus::ChatStart, now_ms);
            }
        }
        for (wid, bt, map_id, zone_id, mut loc, scaled) in post.dyn_spawns {
            let safe_z = ZoneId(zone_id.0.max(1));
            loc.map_id = map_id;
            loc.zone_id = safe_z;
            if (46..=50).contains(&bt.0) {
                for c in self.instances.values_mut().filter(|b| {
                    b.wave_group_id == wid
                        && b.boss_type == bt
                        && b.lifecycle.appear_type == AppearType::CallByAnother
                        && b.lifecycle.status == BossStatus::Rest
                }) {
                    c.map_id = map_id;
                    c.zone_id = safe_z;
                    c.location = loc;
                    c.lifecycle.transition_to(BossStatus::Respawn, now_ms);
                }
            } else if bt == BossTypeId(96)
                && self
                    .instances
                    .values()
                    .filter(|b| b.boss_type == bt && b.map_id == map_id && b.zone_id == safe_z)
                    .count()
                    < 4
                && let Ok(pid) = self.spawn_wave(bt, scaled, safe_z, now_ms)
                && let Some(c) = self.instances.get_mut(&pid.0)
            {
                c.map_id = map_id;
                c.zone_id = safe_z;
                c.location = loc;
                c.flags |= BossFlags::DISPOSE_ON_LEAVE;
                c.lifecycle.transition_to(BossStatus::ChatStart, now_ms);
            }
        }
    }

    /// Applies an attacker's hit to `target_boss`, handling `SuperBu` forwarding & wave summons.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_hit(
        &mut self,
        target_boss: PlayerId,
        attacker: &PlayerView,
        skill: SkillKind,
        raw_damage: i64,
        piercing: bool,
        bulons_alive: bool,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) -> Result<DamageOutcome, BossError> {
        let actual_id = if let Some(b) = self.instances.get(&target_boss.0)
            && matches!(
                b.archetype,
                BossArchetype::MajinBuu14h {
                    is_belly_super_bu: true
                }
            ) {
            self.instances
                .values()
                .find(|o| {
                    o.zone_id == b.zone_id
                        && matches!(
                            o.archetype,
                            BossArchetype::MajinBuu14h {
                                is_belly_super_bu: false
                            }
                        )
                })
                .map_or(target_boss, |o| o.entity_id)
        } else {
            target_boss
        };
        let wave_id = self
            .instances
            .get(&actual_id.0)
            .map(|b| b.wave_group_id)
            .ok_or(BossError::UnknownBossDefinition(BossTypeId(0)))?;
        let sibling_alive = self.instances.values().any(|o| {
            o.wave_group_id == wave_id
                && o.entity_id != actual_id
                && o.stats.hp > 0
                && matches!(
                    o.archetype,
                    BossArchetype::AndroidTrio {
                        member: 14 | 15,
                        ..
                    }
                )
        });
        let Some(inst) = self.instances.get_mut(&actual_id.0) else {
            return Err(BossError::UnknownBossDefinition(BossTypeId(0)));
        };
        if inst.lifecycle.status != BossStatus::Active || inst.lifecycle.pk_mode == PkMode::NonPk {
            return Ok(DamageOutcome {
                applied_damage: 0,
                healed_hp: 0,
                killed: false,
                yardrat_drop_proc: false,
            });
        }
        let prev_called = matches!(
            inst.archetype,
            BossArchetype::AndroidTrio {
                called_13: true,
                ..
            } | BossArchetype::SuperCellSwarm { spawned_jrs: true }
        );
        let outcome = evaluate_boss_damage(
            inst.entity_id,
            inst.lifecycle.current_stage,
            &mut inst.stats,
            &mut inst.shield_active,
            &inst.damage_rule,
            &mut inst.archetype,
            Some(attacker),
            skill,
            raw_damage,
            piercing,
            bulons_alive,
            sibling_alive,
            self.parity,
            rng,
            effects,
        );
        let now_called = matches!(
            inst.archetype,
            BossArchetype::AndroidTrio {
                called_13: true,
                ..
            } | BossArchetype::SuperCellSwarm { spawned_jrs: true }
        );
        let (caller_map, caller_zone) = (inst.map_id, ZoneId(inst.zone_id.0.max(1)));
        let is_android_caller = matches!(
            inst.archetype,
            BossArchetype::AndroidTrio {
                member: 14 | 15,
                ..
            }
        );
        inst.zone_id = caller_zone;
        inst.location.map_id = caller_map;
        inst.location.zone_id = caller_zone;
        let loc = inst.location;
        if outcome.yardrat_drop_proc {
            let mut bk = ItemInstance::new(TemplateId(590), 1);
            bk.options.push(ItemOption::new(31, 1));
            effects.push(EngineEffect::DropItem(GroundDrop {
                item: bk,
                location: attacker.location,
                owner: Some(attacker.id),
            }));
        }
        if matches!(
            inst.archetype,
            BossArchetype::TetLanCon {
                tamed_owner: Some(_)
            }
        ) {
            inst.lifecycle.pk_mode = PkMode::NonPk;
            inst.lifecycle.afk_revive_at_ms = Some(now_ms.saturating_add(300_000));
            inst.lifecycle.transition_to(BossStatus::Afk, now_ms);
        }
        if outcome.killed {
            let killed_type = inst.boss_type;
            inst.last_killer = Some(attacker.clone());
            inst.lifecycle.transition_to(BossStatus::Die, now_ms);
            for o in self.instances.values_mut() {
                if o.wave_group_id == wave_id
                    && o.entity_id != actual_id
                    && let BossArchetype::RivalFlagDuel {
                        afk_reward_item, ..
                    } = o.archetype
                {
                    o.stats.hp = 0;
                    o.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                    RewardEngine::roll_boss_drops(
                        BossLootProfile::SeasonalItem {
                            item_id: afk_reward_item,
                            with_costume_opts: true,
                        },
                        loc,
                        attacker,
                        1,
                        0,
                        rng,
                        effects,
                    );
                } else if killed_type == BossTypeId(95)
                    && o.boss_type == BossTypeId(96)
                    && o.map_id == caller_map
                    && o.zone_id == caller_zone
                    && !matches!(o.lifecycle.status, BossStatus::Rest | BossStatus::LeaveMap)
                {
                    o.lifecycle.transition_to(BossStatus::LeaveMap, now_ms);
                }
            }
        }
        if !prev_called && now_called {
            for o in self
                .instances
                .values_mut()
                .filter(|b| b.wave_group_id == wave_id)
            {
                if o.lifecycle.appear_type == AppearType::CallByAnother
                    && o.lifecycle.status == BossStatus::Rest
                {
                    o.map_id = caller_map;
                    o.zone_id = caller_zone;
                    o.location.map_id = caller_map;
                    o.location.zone_id = caller_zone;
                    o.lifecycle.transition_to(BossStatus::Respawn, now_ms);
                } else if is_android_caller
                    && let BossArchetype::AndroidTrio {
                        member: 14 | 15,
                        called_13,
                    } = &mut o.archetype
                {
                    *called_13 = true;
                    o.stats.hp = o.stats.hp_max;
                }
            }
        }
        Ok(outcome)
    }

    pub(super) fn reconcile_all_waves(&mut self, now_ms: u64, effects: &mut Vec<EngineEffect>) {
        let mut wids: SmallVec<[u64; 16]> =
            self.instances.values().map(|b| b.wave_group_id).collect();
        wids.sort_unstable();
        wids.dedup();
        for wid in wids {
            let keys: SmallVec<[i64; 12]> = self
                .instances
                .iter()
                .filter(|(_, b)| b.wave_group_id == wid)
                .map(|(&k, _)| k)
                .collect();
            if keys.len() <= 1 {
                continue;
            }
            let mut snap: SmallVec<[(PlayerId, BossTypeId, LifecycleState, bool); 12]> = keys
                .iter()
                .map(|k| {
                    let b = &self.instances[k];
                    (
                        b.entity_id,
                        b.boss_type,
                        b.lifecycle.clone(),
                        b.stats.hp > 0 && b.lifecycle.status != BossStatus::Rest,
                    )
                })
                .collect();
            {
                let mut refs: SmallVec<[(PlayerId, BossTypeId, &mut LifecycleState, bool); 12]> =
                    snap.iter_mut()
                        .map(|(id, bt, lc, a)| (*id, *bt, lc, *a))
                        .collect();
                reconcile_wave_deadlocks(&mut refs, now_ms, effects);
            }
            for (k, (_, _, lc, _)) in keys.into_iter().zip(snap) {
                if let Some(inst) = self.instances.get_mut(&k) {
                    inst.lifecycle = lc;
                }
            }
        }
    }
}
