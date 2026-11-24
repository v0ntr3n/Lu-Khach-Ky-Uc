//! Zone UI (`handle_open_zone_ui`) and waypoint map transitions (`handle_waypoint_change`).

use nro_core::{MapId, ZoneId};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::super::{ServerSessionHandler, send_thong_bao};

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
