//! Shared Pet (`Đệ tử`) snapshot lifecycle, stat/skill recalculation, appearance resolution,
//! `PET_INFO (-107)` serialization, home egg `-122` status, pea healing, and live map sync.

#[path = "router_pet_ops.rs"]
mod router_pet_ops;
#[path = "router_pet_pkt.rs"]
mod router_pet_pkt;

pub(crate) use router_pet_ops::{
    apply_pet_chat_command, build_home_egg_pkts, build_pet_companion, heal_pet_with_pea,
    sync_online_pet_map_state, try_sync_online_pet_map_state,
};
pub(crate) use router_pet_pkt::{build_pet_detail_pkt, resolve_pet_appearance};

use nro_core::{CoreStats, Gender, ItemInstance, ParityMode};
use nro_db::{
    json_columns::{PetSnapshot, SkillSlot},
    player_repo::PlayerSnapshot,
    templates::GameTemplates,
};
use nro_player::{
    npoint::{ActiveBuffs, NPoint, StatContext},
    pet::{FusionKind, PetKind},
    set_clothes::SetClothes,
};

use crate::runtime::SharedServerState;

pub(crate) const fn default_pet_name(type_pet: i8) -> &'static str {
    match type_pet {
        1 => "$Mabư",
        2 => "$Uub",
        3 => "$Kid Beer",
        4 => "$Kid Jiren",
        _ => "$Đệ tử",
    }
}

/// Constructs a fresh [`PetSnapshot`] for `type_pet` (`0..=4`) and `gender`.
#[must_use]
pub(crate) fn make_fresh_pet(type_pet: i8, gender: Gender) -> PetSnapshot {
    let kind = type_pet.clamp(0, 4);
    let (name, power, hpg, mpg, dameg, defg, critg): (&str, i64, i64, i64, i64, i32, u8) =
        match kind {
            1 => ("$Mabư", 1_500_000, 2_200, 2_200, 110, 25, 3),
            2 => ("$Uub", 1_500_000, 3_000, 3_000, 150, 35, 4),
            3 => ("$Kid Beer", 1_500_000, 3_200, 3_200, 165, 40, 5),
            4 => ("$Kid Jiren", 1_500_000, 3_500, 3_500, 170, 45, 5),
            _ => ("$Đệ tử", 2_000, 1_200, 1_200, 50, 15, 2),
        };
    let body_len = if matches!(kind, 2..=4) { 9 } else { 7 };
    let skill_len = if matches!(kind, 2..=4) { 5 } else { 4 };
    let first_sk = match gender {
        Gender::TraiDat => 0,
        Gender::Namec => 2,
        Gender::Xayda => 4,
    };
    let mut skills = vec![SkillSlot {
        template_id: first_sk,
        point: 1,
        last_time_use_ms: 0,
        curr_level: 0,
    }];
    skills.resize(
        skill_len,
        SkillSlot {
            template_id: -1,
            point: 0,
            last_time_use_ms: 0,
            curr_level: 0,
        },
    );
    PetSnapshot {
        type_pet: kind,
        gender,
        name: name.to_owned(),
        fusion_type: 0,
        fusion_time_left_ms: 0,
        status: 0,
        items_body: vec![None; body_len],
        skills,
        stats: CoreStats {
            limit_power: 0,
            power,
            tiem_nang: power,
            stamina: 1_000,
            max_stamina: 1_000,
            hpg,
            mpg,
            dameg,
            defg,
            critg,
            hp: hpg,
            mp: mpg,
            hp_max: hpg,
            mp_max: mpg,
            dame: dameg,
            def: defg,
            crit: critg,
            speed: 5,
            ..CoreStats::default()
        },
    }
}

pub(self) fn is_valid_item(tpl: &GameTemplates, item_id: i16) -> bool {
    item_id >= 0 && (tpl.items.is_empty() || tpl.items.iter().any(|r| r.id == i32::from(item_id)))
}

pub(self) fn is_valid_part(tpl: &GameTemplates, part_id: i16, expected_type: i8) -> bool {
    if part_id < 0 {
        return false;
    }
    if tpl.parts.is_empty() {
        return part_id < 2_000;
    }
    let found = tpl
        .parts
        .binary_search_by_key(&part_id, |&(id, _, _)| id)
        .ok()
        .map(|idx| &tpl.parts[idx])
        .or_else(|| tpl.parts.iter().find(|&&(id, _, _)| id == part_id));
    found.is_some_and(|&(_, part_type, ref pi)| {
        part_type == expected_type
            && pi
                .iter()
                .any(|&(icon_id, _, _)| icon_id > 0 && icon_id != 2955)
    })
}

pub(self) fn compute_pet_npoint(
    templates: &GameTemplates,
    parity: ParityMode,
    pet: &PetSnapshot,
) -> (NPoint, Vec<Option<ItemInstance>>) {
    let valid_body: Vec<Option<ItemInstance>> = pet
        .items_body
        .iter()
        .map(|slot| {
            slot.as_ref()
                .filter(|it| is_valid_item(templates, it.template_id.0))
                .cloned()
        })
        .collect();
    let mut npoint = NPoint::new(pet.stats);
    let kind =
        PetKind::from_repr(u8::try_from(pet.type_pet.max(0)).unwrap_or(0)).unwrap_or_default();
    let fusion_kind = if pet.fusion_type == 0 {
        FusionKind::None
    } else {
        FusionKind::from_repr(u8::try_from(pet.fusion_type).unwrap_or(0))
            .unwrap_or(FusionKind::LuongLongNhatThe)
    };
    let ctx = StatContext {
        body: &valid_body,
        radar_options: &[],
        badge_options: &[],
        bag_gear: (None, None),
        sets: SetClothes::recompute(&valid_body, None, false),
        buffs: ActiveBuffs {
            war_mults: (1, 1),
            ..Default::default()
        },
        is_human_player: false,
        self_pet_info: Some((kind, fusion_kind, pet.fusion_type != 0)),
        fused_pet_stats: None,
        form_state: (0, 0, None),
        map_env: (false, false),
        parity,
    };
    npoint.cal_point(&ctx);
    (npoint, valid_body)
}

fn pick_valid_skill_id(tpl: &GameTemplates, candidates: &[i32], seed: u64) -> i32 {
    let mut valid = Vec::with_capacity(candidates.len());
    for &cid in candidates {
        if tpl.skills.is_empty()
            || tpl
                .skills
                .iter()
                .any(|s| i32::from(s.id) == cid && !s.levels.is_empty())
        {
            valid.push(cid);
        }
    }
    let pool = if valid.is_empty() {
        candidates
    } else {
        valid.as_slice()
    };
    pool[(seed as usize) % pool.len()]
}

/// Ensures slot counts, auto-unlocks milestone skills (`150M`/`1.5B`/`20B`/`40B`), and recomputes Pet [`CoreStats`].
pub(crate) fn recalculate_pet_in_snapshot(shared: &SharedServerState, snap: &mut PlayerSnapshot) {
    recalculate_pet_with_templates(&shared.templates, shared.config.parity_mode, snap);
}

pub(crate) fn recalculate_pet_with_templates(
    templates: &GameTemplates,
    parity: ParityMode,
    snap: &mut PlayerSnapshot,
) {
    let master_seed = snap.id.0.cast_unsigned();
    let Some(pet) = snap.pet.as_mut() else {
        return;
    };
    if pet.name.trim().is_empty() {
        default_pet_name(pet.type_pet).clone_into(&mut pet.name);
    } else if !pet.name.starts_with('$') {
        pet.name = format!("${}", pet.name.trim());
    }
    let min_body = if matches!(pet.type_pet, 2..=4) { 9 } else { 7 };
    while pet.items_body.len() < min_body {
        pet.items_body.push(None);
    }
    let min_skills = if matches!(pet.type_pet, 2..=4) { 5 } else { 4 };
    while pet.skills.len() < min_skills {
        pet.skills.push(SkillSlot {
            template_id: -1,
            point: 0,
            last_time_use_ms: 0,
            curr_level: 0,
        });
    }
    if pet.skills[0].template_id < 0 || pet.skills[0].point <= 0 {
        let s0 = match pet.gender {
            Gender::TraiDat => 0,
            Gender::Namec => 2,
            Gender::Xayda => 4,
        };
        pet.skills[0] = SkillSlot {
            template_id: s0,
            point: 1,
            last_time_use_ms: 0,
            curr_level: 0,
        };
    }
    let pwr = pet.stats.power;
    let seed_base = master_seed ^ pwr.cast_unsigned() ^ u64::from(u8::from(pet.gender));
    let milestones: [(usize, i64, &[i32]); 4] = [
        (1, 150_000_000, &[1, 3, 5]),
        (2, 1_500_000_000, &[6, 7, 8]),
        (3, 20_000_000_000, &[13, 12, 19]),
        (4, 40_000_000_000, &[20, 21]),
    ];
    for (idx, req, pool) in milestones {
        if idx < pet.skills.len() && pwr >= req {
            if pet.skills[idx].template_id < 0 {
                let chosen =
                    pick_valid_skill_id(templates, pool, seed_base.wrapping_add(idx as u64));
                pet.skills[idx] = SkillSlot {
                    template_id: chosen,
                    point: 1,
                    last_time_use_ms: 0,
                    curr_level: 0,
                };
            } else if pet.skills[idx].point <= 0 {
                pet.skills[idx].point = 1;
            }
        }
    }

    let was_uninitialized = pet.stats.hp_max == 0;
    pet.stats.hpg = pet.stats.hpg.max(200);
    pet.stats.mpg = pet.stats.mpg.max(200);
    pet.stats.dameg = pet.stats.dameg.max(10);
    pet.stats.defg = pet.stats.defg.max(1);
    pet.stats.max_stamina = pet.stats.max_stamina.max(1_000);
    if was_uninitialized && pet.stats.stamina == 0 {
        pet.stats.stamina = pet.stats.max_stamina;
    } else {
        pet.stats.stamina = pet.stats.stamina.min(pet.stats.max_stamina);
    }

    let (npoint, _) = compute_pet_npoint(templates, parity, pet);
    pet.stats.hp_max = i64::from(npoint.hp_max).max(1);
    pet.stats.mp_max = i64::from(npoint.mp_max).max(1);
    pet.stats.dame = i64::from(npoint.dame).max(1);
    pet.stats.def = npoint.def;
    pet.stats.crit = u8::try_from(npoint.crit.clamp(0, i32::from(u8::MAX))).unwrap_or(0);
    pet.stats.speed = npoint.speed.max(5);
    pet.stats.dodge = u8::try_from(npoint.mods.tl_ne_don.clamp(0, 90)).unwrap_or(0);
    if was_uninitialized {
        pet.stats.hp = pet.stats.hp.clamp(1, pet.stats.hp_max);
        pet.stats.mp = pet.stats.mp.clamp(1, pet.stats.mp_max);
    } else {
        pet.stats.hp = pet.stats.hp.clamp(0, pet.stats.hp_max);
        pet.stats.mp = pet.stats.mp.clamp(0, pet.stats.mp_max);
    }
}
