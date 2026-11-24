//! Character creation (`CREATE_PLAYER`) validation and zone admission pipeline.

#[path = "router_auth_evict.rs"]
mod router_auth_evict;

use nro_core::{AccountId, Gender, Location, MapId, ZoneId};
use nro_db::player_repo::{NewPlayerSeed, PlayerSnapshot};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::ZoneCommand;
use slotmap::SlotMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::super::{effective_appearance, recalculate_snapshot_stats, send_dialog};
use super::ServerSessionHandler;
use crate::data_game::DataGameAssets;
use crate::data_game_tpl;
use crate::runtime::{ActivePlayerEntry, LeaderboardSlot, unix_now_ms};

const STARTER_HAIRS: [[i16; 3]; 3] = [[64, 30, 31], [9, 29, 32], [6, 27, 28]];

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_create_char(
        &self,
        session: &SessionHandle,
        name: &str,
        gender_raw: i8,
        hair: i8,
    ) -> Result<(), ProtocolError> {
        if session.player_id().0 != 0 {
            return Ok(());
        }
        let account_id = session.account_id();
        info!(sid = session.id(), account_id = account_id.0, %name, gender = gender_raw, hair, "received CreateChar");
        if self.shared.maintenance.is_maintaining() {
            return send_dialog(
                session,
                "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!",
            );
        }
        if account_id.0 <= 0 {
            return send_dialog(
                session,
                "Phiên đăng nhập chưa xác thực, vui lòng đăng nhập lại!",
            );
        }
        let mut guard = match self.try_reserve_account(account_id, session.id()) {
            Ok(g) => g,
            Err(old_sid) => {
                if old_sid != session.id() {
                    self.evict_online_session(old_sid).await;
                }
                return send_dialog(
                    session,
                    "Tài khoản đang đăng nhập ở nơi khác, vui lòng thử lại sau vài giây!",
                );
            }
        };
        self.evict_account_sessions(account_id, session.id()).await;
        let (is_admin, activated) = match self.shared.accounts_repo.get_by_id(account_id) {
            Ok(Some(acc)) => (acc.is_admin, acc.active != 0),
            _ => (false, false),
        };
        if let Ok(Some(existing)) = self
            .shared
            .players_repo
            .load_by_account(account_id, self.shared.config.parity_mode)
            .await
        {
            if self
                .admit_player_into_world(session, account_id, existing, is_admin, activated)
                .await?
            {
                guard.armed = false;
            }
            return Ok(());
        }
        if !(3..=12).contains(&name.len())
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return send_dialog(
                session,
                "Tên nhân vật phải từ 3 đến 12 ký tự chữ thường hoặc số (a-z, 0-9)!",
            );
        }
        if !matches!(gender_raw, 0..=2)
            || !STARTER_HAIRS[gender_raw as usize].contains(&i16::from(hair))
        {
            return send_dialog(session, "Hành tinh hoặc kiểu tóc khởi tạo không hợp lệ!");
        }
        let seed = NewPlayerSeed {
            account_id,
            name: name.to_owned(),
            gender: Gender::from_i8_lossy(gender_raw),
            hair: i16::from(hair),
            test_mode: false,
            now_ms: unix_now_ms(),
        };
        match self.shared.players_repo.create_player(&seed).await {
            Ok(_pid) => match self
                .shared
                .players_repo
                .load_by_account(account_id, self.shared.config.parity_mode)
                .await
            {
                Ok(Some(snap)) => {
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Task);
                    if self
                        .admit_player_into_world(session, account_id, snap, is_admin, activated)
                        .await?
                    {
                        guard.armed = false;
                    }
                }
                Ok(None) => {
                    warn!(
                        account_id = account_id.0,
                        "created player missing on immediate reload"
                    );
                    send_dialog(
                        session,
                        "Không thể tải nhân vật vừa tạo, vui lòng đăng nhập lại!",
                    )?;
                }
                Err(err) => {
                    warn!(account_id = account_id.0, error = %err, "failed loading newly created character");
                    send_dialog(session, "Lỗi tải nhân vật vừa tạo, vui lòng thử lại!")?;
                }
            },
            Err(err) => {
                debug!(account_id = account_id.0, error = %err, "character creation rejected");
                send_dialog(session, "Tên nhân vật không hợp lệ hoặc đã tồn tại!")?;
            }
        }
        Ok(())
    }

    pub(super) async fn admit_player_into_world(
        &self,
        session: &SessionHandle,
        account_id: AccountId,
        mut snap: PlayerSnapshot,
        is_admin: bool,
        activated: bool,
    ) -> Result<bool, ProtocolError> {
        crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut snap);
        recalculate_snapshot_stats(&mut snap, self.shared.config.parity_mode);
        let (eff_head, eff_body, eff_leg, _, _) =
            effective_appearance(&self.shared.templates, &snap);

        let mut sm: SlotMap<nro_core::SessionKey, ()> = SlotMap::with_key();
        let session_key = sm.insert(());
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Packet>(256);

        let home_map = MapId(21 + i16::from(i8::from(snap.gender)));
        if matches!(snap.location.map_id.0, 21..=23) && snap.location.map_id != home_map {
            snap.location = Location {
                map_id: home_map,
                zone_id: ZoneId(0),
                x: 300,
                y: 336,
            };
        }
        let zones_count = self
            .shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == snap.location.map_id)
            .map_or(1_u16, |m| u16::from(m.zones.max(1)));
        let start_z = snap.location.zone_id.0 % zones_count;

        let mut entity_key = None;
        for offset in 0..zones_count {
            let z = ZoneId((start_z + offset) % zones_count);
            let cand_loc = Location {
                zone_id: z,
                ..snap.location
            };
            if let Some(ek) = self
                .try_enter_zone(
                    account_id,
                    session_key,
                    &snap,
                    cand_loc,
                    (eff_head, eff_body, eff_leg),
                    outbound_tx.clone(),
                )
                .await
            {
                snap.location = cand_loc;
                entity_key = Some(ek);
                break;
            }
        }
        if entity_key.is_none() {
            let home_loc = Location {
                map_id: home_map,
                zone_id: ZoneId(0),
                x: 300,
                y: 336,
            };
            if let Some(ek) = self
                .try_enter_zone(
                    account_id,
                    session_key,
                    &snap,
                    home_loc,
                    (eff_head, eff_body, eff_leg),
                    outbound_tx.clone(),
                )
                .await
            {
                snap.location = home_loc;
                entity_key = Some(ek);
            }
        }
        let Some(ek) = entity_key else {
            warn!(
                sid = session.id(),
                account_id = account_id.0,
                map_id = snap.location.map_id.0,
                "failed admitting player into any zone; rejecting login"
            );
            send_dialog(
                session,
                "Tất cả khu vực đều đang đầy, vui lòng thử lại sau!",
            )?;
            return Ok(false);
        };
        drop(outbound_tx);
        let (fwd_session, cancel) = (session.clone(), session.cancel_token());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => break,
                    maybe = outbound_rx.recv() => {
                        let Some(pkt) = maybe else { break; };
                        if fwd_session.send(pkt).await.is_err() { break; }
                    }
                }
            }
        });

        let selected_skill = snap
            .skills
            .iter()
            .find(|sk| sk.point > 0)
            .and_then(|sk| i16::try_from(sk.template_id).ok())
            .unwrap_or(0);
        let new_loc = snap.location;
        session.mark_authenticated(account_id, snap.id);
        let entry = ActivePlayerEntry {
            handle: session.clone(),
            session_key,
            entity_key: Some(ek),
            snapshot: snap,
            is_admin,
            activated,
            selected_skill,
            last_pea_ms: 0,
            c_flag: 0,
        };
        if let Some(evicted) =
            self.shared
                .clients
                .bind_authenticated(session.id(), account_id, entry)
        {
            let (ev_pid, ev_loc) = (evicted.snapshot.id, evicted.snapshot.location);
            self.cleanup_trade_on_disconnect(ev_pid);
            self.cleanup_combat_on_disconnect(ev_pid, ev_loc, 0);
            if (ev_loc.map_id, ev_loc.zone_id) != (new_loc.map_id, new_loc.zone_id)
                && let Ok(old_tx) = self
                    .shared
                    .world_router
                    .resolve_sender(ev_loc.map_id, ev_loc.zone_id)
            {
                let _ = old_tx
                    .send(ZoneCommand::LeavePlayer { player_id: ev_pid })
                    .await;
            }
        }
        session
            .send(
                self.shared
                    .assets
                    .pkt_small_version_for_zoom(session.zoom_level()),
            )
            .await?;
        session
            .send(
                self.shared
                    .assets
                    .pkt_bg_item_version_for_zoom(session.zoom_level()),
            )
            .await?;
        session.send(DataGameAssets::pkt_version_game()).await?;
        session
            .send(data_game_tpl::pkt_data_item_bg(&self.shared.templates))
            .await?;
        if let Some(tile_pkt) = self.shared.assets.pkt_tile_set_info() {
            session.send(tile_pkt).await?;
        }
        Ok(true)
    }
}
