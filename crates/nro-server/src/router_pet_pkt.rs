//! Pet appearance resolution and `PET_INFO (-107)` wire serialization.

use nro_consts::cmd_message;
use nro_core::{Gender, ItemInstance};
use nro_db::{json_columns::PetSnapshot, player_repo::PlayerSnapshot, templates::GameTemplates};
use nro_protocol::{Packet, PacketWriter};
use nro_world::s2c_packets;

use super::{is_valid_item, is_valid_part};
use crate::runtime::SharedServerState;

const CAPTION_THRESHOLDS: [i64; 22] = [
    1_000,
    3_000,
    15_000,
    40_000,
    90_000,
    170_000,
    340_000,
    700_000,
    1_500_000,
    15_000_000,
    150_000_000,
    1_500_000_000,
    5_000_000_000,
    10_000_000_000,
    40_000_000_000,
    50_010_000_000,
    60_010_000_000,
    70_010_000_000,
    80_010_000_000,
    100_010_000_000,
    1_000_010_000_000,
    10_000_010_000_000,
];

const CAPTIONS_TD: [&str; 22] = [
    "Tân binh",
    "Tập sự",
    "Chiến binh",
    "Đội trưởng",
    "Siêu cấp 1",
    "Siêu cấp 2",
    "Siêu cấp 3",
    "Siêu cấp 4",
    "Siêu cấp 5",
    "Siêu cấp 6",
    "Siêu cấp 7",
    "Thần cấp 1",
    "Thần cấp 2",
    "Thần cấp 3",
    "Thần cấp 4",
    "Thần cấp 5",
    "Thần cấp 6",
    "Thần cấp 7",
    "Thần cấp 8",
    "Thần cấp 9",
    "Thần cấp 10",
    "Thần cấp 11",
];

const CAPTIONS_NM: [&str; 22] = [
    "Tân binh",
    "Tập sự",
    "Chiến binh",
    "Đội trưởng",
    "Siêu Namec 1",
    "Siêu Namec 2",
    "Siêu Namec 3",
    "Siêu Namec 4",
    "Siêu Namec 5",
    "Siêu Namec 6",
    "Siêu Namec 7",
    "Thần Namec 1",
    "Thần Namec 2",
    "Thần Namec 3",
    "Thần Namec 4",
    "Thần Namec 5",
    "Thần Namec 6",
    "Thần Namec 7",
    "Thần Namec 8",
    "Thần Namec 9",
    "Thần Namec 10",
    "Thần Namec 11",
];

const CAPTIONS_XD: [&str; 22] = [
    "Tân binh",
    "Tập sự",
    "Chiến binh",
    "Đội trưởng",
    "Siêu Xayda 1",
    "Siêu Xayda 2",
    "Siêu Xayda 3",
    "Siêu Xayda 4",
    "Siêu Xayda 5",
    "Siêu Xayda 6",
    "Siêu Xayda 7",
    "Thần Xayda 1",
    "Thần Xayda 2",
    "Thần Xayda 3",
    "Thần Xayda 4",
    "Thần Xayda 5",
    "Thần Xayda 6",
    "Thần Xayda 7",
    "Thần Xayda 8",
    "Thần Xayda 9",
    "Thần Xayda 10",
    "Thần Xayda 11",
];

/// Resolves validated `(head, body, leg, flag_bag)` sprite parts for `pet`.
#[must_use]
pub(crate) fn resolve_pet_appearance(
    tpl: &GameTemplates,
    pet: &PetSnapshot,
) -> (i16, i16, i16, i16) {
    let g = usize::from(u8::from(pet.gender)).min(2);
    let valid_triplet = |(h, b, l): (i16, i16, i16)| {
        is_valid_part(tpl, h) && is_valid_part(tpl, b) && is_valid_part(tpl, l)
    };
    let (mut head, mut body, mut leg) = match pet.type_pet {
        1 => (297_i16, 298_i16, 299_i16),
        2 => [(946, 947, 948), (297, 298, 299)]
            .into_iter()
            .find(|&t| valid_triplet(t))
            .unwrap_or((297, 298, 299)),
        3 => [(508, 509, 510), (297, 298, 299)]
            .into_iter()
            .find(|&t| valid_triplet(t))
            .unwrap_or((297, 298, 299)),
        4 => [(882, 883, 884), (297, 298, 299)]
            .into_iter()
            .find(|&t| valid_triplet(t))
            .unwrap_or((297, 298, 299)),
        _ if pet.stats.power >= 150_000_000 => {
            let adult = ([282, 285, 288][g], [283, 286, 289][g], [284, 287, 290][g]);
            if valid_triplet(adult) {
                adult
            } else {
                ([303, 304, 305][g], [57, 59, 57][g], [60, 60, 61][g])
            }
        }
        _ => ([303, 304, 305][g], [57, 59, 57][g], [60, 60, 61][g]),
    };

    let item_part = |slot: usize| -> Option<i16> {
        let tid = pet.items_body.get(slot)?.as_ref()?.template_id.0;
        let rec = tpl.items.iter().find(|r| r.id == i32::from(tid))?;
        let p = rec.part as i16;
        is_valid_part(tpl, p).then_some(p)
    };
    if let Some(p_body) = item_part(0) {
        body = p_body;
    }
    if let Some(p_leg) = item_part(1) {
        leg = p_leg;
    }
    if let Some(Some(disg)) = pet.items_body.get(5)
        && let Some(rec) = tpl
            .items
            .iter()
            .find(|r| r.id == i32::from(disg.template_id.0))
    {
        if is_valid_part(tpl, rec.head as i16) {
            head = rec.head as i16;
        }
        if is_valid_part(tpl, rec.body as i16) {
            body = rec.body as i16;
        }
        if is_valid_part(tpl, rec.leg as i16) {
            leg = rec.leg as i16;
        }
    }
    if !is_valid_part(tpl, head) {
        head = if is_valid_part(tpl, 297) {
            297
        } else {
            [303, 304, 305][g]
        };
    }
    if !is_valid_part(tpl, body) {
        body = if head == 297 && is_valid_part(tpl, 298) {
            298
        } else {
            [57, 59, 57][g]
        };
    }
    if !is_valid_part(tpl, leg) {
        leg = if head == 297 && is_valid_part(tpl, 299) {
            299
        } else {
            [58, 60, 58][g]
        };
    }
    let flag_bag = pet
        .items_body
        .get(8)
        .and_then(Option::as_ref)
        .and_then(|it| {
            tpl.items
                .iter()
                .find(|r| r.id == i32::from(it.template_id.0))
        })
        .filter(|r| r.part >= 0)
        .map_or(-1_i16, |r| r.part as i16);
    (head, body, leg, flag_bag)
}

fn pet_tier_caption(gender: Gender, power: i64) -> &'static str {
    let table = match gender {
        Gender::TraiDat => &CAPTIONS_TD,
        Gender::Namec => &CAPTIONS_NM,
        Gender::Xayda => &CAPTIONS_XD,
    };
    let idx = CAPTION_THRESHOLDS
        .iter()
        .rposition(|&th| power >= th)
        .unwrap_or(0)
        .min(table.len() - 1);
    table[idx]
}

/// Builds opcode `-107` (`PET_INFO` mode `2`) with validated items, parts, skills, and `hp.max(0)`.
#[must_use]
pub(crate) fn build_pet_detail_pkt(
    shared: &SharedServerState,
    snap: &PlayerSnapshot,
) -> Option<Packet> {
    build_pet_detail_pkt_from_templates(&shared.templates, snap)
}

#[must_use]
pub(crate) fn build_pet_detail_pkt_from_templates(
    templates: &GameTemplates,
    snap: &PlayerSnapshot,
) -> Option<Packet> {
    let pet = snap.pet.as_ref()?;
    let (head, _, _, _) = resolve_pet_appearance(templates, pet);
    let sanitized_body: Vec<Option<ItemInstance>> = pet
        .items_body
        .iter()
        .map(|slot| {
            slot.as_ref()
                .filter(|it| is_valid_item(templates, it.template_id.0))
                .cloned()
        })
        .collect();
    let mut w = PacketWriter::with_capacity(224);
    w.write_i8(2);
    w.write_i16(head);
    let _ = s2c_packets::write_inventory_slots(&mut w, &sanitized_body);
    for v in [
        pet.stats.hp.max(0),
        pet.stats.hp_max.max(1),
        pet.stats.mp.max(0),
        pet.stats.mp_max.max(1),
        pet.stats.dame.max(1),
    ] {
        w.write_i64(v);
    }
    let disp_name = if pet.name.is_empty() {
        "$Đệ tử"
    } else {
        pet.name.as_str()
    };
    let _ = w.write_utf(disp_name);
    let _ = w.write_utf(pet_tier_caption(pet.gender, pet.stats.power));
    for v in [pet.stats.power.max(0), pet.stats.tiem_nang.max(0)] {
        w.write_i64(v);
    }
    w.write_i8(pet.status.clamp(0, 4));
    w.write_i16(pet.stats.stamina.min(i16::MAX as u16) as i16);
    w.write_i16(pet.stats.max_stamina.clamp(1_000, i16::MAX as u16) as i16);
    w.write_i8(pet.stats.crit.min(i8::MAX as u8) as i8);
    w.write_i64(i64::from(pet.stats.def.max(0)));
    let min_sk = if matches!(pet.type_pet, 3 | 4) { 5 } else { 4 };
    let sk_n = pet.skills.len().max(min_sk).min(5);
    w.write_i8(sk_n as i8);
    for idx in 0..sk_n {
        let unlocked_lv = pet
            .skills
            .get(idx)
            .filter(|sk| sk.template_id >= 0 && sk.point > 0)
            .and_then(|sk| {
                let st = templates
                    .skills
                    .iter()
                    .find(|s| i32::from(s.id) == sk.template_id)?;
                st.levels
                    .iter()
                    .find(|l| l.point == sk.point)
                    .or_else(|| st.levels.first())
                    .map(|l| l.id)
            });
        if let Some(lv_id) = unlocked_lv {
            w.write_i16(lv_id);
        } else {
            w.write_i16(-1);
            let msg = if pet
                .skills
                .get(idx)
                .is_some_and(|sk| sk.template_id >= 0 && sk.point > 0)
            {
                "Kỹ năng chưa mở"
            } else {
                match idx {
                    1 => "Cần 150tr sức mạnh để mở",
                    2 => "Cần 1tỷ5 sức mạnh để mở",
                    3 => "Cần 20tỷ sức mạnh để mở",
                    4 => "Cần 40tỷ sức mạnh để mở",
                    _ => "Kỹ năng chưa mở",
                }
            };
            let _ = w.write_utf(msg);
        }
    }
    w.write_i8(0);
    w.write_i16(0);
    Some(w.finish(cmd_message::PET_INFO))
}
