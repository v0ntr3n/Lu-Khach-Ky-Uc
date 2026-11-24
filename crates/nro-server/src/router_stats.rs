//! Shared stat recalculation, appearance resolution, `DbItemCatalog`, and inventory snapshot bridges.

use nro_consts::{cmd_message, const_player};
use nro_core::{Gender, ItemOption, MapId, ParityMode, PlayerId, TemplateId};
use nro_db::{
    json_columns::InventoryWallet, player_repo::PlayerSnapshot, templates::GameTemplates,
};
use nro_economy::{BuffSlot, Inventory, ItemCatalog, ItemMeta};
use nro_player::{
    Player,
    npoint::NPoint,
    pet::{FusionKind, Pet, PetKind, PetStatus},
    progression::RadarCard,
    skills::EffectSkillState,
};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{router_economy, router_npc, router_secondary, router_shop, router_skill};
use crate::runtime::{SharedServerState, unix_now_ms};

/// `GameTemplates`-backed [`ItemCatalog`] falling back to [`ItemMeta::classify_builtin`].
pub(super) struct DbItemCatalog<'a>(pub &'a GameTemplates);

impl ItemCatalog for DbItemCatalog<'_> {
    fn meta_of(&self, id: TemplateId) -> ItemMeta {
        let Some(rec) = self.0.items.iter().find(|r| r.id == i32::from(id.0)) else {
            return ItemMeta::classify_builtin(id);
        };
        let gender = match rec.gender {
            0 => Some(Gender::TraiDat),
            1 => Some(Gender::Namec),
            2 => Some(Gender::Xayda),
            _ => None,
        };
        let base_gold = if id.0 == 457 {
            148_000_000
        } else if rec.gold > 0 {
            u64::try_from(rec.gold).unwrap_or(4_000)
        } else {
            4_000
        };
        ItemMeta {
            template_id: id,
            item_type: u8::try_from(rec.item_type.max(0)).unwrap_or(27),
            gender,
            power_require: i64::from(rec.power_require),
            base_gold_value: base_gold,
            base_gem_value: u32::try_from(rec.gem.max(0)).unwrap_or(0),
            stackable: rec.is_up_to_up,
        }
    }
}

/// Returns `true` when `map_id` is the player's own planet house (`21 + gender`).
#[inline]
#[must_use]
pub(crate) fn is_own_home_map(map_id: MapId, gender: Gender) -> bool {
    map_id.0 == 21 + i16::from(i8::from(gender))
}

/// Builds opcode `125` (`FUSION`) packet (`[wire_mode: i8, player_id: i32]`).
#[must_use]
pub(crate) fn pkt_fusion(pid: PlayerId, wire_mode: i8) -> Packet {
    let mut w = PacketWriter::with_capacity(5);
    w.write_i8(wire_mode);
    w.write_i32(pid.0 as i32);
    w.finish(cmd_message::FUSION)
}

/// Builds sprite restoration packets for `Controller.cs:651` (`b48 = -1` alone does not restore `head/body/leg`!).
#[must_use]
pub(crate) fn sprite_restore_packets(
    pid: PlayerId,
    eh: i16,
    eb: i16,
    el: i16,
    is_disg: bool,
) -> [Option<Packet>; 2] {
    [
        Some(s2c_packets::update_body(pid, Some((eh, eb, el, 0)))),
        (!is_disg).then(|| s2c_packets::update_body(pid, None)),
    ]
}

/// Computes effective `(head, body, leg, bag, is_disguised)` sprite parts from worn equipment.
#[must_use]
pub(crate) fn effective_appearance(
    tpl: &GameTemplates,
    snap: &PlayerSnapshot,
) -> (i16, i16, i16, i16, bool) {
    let g = match snap.gender {
        Gender::TraiDat => 0_usize,
        Gender::Namec => 1,
        Gender::Xayda => 2,
    };
    let part_of = |slot: usize| -> Option<i16> {
        let tid = snap.items_body.get(slot)?.as_ref()?.template_id.0;
        let p = tpl.items.iter().find(|r| r.id == i32::from(tid))?.part;
        (p != -1).then_some(p as i16)
    };
    let bag = part_of(8).unwrap_or(-1);
    if snap.pet.as_ref().is_some_and(|p| p.fusion_type != 0) {
        let (fh, fb, fl) = match snap.gender {
            Gender::Namec => (391, 392, 393),
            Gender::TraiDat | Gender::Xayda => (380, 381, 382),
        };
        return (fh, fb, fl, bag, true);
    }
    let (mut head, mut body, mut leg, mut is_disguised) = (
        snap.head,
        part_of(0).unwrap_or([57, 59, 57][g]),
        part_of(1).unwrap_or([58, 60, 58][g]),
        false,
    );
    if let Some(Some(disg)) = snap.items_body.get(5)
        && let Some(rec) = tpl
            .items
            .iter()
            .find(|r| r.id == i32::from(disg.template_id.0))
        && (rec.head != -1 || rec.body != -1 || rec.leg != -1)
    {
        if rec.head != -1 {
            head = rec.head as i16;
        }
        if rec.body != -1 {
            body = rec.body as i16;
        }
        if rec.leg != -1 {
            leg = rec.leg as i16;
        }
        is_disguised = true;
    }
    (head, body, leg, bag, is_disguised)
}

fn recalculate_snapshot_stats_unclamped(snap: &mut PlayerSnapshot, parity: ParityMode) {
    let mut player = Player::new(
        snap.id,
        snap.account_id.unwrap_or_default(),
        snap.gender,
        snap.stats,
        snap.location,
    );
    player.inventory.items_body.clear();
    player
        .inventory
        .items_body
        .extend(snap.items_body.iter().cloned());
    player.inventory.items_bag.clone_from(&snap.items_bag);
    player.inventory.items_box.clone_from(&snap.items_box);
    player.inventory.gold = snap.wallet.gold.max(0).cast_unsigned();
    player.inventory.gem = snap.wallet.gem.max(0).cast_unsigned();
    player.inventory.ruby = snap.wallet.ruby.max(0).cast_unsigned();
    for rc in &snap.radar_cards {
        player.radar_cards.push(RadarCard {
            id: rc.id.0.max(0).cast_unsigned(),
            level: rc.level.0,
            progress: (
                rc.amount.0.max(0).cast_unsigned(),
                rc.max.0.max(0).cast_unsigned(),
            ),
            used: rc.used.0 != 0,
            aura_id: -1,
            options: rc
                .option
                .iter()
                .map(|o| {
                    (
                        ItemOption {
                            id: o.id.0.max(0) as u16,
                            param: o.param.0,
                        },
                        o.active.0,
                    )
                })
                .collect(),
        });
    }
    if let Some(ps) = &snap.pet {
        let fused = ps.fusion_type != 0;
        player.fusion.kind = if fused {
            FusionKind::from_repr(u8::try_from(ps.fusion_type).unwrap_or(0))
                .unwrap_or(FusionKind::LuongLongNhatThe)
        } else {
            FusionKind::None
        };
        let status = if fused {
            PetStatus::Fusion
        } else {
            PetStatus::from_repr(u8::try_from(ps.status.max(0)).unwrap_or(0)).unwrap_or_default()
        };
        player.pet = Some(Pet {
            id: PlayerId(snap.id.0.saturating_neg()),
            kind: PetKind::from_repr(u8::try_from(ps.type_pet.max(0)).unwrap_or(0))
                .unwrap_or_default(),
            gender: ps.gender,
            status,
            is_transform: false,
            location: snap.location,
            npoint: NPoint::new(ps.stats),
            items_body: ps.items_body.iter().cloned().collect(),
            skills: Default::default(),
            effects: EffectSkillState::default(),
            timers_ms: (0, 0, 0),
            home_dir: -1,
        });
    }
    player.recalculate_stats(false, false, unix_now_ms().cast_unsigned(), parity);
    snap.stats.hp_max = i64::from(player.npoint.hp_max);
    snap.stats.mp_max = i64::from(player.npoint.mp_max);
    snap.stats.dame = i64::from(player.npoint.dame);
    snap.stats.def = player.npoint.def;
    snap.stats.crit = u8::try_from(player.npoint.crit.clamp(0, i32::from(u8::MAX))).unwrap_or(0);
    snap.stats.speed = player.npoint.speed;
    snap.stats.dodge = u8::try_from(player.npoint.mods.tl_ne_don.clamp(0, 90)).unwrap_or(0);
    router_npc::apply_npc_buffs_before_clamp(snap.id, &mut snap.stats);
}

pub(crate) fn recalculate_snapshot_stats(snap: &mut PlayerSnapshot, parity: ParityMode) {
    recalculate_snapshot_stats_unclamped(snap, parity);
    snap.stats.hp = snap.stats.hp.clamp(0, snap.stats.hp_max);
    snap.stats.mp = snap.stats.mp.clamp(0, snap.stats.mp_max);
}

pub(crate) fn recalculate_session_stats(sid: u64, snap: &mut PlayerSnapshot, parity: ParityMode) {
    recalculate_snapshot_stats_unclamped(snap, parity);
    router_economy::apply_session_buffs_before_clamp(sid, &mut snap.stats);
    router_skill::apply_skill_buffs_before_clamp(snap.id, &mut snap.stats);
    snap.stats.hp = snap.stats.hp.clamp(0, snap.stats.hp_max);
    snap.stats.mp = snap.stats.mp.clamp(0, snap.stats.mp_max);
}

/// Unfuses a Porata-fused player if their required Porata earring was banked/removed from bag.
pub(crate) fn unfuse_if_missing_porata(
    shared: &SharedServerState,
    sid: u64,
    snap: &mut PlayerSnapshot,
    now_ms: i64,
) -> bool {
    let req_tid = match snap.pet.as_ref().map_or(0, |p| p.fusion_type) {
        const_player::HOP_THE_PORATA => Some(454_i16),
        const_player::HOP_THE_PORATA2 => Some(921_i16),
        const_player::HOP_THE_PORATA3 => Some(1819_i16),
        _ => None,
    };
    let Some(tid) = req_tid else {
        return false;
    };
    if snap
        .items_bag
        .iter()
        .flatten()
        .any(|it| it.template_id.0 == tid && it.quantity > 0)
    {
        return false;
    }
    if let Some(pet) = snap.pet.as_mut() {
        pet.fusion_type = const_player::NON_FUSION;
        pet.fusion_time_left_ms = 0;
        pet.status = 0;
    }
    router_secondary::record_pet_unfusion_cooldown(snap.id, now_ms);
    router_secondary::last_fusion_enter_ms().remove(&snap.id);
    crate::router_pet::recalculate_pet_in_snapshot(shared, snap);
    recalculate_session_stats(sid, snap, shared.config.parity_mode);
    true
}

pub(super) fn send_dialog(session: &SessionHandle, text: &str) -> Result<(), ProtocolError> {
    let mut pw = PacketWriter::with_capacity(text.len() + 4);
    pw.write_utf(text)?;
    let _ = session.try_send(pw.finish(cmd_message::DIALOG_MESSAGE));
    Ok(())
}

pub(super) fn send_thong_bao(session: &SessionHandle, text: &str) -> Result<(), ProtocolError> {
    let pkt = s2c_packets::thong_bao(text)?;
    let _ = session.try_send(pkt);
    Ok(())
}

pub(crate) fn is_player_in_trade(pid: PlayerId) -> bool {
    router_shop::is_in_trade(pid)
}

pub(crate) fn has_x2_pet_exp_buff(sid: u64, now_ms: i64) -> bool {
    router_economy::session_buffs()
        .get(&sid)
        .is_some_and(|b| b.has_active(now_ms, |s| s == BuffSlot::X2PetExp))
}

pub(crate) fn wallet_money_pkt(w: &InventoryWallet) -> Packet {
    s2c_packets::send_money(
        w.gold.max(0).cast_unsigned(),
        u64::from(w.gem.max(0).cast_unsigned()),
        u64::from(w.ruby.max(0).cast_unsigned()),
    )
}

pub(crate) fn snapshot_to_economy_inv(snap: &PlayerSnapshot) -> Inventory {
    Inventory {
        gold: snap.wallet.gold.max(0).cast_unsigned(),
        gem: snap.wallet.gem.max(0).cast_unsigned(),
        ruby: snap.wallet.ruby.max(0).cast_unsigned(),
        coupon: snap.wallet.coupon.max(0).cast_unsigned(),
        body: snap.items_body.clone(),
        bag: snap.items_bag.clone(),
        box_items: snap.items_box.clone(),
        lucky_round_box: snap.items_box_lucky_round.clone(),
    }
}

pub(crate) fn sync_economy_inv_to_snapshot(inv: &Inventory, snap: &mut PlayerSnapshot) {
    snap.wallet.gold = i64::try_from(inv.gold).unwrap_or(i64::MAX);
    snap.wallet.gem = i32::try_from(inv.gem).unwrap_or(i32::MAX);
    snap.wallet.ruby = i32::try_from(inv.ruby).unwrap_or(i32::MAX);
    snap.wallet.coupon = i32::try_from(inv.coupon).unwrap_or(i32::MAX);
    snap.items_body.clone_from(&inv.body);
    snap.items_bag.clone_from(&inv.bag);
    snap.items_box.clone_from(&inv.box_items);
    snap.items_box_lucky_round.clone_from(&inv.lucky_round_box);
}
