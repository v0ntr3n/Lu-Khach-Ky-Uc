//! Single-threaded `50 ms` [`ZoneActor`] combining `slotmap::SlotMap` generational handles with
//! Struct-of-Arrays (SoA) slices and deadlock-free two-phase `ChangeMap` handoffs.

#[path = "zone_actor_arena.rs"]
mod zone_actor_arena;
#[path = "zone_actor_ops.rs"]
mod zone_actor_ops;
#[path = "zone_mobs.rs"]
mod zone_mobs;
#[cfg(test)]
#[path = "zone_actor_tests.rs"]
mod tests;

pub use zone_actor_arena::*;
pub use zone_mobs::ZoneMobSeed;

use std::sync::Arc;

use dashmap::DashMap;
use nro_core::{CoreStats, EntityKey, GameRng, Location, MapId, PlayerId, TemplateId, ZoneId};
use nro_protocol::Packet;
use slotmap::SlotMap;
use smallvec::SmallVec;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Duration, MissedTickBehavior};
use tracing::{debug, info, instrument};
use zone_mobs::{ZoneMob, is_offline_map};

use crate::{
    WorldError,
    s2c_packets::{self, GroundItemWire},
};

/// Simulation interval for every [`ZoneActor`] (`50 ms` = 20 Hz).
pub const ZONE_TICK_MS: u64 = 50;

/// Mailbox commands processed sequentially by [`ZoneActor`].
#[derive(Debug)]
pub enum ZoneCommand {
    /// Admits an entering player (`Box<PlayerIngress>` + `EntityKey` reply).
    EnterPlayer(Box<PlayerIngress>, oneshot::Sender<Result<EntityKey, WorldError>>),
    /// Re-admits a player after a failed `ChangeMap`, bypassing the zone player cap.
    RollbackPlayer {
        /// Restored player ingress payload.
        ingress: Box<PlayerIngress>,
        /// Reply channel receiving the re-assigned [`EntityKey`].
        reply: oneshot::Sender<Result<EntityKey, WorldError>>,
    },
    /// Removes a disconnecting/leaving player and broadcasts `-6` (`PLAYER_REMOVE`).
    LeavePlayer {
        /// Leaving player's ID.
        player_id: PlayerId,
    },
    /// Moves an entity (`-7` `PLAYER_MOVE` or `46` `RESET_POINT`).
    MovePlayer(EntityKey, Location),
    /// Broadcasts zone chat (`44`).
    ChatMap(EntityKey, String),
    /// Spawns a ground item (`68` `ITEMMAP_ADD`).
    DropItem(TemplateId, u32, Location, Option<PlayerId>, GroundDropKind),
    /// Picks up a ground item by wire `item_map_id` (`-20` / `-19`).
    PickItem(EntityKey, i16, oneshot::Sender<Result<GroundItem, WorldError>>),
    /// Restores a previously picked [`GroundItem`] back onto the zone floor (e.g. bag full).
    RestoreGroundItem(GroundItem),
    /// Performs a non-blocking two-phase `ChangeMap` handoff via `DashMap` with automatic rollback.
    ChangeMap(
        EntityKey,
        (MapId, ZoneId),
        Location,
        i8,
        oneshot::Sender<Result<EntityKey, (WorldError, Option<EntityKey>)>>,
    ),
    /// Populates live monsters (`0..127`) for this zone.
    SeedMobs(Vec<ZoneMobSeed>),
    /// Resolves a player's attack against zone monsters, replying with `(tnsm, killed_temp_ids)`.
    AttackMobs {
        /// Attacking player's arena key.
        attacker: EntityKey,
        /// Client skill paint ID (`u8`).
        skill_paint_id: u8,
        /// Targeted mob slot indices (`u8`).
        mob_ids: Vec<u8>,
        /// Hit damage before clamping.
        damage: u64,
        /// Critical strike flag.
        is_crit: bool,
        /// Reply channel receiving `(earned_tnsm, killed_mob_template_ids)`.
        reply: oneshot::Sender<(u64, Vec<i16>)>,
    },
    /// Sends existing peers, ground drops, and non-full mob HP states to `EntityKey`.
    SyncZoneState(EntityKey),
    /// Updates a player's live [`CoreStats`] in the zone arena.
    UpdatePlayerStats(EntityKey, CoreStats),
    /// Updates a player's live `(head, body, leg)` sprite parts in the zone arena.
    UpdatePlayerParts(EntityKey, (i16, i16, i16)),
    /// Terminates the actor loop cleanly.
    Shutdown,
}

/// Isolated single-threaded zone simulation actor ticking every `50 ms`.
pub struct ZoneActor {
    cfg: ZoneConfig,
    rx: mpsc::Receiver<ZoneCommand>,
    self_tx: mpsc::Sender<ZoneCommand>,
    router: Arc<DashMap<(MapId, ZoneId), mpsc::Sender<ZoneCommand>>>,
    arena: ZoneSoaArena,
    mobs: Vec<ZoneMob>,
    ground_items: SlotMap<EntityKey, GroundItem>,
    traps: Vec<TrapVolume>,
    rng: GameRng,
    next_wire_id: i16,
    now_ms: u64,
    ticks: u64,
}

impl ZoneActor {
    /// Constructs a [`ZoneActor`] and registers its mailbox in `router`.
    #[must_use]
    pub fn new(
        cfg: ZoneConfig,
        router: Arc<DashMap<(MapId, ZoneId), mpsc::Sender<ZoneCommand>>>,
        traps: Vec<TrapVolume>,
        rng: GameRng,
    ) -> (Self, mpsc::Sender<ZoneCommand>) {
        let (tx, rx) = mpsc::channel(64);
        router.insert(cfg.id, tx.clone());
        (
            Self {
                cfg,
                rx,
                self_tx: tx.clone(),
                router,
                arena: ZoneSoaArena::default(),
                mobs: Vec::new(),
                ground_items: SlotMap::with_key(),
                traps,
                rng,
                next_wire_id: 1,
                now_ms: 0,
                ticks: 0,
            },
            tx,
        )
    }

    /// Runs the `50 ms` `tokio::time::interval` loop until `Shutdown` or channel drop.
    #[instrument(skip(self), fields(zone = ?self.cfg.id))]
    pub async fn run(mut self) {
        let mut ticker = time::interval(Duration::from_millis(ZONE_TICK_MS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        info!("zone actor online");
        loop {
            tokio::select! {
                _ = ticker.tick() => self.on_tick(),
                cmd = self.rx.recv() => match cmd {
                    Some(c) => if !self.handle_command(c) { break; },
                    None => break,
                },
            }
        }
        self.router.remove(&self.cfg.id);
        debug!("zone actor offline");
    }

    fn broadcast(&self, pkt: &Packet, skip: Option<usize>) {
        for (i, tx) in self.arena.outbound_txs.iter().enumerate() {
            if skip != Some(i)
                && let Some(ch) = tx
            {
                let _ = ch.try_send(pkt.clone());
            }
        }
    }

    fn handle_command(&mut self, cmd: ZoneCommand) -> bool {
        let offline = is_offline_map(self.cfg.id.0);
        match cmd {
            ZoneCommand::EnterPlayer(ing, reply) => {
                let _ = reply.send(self.admit_player(*ing));
            }
            ZoneCommand::RollbackPlayer { ingress, reply } => {
                let _ = reply.send(self.admit_player_inner(*ingress, true));
            }
            ZoneCommand::LeavePlayer { player_id } => {
                if let Some(key) = self.arena.find_player_key(player_id) {
                    let _ = self.arena.swap_remove(key);
                    if !offline {
                        self.broadcast(&s2c_packets::player_remove(player_id), None);
                    }
                }
            }
            ZoneCommand::MovePlayer(key, to) => {
                if let Some(i) = self.arena.dense_index(key) {
                    if (85..=91).contains(&self.cfg.id.0.0)
                        && (to.x < 24 || to.x > self.cfg.map_width.saturating_sub(24))
                    {
                        if let Some(tx) = &self.arena.outbound_txs[i] {
                            let _ = tx.try_send(s2c_packets::reset_point(self.arena.locations[i]));
                        }
                    } else {
                        self.arena.locations[i] = Location {
                            x: to.x.clamp(0, self.cfg.map_width.max(4_095)),
                            ..to
                        };
                        if !offline
                            && let Some(pid) = self.arena.player_ids[i]
                        {
                            self.broadcast(
                                &s2c_packets::player_move(pid, self.arena.locations[i]),
                                Some(i),
                            );
                        }
                    }
                }
            }
            ZoneCommand::ChatMap(key, text) => {
                if let Some(i) = self.arena.dense_index(key)
                    && let Some(pid) = self.arena.player_ids[i]
                    && let Ok(pkt) = s2c_packets::chat_map(pid, &text)
                {
                    self.emit_for_actor(i, &pkt);
                }
            }
            ZoneCommand::DropItem(tid, qty, loc, owner, kind) => {
                let wire_id = self.next_wire_id;
                self.next_wire_id = self.next_wire_id.wrapping_add(1).max(1);
                self.ground_items.insert(GroundItem {
                    wire_id,
                    stack: (tid, qty),
                    loc,
                    owner,
                    times_ms: (self.now_ms, self.now_ms),
                    kind,
                    options: SmallVec::new(),
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
                if offline
                    && let Some(owner_pid) = owner
                {
                    if let Some(ek) = self.arena.find_player_key(owner_pid)
                        && let Some(idx) = self.arena.dense_index(ek)
                    {
                        self.emit_for_actor(idx, &pkt);
                    }
                } else {
                    self.broadcast(&pkt, None);
                }
            }
            ZoneCommand::PickItem(picker, item_map_id, reply) => {
                let _ = reply.send(self.pick_item(picker, item_map_id));
            }
            ZoneCommand::RestoreGroundItem(item) => {
                self.restore_ground_item(item);
            }
            ZoneCommand::ChangeMap(key, dest, dest_loc, ship, reply) => {
                self.two_phase_change_map(key, dest, dest_loc, ship, reply);
            }
            ZoneCommand::SeedMobs(seeds) => self.seed_mobs(seeds),
            ZoneCommand::AttackMobs { attacker, skill_paint_id, mob_ids, damage, is_crit, reply } => {
                let res = self.attack_mobs(attacker, skill_paint_id, &mob_ids, damage, is_crit);
                let _ = reply.send(res);
            }
            ZoneCommand::SyncZoneState(ek) => self.sync_zone_state(ek),
            ZoneCommand::UpdatePlayerStats(ek, stats) => {
                if let Some(i) = self.arena.dense_index(ek) {
                    self.arena.stats[i] = stats;
                }
            }
            ZoneCommand::UpdatePlayerParts(ek, parts) => {
                if let Some(i) = self.arena.dense_index(ek) {
                    self.arena.parts[i] = parts;
                }
            }
            ZoneCommand::Shutdown => return false,
        }
        true
    }
}
