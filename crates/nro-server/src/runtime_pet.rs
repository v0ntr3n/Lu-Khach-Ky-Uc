//! Runtime `ZoneEvent` consumer (`PlayerHpDelta` & `PetCombatReward`) and online pet passive revival/regen.

use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use nro_boss::EngineEffect;
use nro_consts::const_player;
use nro_core::PlayerId;
use nro_player::{combat::StatUpgradeKind, npoint::NPoint, pet::PET_REVIVE_DELAY_MS};
use nro_world::{ZoneCommand, ZoneEvent, s2c_packets};

use super::{LeaderboardSlot, SharedServerState, unix_now_ms};
use crate::router::ServerSessionHandler;

const MAX_PET_POWER: i64 = 200_000_000_000;

static DEAD_PET_SINCE_MS: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn dead_pet_since_ms() -> &'static DashMap<PlayerId, i64> {
    DEAD_PET_SINCE_MS.get_or_init(DashMap::new)
}

pub(super) async fn handle_zone_event(shared: &Arc<SharedServerState>, ev: ZoneEvent) {
    match ev {
        ZoneEvent::PlayerHpDelta { player_id, delta } if delta < 0 => {
            let dmg = EngineEffect::DamagePlayer {
                target: player_id,
                damage: delta.saturating_neg(),
                piercing: true,
            };
            shared.dispatch_boss_effects(vec![dmg]).await;
        }
        ZoneEvent::PlayerHpDelta { player_id, delta } if delta > 0 => {
            let sid = shared.clients.players.get(&player_id).map(|s| *s.value());
            if let Some(sid) = sid
                && let Some(mut entry) = shared.clients.authenticated.get_mut(&sid)
                && entry.snapshot.stats.hp > 0
            {
                let new_hp = entry
                    .snapshot
                    .stats
                    .hp
                    .saturating_add(delta)
                    .min(entry.snapshot.stats.hp_max.max(1));
                if new_hp != entry.snapshot.stats.hp {
                    entry.snapshot.stats.hp = new_hp;
                    if entry.handle.sent_key() {
                        let _ = entry.handle.try_send(s2c_packets::me_load_hp(new_hp));
                    }
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(ztx)) = (
                        entry.entity_key,
                        shared.world_router.resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ =
                            ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                }
            }
        }
        ZoneEvent::PlayerHpDelta { .. } => {}
        ZoneEvent::PetCombatReward {
            player_id,
            earned_tnsm,
            killed_mob_temp_id,
        } => {
            if earned_tnsm == 0 {
                return;
            }
            let Some(sid) = shared.clients.players.get(&player_id).map(|s| *s.value()) else {
                return;
            };
            let now_ms = unix_now_ms();
            let handler = ServerSessionHandler::new(Arc::clone(shared));
            let outcome = shared
                .clients
                .authenticated
                .get_mut(&sid)
                .and_then(|mut entry| {
                    let scaled_base = handler.compute_player_tnsm(&entry, earned_tnsm, now_ms);
                    let pet_gain = i64::try_from(scaled_base).unwrap_or(i64::MAX).max(1);
                    let master_gain = (pet_gain / 2).max(1);
                    let pet = entry.snapshot.pet.as_mut()?;
                    if pet.fusion_type != const_player::NON_FUSION {
                        return None;
                    }
                    pet.stats.power = pet.stats.power.saturating_add(pet_gain).min(MAX_PET_POWER);
                    pet.stats.tiem_nang = pet
                        .stats
                        .tiem_nang
                        .saturating_add(pet_gain)
                        .min(MAX_PET_POWER);
                    let mut np = NPoint::new(pet.stats);
                    let cycle = [
                        StatUpgradeKind::Hp,
                        StatUpgradeKind::Mp,
                        StatUpgradeKind::Dame,
                        StatUpgradeKind::Def,
                    ];
                    let start = (now_ms.cast_unsigned() as usize) & 3;
                    for step in 0..16_usize {
                        let kind = cycle[(start + step) & 3];
                        let _ = np.increase_point(kind, 1, shared.config.parity_mode);
                    }
                    pet.stats = np.core;
                    if (pet.stats.power & 3) == 0 && pet.stats.stamina > 0 {
                        pet.stats.stamina -= 1;
                    }
                    entry.snapshot.stats.power =
                        entry.snapshot.stats.power.saturating_add(master_gain);
                    entry.snapshot.stats.tiem_nang =
                        entry.snapshot.stats.tiem_nang.saturating_add(master_gain);
                    crate::router_pet::recalculate_pet_in_snapshot(shared, &mut entry.snapshot);
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(ztx)) = (
                        entry.entity_key,
                        shared.world_router.resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ =
                            ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    let exp_i32 =
                        i32::try_from(master_gain.min(i64::from(i32::MAX))).unwrap_or(i32::MAX);
                    let det_pkt = crate::router_pet::build_pet_detail_pkt(shared, &entry.snapshot);
                    Some((
                        entry.handle.clone(),
                        s2c_packets::player_up_exp(2, exp_i32),
                        det_pkt,
                    ))
                });

            if let Some((handle, exp_pkt, det_pkt)) = outcome {
                shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                if handle.sent_key() {
                    let _ = handle.try_send(exp_pkt);
                    if let Some(dp) = det_pkt {
                        let _ = handle.try_send(dp);
                    }
                }
                crate::router_pet::sync_online_pet_map_state(shared, player_id).await;
                handler
                    .on_pet_combat_progress(&handle, sid, killed_mob_temp_id)
                    .await;
            }
        }
    }
}

pub(super) async fn tick_passive_pet_regen(shared: &SharedServerState) {
    let now_ms = unix_now_ms();
    let mut updates = Vec::new();
    for mut entry in shared.clients.authenticated.iter_mut() {
        let pid = entry.snapshot.id;
        let Some(pet) = entry.snapshot.pet.as_mut() else {
            dead_pet_since_ms().remove(&pid);
            continue;
        };
        if pet.fusion_type != const_player::NON_FUSION {
            dead_pet_since_ms().remove(&pid);
            continue;
        }
        let hp_max = pet.stats.hp_max.max(1);
        let mp_max = pet.stats.mp_max.max(1);
        let max_stam = pet.stats.max_stamina.max(1_000);
        let changed = if pet.stats.hp <= 0 {
            let dead_since = *dead_pet_since_ms().entry(pid).or_insert(now_ms);
            if now_ms.saturating_sub(dead_since) >= PET_REVIVE_DELAY_MS as i64 {
                dead_pet_since_ms().remove(&pid);
                pet.stats.hp = hp_max;
                pet.stats.mp = mp_max;
                pet.stats.stamina = pet.stats.stamina.max(100).min(max_stam);
                true
            } else {
                false
            }
        } else {
            dead_pet_since_ms().remove(&pid);
            let new_hp = if pet.stats.hp < hp_max {
                pet.stats
                    .hp
                    .saturating_add((hp_max / 20).max(1))
                    .min(hp_max)
            } else {
                pet.stats.hp
            };
            let new_mp = if pet.stats.mp < mp_max {
                pet.stats
                    .mp
                    .saturating_add((mp_max / 20).max(1))
                    .min(mp_max)
            } else {
                pet.stats.mp
            };
            let new_stam = if pet.stats.stamina < max_stam {
                pet.stats.stamina.saturating_add(10).min(max_stam)
            } else {
                pet.stats.stamina
            };
            let diff =
                new_hp != pet.stats.hp || new_mp != pet.stats.mp || new_stam != pet.stats.stamina;
            pet.stats.hp = new_hp;
            pet.stats.mp = new_mp;
            pet.stats.stamina = new_stam;
            diff
        };
        if changed {
            let pkt = crate::router_pet::build_pet_detail_pkt(shared, &entry.snapshot);
            updates.push((pid, entry.handle.clone(), pkt));
        }
    }
    dead_pet_since_ms().retain(|pid, _| shared.clients.players.contains_key(pid));
    for (pid, handle, maybe_pkt) in updates {
        if handle.sent_key()
            && let Some(pkt) = maybe_pkt
        {
            let _ = handle.try_send(pkt);
        }
        crate::router_pet::sync_online_pet_map_state(shared, pid).await;
    }
}
