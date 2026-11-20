//! Tick simulation, player admission, item pickup, and two-phase `ChangeMap` handlers for [`ZoneActor`].

use nro_core::{EntityKey, Location, MapId, ZoneId};
use tokio::sync::oneshot;
use tracing::warn;

use crate::{
    WorldError,
    s2c_packets::{self, PlayerSpawnWire},
};

use super::{
    EntityKind, GroundDropKind, GroundItem, PlayerIngress, ZONE_TICK_MS, ZoneActor, ZoneCommand,
    zone_mobs::is_offline_map,
};

const MAX_GROUND_ITEMS: usize = 512;

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
                        && (self.arena.locations[i].x - item.loc.x).abs() <= 200
                        && self.arena.stats[i].hp > 0
                    {
                        let prev_hp = self.arena.stats[i].hp;
                        let h = self.arena.stats[i].hp_max / 20;
                        let new_hp = prev_hp.saturating_add(h).min(self.arena.stats[i].hp_max);
                        if new_hp > prev_hp {
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

    fn step_1hz(&mut self) {
        self.respawn_dead_mobs();
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
                    let dmg =
                        trap.base_damage.saturating_mul(90 + (self.rng.next_u64() % 21)) / 100;
                    self.arena.stats[i].hp = self.arena.stats[i]
                        .hp
                        .saturating_sub(i64::try_from(dmg).unwrap_or(i64::MAX))
                        .max(0);
                    if let Some(tx) = &self.arena.outbound_txs[i] {
                        let _ = tx.try_send(s2c_packets::me_load_point(&self.arena.stats[i]));
                    }
                }
            }
        }
        let p0_x = self
            .arena
            .kinds
            .iter()
            .position(|&k| k == EntityKind::Player)
            .map(|i| self.arena.locations[i].x);
        let mut dead = Vec::new();
        for (k, it) in &mut self.ground_items {
            let age = self.now_ms.saturating_sub(it.times_ms.0);
            if matches!(it.kind, GroundDropKind::Standard | GroundDropKind::EarthDragonBall)
                && age >= 45_000
            {
                it.owner = None;
            }
            match it.kind {
                GroundDropKind::Standard if age >= 60_000 => dead.push((k, it.wire_id)),
                GroundDropKind::EarthDragonBall if age >= 300_000 => dead.push((k, it.wire_id)),
                GroundDropKind::Satellite { .. } if age >= 1_800_000 => dead.push((k, it.wire_id)),
                GroundDropKind::BlackBall if self.now_ms.saturating_sub(it.times_ms.1) >= 10_000 => {
                    it.times_ms.1 = self.now_ms;
                    if let Some(px) = p0_x {
                        it.loc.x = it.loc.x.saturating_add((px - it.loc.x).clamp(-50, 50));
                    }
                }
                _ => {}
            }
        }
        for (k, wid) in dead {
            self.ground_items.remove(k);
            self.broadcast(&s2c_packets::item_map_remove(wid), None);
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
        while let Some(stale_key) = self.arena.find_player_key(pid) {
            let _ = self.arena.swap_remove(stale_key);
        }
        let key = self.arena.insert_player(ing);
        let new_idx = self.arena.dense_index(key);
        if !offline {
            self.broadcast(&add_pkt, new_idx);
            if ship_type > 0 {
                self.broadcast(&s2c_packets::spaceship_arrive(pid, ship_type), None);
            }
        } else if ship_type > 0
            && let Some(idx) = new_idx
            && let Some(tx) = &self.arena.outbound_txs[idx]
        {
            let _ = tx.try_send(s2c_packets::spaceship_arrive(pid, ship_type));
        }
        Ok(key)
    }

    pub(super) fn pick_item(
        &mut self,
        picker: EntityKey,
        item_map_id: i16,
    ) -> Result<GroundItem, WorldError> {
        let idx = self.arena.dense_index(picker).ok_or(WorldError::EntityNotFound(picker))?;
        let pid = self.arena.player_ids[idx].ok_or(WorldError::EntityNotFound(picker))?;
        let item_key = self
            .ground_items
            .iter()
            .find_map(|(k, it)| (it.wire_id == item_map_id).then_some(k))
            .ok_or(WorldError::DungeonRule("ground item not found"))?;
        let it = self.ground_items.get(item_key).ok_or(WorldError::EntityNotFound(item_key))?;
        if let Some(owner) = it.owner
            && owner != pid
        {
            return Err(WorldError::DungeonRule("item belongs to another player"));
        }
        self.ground_items.remove(item_key).ok_or(WorldError::EntityNotFound(item_key))
    }

    pub(super) fn restore_ground_item(&mut self, item: GroundItem) {
        if self.ground_items.len() < MAX_GROUND_ITEMS {
            self.ground_items.insert(item);
        }
    }

    pub(super) fn two_phase_change_map(
        &mut self,
        key: EntityKey,
        dest: (MapId, ZoneId),
        dest_loc: Location,
        ship_type: i8,
        reply: oneshot::Sender<Result<EntityKey, (WorldError, Option<EntityKey>)>>,
    ) {
        let Some(dest_tx) = self.router.get(&dest).map(|e| e.value().clone()) else {
            let _ = reply.send(Err((
                WorldError::ZoneNotFound { map_id: dest.0, zone_id: dest.1 },
                Some(key),
            )));
            return;
        };
        let Some(mut p) = self.arena.swap_remove(key) else {
            let _ = reply.send(Err((WorldError::EntityNotFound(key), None)));
            return;
        };
        if !is_offline_map(self.cfg.id.0) {
            self.broadcast(&s2c_packets::player_remove(p.identity.0), None);
        }
        let (prev_loc, rb_tx) = (p.loc, self.self_tx.clone());
        p.loc = dest_loc;
        p.ship_type = ship_type;
        tokio::spawn(async move {
            let mut backup = p.clone();
            let (ack_tx, ack_rx) = oneshot::channel();
            let err = match dest_tx.send(ZoneCommand::EnterPlayer(Box::new(p), ack_tx)).await {
                Ok(()) => match ack_rx.await {
                    Ok(Ok(new_key)) => {
                        let _ = reply.send(Ok(new_key));
                        return;
                    }
                    Ok(Err(e)) => e,
                    Err(_) => WorldError::ZoneNotFound { map_id: dest.0, zone_id: dest.1 },
                },
                Err(_) => WorldError::ZoneNotFound { map_id: dest.0, zone_id: dest.1 },
            };
            warn!(?dest, %err, "ChangeMap rejected; rolling back to source zone");
            backup.loc = prev_loc;
            backup.ship_type = 0;
            let (rb_ack_tx, rb_rx) = oneshot::channel();
            let rollback_key = if rb_tx
                .send(ZoneCommand::RollbackPlayer {
                    ingress: Box::new(backup),
                    reply: rb_ack_tx,
                })
                .await
                .is_ok()
            {
                rb_rx.await.ok().and_then(Result::ok)
            } else {
                None
            };
            let _ = reply.send(Err((err, rollback_key)));
        });
    }
}
