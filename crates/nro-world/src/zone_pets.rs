//! Companion disciple (`PetCompanion`) zone visibility, follow movement, and 1 Hz live mob combat.

use nro_core::{CoreStats, Gender, Location, MapId, PlayerId};
use nro_protocol::{Packet, PacketError};
use serde::{Deserialize, Serialize};

use crate::s2c_packets::{self, PlayerSpawnWire};

use super::{EntityKind, ZoneActor, ZoneEvent, zone_mobs::is_offline_map};

/// Lightweight live companion pet state replicated inside a [`ZoneActor`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetCompanion {
    /// Display name prefixed with `"$"` so the client sets `Char.isPet = true`.
    pub name: String,
    /// Pet's planet lineage (`0..=2` for `nClass` and `cgender`).
    pub gender: Gender,
    /// Sprite `(head, body, leg)` parts.
    pub parts: (i16, i16, i16),
    /// Equipped flag/clan bag part ID (`-1` when none).
    pub flag_bag: i16,
    /// Current HP (`>= 0`).
    pub hp: i64,
    /// Maximum HP (`>= 1`).
    pub hp_max: i64,
    /// Outgoing attack damage (`u64`).
    pub dame: u64,
    /// Pet command mode (`0 = Follow`, `1 = Protect`, `2 = Attack`, `3 = GoHome`).
    pub status: i8,
    /// Master's planet lineage (home map is `21 + master_gender as u16`).
    pub master_gender: Gender,
    /// Current coordinate inside the zone.
    pub loc: Location,
}

impl PetCompanion {
    /// Returns `true` iff this pet should be visible on `zone_map_id`.
    #[inline]
    #[must_use]
    pub const fn is_visible_on(&self, zone_map_id: MapId) -> bool {
        self.status != 3
            || (zone_map_id.0 >= 0 && zone_map_id.0 as u16 == 21 + self.master_gender as u16)
    }
}

fn ensure_dollar_prefix(name: &mut String) {
    if name.trim().is_empty() {
        "$Đệ tử".clone_into(name);
    } else if !name.starts_with('$') {
        name.insert(0, '$');
    }
}

fn pet_add_packet(master_pid: PlayerId, pet: &PetCompanion) -> Result<Packet, PacketError> {
    let pet_id = PlayerId(master_pid.0.saturating_neg());
    let owned_name;
    let wire_name: &str = if pet.name.starts_with('$') {
        &pet.name
    } else {
        owned_name = format!("${}", pet.name);
        &owned_name
    };
    let stats = CoreStats {
        hp: pet.hp.max(0),
        hp_max: pet.hp_max.max(1),
        ..CoreStats::default()
    };
    let sw = PlayerSpawnWire {
        identity: (pet_id, None),
        level_and_pk: (1, 0),
        gender: pet.gender,
        parts: (pet.parts.0, pet.parts.1, pet.parts.2, -1, -1, -1),
        name: wire_name,
        stats: &stats,
        flags: (0, 0, false, 0, -1),
        loc: pet.loc,
    };
    s2c_packets::player_add_with_bag(&sw, pet.flag_bag)
}

impl ZoneActor {
    pub(super) fn sync_player_pet(&mut self, player_id: PlayerId, incoming: Option<PetCompanion>) {
        let Some(master_ek) = self.arena.find_player_key(player_id) else {
            self.pets.remove(&player_id);
            return;
        };
        let Some(master_idx) = self.arena.dense_index(master_ek) else {
            self.pets.remove(&player_id);
            return;
        };
        let master_loc = self.arena.locations[master_idx];
        let map_id = self.cfg.id.0;
        let zone_id = self.cfg.id.1;
        let prev = self.pets.remove(&player_id);
        let mut next = incoming.filter(|p| p.is_visible_on(map_id));
        if let Some(ref mut cur) = next {
            ensure_dollar_prefix(&mut cur.name);
            cur.loc = if let Some(ref old) = prev {
                Location {
                    map_id,
                    zone_id,
                    ..old.loc
                }
            } else if cur.status == 3 && matches!(map_id.0, 21..=23) {
                Location {
                    map_id,
                    zone_id,
                    x: if map_id.0 == 22 { 452 } else { 220 },
                    y: 336,
                }
            } else {
                Location {
                    map_id,
                    zone_id,
                    x: (master_loc.x.saturating_sub(28)).max(24),
                    y: master_loc.y,
                }
            };
        }

        let pet_id = PlayerId(player_id.0.saturating_neg());
        match (prev, next) {
            (Some(_), None) => {
                self.emit_for_actor(master_idx, &s2c_packets::player_remove(pet_id));
            }
            (None, Some(cur)) => {
                if let Ok(add_pkt) = pet_add_packet(player_id, &cur) {
                    self.emit_for_actor(master_idx, &add_pkt);
                }
                if cur.hp <= 0 {
                    let (_, die_pkt) = s2c_packets::char_die_packets(pet_id, 0, cur.loc, 0);
                    self.emit_for_actor(master_idx, &die_pkt);
                }
                self.pets.insert(player_id, cur);
            }
            (Some(old), Some(cur)) => {
                let visual_changed = old.parts != cur.parts
                    || old.flag_bag != cur.flag_bag
                    || old.name != cur.name
                    || old.gender != cur.gender;
                if visual_changed {
                    self.emit_for_actor(master_idx, &s2c_packets::player_remove(pet_id));
                    if let Ok(add_pkt) = pet_add_packet(player_id, &cur) {
                        self.emit_for_actor(master_idx, &add_pkt);
                    }
                    if cur.hp <= 0 {
                        let (_, die_pkt) = s2c_packets::char_die_packets(pet_id, 0, cur.loc, 0);
                        self.emit_for_actor(master_idx, &die_pkt);
                    }
                } else if old.hp != cur.hp || old.hp_max != cur.hp_max {
                    if old.hp <= 0 && cur.hp > 0 {
                        let (_, live_pkt) =
                            s2c_packets::char_revive_packets(pet_id, cur.hp, cur.hp_max, cur.loc);
                        self.emit_for_actor(master_idx, &live_pkt);
                    }
                    self.emit_for_actor(
                        master_idx,
                        &s2c_packets::player_load_hp(pet_id, cur.hp, cur.hp_max),
                    );
                    if old.hp > 0 && cur.hp <= 0 {
                        let (_, die_pkt) = s2c_packets::char_die_packets(pet_id, 0, cur.loc, 0);
                        self.emit_for_actor(master_idx, &die_pkt);
                    }
                }
                self.pets.insert(player_id, cur);
            }
            (None, None) => {}
        }
    }

    pub(super) fn sync_pets_on_zone_state(&self, target_idx: usize, pid: PlayerId) {
        let Some(target_tx) = self
            .arena
            .outbound_txs
            .get(target_idx)
            .and_then(Option::as_ref)
        else {
            return;
        };
        let offline = is_offline_map(self.cfg.id.0);
        if let Some(own_pet) = self.pets.get(&pid)
            && let Ok(pkt) = pet_add_packet(pid, own_pet)
        {
            let _ = target_tx.try_send(pkt.clone());
            if !offline {
                self.broadcast(&pkt, Some(target_idx));
            }
            if own_pet.hp <= 0 {
                let (_, die_pkt) = s2c_packets::char_die_packets(
                    PlayerId(pid.0.saturating_neg()),
                    0,
                    own_pet.loc,
                    0,
                );
                let _ = target_tx.try_send(die_pkt.clone());
                if !offline {
                    self.broadcast(&die_pkt, Some(target_idx));
                }
            }
        }
        if !offline {
            for (&other_pid, other_pet) in &self.pets {
                if other_pid != pid
                    && let Ok(pkt) = pet_add_packet(other_pid, other_pet)
                {
                    let _ = target_tx.try_send(pkt);
                    if other_pet.hp <= 0 {
                        let (_, die_pkt) = s2c_packets::char_die_packets(
                            PlayerId(other_pid.0.saturating_neg()),
                            0,
                            other_pet.loc,
                            0,
                        );
                        let _ = target_tx.try_send(die_pkt);
                    }
                }
            }
        }
    }

    pub(super) fn move_following_pet(&mut self, master_idx: usize, prev_x: i16) {
        let Some(pid) = self.arena.player_ids[master_idx] else {
            return;
        };
        let master_loc = self.arena.locations[master_idx];
        let max_x = self.cfg.map_width.max(4_095);
        let Some(pet) = self.pets.get_mut(&pid) else {
            return;
        };
        if pet.status == 3 {
            return;
        }
        let delta: i16 = if master_loc.x >= prev_x { -28 } else { 28 };
        let next_x = master_loc.x.saturating_add(delta).max(24).min(max_x);
        pet.loc = Location {
            x: next_x,
            y: master_loc.y,
            ..master_loc
        };
        let move_pkt = s2c_packets::player_move(PlayerId(pid.0.saturating_neg()), pet.loc);
        self.emit_for_actor(master_idx, &move_pkt);
    }

    pub(super) fn remove_pet_on_leave(&mut self, pid: PlayerId) {
        if self.pets.remove(&pid).is_some() && !is_offline_map(self.cfg.id.0) {
            self.broadcast(
                &s2c_packets::player_remove(PlayerId(pid.0.saturating_neg())),
                None,
            );
        }
    }

    pub(super) fn tick_zone_pets(&mut self) {
        if self.pets.is_empty() {
            return;
        }
        let max_x = self.cfg.map_width.max(4_095);
        let masters: Vec<(PlayerId, usize, Location)> = (0..self.arena.len())
            .filter(|&i| self.arena.kinds[i] == EntityKind::Player && self.arena.stats[i].hp > 0)
            .filter_map(|i| self.arena.player_ids[i].map(|pid| (pid, i, self.arena.locations[i])))
            .collect();

        for (pid, master_idx, master_loc) in masters {
            let Some(pet_snap) = self.pets.get(&pid).cloned() else {
                continue;
            };
            if pet_snap.hp <= 0 {
                continue;
            }
            let pet_id = PlayerId(pid.0.saturating_neg());
            if pet_snap.status == 3 {
                if matches!(self.cfg.id.0.0, 21..=23) && self.ticks.is_multiple_of(100) {
                    let (lo, hi) = if self.cfg.id.0.0 == 22 {
                        (452_i16, 500_i16)
                    } else {
                        (200, 250)
                    };
                    let nx = if pet_snap.loc.x <= lo { hi } else { lo };
                    let next_loc = Location {
                        x: nx,
                        y: 336,
                        ..pet_snap.loc
                    };
                    if let Some(p) = self.pets.get_mut(&pid) {
                        p.loc = next_loc;
                    }
                    self.emit_for_actor(master_idx, &s2c_packets::player_move(pet_id, next_loc));
                }
                continue;
            }
            if pet_snap.status != 1 && pet_snap.status != 2 {
                continue;
            }
            let leash_px: i64 = if pet_snap.status == 1 { 220 } else { 400 };
            let leash_sq = leash_px * leash_px;
            let target_mob = self
                .mobs
                .iter()
                .filter(|m| m.status != 0 && m.hp > 0)
                .filter_map(|m| {
                    let dx = i64::from(master_loc.x) - i64::from(m.loc.x);
                    let dy = i64::from(master_loc.y) - i64::from(m.loc.y);
                    let dist_sq = dx * dx + dy * dy;
                    (dist_sq <= leash_sq).then_some((dist_sq, m.slot_id, m.loc))
                })
                .min_by_key(|&(dist_sq, slot_id, _)| (dist_sq, slot_id));

            if let Some((_, mob_slot, mob_loc)) = target_mob {
                let side: i16 = if pet_snap.loc.x <= mob_loc.x { -24 } else { 24 };
                let fight_loc = Location {
                    x: mob_loc.x.saturating_add(side).max(24).min(max_x),
                    y: mob_loc.y,
                    map_id: self.cfg.id.0,
                    zone_id: self.cfg.id.1,
                };
                if let Some(p) = self.pets.get_mut(&pid) {
                    p.loc = fight_loc;
                }
                self.emit_for_actor(master_idx, &s2c_packets::player_move(pet_id, fight_loc));
                self.emit_for_actor(
                    master_idx,
                    &s2c_packets::peer_attack_mobs(pet_id, 0, &[mob_slot]),
                );
                if let Some((earned_tnsm, killed_mob_temp_id)) =
                    self.hit_single_mob(pid, master_idx, mob_slot, pet_snap.dame.max(15), false)
                    && earned_tnsm > 0
                    && let Some(tx) = &self.event_tx
                {
                    let _ = tx.try_send(ZoneEvent::PetCombatReward {
                        player_id: pid,
                        earned_tnsm,
                        killed_mob_temp_id,
                    });
                }
            } else if (i64::from(pet_snap.loc.x) - i64::from(master_loc.x)).abs() > 60
                || pet_snap.loc.y != master_loc.y
            {
                let idle_loc = Location {
                    x: (master_loc.x.saturating_sub(28)).max(24),
                    y: master_loc.y,
                    ..master_loc
                };
                if let Some(p) = self.pets.get_mut(&pid) {
                    p.loc = idle_loc;
                }
                self.emit_for_actor(master_idx, &s2c_packets::player_move(pet_id, idle_loc));
            }
        }
    }
}
