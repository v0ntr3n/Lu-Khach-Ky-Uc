//! Password hashing/verification helpers and login authentication methods for [`AccountRepo`].

use argon2::{Argon2, PasswordHash};
use chrono::{DateTime, Utc};
use nro_core::AccountId;
use password_hash::{PasswordHasher, PasswordVerifier};
use tracing::info;

use super::AccountRepo;
use crate::codec::{canonical_lookup_key, dec_i32, enc_i32, encode_val};
use crate::{DbError, DbResult};

/// Maximum PHC password string length enforced by [`verify_or_upgrade_password`].
pub const ACCOUNT_PASSWORD_MAX_LEN: usize = 100;

/// Verified session projection returned on successful [`AccountRepo::login`] or
/// [`AccountRepo::login_with_type`].
#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedAccount {
    pub user_id: AccountId,
    pub username: String,
    pub is_admin: bool,
    pub active: bool,
    pub gold_bar: i32,
    pub luot_quay: i32,
    pub gold: i64,
    pub event_point: i32,
    pub bd_player: f64,
    pub vnd: i32,
    pub tong_nap: i32,
    pub vip: i32,
    pub create_time: DateTime<Utc>,
    pub last_time_login: DateTime<Utc>,
    pub last_time_logout: DateTime<Utc>,
    pub delta_time_secs: i32,
    pub is_new_member: bool,
    pub password_upgraded: bool,
}

/// Constant-time byte slice equality check used when comparing legacy plaintext passwords.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Hashes a plaintext password into Argon2id PHC format synchronously.
pub fn hash_password_sync(password: &str) -> DbResult<String> {
    let hashed: PasswordHash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| DbError::PasswordHash(e.to_string()))?;
    Ok(hashed.to_string())
}

/// Hashes a plaintext password into Argon2id PHC format on a CPU worker thread.
pub async fn hash_password(password: String) -> DbResult<String> {
    tokio::task::spawn_blocking(move || hash_password_sync(&password)).await?
}

/// Verifies `candidate` against `stored` (Argon2id PHC or legacy plaintext) on a CPU worker thread.
pub async fn verify_or_upgrade_password(
    stored: String,
    candidate: String,
) -> DbResult<Option<String>> {
    tokio::task::spawn_blocking(move || verify_or_upgrade_password_sync(&stored, &candidate)).await?
}

/// Synchronous Argon2id verifier / upgrader shared with CLI import tooling.
pub fn verify_or_upgrade_password_sync(
    stored: &str,
    candidate: &str,
) -> DbResult<Option<String>> {
    if stored.starts_with("$argon2") {
        let parsed = PasswordHash::new(stored).map_err(|e| DbError::PasswordHash(e.to_string()))?;
        Argon2::default()
            .verify_password(candidate.as_bytes(), &parsed)
            .map(|()| None)
            .map_err(|_| DbError::InvalidCredentials)
    } else if constant_time_eq(stored.as_bytes(), candidate.as_bytes()) {
        Ok(Some(hash_password_sync(candidate)?))
    } else {
        Err(DbError::InvalidCredentials)
    }
}

fn check_login_cooldown(
    now: DateTime<Utc>,
    last_login: DateTime<Utc>,
    last_logout: DateTime<Utc>,
    second_wait_login: u32,
) -> DbResult<()> {
    let wait_limit = i64::from(second_wait_login);
    let pass_login = (now - last_login).num_seconds().max(0);
    let pass_logout = (now - last_logout).num_seconds().max(0);
    if pass_login < wait_limit {
        let elapsed = pass_logout.min(pass_login);
        return Err(DbError::LoginCooldown {
            wait_seconds: u32::try_from(wait_limit - elapsed).unwrap_or(second_wait_login),
        });
    }
    if pass_logout < wait_limit {
        return Err(DbError::LoginCooldown {
            wait_seconds: u32::try_from(wait_limit - pass_logout).unwrap_or(second_wait_login),
        });
    }
    Ok(())
}

impl AccountRepo {
    /// Authenticates `username` + `password` with default `login_type = 0`.
    ///
    /// Delegates to [`Self::login_with_type`].
    pub async fn login(
        &self,
        username: &str,
        password: &str,
        client_ip: &str,
        second_wait_login: u32,
        now: DateTime<Utc>,
    ) -> DbResult<AuthenticatedAccount> {
        self.login_with_type(username, password, 0, client_ip, second_wait_login, now).await
    }

    /// Authenticates `username` + `password` (case-insensitive username match) with client
    /// `login_type`, upgrades legacy plaintext hashes to Argon2id PHC, enforces ban/cooldown rules,
    /// and updates login metadata.
    ///
    /// Supports the C# client guest relogin flow (`LoginScr.cs:469-473`) where subsequent guest
    /// logins send `login_type == 1` and `password == "a"` against a guest account whose stored
    /// password is empty or an Argon2id hash of `""`.
    pub async fn login_with_type(
        &self,
        username: &str,
        password: &str,
        login_type: i8,
        client_ip: &str,
        second_wait_login: u32,
        now: DateTime<Utc>,
    ) -> DbResult<AuthenticatedAccount> {
        let uname_key = canonical_lookup_key(username);
        let id_bytes = self
            .pool
            .ks()
            .idx_account_user
            .get(&uname_key)?
            .ok_or(DbError::InvalidCredentials)?;
        let account_id = AccountId(dec_i32(&id_bytes)?);
        let row = self.get_by_id(account_id)?.ok_or(DbError::InvalidCredentials)?;

        let is_guest_relogin = login_type == 1
            && password == "a"
            && (row.password.is_empty() || row.username.starts_with("User"));

        let upgraded_phc = if is_guest_relogin {
            match verify_or_upgrade_password(row.password.clone(), String::new()).await {
                Ok(upgraded) => upgraded,
                Err(DbError::InvalidCredentials) => {
                    verify_or_upgrade_password(row.password.clone(), password.to_owned()).await?
                }
                Err(err) => return Err(err),
            }
        } else {
            verify_or_upgrade_password(row.password.clone(), password.to_owned()).await?
        };

        if row.ban {
            return Err(DbError::AccountBanned {
                username: row.username,
            });
        }

        check_login_cooldown(now, row.last_time_login, row.last_time_logout, second_wait_login)?;

        let truncated_ip: String = client_ip.chars().take(50).collect();
        let (final_row, password_upgraded) = {
            let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
            let mut latest = self.get_by_id(account_id)?.ok_or(DbError::InvalidCredentials)?;
            if latest.password != row.password {
                return Err(DbError::InvalidCredentials);
            }
            if latest.ban {
                return Err(DbError::AccountBanned {
                    username: latest.username,
                });
            }
            check_login_cooldown(
                now,
                latest.last_time_login,
                latest.last_time_logout,
                second_wait_login,
            )?;
            let upgraded = if let Some(phc) = upgraded_phc {
                latest.password = phc;
                latest.update_time = Some(now);
                true
            } else {
                false
            };
            latest.last_time_login = now;
            latest.ip_address = Some(truncated_ip);
            self.pool
                .ks()
                .accounts
                .insert(&enc_i32(account_id.0)[..], encode_val("account", &latest)?)?;
            (latest, upgraded)
        };

        if password_upgraded {
            info!(account_id = account_id.0, "upgraded legacy plaintext password to Argon2id PHC");
        }

        let create_time = final_row.create_time.unwrap_or(now);
        let delta_secs = (now - create_time).num_seconds().clamp(0, i64::from(i32::MAX)) as i32;
        let is_new_member = (now - create_time).num_days() <= 35;

        Ok(AuthenticatedAccount {
            user_id: account_id,
            username: final_row.username,
            is_admin: final_row.is_admin,
            active: final_row.active != 0,
            gold_bar: final_row.thoi_vang,
            luot_quay: final_row.luotquay,
            gold: final_row.vang,
            event_point: final_row.event_point,
            bd_player: final_row.bd_player.unwrap_or(1.0),
            vnd: final_row.vnd,
            tong_nap: final_row.tongnap,
            vip: final_row.vip,
            create_time,
            last_time_login: final_row.last_time_login,
            last_time_logout: final_row.last_time_logout,
            delta_time_secs: delta_secs,
            is_new_member,
            password_upgraded,
        })
    }
}
