//! Embedded `fjall` repositories for `clans`, `consign_shop`, `super_rank`, `giftcodes`,
//! `history_tx`, and `events`, plus re-export of [`crate::templates::GameTemplates`].

pub use crate::templates::*;

use chrono::{DateTime, Duration, Utc};
use nro_core::{ClanId, ItemOption, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::codec::{
    dec_i32, decode_val, enc_history_tx_key, enc_i32, enc_i64, enc_super_rank_key, encode_val,
};
use crate::engine::{FjallEngine, WOMENS_DAY_EVENT_KEY};
use crate::{DbResult, JsonStr, StrNum, UnquotedLeaderInfo, parse_lenient_json_vec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClanMemberRecord {
    pub id: i32, pub name: String, pub head: i16, pub body: i16, pub leg: i16, pub role: i8,
    pub donate: i32, pub receive_donate: i32, pub member_point: i32, pub clan_point: i32,
    pub join_time: i32, pub ask_pea_time: i64, #[serde(default)] pub power: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClanRecord {
    pub id: ClanId, pub name: String, pub name_2: String, pub slogan: String, pub img_id: i8,
    pub power_point: i64, pub max_member: i8, pub clan_point: i32, pub level: i8,
    pub members: Vec<ClanMemberRecord>, pub create_time: DateTime<Utc>,
    pub bdkb_level_done: i32, pub bdkb_duration_ms: i64, pub leader_info: UnquotedLeaderInfo,
}

#[derive(Debug, Clone)]
pub struct ClanRepo { pool: FjallEngine }

impl ClanRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    pub async fn load_all(&self) -> DbResult<Vec<ClanRecord>> {
        let mut out = Vec::new();
        for guard in self.pool.ks().clans.iter() {
            let bytes = guard.value()?;
            out.push(decode_val("clan", &bytes)?);
        }
        Ok(out)
    }

    pub async fn next_clan_id(&self) -> DbResult<ClanId> {
        Ok(ClanId(self.pool.alloc_clan_id()))
    }

    pub async fn insert_clan(&self, c: &ClanRecord) -> DbResult<()> {
        self.pool.ks().clans.insert(&enc_i32(c.id.0)[..], encode_val("clan", c)?)?;
        self.pool.bump_sequences(0, 0, c.id.0.saturating_add(1));
        Ok(())
    }

    pub async fn update_clan(&self, c: &ClanRecord) -> DbResult<()> {
        let _guard = self.pool.lock_stripe(u64::from(c.id.0.cast_unsigned()));
        self.pool.ks().clans.insert(&enc_i32(c.id.0)[..], encode_val("clan", c)?)?;
        Ok(())
    }

    pub async fn delete_clan(&self, clan_id: ClanId) -> DbResult<()> {
        let _guard = self.pool.lock_stripe(u64::from(clan_id.0.cast_unsigned()));
        self.pool.ks().clans.remove(&enc_i32(clan_id.0)[..])?;
        Ok(())
    }

    pub async fn update_bdkb_record(&self, clan_id: ClanId, level: i32, duration_ms: i64) -> DbResult<()> {
        let key = enc_i32(clan_id.0);
        let _guard = self.pool.lock_stripe(u64::from(clan_id.0.cast_unsigned()));
        if let Some(raw) = self.pool.ks().clans.get(key)? {
            let mut clan: ClanRecord = decode_val("clan", &raw)?;
            clan.bdkb_level_done = level;
            clan.bdkb_duration_ms = duration_ms;
            self.pool.ks().clans.insert(&key[..], encode_val("clan", &clan)?)?;
        }
        Ok(())
    }

    pub async fn update_leader_info(&self, clan_id: ClanId, leader: &UnquotedLeaderInfo) -> DbResult<()> {
        let key = enc_i32(clan_id.0);
        let _guard = self.pool.lock_stripe(u64::from(clan_id.0.cast_unsigned()));
        if let Some(raw) = self.pool.ks().clans.get(key)? {
            let mut clan: ClanRecord = decode_val("clan", &raw)?;
            clan.leader_info = leader.clone();
            self.pool.ks().clans.insert(&key[..], encode_val("clan", &clan)?)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct ConsignOptWire { pub id: StrNum<i32>, pub param: StrNum<i32> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsignItemRecord {
    pub id: i32, pub player_id: i32, pub tab: i8, pub item_id: TemplateId, pub gold: i32,
    pub gem: i32, pub quantity: i32, pub options: SmallVec<[ItemOption; 4]>,
    pub is_up_top: i8, pub is_buy: bool,
}

#[derive(Debug, Clone)]
pub struct ConsignShopRepo { pool: FjallEngine }

impl ConsignShopRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    pub async fn load_all(&self) -> DbResult<Vec<ConsignItemRecord>> {
        let mut out = Vec::new();
        for guard in self.pool.ks().consign_shop.iter() {
            let bytes = guard.value()?;
            out.push(decode_val("consign_shop", &bytes)?);
        }
        Ok(out)
    }

    pub async fn save_all(&self, items: &[ConsignItemRecord]) -> DbResult<()> {
        let _guard = self.pool.lock_stripe(1);
        let mut batch = self.pool.ks().db.batch();
        for guard in self.pool.ks().consign_shop.iter() {
            let k = guard.key()?;
            batch.remove(&self.pool.ks().consign_shop, k);
        }
        for it in items {
            batch.insert(&self.pool.ks().consign_shop, &enc_i32(it.id)[..], encode_val("consign_shop", it)?);
        }
        batch.commit()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SuperRankInfo {
    pub head: i32, pub body: i32, pub leg: i32, pub hp: i64, pub dame: i64, pub def: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperRankHistoryItem { pub event: String, pub timestamp: i64 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperRankRecord {
    pub player_id: PlayerId, pub name: String, pub rank: i32, pub last_pk_time_ms: i64,
    pub last_reward_time_ms: i64, pub ticket: i32, pub win: i32, pub lose: i32,
    pub history: Vec<SuperRankHistoryItem>, pub info: SuperRankInfo,
}

#[derive(Debug, Clone)]
pub struct SuperRankRepo { pool: FjallEngine }

impl SuperRankRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decode_sql_fields(
        player_id: i32, name: String, rank: i32, last_pk_time: i64, last_reward_time: i64,
        ticket: i32, win: i32, lose: i32, raw_history: &str, raw_info: &str,
    ) -> DbResult<SuperRankRecord> {
        let history = parse_lenient_json_vec::<JsonStr<SuperRankHistoryItem>>("history", raw_history)?
            .into_iter().map(|h| h.0).collect();
        let info = serde_json::from_str::<SuperRankInfo>(raw_info).unwrap_or_default();
        Ok(SuperRankRecord {
            player_id: PlayerId::from(player_id), name, rank,
            last_pk_time_ms: last_pk_time, last_reward_time_ms: last_reward_time,
            ticket, win, lose, history, info,
        })
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn prepare_upsert_ops(
        pool: &FjallEngine,
        sr: &SuperRankRecord,
    ) -> DbResult<([u8; 8], Vec<u8>, Option<[u8; 12]>, Option<[u8; 12]>)> {
        let pk = enc_i64(sr.player_id.0);
        let old_idx = if let Some(prev_bytes) = pool.ks().super_rank.get(pk)? {
            let prev: SuperRankRecord = decode_val("super_rank", &prev_bytes)?;
            (prev.rank != sr.rank && prev.rank > 0)
                .then(|| enc_super_rank_key(prev.rank, sr.player_id.0))
        } else {
            None
        };
        let new_idx = (sr.rank > 0).then(|| enc_super_rank_key(sr.rank, sr.player_id.0));
        Ok((pk, encode_val("super_rank", sr)?, old_idx, new_idx))
    }

    pub async fn upsert(&self, sr: &SuperRankRecord) -> DbResult<()> {
        let _guard = self.pool.lock_stripe(sr.player_id.0.cast_unsigned());
        let (pk, bytes, old_idx, new_idx) = Self::prepare_upsert_ops(&self.pool, sr)?;
        let mut batch = self.pool.ks().db.batch();
        if let Some(old_k) = old_idx { batch.remove(&self.pool.ks().idx_super_rank, &old_k[..]); }
        batch.insert(&self.pool.ks().super_rank, &pk[..], bytes);
        if let Some(new_k) = new_idx { batch.insert(&self.pool.ks().idx_super_rank, &new_k[..], &[][..]); }
        batch.commit()?;
        Ok(())
    }

    pub async fn load_by_player(&self, player_id: PlayerId) -> DbResult<Option<SuperRankRecord>> {
        self.pool.ks().super_rank.get(enc_i64(player_id.0))?
            .map(|b| decode_val("super_rank", &b))
            .transpose()
    }

    pub async fn list_in_rank_range(&self, rank: i32, limit: i32) -> DbResult<Vec<SuperRankRecord>> {
        if limit <= 0 { return Ok(Vec::new()); }
        let lower = enc_super_rank_key(1, i64::MIN);
        let upper = enc_super_rank_key(rank.max(10), i64::MAX);
        let mut out = Vec::with_capacity(limit.min(64) as usize);
        for guard in self.pool.ks().idx_super_rank.range(lower..=upper).rev().take(limit as usize) {
            let idx_key = guard.key()?;
            if let Some(val) = self.pool.ks().super_rank.get(&idx_key[4..12])? {
                out.push(decode_val("super_rank", &val)?);
            }
        }
        Ok(out)
    }

    pub async fn highest_rank(&self) -> DbResult<i32> {
        match self.pool.ks().idx_super_rank.last_key_value() {
            Some(guard) => dec_i32(&guard.key()?[..4]),
            None => Ok(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumericOptionJson { pub id: i32, pub param: i32 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftCodeItem {
    pub id: i32, pub quantity: i32, #[serde(default)] pub options: Vec<NumericOptionJson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftCodeRecord {
    pub id: i32, pub code: String, pub count_left: i32, pub items: Vec<GiftCodeItem>,
    pub date_created: DateTime<Utc>, pub expired: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GiftCodeRepo { pool: FjallEngine }

impl GiftCodeRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    pub fn insert_raw(&self, record: &GiftCodeRecord) -> DbResult<()> {
        self.pool.ks().giftcodes.insert(&enc_i32(record.id)[..], encode_val("giftcode", record)?)?;
        Ok(())
    }

    pub async fn load_all(&self) -> DbResult<Vec<GiftCodeRecord>> {
        let mut out = Vec::new();
        for guard in self.pool.ks().giftcodes.iter() {
            let mut rec: GiftCodeRecord = decode_val("giftcode", &guard.value()?)?;
            if rec.count_left == -1 { rec.count_left = 999_999_999; }
            out.push(rec);
        }
        Ok(out)
    }

    pub async fn redeem_decrement(&self, id: i32) -> DbResult<bool> {
        let key = enc_i32(id);
        let _guard = self.pool.lock_stripe(u64::from(id.cast_unsigned()));
        let Some(bytes) = self.pool.ks().giftcodes.get(key)? else { return Ok(false); };
        let mut rec: GiftCodeRecord = decode_val("giftcode", &bytes)?;
        if rec.count_left == -1 {
            return Ok(true);
        }
        if rec.count_left <= 0 {
            return Ok(false);
        }
        rec.count_left -= 1;
        self.pool.ks().giftcodes.insert(&key[..], encode_val("giftcode", &rec)?)?;
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct TradeAuditEntry<'a> {
    pub player_1: &'a str, pub player_2: &'a str, pub item_player_1: &'a str,
    pub item_player_2: &'a str, pub bag_1_before: &'a str, pub bag_2_before: &'a str,
    pub bag_1_after: &'a str, pub bag_2_after: &'a str, pub time_tran: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTradeAudit {
    pub player_1: String, pub player_2: String, pub item_player_1: String,
    pub item_player_2: String, pub bag_1_before: String, pub bag_2_before: String,
    pub bag_1_after: String, pub bag_2_after: String, pub time_tran_ms: i64,
}

#[derive(Debug, Clone)]
pub struct HistoryTransactionRepo { pool: FjallEngine }

impl HistoryTransactionRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    pub async fn insert(&self, e: &TradeAuditEntry<'_>) -> DbResult<()> {
        let ts_ms = e.time_tran.timestamp_millis();
        let seq = self.pool.alloc_tx_seq();
        let stored = StoredTradeAudit {
            player_1: e.player_1.to_owned(), player_2: e.player_2.to_owned(),
            item_player_1: e.item_player_1.to_owned(), item_player_2: e.item_player_2.to_owned(),
            bag_1_before: e.bag_1_before.to_owned(), bag_2_before: e.bag_2_before.to_owned(),
            bag_1_after: e.bag_1_after.to_owned(), bag_2_after: e.bag_2_after.to_owned(),
            time_tran_ms: ts_ms,
        };
        self.pool.ks().history_tx.insert(
            &enc_history_tx_key(ts_ms, seq)[..],
            encode_val("history_tx", &stored)?,
        )?;
        Ok(())
    }

    pub async fn delete_older_than_days(&self, now: DateTime<Utc>, days: i64) -> DbResult<u64> {
        let cutoff_ms = (now - Duration::days(days)).timestamp_millis();
        let upper = enc_history_tx_key(cutoff_ms, 0);
        let mut batch = self.pool.ks().db.batch();
        let mut count = 0_u64;
        for guard in self.pool.ks().history_tx.range(..upper) {
            batch.remove(&self.pool.ks().history_tx, guard.key()?);
            count += 1;
        }
        if count > 0 { batch.commit()?; }
        Ok(count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WomensDayBuffs {
    #[serde(rename = "damePrecent")] pub dame_percent_ms: i64,
    #[serde(rename = "hpPrecent")] pub hp_percent_ms: i64,
    #[serde(rename = "mpPrecent")] pub mp_percent_ms: i64,
    #[serde(rename = "papPrecent")] pub pap_percent_ms: i64,
}

#[derive(Debug, Clone)]
pub struct EventRepo { pool: FjallEngine }

impl EventRepo {
    #[must_use]
    pub const fn new(pool: FjallEngine) -> Self { Self { pool } }

    pub async fn load_womens_day(&self) -> DbResult<WomensDayBuffs> {
        match self.pool.ks().events.get(WOMENS_DAY_EVENT_KEY)? {
            Some(bytes) => decode_val("event", &bytes),
            None => Ok(WomensDayBuffs::default()),
        }
    }

    pub async fn save_womens_day(&self, buffs: &WomensDayBuffs) -> DbResult<()> {
        self.pool.ks().events.insert(WOMENS_DAY_EVENT_KEY, encode_val("event", buffs)?)?;
        Ok(())
    }
}
