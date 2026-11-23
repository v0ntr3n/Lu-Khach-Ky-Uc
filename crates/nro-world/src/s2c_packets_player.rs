//! Player lifecycle, chat, NPC dialog, and Shenron `S2C` [`Packet`] builders.

use bytes::Bytes;
use nro_consts::cmd_message;
use nro_core::{Gender, Location, MapId, PlayerId, ZoneId};
use nro_protocol::{Packet, PacketError};

use super::{PlayerSpawnWire, build_pkt, build_pkt_res, pid_i32, sub_command_writer};

#[inline]
const fn gender_i8(g: Gender) -> i8 {
    match g {
        Gender::TraiDat => 0,
        Gender::Namec => 1,
        Gender::Xayda => 2,
    }
}

/// Builds opcode `-5` (`PLAYER_ADD` / `Zone.infoPlayer`) using v2.5.0 64-bit HP and 16-bit bag IDs.
pub fn player_add(s: &PlayerSpawnWire<'_>, client_version: u16) -> Result<Packet, PacketError> {
    let bag: i16 = if s.flags.0 == 0 {
        -1_i16
    } else if client_version >= 220 && s.flags.0 == 83 {
        205
    } else {
        i16::from(s.flags.0)
    };
    player_add_with_bag(s, bag)
}

/// Builds opcode `-5` (`PLAYER_ADD`) with an explicit 16-bit `flag_bag` (`-1` when unequipped).
pub fn player_add_with_bag(s: &PlayerSpawnWire<'_>, bag: i16) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::PLAYER_ADD, 74 + s.name.len(), |w| {
        for id in [
            pid_i32(s.identity.0),
            s.identity.1.map_or(-1, |c| i32::from(c.0)),
        ] {
            w.write_i32(id);
        }
        let g = gender_i8(s.gender);
        for b in [s.level_and_pk.0, 0, s.level_and_pk.1, g, g] {
            w.write_i8(b);
        }
        w.write_i16(s.parts.0);
        w.write_utf(s.name)?;
        w.write_i64(s.stats.hp.max(0));
        w.write_i64(s.stats.hp_max.max(0));
        for v in [s.parts.1, s.parts.2] {
            w.write_i16(v);
        }
        w.write_i16(if bag < 0 { -1 } else { bag });
        w.write_i8(-1);
        for v in [s.loc.x, s.loc.y, 0, 0] {
            w.write_i16(v);
        }
        for b in [0, s.flags.1, i8::from(s.flags.2)] {
            w.write_i8(b);
        }
        w.write_i16(s.parts.3);
        for b in [s.flags.3, 0] {
            w.write_i8(b);
        }
        w.write_i16(s.parts.4);
        w.write_i8(s.flags.4);
        w.write_i16(s.parts.5);
        Ok(())
    })
}

/// Builds `-30` sub `14` (`PLAYER_LOAD_HP`) 64-bit peer/pet HP update (`Controller.cs:6126-6149`).
#[must_use]
pub fn player_load_hp(player_id: PlayerId, hp: i64, hp_max: i64) -> Packet {
    let mut w = sub_command_writer(cmd_message::PLAYER_LOAD_HP, 21);
    w.write_i32(pid_i32(player_id));
    w.write_i64(hp.max(0));
    w.write_i8(0);
    w.write_i64(hp_max.max(0));
    w.finish(cmd_message::SUB_COMMAND)
}

/// Builds opcode `-7` (`PLAYER_MOVE`).
#[must_use]
pub fn player_move(player_id: PlayerId, loc: Location) -> Packet {
    build_pkt(cmd_message::PLAYER_MOVE, 8, |w| {
        w.write_i32(pid_i32(player_id));
        w.write_i16(loc.x);
        w.write_i16(loc.y);
    })
}

/// Builds opcode `46` (`RESET_POINT`) when clamping coordinates inside `mapWidth - 24`.
#[must_use]
pub fn reset_point(loc: Location) -> Packet {
    build_pkt(cmd_message::RESET_POINT, 4, |w| {
        w.write_i16(loc.x);
        w.write_i16(loc.y);
    })
}

/// Builds opcode `-6` (`PLAYER_REMOVE`).
#[must_use]
pub fn player_remove(player_id: PlayerId) -> Packet {
    build_pkt(cmd_message::PLAYER_REMOVE, 4, |w| {
        w.write_i32(pid_i32(player_id));
    })
}

/// Builds self death (`ME_DIE = -17`) and zone death (`PLAYER_DIE = -8`, always 32-bit player ID).
#[must_use]
pub fn char_die_packets(
    player_id: PlayerId,
    c_pk: i8,
    loc: Location,
    c_power: i64,
) -> (Packet, Packet) {
    let pid = pid_i32(player_id);
    let sw = build_pkt(cmd_message::ME_DIE, 13, |w| {
        w.write_i8(c_pk);
        w.write_i16(loc.x);
        w.write_i16(loc.y);
        w.write_i64(c_power.max(0));
    });
    let zw = build_pkt(cmd_message::PLAYER_DIE, 9, |w| {
        w.write_i32(pid);
        w.write_i8(c_pk);
        w.write_i16(loc.x);
        w.write_i16(loc.y);
    });
    (sw, zw)
}

/// Builds self revival (`ME_LIVE = -16`) and peer revival (`-30` sub `15`) with 64-bit `hp` and `hp_max`.
#[must_use]
pub fn char_revive_packets(
    player_id: PlayerId,
    hp: i64,
    hp_max: i64,
    loc: Location,
) -> (Packet, Packet) {
    let mut zw = sub_command_writer(cmd_message::PLAYER_LOAD_LIVE, 24);
    zw.write_i32(pid_i32(player_id));
    zw.write_i64(hp.max(0));
    zw.write_i64(hp_max.max(0));
    zw.write_i16(loc.x);
    zw.write_i16(loc.y);
    (
        Packet::new(cmd_message::ME_LIVE, Bytes::new()),
        zw.finish(cmd_message::SUB_COMMAND),
    )
}

/// Builds zone chat (`44`).
pub fn chat_map(speaker_id: PlayerId, text: &str) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::CHAT_MAP, 6 + text.len(), |w| {
        w.write_i32(pid_i32(speaker_id));
        w.write_utf(text)
    })
}

/// Builds opcode `92` (`CHAT_THEGIOI_SERVER` / `ChatGlobalService.chatGlobal`).
pub fn chat_global(
    sender: PlayerId,
    name: &str,
    text: &str,
    parts: (i16, i16, i16, i16),
) -> Result<Packet, PacketError> {
    build_pkt_res(
        cmd_message::CHAT_THEGIOI_SERVER,
        32 + name.len() + text.len(),
        |w| {
            w.write_utf(name)?;
            w.write_utf(&format!("|5|{text}"))?;
            w.write_i32(pid_i32(sender));
            for v in [parts.0, -1, parts.1, parts.2, parts.3] {
                w.write_i16(v);
            }
            w.write_i8(0);
            Ok(())
        },
    )
}

/// Builds opcode `-25` (`SERVER_MESSAGE` / `Service.sendThongBao`).
pub fn thong_bao(message: &str) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::SERVER_MESSAGE, 2 + message.len(), |w| {
        w.write_utf(message)
    })
}

/// Builds opcode `32` (`OPEN_UI_CONFIRM` / `NpcService.createMenu`).
pub fn npc_open_menu(
    npc_id: i16,
    say: &str,
    opts: &[&str],
    avatar: Option<i16>,
) -> Result<Packet, PacketError> {
    let n = opts.len().min(127);
    build_pkt_res(cmd_message::OPEN_UI_CONFIRM, 16 + say.len() + n * 10, |w| {
        w.write_i16(npc_id);
        w.write_utf(say)?;
        w.write_i8(n as i8);
        for o in &opts[..n] {
            w.write_utf(o)?;
        }
        if let Some(av) = avatar {
            w.write_i16(av);
        }
        Ok(())
    })
}

/// Builds opcode `38` (`OPEN_UI_SAY` / `Service.openMenu`).
pub fn npc_open_say(npc_id: i16, say: &str, avatar: Option<i16>) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::OPEN_UI_SAY, 8 + say.len(), |w| {
        w.write_i16(npc_id);
        w.write_utf(say)?;
        if let Some(av) = avatar {
            w.write_i16(av);
        }
        Ok(())
    })
}

/// Builds opcode `-83` (`CALL_DRAGON`) appear/disappear packet.
pub fn call_dragon(
    appear: Option<(MapId, i16, ZoneId, PlayerId, Location, i8)>,
) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::CALL_DRAGON, 24, |w| {
        if let Some((map_id, bg_id, zone_id, summoner, loc, dragon_kind)) = appear {
            w.write_i8(0);
            w.write_i16(i32::from(map_id.0) as i16);
            w.write_i16(bg_id);
            w.write_i8(zone_id.0.min(127) as i8);
            w.write_i32(pid_i32(summoner));
            w.write_utf("")?;
            w.write_i16(loc.x);
            w.write_i16(loc.y);
            w.write_i8(dragon_kind);
        } else {
            w.write_i8(1);
        }
        Ok(())
    })
}
