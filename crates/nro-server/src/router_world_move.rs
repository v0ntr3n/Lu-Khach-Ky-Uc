//! Zone UI (`handle_open_zone_ui`), two-phase map/zone switching, and waypoint transitions.

use nro_core::{Location, MapId, ZoneId};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;
use tracing::warn;

use super::super::{ServerSessionHandler, send_thong_bao};
use super::MapChangeGuard;
use crate::data_game_tpl;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_open_zone_ui(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let Some(cur_map) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.location.map_id)
        else {
            return Ok(());
        };
        let (zone_count, max_per_zone) = self
            .shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == cur_map)
            .map_or((15_usize, 15_u8), |m| {
                (usize::from(m.zones.clamp(1, 20)), m.max_player.clamp(1, 30))
            });
        let mut counts = vec![0_u8; zone_count];
        for entry in &self.shared.clients.authenticated {
            if entry.snapshot.location.map_id == cur_map
                && let Some(slot) = counts.get_mut(usize::from(entry.snapshot.location.zone_id.0))
            {
                *slot = slot.saturating_add(1);
            }
        }
        let rows: Vec<s2c_packets::ZoneUiRow> = counts
            .into_iter()
            .enumerate()
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

    pub(in crate::router) async fn switch_player_zone(
        &self,
        session: &SessionHandle,
        sid: u64,
        dest_map: Option<MapId>,
        dest_zone: ZoneId,
        dest_coords: Option<(i16, i16)>,
    ) -> Result<(), ProtocolError> {
        let _ = self
            .switch_player_zone_with_ship(session, sid, dest_map, dest_zone, dest_coords, 0)
            .await?;
        Ok(())
    }

    pub(in crate::router) async fn switch_player_zone_with_ship(
        &self,
        session: &SessionHandle,
        sid: u64,
        dest_map: Option<MapId>,
        dest_zone: ZoneId,
        dest_coords: Option<(i16, i16)>,
        arrival_ship: i8,
    ) -> Result<bool, ProtocolError> {
        let Some(_inflight) = MapChangeGuard::acquire(sid) else {
            return Ok(false);
        };
        let Some((pid, ek, cur_loc)) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            e.entity_key
                .map(|k| (e.snapshot.id, k, e.snapshot.location))
        }) else {
            return Ok(false);
        };
        let target_map = dest_map.unwrap_or(cur_loc.map_id);
        let Some(dest_tpl) = (target_map.0 >= 0)
            .then(|| {
                self.shared
                    .templates
                    .maps
                    .iter()
                    .find(|m| m.id == target_map)
            })
            .flatten()
        else {
            session.send(s2c_packets::reset_point(cur_loc)).await?;
            send_thong_bao(session, "Bản đồ này chưa mở hoặc không tồn tại!")?;
            return Ok(false);
        };
        let max_zones = u16::from(dest_tpl.zones.max(1));
        let safe_zone = if super::super::router_combat::is_offline_map(target_map)
            || dest_tpl.map_type == 1
            || dest_zone.0 >= max_zones
        {
            ZoneId(0)
        } else {
            dest_zone
        };
        if (target_map, safe_zone) == (cur_loc.map_id, cur_loc.zone_id) && dest_coords.is_none() {
            return Ok(true);
        }
        let (Ok(src_tx), Ok(dest_tx)) = (
            self.shared
                .world_router
                .resolve_sender(cur_loc.map_id, cur_loc.zone_id),
            self.shared
                .world_router
                .resolve_sender(target_map, safe_zone),
        ) else {
            session.send(s2c_packets::reset_point(cur_loc)).await?;
            send_thong_bao(session, "Không thể chuyển sang khu vực này!")?;
            return Ok(false);
        };
        let (target_x, target_y) = dest_coords.unwrap_or_else(|| {
            if target_map == cur_loc.map_id {
                (cur_loc.x, cur_loc.y)
            } else if matches!(target_map.0, 21..=23) {
                (300, 336)
            } else {
                (180, 336)
            }
        });
        let new_loc = Location {
            map_id: target_map,
            zone_id: safe_zone,
            x: target_x,
            y: target_y,
        };
        session.send(Packet::empty(-22)).await?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let rollback_key =
            if src_tx
                .send(ZoneCommand::ChangeMap(
                    ek,
                    (target_map, safe_zone),
                    new_loc,
                    arrival_ship,
                    reply_tx,
                ))
                .await
                .is_ok()
            {
                match reply_rx.await {
                    Ok(Ok(new_ek)) => {
                        let updated = self.shared.clients.authenticated.get_mut(&sid).is_some_and(
                            |mut entry| {
                                entry.entity_key = Some(new_ek);
                                entry.snapshot.location = new_loc;
                                true
                            },
                        );
                        if !updated {
                            if self.shared.clients.players.get(&pid).is_none() {
                                let _ = dest_tx
                                    .send(ZoneCommand::LeavePlayer { player_id: pid })
                                    .await;
                            }
                            return Ok(false);
                        }
                        self.cleanup_trade_on_disconnect(pid);
                        self.cleanup_combat_on_zone_change(pid, cur_loc, sid);
                        super::super::npc_menu_index().remove(&pid);
                        if target_map != cur_loc.map_id {
                            super::super::open_shop_state().remove(&pid);
                            super::super::pending_combine().remove(&pid);
                            super::super::pending_input().remove(&pid);
                            super::super::con_meo_state().remove(&pid);
                            super::super::router_world_tree::clear_magic_tree_menu_state(pid);
                            if (114..=120).contains(&cur_loc.map_id.0) {
                                super::super::router_npc::reset_mabu_points(pid);
                            }
                            if super::super::router_npc::clear_map_scoped_buffs(
                                pid,
                                cur_loc.map_id,
                                target_map,
                            ) && let Some(pt_pkt) = self
                                .shared
                                .clients
                                .authenticated
                                .get_mut(&sid)
                                .map(|mut entry| {
                                    super::super::recalculate_session_stats(
                                        sid,
                                        &mut entry.snapshot,
                                        self.shared.config.parity_mode,
                                    );
                                    let _ = dest_tx.try_send(ZoneCommand::UpdatePlayerStats(
                                        new_ek,
                                        entry.snapshot.stats,
                                    ));
                                    s2c_packets::me_load_point(&entry.snapshot.stats)
                                })
                            {
                                session.send(pt_pkt).await?;
                            }
                        }
                        self.advance_task_on_enter_map(session, sid, target_map)
                            .await?;
                        if let Some(snap) = self
                            .shared
                            .clients
                            .authenticated
                            .get(&sid)
                            .map(|e| e.snapshot.clone())
                        {
                            session
                                .send(data_game_tpl::pkt_task_get(&self.shared.templates, &snap))
                                .await?;
                            session
                                .send(data_game_tpl::pkt_map_info_with_ship(
                                    &self.shared.assets,
                                    &self.shared.templates,
                                    &snap,
                                    arrival_ship,
                                ))
                                .await?;
                            for egg_pkt in
                                crate::router_pet::build_home_egg_pkts(&snap, unix_now_ms())
                            {
                                session.send(egg_pkt).await?;
                            }
                        }
                        let _ = dest_tx.try_send(ZoneCommand::SyncZoneState(new_ek));
                        crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                        return Ok(true);
                    }
                    Ok(Err((_err, rollback_key))) => rollback_key,
                    Err(_) => None,
                }
            } else {
                None
            };

        let mut matched_stale = false;
        let present = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .is_some_and(|mut entry| {
                if entry.entity_key == Some(ek) {
                    entry.entity_key = rollback_key;
                    matched_stale = true;
                }
                true
            });
        if !present {
            if rollback_key.is_some() && self.shared.clients.players.get(&pid).is_none() {
                let _ = src_tx
                    .send(ZoneCommand::LeavePlayer { player_id: pid })
                    .await;
            }
            return Ok(false);
        }
        if !matched_stale {
            return Ok(false);
        }
        let Some(rb_ek) = rollback_key else {
            warn!(
                sid,
                to_map = target_map.0,
                to_zone = safe_zone.0,
                "ChangeMap failed without rollback entity; disconnecting session"
            );
            session.disconnect();
            return Ok(false);
        };
        warn!(
            sid,
            to_map = target_map.0,
            to_zone = safe_zone.0,
            "ChangeMap failed; restoring current map"
        );
        if let Some(snap) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.clone())
        {
            session
                .send(data_game_tpl::pkt_map_info(
                    &self.shared.assets,
                    &self.shared.templates,
                    &snap,
                ))
                .await?;
            for egg_pkt in crate::router_pet::build_home_egg_pkts(&snap, unix_now_ms()) {
                session.send(egg_pkt).await?;
            }
        }
        let _ = src_tx.try_send(ZoneCommand::SyncZoneState(rb_ek));
        crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
        session.send(s2c_packets::reset_point(cur_loc)).await?;
        send_thong_bao(session, "Khu vực đã đầy hoặc không thể vào!")?;
        Ok(false)
    }

    pub(in crate::router) async fn handle_waypoint_change(
        &self,
        session: &SessionHandle,
        sid: u64,
        is_offline: bool,
    ) -> Result<(), ProtocolError> {
        const WP_MARGIN: i32 = 120;
        let Some(loc) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.location)
        else {
            return Ok(());
        };
        let air_drop = match loc.map_id.0 {
            45 if loc.y >= 300 => Some((MapId(46), 354_i16, 120_i16)),
            46 if loc.y >= 300 => Some((MapId(47), 612_i16, 144_i16)),
            _ => None,
        };
        let (px, py) = (i32::from(loc.x), i32::from(loc.y));
        let mut best_wp = None;
        let mut min_score = i64::MAX;
        if let Some(map_tpl) = self
            .shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == loc.map_id)
        {
            for wp in &map_tpl.waypoints {
                if wp.7.0 < 0 || !self.shared.templates.maps.iter().any(|m| m.id == wp.7) {
                    continue;
                }
                let (min_x, max_x) = (i32::from(wp.1.min(wp.3)), i32::from(wp.1.max(wp.3)));
                let (min_y, max_y) = (i32::from(wp.2.min(wp.4)), i32::from(wp.2.max(wp.4)));
                if px < min_x - WP_MARGIN
                    || px > max_x + WP_MARGIN
                    || py < min_y - WP_MARGIN
                    || py > max_y + WP_MARGIN
                {
                    continue;
                }
                let dx = i64::from(loc.x) - i64::midpoint(i64::from(wp.1), i64::from(wp.3));
                let dy = i64::from(loc.y) - i64::midpoint(i64::from(wp.2), i64::from(wp.4));
                if air_drop.is_some() && dy.abs() > 160 {
                    continue;
                }
                let score = dx * dx
                    + dy * dy
                    + if (wp.6 != 0) == is_offline {
                        0
                    } else {
                        1_000_000
                    };
                if score < min_score {
                    min_score = score;
                    best_wp = Some((wp.7, wp.8, wp.9));
                }
            }
        }
        if let Some((target_map, tx, ty)) = best_wp.or(air_drop) {
            return self
                .switch_player_zone(session, sid, Some(target_map), loc.zone_id, Some((tx, ty)))
                .await;
        }
        session.send(s2c_packets::reset_point(loc)).await?;
        send_thong_bao(session, "Không thể chuyển khu vực tại đây!")
    }
}
