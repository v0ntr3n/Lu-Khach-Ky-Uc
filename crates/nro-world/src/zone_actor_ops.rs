//! Tick simulation, player admission, item pickup, and two-phase `ChangeMap` handlers for [`ZoneActor`].

use nro_core::{EntityKey, Location, MapId, PlayerId, ZoneId};
use tokio::sync::oneshot;
use tracing::warn;

use crate::{
    WorldError,
    s2c_packets::{self, GroundItemWire, PlayerSpawnWire},
};

use super::{
    EntityKind, GroundDropKind, GroundItem, MAX_GROUND_ITEMS, PlayerIngress, ZONE_TICK_MS,
    ZoneActor, ZoneCommand, ZoneEvent, zone_mobs::is_offline_map,
};

impl ZoneActor {
    pub(super) fn on_tick(&mut self) {
        self.now_ms = self.now_ms.saturating_add(ZONE_TICK_MS);
        self.ticks = self.ticks.saturating_add(1);
        if self.ticks.is_multiple_of(20) {
            self.step_1hz();
        }
        if self.ticks.is_multiple_of(60) {
            for item in self.ground_items.values() {
                let GroundDropKind::Satellite { clan_id } = item.kind else {
                    continue;
                };
                for i in 0..self.arena.len() {
                    if self.arena.clan_ids[i] == Some(clan_id)
                        && (i32::from(self.arena.locations[i].x) - i32::from(item.loc.x)).abs()
                            <= 200
                        && self.arena.stats[i].hp > 0
                    {
                        let prev_hp = self.arena.stats[i].hp;
                        let h = self.arena.stats[i].hp_max / 20;
                        let new_hp = prev_hp.saturating_add(h).min(self.arena.stats[i].hp_max);
                        if new_hp > prev_hp {
                            if let Some(tx) = &self.event_tx {
                                if let Some(player_id) = self.arena.player_ids[i] {
                                    let _ = tx.try_send(ZoneEvent::PlayerHpDelta {
                                        player_id,
                                        delta: new_hp - prev_hp,
                                    });
                                }
                            } else {
                                self.arena.stats[i].hp = new_hp;
                                if let Some(tx) = &self.arena.outbound_txs[i] {
                                    let _ = tx.try_send(s2c_packets::me_load_hp(new_hp));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn step_1hz(&mut self) {
        self.respawn_dead_mobs();
        self.tick_zone_pets();
        for trap in &self.traps {
            for i in 0..self.arena.len() {
                let l = self.arena.locations[i];
                if self.arena.kinds[i] == EntityKind::Player
                    && self.arena.stats[i].hp > 0
                    && l.x >= trap.rect.0
                    && l.x <= trap.rect.0.saturating_add(trap.rect.2)
                    && l.y >= trap.rect.1
                    && l.y <= trap.rect.1.saturating_add(trap.rect.3)
                {
                    let dmg = trap
                        .base_damage
                        .saturating_mul(90 + (self.rng.next_u64() % 21))
                        / 100;
                    let dmg_i64 = i64::try_from(dmg).unwrap_or(i64::MAX);
                    if let Some(tx) = &self.event_tx {
                        if let Some(player_id) = self.arena.player_ids[i]
                            && dmg_i64 > 0
                        {
                            let _ = tx.try_send(ZoneEvent::PlayerHpDelta {
                                player_id,
                                delta: dmg_i64.saturating_neg(),
                            });
                        }
                    } else {
                        self.arena.stats[i].hp =
                            self.arena.stats[i].hp.saturating_sub(dmg_i64).max(0);
                        if let Some(tx) = &self.arena.outbound_txs[i] {
                            let _ = tx.try_send(s2c_packets::me_load_point(&self.arena.stats[i]));
                        }
                    }
                }
            }
        }
        let offline = is_offline_map(self.cfg.id.0);
        let p0_x = self
            .arena
            .kinds
            .iter()
            .position(|&k| k == EntityKind::Player)
            .map(|i| self.arena.locations[i].x);
        let mut dead = Vec::new();
        let mut hopped = Vec::new();
        let max_x = self.effective_map_width();
        for (k, it) in &mut self.ground_items {
            let age = self.now_ms.saturating_sub(it.times_ms.0);
            if !offline
                && matches!(
                    it.kind,
                    GroundDropKind::Standard
                        | GroundDropKind::PlayerDiscarded
                        | GroundDropKind::EarthDragonBall
                )
                && age >= 45_000
            {
                it.owner = None;
            }
            match it.kind {
                GroundDropKind::Standard | GroundDropKind::PlayerDiscarded if age >= 60_000 => {
                    dead.push((k, it.wire_id, it.owner));
                }
                GroundDropKind::EarthDragonBall if age >= 300_000 => {
                    dead.push((k, it.wire_id, it.owner));
                }
                GroundDropKind::Satellite { .. } if age >= 1_800_000 => {
                    dead.push((k, it.wire_id, it.owner));
                }
                GroundDropKind::BlackBall
                    if self.now_ms.saturating_sub(it.times_ms.1) >= 10_000 =>
                {
                    it.times_ms.1 = self.now_ms;
                    if let Some(px) = p0_x {
                        let delta = (i32::from(px) - i32::from(it.loc.x)).clamp(-50, 50) as i16;
                        let new_x = it.loc.x.saturating_add(delta).clamp(0, max_x);
                        if new_x != it.loc.x {
                            it.loc.x = new_x;
                            hopped.push((it.wire_id, it.stack.0, it.loc, it.owner));
                        }
                    }
                }
                _ => {}
            }
        }
        for (k, wid, owner) in dead {
            self.ground_items.remove(k);
            self.broadcast_ground_item_pkt(&s2c_packets::item_map_remove(wid), owner);
        }
        for (wid, tid, loc, owner) in hopped {
            self.broadcast_ground_item_pkt(&s2c_packets::item_map_remove(wid), owner);
            let wire = GroundItemWire {
                item_map_id: wid,
                template_id: tid,
                loc,
                owner_id: owner,
            };
            let add_pkt = s2c_packets::item_map_add(&wire, self.cfg.parity);
            self.broadcast_ground_item_pkt(&add_pkt, owner);
        }
    }

    pub(super) fn wipe_mobs(&mut self) {
        let mut died = Vec::new();
        for mob in &mut self.mobs {
            if mob.status != 0 && mob.hp > 0 {
                let dmg = mob.hp;
                mob.hp = 0;
                mob.status = 0;
                mob.died_at_ms = self.now_ms;
                died.push((mob.slot_id, dmg));
            }
        }
        if !is_offline_map(self.cfg.id.0) {
            for (slot_id, dmg) in died {
                self.broadcast(&s2c_packets::mob_die(slot_id, dmg, false, &[]), None);
            }
        }
    }

    pub(super) fn admit_player(&mut self, ing: PlayerIngress) -> Result<EntityKey, WorldError> {
        self.admit_player_inner(ing, false)
    }

    pub(super) fn admit_player_inner(
        &mut self,
        ing: PlayerIngress,
        bypass_cap: bool,
    ) -> Result<EntityKey, WorldError> {
        let offline = is_offline_map(self.cfg.id.0);
        let (pid, ship_type) = (ing.identity.0, ing.ship_type);
        let has_existing = self.arena.find_player_key(pid).is_some();
        if !offline
            && !bypass_cap
            && !has_existing
            && usize::from(self.arena.player_count()) >= usize::from(self.cfg.max_players)
        {
            return Err(WorldError::ZoneFull {
                map_id: self.cfg.id.0,
                zone_id: self.cfg.id.1,
                max_players: self.cfg.max_players,
            });
        }
        let sw = PlayerSpawnWire {
            identity: ing.identity,
            level_and_pk: (1, 0),
            gender: ing.gender,
            parts: (ing.parts.0, ing.parts.1, ing.parts.2, -1, -1, -1),
            name: &ing.name,
            stats: &ing.stats,
            flags: (0, ship_type, false, 0, -1),
            loc: ing.loc,
        };
        let add_pkt = s2c_packets::player_add(&sw, 231)?;
        if has_existing {
            self.remove_pet_on_leave(pid);
            while let Some(stale_key) = self.arena.find_player_key(pid) {
                let _ = self.arena.swap_remove(stale_key);
            }
            if !offline {
                self.broadcast(&s2c_packets::player_remove(pid), None);
            }
        }
        let key = self.arena.insert_player(ing);
        let new_idx = self.arena.dense_index(key);
        if !offline {
            self.broadcast(&add_pkt, new_idx);
        }
        Ok(key)
    }

    pub(super) fn pick_item(
        &mut self,
        picker: EntityKey,
        item_map_id: i16,
    ) -> Result<GroundItem, WorldError> {
        let idx = self
            .arena
            .dense_index(picker)
            .ok_or(WorldError::EntityNotFound(picker))?;
        let pid = self.arena.player_ids[idx].ok_or(WorldError::EntityNotFound(picker))?;
        let item_key = self
            .ground_items
            .iter()
            .find_map(|(k, it)| (it.wire_id == item_map_id).then_some(k))
            .ok_or(WorldError::DungeonRule("ground item not found"))?;
        let it = self
            .ground_items
            .get(item_key)
            .ok_or(WorldError::EntityNotFound(item_key))?;
        if matches!(it.kind, GroundDropKind::Satellite { .. }) {
            return Err(WorldError::DungeonRule("cannot pick up satellite"));
        }
        if let Some(owner) = it.owner
            && owner != pid
        {
            return Err(WorldError::DungeonRule("item belongs to another player"));
        }
        let picker_loc = self.arena.locations[idx];
        if (i32::from(picker_loc.x) - i32::from(it.loc.x)).abs() > 150
            || (i32::from(picker_loc.y) - i32::from(it.loc.y)).abs() > 150
        {
            return Err(WorldError::DungeonRule("item too far"));
        }
        self.ground_items
            .remove(item_key)
            .ok_or(WorldError::EntityNotFound(item_key))
    }

    pub(super) fn restore_ground_item(&mut self, mut item: GroundItem) {
        if self.ground_items.len() >= MAX_GROUND_ITEMS {
            self.broadcast_ground_item_pkt(&s2c_packets::item_map_remove(item.wire_id), item.owner);
            return;
        }
        if self
            .ground_items
            .values()
            .any(|it| it.wire_id == item.wire_id)
        {
            let Some(new_id) = self.alloc_wire_id() else {
                return;
            };
            item.wire_id = new_id;
            let owner = item.owner;
            let wire = GroundItemWire {
                item_map_id: new_id,
                template_id: item.stack.0,
                loc: item.loc,
                owner_id: owner,
            };
            let pkt = s2c_packets::item_map_add(&wire, self.cfg.parity);
            self.ground_items.insert(item);
            self.broadcast_ground_item_pkt(&pkt, owner);
            return;
        }
        self.ground_items.insert(item);
    }

    pub(super) fn two_phase_change_map(
        &mut self,
        key: EntityKey,
        dest: (MapId, ZoneId),
        dest_loc: Location,
        ship_type: i8,
        reply: oneshot::Sender<Result<EntityKey, (WorldError, Option<EntityKey>)>>,
    ) {
        let Some(idx) = self.arena.dense_index(key) else {
            let _ = reply.send(Err((WorldError::EntityNotFound(key), None)));
            return;
        };
        if self.arena.kinds[idx] != EntityKind::Player {
            let _ = reply.send(Err((WorldError::EntityNotFound(key), None)));
            return;
        }
        let Some(dest_tx) = self.router.get(&dest).map(|e| e.value().clone()) else {
            let _ = reply.send(Err((
                WorldError::ZoneNotFound {
                    map_id: dest.0,
                    zone_id: dest.1,
                },
                Some(key),
            )));
            return;
        };
        let Some(mut p) = self.arena.swap_remove(key) else {
            let _ = reply.send(Err((WorldError::EntityNotFound(key), None)));
            return;
        };
        let pid = p.identity.0;
        let offline = is_offline_map(self.cfg.id.0);
        let pet_backup = self.pets.remove(&pid);
        if pet_backup.is_some() && !offline {
            self.broadcast(
                &s2c_packets::player_remove(PlayerId(pid.0.saturating_neg())),
                None,
            );
        }
        if !offline {
            self.broadcast(&s2c_packets::player_remove(pid), None);
        }
        let (prev_loc, rb_tx) = (p.loc, self.self_tx.clone());
        p.loc = dest_loc;
        p.ship_type = ship_type;
        tokio::spawn(async move {
            let mut backup = p.clone();
            let (ack_tx, ack_rx) = oneshot::channel();
            let err = match dest_tx
                .send(ZoneCommand::EnterPlayer(Box::new(p), ack_tx))
                .await
            {
                Ok(()) => match ack_rx.await {
                    Ok(Ok(new_key)) => {
                        if reply.send(Ok(new_key)).is_err() {
                            let _ = dest_tx
                                .send(ZoneCommand::LeavePlayer { player_id: pid })
                                .await;
                        }
                        return;
                    }
                    Ok(Err(e)) => e,
                    Err(_) => WorldError::ZoneNotFound {
                        map_id: dest.0,
                        zone_id: dest.1,
                    },
                },
                Err(_) => WorldError::ZoneNotFound {
                    map_id: dest.0,
                    zone_id: dest.1,
                },
            };
            warn!(?dest, %err, "ChangeMap rejected; rolling back to source zone");
            backup.loc = prev_loc;
            backup.ship_type = 0;
            let (rb_ack_tx, rb_rx) = oneshot::channel();
            let rollback_key = if rb_tx
                .send(ZoneCommand::RollbackPlayer {
                    ingress: Box::new(backup),
                    pet: pet_backup,
                    reply: rb_ack_tx,
                })
                .await
                .is_ok()
            {
                rb_rx.await.ok().and_then(Result::ok)
            } else {
                None
            };
            let has_rollback = rollback_key.is_some();
            if reply.send(Err((err, rollback_key))).is_err() && has_rollback {
                let _ = rb_tx
                    .send(ZoneCommand::LeavePlayer { player_id: pid })
                    .await;
            }
        });
    }
}
