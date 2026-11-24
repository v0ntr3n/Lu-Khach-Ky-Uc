//! Auxiliary in-world packet handlers (`LoadRms`, `PlayerMove`, `PotentialUp`, `BoxSort`, `ChangeName`, `CombinePreview`, `LockInventory`).

use nro_player::{combat::StatUpgradeKind, npoint::NPoint};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{ServerSessionHandler, recalculate_session_stats, router_skill, send_thong_bao};
use crate::runtime::{LeaderboardSlot, unix_now_ms};

impl ServerSessionHandler {
    pub(super) async fn handle_load_rms(
        &self,
        session: &SessionHandle,
        sid: u64,
        key: &str,
    ) -> Result<(), ProtocolError> {
        if let Some(p) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            let (sc, cs) = (
                e.snapshot.skills_shortcut.map(|s| s as u8),
                [e.selected_skill as u8],
            );
            s2c_packets::key_skill_rms(
                key,
                match key {
                    "KSkill" | "OSkill" => &sc,
                    "CSkill" => &cs,
                    _ => &[],
                },
            )
            .ok()
        }) {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_player_move(
        &self,
        session: &SessionHandle,
        sid: u64,
        x: i16,
        maybe_y: Option<i16>,
    ) -> Result<(), ProtocolError> {
        router_skill::clear_ttnl(session.player_id());
        if let Some((mid, cx)) =
            self.shared
                .clients
                .authenticated
                .get_mut(&sid)
                .and_then(|mut e| {
                    let mid = e.snapshot.location.map_id;
                    let wp_w = self
                        .shared
                        .templates
                        .maps
                        .iter()
                        .find(|m| m.id == mid)
                        .and_then(|m| m.waypoints.iter().map(|w| w.3.saturating_add(48)).max())
                        .unwrap_or(1_200);
                    let map_w = self.shared.assets.map_pixel_width(mid.0).max(wp_w);
                    if (85..=91).contains(&mid.0) && (x < 24 || x > map_w.saturating_sub(24)) {
                        let _ = session.try_send(s2c_packets::reset_point(e.snapshot.location));
                        return None;
                    }
                    let cx = x.clamp(0, map_w.max(4_095));
                    e.snapshot.location.x = cx;
                    if let Some(y) = maybe_y {
                        e.snapshot.location.y = y;
                    }
                    let loc = e.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (
                        e.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ = tx.try_send(ZoneCommand::MovePlayer(ek, loc));
                    }
                    Some((mid, cx))
                })
        {
            self.advance_task_on_move(session, sid, mid, cx).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_potential_up(
        &self,
        session: &SessionHandle,
        sid: u64,
        stat_idx: i8,
        points: i16,
    ) -> Result<(), ProtocolError> {
        if let Some((pt_pkt, err_pkt, upgraded)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                let (mut err_pkt, mut upgraded) = (None, false);
                if let Some(kind) = u8::try_from(stat_idx)
                    .ok()
                    .and_then(StatUpgradeKind::from_repr)
                    && let Ok(pts) = u16::try_from(points)
                {
                    let mut np = NPoint::new(entry.snapshot.stats);
                    if np
                        .increase_point(kind, pts, self.shared.config.parity_mode)
                        .is_ok()
                    {
                        entry.snapshot.stats = np.core;
                        recalculate_session_stats(
                            sid,
                            &mut entry.snapshot,
                            self.shared.config.parity_mode,
                        );
                        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                        upgraded = true;
                    } else {
                        err_pkt = s2c_packets::thong_bao(
                            "Không đủ tiềm năng hoặc chưa mở giới hạn sức mạnh!",
                        )
                        .ok();
                    }
                } else {
                    err_pkt = s2c_packets::thong_bao("Điểm nâng không hợp lệ!").ok();
                }
                let loc = entry.snapshot.location;
                if let (Some(ek), Ok(tx)) = (
                    entry.entity_key,
                    self.shared
                        .world_router
                        .resolve_sender(loc.map_id, loc.zone_id),
                ) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                }
                (
                    s2c_packets::me_load_point(&entry.snapshot.stats),
                    err_pkt,
                    upgraded,
                )
            })
        {
            session.send(pt_pkt).await?;
            if let Some(ep) = err_pkt {
                session.send(ep).await?;
            }
            if upgraded {
                self.advance_task_on_potential_up(session, sid).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_box_sort(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        if let Some(p) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut e| {
                let cap = e.snapshot.items_box.len();
                let mut packed: Vec<_> =
                    e.snapshot.items_box.drain(..).flatten().map(Some).collect();
                packed.resize(cap, None);
                e.snapshot.items_box = packed;
                s2c_packets::inventory_box(&e.snapshot.items_box)
            })
        {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_change_name(
        &self,
        session: &SessionHandle,
        sid: u64,
        name: &str,
    ) -> Result<(), ProtocolError> {
        let trimmed = name.trim();
        let Some(mut cand) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.clone())
        else {
            return Ok(());
        };
        if !(3..=12).contains(&trimmed.chars().count())
            || !trimmed.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return send_thong_bao(session, "Tên nhân vật phải từ 3 đến 12 ký tự chữ hoặc số!");
        }
        if self
            .shared
            .clients
            .authenticated
            .iter()
            .any(|r| *r.key() != sid && r.value().snapshot.name.eq_ignore_ascii_case(trimmed))
            || self
                .shared
                .players_repo
                .is_name_taken(trimmed, cand.id)
                .unwrap_or(true)
        {
            return send_thong_bao(session, "Tên nhân vật đã được sử dụng!");
        }
        trimmed.clone_into(&mut cand.name);
        match self
            .shared
            .players_repo
            .save_player(&cand, None, self.shared.config.parity_mode, unix_now_ms())
            .await
        {
            Ok(()) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
                    trimmed.clone_into(&mut e.snapshot.name);
                }
                self.shared.leaderboards.mark_all_dirty();
                send_thong_bao(session, "Đổi tên nhân vật thành công!")
            }
            Err(nro_db::DbError::DuplicateKey { .. }) => {
                send_thong_bao(session, "Tên nhân vật đã được sử dụng!")
            }
            Err(_) => send_thong_bao(session, "Không thể đổi tên nhân vật lúc này!"),
        }
    }

    pub(super) async fn handle_combine_preview_cmd(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: i8,
        bag_indices: &[i8],
    ) -> Result<(), ProtocolError> {
        match action {
            1 => {
                session
                    .send(s2c_packets::combine_items(bag_indices))
                    .await?;
                self.handle_combine_preview(session, sid, bag_indices)
                    .await?;
            }
            2 | 3 | 8 => session.send(s2c_packets::combine_result(action)).await?,
            4 | 5 | 7 => {
                session
                    .send(s2c_packets::combine_result_with_icon(action, 0))
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn handle_lock_inventory(
        &self,
        session: &SessionHandle,
        sid: u64,
        pin: i32,
    ) -> Result<(), ProtocolError> {
        let ok = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .is_some_and(|mut e| {
                if e.snapshot.mbv_pin.0 == 0 {
                    e.snapshot.mbv_pin = (pin, pin != 0, unix_now_ms());
                    true
                } else if pin == e.snapshot.mbv_pin.0 {
                    e.snapshot.mbv_pin.1 = !e.snapshot.mbv_pin.1;
                    e.snapshot.mbv_pin.2 = unix_now_ms();
                    true
                } else {
                    false
                }
            });
        send_thong_bao(
            session,
            if ok {
                "Đã cập nhật khóa bảo vệ hành trang!"
            } else {
                "Mã bảo vệ không chính xác!"
            },
        )
    }
}
