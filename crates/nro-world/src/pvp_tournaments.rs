//! Open-world `PVP` state machines (`ThachDau` fixing double loser gold deduction, `LuyenTap`,
//! `TraThu`, `PKCommeson`) and all 4 Martial Arts Tournaments (`DHVT23`, `DeathOrAliveArena`,
//! `WorldMartialArts`, and `SuperRank`).

use nro_consts::{const_player, const_tournament};
use nro_core::{GameRng, ItemInstance, ItemOption, Location, ParityMode, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::smallvec;

use crate::WorldError;

/// Open-world PvP mode variants (`nro.models.matches.*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PvpMode {
    /// Gold duel (`1M / 10M / 100M` Gold; winner receives `1.8x` stake).
    ThachDau {
        /// Escrowed gold stake per duelist.
        stake_gold: u64,
    },
    /// Free practice duel (`PK_PVP_2`, zero penalty).
    LuyenTap,
    /// Revenge (`1` Gem, `5m` cooldown, `3s` post-teleport engagement delay).
    TraThu {
        /// Timestamp when `PK_PVP` engages (`now_ms + 3,000`).
        engage_at_ms: u64,
    },
    /// Map `140` Commeson trial vs `10x`-stat `NhanBan` clone (`5m` limit).
    PkCommeson {
        /// Expiration timestamp (`now_ms + 300,000`).
        expires_at_ms: u64,
    },
}

/// Post-duel gold settlement summary returned by [`PvpSession::resolve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PvpResolution {
    /// `(winner_id, loser_id)`.
    pub pair: (PlayerId, PlayerId),
    /// Winner balance after `1.8x` payout.
    pub winner_gold_after: u64,
    /// Loser balance (deducted once at escrow in hardened mode!).
    pub loser_gold_after: u64,
}

/// Active open-world PvP session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PvpSession {
    /// `(challenger_id, defender_id)`.
    pub pair: (PlayerId, PlayerId),
    /// Duel mode.
    pub mode: PvpMode,
    /// Wire PK state (`PK_PVP = 3` or `PK_PVP_2 = 4`).
    pub wire_pk_type: i8,
}

impl PvpSession {
    /// Starts `ThachDau` (`1M / 10M / 100M` Gold), escrowing `stake` once from both players.
    ///
    /// # Errors
    /// Returns [`WorldError`] on invalid stake tier or insufficient gold.
    pub fn start_thach_dau(p1: PlayerId, g1: &mut u64, p2: PlayerId, g2: &mut u64, stake: u64) -> Result<Self, WorldError> {
        if !matches!(stake, 1_000_000 | 10_000_000 | 100_000_000) {
            return Err(WorldError::ArenaRule("ThachDau stake must be 1M, 10M, or 100M gold"));
        }
        if *g1 < stake || *g2 < stake {
            return Err(WorldError::InsufficientCurrency { kind: "gold", required: stake, available: (*g1).min(*g2) });
        }
        *g1 -= stake;
        *g2 -= stake;
        Ok(Self { pair: (p1, p2), mode: PvpMode::ThachDau { stake_gold: stake }, wire_pk_type: const_player::PK_PVP })
    }

    /// Starts `TraThu` revenge (`1` Gem, `300,000 ms` cooldown, `3,000 ms` engagement delay).
    ///
    /// # Errors
    /// Returns [`WorldError`] if on cooldown or lacking `1` Gem.
    pub fn start_tra_thu(p1: PlayerId, gems: &mut u64, p2: PlayerId, now_ms: u64, last_ms: u64) -> Result<Self, WorldError> {
        let elapsed = now_ms.saturating_sub(last_ms);
        if last_ms > 0 && elapsed < 300_000 { return Err(WorldError::CooldownActive { remaining_ms: 300_000 - elapsed }); }
        if *gems < 1 { return Err(WorldError::InsufficientCurrency { kind: "gem", required: 1, available: *gems }); }
        *gems -= 1;
        Ok(Self { pair: (p1, p2), mode: PvpMode::TraThu { engage_at_ms: now_ms.saturating_add(3_000) }, wire_pk_type: const_player::PK_PVP })
    }

    /// Starts free `LuyenTap` sparring (`PK_PVP_2`).
    #[must_use]
    pub const fn start_luyen_tap(p1: PlayerId, p2: PlayerId) -> Self {
        Self { pair: (p1, p2), mode: PvpMode::LuyenTap, wire_pk_type: const_player::PK_PVP_2 }
    }

    /// Starts a 5-minute `PkCommeson` trial on Map `140`.
    #[must_use]
    pub const fn start_commeson(player: PlayerId, clone: PlayerId, now_ms: u64) -> Self {
        Self { pair: (player, clone), mode: PvpMode::PkCommeson { expires_at_ms: now_ms.saturating_add(300_000) }, wire_pk_type: const_player::PK_PVP }
    }

    /// Resolves the duel, **fixing `ThachDau.java:94` double loser deduction** in hardened mode.
    #[must_use]
    pub fn resolve(self, winner: PlayerId, w_gold: &mut u64, loser: PlayerId, l_gold: &mut u64, parity: ParityMode) -> PvpResolution {
        if let PvpMode::ThachDau { stake_gold } = self.mode {
            *w_gold = w_gold.saturating_add(stake_gold + stake_gold * 80 / 100);
            if matches!(parity, ParityMode::Legacy) { *l_gold = l_gold.saturating_sub(stake_gold); }
        }
        PvpResolution { pair: (winner, loser), winner_gold_after: *w_gold, loser_gold_after: *l_gold }
    }
}

/// 1. 23rd Martial Arts Congress (`The23rdMartialArtCongress`, Map `129`, 12 Bosses).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dhvt23Session {
    /// Current boss index (`0..12`: `SoiHecQuyn`..`Pocolo`).
    pub round: u8,
    /// Progressive `(gold_fee, gem_fee)` (`50k * 2^n`, `2 + 2n`).
    pub fees: (u64, u64),
    /// Highest cleared chest tier today (`0..=12`).
    pub level_wood_chest: u8,
}

impl Default for Dhvt23Session {
    fn default() -> Self { Self { round: 0, fees: (50_000, 2), level_wood_chest: 0 } }
}

impl Dhvt23Session {
    /// Ring boundary check on Map `129` (`x in 151..630` if `y > 264`).
    #[must_use]
    pub const fn in_ring_bounds(loc: Location) -> bool { loc.y <= 264 || (loc.x > 150 && loc.x < 630) }

    /// Deducts progressive Gem/Gold entry fee and starts Round `0`.
    ///
    /// # Errors
    /// Returns [`WorldError`] if already champion (`12`), holding unopened chest `570`, or broke.
    pub fn enter_challenge(&mut self, use_gems: bool, gold: &mut u64, gems: &mut u64, has_chest_570: bool) -> Result<(), WorldError> {
        if self.level_wood_chest >= 12 || has_chest_570 {
            return Err(WorldError::ArenaRule("already champion today or carrying unopened Wood Chest (570)"));
        }
        if use_gems {
            if *gems < self.fees.1 { return Err(WorldError::InsufficientCurrency { kind: "gem", required: self.fees.1, available: *gems }); }
            *gems -= self.fees.1;
        } else {
            if *gold < self.fees.0 { return Err(WorldError::InsufficientCurrency { kind: "gold", required: self.fees.0, available: *gold }); }
            *gold -= self.fees.0;
        }
        self.fees = (self.fees.0.saturating_mul(2), self.fees.1.saturating_add(2));
        self.round = 0;
        Ok(())
    }

    /// Advances round (`13s` wait + `181s` fight) and updates `level_wood_chest`.
    pub fn complete_round(&mut self) {
        if self.round < 12 { self.round += 1; self.level_wood_chest = self.level_wood_chest.max(self.round); }
    }

    /// Claims daily `Rương Gỗ` (`570`, Option `72 = level`, Option `30 = 0`).
    ///
    /// # Errors
    /// Returns [`WorldError`] if `level_wood_chest == 0`.
    pub fn claim_wood_chest(&mut self) -> Result<ItemInstance, WorldError> {
        if self.level_wood_chest == 0 { return Err(WorldError::ArenaRule("no Wood Chest reward available")); }
        let lv = i32::from(self.level_wood_chest);
        self.level_wood_chest = 0;
        Ok(ItemInstance { template_id: TemplateId(570), quantity: 1, options: smallvec![ItemOption { id: 72, param: lv }, ItemOption { id: 30, param: 0 }], create_time_ms: 0 })
    }
}

/// 2. Bà Hạt Mít Death-or-Alive Arena (`DeathOrAliveArena`, Map `112`, 5 Undead Bosses).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeathOrAliveSession {
    /// Current round (`0..5`: `Dracula`..`ThoDauBac`).
    pub round: u8,
    /// Progressive Gold Bar (`457`) fee (`0, 10, 20, ...`).
    pub gold_bar_fee: u32,
    /// Spectator votes (`1M` Gold/vote) for `(contender, ba_hat_mit)`.
    pub votes: (Vec<(PlayerId, u32)>, Vec<(PlayerId, u32)>),
    /// Unlocks 30-day random Satellite (`342..=345`) after clearing Round `5`.
    pub reward_ready: bool,
}

impl DeathOrAliveSession {
    /// Ring boundary check on Map `112` (`x in 323..614` if `y > 336`).
    #[must_use]
    pub const fn in_ring_bounds(loc: Location) -> bool { loc.y <= 336 || (loc.x > 322 && loc.x < 614) }

    /// Pays `gold_bar_fee` (`457`), increments next fee by `+10`, and resets votes.
    ///
    /// # Errors
    /// Returns [`WorldError`] on unclaimed reward or insufficient Gold Bars (`457`).
    pub fn start_run(&mut self, bars_457: &mut u32) -> Result<(), WorldError> {
        if self.reward_ready { return Err(WorldError::ArenaRule("claim Satellite reward before re-entering")); }
        if *bars_457 < self.gold_bar_fee {
            return Err(WorldError::InsufficientItem { template_id: TemplateId(457), required: self.gold_bar_fee, available: *bars_457 });
        }
        *bars_457 -= self.gold_bar_fee;
        self.gold_bar_fee = self.gold_bar_fee.saturating_add(10);
        self.round = 0;
        self.votes.0.clear();
        self.votes.1.clear();
        Ok(())
    }

    /// Places a `1,000,000` Gold spectator vote on Contender (`back_player = true`) or Bà Hạt Mít.
    ///
    /// # Errors
    /// Returns [`WorldError`] if `spectator_gold < 1,000,000`.
    pub fn place_spectator_bet(&mut self, voter: PlayerId, spectator_gold: &mut u64, back_player: bool) -> Result<(), WorldError> {
        if *spectator_gold < 1_000_000 {
            return Err(WorldError::InsufficientCurrency { kind: "gold", required: 1_000_000, available: *spectator_gold });
        }
        *spectator_gold -= 1_000_000;
        let list = if back_player { &mut self.votes.0 } else { &mut self.votes.1 };
        if let Some((_, c)) = list.iter_mut().find(|(p, _)| *p == voter) { *c += 1; } else { list.push((voter, 1)); }
        Ok(())
    }

    /// Settles spectator payouts (`total_votes * 900,000` split pro-rata to winning voters).
    #[must_use]
    pub fn settle_bets(&mut self, player_won_all: bool) -> Vec<(PlayerId, u64)> {
        if player_won_all { self.reward_ready = true; }
        let sp: u64 = self.votes.0.iter().map(|(_, c)| u64::from(*c)).sum();
        let sh: u64 = self.votes.1.iter().map(|(_, c)| u64::from(*c)).sum();
        let pool_90 = (sp + sh) * 900_000;
        let (winners, win_sum) = if player_won_all { (&self.votes.0, sp) } else { (&self.votes.1, sh) };
        if win_sum == 0 { return Vec::new(); }
        winners.iter().map(|&(p, v)| (p, pool_90 * u64::from(v) / win_sum)).collect()
    }

    /// Mints the 30-day random Satellite (`342..=345`, Option `93 = 30`).
    ///
    /// # Errors
    /// Returns [`WorldError`] if `reward_ready` is false.
    #[allow(clippy::cast_possible_truncation)]
    pub fn claim_satellite(&mut self, rng: &mut GameRng) -> Result<ItemInstance, WorldError> {
        if !self.reward_ready { return Err(WorldError::ArenaRule("no VDST Satellite reward pending")); }
        self.reward_ready = false;
        Ok(ItemInstance { template_id: TemplateId(342 + (rng.next_u64() % 4) as i16), quantity: 1, options: smallvec![ItemOption { id: 93, param: 30 }], create_time_ms: 0 })
    }
}

/// 3. World Martial Arts Tournament (`WorldMartialArtsTournament`, Lobby `52`, Ring `51`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TournamentTier {
    /// `8, 14, 18h`, `< 1.5M` Power, `2` Gems.
    NhiDong,
    /// `9, 13, 19h`, `< 15M` Power, `4` Gems.
    SieuCap1,
    /// `10, 15, 20h`, `< 150M` Power, `6` Gems.
    SieuCap2,
    /// `11, 16, 21h`, `< 1.5B` Power, `8` Gems.
    SieuCap3,
    /// `12, 17, 22, 23h`, Uncapped Power, `10,000` Gold.
    NgoaiHang,
}

impl TournamentTier {
    /// Active tournament division for `hour` (`0..=23`).
    #[must_use]
    pub const fn for_hour(hour: u8) -> Option<Self> {
        match hour {
            8 | 14 | 18 => Some(Self::NhiDong), 9 | 13 | 19 => Some(Self::SieuCap1),
            10 | 15 | 20 => Some(Self::SieuCap2), 11 | 16 | 21 => Some(Self::SieuCap3),
            12 | 17 | 22 | 23 => Some(Self::NgoaiHang), _ => None,
        }
    }

    /// Division `(max_power_cap, gem_fee, gold_fee)`.
    #[must_use]
    pub const fn limits(self) -> (Option<i64>, u64, u64) {
        match self {
            Self::NhiDong => (Some(1_500_000), 2, 0), Self::SieuCap1 => (Some(15_000_000), 4, 0),
            Self::SieuCap2 => (Some(150_000_000), 6, 0), Self::SieuCap3 => (Some(1_500_000_000), 8, 0),
            Self::NgoaiHang => (None, 0, 10_000),
        }
    }
}

/// Bracket coordinator (`:00–:24` reg, `:30–:57` matches, ring bounds `x in 158..=610, y <= 320`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldMartialArtsBracket {
    /// Active division.
    pub tier: TournamentTier,
    /// Registered contenders.
    pub contenders: Vec<PlayerId>,
}

impl WorldMartialArtsBracket {
    /// Creates an empty bracket for `tier`.
    #[must_use]
    pub const fn new(tier: TournamentTier) -> Self { Self { tier, contenders: Vec::new() } }

    /// Registers a player before `:25` (`MINS_MAX_CAN_REG`).
    ///
    /// # Errors
    /// Returns [`WorldError`] if past `:24`, above division power cap, duplicate, or broke.
    pub fn register(&mut self, pl: PlayerId, power: i64, min: u8, gold: &mut u64, gems: &mut u64) -> Result<(), WorldError> {
        if i32::from(min) >= const_tournament::MINS_MAX_CAN_REG || self.contenders.contains(&pl) {
            return Err(WorldError::ArenaRule("registration closed or already registered"));
        }
        let (cap, gem_fee, gold_fee) = self.tier.limits();
        if let Some(max_p) = cap && power >= max_p { return Err(WorldError::ArenaRule("power exceeds division cap")); }
        if *gems < gem_fee || *gold < gold_fee {
            return Err(WorldError::InsufficientCurrency { kind: "fee", required: gem_fee.max(gold_fee), available: 0 });
        }
        *gems -= gem_fee;
        *gold -= gold_fee;
        self.contenders.push(pl);
        Ok(())
    }

    /// Timeout tie-breaker: contender with **lower `total_damage_taken`** wins.
    #[must_use]
    pub const fn resolve_timeout(p1: (PlayerId, u64), p2: (PlayerId, u64)) -> PlayerId { if p1.1 <= p2.1 { p1.0 } else { p2.0 } }
}

/// Post-match ladder state returned by [`SuperRankArena::settle_match`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperRankOutcome {
    /// Updated `(challenger_rank, defender_rank)`.
    pub ranks: (u32, u32),
    /// Remaining `(tickets, gems)`.
    pub remaining: (u8, u64),
}

/// 4. Async `SuperRank` (`Giải Siêu Hạng`, Map `113`).
pub struct SuperRankArena;

impl SuperRankArena {
    /// Enforces Top-10 max-2 rank jump, swaps ranks on win, and deducts tickets/gems.
    ///
    /// # Errors
    /// Returns [`WorldError`] on invalid rank target or insufficient gems.
    pub fn settle_match(c_rank: u32, d_rank: u32, won: bool, mut tickets: u8, mut gems: u64) -> Result<SuperRankOutcome, WorldError> {
        if c_rank <= d_rank || (d_rank <= 10 && c_rank - d_rank > 2) {
            return Err(WorldError::ArenaRule("illegal SuperRank challenge target"));
        }
        if won {
            if tickets == 0 {
                if gems < 2 { return Err(WorldError::InsufficientCurrency { kind: "gem", required: 2, available: gems }); }
                gems -= 2;
            }
            Ok(SuperRankOutcome { ranks: (d_rank, c_rank), remaining: (tickets, gems) })
        } else {
            if tickets > 0 { tickets -= 1; } else if gems >= 3 { gems -= 3; } else {
                return Err(WorldError::InsufficientCurrency { kind: "gem", required: 3, available: gems });
            }
            Ok(SuperRankOutcome { ranks: (c_rank, d_rank), remaining: (tickets, gems) })
        }
    }

    /// Daily reset Gem reward (`#1: 100`, `#2..=10: 20`, `#11..=100: 5`, `#101..=1000: 1`).
    #[must_use]
    pub const fn daily_gem_reward(rank: u32) -> u32 {
        match rank { 1 => 100, 2..=10 => 20, 11..=100 => 5, 101..=1_000 => 1, _ => 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_thach_dau_deducts_loser_once(idx in 0usize..3usize, w_extra in 0u64..100_000_000, l_extra in 0u64..100_000_000) {
            let stake = [1_000_000u64, 10_000_000, 100_000_000][idx];
            let (mut g1, mut g2) = (stake + w_extra, stake + l_extra);
            let before = g1 + g2;
            let duel = PvpSession::start_thach_dau(PlayerId(1), &mut g1, PlayerId(2), &mut g2, stake).expect("duel");
            let res = duel.resolve(PlayerId(1), &mut g1, PlayerId(2), &mut g2, ParityMode::Fixed);
            prop_assert_eq!(res.loser_gold_after, l_extra);
            prop_assert_eq!(res.winner_gold_after + res.loser_gold_after + stake / 5, before);
        }
    }
}
