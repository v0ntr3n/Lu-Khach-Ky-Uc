//! Embedded `fjall` `player` repository (`load_player`, `load_by_account`, `create_player`,
//! `save_player`, and `top_leaderboard`) porting `MrBlue.java`, `PlayerDAO.java`, and `TraningDAO.java`.

pub use crate::player_model::{NewPlayerSeed, PlayerRow, PlayerSnapshot};

use nro_core::{AccountId, Location, MapId, ParityMode, PlayerId, ZoneId};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::codec::{
    canonical_lookup_key, dec_i64, decode_val, enc_i32, enc_i64, enc_leaderboard_key, encode_val,
};
use crate::engine::FjallEngine;
use crate::json_columns::{TrainingState, sanitize_login_location};
use crate::world_repos::{SuperRankRecord, SuperRankRepo};
use crate::{DbError, DbResult, gender_to_sql, is_legacy_parity};

/// Gold cap enforced by `PlayerDAO.updatePlayer` (`Inventory.LIMIT_GOLD`).
pub const LIMIT_GOLD: i64 = 200_000_000_000;

/// Leaderboard index discriminants stored as byte `0` of `idx_leaderboard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LeaderboardIndexKind {
    Power = 0,
    Task = 1,
    SuKien1 = 2,
    SuKien2 = 3,
}

/// Projected entry stored inside `idx_leaderboard` for O(limit) top-N scans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderboardEntryRecord {
    pub player_id: PlayerId,
    pub player_name: String,
    pub score: i64,
}

fn leaderboard_metrics(p: &PlayerSnapshot) -> [(LeaderboardIndexKind, u64, i64); 4] {
    let power_u64 = p.stats.power.max(0).cast_unsigned();
    let task_sort = (u64::from(p.main_task.task_id.max(0).cast_unsigned()) << 32)
        | (u64::from(p.main_task.sub_index.max(0).cast_unsigned()) << 16)
        | u64::from(p.main_task.count.max(0).cast_unsigned());
    let sk1 = i64::from(p.point_sukien1.max(0));
    let sk2 = i64::from(p.point_sukien2.max(0));
    [
        (LeaderboardIndexKind::Power, power_u64, p.stats.power),
        (LeaderboardIndexKind::Task, task_sort, i64::from(p.main_task.task_id.max(0))),
        (LeaderboardIndexKind::SuKien1, sk1.cast_unsigned(), sk1),
        (LeaderboardIndexKind::SuKien2, sk2.cast_unsigned(), sk2),
    ]
}

/// Repository managing `players` lifecycle, secondary indexes, and atomic cross-keyspace saves.
#[derive(Debug, Clone)]
pub struct PlayerRepo {
    pool: FjallEngine,
}

impl PlayerRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self {
        Self { pool }
    }

    /// Inserts an imported [`PlayerSnapshot`] alongside its account, name, and leaderboard indexes.
    pub fn upsert_imported_player(&self, snap: &PlayerSnapshot) -> DbResult<()> {
        let pid_key = enc_i64(snap.id.0);
        let mut batch = self.pool.ks().db.batch();
        batch.insert(&self.pool.ks().players, &pid_key[..], encode_val("player", snap)?);
        if let Some(acc) = snap.account_id {
            batch.insert(&self.pool.ks().idx_player_account, &enc_i32(acc.0)[..], &pid_key[..]);
        }
        batch.insert(&self.pool.ks().idx_player_name, canonical_lookup_key(&snap.name), &pid_key[..]);
        for (kind, sort_score, display_score) in leaderboard_metrics(snap) {
            let rec = LeaderboardEntryRecord {
                player_id: snap.id,
                player_name: snap.name.clone(),
                score: display_score,
            };
            batch.insert(
                &self.pool.ks().idx_leaderboard,
                &enc_leaderboard_key(kind as u8, sort_score, snap.id.0)[..],
                encode_val("idx_leaderboard", &rec)?,
            );
        }
        batch.commit()?;
        Ok(())
    }

    pub async fn load_by_account(
        &self,
        account_id: AccountId,
        mode: ParityMode,
    ) -> DbResult<Option<PlayerSnapshot>> {
        let Some(pid_bytes) = self.pool.ks().idx_player_account.get(enc_i32(account_id.0))? else {
            return Ok(None);
        };
        let pid = PlayerId(dec_i64(&pid_bytes)?);
        self.load_player(pid, mode, false).await.map(Some)
    }

    pub async fn load_player(
        &self,
        player_id: PlayerId,
        mode: ParityMode,
        is_offline: bool,
    ) -> DbResult<PlayerSnapshot> {
        let raw = self
            .pool
            .ks()
            .players
            .get(enc_i64(player_id.0))?
            .ok_or_else(|| DbError::NotFound {
                entity: "player",
                id: player_id.to_string(),
            })?;
        let mut snap: PlayerSnapshot = decode_val("player", &raw)?;
        if !is_offline {
            snap.location = sanitize_login_location(snap.location, snap.gender);
        }
        if is_legacy_parity(mode) {
            snap.wallet.event = 0;
        }
        Ok(snap)
    }

    pub async fn create_player(&self, seed: &NewPlayerSeed) -> DbResult<PlayerId> {
        let name_key = canonical_lookup_key(&seed.name);
        let acc_key = enc_i32(seed.account_id.0);
        let _guard = self.pool.lock_stripe(0);
        if self.pool.ks().idx_player_name.contains_key(&name_key)? {
            return Err(DbError::DuplicateKey {
                entity: "player.name",
                key: seed.name.clone(),
            });
        }
        if self.pool.ks().idx_player_account.contains_key(acc_key)? {
            return Err(DbError::DuplicateKey {
                entity: "player.account_id",
                key: seed.account_id.to_string(),
            });
        }

        let pid = PlayerId(self.pool.alloc_player_id());
        let snap = PlayerSnapshot::from_seed(pid, seed);
        let pid_key = enc_i64(pid.0);

        let mut batch = self.pool.ks().db.batch();
        batch.insert(&self.pool.ks().players, &pid_key[..], encode_val("player", &snap)?);
        batch.insert(&self.pool.ks().idx_player_account, &acc_key[..], &pid_key[..]);
        batch.insert(&self.pool.ks().idx_player_name, name_key, &pid_key[..]);
        for (kind, sort_score, display_score) in leaderboard_metrics(&snap) {
            let rec = LeaderboardEntryRecord {
                player_id: pid,
                player_name: snap.name.clone(),
                score: display_score,
            };
            batch.insert(
                &self.pool.ks().idx_leaderboard,
                &enc_leaderboard_key(kind as u8, sort_score, pid.0)[..],
                encode_val("idx_leaderboard", &rec)?,
            );
        }
        batch.commit()?;
        Ok(pid)
    }

    pub async fn save_player(
        &self,
        p: &PlayerSnapshot,
        super_rank: Option<&SuperRankRecord>,
        mode: ParityMode,
        now_ms: i64,
    ) -> DbResult<()> {
        let mut normalized = p.clone();
        normalized.wallet.gold = normalized.wallet.gold.min(LIMIT_GOLD);
        if normalized.stats.hp <= 0 {
            normalized.location = Location {
                map_id: MapId(21 + i16::from(gender_to_sql(normalized.gender))),
                zone_id: ZoneId(0),
                x: 300,
                y: 336,
            };
            normalized.stats.hp = 1;
            normalized.stats.mp = 1;
        }
        if is_legacy_parity(mode) {
            for slot in &mut normalized.item_event_counters[6..] {
                *slot = 0;
            }
        }
        normalized.training.last_time_offline_ms = now_ms;

        let pid_key = enc_i64(normalized.id.0);
        let _guard = self.pool.lock_stripe(normalized.id.0.cast_unsigned());
        let prev_snap: Option<PlayerSnapshot> = self
            .pool
            .ks()
            .players
            .get(pid_key)?
            .map(|b| decode_val("player", &b))
            .transpose()?;

        let mut batch = self.pool.ks().db.batch();
        batch.insert(&self.pool.ks().players, &pid_key[..], encode_val("player", &normalized)?);

        let new_lb = leaderboard_metrics(&normalized);
        if let Some(prev) = &prev_snap {
            for ((kind, old_sort, _), (_, new_sort, _)) in
                leaderboard_metrics(prev).into_iter().zip(new_lb)
            {
                if old_sort != new_sort || prev.name != normalized.name {
                    batch.remove(
                        &self.pool.ks().idx_leaderboard,
                        &enc_leaderboard_key(kind as u8, old_sort, normalized.id.0)[..],
                    );
                }
            }
        }
        for (kind, sort_score, display_score) in new_lb {
            let rec = LeaderboardEntryRecord {
                player_id: normalized.id,
                player_name: normalized.name.clone(),
                score: display_score,
            };
            batch.insert(
                &self.pool.ks().idx_leaderboard,
                &enc_leaderboard_key(kind as u8, sort_score, normalized.id.0)[..],
                encode_val("idx_leaderboard", &rec)?,
            );
        }

        if let Some(sr) = super_rank {
            let (sr_pk, sr_bytes, old_idx, new_idx) =
                SuperRankRepo::prepare_upsert_ops(&self.pool, sr)?;
            if let Some(old_k) = old_idx {
                batch.remove(&self.pool.ks().idx_super_rank, &old_k[..]);
            }
            batch.insert(&self.pool.ks().super_rank, &sr_pk[..], sr_bytes);
            if let Some(new_k) = new_idx {
                batch.insert(&self.pool.ks().idx_super_rank, &new_k[..], &[][..]);
            }
        }
        batch.commit()?;
        debug!(player_id = normalized.id.0, "saved player snapshot atomically to fjall");
        Ok(())
    }

    pub async fn save_training_state(
        &self,
        player_id: PlayerId,
        training: &TrainingState,
    ) -> DbResult<()> {
        let pid_key = enc_i64(player_id.0);
        let _guard = self.pool.lock_stripe(player_id.0.cast_unsigned());
        if let Some(raw) = self.pool.ks().players.get(pid_key)? {
            let mut snap: PlayerSnapshot = decode_val("player", &raw)?;
            snap.training = *training;
            self.pool.ks().players.insert(&pid_key[..], encode_val("player", &snap)?)?;
        }
        Ok(())
    }

    /// Reads the top `limit` entries for `kind` in O(limit) via descending prefix scan.
    pub fn top_leaderboard(
        &self,
        kind: LeaderboardIndexKind,
        limit: usize,
    ) -> DbResult<Vec<LeaderboardEntryRecord>> {
        let mut out = Vec::with_capacity(limit.min(128));
        for guard in self.pool.ks().idx_leaderboard.prefix([kind as u8]).take(limit) {
            let bytes = guard.value()?;
            out.push(decode_val("idx_leaderboard", &bytes)?);
        }
        Ok(out)
    }
}
