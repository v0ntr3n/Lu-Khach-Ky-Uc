//! `GameTemplates`, `ItemData`, `FlagBagService`, `Service.player`, and `Zone.mapInfo` S2C builders.
//!
//! Serializes static template packets (`-28` subcommands `6`, `7`, `8`, `12` sub `0`, `-31`, `-62`, `-63`)
//! and post-login world-entry packets (`-30` subcommand `0`, `-64`, `-24`) directly from [`GameTemplates`]
//! and [`crate::data_game::DataGameAssets`].

#[path = "data_game_tpl_world.rs"]
mod data_game_tpl_world;

pub use data_game_tpl_world::*;

use nro_db::templates::{GameTemplates, ItemTemplateRecord, SkillLevelRecord};
use nro_protocol::{Packet, PacketWriter};

use crate::data_game::{VS_ITEM, VS_MAP, VS_SKILL, put_utf};

/// Maximum item templates encoded in a single `-28` subcommand `8` chunk so payload stays `< 65,535` bytes.
const ITEM_CHUNK_CAP: usize = 750;

/// `-28` subcommand `6`: `DataGame.updateMap` (dense-padded `0..=max_id` so client array indices match SQL IDs 1:1).
#[must_use]
pub fn pkt_update_map(tpl: &GameTemplates) -> Packet {
    let mut pw = PacketWriter::with_capacity(8_192);
    pw.write_i8(6);
    pw.write_i8(VS_MAP);

    let map_len = tpl
        .maps
        .iter()
        .filter_map(|m| usize::try_from(m.id.0).ok())
        .max()
        .map_or(0, |max_id| (max_id + 1).min(i16::MAX as usize));
    let mut map_names = vec![""; map_len];
    for m in &tpl.maps {
        if let Ok(idx) = usize::try_from(m.id.0)
            && let Some(slot) = map_names.get_mut(idx)
        {
            *slot = m.name.as_str();
        }
    }
    pw.write_i16(map_names.len() as i16);
    for name in map_names {
        put_utf(&mut pw, name);
    }

    let npc_len = tpl
        .npcs
        .iter()
        .filter_map(|n| usize::try_from(n.id).ok())
        .max()
        .map_or(0, |max_id| max_id.min(126) + 1);
    let mut npc_slots: Vec<(&str, i16, i16, i16)> = vec![("", 0, 0, 0); npc_len];
    for n in &tpl.npcs {
        if let Ok(idx) = usize::try_from(n.id)
            && let Some(slot) = npc_slots.get_mut(idx)
        {
            *slot = (n.name.as_str(), n.head as i16, n.body as i16, n.leg as i16);
        }
    }
    pw.write_i8(npc_slots.len() as i8);
    for (name, head, body, leg) in npc_slots {
        put_utf(&mut pw, name);
        pw.write_i16(head);
        pw.write_i16(body);
        pw.write_i16(leg);
        pw.write_i8(0);
    }

    let mob_len = tpl
        .mobs
        .iter()
        .filter_map(|m| usize::try_from(m.id).ok())
        .max()
        .map_or(0, |max_id| (max_id + 1).min(i16::MAX as usize));
    let mut mob_slots: Vec<(i8, &str, i64, i8, i8, i8)> = vec![(0, "", 0, 0, 0, 0); mob_len];
    for mob in &tpl.mobs {
        if let Ok(idx) = usize::try_from(mob.id)
            && let Some(slot) = mob_slots.get_mut(idx)
        {
            *slot = (
                mob.mob_type as i8,
                mob.name.as_str(),
                i64::from(mob.hp),
                mob.range_move as i8,
                mob.speed as i8,
                mob.dart_type as i8,
            );
        }
    }
    pw.write_i16(mob_slots.len() as i16);
    for (mtype, name, hp, range_move, speed, dart_type) in mob_slots {
        pw.write_i8(mtype);
        put_utf(&mut pw, name);
        pw.write_i64(hp);
        pw.write_i8(range_move);
        pw.write_i8(speed);
        pw.write_i8(dart_type);
    }
    pw.finish(-28)
}

/// `-28` subcommand `7`: `DataGame.updateSkill` (including `105..=106` padding on `id == 0`).
#[must_use]
pub fn pkt_update_skill(tpl: &GameTemplates) -> Packet {
    let mut pw = PacketWriter::with_capacity(8_192);
    pw.write_i8(7);
    pw.write_i8(VS_SKILL);
    pw.write_i8(0);
    let classes = [(0_i8, "Trái Đất"), (1_i8, "Namếc"), (2_i8, "Xayda")];
    pw.write_u8(classes.len() as u8);
    for (cid, cname) in classes {
        put_utf(&mut pw, cname);
        let group: Vec<_> = tpl.skills.iter().filter(|s| s.nclass_id == cid).collect();
        let group_cap = group.len().min(127);
        pw.write_u8(group_cap as u8);
        for st in group.into_iter().take(group_cap) {
            pw.write_i8(st.id);
            put_utf(&mut pw, &st.name);
            pw.write_i8(st.max_point);
            pw.write_i8(st.mana_use_type);
            pw.write_i8(st.skill_type);
            pw.write_i16(st.icon_id);
            put_utf(&mut pw, &st.dam_info);
            put_utf(&mut pw, "null");
            let extra = if st.id == 0 { 2 } else { 0 };
            let total_levels = (st.levels.len() + extra).min(127);
            let base_cap = total_levels.saturating_sub(extra);
            pw.write_u8(total_levels as u8);
            for lv in st.levels.iter().take(base_cap) {
                write_skill_level(&mut pw, lv);
            }
            if st.id == 0 {
                for dummy_id in (105_i16..=106).take(total_levels - base_cap) {
                    write_skill_level(
                        &mut pw,
                        &SkillLevelRecord {
                            id: dummy_id,
                            point: 0,
                            power_require: 0,
                            mana_use: 0,
                            cool_down: 0,
                            dx: 0,
                            dy: 0,
                            max_fight: 0,
                            damage: 0,
                            price: 0,
                            info: String::new(),
                        },
                    );
                }
            }
        }
    }
    pw.finish(-28)
}

/// `-28` subcommand `8` & `12` subcommand `0`: `ItemData.updateItem`
/// (`0` dense-padded options `0..=max_opt_id`, `100` head frames, and dense-padded items
/// `0..=max_item_id` via `-28` sub `8` type `1` when `<= 750` or 24-bit opcode `12` sub `0`).
#[must_use]
pub fn pkts_update_item(tpl: &GameTemplates) -> Vec<Packet> {
    let mut out = Vec::with_capacity(3);

    let opt_len = tpl
        .item_options
        .iter()
        .filter_map(|(id, _)| usize::try_from(*id).ok())
        .max()
        .map_or(0, |max_id| (max_id + 1).min(i16::MAX as usize));
    let mut opt_names = vec![""; opt_len];
    for (id, name) in &tpl.item_options {
        if let Ok(idx) = usize::try_from(*id)
            && let Some(slot) = opt_names.get_mut(idx)
        {
            *slot = name.as_str();
        }
    }
    let mut opt_w = PacketWriter::with_capacity(2_048);
    opt_w.write_i8(8);
    opt_w.write_i8(VS_ITEM);
    opt_w.write_i8(0);
    opt_w.write_i16(opt_names.len() as i16);
    for name in opt_names {
        put_utf(&mut opt_w, name);
        opt_w.write_i8(0);
    }
    opt_w.write_i16(0);
    out.push(opt_w.finish(-28));

    let mut arr_w = PacketWriter::with_capacity(1_024);
    arr_w.write_i8(8);
    arr_w.write_i8(VS_ITEM);
    arr_w.write_i8(100);
    let head_count = tpl.arr_head_2_frames.len().min(i16::MAX as usize);
    arr_w.write_i16(head_count as i16);
    for frames in tpl.arr_head_2_frames.iter().take(head_count) {
        let f_cap = frames.len().min(127);
        arr_w.write_u8(f_cap as u8);
        for &f in frames.iter().take(f_cap) {
            arr_w.write_i16(f);
        }
    }
    out.push(arr_w.finish(-28));

    let item_len = tpl
        .items
        .iter()
        .filter_map(|it| usize::try_from(it.id).ok())
        .max()
        .map_or(0, |max_id| (max_id + 1).min(i16::MAX as usize));
    let mut dense_items: Vec<Option<&ItemTemplateRecord>> = vec![None; item_len];
    for it in &tpl.items {
        if let Ok(idx) = usize::try_from(it.id)
            && let Some(slot) = dense_items.get_mut(idx)
        {
            *slot = Some(it);
        }
    }

    if dense_items.len() <= ITEM_CHUNK_CAP {
        let mut p1 = PacketWriter::with_capacity(32_768);
        p1.write_i8(8);
        p1.write_i8(VS_ITEM);
        p1.write_i8(1);
        p1.write_i16(dense_items.len() as i16);
        write_dense_items(&mut p1, &dense_items);
        out.push(p1.finish(-28));
    } else {
        let count = dense_items.len();
        let mut p_big = PacketWriter::with_capacity(64 + count * 64);
        p_big.write_i8(0);
        p_big.write_i8(VS_ITEM);
        p_big.write_i8(1);
        p_big.write_i16(count as i16);
        write_dense_items(&mut p_big, &dense_items);
        out.push(p_big.finish(12));
    }

    out
}

/// `-31`: `DataGame.sendDataItemBG` (dense-padded `0..=max_bg_id` so `BgItem.id` never scrambles).
#[must_use]
pub fn pkt_data_item_bg(tpl: &GameTemplates) -> Packet {
    let bg_len = tpl
        .bg_items
        .iter()
        .filter_map(|b| usize::try_from(b.0).ok())
        .max()
        .map_or(0, |max_id| (max_id + 1).min(i16::MAX as usize));
    let mut dense: Vec<(i16, i8, i16, i16)> = vec![(0, 0, 0, 0); bg_len];
    for &(id, image_id, layer, dx, dy) in &tpl.bg_items {
        if let Ok(idx) = usize::try_from(id)
            && let Some(slot) = dense.get_mut(idx)
        {
            *slot = (image_id, layer, dx, dy);
        }
    }
    let mut pw = PacketWriter::with_capacity(2 + dense.len() * 8);
    pw.write_i16(dense.len() as i16);
    for (image_id, layer, dx, dy) in dense {
        pw.write_i16(image_id);
        pw.write_i8(layer);
        pw.write_i16(dx);
        pw.write_i16(dy);
        pw.write_i8(0);
    }
    pw.finish(-31)
}

/// `-62` (`include_base_icon = true`) & `-63` (`include_base_icon = false`): `FlagBagService`.
#[must_use]
pub fn pkt_flag_bag(tpl: &GameTemplates, flag_id: i32, include_base_icon: bool) -> Option<Packet> {
    let &(id, _, _, _, icon_id, ref effects) = tpl.flag_bags.iter().find(|f| f.0 == flag_id)?;
    let mut pw = PacketWriter::with_capacity(5 + effects.len() * 2);
    if include_base_icon {
        let total = (effects.len() + 1).min(127);
        let eff_cap = total.saturating_sub(1);
        pw.write_u8(id as u8);
        pw.write_u8(total as u8);
        pw.write_i16(icon_id);
        for &eff in effects.iter().take(eff_cap) {
            pw.write_i16(eff);
        }
        Some(pw.finish(-62))
    } else {
        let eff_cap = effects.len().min(127);
        pw.write_i16(id as i16);
        pw.write_u8(eff_cap as u8);
        for &eff in effects.iter().take(eff_cap) {
            pw.write_i16(eff);
        }
        Some(pw.finish(-63))
    }
}

fn write_skill_level(pw: &mut PacketWriter, lv: &SkillLevelRecord) {
    pw.write_i16(lv.id);
    pw.write_i8(lv.point);
    pw.write_i64(lv.power_require);
    pw.write_i16(lv.mana_use as i16);
    pw.write_i32(lv.cool_down);
    pw.write_i16(lv.dx as i16);
    pw.write_i16(lv.dy as i16);
    pw.write_i8(lv.max_fight as i8);
    pw.write_i16(lv.damage);
    pw.write_i16(lv.price);
    put_utf(pw, &lv.info);
}

fn write_dense_items(pw: &mut PacketWriter, items: &[Option<&ItemTemplateRecord>]) {
    for slot in items {
        if let Some(it) = slot {
            pw.write_i8(it.item_type as i8);
            pw.write_i8(it.gender as i8);
            put_utf(pw, &it.name);
            put_utf(pw, it.description.as_deref().unwrap_or(""));
            pw.write_i8(it.level as i8);
            pw.write_i32(it.power_require);
            pw.write_i16(it.icon_id as i16);
            pw.write_i16(it.part as i16);
            pw.write_bool(it.is_up_to_up);
        } else {
            pw.write_i8(0);
            pw.write_i8(0);
            put_utf(pw, "");
            put_utf(pw, "");
            pw.write_i8(0);
            pw.write_i32(0);
            pw.write_i16(0);
            pw.write_i16(0);
            pw.write_bool(false);
        }
    }
}

#[cfg(test)]
mod dense_item_tests {
    use super::*;

    #[test]
    fn pkts_update_item_dense_pads_sparse_options_and_items() {
        let mut tpl = GameTemplates::default();
        tpl.item_options = vec![(0, "Tấn công+#".to_owned()), (3, "HP+#".to_owned())];
        tpl.items = vec![
            ItemTemplateRecord {
                id: 0, item_type: 0, gender: 0, name: "Áo 0".to_owned(), description: None,
                level: 1, icon_id: 10, part: 1, is_up_to_up: false, power_require: 0,
                gold: 0, gem: 0, head: -1, body: -1, leg: -1,
            },
            ItemTemplateRecord {
                id: 4, item_type: 1, gender: 2, name: "Quần 4".to_owned(), description: Some("Desc".to_owned()),
                level: 2, icon_id: 20, part: 2, is_up_to_up: true, power_require: 100,
                gold: 0, gem: 0, head: -1, body: -1, leg: -1,
            },
        ];
        let pkts = pkts_update_item(&tpl);
        let mut opt_r = pkts[0].reader();
        assert_eq!(
            (opt_r.read_i8().expect("sub"), opt_r.read_i8().expect("ver"), opt_r.read_i8().expect("type")),
            (8, VS_ITEM, 0)
        );
        assert_eq!(opt_r.read_i16().expect("opt count"), 4);
        let opts: Vec<String> = (0..4)
            .map(|_| {
                let s = opt_r.read_utf().expect("opt name");
                let _ = opt_r.read_i8().expect("opt type");
                s
            })
            .collect();
        assert_eq!(opts, vec!["Tấn công+#", "", "", "HP+#"]);

        let mut it_r = pkts[2].reader();
        assert_eq!(
            (it_r.read_i8().expect("sub"), it_r.read_i8().expect("ver"), it_r.read_i8().expect("type")),
            (8, VS_ITEM, 1)
        );
        assert_eq!(it_r.read_i16().expect("item count"), 5);
        let mut names = Vec::new();
        for _ in 0..5 {
            let _ = (it_r.read_i8().expect("item type"), it_r.read_i8().expect("gender"));
            names.push(it_r.read_utf().expect("item name"));
            let _ = it_r.read_utf().expect("desc");
            let _ = (
                it_r.read_i8().expect("level"),
                it_r.read_i32().expect("power"),
                it_r.read_i16().expect("icon"),
                it_r.read_i16().expect("part"),
                it_r.read_bool().expect("up_to_up"),
            );
        }
        assert_eq!(names, vec!["Áo 0", "", "", "", "Quần 4"]);
    }
}
