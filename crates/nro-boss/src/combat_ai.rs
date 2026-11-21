//! Active combat tick executor (`Boss.attack()` & `Boss.active()`) across all boss archetypes.
//!
//! Drives Cell `hapThu` absorption, Golden Frieza `2.1B` AoE bomb & `DeathBeam` contact kills,
//! Majin Buu 12h stone/chocolate & 14h Map `128` swallowing, Ginyu body-change, Thief `AnTrom`
//! gold theft, `Dracula` blood drain, `ChanXu` stun, `OngGiaNoel` `648` gift boxes, and
//! `ToSuKaio` passive TNSM ticks.

use crate::{
    BossArchetype, BossTypeId, CoreStats, EngineEffect, GameRng, GroundDrop, ItemInstance,
    Location, MapId, ParityMode, PlayerDebuffKind, PlayerId, PlayerView, TemplateId, ZoneId,
    loc_distance,
    mechanics::{broly_grow, steal_thief_gold},
};

/// Executes one active combat tick (`BossStatus::Active`) for a `BossArchetype`.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn run_boss_combat_tick(
    boss_id: PlayerId,
    map_id: MapId,
    zone_id: ZoneId,
    location: &mut Location,
    stats: &mut CoreStats,
    archetype: &mut BossArchetype,
    players: &[PlayerView],
    parity: ParityMode,
    rng: &mut GameRng,
    effects: &mut Vec<EngineEffect>,
) {
    if players.is_empty() {
        return;
    }
    let pick = &players[rng.next_int(0, (players.len() - 1) as i32) as usize];

    match archetype {
        BossArchetype::Broly { is_super } => {
            if rng.is_true(1, 100) {
                broly_grow(stats, *is_super, rng);
            }
            effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: stats.dame,
                piercing: false,
            });
        }
        BossArchetype::CellAbsorb => {
            if rng.is_true(1, 100) {
                stats.dame = stats.dame.saturating_add(pick.stats.dame.saturating_mul(5) / 100);
                stats.hp_max = stats.hp_max.saturating_add(pick.stats.hp.saturating_mul(2) / 100);
                stats.hp = stats.hp.saturating_add(pick.stats.hp).min(stats.hp_max);
                stats.crit = stats.crit.saturating_add(1).min(100);
                *location = pick.location;
                effects.push(EngineEffect::DamagePlayer {
                    target: pick.id,
                    damage: pick.stats.hp_max,
                    piercing: true,
                });
            }
        }
        BossArchetype::GoldenFrieza21h => match rng.next_int(0, 2) {
            0 => {
                for pl in players {
                    effects.push(EngineEffect::DamagePlayer {
                        target: pl.id,
                        damage: 2_100_000_000,
                        piercing: true,
                    });
                }
            }
            1 => {
                for slot in 0..5_i32 {
                    effects.push(EngineEffect::SpawnDynamicBoss {
                        boss_type: BossTypeId(46 + slot),
                        map_id,
                        zone_id,
                        location: *location,
                        scaled_stats: None,
                    });
                }
            }
            _ => effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: stats.dame,
                piercing: false,
            }),
        },
        BossArchetype::DeathBeamProjectile { .. } => {
            location.x = pick.location.x;
            for pl in players {
                if (i32::from(pl.location.x) - i32::from(location.x)).abs() < 5 {
                    effects.push(EngineEffect::DamagePlayer {
                        target: pl.id,
                        damage: 2_100_000_000,
                        piercing: true,
                    });
                }
            }
        }
        BossArchetype::MajinBuu12h { casts_stone, is_final_mabu, .. } => {
            if *casts_stone && rng.is_true(1, 20) {
                effects.push(EngineEffect::DebuffPlayer {
                    target: pick.id,
                    kind: PlayerDebuffKind::PetrifyStone,
                    duration_ms: 22_000,
                });
            }
            if *is_final_mabu && rng.is_true(1, 15) {
                effects.push(EngineEffect::DebuffPlayer {
                    target: pick.id,
                    kind: PlayerDebuffKind::Chocolate,
                    duration_ms: 30_000,
                });
            }
            effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: stats.dame,
                piercing: false,
            });
        }
        BossArchetype::MajinBuu14h { is_belly_super_bu: false } => {
            if rng.is_true(1, 5) {
                effects.push(EngineEffect::TeleportPlayer {
                    target: pick.id,
                    map_id: MapId(128),
                    zone_id,
                    location: Location::new(300, 300),
                });
            }
            effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: stats.dame,
                piercing: false,
            });
        }
        BossArchetype::GinyuCaptain => {
            if rng.is_true(1, 2) {
                effects.push(EngineEffect::DebuffPlayer {
                    target: pick.id,
                    kind: PlayerDebuffKind::BodyChange,
                    duration_ms: 15_000,
                });
            }
        }
        BossArchetype::ThiefAnTrom { stolen_gold } => {
            let amt = steal_thief_gold(stolen_gold, pick, *location, parity, rng);
            if amt > 0 {
                effects.push(EngineEffect::StealGold { target: pick.id, amount: amt });
            }
        }
        BossArchetype::MiniHazard { variant } => match *variant {
            0 | 1 => {
                for pl in players {
                    if loc_distance(*location, pl.location) <= 200 && rng.is_true(30, 100) {
                        effects.push(EngineEffect::DebuffPlayer {
                            target: pl.id,
                            kind: PlayerDebuffKind::Contagion { lethal_on_expire: true },
                            duration_ms: if *variant == 0 { 30_000 } else { 300_000 },
                        });
                    }
                }
            }
            _ => {
                effects.push(EngineEffect::DamagePlayer {
                    target: pick.id,
                    damage: (pick.stats.hp_max / 10).max(1),
                    piercing: true,
                });
            }
        },
        BossArchetype::DhvtArena { is_chan_xu, spawns_clones, .. } => {
            if *is_chan_xu && rng.is_true(1, 5) {
                let dur = (rng.next_int(1, 10) * 1_000) as u32;
                stats.crit = 100;
                effects.push(EngineEffect::DebuffPlayer {
                    target: pick.id,
                    kind: PlayerDebuffKind::Stun,
                    duration_ms: dur,
                });
            }
            if *spawns_clones && rng.is_true(1, 30) {
                for _ in 0..4 {
                    effects.push(EngineEffect::SpawnDynamicBoss {
                        boss_type: BossTypeId(96),
                        map_id,
                        zone_id,
                        location: *location,
                        scaled_stats: Some((5_000, 20_000_000)),
                    });
                }
            }
        }
        BossArchetype::HatMitSpec { drains_blood, cycles_stealth, .. } => {
            if *drains_blood {
                let drain = (pick.stats.hp_max / 10).max(1);
                stats.hp = (stats.hp + drain).min(stats.hp_max);
                effects.push(EngineEffect::DamagePlayer {
                    target: pick.id,
                    damage: drain,
                    piercing: true,
                });
            }
            if *cycles_stealth && rng.is_true(1, 3) {
                location.y = if location.y == 10_000 { 360 } else { 10_000 };
            }
        }
        BossArchetype::TrainingMaster { passive_tnsm_per_10s, .. } if *passive_tnsm_per_10s > 0 => {
            effects.push(EngineEffect::GrantTnsm {
                target: pick.id,
                amount: *passive_tnsm_per_10s,
            });
        }
        BossArchetype::HalloweenBoss { disguise_code } => {
            let dyn_dame = (pick.stats.hp_max / i64::from(rng.next_int(30, 50))).max(1);
            effects.push(EngineEffect::DebuffPlayer {
                target: pick.id,
                kind: PlayerDebuffKind::HalloweenCurse(*disguise_code),
                duration_ms: 1_800_000,
            });
            effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: dyn_dame,
                piercing: false,
            });
        }
        BossArchetype::ChristmasSanta => {
            if loc_distance(*location, pick.location) <= 50 {
                for (den, dx) in [(3, 0_i16), (5, 25), (7, -25)] {
                    if rng.is_true(1, den) {
                        effects.push(EngineEffect::DropItem(GroundDrop {
                            item: ItemInstance::new(TemplateId(648), 1),
                            location: Location { x: location.x.saturating_add(dx), ..*location },
                            owner: None,
                        }));
                    }
                }
            }
        }
        BossArchetype::YardratPatrol { point_a, point_b, .. } => {
            *location = if rng.is_true(1, 2) { *point_a } else { *point_b };
            let pct_dmg = (pick.stats.hp_max.saturating_mul(i64::from(rng.next_int(1, 3))) / 100)
                / i64::from(rng.next_int(10, 30));
            effects.push(EngineEffect::DamagePlayer {
                target: pick.id,
                damage: pct_dmg.max(1),
                piercing: false,
            });
        }
        _ => {
            if stats.dame > 0 {
                effects.push(EngineEffect::DamagePlayer {
                    target: pick.id,
                    damage: stats.dame,
                    piercing: false,
                });
            }
        }
    }
    let _ = boss_id;
}
