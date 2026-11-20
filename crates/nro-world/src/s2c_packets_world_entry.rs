//! World-entry, magic-tree, combine, and trade `S2C` [`Packet`] builders.

use nro_consts::cmd_message;
use nro_protocol::{Packet, PacketError};

use super::{build_pkt, build_pkt_res};

/// Builds opcode `-30` (`SUB_COMMAND`, sub `61`) client skill RMS sync (`Controller.cs:6035-6057`).
pub fn key_skill_rms(key: &str, data: &[u8]) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::SUB_COMMAND, 7 + key.len() + data.len(), |w| {
        w.write_i8(61);
        w.write_utf(key)?;
        w.write_i32(data.len() as i32);
        w.write_bytes(data);
        Ok(())
    })
}

/// Builds opcode `-69` (`MAXSTAMINA`).
#[must_use]
pub fn max_stamina(max_stamina: i16) -> Packet {
    build_pkt(cmd_message::MAXSTAMINA, 2, |w| {
        w.write_i16(max_stamina);
    })
}

/// Builds opcode `-68` (`STAMINA`).
#[must_use]
pub fn stamina(stamina: i16) -> Packet {
    build_pkt(cmd_message::STAMINA, 2, |w| {
        w.write_i16(stamina);
    })
}

/// Builds opcode `-97` (`UPDATE_ACTIVEPOINT`).
#[must_use]
pub fn active_point(points: i32) -> Packet {
    build_pkt(cmd_message::UPDATE_ACTIVEPOINT, 4, |w| {
        w.write_i32(points);
    })
}

/// Builds opcode `-107` (`PET_INFO` / `havePet`).
#[must_use]
pub fn have_pet(has_pet: bool) -> Packet {
    build_pkt(cmd_message::PET_INFO, 1, |w| {
        w.write_i8(i8::from(has_pet));
    })
}

/// Builds opcode `112` (`SPEACIAL_SKILL`, sub `0`) intrinsic skill info.
pub fn intrinsic_info(icon: i16, info: &str) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::SPEACIAL_SKILL, 5 + info.len(), |w| {
        w.write_i8(0);
        w.write_i16(icon);
        w.write_utf(info)?;
        Ok(())
    })
}

/// Builds opcode `-28` (`NOT_MAP`, sub `36`) account activation status.
#[must_use]
pub fn type_active(activated: bool) -> Packet {
    build_pkt(cmd_message::NOT_MAP, 2, |w| {
        w.write_i8(36);
        w.write_i8(i8::from(activated));
    })
}

/// Positive pea offsets inside `(0..treeW, 0..treeH)` for [`magic_tree_load`] (up to 23 peas).
const MAGIC_TREE_PEA_POS: [(i8, i8); 23] = [
    (18, 22), (36, 18), (52, 26), (24, 38), (46, 40),
    (14, 32), (32, 30), (58, 34), (20, 48), (40, 50),
    (56, 46), (28, 20), (44, 22), (16, 42), (34, 42),
    (50, 36), (26, 54), (44, 56), (60, 24), (22, 28),
    (38, 28), (48, 48), (30, 46),
];

/// Builds opcode `-34` (`MAGIC_TREE`, action `0`) load payload (`Controller.cs:2531-2562`).
#[allow(clippy::too_many_arguments)]
pub fn magic_tree_load(
    icon_id: i16,
    name: &str,
    x: i16,
    y: i16,
    level: i8,
    curr_peas: i16,
    max_peas: i16,
    info: &str,
    seconds: i32,
    is_upgrade: bool,
) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::MAGIC_TREE, 24 + name.len() + info.len(), |w| {
        w.write_i8(0);
        w.write_i16(icon_id);
        w.write_utf(name)?;
        w.write_i16(x);
        w.write_i16(y);
        w.write_i8(level);
        w.write_i16(curr_peas);
        w.write_i16(max_peas);
        w.write_utf(info)?;
        w.write_i32(seconds.max(0));
        let count = usize::try_from(max_peas.clamp(1, 23)).unwrap_or(1);
        w.write_i8(count as i8);
        for &(px, py) in &MAGIC_TREE_PEA_POS[..count] {
            w.write_i8(px);
            w.write_i8(py);
        }
        w.write_bool(is_upgrade);
        Ok(())
    })
}

/// Builds opcode `-34` (`MAGIC_TREE`, action `2`) harvest response (`Controller.cs:2580-2587`).
#[must_use]
pub fn magic_tree_harvest(remain_peas: i16, seconds: i32) -> Packet {
    build_pkt(cmd_message::MAGIC_TREE, 7, |w| {
        w.write_i8(2);
        w.write_i16(remain_peas.max(0));
        w.write_i32(seconds.max(0));
    })
}

/// Builds opcode `-34` (`MAGIC_TREE`, action `1`) menu payload (`Controller.cs:2563-2579`).
pub fn magic_tree_menu(options: &[&str]) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::MAGIC_TREE, 1 + options.len() * 16, |w| {
        w.write_i8(1);
        for &opt in options {
            w.write_utf(opt)?;
        }
        Ok(())
    })
}

/// Builds opcode `-81` (`COMBINNE`, action `1`) selected combine slot indices.
#[must_use]
pub fn combine_items(indices: &[i8]) -> Packet {
    let n = indices.len().min(i8::MAX as usize);
    build_pkt(cmd_message::COMBINNE, 2 + n, |w| {
        w.write_i8(1);
        w.write_i8(n as i8);
        for &idx in &indices[..n] {
            w.write_i8(idx);
        }
    })
}

/// Builds opcode `-81` (`COMBINNE`) combine result packet with icon ID.
#[must_use]
pub fn combine_result_with_icon(action: i8, icon_id: i16) -> Packet {
    build_pkt(cmd_message::COMBINNE, 3, |w| {
        w.write_i8(action);
        w.write_i16(icon_id);
    })
}

/// Builds opcode `-86` (`GIAO_DICH`, action `7`) trade close packet.
#[must_use]
pub fn trade_close() -> Packet {
    build_pkt(cmd_message::GIAO_DICH, 1, |w| {
        w.write_i8(7);
    })
}
