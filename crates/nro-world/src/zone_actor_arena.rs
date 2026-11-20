//! Data types and Struct-of-Arrays (SoA) entity arena for [`super::ZoneActor`].

use nro_core::{
    AccountId, ClanId, CoreStats, EntityKey, Gender, ItemOption, Location, MapId, ParityMode,
    PlayerId, SessionKey, TemplateId, ZoneId,
};
use nro_protocol::Packet;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use smallvec::SmallVec;
use tokio::sync::mpsc;

/// Entity class discriminator inside [`ZoneSoaArena`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    /// Connected human player, companion pet, bot, regular mob, or boss.
    Player,
    /// Companion disciple (`Pet`).
    Pet,
    /// Server bot/clone.
    Bot,
    /// Regular zone monster (`Mob`).
    Mob,
    /// Dungeon, world, or tournament boss (`Boss`).
    Boss,
}

/// Ownership & despawn lifecycle policy for ground drops (`ItemMap.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundDropKind {
    /// `45s` exclusive owner -> `15s` public -> `60s` despawn.
    Standard,
    /// Earth Dragon Ball `14..=20` (`45s` owner -> `300s` despawn).
    EarthDragonBall,
    /// Clan satellite `342..=345` (`1,800s` lifetime, `3s` pulse within `200 px`).
    Satellite {
        /// Clan buffed by the satellite.
        clan_id: ClanId,
    },
    /// Black Dragon Ball `372..=378` (hops `±50 px` toward player 0 every `10s`).
    BlackBall,
    /// Namek Dragon Ball `353..=359` / `362`.
    NamekBall,
}

/// Ground item state stored in `SlotMap<EntityKey, GroundItem>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundItem {
    /// Wire 16-bit ground item ID.
    pub wire_id: i16,
    /// `(template_id, quantity)` stack.
    pub stack: (TemplateId, u32),
    /// Ground coordinate.
    pub loc: Location,
    /// Exclusive loot owner (`None` = public).
    pub owner: Option<PlayerId>,
    /// `(spawned_at_ms, last_hop_ms)` timestamps.
    pub times_ms: (u64, u64),
    /// Lifecycle rule.
    pub kind: GroundDropKind,
    /// Preserved item options carried by this ground drop.
    pub options: SmallVec<[ItemOption; 4]>,
}

/// Environmental hazard volume (`TrapMap.java`, e.g. Map `135` spike trap `effectId = 49`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrapVolume {
    /// `(x, y, width, height)` bounding rectangle.
    pub rect: (i16, i16, i16, i16),
    /// Base damage per `1,000 ms` pulse (`±10%` jitter).
    pub base_damage: u64,
}

/// Serializable player state moved atomically across zones during `ChangeMap`.
#[derive(Clone, Debug)]
pub struct PlayerIngress {
    /// `(account_id, session_key)` session identity.
    pub session: (AccountId, SessionKey),
    /// `(player_id, clan_id)` character identity.
    pub identity: (PlayerId, Option<ClanId>),
    /// Planet gender.
    pub gender: Gender,
    /// Display name.
    pub name: String,
    /// Destination coordinate.
    pub loc: Location,
    /// Combat stats.
    pub stats: CoreStats,
    /// `(head, body, leg)` sprite parts.
    pub parts: (i16, i16, i16),
    /// Spaceship arrival style (`0..=3`).
    pub ship_type: i8,
    /// Outbound session packet sender.
    pub outbound_tx: Option<mpsc::Sender<Packet>>,
}

/// Static configuration for a [`super::ZoneActor`].
#[derive(Clone, Copy, Debug)]
pub struct ZoneConfig {
    /// Owning `(map_id, zone_id)`.
    pub id: (MapId, ZoneId),
    /// Maximum concurrent human players (`maxPlayer`).
    pub max_players: u8,
    /// Horizontal map width in pixels.
    pub map_width: i16,
    /// Wire parity mode.
    pub parity: ParityMode,
}

/// Generational `SlotMap<EntityKey, usize>` paired with cache-contiguous Struct-of-Arrays slices.
#[derive(Debug, Default)]
pub struct ZoneSoaArena {
    index_map: SlotMap<EntityKey, usize>,
    dense_keys: Vec<EntityKey>,
    /// Entity classification slice.
    pub kinds: Vec<EntityKind>,
    /// Optional wire `PlayerId` slice.
    pub player_ids: Vec<Option<PlayerId>>,
    /// Optional `(AccountId, SessionKey)` slice.
    pub sessions: Vec<Option<(AccountId, SessionKey)>>,
    /// Optional `ClanId` slice.
    pub clan_ids: Vec<Option<ClanId>>,
    /// Planet gender slice.
    pub genders: Vec<Gender>,
    /// Display name slice.
    pub names: Vec<String>,
    /// Spatial coordinate slice.
    pub locations: Vec<Location>,
    /// Combat stat slice (`u64` HP/MP/Attack).
    pub stats: Vec<CoreStats>,
    /// `(head, body, leg)` part slice.
    pub parts: Vec<(i16, i16, i16)>,
    /// Outbound session channel slice.
    pub outbound_txs: Vec<Option<mpsc::Sender<Packet>>>,
    player_count: u8,
}

impl ZoneSoaArena {
    /// Pre-allocates parallel SoA columns for `cap` entities.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            index_map: SlotMap::with_capacity_and_key(cap),
            dense_keys: Vec::with_capacity(cap),
            kinds: Vec::with_capacity(cap),
            player_ids: Vec::with_capacity(cap),
            sessions: Vec::with_capacity(cap),
            clan_ids: Vec::with_capacity(cap),
            genders: Vec::with_capacity(cap),
            names: Vec::with_capacity(cap),
            locations: Vec::with_capacity(cap),
            stats: Vec::with_capacity(cap),
            parts: Vec::with_capacity(cap),
            outbound_txs: Vec::with_capacity(cap),
            player_count: 0,
        }
    }

    /// Connected human player count.
    #[must_use]
    pub const fn player_count(&self) -> u8 {
        self.player_count
    }

    /// Total dense entity count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dense_keys.len()
    }

    /// Returns `true` if no entities inhabit the arena.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dense_keys.is_empty()
    }

    /// Resolves `key` to its current dense SoA slice index.
    #[must_use]
    pub fn dense_index(&self, key: EntityKey) -> Option<usize> {
        self.index_map.get(key).copied()
    }

    /// Resolves a connected human player's [`EntityKey`] by [`PlayerId`].
    #[must_use]
    pub fn find_player_key(&self, player_id: PlayerId) -> Option<EntityKey> {
        self.player_ids
            .iter()
            .enumerate()
            .find_map(|(i, &pid)| {
                (self.kinds[i] == EntityKind::Player && pid == Some(player_id))
                    .then_some(self.dense_keys[i])
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn push_row(
        &mut self,
        kind: EntityKind,
        pid: Option<PlayerId>,
        sess: Option<(AccountId, SessionKey)>,
        clan: Option<ClanId>,
        g: Gender,
        name: String,
        loc: Location,
        st: CoreStats,
        parts: (i16, i16, i16),
        tx: Option<mpsc::Sender<Packet>>,
    ) -> EntityKey {
        let idx = self.dense_keys.len();
        let key = self.index_map.insert(idx);
        self.dense_keys.push(key);
        self.kinds.push(kind);
        self.player_ids.push(pid);
        self.sessions.push(sess);
        self.clan_ids.push(clan);
        self.genders.push(g);
        self.names.push(name);
        self.locations.push(loc);
        self.stats.push(st);
        self.parts.push(parts);
        self.outbound_txs.push(tx);
        key
    }

    /// Inserts a human [`PlayerIngress`] into all SoA slices in `O(1)`.
    pub fn insert_player(&mut self, ing: PlayerIngress) -> EntityKey {
        self.player_count = self.player_count.saturating_add(1);
        self.push_row(
            EntityKind::Player,
            Some(ing.identity.0),
            Some(ing.session),
            ing.identity.1,
            ing.gender,
            ing.name,
            ing.loc,
            ing.stats,
            ing.parts,
            ing.outbound_tx,
        )
    }

    /// Inserts a Pet, Bot, Mob, or Boss into all SoA slices in `O(1)`.
    pub fn insert_non_player(
        &mut self,
        kind: EntityKind,
        id: Option<PlayerId>,
        name: String,
        loc: Location,
        stats: CoreStats,
    ) -> EntityKey {
        self.push_row(kind, id, None, None, Gender::TraiDat, name, loc, stats, (0, 0, 0), None)
    }

    /// Removes `key` in `O(1)` via swap-remove across all SoA slices, returning [`PlayerIngress`] for players.
    pub fn swap_remove(&mut self, key: EntityKey) -> Option<PlayerIngress> {
        let idx = self.index_map.remove(key)?;
        let kind = self.kinds.swap_remove(idx);
        self.dense_keys.swap_remove(idx);
        let (pid, sess, clan_id) = (
            self.player_ids.swap_remove(idx),
            self.sessions.swap_remove(idx),
            self.clan_ids.swap_remove(idx),
        );
        let (gender, name, loc) = (
            self.genders.swap_remove(idx),
            self.names.swap_remove(idx),
            self.locations.swap_remove(idx),
        );
        let (stats, parts, outbound_tx) = (
            self.stats.swap_remove(idx),
            self.parts.swap_remove(idx),
            self.outbound_txs.swap_remove(idx),
        );
        if idx < self.dense_keys.len()
            && let Some(slot) = self.index_map.get_mut(self.dense_keys[idx])
        {
            *slot = idx;
        }
        (kind == EntityKind::Player).then(|| {
            self.player_count = self.player_count.saturating_sub(1);
            Some(PlayerIngress {
                session: sess?,
                identity: (pid?, clan_id),
                gender,
                name,
                loc,
                stats,
                parts,
                ship_type: 0,
                outbound_tx,
            })
        })?
    }
}
