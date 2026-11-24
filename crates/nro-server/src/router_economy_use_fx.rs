//! Side-effect execution (`UseItemEffect`) for [`super`].

use nro_consts::const_npc;
use nro_core::{ClanId, GameRng, Gender, Location, MapId, PlayerId, TemplateId};
use nro_db::player_repo::PlayerSnapshot;
use nro_economy::{Inventory, UseItemEffect};
use nro_protocol::Packet;
use nro_world::s2c_packets;

use super::super::{
    ServerSessionHandler, capsule_dests, con_meo_state, is_own_home_map, npc_menu_index,
    recalculate_session_stats,
};
use crate::runtime::LeaderboardSlot;

#[derive(Default)]
pub(super) struct UseItemPostState {
    pub(super) teleport_target: Option<MapId>,
    pub(super) died_info: Option<(PlayerId, Location, i64)>,
    pub(super) extra_pkts: Vec<Packet>,
    pub(super) zone_bcasts: Option<(Location, Vec<Packet>)>,
    pub(super) err_msg: Option<String>,
    pub(super) power_gained: bool,
    pub(super) satellite_drop: Option<(TemplateId, Location, PlayerId, ClanId)>,
    pub(super) pet_sync_pid: Option<PlayerId>,
    pub(super) refresh_home_egg: bool,
}

impl ServerSessionHandler {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_use_item_effect(
        &self,
        sid: u64,
        snap: &mut PlayerSnapshot,
        inv: &Inventory,
        effect: UseItemEffect,
        item_tid: TemplateId,
        fusion_changed: bool,
        rng: &mut GameRng,
        post: &mut UseItemPostState,
    ) {
        let pid = snap.id;
        match effect {
            UseItemEffect::Applied => {
                if item_tid.0 == 568 {
                    if let Ok(tb) = s2c_packets::thong_bao("Đã đặt Trứng Mabư tại nhà!")
                    {
                        post.extra_pkts.push(tb);
                    }
                    if is_own_home_map(snap.location.map_id, snap.gender) {
                        post.refresh_home_egg = true;
                        post.pet_sync_pid = Some(pid);
                    }
                }
                if item_tid.0 == 1795 {
                    if let Some(pet) = snap.pet.as_mut() {
                        crate::router_pet::default_pet_name(pet.type_pet).clone_into(&mut pet.name);
                    }
                    crate::router_pet::recalculate_pet_in_snapshot(&self.shared, snap);
                    recalculate_session_stats(sid, snap, self.shared.config.parity_mode);
                    if let Some(pp) = self.pkt_pet_detail(snap) {
                        post.extra_pkts.push(pp);
                    }
                    post.pet_sync_pid = Some(pid);
                }
                if fusion_changed && let Some(pp) = self.pkt_pet_detail(snap) {
                    post.extra_pkts.push(pp);
                }
            }
            UseItemEffect::RerolledNormalPet => {
                let mut fresh = crate::router_pet::make_fresh_pet(
                    0,
                    Gender::from_i8_lossy((rng.next_u64() % 3) as i8),
                );
                if let Some(prev) = snap.pet.take() {
                    for (i, s) in prev.items_body.into_iter().enumerate() {
                        if let Some(it) = s {
                            while fresh.items_body.len() <= i {
                                fresh.items_body.push(None);
                            }
                            fresh.items_body[i] = Some(it);
                        }
                    }
                }
                snap.pet = Some(fresh);
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, snap);
                recalculate_session_stats(sid, snap, self.shared.config.parity_mode);
                if let Some(pp) = self.pkt_pet_detail(snap) {
                    post.extra_pkts.push(pp);
                }
                if let Ok(tb) = s2c_packets::thong_bao("Đã đổi đệ tử mới thành công!")
                {
                    post.extra_pkts.push(tb);
                }
                post.pet_sync_pid = Some(pid);
            }
            UseItemEffect::UpgradedPetSkill(slot_idx) => {
                let mut new_lv = 1_i8;
                if let Some(pet) = snap.pet.as_mut()
                    && let Some(sk) = pet.skills.get_mut(usize::from(slot_idx))
                    && sk.template_id >= 0
                {
                    let max_pt = self
                        .shared
                        .templates
                        .skills
                        .iter()
                        .find(|st| i32::from(st.id) == sk.template_id)
                        .map_or(7, |st| st.max_point.max(1));
                    sk.point = (sk.point + 1).clamp(1, max_pt);
                    new_lv = sk.point;
                }
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, snap);
                if let Some(pp) = self.pkt_pet_detail(snap) {
                    post.extra_pkts.push(pp);
                }
                if let Ok(tb) = s2c_packets::thong_bao(&format!(
                    "Đã nâng kỹ năng ô {} đệ tử lên cấp {new_lv}!",
                    slot_idx + 1
                )) {
                    post.extra_pkts.push(tb);
                }
                post.pet_sync_pid = Some(pid);
            }
            UseItemEffect::SpawnedPet2(h, b, l) => {
                if h >= 0 && b >= 0 && l >= 0 {
                    post.extra_pkts.push(s2c_packets::update_body(
                        PlayerId(pid.0.saturating_neg()),
                        Some((h, b, l, 0)),
                    ));
                }
                if let Ok(tb) = s2c_packets::thong_bao("Đã triệu hồi thú cưng đi theo!") {
                    post.extra_pkts.push(tb);
                }
            }
            UseItemEffect::OpenCapsuleUi(is_194) => {
                let dests: Vec<MapId> = [
                    21 + i16::from(i8::from(snap.gender)),
                    0,
                    7,
                    14,
                    5,
                    13,
                    20,
                    24,
                    25,
                    26,
                    46,
                    52,
                    84,
                ]
                .into_iter()
                .map(MapId)
                .collect();
                let rows: Vec<(&str, &str)> = dests
                    .iter()
                    .map(|&mid| self.capsule_label_for(mid))
                    .collect();
                if let Ok(p) = s2c_packets::open_transport_tab(
                    &rows,
                    &[],
                    true,
                    self.shared.config.parity_mode,
                ) {
                    post.extra_pkts.push(p);
                }
                capsule_dests().insert(pid, (is_194, dests));
            }
            UseItemEffect::SummonDragon(is_earth) => {
                let mut ball_range = if is_earth { 14..=20 } else { 353..=359 };
                if ball_range.all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1) {
                    let npc_id = if is_earth {
                        i16::from(const_npc::RONG_THIENG)
                    } else {
                        i16::from(const_npc::RONG_THIENG_NAMEC)
                    };
                    npc_menu_index().insert(pid, (npc_id, const_npc::SHENRON_1_1));
                    if let Ok(menu) = s2c_packets::npc_open_menu(
                        npc_id,
                        "Hãy nói ra điều ước của ngươi...",
                        &[
                            "Đẹp trai nhất vũ trụ",
                            "Găng đang mang lên 1 cấp",
                            "Đổi skill 2, 3 đệ tử",
                            "Điều ước khác",
                        ],
                        None,
                    ) {
                        post.extra_pkts.push(menu);
                    }
                } else if let Ok(tb) = s2c_packets::thong_bao(
                    "Cần đủ 7 viên Ngọc Rồng (1 sao đến 7 sao) để gọi Rồng Thần!",
                ) {
                    post.extra_pkts.push(tb);
                }
            }
            UseItemEffect::DropSatellite(tid) => {
                post.satellite_drop = Some((tid, snap.location, pid, snap.clan_id));
                if let Ok(tb) = s2c_packets::thong_bao("Đã đặt vệ tinh trong khu vực!") {
                    post.extra_pkts.push(tb);
                }
            }
            UseItemEffect::PromptSkhChoice => {
                con_meo_state().insert(pid, 5551);
                if let Ok(menu) = s2c_packets::npc_open_menu(
                    i16::from(const_npc::CON_MEO),
                    "Hãy chọn món trang bị Set Kích Hoạt:",
                    &["Áo", "Quần", "Găng", "Giày", "Rada"],
                    None,
                ) {
                    post.extra_pkts.push(menu);
                }
            }
            UseItemEffect::LearnedSkill(_) => {}
            UseItemEffect::BonusTnsm(tnsm) => {
                snap.stats.power = snap.stats.power.saturating_add(tnsm);
                snap.stats.tiem_nang = snap.stats.tiem_nang.saturating_add(tnsm);
                self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                post.extra_pkts.push(s2c_packets::player_up_exp(
                    2,
                    tnsm.clamp(0, i64::from(i32::MAX)) as i32,
                ));
                post.power_gained = true;
            }
            UseItemEffect::RerolledPetSkill(slot_idx) => {
                if let Ok(tb) =
                    s2c_packets::thong_bao(&format!("Đã đổi kỹ năng ô {slot_idx} cho đệ tử!"))
                {
                    post.extra_pkts.push(tb);
                }
                if let Some(pp) = self.pkt_pet_detail(snap) {
                    post.extra_pkts.push(pp);
                }
            }
            UseItemEffect::UpdatedRadarCard(tid) => {
                self.apply_radar_card_update(sid, snap, tid, post);
            }
            UseItemEffect::TeleportMap(map_id) => {
                post.teleport_target = Some(MapId(map_id as i16));
            }
            UseItemEffect::PoisonedDeath => {
                post.died_info = Some((pid, snap.location, snap.stats.power));
            }
        }
    }
}
