//! Live zone monster state, combat resolution, 5-second respawn, offline isolation, and state sync.

use nro_core::{EntityKey, Location, MapId, PlayerId, TemplateId};
use nro_protocol::Packet;
use smallvec::SmallVec;

use crate::s2c_packets::{self, GroundItemWire, PlayerSpawnWire};

use super::{EntityKind, GroundDropKind, GroundItem, MAX_GROUND_ITEMS, ZoneActor};

/// Initial monster spawn descriptor seeded into a [`ZoneActor`] via [`super::ZoneCommand::SeedMobs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneMobSeed {
    /// Monster template ID (`mob_template.id`).
    pub temp_id: i16,
    /// Monster combat level.
    pub level: u8,
    /// Maximum HP (`>= 1`).
    pub max_hp: u64,
    /// Initial spawn coordinate inside the zone.
    pub loc: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ZoneMob {
    pub slot_id: u8,
    pub temp_id: i16,
    pub level: u8,
    pub hp: u64,
    pub max_hp: u64,
    pub status: i8,
    pub loc: Location,
    pub died_at_ms: u64,
}

#[inline]
pub(super) const fn is_offline_map(map_id: MapId) -> bool {
    matches!(map_id.0, 21..=23 | 39..=41 | 45..=50)
}

impl ZoneActor {
    pub(super) fn emit_for_actor(&self, actor_idx: usize, pkt: &Packet) {
        if is_offline_map(self.cfg.id.0) {
            if let Some(tx) = self
                .arena
                .outbound_txs
                .get(actor_idx)
                .and_then(Option::as_ref)
            {
                let _ = tx.try_send(pkt.clone());
            }
        } else {
            self.broadcast(pkt, None);
        }
    }

    pub(super) fn send_existing_peers(&self, target_idx: usize) {
        let Some(target_tx) = self
            .arena
            .outbound_txs
            .get(target_idx)
            .and_then(Option::as_ref)
        else {
            return;
        };
        for i in 0..self.arena.len() {
            if i == target_idx || self.arena.kinds[i] != EntityKind::Player {
                continue;
            }
            let Some(other_pid) = self.arena.player_ids[i] else {
                continue;
            };
            let other_parts = self.arena.parts[i];
            let other_sw = PlayerSpawnWire {
                identity: (other_pid, self.arena.clan_ids[i]),
                level_and_pk: (1, 0),
                gender: self.arena.genders[i],
                parts: (other_parts.0, other_parts.1, other_parts.2, -1, -1, -1),
                name: &self.arena.names[i],
                stats: &self.arena.stats[i],
                flags: (0, 0, false, 0, -1),
                loc: self.arena.locations[i],
            };
            if let Ok(other_pkt) = s2c_packets::player_add(&other_sw, 231) {
                let _ = target_tx.try_send(other_pkt);
            }
        }
    }

    pub(super) fn seed_mobs(&mut self, seeds: Vec<ZoneMobSeed>) {
        self.mobs = seeds
            .into_iter()
            .take(127)
            .enumerate()
            .map(|(idx, s)| {
                let max_hp = s.max_hp.max(1);
                ZoneMob {
                    slot_id: idx as u8,
                    temp_id: s.temp_id,
                    level: s.level,
                    hp: max_hp,
                    max_hp,
                    status: 5,
                    loc: s.loc,
                    died_at_ms: 0,
                }
            })
            .collect();
    }

    pub(super) fn hit_single_mob(
        &mut self,
        owner_pid: PlayerId,
        actor_idx: usize,
        mob_u8: u8,
        damage: u64,
        is_crit: bool,
    ) -> Option<(u64, Option<i16>)> {
        let outcome = {
            let mob = self.mobs.get_mut(usize::from(mob_u8))?;
            if mob.status == 0 || mob.hp == 0 {
                return None;
            }
            let actual = damage.max(2).min(mob.hp);
            mob.hp -= actual;
            if mob.hp == 0 {
                mob.status = 0;
                mob.died_at_ms = self.now_ms;
            }
            (actual, mob.hp, mob.temp_id, mob.level, mob.loc)
        };
        let (actual, hp_after, temp_id, level, drop_loc) = outcome;
        let mob_tnsm = if temp_id == 0 {
            1
        } else {
            actual.saturating_mul(5).clamp(10, 250_000)
        };
        if hp_after == 0 {
            let mut drops: SmallVec<[GroundItemWire; 1]> = SmallVec::new();
            if self.ground_items.len() < MAX_GROUND_ITEMS
                && let Some(wire_id) = self.alloc_wire_id()
            {
                let gold_qty = (u32::from(level.max(1)) * 50).max(100);
                self.ground_items.insert(GroundItem {
                    wire_id,
                    stack: (TemplateId(76), gold_qty),
                    loc: drop_loc,
                    owner: Some(owner_pid),
                    times_ms: (self.now_ms, self.now_ms),
                    kind: GroundDropKind::Standard,
                    options: SmallVec::new(),
                });
                drops.push(GroundItemWire {
                    item_map_id: wire_id,
                    template_id: TemplateId(76),
                    loc: drop_loc,
                    owner_id: Some(owner_pid),
                });
            }
            let die_pkt = s2c_packets::mob_die(mob_u8, actual, is_crit, &drops);
            self.emit_for_actor(actor_idx, &die_pkt);
            Some((mob_tnsm, Some(temp_id)))
        } else {
            let hp_pkt = s2c_packets::mob_hp_after_hit(mob_u8, hp_after, actual, is_crit);
            self.emit_for_actor(actor_idx, &hp_pkt);
            Some((mob_tnsm, None))
        }
    }

    pub(super) fn attack_mobs(
        &mut self,
        attacker: EntityKey,
        skill_paint_id: u8,
        mob_ids: &[u8],
        damage: u64,
        is_crit: bool,
    ) -> (u64, Vec<i16>) {
        const MAX_MOB_RANGE_SQ: i64 = 690 * 690;
        let Some(attacker_idx) = self.arena.dense_index(attacker) else {
            return (0, Vec::new());
        };
        let Some(pid) = self.arena.player_ids[attacker_idx] else {
            return (0, Vec::new());
        };
        let attacker_loc = self.arena.locations[attacker_idx];
        let mob_limit = self.mobs.len().min(128);
        let mut valid_mobs: SmallVec<[u8; 10]> = SmallVec::new();
        let mut seen = [false; 128];
        for &mob_u8 in mob_ids {
            let idx = usize::from(mob_u8);
            if idx >= mob_limit || seen[idx] {
                continue;
            }
            seen[idx] = true;
            let mob = &self.mobs[idx];
            let dx = i64::from(attacker_loc.x) - i64::from(mob.loc.x);
            let dy = i64::from(attacker_loc.y) - i64::from(mob.loc.y);
            if mob.status != 0 && mob.hp > 0 && dx * dx + dy * dy <= MAX_MOB_RANGE_SQ {
                valid_mobs.push(mob_u8);
            }
        }
        if valid_mobs.is_empty() {
            return (0, Vec::new());
        }
        if !is_offline_map(self.cfg.id.0) {
            self.broadcast(
                &s2c_packets::peer_attack_mobs(pid, skill_paint_id, &valid_mobs),
                Some(attacker_idx),
            );
        }
        let mut tnsm = 0_u64;
        let mut killed_temp_ids = Vec::new();
        for mob_u8 in valid_mobs {
            if let Some((mob_tnsm, maybe_killed)) =
                self.hit_single_mob(pid, attacker_idx, mob_u8, damage, is_crit)
            {
                tnsm = tnsm.saturating_add(mob_tnsm);
                if let Some(tid) = maybe_killed {
                    killed_temp_ids.push(tid);
                }
            }
        }
        (tnsm, killed_temp_ids)
    }

    pub(super) fn respawn_dead_mobs(&mut self) {
        if matches!(
            self.cfg.id.0.0,
            53..=62 | 135..=138 | 141..=144 | 146..=149 | 151 | 152
        ) {
            return;
        }
        let mut live_pkts = Vec::new();
        for mob in &mut self.mobs {
            if mob.status == 0 && self.now_ms.saturating_sub(mob.died_at_ms) >= 5_000 {
                mob.hp = mob.max_hp;
                mob.status = 5;
                let hp_i64 = i64::try_from(mob.hp).unwrap_or(i64::MAX);
                live_pkts.push(s2c_packets::npc_live(mob.slot_id, 0, 0, hp_i64));
            }
        }
        for pkt in &live_pkts {
            self.broadcast(pkt, None);
        }
    }

    pub(super) fn sync_zone_state(&self, ek: EntityKey) {
        let Some(idx) = self.arena.dense_index(ek) else {
            return;
        };
        let Some(pid) = self.arena.player_ids[idx] else {
            return;
        };
        let offline = is_offline_map(self.cfg.id.0);
        if !offline {
            self.send_existing_peers(idx);
        }
        self.sync_pets_on_zone_state(idx, pid);
        let Some(tx) = &self.arena.outbound_txs[idx] else {
            return;
        };
        for it in self.ground_items.values() {
            if offline && it.owner != Some(pid) {
                continue;
            }
            let wire = GroundItemWire {
                item_map_id: it.wire_id,
                template_id: it.stack.0,
                loc: it.loc,
                owner_id: it.owner,
            };
            let _ = tx.try_send(s2c_packets::item_map_add(&wire, self.cfg.parity));
        }
        for mob in &self.mobs {
            if mob.status == 0 {
                let _ = tx.try_send(s2c_packets::mob_die(mob.slot_id, 0, false, &[]));
            } else if mob.hp < mob.max_hp {
                let _ = tx.try_send(s2c_packets::mob_hp_after_hit(mob.slot_id, mob.hp, 1, false));
            }
        }
    }
}
