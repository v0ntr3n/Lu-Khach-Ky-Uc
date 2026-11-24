//! Ground item spawning and `Bùa Thu Hút` (`td_thu_hut_ms`, item `219`) magnet pickup for [`super::ZoneActor`].

use nro_core::{EntityKey, ItemOption, Location, PlayerId, TemplateId};
use smallvec::SmallVec;

use crate::s2c_packets::{self, GroundItemWire};

use super::{EntityKind, GroundDropKind, GroundItem, MAX_GROUND_ITEMS, ZoneActor};

/// Maximum horizontal/vertical attraction radius (`px`) for public (`owner == None`) drops.
const MAGNET_PUBLIC_RANGE_PX: i32 = 500;
/// Minimum ground dwell time (`ms`) before periodic magnet sweep pulls an item so client `ItemMap` spawns cleanly.
const MAGNET_SWEEP_MIN_AGE_MS: u64 = 250;
/// Maximum ground items pulled per player per magnet sweep (`500 ms`).
const MAGNET_MAX_BATCH: usize = 8;

#[inline]
const fn is_currency_template(tid: TemplateId) -> bool {
    matches!(tid.0, 76 | 77 | 188..=190 | 861)
}

impl ZoneActor {
    pub(super) fn spawn_ground_item(
        &mut self,
        tid: TemplateId,
        qty: u32,
        loc: Location,
        owner: Option<PlayerId>,
        kind: GroundDropKind,
        options: SmallVec<[ItemOption; 4]>,
    ) {
        if self.ground_items.len() >= MAX_GROUND_ITEMS {
            return;
        }
        let Some(wire_id) = self.alloc_wire_id() else {
            return;
        };
        self.ground_items.insert(GroundItem {
            wire_id,
            stack: (tid, qty),
            loc,
            owner,
            times_ms: (self.now_ms, self.now_ms),
            kind,
            options,
        });
        let pkt = s2c_packets::item_map_add(
            &GroundItemWire {
                item_map_id: wire_id,
                template_id: tid,
                loc,
                owner_id: owner,
            },
            self.cfg.parity,
        );
        self.broadcast_ground_item_pkt(&pkt, owner);
    }

    pub(super) fn magnet_pick_items(
        &mut self,
        picker: EntityKey,
        specific_wire_id: Option<i16>,
        has_empty_bag_slot: bool,
        stackable_bag_tids: &[TemplateId],
    ) -> SmallVec<[GroundItem; 8]> {
        let mut out = SmallVec::new();
        let Some(idx) = self.arena.dense_index(picker) else {
            return out;
        };
        if self.arena.kinds[idx] != EntityKind::Player || self.arena.stats[idx].hp <= 0 {
            return out;
        }
        let Some(pid) = self.arena.player_ids[idx] else {
            return out;
        };
        let picker_loc = self.arena.locations[idx];
        let mut keys: SmallVec<[EntityKey; 8]> = SmallVec::new();

        for (k, it) in &self.ground_items {
            if keys.len() >= MAGNET_MAX_BATCH {
                break;
            }
            if let Some(target_wid) = specific_wire_id {
                if it.wire_id != target_wid {
                    continue;
                }
            } else if self.now_ms.saturating_sub(it.times_ms.0) < MAGNET_SWEEP_MIN_AGE_MS {
                continue;
            }
            if matches!(it.kind, GroundDropKind::Satellite { .. }) {
                continue;
            }
            if specific_wire_id.is_none()
                && matches!(it.kind, GroundDropKind::PlayerDiscarded)
                && it.owner == Some(pid)
            {
                continue;
            }
            let in_range = match it.owner {
                Some(owner) => owner == pid,
                None => {
                    (i32::from(picker_loc.x) - i32::from(it.loc.x)).abs() <= MAGNET_PUBLIC_RANGE_PX
                        && (i32::from(picker_loc.y) - i32::from(it.loc.y)).abs()
                            <= MAGNET_PUBLIC_RANGE_PX
                }
            };
            if !in_range {
                continue;
            }
            if specific_wire_id.is_none()
                && !is_currency_template(it.stack.0)
                && !has_empty_bag_slot
                && !stackable_bag_tids.contains(&it.stack.0)
            {
                continue;
            }
            keys.push(k);
            if specific_wire_id.is_some() {
                break;
            }
        }

        for k in keys {
            if let Some(item) = self.ground_items.remove(k) {
                out.push(item);
            }
        }
        out
    }
}
