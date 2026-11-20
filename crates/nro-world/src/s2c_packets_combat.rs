//! Stats, inventory, combat, ground item, and social UI `S2C` [`Packet`] builders.

use nro_consts::cmd_message;
use nro_core::{CoreStats, ItemInstance, ItemOption, Location, ParityMode, PlayerId};
use nro_protocol::{Packet, PacketError, PacketWriter};

use super::{
    GroundItemWire, build_pkt, build_pkt_res, clamp_u64_to_wire_i32, clamp_u64_to_wire_i64,
    pid_i32, put_ground,
};

/// Builds opcode `-42` (`ME_LOAD_POINT` / `Service.point`) matching v2.5.0 `Controller.cs:1710-1732`.
#[must_use]
pub fn me_load_point(stats: &CoreStats) -> Packet {
    build_pkt(cmd_message::ME_LOAD_POINT, 83, |w| {
        w.write_i32(clamp_u64_to_wire_i32(stats.hpg.max(0) as u64));
        w.write_i32(clamp_u64_to_wire_i32(stats.mpg.max(0) as u64));
        w.write_i32(clamp_u64_to_wire_i32(stats.dameg.max(0) as u64));
        w.write_i64(stats.hp_max.max(0));
        w.write_i64(stats.mp_max.max(0));
        w.write_i64(stats.hp.max(0));
        w.write_i64(stats.mp.max(0));
        w.write_i8(stats.speed as i8);
        w.write_i8(20);
        w.write_i8(20);
        w.write_i8(1);
        w.write_i64(stats.dame.max(0));
        w.write_i64(i64::from(stats.def.max(0)));
        w.write_i8(stats.crit as i8);
        w.write_i64(stats.tiem_nang.max(0));
        w.write_i16(100);
        w.write_i32(stats.defg.max(0));
        w.write_i8(stats.critg as i8);
        w.write_i8(0);
        w.write_i16(0);
    })
}

/// Builds opcode `6` (`ITEM_BUY` / `Service.sendMoney`).
#[must_use]
pub fn send_money(gold: u64, gems: u64, ruby: u64) -> Packet {
    build_pkt(cmd_message::ITEM_BUY, 16, |w| {
        w.write_i64(clamp_u64_to_wire_i64(gold));
        w.write_i32(clamp_u64_to_wire_i32(gems));
        w.write_i32(clamp_u64_to_wire_i32(ruby));
    })
}

/// Serializes inventory slots (`body`/`bag`/`box`) using v2.5.0 `u8` slot count and 6 B/option (`i16` id + `i32` param).
pub fn write_inventory_slots(
    w: &mut PacketWriter,
    slots: &[Option<ItemInstance>],
) -> Result<(), PacketError> {
    let n = slots.len().min(u8::MAX as usize);
    w.write_u8(n as u8);
    let fallback = [ItemOption::new(73, 0)];
    for slot in &slots[..n] {
        if let Some(it) = slot {
            w.write_i16(it.template_id.0);
            w.write_i32(it.quantity.min(i32::MAX as u32) as i32);
            w.write_utf("")?;
            w.write_utf("")?;
            let opts: &[ItemOption] = if it.options.is_empty() {
                &fallback
            } else {
                it.options.as_slice()
            };
            let opt_n = opts.len().min(u8::MAX as usize);
            w.write_u8(opt_n as u8);
            for o in &opts[..opt_n] {
                w.write_i16(o.id.min(i16::MAX as u16) as i16);
                w.write_i32(o.param);
            }
        } else {
            w.write_i16(-1);
        }
    }
    Ok(())
}

/// Builds mob HP (`-9`) packet after an attack (v2.5.0 64-bit HP/damage).
#[must_use]
pub fn mob_hp_after_hit(mob_id: u8, remaining_hp: u64, damage: u64, is_crit: bool) -> Packet {
    build_pkt(cmd_message::MOB_HP, 19, |w| {
        w.write_u8(mob_id);
        w.write_i64(clamp_u64_to_wire_i64(remaining_hp));
        w.write_i64(clamp_u64_to_wire_i64(damage));
        w.write_bool(is_crit);
        w.write_i8(-1);
    })
}

/// Builds opcode `-12` (`NPC_DIE`) with 64-bit damage and dropped ground items.
#[must_use]
pub fn mob_die(mob_id: u8, damage: u64, is_crit: bool, drops: &[GroundItemWire]) -> Packet {
    let n = drops.len().min(127);
    build_pkt(cmd_message::NPC_DIE, 11 + n * 12, |w| {
        w.write_u8(mob_id);
        w.write_i64(clamp_u64_to_wire_i64(damage));
        w.write_bool(is_crit);
        w.write_i8(n as i8);
        for d in &drops[..n] {
            put_ground(w, d, None);
        }
    })
}

/// Builds opcode `51` (`BOSS_SKILL` / `sendMabuAttackSkill`) with 64-bit damage per hit.
#[must_use]
pub fn mabu_attack_skill(
    boss_id: PlayerId,
    skill_id: i8,
    loc: Location,
    hits: &[(PlayerId, u64)],
) -> Packet {
    let n = hits.len().min(127);
    build_pkt(cmd_message::BOSS_SKILL, 10 + n * 12, |w| {
        w.write_i32(pid_i32(boss_id));
        w.write_i8(skill_id);
        w.write_i16(loc.x);
        w.write_i16(loc.y);
        w.write_i8(n as i8);
        for &(pl, dmg) in &hits[..n] {
            w.write_i32(pid_i32(pl));
            w.write_i64(clamp_u64_to_wire_i64(dmg));
        }
    })
}

/// Builds ground item spawn (`68`) packet.
#[must_use]
pub fn item_map_add(item: &GroundItemWire, parity: ParityMode) -> Packet {
    build_pkt(cmd_message::ITEMMAP_ADD, 12, |w| {
        put_ground(w, item, matches!(parity, ParityMode::Legacy).then_some(3));
    })
}

/// Builds self pickup (`-20`, `ITEMMAP_MYPICK`) packet; writes the second `i16` quantity for currencies.
pub fn item_map_my_pick(
    id: i16,
    qty: u32,
    notice: &str,
    is_currency: bool,
) -> Result<Packet, PacketError> {
    let qty_i16 = i16::try_from(qty).unwrap_or(9_999);
    build_pkt_res(cmd_message::ITEMMAP_MYPICK, 8 + notice.len(), |w| {
        w.write_i16(id);
        w.write_utf(notice)?;
        w.write_i16(qty_i16);
        if is_currency {
            w.write_i16(qty_i16);
        }
        Ok(())
    })
}

/// Builds self pickup (`-20`) and peer pickup (`-19`) packets.
pub fn item_map_pickup_packets(
    id: i16,
    picker: PlayerId,
    qty: u32,
    notice: &str,
    is_currency: bool,
) -> Result<(Packet, Packet), PacketError> {
    let my_p = item_map_my_pick(id, qty, notice, is_currency)?;
    let peer_p = build_pkt(cmd_message::ITEMMAP_PLAYERPICK, 6, |w| {
        w.write_i16(id);
        w.write_i32(pid_i32(picker));
    });
    Ok((my_p, peer_p))
}

/// Builds opcode `-21` (`ITEMMAP_REMOVE`).
#[must_use]
pub fn item_map_remove(item_map_id: i16) -> Packet {
    build_pkt(cmd_message::ITEMMAP_REMOVE, 2, |w| {
        w.write_i16(item_map_id);
    })
}

/// Builds opcode `-41` (`UPDATE_CAPTION`) planet-specific 22-tier power captions.
#[must_use]
pub fn update_caption(gender: i8) -> Packet {
    const TRAI_DAT: [&str; 22] = [
        "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng",
        "Siêu cấp 1", "Siêu cấp 2", "Siêu cấp 3", "Siêu cấp 4", "Siêu cấp 5",
        "Siêu cấp 6", "Siêu cấp 7",
        "Thần cấp 1", "Thần cấp 2", "Thần cấp 3", "Thần cấp 4", "Thần cấp 5",
        "Thần cấp 6", "Thần cấp 7", "Thần cấp 8", "Thần cấp 9", "Thần cấp 10",
        "Thần cấp 11",
    ];
    const NAMEC: [&str; 22] = [
        "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng",
        "Siêu Namec 1", "Siêu Namec 2", "Siêu Namec 3", "Siêu Namec 4", "Siêu Namec 5",
        "Siêu Namec 6", "Siêu Namec 7",
        "Thần Namec 1", "Thần Namec 2", "Thần Namec 3", "Thần Namec 4", "Thần Namec 5",
        "Thần Namec 6", "Thần Namec 7", "Thần Namec 8", "Thần Namec 9", "Thần Namec 10",
        "Thần Namec 11",
    ];
    const XAYDA: [&str; 22] = [
        "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng",
        "Siêu Xayda 1", "Siêu Xayda 2", "Siêu Xayda 3", "Siêu Xayda 4", "Siêu Xayda 5",
        "Siêu Xayda 6", "Siêu Xayda 7",
        "Thần Xayda 1", "Thần Xayda 2", "Thần Xayda 3", "Thần Xayda 4", "Thần Xayda 5",
        "Thần Xayda 6", "Thần Xayda 7", "Thần Xayda 8", "Thần Xayda 9", "Thần Xayda 10",
        "Thần Xayda 11",
    ];
    let captions: &[&str] = match gender {
        1 => &NAMEC,
        2 => &XAYDA,
        _ => &TRAI_DAT,
    };
    build_pkt(cmd_message::UPDATE_CAPTION, 1 + captions.len() * 24, |w| {
        w.write_i8(captions.len() as i8);
        for &cap in captions {
            let _ = w.write_utf(cap);
        }
    })
}

/// Builds opcode `-113` (`CHANGE_ONSKILL`, 10-byte skill shortcut bar).
#[must_use]
pub fn change_on_skill(shortcuts: &[i8]) -> Packet {
    build_pkt(cmd_message::CHANGE_ONSKILL, 10, |w| {
        for idx in 0..10_usize {
            w.write_i8(shortcuts.get(idx).copied().unwrap_or(-1));
        }
    })
}

/// Builds opcode `1` (`REGISTER`) account registration response.
#[must_use]
pub fn register_result(ok: bool, msg: &str) -> Packet {
    build_pkt(cmd_message::REGISTER, 3 + msg.len(), |w| {
        w.write_bool(ok);
        if !ok {
            let _ = w.write_utf(msg);
        }
    })
}

/// Builds opcode `-37` (`BODY`) equipped inventory packet.
#[must_use]
pub fn inventory_body(head: i16, slots: &[Option<ItemInstance>]) -> Packet {
    build_pkt(cmd_message::BODY, 4 + slots.len() * 16, |w| {
        w.write_i8(0);
        w.write_i16(head);
        let _ = write_inventory_slots(w, slots);
    })
}

/// Builds opcode `-36` (`BAG`) backpack inventory packet.
#[must_use]
pub fn inventory_bag(slots: &[Option<ItemInstance>]) -> Packet {
    build_pkt(cmd_message::BAG, 2 + slots.len() * 16, |w| {
        w.write_i8(0);
        let _ = write_inventory_slots(w, slots);
    })
}

/// Builds opcode `-35` (`BOX`, sub `0`) storage box inventory packet.
#[must_use]
pub fn inventory_box(slots: &[Option<ItemInstance>]) -> Packet {
    build_pkt(cmd_message::BOX, 2 + slots.len() * 16, |w| {
        w.write_i8(0);
        let _ = write_inventory_slots(w, slots);
    })
}

/// Builds opcode `-35` (`BOX`, sub `1`) open storage box UI packet.
#[must_use]
pub fn inventory_box_open(is_clan: i8) -> Packet {
    build_pkt(cmd_message::BOX, 2, |w| {
        w.write_i8(1);
        w.write_i8(is_clan);
    })
}

/// Builds opcode `56` (`HAVE_ATTACK_PLAYER`) PvP damage broadcast packet.
#[must_use]
pub fn have_attack_player(
    target_id: PlayerId,
    hp_after: i64,
    damage: i64,
    is_crit: bool,
    eff: i8,
) -> Packet {
    build_pkt(cmd_message::HAVE_ATTACK_PLAYER, 22, |w| {
        w.write_i32(pid_i32(target_id));
        w.write_i64(hp_after.max(0));
        w.write_i64(damage.max(0));
        w.write_bool(is_crit);
        w.write_i8(eff);
    })
}

/// Builds opcode `-13` (`NPC_LIVE`) mob respawn packet.
#[must_use]
pub fn npc_live(mob_id: u8, sys: i8, level_boss: i8, hp: i64) -> Packet {
    build_pkt(cmd_message::NPC_LIVE, 11, |w| {
        w.write_u8(mob_id);
        w.write_i8(sys);
        w.write_i8(level_boss);
        w.write_i64(hp.max(0));
    })
}

/// Builds opcode `-81` (`COMBINNE`, sub `0`) open item combine UI packet.
#[must_use]
pub fn combine_open(info: &str, top_info: &str) -> Packet {
    build_pkt(cmd_message::COMBINNE, 5 + info.len() + top_info.len(), |w| {
        w.write_i8(0);
        let _ = w.write_utf(info);
        let _ = w.write_utf(top_info);
    })
}

/// Builds opcode `-81` (`COMBINNE`) combine result/effect action packet.
#[must_use]
pub fn combine_result(action: i8) -> Packet {
    build_pkt(cmd_message::COMBINNE, 1, |w| {
        w.write_i8(action);
    })
}

/// Builds opcode `-86` (`GIAO_DICH`) trade invitation/notification packet.
#[must_use]
pub fn trade_invite_notify(action: i8, player_id: PlayerId) -> Packet {
    build_pkt(cmd_message::GIAO_DICH, 5, |w| {
        w.write_i8(action);
        w.write_i32(pid_i32(player_id));
    })
}
