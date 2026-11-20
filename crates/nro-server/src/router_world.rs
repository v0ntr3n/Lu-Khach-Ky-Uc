//! Zone UI, Magic Tree, map/zone switching, waypoint transitions, and map chat handlers.

use nro_core::{ItemInstance, ItemOption, Location, MapId, TemplateId, ZoneId};
use nro_db::player_repo::PlayerSnapshot;
use nro_protocol::{
    AdminCommand, ChatDirective, Packet, PacketError, ProtocolError, SessionHandle, parse_chat_command,
};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;
use tracing::{info, warn};

use super::{
    ServerSessionHandler, effective_appearance, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot,
};
use crate::data_game::DataGameAssets;
use crate::data_game_tpl;
use crate::runtime::unix_now_ms;

const PEA_IDS: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 295, 523, 595];

impl ServerSessionHandler {
    pub(super) async fn handle_client_ok(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some((snap, activated, ek)) = self
            .shared.clients.authenticated.get(&sid)
            .map(|e| (e.snapshot.clone(), e.activated, e.entity_key))
        else {
            return Ok(());
        };
        info!(sid, player_id = snap.id.0, map_id = snap.location.map_id.0, "ClientOk received; sending player & mapInfo");
        let (eff_head, eff_body, eff_leg, eff_bag, is_disguised) =
            effective_appearance(&self.shared.templates, &snap);
        let [p_all, p_flag, p_task, p_map] =
            data_game_tpl::pkts_client_ok(&self.shared.assets, &self.shared.templates, &snap);
        let skill_bytes: [u8; 10] = snap.skills_shortcut.map(|s| s as u8);
        let first_skill_u8 = snap.skills_shortcut.iter().copied().find(|&s| s >= 0)
            .or_else(|| snap.skills.iter().find(|sk| sk.point > 0).map(|sk| sk.template_id as i8))
            .unwrap_or(0) as u8;
        session.send(p_all).await?;
        if is_disguised {
            session.send(s2c_packets::update_body(snap.id, Some((eff_head, eff_body, eff_leg, 0)))).await?;
        }
        session.send(if eff_bag >= 0 { s2c_packets::send_flag_bag(snap.id, eff_bag) } else { p_flag }).await?;
        session.send(s2c_packets::key_skill_rms("KSkill", &skill_bytes)?).await?;
        session.send(s2c_packets::key_skill_rms("OSkill", &skill_bytes)?).await?;
        session.send(s2c_packets::key_skill_rms("CSkill", &[first_skill_u8])?).await?;
        session.send(s2c_packets::me_load_point(&snap.stats)).await?;
        session.send(s2c_packets::max_stamina(snap.stats.max_stamina.min(i16::MAX as u16) as i16)).await?;
        session.send(s2c_packets::stamina(snap.stats.stamina.min(i16::MAX as u16) as i16)).await?;
        session.send(s2c_packets::active_point(0)).await?;
        session.send(DataGameAssets::pkt_update_caption(i8::from(snap.gender))).await?;
        session.send(s2c_packets::have_pet(snap.pet.is_some())).await?;
        session.send(s2c_packets::intrinsic_info(0, "Chưa mở nội tại")?).await?;
        session.send(s2c_packets::type_active(activated)).await?;
        self.send_login_secondary(session, &snap).await?;
        session.send(p_task).await?;
        session.send(p_map).await?;
        if let (Some(ek), Ok(tx)) = (ek, self.shared.world_router.resolve_sender(snap.location.map_id, snap.location.zone_id)) {
            let _ = tx.try_send(ZoneCommand::SyncZoneState(ek));
        }
        Ok(())
    }

    pub(super) async fn handle_magic_tree(&self, session: &SessionHandle, sid: u64, action: i8) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let Some(snap) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
            tick_magic_tree(&mut e.snapshot, now_ms);
            e.snapshot.clone()
        }) else {
            return Ok(());
        };
        match action {
            1 => session.send(s2c_packets::magic_tree_menu(&["Thu hoạch", "Nâng cấp"])?).await?,
            0 | 2 => session.send(build_magic_tree_load(&snap)?).await?,
            _ => {}
        }
        Ok(())
    }

    pub(super) async fn handle_magic_tree_confirm(&self, session: &SessionHandle, sid: u64, menu_id: i8) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let mut harvested_ok = false;
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if !matches!(entry.snapshot.location.map_id.0, 21..=23) { return (None, None, None); }
            tick_magic_tree(&mut entry.snapshot, now_ms);
            match menu_id {
                0 => {
                    if super::router_shop::is_in_trade(entry.snapshot.id) { return (None, None, Some("Không thể thao tác khi đang giao dịch!".to_owned())); }
                    let harvested = entry.snapshot.magic_tree.curr_peas.max(0) as u32;
                    if harvested == 0 {
                        return (None, None, Some("Chưa có đậu thần để thu hoạch!".to_owned()));
                    }
                    let lv = entry.snapshot.magic_tree.level.clamp(1, 10);
                    let pea_tid = TemplateId(PEA_IDS[usize::from((lv - 1).cast_unsigned())]);
                    let pea_opt = if lv <= 3 { ItemOption::new(2, i32::from(lv)) } else { ItemOption::new(48, i32::from(lv) * 2_000) };
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    if inv.add_to_bag(ItemInstance::with_options(pea_tid, harvested, [pea_opt], now_ms), true).is_err() {
                        return (None, None, Some("Hành trang đã đầy, không thể thu hoạch đậu!".to_owned()));
                    }
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    entry.snapshot.magic_tree.curr_peas = 0;
                    entry.snapshot.magic_tree.last_time_harvest_ms = now_ms;
                    harvested_ok = true;
                    (
                        Some(s2c_packets::magic_tree_harvest(0, i32::from(lv) * 60)),
                        Some(s2c_packets::inventory_bag(&entry.snapshot.items_bag)),
                        Some(format!("Bạn vừa thu hoạch được {harvested} hạt đậu thần!")),
                    )
                }
                1 => {
                    let lv = entry.snapshot.magic_tree.level.clamp(1, 10);
                    if lv >= 10 {
                        return (None, None, Some("Đậu thần đã đạt cấp tối đa!".to_owned()));
                    }
                    let next_lv = lv + 1;
                    entry.snapshot.magic_tree.level = next_lv;
                    entry.snapshot.magic_tree.curr_peas = 5 + (next_lv - 1) * 2;
                    entry.snapshot.magic_tree.last_time_harvest_ms = now_ms;
                    (build_magic_tree_load(&entry.snapshot).ok(), None, Some(format!("Đã nâng cấp Đậu thần lên cấp {next_lv}!")))
                }
                _ => (None, None, None),
            }
        });
        if let Some((mt_pkt, bag_pkt, msg)) = outcome {
            if let Some(p) = mt_pkt { session.send(p).await?; }
            if let Some(b) = bag_pkt { session.send(b).await?; }
            if let Some(text) = msg { send_thong_bao(session, &text)?; }
        }
        if harvested_ok {
            self.advance_task_on_harvest(session, sid).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_finish_load_map(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let Some((snap, ek)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
            if matches!(e.snapshot.location.map_id.0, 21..=23) {
                tick_magic_tree(&mut e.snapshot, now_ms);
            }
            (e.snapshot.clone(), e.entity_key)
        }) else {
            return Ok(());
        };
        session.send(s2c_packets::me_load_point(&snap.stats)).await?;
        if matches!(snap.location.map_id.0, 21..=23) {
            session.send(build_magic_tree_load(&snap)?).await?;
        }
        if let (Some(ek), Ok(tx)) = (ek, self.shared.world_router.resolve_sender(snap.location.map_id, snap.location.zone_id)) {
            let _ = tx.try_send(ZoneCommand::SyncZoneState(ek));
        }
        Ok(())
    }

    pub(super) async fn handle_open_zone_ui(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some(cur_map) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.location.map_id) else {
            return Ok(());
        };
        let (zone_count, max_per_zone) = self.shared.templates.maps.iter().find(|m| m.id == cur_map)
            .map_or((15_usize, 15_u8), |m| (usize::from(m.zones.clamp(1, 20)), m.max_player.clamp(1, 30)));
        let mut counts = vec![0_u8; zone_count];
        for entry in &self.shared.clients.authenticated {
            if entry.snapshot.location.map_id == cur_map
                && let Some(slot) = counts.get_mut(usize::from(entry.snapshot.location.zone_id.0))
            {
                *slot = slot.saturating_add(1);
            }
        }
        let rows: Vec<s2c_packets::ZoneUiRow> = counts.into_iter().enumerate()
            .map(|(z, num)| s2c_packets::ZoneUiRow {
                zone_id: ZoneId(u16::try_from(z).unwrap_or(0)),
                counts: (num, max_per_zone),
                rank_match: None,
            })
            .collect();
        if let Ok(pkt) = s2c_packets::open_zone_ui(&rows) {
            session.send(pkt).await?;
        }
        Ok(())
    }

    pub(super) async fn switch_player_zone(
        &self, session: &SessionHandle, sid: u64, dest_map: Option<MapId>, dest_zone: ZoneId, dest_coords: Option<(i16, i16)>,
    ) -> Result<(), ProtocolError> {
        let _ = self.switch_player_zone_with_ship(session, sid, dest_map, dest_zone, dest_coords, 0).await?;
        Ok(())
    }

    pub(super) async fn switch_player_zone_with_ship(
        &self, session: &SessionHandle, sid: u64, dest_map: Option<MapId>, dest_zone: ZoneId, dest_coords: Option<(i16, i16)>, arrival_ship: i8,
    ) -> Result<bool, ProtocolError> {
        let Some((pid, ek, cur_loc)) = self.shared.clients.authenticated.get(&sid)
            .and_then(|e| e.entity_key.map(|k| (e.snapshot.id, k, e.snapshot.location)))
        else {
            return Ok(false);
        };
        let target_map = dest_map.unwrap_or(cur_loc.map_id);
        let Some(dest_tpl) = (target_map.0 >= 0)
            .then(|| self.shared.templates.maps.iter().find(|m| m.id == target_map))
            .flatten()
        else {
            session.send(s2c_packets::reset_point(cur_loc)).await?;
            send_thong_bao(session, "Bản đồ này chưa mở hoặc không tồn tại!")?;
            return Ok(false);
        };
        let max_zones = u16::from(dest_tpl.zones.max(1));
        let safe_zone = if super::router_combat::is_offline_map(target_map) || dest_tpl.map_type == 1 || dest_zone.0 >= max_zones {
            ZoneId(0)
        } else {
            dest_zone
        };
        let (Ok(src_tx), Ok(dest_tx)) = (
            self.shared.world_router.resolve_sender(cur_loc.map_id, cur_loc.zone_id),
            self.shared.world_router.resolve_sender(target_map, safe_zone),
        ) else {
            session.send(s2c_packets::reset_point(cur_loc)).await?;
            send_thong_bao(session, "Không thể chuyển sang khu vực này!")?;
            return Ok(false);
        };
        let (target_x, target_y) = dest_coords.unwrap_or_else(|| {
            if target_map == cur_loc.map_id { (cur_loc.x, cur_loc.y) } else if matches!(target_map.0, 21..=23) { (300, 336) } else { (180, 336) }
        });
        let new_loc = Location { map_id: target_map, zone_id: safe_zone, x: target_x, y: target_y };
        self.cleanup_trade_on_disconnect(pid);
        self.cleanup_combat_on_disconnect(pid, cur_loc, sid);
        super::router_skill::clear_skill_state(pid);
        session.send(Packet::empty(-22)).await?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let rollback_key = if src_tx.send(ZoneCommand::ChangeMap(ek, (target_map, safe_zone), new_loc, arrival_ship, reply_tx)).await.is_ok() {
            match reply_rx.await {
                Ok(Ok(new_ek)) => {
                    let updated = self.shared.clients.authenticated.get_mut(&sid).is_some_and(|mut entry| {
                        entry.entity_key = Some(new_ek);
                        entry.snapshot.location = new_loc;
                        true
                    });
                    if !updated {
                        if self.shared.clients.players.get(&pid).is_none() {
                            let _ = dest_tx.send(ZoneCommand::LeavePlayer { player_id: pid }).await;
                        }
                        return Ok(false);
                    }
                    self.advance_task_on_enter_map(session, sid, target_map).await?;
                    if let Some(snap) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.clone()) {
                        session.send(data_game_tpl::pkt_task_get(&self.shared.templates, &snap)).await?;
                        session.send(data_game_tpl::pkt_map_info_with_ship(&self.shared.assets, &self.shared.templates, &snap, arrival_ship)).await?;
                    }
                    let _ = dest_tx.try_send(ZoneCommand::SyncZoneState(new_ek));
                    return Ok(true);
                }
                Ok(Err((_err, rollback_key))) => rollback_key,
                Err(_) => None,
            }
        } else {
            None
        };
        let updated = self.shared.clients.authenticated.get_mut(&sid).is_some_and(|mut entry| {
            entry.entity_key = rollback_key;
            true
        });
        if !updated {
            if rollback_key.is_some() && self.shared.clients.players.get(&pid).is_none() {
                let _ = src_tx.send(ZoneCommand::LeavePlayer { player_id: pid }).await;
            }
            return Ok(false);
        }
        if rollback_key.is_none() {
            warn!(sid, to_map = target_map.0, to_zone = safe_zone.0, "ChangeMap failed without rollback entity; disconnecting session");
            session.disconnect();
            return Ok(false);
        }
        warn!(sid, to_map = target_map.0, to_zone = safe_zone.0, "ChangeMap failed; restoring current map");
        if let Some(snap) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.clone()) {
            session.send(data_game_tpl::pkt_map_info(&self.shared.assets, &self.shared.templates, &snap)).await?;
        }
        session.send(s2c_packets::reset_point(cur_loc)).await?;
        send_thong_bao(session, "Khu vực đã đầy hoặc không thể vào!")?;
        Ok(false)
    }

    pub(super) async fn handle_waypoint_change(&self, session: &SessionHandle, sid: u64, is_offline: bool) -> Result<(), ProtocolError> {
        let Some(loc) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.location) else {
            return Ok(());
        };
        let air_drop = match loc.map_id.0 {
            45 if loc.y >= 300 => Some((MapId(46), 354_i16, 120_i16)),
            46 if loc.y >= 300 => Some((MapId(47), 612_i16, 144_i16)),
            _ => None,
        };
        let mut best_wp = None;
        let mut min_score = i64::MAX;
        if let Some(map_tpl) = self.shared.templates.maps.iter().find(|m| m.id == loc.map_id) {
            for wp in &map_tpl.waypoints {
                if wp.7.0 < 0 || !self.shared.templates.maps.iter().any(|m| m.id == wp.7) {
                    continue;
                }
                let dx = i64::from(loc.x) - i64::midpoint(i64::from(wp.1), i64::from(wp.3));
                let dy = i64::from(loc.y) - i64::midpoint(i64::from(wp.2), i64::from(wp.4));
                if air_drop.is_some() && dy.abs() > 160 {
                    continue;
                }
                let score = dx * dx + dy * dy + if (wp.6 != 0) == is_offline { 0 } else { 1_000_000 };
                if score < min_score {
                    min_score = score;
                    best_wp = Some((wp.7, wp.8, wp.9));
                }
            }
        }
        let fallback_air = match loc.map_id.0 {
            45 => Some((MapId(46), 354_i16, 120_i16)),
            46 => Some((MapId(47), 612_i16, 144_i16)),
            _ => None,
        };
        if let Some((target_map, tx, ty)) = best_wp.or(air_drop).or(fallback_air) {
            return self.switch_player_zone(session, sid, Some(target_map), loc.zone_id, Some((tx, ty))).await;
        }
        session.send(s2c_packets::reset_point(loc)).await?;
        send_thong_bao(session, "Không thể chuyển khu vực tại đây!")
    }

    pub(super) async fn handle_chat_map(&self, session: &SessionHandle, sid: u64, raw_text: &str) -> Result<(), ProtocolError> {
        let Some((pid, is_admin)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.is_admin)) else {
            return Ok(());
        };
        if raw_text.trim().eq_ignore_ascii_case("pet") {
            if super::router_shop::is_in_trade(pid) {
                return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
            }
            return self.grant_starter_pet(session, sid).await;
        }
        match parse_chat_command(raw_text, is_admin) {
            ChatDirective::EmptyIgnore => {}
            ChatDirective::Admin(Err(syntax_err)) => send_thong_bao(session, &syntax_err.to_string())?,
            ChatDirective::Admin(Ok(cmd)) => {
                if matches!(cmd, AdminCommand::SpawnItems { .. } | AdminCommand::TeleportMap { .. })
                    && super::router_shop::is_in_trade(pid)
                {
                    return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
                }
                self.exec_admin_chat_cmd(session, sid, cmd).await?;
            }
            ChatDirective::ZoneChat { cleaned_text, .. } => {
                if let Some(e) = self.shared.clients.authenticated.get(&sid)
                    && let (Some(ek), Ok(tx)) = (e.entity_key, self.shared.world_router.resolve_sender(e.snapshot.location.map_id, e.snapshot.location.zone_id))
                {
                    let _ = tx.try_send(ZoneCommand::ChatMap(ek, cleaned_text));
                }
            }
        }
        Ok(())
    }
}

fn tick_magic_tree(snap: &mut PlayerSnapshot, now_ms: i64) {
    snap.magic_tree.is_upgrade = false;
    let lv = snap.magic_tree.level.clamp(1, 10);
    let max_peas = 5 + (lv - 1) * 2;
    if snap.magic_tree.curr_peas < max_peas {
        let interval_ms = i64::from(lv) * 60_000;
        let grown = (now_ms.saturating_sub(snap.magic_tree.last_time_harvest_ms) / interval_ms)
            .clamp(0, i64::from(max_peas)) as i8;
        if grown > 0 {
            snap.magic_tree.curr_peas = snap.magic_tree.curr_peas.saturating_add(grown).min(max_peas);
            snap.magic_tree.last_time_harvest_ms = snap
                .magic_tree
                .last_time_harvest_ms
                .saturating_add(i64::from(grown) * interval_ms)
                .min(now_ms);
        }
    }
    if snap.main_task.task_id == 0 && snap.main_task.sub_index == 4 && snap.magic_tree.curr_peas <= 0 {
        snap.magic_tree.curr_peas = 1;
    }
}

pub(super) fn build_magic_tree_load(snap: &PlayerSnapshot) -> Result<Packet, PacketError> {
    let mt = &snap.magic_tree;
    let level = mt.level.clamp(1, 10);
    let max_peas = i16::from(5 + (level - 1) * 2);
    let curr_peas = i16::from(mt.curr_peas.max(0)).min(max_peas);
    let (x, y, icon_id) = match i8::from(snap.gender) {
        1 => (372_i16, 336_i16, 84_i16),
        2 => (348_i16, 336_i16, 372_i16),
        _ => (348_i16, 336_i16, 84_i16),
    };
    let name = format!("Đậu thần cấp {level}");
    let seconds = if curr_peas < max_peas {
        let interval = i64::from(level) * 60;
        let now_ms = unix_now_ms();
        let elapsed_secs = (now_ms.saturating_sub(mt.last_time_harvest_ms) / 1_000).max(0);
        (interval - (elapsed_secs % interval)) as i32
    } else {
        0
    };
    s2c_packets::magic_tree_load(icon_id, &name, x, y, level, curr_peas, max_peas, &name, seconds, mt.is_upgrade)
}
