//! Live Pet map companion sync, home egg `-122` status packets, pea healing, and chat commands.

use nro_consts::{cmd_message, const_player};
use nro_core::{ItemInstance, PlayerId};
use nro_db::{player_repo::PlayerSnapshot, templates::GameTemplates};
use nro_economy::inventory::get_opt;
use nro_protocol::{Packet, PacketWriter, PetChatCommand};
use nro_world::{PetCompanion, ZoneCommand};

use super::{build_pet_detail_pkt, default_pet_name, resolve_pet_appearance};
use crate::router::is_own_home_map;
use crate::runtime::SharedServerState;

const PEA_TEMPLATES: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 352, 523, 595];

#[must_use]
pub(crate) fn build_pet_companion(
    tpl: &GameTemplates,
    snap: &PlayerSnapshot,
) -> Option<PetCompanion> {
    let pet = snap.pet.as_ref()?;
    if pet.fusion_type != const_player::NON_FUSION {
        return None;
    }
    let (head, body, leg, flag_bag) = resolve_pet_appearance(tpl, pet);
    let name = if pet.name.trim().is_empty() {
        default_pet_name(pet.type_pet).to_owned()
    } else if pet.name.starts_with('$') {
        pet.name.clone()
    } else {
        format!("${}", pet.name.trim())
    };
    Some(PetCompanion {
        name,
        gender: pet.gender,
        parts: (head, body, leg),
        flag_bag,
        hp: pet.stats.hp.max(0),
        hp_max: pet.stats.hp_max.max(1),
        dame: u64::try_from(pet.stats.dame.max(15)).unwrap_or(15),
        status: pet.status,
        master_gender: snap.gender,
        loc: snap.location,
    })
}

fn snapshot_pet_sync_target(
    shared: &SharedServerState,
    pid: PlayerId,
) -> Option<(nro_core::Location, Option<PetCompanion>)> {
    shared
        .clients
        .players
        .get(&pid)
        .map(|s| *s.value())
        .and_then(|sid| {
            shared.clients.authenticated.get(&sid).map(|entry| {
                (
                    entry.snapshot.location,
                    build_pet_companion(&shared.templates, &entry.snapshot),
                )
            })
        })
}

/// Synchronizes an online player's live [`PetCompanion`] into their current [`nro_world::ZoneActor`].
pub(crate) async fn sync_online_pet_map_state(shared: &SharedServerState, pid: PlayerId) {
    let Some((loc, pet)) = snapshot_pet_sync_target(shared, pid) else {
        return;
    };
    if let Ok(zone_tx) = shared.world_router.resolve_sender(loc.map_id, loc.zone_id) {
        let _ = zone_tx
            .send(ZoneCommand::SyncPlayerPet {
                player_id: pid,
                pet,
            })
            .await;
    }
}

/// Non-blocking variant of [`sync_online_pet_map_state`] for hot `ZoneEvent` consumer paths.
pub(crate) fn try_sync_online_pet_map_state(shared: &SharedServerState, pid: PlayerId) {
    let Some((loc, pet)) = snapshot_pet_sync_target(shared, pid) else {
        return;
    };
    if let Ok(zone_tx) = shared.world_router.resolve_sender(loc.map_id, loc.zone_id) {
        let _ = zone_tx.try_send(ZoneCommand::SyncPlayerPet {
            player_id: pid,
            pet,
        });
    }
}

/// Builds `-122` (`DUAHAU` / `MABU_EGG`) packets for the player's own home map (`21 + gender`).
#[must_use]
pub(crate) fn build_home_egg_pkts(snap: &PlayerSnapshot, now_ms: i64) -> Vec<Packet> {
    if !is_own_home_map(snap.location.map_id, snap.gender) {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(egg) = snap.mabu_egg {
        let rem = (egg
            .last_time_create_ms
            .saturating_add(egg.time_done_ms)
            .saturating_sub(now_ms)
            .max(0)
            / 1_000)
            .min(i64::from(i32::MAX)) as i32;
        let mut w = PacketWriter::with_capacity(10);
        w.write_i16(50);
        w.write_i8(1);
        w.write_i16(4664);
        w.write_i8(0);
        w.write_i32(rem);
        out.push(w.finish(cmd_message::DUAHAU));
    }
    if let Some(egg) = snap.dua_hau_egg {
        let rem = (egg
            .last_time_create_ms
            .saturating_add(egg.time_done_ms)
            .saturating_sub(now_ms)
            .max(0)
            / 1_000)
            .min(i64::from(i32::MAX)) as i32;
        let stage =
            ((now_ms.saturating_sub(egg.last_time_create_ms)) / 21_600_000).clamp(0, 3) as i8;
        let mut w = PacketWriter::with_capacity(16);
        w.write_i16(51);
        w.write_i8(4);
        for img in [4669_i16, 4670, 4671, 4672] {
            w.write_i16(img);
        }
        w.write_i8(stage);
        w.write_i32(rem);
        out.push(w.finish(cmd_message::DUAHAU));
    }
    out
}

/// Heals/revives an unfused Pet alongside the master when eating a Magic Pea.
pub(crate) fn heal_pet_with_pea(
    shared: &SharedServerState,
    snap: &mut PlayerSnapshot,
    pea: &ItemInstance,
) -> Option<Packet> {
    if heal_pet_stats_with_pea(snap, pea) {
        build_pet_detail_pkt(shared, snap)
    } else {
        None
    }
}

pub(crate) fn heal_pet_stats_with_pea(snap: &mut PlayerSnapshot, pea: &ItemInstance) -> bool {
    let master_pid = snap.id;
    let Some(pet) = snap.pet.as_mut() else {
        return false;
    };
    if pet.fusion_type != const_player::NON_FUSION {
        return false;
    }
    let heal = get_opt(pea, 2)
        .map(|v| i64::from(v).saturating_mul(1_000))
        .or_else(|| get_opt(pea, 48).map(i64::from))
        .unwrap_or(500);
    let pea_lv = PEA_TEMPLATES
        .iter()
        .position(|&id| id == pea.template_id.0)
        .map_or(1_u16, |idx| (idx + 1) as u16);
    let hp_max = pet.stats.hp_max.max(1);
    let mp_max = pet.stats.mp_max.max(1);
    let max_stam = pet.stats.max_stamina.max(1_000);
    if pet.stats.hp <= 0 {
        pet.stats.hp = heal.max(1).min(hp_max);
        pet.stats.mp = heal.max(1).min(mp_max);
        crate::runtime::clear_dead_pet_timer(master_pid);
    } else {
        pet.stats.hp = pet.stats.hp.saturating_add(heal).min(hp_max);
        pet.stats.mp = pet.stats.mp.saturating_add(heal).min(mp_max);
    }
    pet.stats.stamina = pet.stats.stamina.saturating_add(100 * pea_lv).min(max_stam);
    true
}

/// Applies a spoken [`PetChatCommand`] to `snap.pet`, returning `(optional_pet_reply, detail_pkt)`.
pub(crate) fn apply_pet_chat_command(
    shared: &SharedServerState,
    snap: &mut PlayerSnapshot,
    cmd: &PetChatCommand,
) -> Option<(&'static str, Option<Packet>)> {
    let reply = apply_pet_chat_to_snapshot(snap, cmd)?;
    Some((reply, build_pet_detail_pkt(shared, snap)))
}

pub(crate) fn apply_pet_chat_to_snapshot(
    snap: &mut PlayerSnapshot,
    cmd: &PetChatCommand,
) -> Option<&'static str> {
    let pet = snap.pet.as_mut()?;
    let reply = match cmd {
        PetChatCommand::Rename { new_name } => {
            let trimmed: String = new_name
                .trim()
                .trim_start_matches(['$', '#'])
                .chars()
                .filter(|c| !c.is_control())
                .take(20)
                .collect();
            if trimmed.is_empty() {
                return None;
            }
            pet.name = format!("${trimmed}");
            "Dạ, con đã nhớ tên mới rồi sư phụ!"
        }
        PetChatCommand::Follow
        | PetChatCommand::Protect
        | PetChatCommand::Attack
        | PetChatCommand::GoHome => {
            if pet.fusion_type != const_player::NON_FUSION {
                return None;
            }
            let (st, line) = match cmd {
                PetChatCommand::Follow => (0, "Con sẽ đi theo sư phụ!"),
                PetChatCommand::Protect => (1, "Con sẽ bảo vệ sư phụ!"),
                PetChatCommand::Attack => (2, "Để con tiêu diệt bọn chúng!"),
                _ => (3, "Con về nhà nghỉ ngơi đây!"),
            };
            pet.status = st;
            line
        }
        PetChatCommand::Transform => "Graooo!",
        PetChatCommand::EquipSkillBook => return None,
    };
    Some(reply)
}

#[cfg(test)]
mod tests {
    use nro_core::{AccountId, Gender, ItemOption, MapId, ParityMode, TemplateId};
    use nro_db::{json_columns::EggTimer, player_repo::NewPlayerSeed};

    use super::*;
    use crate::router_pet::{
        is_valid_part, make_fresh_pet, recalculate_pet_with_templates,
        router_pet_pkt::build_pet_detail_pkt_from_templates,
    };

    #[test]
    fn fresh_pet_milestones_detail_egg_pea_and_chat() {
        let mut tpl = GameTemplates::default();
        let seed = NewPlayerSeed {
            account_id: AccountId(1),
            name: "Goku".into(),
            gender: Gender::TraiDat,
            hair: 0,
            test_mode: false,
            now_ms: 1_000,
        };
        let mut snap = PlayerSnapshot::from_seed(PlayerId(7), &seed);
        snap.location.map_id = MapId(21);

        let uub = make_fresh_pet(2, Gender::TraiDat);
        assert_eq!(uub.skills.len(), 5);
        let jiren = make_fresh_pet(4, Gender::Xayda);
        assert_eq!(resolve_pet_appearance(&tpl, &jiren), (876, 877, 878, -1));
        let baby = make_fresh_pet(0, Gender::Xayda);
        assert_eq!(resolve_pet_appearance(&tpl, &baby), (305, 57, 58, -1));

        let mut pet = make_fresh_pet(0, Gender::Namec);
        pet.stats.power = 20_000_000_000;
        pet.stats.hp = 0;
        snap.pet = Some(pet);
        recalculate_pet_with_templates(&tpl, ParityMode::Fixed, &mut snap);

        let p = snap.pet.as_ref().expect("pet");
        assert!(p.skills[1].template_id >= 0 && p.skills[2].template_id >= 0);
        assert!(p.skills[3].template_id >= 0);
        assert_eq!(p.stats.hp, 0);

        let pkt = build_pet_detail_pkt_from_templates(&tpl, &snap).expect("detail pkt");
        let mut r = pkt.reader();
        assert_eq!(r.read_i8().expect("mode"), 2);
        assert!(r.read_i16().expect("head") > 0);
        let body_len = r.read_u8().expect("body len");
        for _ in 0..body_len {
            assert_eq!(r.read_i16().expect("empty slot"), -1);
        }
        assert_eq!(r.read_i64().expect("dead hp preserved as 0"), 0);

        let pea = ItemInstance::with_options(
            TemplateId(352),
            1,
            vec![ItemOption {
                id: 48,
                param: 64_000,
            }],
            1_000,
        );
        assert!(heal_pet_stats_with_pea(&mut snap, &pea));
        assert!(snap.pet.as_ref().expect("pet").stats.hp > 0);

        snap.mabu_egg = Some(EggTimer {
            last_time_create_ms: 1_000,
            time_done_ms: 61_000,
        });
        assert_eq!(build_home_egg_pkts(&snap, 2_000).len(), 1);
        snap.location.map_id = MapId(22);
        assert!(build_home_egg_pkts(&snap, 2_000).is_empty());

        tpl.parts = vec![(0, 0, vec![(100, 0, 0)]), (50, 0, vec![(2955, 0, 0)])];
        assert!(is_valid_part(&tpl, 0, 0));
        assert!(!is_valid_part(&tpl, 0, 1));
        assert!(!is_valid_part(&tpl, 50, 0));
    }
}
