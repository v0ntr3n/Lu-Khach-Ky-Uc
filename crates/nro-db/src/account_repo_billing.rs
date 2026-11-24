//! Session timestamp metadata and atomic `vnd` / activation operations for [`AccountRepo`].

use chrono::{DateTime, Utc};
use nro_core::AccountId;

use super::AccountRepo;
use crate::DbResult;
use crate::codec::{enc_i32, encode_val};

impl AccountRepo {
    pub async fn update_login_meta(
        &self,
        account_id: AccountId,
        ip_address: &str,
        login_time: DateTime<Utc>,
    ) -> DbResult<()> {
        let truncated_ip: String = ip_address.trim().chars().take(50).collect();
        let _guard = self
            .pool
            .lock_stripe(u64::from(account_id.0.cast_unsigned()));
        if let Some(mut row) = self.get_by_id(account_id)? {
            row.last_time_login = login_time;
            if !truncated_ip.is_empty() {
                row.ip_address = Some(truncated_ip);
            }
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
        let _guard = self
            .pool
            .lock_stripe(u64::from(account_id.0.cast_unsigned()));
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
        let _guard = self
            .pool
            .lock_stripe(u64::from(account_id.0.cast_unsigned()));
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
        let _guard = self
            .pool
            .lock_stripe(u64::from(account_id.0.cast_unsigned()));
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
        let _guard = self
            .pool
            .lock_stripe(u64::from(account_id.0.cast_unsigned()));
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
