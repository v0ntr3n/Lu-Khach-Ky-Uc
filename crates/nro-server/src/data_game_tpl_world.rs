//! Post-login world-entry S2C packet builders (`-30` subcommand `0`, `-64`, `40`, `-24`).

use nro_core::ItemInstance;
use nro_db::player_repo::PlayerSnapshot;
use nro_db::templates::{
    GameTemplates, MapTemplateRecord, SubTaskTemplateRecord, TaskMainTemplateRecord,
};
use nro_protocol::{Packet, PacketWriter};

use crate::data_game::{DataGameAssets, put_utf};

/// Resolves gender-relative `SubTaskMain.npcId` negative sentinels (`-2..=-5`).
#[inline]
#[must_use]
pub(crate) fn resolve_task_npc_id(npc_id: i8, gender_idx: usize) -> i8 {
    let g = gender_idx.min(2);
    match npc_id {
        -2 => [0, 2, 1][g],
        -3 => [10, 11, 12][g],
        -4 => [7, 8, 9][g],
        -5 => [13, 14, 15][g],
        other => other,
    }
}

/// Resolves gender-relative `SubTaskMain.mapId` negative sentinels (`-2..=-9`).
#[inline]
#[must_use]
pub(crate) fn resolve_task_map_id(map_id: i16, gender_idx: usize) -> i16 {
    let g = gender_idx.min(2);
    match map_id {
        -2 => [21, 22, 23][g],
        -3 => [1, 8, 15][g],
        -4 => [42, 43, 44][g],
        -5 => [2, 9, 16][g],
        -6 => [24, 25, 26][g],
        -7 => [3, 11, 17][g],
        -8 => [0, 7, 14][g],
        -9 => [5, 13, 20][g],
        other => other,
    }
}

/// Substitutes planet-specific `%1..%14` placeholders without mangling multi-digit tokens (`%10+`).
#[must_use]
pub(crate) fn resolve_task_text(text: &str, gender_idx: usize) -> String {
    let g = gender_idx.min(2);
    let mut out = String::with_capacity(text.len() + 16);
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            let mut num = 0_u32;
            while j < bytes.len() && bytes[j].is_ascii_digit() && j - (i + 1) < 2 {
                num = num * 10 + u32::from(bytes[j] - b'0');
                j += 1;
            }
            let rep = match num {
                1 => Some(["Làng Aru", "Làng Mori", "Làng Kakarot"][g]),
                2 => Some(["Ông Gôhan", "Ông Moori", "Ông Paragus"][g]),
                3 => Some(["Đồi hoa cúc", "Đồi nấm tím", "Đồi hoang"][g]),
                4 => Some(["Khủng long", "Lợn lòi", "Quỷ đất"][g]),
                5 => Some(["Khủng long mẹ", "Lợn lòi mẹ", "Quỷ đất mẹ"][g]),
                6 => Some(["Thằn lằn bay", "Phi long", "Quỷ bay"][g]),
                7 => Some(["Thằn lằn bay mẹ", "Phi long mẹ", "Quỷ bay mẹ"][g]),
                8 => Some(["Bunma", "Dende", "Appule"][g]),
                9 => Some(["Thằn lằn bay", "Phi long", "Quỷ bay"][g]),
                10 => Some(["Quy Lão Kame", "Trưởng lão Guru", "Vua Vegeta"][g]),
                11 => Some(["Đảo Kamê", "Đảo Guru", "Vách núi đen"][g]),
                12 => Some(["Ốc mượn hồn", "Ốc sên", "Heo Xayda mẹ"][g]),
                13 => Some(["Rừng nấm", "Thung lũng Maima", "Rừng nguyên sinh"][g]),
                14 => Some(["Thằn lằn mẹ", "Phi long mẹ", "Quỷ bay mẹ"][g]),
                _ => None,
            };
            if let Some(s) = rep {
                out.push_str(s);
                i = j;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// `-30` subcommand `0` (`Service.player`) + `-64` (`sendFlagBag`) + `40` (`TASK_GET`) + `-24` (`Zone.mapInfo`).
#[must_use]
pub fn pkts_client_ok(
    assets: &DataGameAssets,
    tpl: &GameTemplates,
    snap: &PlayerSnapshot,
) -> [Packet; 4] {
    let mut p = PacketWriter::with_capacity(2_048);
    p.write_i8(0);
    p.write_i32(snap.id.0 as i32);
    p.write_i8(snap.main_task.task_id);
    p.write_i8(i8::from(snap.gender));
    p.write_i16(snap.head);
    put_utf(&mut p, &snap.name);
    p.write_i8(0);
    p.write_i8(0);
    p.write_i64(snap.stats.power);
    p.write_i16(0);
    p.write_i16(0);
    p.write_i8(i8::from(snap.gender));
    let learned_skill_ids: Vec<i16> = snap
        .skills
        .iter()
        .filter(|sk| sk.point > 0)
        .filter_map(|sk| {
            tpl.skills
                .iter()
                .find(|t| i32::from(t.id) == sk.template_id)
                .and_then(|t| t.levels.iter().find(|lv| lv.point == sk.point))
                .map(|lv| lv.id)
        })
        .take(127)
        .collect();
    p.write_u8(learned_skill_ids.len() as u8);
    for skill_id in learned_skill_ids {
        p.write_i16(skill_id);
    }
    p.write_i64(snap.wallet.gold);
    p.write_i32(snap.wallet.ruby);
    p.write_i32(snap.wallet.gem);
    write_inv_slots(&mut p, &snap.items_body);
    write_inv_slots(&mut p, &snap.items_bag);
    write_inv_slots(&mut p, &snap.items_box);
    let head_av_len = tpl.head_avatars.len().min(i16::MAX as usize);
    p.write_i16(head_av_len as i16);
    for &(hid, aid) in tpl.head_avatars.iter().take(head_av_len) {
        p.write_i16(hid);
        p.write_i16(aid);
    }
    p.write_i16(514);
    p.write_i16(515);
    p.write_i16(537);
    p.write_i8(0);
    p.write_i32((crate::runtime::unix_now_ms() / 1_000) as i32);
    p.write_i8(0);
    p.write_i16(-1);
    p.write_i8(-1);
    p.write_i16(-1);

    let init_bag: i16 = snap
        .items_body
        .get(8)
        .and_then(Option::as_ref)
        .and_then(|it| tpl.items.iter().find(|t| t.id == i32::from(it.template_id.0)))
        .map_or(-1, |t| t.part as i16);
    let mut fb = PacketWriter::with_capacity(6);
    fb.write_i32(snap.id.0 as i32);
    fb.write_i16(init_bag);

    [
        p.finish(-30),
        fb.finish(-64),
        pkt_task_get(tpl, snap),
        pkt_map_info(assets, tpl, snap),
    ]
}

/// `40`: `TASK_GET` sending main task metadata, active subtask index, and completion requirements.
#[must_use]
pub fn pkt_task_get(tpl: &GameTemplates, snap: &PlayerSnapshot) -> Packet {
    let g = usize::from(u8::from(snap.gender)).min(2);
    let task_id = i32::from(snap.main_task.task_id);
    let fallback = TaskMainTemplateRecord {
        id: task_id,
        name: "Nhiệm vụ đầu tiên".into(),
        detail: "Nói chuyện với %2".into(),
        sub_tasks: vec![SubTaskTemplateRecord {
            name: "Nói chuyện với %2".into(),
            max_count: 1,
            notify: "Hãy đến gặp %2".into(),
            npc_id: -2,
            map_id: snap.location.map_id,
        }],
    };
    let t = tpl
        .tasks
        .iter()
        .find(|task| task.id == task_id && !task.sub_tasks.is_empty())
        .unwrap_or(&fallback);
    let sub_cap = t.sub_tasks.len().min(127);
    let max_sub = sub_cap.saturating_sub(1) as i8;
    let sub_index = snap.main_task.sub_index.clamp(0, max_sub);
    let mut pw = PacketWriter::with_capacity(512);
    pw.write_i16(t.id as i16);
    pw.write_i8(sub_index);
    put_utf(&mut pw, &resolve_task_text(&t.name, g));
    put_utf(&mut pw, &resolve_task_text(&t.detail, g));
    pw.write_u8(sub_cap as u8);
    for sub in t.sub_tasks.iter().take(sub_cap) {
        put_utf(&mut pw, &resolve_task_text(&sub.name, g));
        pw.write_i8(resolve_task_npc_id(sub.npc_id, g));
        pw.write_i16(resolve_task_map_id(sub.map_id.0, g));
        put_utf(&mut pw, &resolve_task_text(&sub.notify, g));
    }
    pw.write_i16(snap.main_task.count);
    for sub in t.sub_tasks.iter().take(sub_cap) {
        pw.write_i16(sub.max_count);
    }
    pw.finish(40)
}

/// `-24`: `Zone.mapInfo` with `arrival_ship = 0`.
#[must_use]
pub fn pkt_map_info(assets: &DataGameAssets, tpl: &GameTemplates, snap: &PlayerSnapshot) -> Packet {
    pkt_map_info_with_ship(assets, tpl, snap, 0)
}

/// `-24`: `Zone.mapInfo` including `map/item_bg_map_data/{id}`, `map/eff_map/{id}`, and `arrival_ship`.
#[must_use]
pub fn pkt_map_info_with_ship(
    assets: &DataGameAssets,
    tpl: &GameTemplates,
    snap: &PlayerSnapshot,
    arrival_ship: i8,
) -> Packet {
    let fallback = MapTemplateRecord {
        id: snap.location.map_id,
        name: format!("Map {}", snap.location.map_id.0),
        zones: 1,
        max_player: 15,
        map_type: 0,
        planet_id: i8::from(snap.gender),
        bg_type: 0,
        tile_id: 1,
        bg_id: 0,
        waypoints: Vec::new(),
        mobs: Vec::new(),
        npcs: Vec::new(),
    };
    let map = tpl.maps.iter().find(|m| m.id == snap.location.map_id).unwrap_or(&fallback);
    let mid = map.id.0;
    let mut m = PacketWriter::with_capacity(1_024);
    m.write_i8(mid as i8);
    m.write_i8(map.planet_id);
    m.write_i8(map.tile_id);
    m.write_i8(map.bg_id);
    m.write_i8(map.map_type);
    put_utf(&mut m, &map.name);
    m.write_i8(snap.location.zone_id.0 as i8);
    m.write_i16(snap.location.x);
    m.write_i16(snap.location.y);

    m.write_u8(map.waypoints.len().min(127) as u8);
    for wp in map.waypoints.iter().take(127) {
        m.write_i16(wp.1);
        m.write_i16(wp.2);
        m.write_i16(wp.3);
        m.write_i16(wp.4);
        m.write_bool(wp.5 != 0);
        m.write_bool(wp.6 != 0);
        put_utf(&mut m, &wp.0);
    }
    m.write_u8(map.mobs.len().min(127) as u8);
    for &(tid, lvl, hp, mx, my) in map.mobs.iter().take(127) {
        let mob_tid = i16::from(tid.cast_unsigned());
        let mob_hp = if hp > 0 {
            i64::from(hp)
        } else {
            tpl.mobs
                .iter()
                .find(|mt| mt.id == i32::from(mob_tid))
                .map_or(100, |mt| i64::from(mt.hp.max(100)))
        };
        for _ in 0..5 {
            m.write_bool(false);
        }
        m.write_i16(mob_tid);
        m.write_i8(0);
        m.write_i64(mob_hp);
        m.write_i8(lvl.max(1));
        m.write_i64(mob_hp);
        m.write_i16(mx);
        m.write_i16(my);
        m.write_i8(5);
        m.write_i8(0);
        m.write_bool((73..=77).contains(&mob_tid));
    }
    m.write_i8(0);
    m.write_u8(map.npcs.len().min(127) as u8);
    for &(nid, nx, ny) in map.npcs.iter().take(127) {
        let avatar = tpl
            .npcs
            .iter()
            .find(|n| n.id == i32::from(nid))
            .and_then(|n| n.avatar)
            .unwrap_or(0);
        m.write_i8(1);
        m.write_i16(nx);
        m.write_i16(ny);
        m.write_i8(nid);
        m.write_i16(avatar as i16);
    }
    m.write_u8(0);
    match assets.read_cached(&format!("map/item_bg_map_data/{mid}")) {
        Some(bg) => m.write_bytes(&bg),
        None => m.write_i16(0),
    }
    match assets.read_cached(&format!("map/eff_map/{mid}")) {
        Some(ef) => m.write_bytes(&ef),
        None => m.write_i16(0),
    }
    m.write_i8(map.bg_type);
    m.write_i8(arrival_ship);
    m.write_i8(i8::from(mid == 148));
    m.finish(-24)
}

fn write_inv_slots(pw: &mut PacketWriter, slots: &[Option<ItemInstance>]) {
    pw.write_u8(slots.len().min(127) as u8);
    for slot in slots.iter().take(127) {
        let Some(it) = slot else {
            pw.write_i16(-1);
            continue;
        };
        pw.write_i16(it.template_id.0);
        pw.write_i32(it.quantity.min(i32::MAX as u32) as i32);
        put_utf(pw, "");
        put_utf(pw, "");
        if it.options.is_empty() {
            pw.write_u8(1);
            pw.write_i16(73);
            pw.write_i32(0);
        } else {
            let opt_len = it.options.len().min(127);
            pw.write_u8(opt_len as u8);
            for opt in it.options.iter().take(opt_len) {
                pw.write_i16(opt.id.min(i16::MAX as u16) as i16);
                pw.write_i32(opt.param);
            }
        }
    }
}
