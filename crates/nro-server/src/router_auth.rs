//! Authentication, registration, character creation, and world admission handlers.

use chrono::Utc;
use dashmap::{DashMap, mapref::entry::Entry};
use nro_consts::cmd_message;
use nro_core::{AccountId, Gender, Location, MapId, ZoneId};
use nro_db::DbError;
use nro_db::player_repo::{NewPlayerSeed, PlayerSnapshot};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::{PlayerIngress, ZoneCommand, s2c_packets};
use slotmap::SlotMap;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use super::{ServerSessionHandler, effective_appearance, recalculate_snapshot_stats, send_dialog};
use crate::data_game::DataGameAssets;
use crate::data_game_tpl;
use crate::runtime::{ActivePlayerEntry, LeaderboardSlot, unix_now_ms};

struct AccountReservationGuard<'a> {
    accounts: &'a DashMap<AccountId, u64>,
    account_id: AccountId,
    sid: u64,
    armed: bool,
}

impl Drop for AccountReservationGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.accounts.remove_if(&self.account_id, |_, s| *s == self.sid);
        }
    }
}

impl ServerSessionHandler {
    fn try_reserve_account(&self, account_id: AccountId, sid: u64) -> Result<AccountReservationGuard<'_>, u64> {
        let occupied = match self.shared.clients.accounts.entry(account_id) {
            Entry::Occupied(mut occ) => {
                let old_sid = *occ.get();
                if !self.shared.clients.sessions.contains_key(&old_sid)
                    && !self.shared.clients.authenticated.contains_key(&old_sid)
                {
                    occ.insert(sid);
                    None
                } else {
                    Some(old_sid)
                }
            }
            Entry::Vacant(vac) => {
                vac.insert(sid);
                None
            }
        };
        match occupied {
            Some(old_sid) => Err(old_sid),
            None => Ok(AccountReservationGuard { accounts: &self.shared.clients.accounts, account_id, sid, armed: true }),
        }
    }

    pub(super) async fn handle_login(
        &self,
        session: &SessionHandle,
        username: &str,
        password: &str,
        login_type: i8,
    ) -> Result<(), ProtocolError> {
        if session.account_id().0 > 0 {
            debug!(sid = session.id(), account_id = session.account_id().0, "ignoring duplicate Login on authenticated session");
            return Ok(());
        }
        if self.shared.maintenance.is_maintaining() {
            return send_dialog(session, "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!");
        }
        if let Err(wait) = self.shared.rate_limiter.check_login_cooldown(session.ip()) {
            return send_dialog(session, &format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1)));
        }
        if self.shared.clients.authenticated.len() >= self.shared.config.rate_limit.max_online_players {
            return send_dialog(session, "Máy chủ đã đầy, vui lòng chọn khu vực khác!");
        }
        let ip_str = session.ip().to_string();
        let wait_secs = self.shared.config.rate_limit.login_wait_secs;
        let auth = match self
            .shared
            .accounts_repo
            .login_with_type(username, password, login_type, &ip_str, wait_secs, Utc::now())
            .await
        {
            Ok(acc) => acc,
            Err(DbError::InvalidCredentials) => return send_dialog(session, "Tài khoản hoặc mật khẩu không chính xác!"),
            Err(DbError::AccountBanned { .. }) => return send_dialog(session, "Tài khoản của bạn đang bị khóa!"),
            Err(DbError::LoginCooldown { wait_seconds }) => {
                return send_dialog(session, &format!("Vui lòng đợi {wait_seconds} giây trước khi đăng nhập lại."));
            }
            Err(err) => {
                warn!(%username, error = %err, "database error during login");
                return send_dialog(session, "Lỗi kết nối cơ sở dữ liệu, vui lòng thử lại!");
            }
        };
        let mut guard = match self.try_reserve_account(auth.user_id, session.id()) {
            Ok(g) => g,
            Err(old_sid) => {
                if let Some(h) = self.shared.clients.sessions.get(&old_sid) {
                    h.disconnect();
                }
                return send_dialog(session, "Tài khoản đang đăng nhập ở nơi khác, vui lòng thử lại sau vài giây!");
            }
        };
        match self.shared.players_repo.load_by_account(auth.user_id, self.shared.config.parity_mode).await {
            Ok(Some(snapshot)) => {
                if self.admit_player_into_world(session, auth.user_id, snapshot, auth.is_admin, auth.active).await? {
                    guard.armed = false;
                }
            }
            Ok(None) => {
                drop(guard);
                info!(sid = session.id(), account_id = auth.user_id.0, "login succeeded without character; streaming version/data before CREATE_PLAYER");
                session.mark_authenticated(auth.user_id, 0.into());
                session.send(self.shared.assets.pkt_small_version_for_zoom(session.zoom_level())).await?;
                session.send(self.shared.assets.pkt_bg_item_version_for_zoom(session.zoom_level())).await?;
                session.send(DataGameAssets::pkt_version_game()).await?;
                session.send(data_game_tpl::pkt_data_item_bg(&self.shared.templates)).await?;
                if let Some(tile_pkt) = self.shared.assets.pkt_tile_set_info() {
                    session.send(tile_pkt).await?;
                }
            }
            Err(err) => {
                warn!(account_id = auth.user_id.0, error = %err, "failed loading player snapshot");
                send_dialog(session, "Không thể tải nhân vật, vui lòng liên hệ Admin!")?;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_register(
        &self,
        session: &SessionHandle,
        username: &str,
        password: &str,
        guest_user: Option<&str>,
    ) -> Result<(), ProtocolError> {
        if self.shared.maintenance.is_maintaining() {
            session.send(s2c_packets::register_result(false, "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!")).await?;
            return Ok(());
        }
        if let Err(wait) = self.shared.rate_limiter.check_login_cooldown(session.ip()) {
            let msg = format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1));
            session.send(s2c_packets::register_result(false, &msg)).await?;
            return Ok(());
        }
        let ip_str = session.ip().to_string();
        match self.shared.accounts_repo.register_account(username, password, guest_user, &ip_str, Utc::now()).await {
            Ok(acc) => {
                info!(sid = session.id(), account_id = acc.id, username = %acc.username, "registered account");
                self.shared.rate_limiter.clear_login_cooldown(session.ip());
                session.send(s2c_packets::register_result(true, "")).await?;
            }
            Err(DbError::DuplicateKey { .. }) => {
                session.send(s2c_packets::register_result(false, "Tài khoản đã tồn tại!")).await?;
            }
            Err(DbError::InvalidColumnFormat { .. }) => {
                session.send(s2c_packets::register_result(false, "Tài khoản hoặc mật khẩu không hợp lệ!")).await?;
            }
            Err(err) => {
                warn!(%username, error = %err, "database error during register");
                session.send(s2c_packets::register_result(false, "Đăng ký thất bại, vui lòng thử lại!")).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_guest_login(&self, session: &SessionHandle) -> Result<(), ProtocolError> {
        if self.shared.maintenance.is_maintaining() {
            return send_dialog(session, "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!");
        }
        if let Err(wait) = self.shared.rate_limiter.check_login_cooldown(session.ip()) {
            return send_dialog(session, &format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1)));
        }
        if self.shared.clients.authenticated.len() >= self.shared.config.rate_limit.max_online_players {
            return send_dialog(session, "Máy chủ đã đầy, vui lòng chọn khu vực khác!");
        }
        let ip_str = session.ip().to_string();
        let guest = match self.shared.accounts_repo.create_guest_account(&ip_str, Utc::now()) {
            Ok(acc) => acc,
            Err(err) => {
                warn!(error = %err, "failed creating local guest account");
                return send_dialog(session, "Không thể tạo tài khoản chơi mới, vui lòng thử lại!");
            }
        };
        info!(sid = session.id(), account_id = guest.id, username = %guest.username, "created local guest account for Login2");
        self.shared.rate_limiter.clear_login_cooldown(session.ip());
        let mut pw = PacketWriter::with_capacity(guest.username.len() + 8);
        pw.write_utf(&guest.username)?;
        session.send(pw.finish(cmd_message::LOGIN2)).await?;
        Ok(())
    }

    pub(super) async fn handle_create_char(
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
            return send_dialog(session, "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!");
        }
        if account_id.0 <= 0 {
            return send_dialog(session, "Phiên đăng nhập chưa xác thực, vui lòng đăng nhập lại!");
        }
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 15 {
            return send_dialog(session, "Tên nhân vật không hợp lệ (1-15 ký tự)!");
        }
        let mut guard = match self.try_reserve_account(account_id, session.id()) {
            Ok(g) => g,
            Err(old_sid) => {
                if old_sid != session.id() && let Some(h) = self.shared.clients.sessions.get(&old_sid) {
                    h.disconnect();
                }
                return send_dialog(session, "Tài khoản đang đăng nhập ở nơi khác, vui lòng thử lại sau vài giây!");
            }
        };
        let seed = NewPlayerSeed {
            account_id,
            name: trimmed.to_owned(),
            gender: Gender::from_i8_lossy(gender_raw),
            hair: i16::from(hair),
            test_mode: false,
            now_ms: unix_now_ms(),
        };
        match self.shared.players_repo.create_player(&seed).await {
            Ok(_pid) => match self.shared.players_repo.load_by_account(account_id, self.shared.config.parity_mode).await {
                Ok(Some(snap)) => {
                    let (is_admin, activated) = match self.shared.accounts_repo.get_by_id(account_id) {
                        Ok(Some(acc)) => (acc.is_admin, acc.active != 0),
                        _ => (false, false),
                    };
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Task);
                    if self.admit_player_into_world(session, account_id, snap, is_admin, activated).await? {
                        guard.armed = false;
                    }
                }
                Ok(None) => {
                    warn!(account_id = account_id.0, "created player missing on immediate reload");
                    send_dialog(session, "Không thể tải nhân vật vừa tạo, vui lòng đăng nhập lại!")?;
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

    async fn admit_player_into_world(
        &self,
        session: &SessionHandle,
        account_id: AccountId,
        mut snap: PlayerSnapshot,
        is_admin: bool,
        activated: bool,
    ) -> Result<bool, ProtocolError> {
        recalculate_snapshot_stats(&mut snap, self.shared.config.parity_mode);
        let (eff_head, eff_body, eff_leg, _, _) = effective_appearance(&self.shared.templates, &snap);

        session.send(self.shared.assets.pkt_small_version_for_zoom(session.zoom_level())).await?;
        session.send(self.shared.assets.pkt_bg_item_version_for_zoom(session.zoom_level())).await?;
        session.send(DataGameAssets::pkt_version_game()).await?;
        session.send(data_game_tpl::pkt_data_item_bg(&self.shared.templates)).await?;
        if let Some(tile_pkt) = self.shared.assets.pkt_tile_set_info() {
            session.send(tile_pkt).await?;
        }

        let mut sm: SlotMap<nro_core::SessionKey, ()> = SlotMap::with_key();
        let session_key = sm.insert(());
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Packet>(256);

        let zones_count = self.shared.templates.maps.iter()
            .find(|m| m.id == snap.location.map_id)
            .map_or(1_u16, |m| u16::from(m.zones.max(1)));
        let start_z = snap.location.zone_id.0 % zones_count;

        let mut entity_key = None;
        for offset in 0..zones_count {
            let z = ZoneId((start_z + offset) % zones_count);
            let cand_loc = Location { zone_id: z, ..snap.location };
            if let Some(ek) = self
                .try_enter_zone(account_id, session_key, &snap, cand_loc, (eff_head, eff_body, eff_leg), outbound_tx.clone())
                .await
            {
                snap.location = cand_loc;
                entity_key = Some(ek);
                break;
            }
        }
        if entity_key.is_none() {
            let home_map = MapId(21 + i16::from(u8::from(snap.gender)));
            let home_loc = Location { map_id: home_map, zone_id: ZoneId(0), x: 300, y: 336 };
            if let Some(ek) = self
                .try_enter_zone(account_id, session_key, &snap, home_loc, (eff_head, eff_body, eff_leg), outbound_tx.clone())
                .await
            {
                snap.location = home_loc;
                entity_key = Some(ek);
            }
        }
        let Some(ek) = entity_key else {
            warn!(sid = session.id(), account_id = account_id.0, map_id = snap.location.map_id.0, "failed admitting player into any zone; rejecting login");
            send_dialog(session, "Tất cả khu vực đều đang đầy, vui lòng thử lại sau!")?;
            return Ok(false);
        };
        drop(outbound_tx);
        let fwd_session = session.clone();
        tokio::spawn(async move {
            while let Some(pkt) = outbound_rx.recv().await {
                if fwd_session.send(pkt).await.is_err() {
                    break;
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
            handle: session.clone(), session_key, entity_key: Some(ek), snapshot: snap,
            is_admin, activated, selected_skill, last_pea_ms: 0, c_flag: 0,
        };
        if let Some(evicted) = self.shared.clients.bind_authenticated(session.id(), account_id, entry) {
            let (ev_pid, ev_loc) = (evicted.snapshot.id, evicted.snapshot.location);
            self.cleanup_trade_on_disconnect(ev_pid);
            self.cleanup_combat_on_disconnect(ev_pid, ev_loc, 0);
            super::router_skill::clear_skill_state(ev_pid);
            super::router_secondary::clear_clan_invites(ev_pid);
            if (ev_loc.map_id, ev_loc.zone_id) != (new_loc.map_id, new_loc.zone_id)
                && let Ok(old_tx) = self.shared.world_router.resolve_sender(ev_loc.map_id, ev_loc.zone_id)
            {
                let _ = old_tx.send(ZoneCommand::LeavePlayer { player_id: ev_pid }).await;
            }
        }
        Ok(true)
    }

    async fn try_enter_zone(
        &self,
        account_id: AccountId,
        session_key: nro_core::SessionKey,
        snap: &PlayerSnapshot,
        loc: Location,
        parts: (i16, i16, i16),
        outbound_tx: mpsc::Sender<Packet>,
    ) -> Option<nro_core::EntityKey> {
        let zone_tx = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id).ok()?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let ingress = PlayerIngress {
            session: (account_id, session_key),
            identity: (snap.id, (snap.clan_id.0 >= 0).then_some(snap.clan_id)),
            gender: snap.gender,
            name: snap.name.clone(),
            loc,
            stats: snap.stats,
            parts,
            ship_type: 0,
            outbound_tx: Some(outbound_tx),
        };
        if zone_tx.send(ZoneCommand::EnterPlayer(Box::new(ingress), reply_tx)).await.is_ok()
            && let Ok(Ok(ek)) = reply_rx.await
        {
            return Some(ek);
        }
        None
    }
}
