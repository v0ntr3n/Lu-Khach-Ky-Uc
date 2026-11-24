//! Item consumption/drop (`-43`) entry dispatcher.

#[path = "router_economy_books.rs"]
mod router_economy_books;
#[path = "router_economy_use_fx.rs"]
mod router_economy_use_fx;
#[path = "router_economy_use_post.rs"]
mod router_economy_use_post;
#[path = "router_economy_use_skills.rs"]
mod router_economy_use_skills;

use nro_boss::BossStatus;
use nro_consts::const_player;
use nro_core::{GameRng, TemplateId};
use nro_db::json_columns::EggTimer;
use nro_economy::{ItemCatalog, MabuEggState, UseItemCtx, UseItemEffect, UseItemService};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{
    DbItemCatalog, ServerSessionHandler, effective_appearance, router_economy::session_buffs,
    send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
};
use crate::runtime::unix_now_ms;
use router_economy_use_fx::UseItemPostState;
use router_economy_use_post::check_pea_or_porata_precond;

const PEA_IDS: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 352, 523, 595];

impl ServerSessionHandler {
    pub(super) async fn handle_use_item(
        &self,
        session: &SessionHandle,
        sid: u64,
        action_type: i8,
        where_idx: i8,
        index: i8,
        template_id: Option<TemplateId>,
    ) -> Result<(), ProtocolError> {
        if self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| super::router_shop::is_in_trade(e.snapshot.id))
        {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        if action_type == 1 {
            return session
                .send(s2c_packets::use_item_confirm(
                    1,
                    where_idx,
                    index,
                    "Bạn có chắc chắn muốn vứt bỏ vật phẩm này?",
                ))
                .await;
        }
        if action_type == 2 {
            return self
                .handle_drop_inventory_item(session, sid, where_idx, index)
                .await;
        }
        if !matches!(action_type, -1 | 0 | 3) {
            return Ok(());
        }
        if matches!(action_type, 0 | 3) {
            match where_idx {
                0 => return self.handle_get_item_move(session, sid, 5, index).await,
                2 => return self.handle_get_item_move(session, sid, 0, index).await,
                1 if self.is_bag_equip_shortcut(sid, index) => {
                    return self.handle_get_item_move(session, sid, 4, index).await;
                }
                1 => {}
                _ => return Ok(()),
            }
        }

        let mut post = UseItemPostState::default();
        let hungry_soi_in_zone = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| (e.snapshot.location.map_id, e.snapshot.location.zone_id))
            .is_some_and(|(mid, zid)| {
                self.shared.bosses.lock().instances().values().any(|b| {
                    b.boss_type.0 == 84
                        && b.map_id == mid
                        && b.zone_id == zid
                        && b.stats.hp > 0
                        && !matches!(
                            b.lifecycle.status,
                            BossStatus::Rest | BossStatus::Die | BossStatus::LeaveMap
                        )
                })
            });
        let catalog = DbItemCatalog(&self.shared.templates);
        let packets = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) {
                    post.err_msg = Some(String::from("Không thể thao tác khi đang giao dịch!"));
                    return None;
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                while inv.body.len() < 11 {
                    inv.body.push(None);
                }
                let bag_slot = if action_type == -1 && index == -1 {
                    inv.bag.iter().position(|s| {
                        s.as_ref().is_some_and(|it| {
                            PEA_IDS.contains(&it.template_id.0)
                                || (596..=604).contains(&it.template_id.0)
                        })
                    })
                } else if index >= 0 {
                    usize::try_from(index).ok()
                } else {
                    template_id.and_then(|tid| inv.find_bag_slot(tid))
                }?;
                let used_item = inv.bag_item(bag_slot).ok()?.clone();
                let item_tid = used_item.template_id;
                let now_ms = unix_now_ms();
                let is_pea =
                    PEA_IDS.contains(&item_tid.0) || catalog.meta_of(item_tid).item_type == 6;
                let (prev_fusion, mut fallback_fusion) = (
                    entry
                        .snapshot
                        .pet
                        .as_ref()
                        .map_or(const_player::NON_FUSION, |p| p.fusion_type),
                    const_player::NON_FUSION,
                );
                if let Some(msg) = check_pea_or_porata_precond(
                    &entry.snapshot,
                    item_tid,
                    is_pea,
                    prev_fusion,
                    now_ms,
                ) {
                    post.err_msg = Some(String::from(msg));
                    return None;
                }
                let (current_day, mut last_wood_chest_day, mut last_pea_ms) = (
                    now_ms / 86_400_000,
                    entry.snapshot.ruong_go.3,
                    entry.last_pea_ms,
                );
                let mut buffs = session_buffs()
                    .get(&sid)
                    .map_or_default(|v| v.value().clone());
                let (prev_buffs, prev_stamina, prev_app) = (
                    buffs.clone(),
                    entry.snapshot.stats.stamina,
                    effective_appearance(&self.shared.templates, &entry.snapshot),
                );
                let mut pet_type = entry
                    .snapshot
                    .pet
                    .as_ref()
                    .map_or(0, |p| p.type_pet.max(0).cast_unsigned());
                let pet_unlocked_skills = entry.snapshot.pet.as_ref().map_or(0_u8, |p| {
                    (0..4_u8).fold(0_u8, |acc, i| {
                        let upgradable = p.skills.get(usize::from(i)).is_some_and(|s| {
                            let max_pt = self
                                .shared
                                .templates
                                .skills
                                .iter()
                                .find(|st| i32::from(st.id) == s.template_id)
                                .map_or(7, |st| st.max_point.max(1));
                            s.template_id >= 0 && s.point > 0 && s.point < max_pt
                        });
                        acc | (u8::from(upgradable) << i)
                    })
                });
                let mut mabu_egg = entry.snapshot.mabu_egg.map(|e| MabuEggState {
                    ready_at_ms: e.last_time_create_ms.saturating_add(e.time_done_ms),
                });
                let (gender, has_pet, pet_power, pet_hp, pet_equipped) = (
                    entry.snapshot.gender,
                    entry.snapshot.pet.is_some(),
                    entry.snapshot.pet.as_ref().map_or(0, |p| p.stats.power),
                    entry.snapshot.pet.as_ref().map_or(0, |p| p.stats.hp),
                    entry
                        .snapshot
                        .pet
                        .as_ref()
                        .is_some_and(|p| p.items_body.iter().any(Option::is_some)),
                );
                let mut rng = GameRng::from_seed(now_ms.cast_unsigned() ^ sid);
                let result = {
                    let snap = &mut entry.snapshot;
                    let fusion_mode = snap
                        .pet
                        .as_mut()
                        .map_or(&mut fallback_fusion, |p| &mut p.fusion_type);
                    let mut ctx = UseItemCtx {
                        now_ms,
                        current_day,
                        last_wood_chest_day: &mut last_wood_chest_day,
                        last_pea_ms: &mut last_pea_ms,
                        gender,
                        stats: &mut snap.stats,
                        buffs: &mut buffs,
                        fusion_mode,
                        has_pet,
                        pet_type: &mut pet_type,
                        pet_power,
                        pet_unlocked_skills,
                        zone_satellites: 0,
                        hungry_soi_in_zone,
                        mabu_egg: &mut mabu_egg,
                    };
                    UseItemService::use_bag_item_with_pet(
                        &mut inv,
                        bag_slot,
                        &mut ctx,
                        pet_hp,
                        pet_equipped,
                        &catalog,
                        &mut rng,
                    )
                };
                entry.last_pea_ms = last_pea_ms;
                entry.snapshot.ruong_go.3 = last_wood_chest_day;
                if let Some(pet) = entry.snapshot.pet.as_mut() {
                    pet.type_pet = pet_type.cast_signed();
                }
                if let Some(egg) = mabu_egg
                    && entry.snapshot.mabu_egg.is_none()
                {
                    entry.snapshot.mabu_egg = Some(EggTimer {
                        last_time_create_ms: now_ms,
                        time_done_ms: egg.ready_at_ms.saturating_sub(now_ms).max(0),
                    });
                }
                match result {
                    Ok(effect) => {
                        if let UseItemEffect::LearnedSkill(book_tid) = &effect {
                            let Some(pkt) = self.apply_skill_book(&mut entry.snapshot, *book_tid)
                            else {
                                post.err_msg =
                                    Some(String::from("Không thể học kỹ năng từ sách này!"));
                                return None;
                            };
                            post.extra_pkts.push(pkt);
                        }
                        session_buffs().insert(sid, buffs.clone());
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        let pid = entry.snapshot.id;
                        let cur_fusion = entry
                            .snapshot
                            .pet
                            .as_ref()
                            .map_or(const_player::NON_FUSION, |p| p.fusion_type);
                        if cur_fusion != prev_fusion {
                            if cur_fusion == const_player::LUONG_LONG_NHAT_THE {
                                super::router_secondary::last_fusion_enter_ms().insert(pid, now_ms);
                            } else {
                                super::router_secondary::last_fusion_enter_ms().remove(&pid);
                                if cur_fusion == const_player::NON_FUSION {
                                    super::router_secondary::record_pet_unfusion_cooldown(
                                        pid, now_ms,
                                    );
                                }
                            }
                            if let Some(pet) = entry.snapshot.pet.as_mut() {
                                pet.status = if cur_fusion != const_player::NON_FUSION {
                                    4
                                } else {
                                    0
                                };
                            }
                            crate::router_pet::recalculate_pet_in_snapshot(
                                &self.shared,
                                &mut entry.snapshot,
                            );
                            post.pet_sync_pid = Some(pid);
                        }
                        super::recalculate_session_stats(
                            sid,
                            &mut entry.snapshot,
                            self.shared.config.parity_mode,
                        );
                        if is_pea || (596..=604).contains(&item_tid.0) {
                            post.extra_pkts.extend([
                                s2c_packets::me_load_hp(entry.snapshot.stats.hp),
                                s2c_packets::me_load_mp(entry.snapshot.stats.mp),
                            ]);
                            if let Some(pp) = crate::router_pet::heal_pet_with_pea(
                                &self.shared,
                                &mut entry.snapshot,
                                &used_item,
                            ) {
                                post.extra_pkts.push(pp);
                                post.pet_sync_pid = Some(pid);
                            }
                        }
                        self.apply_use_item_effect(
                            sid,
                            &mut entry.snapshot,
                            &inv,
                            effect,
                            item_tid,
                            cur_fusion != prev_fusion,
                            &mut rng,
                            &mut post,
                        );
                        Some(self.collect_use_item_bcasts_and_base_pkts(
                            &entry,
                            item_tid,
                            now_ms,
                            &buffs,
                            &prev_buffs,
                            prev_stamina,
                            prev_app,
                            prev_fusion,
                            cur_fusion,
                            &mut post,
                        ))
                    }
                    Err(err) => {
                        post.err_msg = Some(format!("Không thể sử dụng vật phẩm: {err}"));
                        None
                    }
                }
            });
        self.finish_use_item_post(session, sid, packets, post).await
    }
}
