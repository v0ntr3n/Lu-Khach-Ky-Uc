//! Authentication, registration, and guest login handlers.

#[path = "router_auth_admit.rs"]
mod router_auth_admit;

use chrono::Utc;
use dashmap::{DashMap, mapref::entry::Entry};
use nro_consts::cmd_message;
use nro_core::AccountId;
use nro_db::DbError;
use nro_protocol::{PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;
use tracing::{debug, info, warn};

use super::{ServerSessionHandler, send_dialog};
use crate::data_game::DataGameAssets;
use crate::data_game_tpl;

pub(self) struct AccountReservationGuard<'a> {
    pub(self) accounts: &'a DashMap<AccountId, u64>,
    pub(self) account_id: AccountId,
    pub(self) sid: u64,
    pub(self) armed: bool,
}

impl Drop for AccountReservationGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.accounts
                .remove_if(&self.account_id, |_, s| *s == self.sid);
        }
    }
}

impl ServerSessionHandler {
    pub(self) fn try_reserve_account(
        &self,
        account_id: AccountId,
        sid: u64,
    ) -> Result<AccountReservationGuard<'_>, u64> {
        let mut already_owned = false;
        let occupied = match self.shared.clients.accounts.entry(account_id) {
            Entry::Occupied(mut occ) => {
                let old_sid = *occ.get();
                if old_sid == sid {
                    already_owned = true;
                    None
                } else if !self.shared.clients.sessions.contains_key(&old_sid)
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
            None => Ok(AccountReservationGuard {
                accounts: &self.shared.clients.accounts,
                account_id,
                sid,
                armed: !already_owned,
            }),
        }
    }

    pub(super) async fn handle_login(
        &self,
        session: &SessionHandle,
        username: &str,
        password: &str,
        login_type: i8,
    ) -> Result<(), ProtocolError> {
        if username.len() > 64 || password.len() > 128 {
            return send_dialog(session, "Tài khoản hoặc mật khẩu quá dài!");
        }
        if session.account_id().0 > 0 {
            debug!(
                sid = session.id(),
                account_id = session.account_id().0,
                "ignoring duplicate Login on authenticated session"
            );
            return Ok(());
        }
        if self.shared.maintenance.is_maintaining() {
            return send_dialog(
                session,
                "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!",
            );
        }
        if self.shared.clients.authenticated.len()
            >= self.shared.config.rate_limit.max_online_players
        {
            return send_dialog(session, "Máy chủ đã đầy, vui lòng chọn khu vực khác!");
        }
        let bypass = session.take_auth_cooldown_bypass();
        if !bypass && let Err(wait) = self.shared.rate_limiter.check_login_cooldown(session.ip()) {
            return send_dialog(
                session,
                &format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1)),
            );
        }
        let (ip_str, wait_secs) = (
            session.ip().to_string(),
            if bypass {
                0
            } else {
                self.shared.config.rate_limit.login_wait_secs
            },
        );
        let auth = match self
            .shared
            .accounts_repo
            .login_with_type(
                username,
                password,
                login_type,
                &ip_str,
                wait_secs,
                Utc::now(),
            )
            .await
        {
            Ok(acc) => acc,
            Err(DbError::InvalidCredentials) => {
                return send_dialog(session, "Tài khoản hoặc mật khẩu không chính xác!");
            }
            Err(DbError::AccountBanned { .. }) => {
                return send_dialog(session, "Tài khoản của bạn đang bị khóa!");
            }
            Err(DbError::LoginCooldown { wait_seconds }) => {
                return send_dialog(
                    session,
                    &format!("Vui lòng đợi {wait_seconds} giây trước khi đăng nhập lại."),
                );
            }
            Err(err) => {
                warn!(%username, error = %err, "database error during login");
                return send_dialog(session, "Lỗi kết nối cơ sở dữ liệu, vui lòng thử lại!");
            }
        };
        let mut guard = match self.try_reserve_account(auth.user_id, session.id()) {
            Ok(g) => g,
            Err(old_sid) => {
                self.evict_online_session(old_sid).await;
                return send_dialog(
                    session,
                    "Tài khoản đang đăng nhập ở nơi khác, vui lòng thử lại sau vài giây!",
                );
            }
        };
        self.evict_account_sessions(auth.user_id, session.id())
            .await;
        match self
            .shared
            .players_repo
            .load_by_account(auth.user_id, self.shared.config.parity_mode)
            .await
        {
            Ok(Some(snapshot)) => {
                if self
                    .admit_player_into_world(
                        session,
                        auth.user_id,
                        snapshot,
                        auth.is_admin,
                        auth.active,
                    )
                    .await?
                {
                    guard.armed = false;
                }
            }
            Ok(None) => {
                guard.armed = false;
                info!(
                    sid = session.id(),
                    account_id = auth.user_id.0,
                    "login succeeded without character; streaming version/data before CREATE_PLAYER"
                );
                session.mark_authenticated(auth.user_id, 0.into());
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
        if username.len() > 64 || password.len() > 128 {
            session
                .send(s2c_packets::register_result(
                    false,
                    "Tài khoản hoặc mật khẩu quá dài!",
                ))
                .await?;
            return Ok(());
        }
        if self.shared.maintenance.is_maintaining() {
            session
                .send(s2c_packets::register_result(
                    false,
                    "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!",
                ))
                .await?;
            return Ok(());
        }
        if let Err(wait) = self
            .shared
            .rate_limiter
            .check_account_creation_limit(session.ip())
        {
            let msg = format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1));
            session
                .send(s2c_packets::register_result(false, &msg))
                .await?;
            return Ok(());
        }
        let ip_str = session.ip().to_string();
        let trimmed_guest = guest_user.map(str::trim).filter(|s| !s.is_empty());
        let mut guest_acc_id = None;
        if let Some(guest) = trimmed_guest {
            match self.shared.accounts_repo.get_by_username(guest) {
                Ok(Some(acc)) => {
                    let ip_ok = acc
                        .ip_address
                        .as_deref()
                        .is_none_or(|ip| ip.is_empty() || ip == ip_str);
                    if session.account_id() != AccountId(acc.id) && !ip_ok {
                        session
                            .send(s2c_packets::register_result(
                                false,
                                "Phiên đăng nhập không khớp tài khoản khách!",
                            ))
                            .await?;
                        return Ok(());
                    }
                    guest_acc_id = Some(AccountId(acc.id));
                }
                Ok(None) => {
                    session
                        .send(s2c_packets::register_result(
                            false,
                            "Tài khoản ảo không tồn tại hoặc đã đăng ký!",
                        ))
                        .await?;
                    return Ok(());
                }
                Err(err) => {
                    warn!(%guest, error = %err, "database error looking up guest account");
                    session
                        .send(s2c_packets::register_result(
                            false,
                            "Đăng ký thất bại, vui lòng thử lại!",
                        ))
                        .await?;
                    return Ok(());
                }
            }
        }
        match self
            .shared
            .accounts_repo
            .register_account(username, password, trimmed_guest, &ip_str, Utc::now())
            .await
        {
            Ok(acc) => {
                let evict_acc = guest_acc_id.unwrap_or(AccountId(acc.id));
                self.evict_account_sessions(evict_acc, session.id()).await;
                info!(sid = session.id(), account_id = acc.id, username = %acc.username, "registered account");
                session.grant_auth_cooldown_bypass();
                session.send(s2c_packets::register_result(true, "")).await?;
            }
            Err(DbError::DuplicateKey { .. }) => {
                session
                    .send(s2c_packets::register_result(false, "Tài khoản đã tồn tại!"))
                    .await?;
            }
            Err(DbError::InvalidColumnFormat { .. }) => {
                session
                    .send(s2c_packets::register_result(
                        false,
                        "Tài khoản hoặc mật khẩu không hợp lệ!",
                    ))
                    .await?;
            }
            Err(err) => {
                warn!(%username, error = %err, "database error during register");
                session
                    .send(s2c_packets::register_result(
                        false,
                        "Đăng ký thất bại, vui lòng thử lại!",
                    ))
                    .await?;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_guest_login(
        &self,
        session: &SessionHandle,
    ) -> Result<(), ProtocolError> {
        if self.shared.maintenance.is_maintaining() {
            return send_dialog(
                session,
                "Máy chủ đang bảo trì định kỳ, vui lòng quay lại sau!",
            );
        }
        if let Err(wait) = self
            .shared
            .rate_limiter
            .check_login_cooldown(session.ip())
            .and_then(|()| {
                self.shared
                    .rate_limiter
                    .check_account_creation_limit(session.ip())
            })
        {
            return send_dialog(
                session,
                &format!("Vui lòng thử lại sau {} giây.", wait.as_secs().max(1)),
            );
        }
        if self.shared.clients.authenticated.len()
            >= self.shared.config.rate_limit.max_online_players
        {
            return send_dialog(session, "Máy chủ đã đầy, vui lòng chọn khu vực khác!");
        }
        let ip_str = session.ip().to_string();
        let guest = match self
            .shared
            .accounts_repo
            .create_guest_account(&ip_str, Utc::now())
        {
            Ok(acc) => acc,
            Err(err) => {
                warn!(error = %err, "failed creating local guest account");
                return send_dialog(
                    session,
                    "Không thể tạo tài khoản chơi mới, vui lòng thử lại!",
                );
            }
        };
        info!(sid = session.id(), account_id = guest.id, username = %guest.username, "created local guest account for Login2");
        session.grant_auth_cooldown_bypass();
        let mut pw = PacketWriter::with_capacity(guest.username.len() + 8);
        pw.write_utf(&guest.username)?;
        session.send(pw.finish(cmd_message::LOGIN2)).await?;
        Ok(())
    }
}
