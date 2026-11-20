//! Complete server-to-client (`S2C`) binary [`Packet`] builders for `Service.java`,
//! `ChangeMapService.java`, `Zone.java`, `Mob.java`, `ChatGlobalService.java`, and `NpcService.java`.

#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::missing_errors_doc)]

#[path = "s2c_packets_combat.rs"]
mod s2c_packets_combat;
#[path = "s2c_packets_player.rs"]
mod s2c_packets_player;
#[path = "s2c_packets_ui.rs"]
mod s2c_packets_ui;
#[path = "s2c_packets_world_entry.rs"]
mod s2c_packets_world_entry;
#[cfg(test)]
#[path = "s2c_packets_tests.rs"]
mod tests;

pub use s2c_packets_combat::*;
pub use s2c_packets_player::*;
pub use s2c_packets_ui::*;
pub use s2c_packets_world_entry::*;

use bytes::Bytes;
use nro_consts::cmd_message;
use nro_core::{
    ClanId, CoreStats, Gender, Location, MapId, ParityMode, PlayerId, TemplateId, ZoneId,
};
use nro_protocol::{Packet, PacketError, PacketWriter};
use serde::{Deserialize, Serialize};

/// Clamps a `u64` stat to `0..=i32::MAX` so 32-bit wire fields never wrap negative (`NPoint.java` fix).
#[inline]
#[must_use]
pub fn clamp_u64_to_wire_i32(val: u64) -> i32 {
    i32::try_from(val).unwrap_or(i32::MAX)
}

/// Clamps a `u64` stat to `0..=i64::MAX` so 64-bit v2.5.0 wire fields never wrap negative.
#[inline]
#[must_use]
pub fn clamp_u64_to_wire_i64(val: u64) -> i64 {
    i64::try_from(val).unwrap_or(i64::MAX)
}

#[inline]
fn pid_i32(id: PlayerId) -> i32 {
    i64::from(id.0) as i32
}

fn build_pkt(cmd: i8, cap: usize, f: impl FnOnce(&mut PacketWriter)) -> Packet {
    let mut w = PacketWriter::with_capacity(cap);
    f(&mut w);
    w.finish(cmd)
}

fn build_pkt_res(
    cmd: i8,
    cap: usize,
    f: impl FnOnce(&mut PacketWriter) -> Result<(), PacketError>,
) -> Result<Packet, PacketError> {
    let mut w = PacketWriter::with_capacity(cap);
    f(&mut w)?;
    Ok(w.finish(cmd))
}

/// Allocates a `-30` (`SUB_COMMAND`) writer prefixed with `sub_cmd`.
#[must_use]
pub fn sub_command_writer(sub_cmd: i8, cap: usize) -> PacketWriter {
    let mut w = PacketWriter::with_capacity(cap + 1);
    w.write_i8(sub_cmd);
    w
}

/// Portal entry serialized inside [`map_info`] (`-24`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WayPointWire {
    /// `(min_x, min_y, max_x, max_y)` portal bounds.
    pub bounds: (i16, i16, i16, i16),
    /// `(is_enter, is_offline)` portal flags.
    pub flags: (bool, bool),
    /// Portal HUD caption.
    pub name: String,
}

/// Monster entry serialized inside [`map_info`] (`-24`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobWire {
    /// `(mob_id, temp_id, level, status, lv_mob, is_big_boss)` descriptor.
    pub meta: (u8, i16, i8, i8, i8, bool),
    /// `(current_hp, max_hp)` clamped to `i64::MAX` on wire.
    pub hp_pair: (u64, u64),
    /// Spawn coordinate.
    pub loc: Location,
}

/// NPC entry serialized inside [`map_info`] (`-24`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcWire {
    /// `(status, temp_id, avatar)` NPC visuals.
    pub meta: (i8, i8, i16),
    /// NPC coordinate.
    pub loc: Location,
}

/// Ground drop entry serialized in [`map_info`] (`-24`), [`mob_die`] (`-12`), and [`item_map_add`] (`68`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundItemWire {
    /// Zone ground-item handle (`short itemMapId`).
    pub item_map_id: i16,
    /// Item template ID.
    pub template_id: TemplateId,
    /// Ground coordinate.
    pub loc: Location,
    /// Exclusive loot owner (`None` -> `-1`).
    pub owner_id: Option<PlayerId>,
}

/// Complete zone snapshot for [`map_info`] (`Zone.mapInfo`, opcode `-24`).
#[derive(Clone, Debug)]
pub struct MapInfoSnapshot<'a> {
    /// `(map_id, zone_id)` identifiers.
    pub zone: (MapId, ZoneId),
    /// `(planet_id, tile_id, bg_id, map_type, bg_type, spaceship_arrive)` header bytes.
    pub header: (i8, i8, i8, i8, i8, i8),
    /// Localized map title.
    pub map_name: &'a str,
    /// Entering player's coordinate.
    pub spawn_loc: Location,
    /// Map portals.
    pub waypoints: &'a [WayPointWire],
    /// Zone monsters.
    pub mobs: &'a [MobWire],
    /// Visible NPCs.
    pub npcs: &'a [NpcWire],
    /// Ground items.
    pub ground_items: &'a [GroundItemWire],
    /// Raw `(bg_item_bytes, eff_map_bytes)` layers (`short 0` emitted when empty).
    pub raw_layers: (Bytes, Bytes),
}

/// Appearance & state payload for [`player_add`] (`Zone.infoPlayer`, opcode `-5`).
#[derive(Clone, Debug)]
pub struct PlayerSpawnWire<'a> {
    /// `(player_id, clan_id)` identity.
    pub identity: (PlayerId, Option<ClanId>),
    /// `(level, type_pk)` bytes.
    pub level_and_pk: (i8, i8),
    /// Planet gender.
    pub gender: Gender,
    /// `(head, body, leg, mount_id, id_aura, id_hat)` sprite parts.
    pub parts: (i16, i16, i16, i16, i16, i16),
    /// Character display name.
    pub name: &'a str,
    /// Combat stats.
    pub stats: &'a CoreStats,
    /// `(flag_bag, spaceship_arrive, is_monkey, c_flag, id_eff_set)` flags.
    pub flags: (u8, i8, bool, i8, i8),
    /// Current zone coordinate.
    pub loc: Location,
}

/// Zone row for [`open_zone_ui`] (`ChangeMapService.openZoneUI`, opcode `29`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneUiRow {
    /// Zone index.
    pub zone_id: ZoneId,
    /// `(num_players, max_players)` occupancy.
    pub counts: (u8, u8),
    /// Optional `SuperRank` arena match banner `(name_a, rank_a, name_b, rank_b)`.
    pub rank_match: Option<(String, i32, String, i32)>,
}

fn put_ground(w: &mut PacketWriter, d: &GroundItemWire, ov: Option<i32>) {
    for v in [d.item_map_id, i32::from(d.template_id.0) as i16, d.loc.x, d.loc.y] {
        w.write_i16(v);
    }
    w.write_i32(ov.unwrap_or_else(|| d.owner_id.map_or(-1, pid_i32)));
}

fn put_layer(w: &mut PacketWriter, b: &Bytes) {
    if b.is_empty() {
        w.write_i16(0);
    } else {
        for &x in b {
            w.write_i8(x as i8);
        }
    }
}

/// Builds opcode `-24` (`MAP_INFO`) matching v2.5.0 `Zone.mapInfo` byte-for-byte.
pub fn map_info(s: &MapInfoSnapshot<'_>) -> Result<Packet, PacketError> {
    build_pkt_res(
        cmd_message::MAP_INFO,
        192 + s.raw_layers.0.len() + s.raw_layers.1.len(),
        |w| {
            let raw_map = i32::from(s.zone.0.0);
            for b in [raw_map as i8, s.header.0, s.header.1, s.header.2, s.header.3] {
                w.write_i8(b);
            }
            w.write_utf(s.map_name)?;
            w.write_i8(s.zone.1.0.min(127) as i8);
            for v in [s.spawn_loc.x, s.spawn_loc.y] {
                w.write_i16(v);
            }
            let wp_n = s.waypoints.len().min(i8::MAX as usize);
            w.write_i8(wp_n as i8);
            for wp in &s.waypoints[..wp_n] {
                for v in [wp.bounds.0, wp.bounds.1, wp.bounds.2, wp.bounds.3] {
                    w.write_i16(v);
                }
                for b in [i8::from(wp.flags.0), i8::from(wp.flags.1)] {
                    w.write_i8(b);
                }
                w.write_utf(&wp.name)?;
            }
            let mob_n = s.mobs.len().min(i8::MAX as usize);
            w.write_i8(mob_n as i8);
            for m in &s.mobs[..mob_n] {
                for b in [0, 0, 0, 0, 0] {
                    w.write_i8(b);
                }
                w.write_i16(m.meta.1);
                w.write_i8(0);
                w.write_i64(clamp_u64_to_wire_i64(m.hp_pair.0));
                w.write_i8(m.meta.2);
                w.write_i64(clamp_u64_to_wire_i64(m.hp_pair.1));
                for v in [m.loc.x, m.loc.y] {
                    w.write_i16(v);
                }
                for b in [m.meta.3, m.meta.4, i8::from(m.meta.5)] {
                    w.write_i8(b);
                }
            }
            let npc_n = s.npcs.len().min(i8::MAX as usize);
            for b in [0, npc_n as i8] {
                w.write_i8(b);
            }
            for n in &s.npcs[..npc_n] {
                w.write_i8(n.meta.0);
                for v in [n.loc.x, n.loc.y] {
                    w.write_i16(v);
                }
                w.write_i8(n.meta.1);
                w.write_i16(n.meta.2);
            }
            let gi_n = s.ground_items.len().min(i8::MAX as usize);
            w.write_i8(gi_n as i8);
            for it in &s.ground_items[..gi_n] {
                put_ground(w, it, None);
            }
            put_layer(w, &s.raw_layers.0);
            put_layer(w, &s.raw_layers.1);
            for b in [s.header.4, s.header.5, i8::from(raw_map == 148)] {
                w.write_i8(b);
            }
            Ok(())
        },
    )
}

/// Builds empty opcode `-22` (`MAP_CLEAR`).
#[must_use]
pub fn map_clear() -> Packet {
    Packet::new(cmd_message::MAP_CLEAR, Bytes::new())
}

/// Builds opcode `29` (`OPEN_UI_ZONE`) with `<5 / <8 / >=8` load colors.
pub fn open_zone_ui(rows: &[ZoneUiRow]) -> Result<Packet, PacketError> {
    let n = rows.len().min(127);
    build_pkt_res(cmd_message::OPEN_UI_ZONE, 1 + n * 8, |w| {
        w.write_i8(n as i8);
        for r in &rows[..n] {
            let st = if r.counts.0 < 5 {
                0
            } else if r.counts.0 < 8 {
                1
            } else {
                2
            };
            for b in [
                r.zone_id.0.min(127) as i8,
                st,
                r.counts.0.min(127) as i8,
                r.counts.1.min(127) as i8,
            ] {
                w.write_i8(b);
            }
            if let Some((t1, r1, t2, r2)) = &r.rank_match {
                w.write_i8(1);
                w.write_utf(t1)?;
                w.write_i32(*r1);
                w.write_utf(t2)?;
                w.write_i32(*r2);
            } else {
                w.write_i8(0);
            }
        }
        Ok(())
    })
}

/// Builds opcode `-91` (`MAP_TRASPORT`), fixing the `ChangeMapService.java:80` fallthrough bug.
pub fn open_transport_tab(
    pri: &[(&str, &str)],
    bb: &[(&str, &str)],
    capsule: bool,
    parity: ParityMode,
) -> Result<Packet, PacketError> {
    let pri_n = pri.len().min(127);
    build_pkt_res(cmd_message::MAP_TRASPORT, 64, |w| {
        w.write_i8(pri_n as i8);
        for (m, p) in &pri[..pri_n] {
            w.write_utf(m)?;
            w.write_utf(p)?;
        }
        if capsule && matches!(parity, ParityMode::Legacy) {
            let bb_n = bb.len().min(127);
            w.write_i8(bb_n as i8);
            for (m, p) in &bb[..bb_n] {
                w.write_utf(m)?;
                w.write_utf(p)?;
            }
        }
        Ok(())
    })
}

/// Builds opcode `-65` (`TELEPORT` / `spaceShipArrive`).
#[must_use]
pub fn spaceship_arrive(player_id: PlayerId, ship_type: i8) -> Packet {
    build_pkt(cmd_message::TELEPORT, 5, |w| {
        w.write_i32(pid_i32(player_id));
        w.write_i8(ship_type);
    })
}

/// Builds opcode `-105` (`TRANSPORT`, `0` = Future 60s, `1` = Dungeon 5s).
#[must_use]
pub fn transport_countdown(max_time_sec: i16, transport_type: i8) -> Packet {
    build_pkt(cmd_message::TRANSPORT, 3, |w| {
        w.write_i16(max_time_sec);
        w.write_i8(transport_type);
    })
}
