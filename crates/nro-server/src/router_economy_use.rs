//! Item consumption/drop (`-43`) and skill learning/upgrades.

use nro_consts::{cmd_message, const_npc, const_player};
use nro_boss::BossStatus;
use nro_core::{ClanId, GameRng, ItemInstance, Location, MapId, PlayerId, TemplateId, ZoneId};
use nro_db::{StrNum, json_columns::{EggTimer, RadarCardOption, RadarCardRecord, SkillSlot}};
use nro_economy::{ItemCatalog, MabuEggState, UseItemCtx, UseItemEffect, UseItemService};
use nro_player::skills::SkillKind;
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};

use super::{
    DbItemCatalog, ServerSessionHandler, capsule_dests, con_meo_state, effective_appearance, npc_menu_index,
    router_economy::{resolve_equip_body_slot, session_buffs},
    send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

const PEA_IDS: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 295, 523, 595];

impl ServerSessionHandler {
    pub(super) async fn handle_use_item(&self, session: &SessionHandle, sid: u64, action_type: i8, where_idx: i8, index: i8, template_id: Option<TemplateId>) -> Result<(), ProtocolError> {
        if self.shared.clients.authenticated.get(&sid).is_some_and(|e| super::router_shop::is_in_trade(e.snapshot.id)) { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
        if action_type == 1 { return session.send(s2c_packets::use_item_confirm(1, where_idx, index, "Bạn có chắc chắn muốn vứt bỏ vật phẩm này?")).await; }
        if action_type == 2 {
            let Ok(idx) = usize::try_from(index) else { return Ok(()); };
            let mut in_trade = false;
            let drop_out = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                if super::router_shop::is_in_trade(entry.snapshot.id) { in_trade = true; return None; }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let taken: ItemInstance = match where_idx {
                    0 => inv.body.get_mut(idx)?.take()?, 1 => inv.bag.get_mut(idx)?.take()?, 2 => inv.box_items.get_mut(idx)?.take()?, _ => return None,
                };
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                if where_idx == 0 { super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode); }
                let (eh, eb, el, ebag, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
                let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                if where_idx == 0 && let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                let bcast = (where_idx == 0).then(|| (s2c_packets::me_load_point(&entry.snapshot.stats), s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0))), s2c_packets::send_flag_bag(pid, ebag)));
                Some((taken, pid, loc, [s2c_packets::inventory_body(eh, &entry.snapshot.items_body), s2c_packets::inventory_bag(&entry.snapshot.items_bag), s2c_packets::inventory_box(&entry.snapshot.items_box)], bcast))
            });
            if in_trade { return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!"); }
            if let Some((dropped, pid, loc, inv_pkts, bcast)) = drop_out {
                if let Ok(tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) {
                    let _ = tx.send(ZoneCommand::DropItem(dropped.template_id, dropped.quantity, loc, Some(pid), GroundDropKind::Standard)).await;
                }
                for p in inv_pkts { session.send(p).await?; }
                if let Some((pt_pkt, upd_body, flag_bag)) = bcast {
                    session.send(pt_pkt).await?; self.broadcast_zone_or_self(loc, sid, &upd_body); self.broadcast_zone_or_self(loc, sid, &flag_bag);
                }
            }
            return Ok(());
        }
        if !matches!(action_type, -1 | 0 | 3) { return Ok(()); }
        if matches!(action_type, 0 | 3) {
            match where_idx {
                0 => return self.handle_get_item_move(session, sid, 5, index).await,
                2 => return self.handle_get_item_move(session, sid, 0, index).await,
                1 => {
                    let is_equip = usize::try_from(index).is_ok_and(|idx| {
                        let catalog = DbItemCatalog(&self.shared.templates);
                        self.shared.clients.authenticated.get(&sid).is_some_and(|entry| {
                            entry.snapshot.items_bag.get(idx).and_then(Option::as_ref).is_some_and(|it| {
                                let meta = catalog.meta_of(it.template_id);
                                let is_pet2 = nro_economy::use_item::PET2_SPRITES.iter().any(|&(pid, _)| pid == it.template_id.0);
                                let is_special = matches!(it.template_id.0, 193 | 194 | 454 | 568 | 570 | 921 | 1655 | 1795 | 1819);
                                matches!(meta.item_type, 0..=5 | 32 | 35 | 72)
                                    && resolve_equip_body_slot(meta.item_type, entry.snapshot.items_body.len().max(11)).is_some()
                                    && !is_pet2
                                    && !is_special
                            })
                        })
                    });
                    if is_equip {
                        return self.handle_get_item_move(session, sid, 4, index).await;
                    }
                }
                _ => return Ok(()),
            }
        }
        let (mut teleport_target, mut died_info, mut extra_pkts, mut zone_bcasts, mut err_msg, mut power_gained, mut satellite_drop) = (None::<MapId>, None::<(PlayerId, Location, i64)>, Vec::<Packet>::new(), None::<(Location, Vec<Packet>)>, None::<String>, false, None::<(TemplateId, Location, PlayerId, ClanId)>);
        let hungry_soi_in_zone = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.location.map_id, e.snapshot.location.zone_id)).is_some_and(|(mid, zid)| {
            self.shared.bosses.lock().instances().values().any(|b| {
                b.boss_type.0 == 84 && b.map_id == mid && b.zone_id == zid && b.stats.hp > 0
                    && !matches!(b.lifecycle.status, BossStatus::Rest | BossStatus::Die | BossStatus::LeaveMap)
            })
        });
        let catalog = DbItemCatalog(&self.shared.templates);
        let packets = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if super::router_shop::is_in_trade(entry.snapshot.id) { err_msg = Some(String::from("Không thể thao tác khi đang giao dịch!")); return None; }
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            while inv.body.len() < 11 { inv.body.push(None); }
            let bag_slot = if action_type == -1 && index == -1 {
                inv.bag.iter().position(|s| s.as_ref().is_some_and(|it| PEA_IDS.contains(&it.template_id.0) || (596..=604).contains(&it.template_id.0)))
            } else if index >= 0 { usize::try_from(index).ok() } else { template_id.and_then(|tid| inv.find_bag_slot(tid)) }?;
            let item_tid = inv.bag_item(bag_slot).ok()?.template_id;
            let now_ms = unix_now_ms();
            let (current_day, mut last_wood_chest_day, mut last_pea_ms) = (now_ms / 86_400_000, entry.snapshot.ruong_go.3, entry.last_pea_ms);
            let mut buffs = session_buffs().get(&sid).map_or_default(|v| v.value().clone());
            let (prev_buffs, prev_stamina, prev_app) = (buffs.clone(), entry.snapshot.stats.stamina, effective_appearance(&self.shared.templates, &entry.snapshot));
            let mut pet_type = entry.snapshot.pet.as_ref().map_or(0, |p| p.type_pet.max(0).cast_unsigned());
            let mut mabu_egg = entry.snapshot.mabu_egg.map(|e| MabuEggState { ready_at_ms: e.last_time_create_ms.saturating_add(e.time_done_ms) });
            let (gender, has_pet, pet_power) = (entry.snapshot.gender, entry.snapshot.pet.is_some(), entry.snapshot.pet.as_ref().map_or(0, |p| p.stats.power));
            let (prev_fusion, mut fallback_fusion) = (entry.snapshot.pet.as_ref().map_or(const_player::NON_FUSION, |p| p.fusion_type), const_player::NON_FUSION);
            let mut rng = GameRng::from_seed(now_ms.cast_unsigned() ^ sid);
            let result = {
                let snap = &mut entry.snapshot;
                let fusion_mode = snap.pet.as_mut().map_or(&mut fallback_fusion, |p| &mut p.fusion_type);
                let mut ctx = UseItemCtx { now_ms, current_day, last_wood_chest_day: &mut last_wood_chest_day, last_pea_ms: &mut last_pea_ms, gender, stats: &mut snap.stats, buffs: &mut buffs, fusion_mode, has_pet, pet_type: &mut pet_type, pet_power, pet_unlocked_skills: 0b1111, zone_satellites: 0, hungry_soi_in_zone, mabu_egg: &mut mabu_egg };
                UseItemService::use_bag_item(&mut inv, bag_slot, &mut ctx, &catalog, &mut rng)
            };
            entry.last_pea_ms = last_pea_ms;
            entry.snapshot.ruong_go.3 = last_wood_chest_day;
            if let Some(pet) = entry.snapshot.pet.as_mut() { pet.type_pet = pet_type.cast_signed(); }
            if let Some(egg) = mabu_egg && entry.snapshot.mabu_egg.is_none() {
                entry.snapshot.mabu_egg = Some(EggTimer { last_time_create_ms: now_ms, time_done_ms: egg.ready_at_ms.saturating_sub(now_ms).max(0) });
            }
            match result {
                Ok(effect) => {
                    if let UseItemEffect::LearnedSkill(book_tid) = &effect {
                        let Some(pkt) = self.apply_skill_book(&mut entry.snapshot, *book_tid) else {
                            err_msg = Some(String::from("Không thể học kỹ năng từ sách này!"));
                            return None;
                        };
                        extra_pkts.push(pkt);
                    }
                    session_buffs().insert(sid, buffs.clone());
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                    if PEA_IDS.contains(&item_tid.0) || (596..=604).contains(&item_tid.0) {
                        extra_pkts.extend([s2c_packets::me_load_hp(entry.snapshot.stats.hp), s2c_packets::me_load_mp(entry.snapshot.stats.mp)]);
                    }
                    if buffs != prev_buffs {
                        let icon_id = self.shared.templates.items.iter().find(|r| r.id == i32::from(item_tid.0)).map_or(0_i16, |r| r.icon_id as i16);
                        for &(slot, exp) in &buffs.active {
                            if exp > now_ms && !prev_buffs.active.iter().any(|&(ps, pe)| ps == slot && pe == exp) {
                                let mut w = PacketWriter::with_capacity(4);
                                w.write_i16(icon_id); w.write_i16(((exp - now_ms) / 1000).clamp(0, i64::from(i16::MAX)) as i16);
                                extra_pkts.push(w.finish(cmd_message::ITEM_TIME));
                            }
                        }
                    }
                    if entry.snapshot.stats.stamina != prev_stamina { extra_pkts.push(s2c_packets::stamina(i16::try_from(entry.snapshot.stats.stamina).unwrap_or(i16::MAX))); }
                    match effect {
                        UseItemEffect::Applied => {
                            if item_tid.0 == 568 && let Ok(tb) = s2c_packets::thong_bao("Đã đặt Trứng Mabư tại nhà!") {
                                extra_pkts.push(tb);
                            }
                            if item_tid.0 == 1795 && let Some(pp) = self.pkt_pet_detail(&entry.snapshot) {
                                extra_pkts.push(pp);
                            }
                        }
                        UseItemEffect::SpawnedPet2(h, b, l) => {
                            if h >= 0 && b >= 0 && l >= 0 {
                                extra_pkts.push(s2c_packets::update_body(PlayerId(entry.snapshot.id.0.saturating_neg()), Some((h, b, l, 0))));
                            }
                            if let Ok(tb) = s2c_packets::thong_bao("Đã triệu hồi thú cưng đi theo!") {
                                extra_pkts.push(tb);
                            }
                        }
                        UseItemEffect::OpenCapsuleUi(is_194) => {
                            let dests: Vec<MapId> = [21 + i16::from(gender), 0, 7, 14, 5, 13, 20, 24, 25, 26, 46, 52, 84].into_iter().map(MapId).collect();
                            let rows: Vec<(&str, &str)> = dests.iter().map(|&mid| self.capsule_label_for(mid)).collect();
                            if let Ok(p) = s2c_packets::open_transport_tab(&rows, &[], true, self.shared.config.parity_mode) { extra_pkts.push(p); }
                            capsule_dests().insert(entry.snapshot.id, (is_194, dests));
                        }
                        UseItemEffect::SummonDragon(is_earth) => {
                            let mut ball_range = if is_earth { 14..=20 } else { 353..=359 };
                            if ball_range.all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1) {
                                let npc_id = if is_earth { i16::from(const_npc::RONG_THIENG) } else { i16::from(const_npc::RONG_THIENG_NAMEC) };
                                npc_menu_index().insert(entry.snapshot.id, (npc_id, const_npc::SHENRON_1_1));
                                if let Ok(menu) = s2c_packets::npc_open_menu(
                                    npc_id,
                                    "Hãy nói ra điều ước của ngươi...",
                                    &["Đẹp trai nhất vũ trụ", "Găng đang mang lên 1 cấp", "Đổi skill 2, 3 đệ tử", "Điều ước khác"],
                                    None,
                                ) {
                                    extra_pkts.push(menu);
                                }
                            } else if let Ok(tb) = s2c_packets::thong_bao("Cần đủ 7 viên Ngọc Rồng (1 sao đến 7 sao) để gọi Rồng Thần!") {
                                extra_pkts.push(tb);
                            }
                        }
                        UseItemEffect::DropSatellite(tid) => {
                            satellite_drop = Some((tid, entry.snapshot.location, entry.snapshot.id, entry.snapshot.clan_id));
                            if let Ok(tb) = s2c_packets::thong_bao("Đã đặt vệ tinh trong khu vực!") { extra_pkts.push(tb); }
                        }
                        UseItemEffect::PromptSkhChoice => {
                            con_meo_state().insert(entry.snapshot.id, 5551);
                            if let Ok(menu) = s2c_packets::npc_open_menu(
                                i16::from(const_npc::CON_MEO),
                                "Hãy chọn món trang bị Set Kích Hoạt:",
                                &["Áo", "Quần", "Găng", "Giày", "Rada"],
                                None,
                            ) {
                                extra_pkts.push(menu);
                            }
                        }
                        UseItemEffect::LearnedSkill(_) => {}
                        UseItemEffect::BonusTnsm(tnsm) => {
                            entry.snapshot.stats.power = entry.snapshot.stats.power.saturating_add(tnsm);
                            entry.snapshot.stats.tiem_nang = entry.snapshot.stats.tiem_nang.saturating_add(tnsm);
                            self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                            extra_pkts.push(s2c_packets::player_up_exp(2, tnsm.clamp(0, i64::from(i32::MAX)) as i32));
                            power_gained = true;
                        }
                        UseItemEffect::RerolledPetSkill(slot_idx) => {
                            if let Ok(tb) = s2c_packets::thong_bao(&format!("Đã đổi kỹ năng ô {slot_idx} cho đệ tử!")) { extra_pkts.push(tb); }
                            if let Some(pp) = self.pkt_pet_detail(&entry.snapshot) { extra_pkts.push(pp); }
                        }
                        UseItemEffect::UpdatedRadarCard(tid) => {
                            let card_upd = if let Some(card) = entry.snapshot.radar_cards.iter_mut().find(|c| c.id.0 == tid.0) {
                                let mut leveled = false;
                                card.amount.0 = card.amount.0.saturating_add(1);
                                if card.max.0 > 0 && card.amount.0 >= card.max.0 { card.level.0 = card.level.0.saturating_add(1); card.amount.0 = 0; leveled = true; }
                                Some((card.amount.0, card.max.0, card.level.0, leveled, card.used.0 != 0))
                            } else if let Some(tpl) = self.shared.templates.radars.iter().find(|r| r.id == tid.0) {
                                let opts = tpl.options.iter().map(|o| RadarCardOption { id: StrNum(o.id), active: StrNum(o.active_card), param: StrNum(i32::from(o.param)) }).collect();
                                let (amt, lvl) = if tpl.max <= 1 { (0, 1) } else { (1, 0) };
                                entry.snapshot.radar_cards.push(RadarCardRecord { id: StrNum(tid.0), amount: StrNum(amt), max: StrNum(tpl.max), option: opts, level: StrNum(lvl), used: StrNum(0) });
                                Some((amt, tpl.max, lvl, lvl > 0, false))
                            } else { None };
                            if let Some((amt, max, lvl, leveled, used)) = card_upd {
                                let mut w3 = PacketWriter::with_capacity(5);
                                w3.write_i8(3); w3.write_i16(tid.0); w3.write_i8(amt); w3.write_i8(max);
                                extra_pkts.push(w3.finish(127));
                                if leveled { let mut w2 = PacketWriter::with_capacity(4); w2.write_i8(2); w2.write_i16(tid.0); w2.write_i8(lvl); extra_pkts.push(w2.finish(127)); }
                                if used { super::recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode); }
                            }
                            if let Ok(tb) = s2c_packets::thong_bao("Đã cập nhật thẻ sưu tầm!") { extra_pkts.push(tb); }
                        }
                        UseItemEffect::TeleportMap(map_id) => teleport_target = Some(MapId(map_id as i16)),
                        UseItemEffect::PoisonedDeath => died_info = Some((entry.snapshot.id, entry.snapshot.location, entry.snapshot.stats.power)),
                    }
                    let (eh, eb, el, ebag, is_disg) = effective_appearance(&self.shared.templates, &entry.snapshot);
                    let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
                    let cur_fusion = entry.snapshot.pet.as_ref().map_or(const_player::NON_FUSION, |p| p.fusion_type);
                    let mut bcasts = Vec::new();
                    if cur_fusion != prev_fusion {
                        let wire_fusion = match cur_fusion { const_player::NON_FUSION => 1_i8, const_player::HOP_THE_PORATA | const_player::HOP_THE_PORATA2 | const_player::HOP_THE_PORATA3 => 6_i8, other => other };
                        let mut w = PacketWriter::with_capacity(5);
                        w.write_i8(wire_fusion); w.write_i32(pid.0 as i32);
                        bcasts.push(w.finish(cmd_message::FUSION));
                    }
                    if (eh, eb, el, is_disg) != (prev_app.0, prev_app.1, prev_app.2, prev_app.4) { bcasts.push(s2c_packets::update_body(pid, is_disg.then_some((eh, eb, el, 0)))); }
                    if ebag != prev_app.3 { bcasts.push(s2c_packets::send_flag_bag(pid, ebag)); }
                    if !bcasts.is_empty() { zone_bcasts = Some((loc, bcasts)); }
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    Some([s2c_packets::inventory_body(eh, &entry.snapshot.items_body), s2c_packets::inventory_bag(&entry.snapshot.items_bag), s2c_packets::me_load_point(&entry.snapshot.stats), wallet_money_pkt(&entry.snapshot.wallet)])
                }
                Err(err) => { err_msg = Some(format!("Không thể sử dụng vật phẩm: {err}")); None }
            }
        });
        if let Some(base_pkts) = packets {
            for p in base_pkts { session.send(p).await?; }
            for p in extra_pkts { session.send(p).await?; }
        }
        if let Some((tid, loc, pid, clan_id)) = satellite_drop && let Ok(tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) {
            let _ = tx.send(ZoneCommand::DropItem(tid, 1, loc, Some(pid), GroundDropKind::Satellite { clan_id })).await;
        }
        if power_gained { self.advance_task_on_power(session, sid).await?; }
        if let Some((loc, bcasts)) = zone_bcasts { for p in bcasts { self.broadcast_zone_or_self(loc, sid, &p); } }
        if let Some(msg) = err_msg { send_thong_bao(session, &msg)?; }
        if let Some((pid, loc, power)) = died_info {
            self.handle_player_death_cleanup(pid, loc, sid);
            let (me_die, peer_die) = s2c_packets::char_die_packets(pid, 0, loc, power);
            session.send(me_die).await?; self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
        }
        if let Some(dst_map) = teleport_target { self.switch_player_zone(session, sid, Some(dst_map), ZoneId(0), None).await?; }
        Ok(())
    }

    pub(super) async fn handle_skill_up(&self, session: &SessionHandle, sid: u64, template_id: i16, _pt: i8) -> Result<(), ProtocolError> {
        let pkts = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut out = Vec::new();
            if let Some(st) = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == template_id && s.nclass_id == i8::from(entry.snapshot.gender)) {
                let cur_pt = entry.snapshot.skills.iter().find(|sk| sk.template_id == i32::from(template_id)).map_or(0, |sk| sk.point);
                let next_pt = (cur_pt + 1).min(st.max_point.max(1));
                if cur_pt <= 0 {
                    if let Ok(tb) = s2c_packets::thong_bao("Cần học sách kỹ năng trước khi nâng cấp!") { out.push(tb); }
                } else if next_pt <= cur_pt {
                    if let Ok(tb) = s2c_packets::thong_bao("Kỹ năng đã đạt cấp tối đa!") { out.push(tb); }
                } else if let Some(lv) = st.levels.iter().find(|l| l.point == next_pt) {
                    let cost = i64::from(lv.price.max(0)).saturating_mul(1_000);
                    if entry.snapshot.stats.power < lv.power_require {
                        if let Ok(tb) = s2c_packets::thong_bao(&format!("Không đủ sức mạnh để nâng kỹ năng (cần {})!", lv.power_require)) { out.push(tb); }
                    } else if entry.snapshot.stats.tiem_nang < cost {
                        if let Ok(tb) = s2c_packets::thong_bao(&format!("Không đủ tiềm năng để nâng kỹ năng (cần {cost})!")) { out.push(tb); }
                    } else {
                        entry.snapshot.stats.tiem_nang -= cost;
                        if let Some(sk) = entry.snapshot.skills.iter_mut().find(|s| s.template_id == i32::from(template_id)) {
                            sk.point = next_pt; out.push(s2c_packets::me_update_skill(lv.id));
                        } else {
                            entry.snapshot.skills.push(SkillSlot { template_id: i32::from(template_id), point: next_pt, last_time_use_ms: 0, curr_level: 0 });
                            out.push(s2c_packets::me_add_skill(lv.id));
                        }
                    }
                }
            }
            out.push(s2c_packets::me_load_point(&entry.snapshot.stats)); out
        });
        if let Some(list) = pkts { for p in list { session.send(p).await?; } }
        Ok(())
    }

    pub(super) fn learn_shop_skill(&self, snap: &mut nro_db::player_repo::PlayerSnapshot, item_id: TemplateId) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let tpl = self.shared.templates.items.iter().find(|r| r.id == i32::from(item_id.0));
        let st = tpl.and_then(|ir| self.shared.templates.skills.iter().find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name)))
            .or_else(|| self.shared.templates.skills.iter().find(|s| s.nclass_id == g_cls && s.levels.iter().any(|l| l.id == item_id.0)))?;
        let cur_pt = snap.skills.iter().find(|sk| sk.template_id == i32::from(st.id)).map_or(0, |sk| sk.point);
        let next_pt = (cur_pt + 1).min(st.max_point.max(1));
        if next_pt <= cur_pt { return None; }
        let lv = st.levels.iter().find(|l| l.point == next_pt).or_else(|| st.levels.first())?;
        let tn_cost = i64::from(lv.price.max(0)).saturating_mul(1_000);
        if snap.stats.power < lv.power_require || snap.stats.tiem_nang < tn_cost { return None; }
        snap.stats.tiem_nang -= tn_cost;
        if let Some(sk) = snap.skills.iter_mut().find(|s| s.template_id == i32::from(st.id)) {
            let was_zero = sk.point <= 0; sk.point = next_pt;
            Some(if was_zero { s2c_packets::me_add_skill(lv.id) } else { s2c_packets::me_update_skill(lv.id) })
        } else {
            snap.skills.push(SkillSlot { template_id: i32::from(st.id), point: next_pt, last_time_use_ms: 0, curr_level: 0 });
            Some(s2c_packets::me_add_skill(lv.id))
        }
    }

    fn capsule_label_for(&self, map_id: MapId) -> (&str, &str) {
        self.shared.templates.maps.iter().find(|m| m.id == map_id).map_or(("Trạm tàu vũ trụ", "Trái Đất"), |m| (m.name.as_str(), match m.planet_id { 1 => "Namếc", 2 => "Xayda", _ => "Trái Đất" }))
    }

    pub(super) fn apply_skill_book(&self, snap: &mut nro_db::player_repo::PlayerSnapshot, book_tid: TemplateId) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let item_rec = self.shared.templates.items.iter().find(|r| r.id == i32::from(book_tid.0));
        let st = u16::try_from(book_tid.0)
            .ok()
            .and_then(SkillKind::from_skill_book_item)
            .and_then(|kind| self.shared.templates.skills.iter().find(|s| s.id == kind as i8))
            .or_else(|| {
                item_rec.and_then(|ir| {
                    self.shared.templates.skills.iter().find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name))
                })
            })?;
        let stid = st.id;
        let new_pt = item_rec.map_or(1_i8, |ir| ir.level.clamp(1, i32::from(st.max_point.max(1))) as i8);
        if let Some(existing) = snap.skills.iter_mut().find(|sk| sk.template_id == i32::from(stid)) {
            let was_zero = existing.point <= 0;
            let final_pt = existing.point.max(new_pt);
            existing.point = final_pt;
            let lv_id = st.levels.iter().find(|l| l.point == final_pt).or_else(|| st.levels.first()).map_or(i16::from(stid), |l| l.id);
            Some(if was_zero { s2c_packets::me_add_skill(lv_id) } else { s2c_packets::me_update_skill(lv_id) })
        } else {
            let lv_id = st.levels.iter().find(|l| l.point == new_pt).or_else(|| st.levels.first()).map_or(i16::from(stid), |l| l.id);
            snap.skills.push(SkillSlot { template_id: i32::from(stid), point: new_pt, last_time_use_ms: 0, curr_level: 0 });
            Some(s2c_packets::me_add_skill(lv_id))
        }
    }
}
