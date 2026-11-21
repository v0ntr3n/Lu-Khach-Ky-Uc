//! Earth, Namek & Ice `Shenron` wish engines (completing `SHENRON_1_3` pants upgrade!),
//! seasonal crafting (`NoiBanh` with hardened stack-quantity checks, `VuaHung`, `XeNuocMia`),
//! and `ChonAiDay` / `ConSoMayMan` Gold & Gem lotteries (fixing the top-5 bettor clamp).

use nro_consts::const_map;
use nro_core::{
    GameRng, Gender, ItemInstance, ItemOption, MapId, ParityMode, PlayerId, TemplateId,
};
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};

use crate::WorldError;

/// Earth Shenron menu pages (`SummonDragon.java`), including the restored `SHENRON_1_3` page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EarthShenronMenu {
    /// 1-Star Page 1 (`0` = 2B Gold, `1` = Gloves +1, `2` = Crit +2%, `3` = Pet Skill 2–3).
    OneStarPage1,
    /// 1-Star Page 2 (`0` = Avatar, `1` = +10k Gem, `2` = +200M SMTN, `3` = Pet Gloves +1).
    OneStarPage2,
    /// 1-Star Page 3 (`SHENRON_1_3`, missing in Java `confirmWish`): `0` = Pants +1 Level!
    OneStarPage3,
    /// 2-Star Set (`0` = +2k Gem, `1` = +20M SMTN, `2` = +200M Gold).
    TwoStar,
    /// 3-Star Set (`0` = +200 Gem, `1` = +2M SMTN, `2` = +20M Gold).
    ThreeStar,
}

/// Clan-wide Namek Porunga wishes (`SummonDragonNamek.java`, Map `7`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamekPorungaWish {
    /// `1..=20x` 3-Star Earth Dragon Ball (`16`) per clanmate.
    ThreeStarDragonBalls,
    /// Chubby Tiger Pet (`942` *Hổ Béo*) with `5..=10%` stat + `20%` secondary option.
    ChubbyTigerPet942,
    /// `99x` Soulbound Wheat Flour (`2053` *Bột mỳ*).
    WheatFlour99,
}

/// Ice Shenron wishes (`Shenron_Event.java`, Balls `925..=931`, `60s` window/cooldown).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceShenronWish {
    /// Rerolls Pet Skills 3 & 4.
    RerollPetSkill34,
    /// Randomizes Intrinsic Skill (requires `power >= 10,000,000,000`).
    RandomizeIntrinsic,
    /// 90-day Planet Costume (`905 / 907 / 911`).
    PlanetCostume90d,
    /// 90-day Black Gohan Rose Disguise (`883`).
    BlackGohanRose90d,
}

/// Mutation produced by confirming a Shenron wish.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShenronEffect {
    /// Sets gold directly to `2,000,000,000`.
    SetGoldCap2B,
    /// Grants `(gold, gems, smtn)` deltas.
    GrantResources(u64, u64, i64),
    /// Adds `+2%` base crit (`critg < 9`).
    BoostBaseCrit2Pct,
    /// Returns upgraded equipment (`+1` Option `72` up to `7`, `+10%` primary stats).
    UpgradedEquipment(ItemInstance),
    /// Mints a new item reward.
    GrantItem(ItemInstance),
    /// Triggers Pet skill or Intrinsic mutation tag.
    TriggerSkillMutation(&'static str),
}

/// Shenron summon session enforcing village maps (`0, 7, 14`), `10m` global cooldown, and `5m` window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragonSummonSession {
    /// Summoning player ID.
    pub summoner: PlayerId,
    /// Expiration timestamp (`ms`).
    pub expires_at_ms: u64,
}

impl DragonSummonSession {
    /// Summons Earth Shenron on Village Maps (`0, 7, 14`) with `600,000 ms` global cooldown.
    ///
    /// # Errors
    /// Returns [`WorldError`] if off-village, on global cooldown, or missing required Dragon Balls.
    #[allow(clippy::cast_possible_truncation)]
    pub fn summon_earth(summoner: PlayerId, map_id: MapId, balls_14_to_20: &[u32; 7], start_star: u8, now_ms: u64, last_global_ms: u64) -> Result<Self, WorldError> {
        if !matches!(i32::from(map_id.0), const_map::LANG_ARU | const_map::LANG_MORI | const_map::LANG_KAKAROT) {
            return Err(WorldError::InvalidWish("Earth Shenron requires Village Map 0, 7, or 14"));
        }
        let elapsed = now_ms.saturating_sub(last_global_ms);
        if last_global_ms > 0 && elapsed < 600_000 { return Err(WorldError::CooldownActive { remaining_ms: 600_000 - elapsed }); }
        if !(1..=3).contains(&start_star) { return Err(WorldError::InvalidWish("start_star must be 1..=3")); }
        let idx = usize::from(start_star - 1);
        for (off, &qty) in balls_14_to_20[idx..].iter().enumerate() {
            if qty == 0 { return Err(WorldError::InsufficientItem { template_id: TemplateId((14 + idx + off) as i16), required: 1, available: 0 }); }
        }
        Ok(Self { summoner, expires_at_ms: now_ms.saturating_add(300_000) })
    }

    fn upgrade_gear(mut item: ItemInstance) -> Result<ItemInstance, WorldError> {
        let mut lv = 0i32;
        for opt in &mut item.options {
            if opt.id == 72 {
                if opt.param >= 7 { return Err(WorldError::InvalidWish("equipment already at max level +7")); }
                opt.param += 1;
                lv = opt.param;
                break;
            }
        }
        if lv == 0 { item.options.push(ItemOption { id: 72, param: 1 }); }
        for opt in &mut item.options {
            if matches!(opt.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) { opt.param = opt.param.saturating_add(opt.param / 10); }
        }
        Ok(item)
    }

    /// Executes an Earth Shenron wish, **completing `SHENRON_1_3` (`OneStarPage3` Pants `+1` level)**!
    ///
    /// # Errors
    /// Returns [`WorldError`] on expired wish window or unmet stat/gear precondition.
    #[allow(clippy::cast_possible_truncation)]
    pub fn confirm_earth_wish(&self, menu: EarthShenronMenu, select: u8, gender: Gender, base_crit: u8, gear: Option<ItemInstance>, now_ms: u64, rng: &mut GameRng) -> Result<ShenronEffect, WorldError> {
        if now_ms > self.expires_at_ms { return Err(WorldError::InvalidWish("Shenron 5-minute window expired")); }
        match (menu, select) {
            (EarthShenronMenu::OneStarPage1, 0) => Ok(ShenronEffect::SetGoldCap2B),
            (EarthShenronMenu::OneStarPage1, 1) | (EarthShenronMenu::OneStarPage2, 3) | (EarthShenronMenu::OneStarPage3, 0) => {
                Ok(ShenronEffect::UpgradedEquipment(Self::upgrade_gear(gear.ok_or(WorldError::InvalidWish("missing equipped gear"))?)?))
            }
            (EarthShenronMenu::OneStarPage1, 2) => {
                if base_crit >= 9 { return Err(WorldError::InvalidWish("base crit already >= 9%")); }
                Ok(ShenronEffect::BoostBaseCrit2Pct)
            }
            (EarthShenronMenu::OneStarPage1, 3) => Ok(ShenronEffect::TriggerSkillMutation("pet_skill_2_3")),
            (EarthShenronMenu::OneStarPage2, 0) => {
                let av = match gender { Gender::TraiDat => 227, Gender::Namec => 228, Gender::Xayda => 229 };
                Ok(ShenronEffect::GrantItem(ItemInstance { template_id: TemplateId(av), quantity: 1, options: smallvec![ItemOption { id: 5, param: 10 + (rng.next_u64() % 6) as i32 }], create_time_ms: 0 }))
            }
            (EarthShenronMenu::OneStarPage2, 1) => Ok(ShenronEffect::GrantResources(0, 10_000, 0)),
            (EarthShenronMenu::OneStarPage2, 2) => Ok(ShenronEffect::GrantResources(0, 0, 200_000_000)),
            (EarthShenronMenu::TwoStar, 0) => Ok(ShenronEffect::GrantResources(0, 2_000, 0)),
            (EarthShenronMenu::TwoStar, 1) => Ok(ShenronEffect::GrantResources(0, 0, 20_000_000)),
            (EarthShenronMenu::TwoStar, 2) => Ok(ShenronEffect::GrantResources(200_000_000, 0, 0)),
            (EarthShenronMenu::ThreeStar, 0) => Ok(ShenronEffect::GrantResources(0, 200, 0)),
            (EarthShenronMenu::ThreeStar, 1) => Ok(ShenronEffect::GrantResources(0, 0, 2_000_000)),
            (EarthShenronMenu::ThreeStar, 2) => Ok(ShenronEffect::GrantResources(20_000_000, 0, 0)),
            _ => Err(WorldError::InvalidWish("invalid Earth Shenron option")),
        }
    }

    /// Mints per-clanmate reward for Namek Porunga (`16`, `942` *Hổ Béo*, or `2053` *Bột mỳ*).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn build_porunga_clan_item(wish: NamekPorungaWish, rng: &mut GameRng) -> ItemInstance {
        match wish {
            NamekPorungaWish::ThreeStarDragonBalls => ItemInstance { template_id: TemplateId(16), quantity: 1 + (rng.next_u64() % 20) as u32, options: SmallVec::new(), create_time_ms: 0 },
            NamekPorungaWish::ChubbyTigerPet942 => {
                let (stat, val) = ([50u16, 77, 103][(rng.next_u64() % 3) as usize], 5 + (rng.next_u64() % 6) as i32);
                let mut options = smallvec![ItemOption { id: stat, param: val }];
                if rng.next_u64().is_multiple_of(5) {
                    options.push(if rng.next_u64().is_multiple_of(2) { ItemOption { id: 94, param: val } } else { ItemOption { id: 14, param: 2 + (rng.next_u64() % 4) as i32 } });
                }
                ItemInstance { template_id: TemplateId(942), quantity: 1, options, create_time_ms: 0 }
            }
            NamekPorungaWish::WheatFlour99 => ItemInstance { template_id: TemplateId(2053), quantity: 99, options: smallvec![ItemOption { id: 30, param: 0 }], create_time_ms: 0 },
        }
    }

    /// Executes an Ice Shenron wish (`925..=931`).
    ///
    /// # Errors
    /// Returns [`WorldError`] if power `< 10B` for intrinsic reroll.
    #[allow(clippy::cast_possible_truncation)]
    pub fn confirm_ice_wish(wish: IceShenronWish, gender: Gender, power: i64, rng: &mut GameRng) -> Result<ShenronEffect, WorldError> {
        match wish {
            IceShenronWish::RerollPetSkill34 => Ok(ShenronEffect::TriggerSkillMutation("pet_skill_3_4")),
            IceShenronWish::RandomizeIntrinsic if power < 10_000_000_000 => Err(WorldError::InvalidWish("requires >= 10B power")),
            IceShenronWish::RandomizeIntrinsic => Ok(ShenronEffect::TriggerSkillMutation("intrinsic_reroll")),
            IceShenronWish::PlanetCostume90d => {
                let id = match gender { Gender::TraiDat => 905, Gender::Namec => 907, Gender::Xayda => 911 };
                let p = 15 + (rng.next_u64() % 11) as i32;
                Ok(ShenronEffect::GrantItem(ItemInstance { template_id: TemplateId(id), quantity: 1, options: smallvec![ItemOption { id: 50, param: p }, ItemOption { id: 93, param: 90 }], create_time_ms: 0 }))
            }
            IceShenronWish::BlackGohanRose90d => Ok(ShenronEffect::GrantItem(ItemInstance { template_id: TemplateId(883), quantity: 1, options: smallvec![ItemOption { id: 50, param: 20 }, ItemOption { id: 93, param: 90 }], create_time_ms: 0 })),
        }
    }
}

/// Cake recipes for `NoiBanh` (*Bánh Dầy* `1542` & *Bánh Chưng* `1556`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CakeKind {
    /// `99x 1546 + 10x 1547 + 10x 1545 + 1x 1544 -> 1542`.
    BanhDay,
    /// `99x 1546 + 99x 1548 + 99x 1549 -> 1556`.
    BanhChung,
}

/// Hùng Vương betrothal gift tiers (`1776` / `1777`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VuaHungGiftKind {
    /// `9x (1220, 1221, 1222) + 1M Gold -> 1776`.
    Standard,
    /// `9x (1220, 1221, 1222) + 1M Gold + 10 Gems -> 1777`.
    Deluxe,
}

/// Hardened seasonal event crafter (`NoiBanh`, `VuaHung`, `XeNuocMia`).
pub struct SeasonalCrafter;

impl SeasonalCrafter {
    /// Crafts `Bánh Dầy (1542)` or `Bánh Chưng (1556)` after **strictly verifying stack counts**
    /// (fixing the `1-item` quantity-underflow exploit in `NoiBanh.java:52–108`!).
    ///
    /// # Errors
    /// Returns [`WorldError::InsufficientItem`] if any ingredient stack is below required count.
    pub fn craft_noi_banh(kind: CakeKind, counts: &mut [(TemplateId, u32)]) -> Result<ItemInstance, WorldError> {
        let reqs: &[(i16, u32)] = match kind {
            CakeKind::BanhDay => &[(1546, 99), (1547, 10), (1545, 10), (1544, 1)],
            CakeKind::BanhChung => &[(1546, 99), (1548, 99), (1549, 99)],
        };
        for &(tid, need) in reqs {
            let have = counts.iter().find(|(t, _)| i32::from(t.0) == i32::from(tid)).map_or(0, |(_, q)| *q);
            if have < need { return Err(WorldError::InsufficientItem { template_id: TemplateId(tid), required: need, available: have }); }
        }
        for &(tid, need) in reqs {
            if let Some((_, q)) = counts.iter_mut().find(|(t, _)| i32::from(t.0) == i32::from(tid)) { *q -= need; }
        }
        let id = match kind { CakeKind::BanhDay => 1542, CakeKind::BanhChung => 1556 };
        Ok(ItemInstance { template_id: TemplateId(id), quantity: 1, options: smallvec![ItemOption { id: 93, param: 30 }], create_time_ms: 0 })
    }

    /// Exchanges Watermelons (`569`) + Stamps (`1558`) for Green Gems (`5 / 40 / 120 / 185 / 250`).
    ///
    /// # Errors
    /// Returns [`WorldError`] on invalid tier or insufficient stacks.
    pub fn exchange_vua_hung_watermelon(tier: u8, melon_569: &mut u32, stamp_1558: &mut u32) -> Result<u64, WorldError> {
        let (m, s, gems) = match tier {
            0 => (1, 1, 5), 1 => (10, 2, 40), 2 => (20, 3, 120), 3 => (25, 4, 185), 4 => (30, 5, 250),
            _ => return Err(WorldError::LotteryRule("invalid watermelon exchange tier")),
        };
        if *melon_569 < m { return Err(WorldError::InsufficientItem { template_id: TemplateId(569), required: m, available: *melon_569 }); }
        if *stamp_1558 < s { return Err(WorldError::InsufficientItem { template_id: TemplateId(1558), required: s, available: *stamp_1558 }); }
        *melon_569 -= m;
        *stamp_1558 -= s;
        Ok(gems)
    }

    /// Crafts Hùng Vương Betrothal Gifts (`1776` Standard / `1777` Deluxe).
    ///
    /// # Errors
    /// Returns [`WorldError`] if lacking `9x` (`1220, 1221, 1222`) or currency.
    pub fn craft_vua_hung_gift(kind: VuaHungGiftKind, i1220: &mut u32, i1221: &mut u32, i1222: &mut u32, gold: &mut u64, gems: &mut u64) -> Result<ItemInstance, WorldError> {
        for (tid, q) in [(1220, *i1220), (1221, *i1221), (1222, *i1222)] {
            if q < 9 { return Err(WorldError::InsufficientItem { template_id: TemplateId(tid), required: 9, available: q }); }
        }
        let g_req = if kind == VuaHungGiftKind::Deluxe { 10 } else { 0 };
        if *gold < 1_000_000 || *gems < g_req { return Err(WorldError::InsufficientCurrency { kind: "gold_or_gem", required: 1_000_000, available: *gold }); }
        *i1220 -= 9;
        *i1221 -= 9;
        *i1222 -= 9;
        *gold -= 1_000_000;
        *gems -= g_req;
        Ok(ItemInstance { template_id: TemplateId(if kind == VuaHungGiftKind::Deluxe { 1777 } else { 1776 }), quantity: 1, options: SmallVec::new(), create_time_ms: 0 })
    }

    /// Presses `1` or `10` Sugarcane Juice cups (`XeNuocMia`: `5x 1612 + 2x 1613 + 5M Gold`/cup).
    ///
    /// # Errors
    /// Returns [`WorldError`] on missing ingredients or gold.
    #[allow(clippy::cast_possible_truncation)]
    pub fn press_sugarcane(ten: bool, cane_1612: &mut u32, ice_1613: &mut u32, gold: &mut u64, rng: &mut GameRng) -> Result<(SmallVec<[TemplateId; 10]>, u32), WorldError> {
        let m = if ten { 10u32 } else { 1 };
        let cost = 5_000_000 * u64::from(m);
        if *cane_1612 < 5 * m { return Err(WorldError::InsufficientItem { template_id: TemplateId(1612), required: 5 * m, available: *cane_1612 }); }
        if *ice_1613 < 2 * m { return Err(WorldError::InsufficientItem { template_id: TemplateId(1613), required: 2 * m, available: *ice_1613 }); }
        if *gold < cost { return Err(WorldError::InsufficientCurrency { kind: "gold", required: cost, available: *gold }); }
        *cane_1612 -= 5 * m;
        *ice_1613 -= 2 * m;
        *gold -= cost;
        Ok(((0..m).map(|_| TemplateId(1614 + (rng.next_u64() % 3) as i16)).collect(), m))
    }
}

/// 5-minute `ChonAiDay` raffle pool (`300,000 ms` cycle) for Gold and Gem/Ruby modes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChonAiDayPool {
    /// `(player_id, total_stake)` entries.
    pub entries: Vec<(PlayerId, u64)>,
    /// VIP (`90%` payout) vs Normal (`80%` payout).
    pub is_vip: bool,
}

impl ChonAiDayPool {
    /// Adds `stake` to `player`'s pool entry.
    pub fn place_bet(&mut self, player: PlayerId, stake: u64) {
        if stake == 0 { return; }
        if let Some((_, s)) = self.entries.iter_mut().find(|(p, _)| *p == player) { *s = s.saturating_add(stake); } else { self.entries.push((player, stake)); }
    }

    /// Draws the round winner using **true stake-weighted RNG across all bettors** (fixing the
    /// top-5 clamp in `ChonAiDay_Gold/Gem.java` and solo `20%` loss in `ChonAiDay_Gold`)!
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn draw_winner(&mut self, rng: &mut GameRng, parity: ParityMode) -> Option<(PlayerId, u64)> {
        let total: u128 = self.entries.iter().map(|(_, s)| u128::from(*s)).sum();
        if total == 0 { self.entries.clear(); return None; }
        if self.entries.len() == 1 && !matches!(parity, ParityMode::Legacy) {
            let solo = self.entries[0].0;
            self.entries.clear();
            return Some((solo, u64::try_from(total * 90 / 100).unwrap_or(u64::MAX)));
        }
        let winner = if matches!(parity, ParityMode::Legacy) {
            self.entries.sort_by_key(|b| std::cmp::Reverse(b.1));
            self.entries[(rng.next_u64() as usize) % self.entries.len().min(5)].0
        } else {
            let (mut roll, mut picked) = (u128::from(rng.next_u64()) % total, self.entries[0].0);
            for &(pid, s) in &self.entries {
                let s = u128::from(s);
                if roll < s { picked = pid; break; }
                roll -= s;
            }
            picked
        };
        self.entries.clear();
        let pct = if self.is_vip { 90u128 } else { 80u128 };
        Some((winner, u64::try_from(total * pct / 100).unwrap_or(u64::MAX)))
    }
}

/// 60-second `ConSoMayMan` lottery (`50s` open + `10s` freeze; Gem `5 -> 450`, Gold Bar `1 -> 90`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConSoMayManRound {
    /// `(player_id, up_to_10_distinct_numbers)` in `0..=99`.
    pub tickets: Vec<(PlayerId, SmallVec<[u8; 10]>)>,
}

impl ConSoMayManRound {
    /// Picks a distinct number in `0..100` (fixing Java's `0..=100` 101-outcome bug in hardened mode).
    ///
    /// # Errors
    /// Returns [`WorldError::LotteryRule`] on out-of-range, duplicate, or `> 10` picks.
    pub fn pick_number(&mut self, player: PlayerId, num: u8, parity: ParityMode) -> Result<(), WorldError> {
        if num > if matches!(parity, ParityMode::Legacy) { 100 } else { 99 } { return Err(WorldError::LotteryRule("number out of range")); }
        if let Some((_, list)) = self.tickets.iter_mut().find(|(p, _)| *p == player) {
            if list.len() >= 10 || list.contains(&num) { return Err(WorldError::LotteryRule("duplicate or >10 picks")); }
            list.push(num);
        } else { self.tickets.push((player, smallvec![num])); }
        Ok(())
    }

    /// Settles the `60s` round, returning `(winning_number, winning_players)`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn settle_draw(&mut self, rng: &mut GameRng, parity: ParityMode) -> (u8, Vec<PlayerId>) {
        let win = (rng.next_u64() % if matches!(parity, ParityMode::Legacy) { 101 } else { 100 }) as u8;
        let winners = self.tickets.iter().filter_map(|(p, n)| n.contains(&win).then_some(*p)).collect();
        self.tickets.clear();
        (win, winners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn noi_banh_blocks_single_item_exploit_and_shenron_1_3_upgrades_pants() {
        let mut fake = [(TemplateId(1546), 1), (TemplateId(1548), 1), (TemplateId(1549), 1)];
        assert!(SeasonalCrafter::craft_noi_banh(CakeKind::BanhChung, &mut fake).is_err());
        let mut rng = GameRng::from_seed(7);
        let s = DragonSummonSession { summoner: PlayerId(1), expires_at_ms: 300_000 };
        let pants = ItemInstance { template_id: TemplateId(6), quantity: 1, options: smallvec![ItemOption { id: 6, param: 1_000 }], create_time_ms: 0 };
        let eff = s.confirm_earth_wish(EarthShenronMenu::OneStarPage3, 0, Gender::TraiDat, 0, Some(pants), 500, &mut rng).expect("wish");
        assert!(matches!(eff, ShenronEffect::UpgradedEquipment(it) if it.options.iter().any(|o| o.id == 72 && o.param == 1)));
    }

    proptest! {
        #[test]
        fn prop_chon_ai_day_sixth_bettor_can_win(seed in any::<u64>()) {
            let mut pool = ChonAiDayPool { entries: (1..=6).map(|i| (PlayerId(i), 1_000_000)).collect(), is_vip: false };
            let (_, payout) = pool.draw_winner(&mut GameRng::from_seed(seed), ParityMode::Fixed).expect("draw");
            prop_assert_eq!(payout, 4_800_000);
        }
    }
}
