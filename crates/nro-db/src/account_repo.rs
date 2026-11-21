//! Embedded `fjall` `account` repository remediating `MrBlue.java:65-126` SQL injection, nullable
//! `TIMESTAMP` NPEs, TOCTOU `vnd` deductions, and plaintext password storage via Argon2id.

use chrono::{DateTime, Utc};
use nro_core::AccountId;
use serde::{Deserialize, Serialize};

use crate::codec::{canonical_lookup_key, dec_i32, decode_val, enc_i32, encode_val};
use crate::engine::FjallEngine;
use crate::{DbError, DbResult};

#[path = "account_repo_auth.rs"]
mod account_repo_auth;
pub use account_repo_auth::*;

/// Full 35-field mapping of the `account` table in `database team2026.sql` & `SRC/sql/nro1.sql`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountRow {
    pub id: i32,
    pub username: String,
    pub password: String,
    pub email: String,
    pub create_time: Option<DateTime<Utc>>,
    pub update_time: Option<DateTime<Utc>>,
    pub ban: bool,
    pub is_admin: bool,
    pub last_time_login: DateTime<Utc>,
    pub last_time_logout: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub active: i32,
    pub thoi_vang: i32,
    pub server_login: i32,
    pub bd_player: Option<f64>,
    pub is_gift_box: Option<bool>,
    pub gift_time: Option<String>,
    pub reward: Option<String>,
    pub vnd: i32,
    pub tongnap: i32,
    pub token: String,
    pub xsrf_token: String,
    pub newpass: String,
    pub luotquay: i32,
    pub vang: i64,
    pub event_point: i32,
    pub vip: i32,
    pub tichdiem: i32,
    pub point_post: i32,
    pub last_post: i32,
    pub gioithieu: Option<i32>,
    pub xacnhan_gioitheu: i32,
    pub baiviet: i32,
    pub xacminh: i32,
    pub admin: i32,
}

/// Repository for `accounts` authentication, session metadata, and linearizable `vnd` billing.
#[derive(Debug, Clone)]
pub struct AccountRepo {
    pool: FjallEngine,
}

impl AccountRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self {
        Self { pool }
    }

    /// Inserts or replaces an account record and its case-insensitive `idx_account_user` key atomically.
    pub fn put_account(&self, row: &AccountRow) -> DbResult<()> {
        let pk = enc_i32(row.id);
        let uname_key = canonical_lookup_key(&row.username);
        let mut batch = self.pool.ks().db.batch();
        batch.insert(&self.pool.ks().accounts, &pk[..], encode_val("account", row)?);
        batch.insert(&self.pool.ks().idx_account_user, uname_key, &pk[..]);
        batch.commit()?;
        Ok(())
    }

    /// Allocates and persists a new local guest account (`User{id}`, empty password) for `LOGIN2`.
    pub fn create_guest_account(
        &self,
        client_ip: &str,
        now: DateTime<Utc>,
    ) -> DbResult<AccountRow> {
        let _guard = self.pool.lock_stripe(0);
        let (id, username) = loop {
            let id = self.pool.alloc_account_id();
            let username = format!("User{id}");
            if !self
                .pool
                .ks()
                .idx_account_user
                .contains_key(canonical_lookup_key(&username))?
            {
                break (id, username);
            }
        };
        let truncated_ip: String = client_ip.chars().take(50).collect();
        let row = new_account_row(id, username, String::new(), truncated_ip, now);
        self.put_account(&row)?;
        Ok(row)
    }

    /// Registers a new account or upgrades an existing guest account in-place (`REGISTER`, `-29` sub `1`).
    ///
    /// Validates non-empty `username` and `password`, rejects duplicate case-insensitive usernames
    /// with [`DbError::DuplicateKey`], hashes `password` using Argon2id, and either upgrades
    /// `guest_username` (when provided and found) or inserts a new [`AccountRow`].
    pub async fn register_account(
        &self,
        username: &str,
        password: &str,
        guest_username: Option<&str>,
        client_ip: &str,
        now: DateTime<Utc>,
    ) -> DbResult<AccountRow> {
        let trimmed_username = username.trim();
        if trimmed_username.is_empty() {
            return Err(DbError::InvalidColumnFormat {
                column: "account.username",
                reason: "username must not be empty".to_owned(),
            });
        }
        if password.is_empty() {
            return Err(DbError::InvalidColumnFormat {
                column: "account.password",
                reason: "password must not be empty".to_owned(),
            });
        }

        let new_uname_key = canonical_lookup_key(trimmed_username);
        let guest_key = guest_username
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(canonical_lookup_key);

        let mut verified_guest_pw: Option<String> = None;
        if let Some(g_key) = &guest_key
            && let Some(id_bytes) = self.pool.ks().idx_account_user.get(g_key)?
        {
            let guest_id = AccountId(dec_i32(&id_bytes)?);
            if let Some(row) = self.get_by_id(guest_id)? {
                let empty_pw = row.password.is_empty()
                    || (row.password.starts_with("$argon2")
                        && verify_or_upgrade_password(row.password.clone(), String::new())
                            .await
                            .is_ok());
                if !row.username.starts_with("User") || row.is_admin || row.ban || !empty_pw {
                    return Err(DbError::InvalidColumnFormat {
                        column: "account.username",
                        reason: "target account is not an upgradable guest account".to_owned(),
                    });
                }
                verified_guest_pw = Some(row.password);
            }
        }

        let upgrading_same_key =
            verified_guest_pw.is_some() && guest_key.as_ref() == Some(&new_uname_key);
        if !upgrading_same_key && self.pool.ks().idx_account_user.contains_key(&new_uname_key)? {
            return Err(DbError::DuplicateKey {
                entity: "account.username",
                key: trimmed_username.to_owned(),
            });
        }

        let hashed_password = hash_password(password.to_owned()).await?;
        let truncated_ip: String = client_ip.chars().take(50).collect();

        let _guard = self.pool.lock_stripe(0);

        if let Some(g_key) = &guest_key
            && let Some(id_bytes) = self.pool.ks().idx_account_user.get(g_key)?
        {
            let guest_id = AccountId(dec_i32(&id_bytes)?);
            let guest_seed = u64::from(guest_id.0.cast_unsigned());
            let _guest_guard = (guest_seed & 63 != 0).then(|| self.pool.lock_stripe(guest_seed));
            if let Some(mut row) = self.get_by_id(guest_id)? {
                let empty_pw = row.password.is_empty()
                    || verified_guest_pw.as_deref() == Some(row.password.as_str())
                    || (row.password.starts_with("$argon2")
                        && verify_or_upgrade_password_sync(&row.password, "").is_ok());
                if !row.username.starts_with("User") || row.is_admin || row.ban || !empty_pw {
                    return Err(DbError::InvalidColumnFormat {
                        column: "account.username",
                        reason: "target account is not an upgradable guest account".to_owned(),
                    });
                }
                if g_key != &new_uname_key
                    && self.pool.ks().idx_account_user.contains_key(&new_uname_key)?
                {
                    return Err(DbError::DuplicateKey {
                        entity: "account.username",
                        key: trimmed_username.to_owned(),
                    });
                }

                trimmed_username.clone_into(&mut row.username);
                row.password = hashed_password;
                row.update_time = Some(now);
                row.ip_address = Some(truncated_ip);

                let pk = enc_i32(row.id);
                let mut batch = self.pool.ks().db.batch();
                if g_key != &new_uname_key {
                    batch.remove(&self.pool.ks().idx_account_user, g_key.as_slice());
                }
                batch.insert(&self.pool.ks().accounts, &pk[..], encode_val("account", &row)?);
                batch.insert(&self.pool.ks().idx_account_user, new_uname_key, &pk[..]);
                batch.commit()?;
                return Ok(row);
            }
        }

        if self.pool.ks().idx_account_user.contains_key(&new_uname_key)? {
            return Err(DbError::DuplicateKey {
                entity: "account.username",
                key: trimmed_username.to_owned(),
            });
        }

        let id = self.pool.alloc_account_id();
        let row = new_account_row(id, trimmed_username.to_owned(), hashed_password, truncated_ip, now);
        self.put_account(&row)?;
        Ok(row)
    }

    /// Fetches an account record by primary key.
    pub fn get_by_id(&self, account_id: AccountId) -> DbResult<Option<AccountRow>> {
        self.pool
            .ks()
            .accounts
            .get(enc_i32(account_id.0))?
            .map(|bytes| decode_val("account", &bytes))
            .transpose()
    }

    pub async fn update_login_meta(
        &self,
        account_id: AccountId,
        ip_address: &str,
        login_time: DateTime<Utc>,
    ) -> DbResult<()> {
        let truncated_ip: String = ip_address.chars().take(50).collect();
        let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
        if let Some(mut row) = self.get_by_id(account_id)? {
            row.last_time_login = login_time;
            row.ip_address = Some(truncated_ip);
            self.pool
                .ks()
                .accounts
                .insert(&enc_i32(account_id.0)[..], encode_val("account", &row)?)?;
        }
        Ok(())
    }

    pub async fn update_logout_time(
        &self,
        account_id: AccountId,
        logout_time: DateTime<Utc>,
    ) -> DbResult<()> {
        let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
        if let Some(mut row) = self.get_by_id(account_id)? {
            row.last_time_logout = logout_time;
            self.pool
                .ks()
                .accounts
                .insert(&enc_i32(account_id.0)[..], encode_val("account", &row)?)?;
        }
        Ok(())
    }

    pub async fn check_logout(&self, account_id: AccountId) -> DbResult<bool> {
        Ok(self
            .get_by_id(account_id)?
            .is_some_and(|r| r.last_time_logout > r.last_time_login))
    }

    pub async fn sub_vnd(&self, account_id: AccountId, amount: i32) -> DbResult<bool> {
        if amount < 0 {
            return Ok(false);
        }
        let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
        let Some(mut row) = self.get_by_id(account_id)? else {
            return Ok(false);
        };
        if row.vnd < amount {
            return Ok(false);
        }
        row.vnd -= amount;
        self.pool
            .ks()
            .accounts
            .insert(&enc_i32(account_id.0)[..], encode_val("account", &row)?)?;
        Ok(true)
    }

    pub async fn add_vnd(&self, account_id: AccountId, amount: i64) -> DbResult<()> {
        if amount <= 0 {
            return Ok(());
        }
        let delta = i32::try_from(amount).unwrap_or(i32::MAX);
        let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
        if let Some(mut row) = self.get_by_id(account_id)? {
            row.vnd = row.vnd.saturating_add(delta);
            self.pool
                .ks()
                .accounts
                .insert(&enc_i32(account_id.0)[..], encode_val("account", &row)?)?;
        }
        Ok(())
    }

    pub async fn activate_member(
        &self,
        account_id: AccountId,
        cost_vnd: i32,
        active: bool,
    ) -> DbResult<bool> {
        if cost_vnd < 0 {
            return Ok(false);
        }
        let _guard = self.pool.lock_stripe(u64::from(account_id.0.cast_unsigned()));
        let Some(mut row) = self.get_by_id(account_id)? else {
            return Ok(false);
        };
        if row.vnd < cost_vnd {
            return Ok(false);
        }
        row.vnd -= cost_vnd;
        row.active = i32::from(active);
        self.pool
            .ks()
            .accounts
            .insert(&enc_i32(account_id.0)[..], encode_val("account", &row)?)?;
        Ok(true)
    }
}

fn new_account_row(
    id: i32,
    username: String,
    password: String,
    ip: String,
    now: DateTime<Utc>,
) -> AccountRow {
    AccountRow {
        id,
        username,
        password,
        email: String::new(),
        create_time: Some(now),
        update_time: Some(now),
        ban: false,
        is_admin: false,
        last_time_login: DateTime::UNIX_EPOCH,
        last_time_logout: DateTime::UNIX_EPOCH,
        ip_address: Some(ip),
        active: 1,
        thoi_vang: 0,
        server_login: 0,
        bd_player: Some(1.0),
        is_gift_box: None,
        gift_time: None,
        reward: None,
        vnd: 0,
        tongnap: 0,
        token: String::new(),
        xsrf_token: String::new(),
        newpass: String::new(),
        luotquay: 0,
        vang: 0,
        event_point: 0,
        vip: 0,
        tichdiem: 0,
        point_post: 0,
        last_post: 0,
        gioithieu: None,
        xacnhan_gioitheu: 0,
        baiviet: 0,
        xacminh: 0,
        admin: 0,
    }
}
