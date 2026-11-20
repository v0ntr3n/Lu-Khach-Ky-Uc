//! Senzu bean consumption (`PEA_IDS`), `MagicTreeState`, `MabuEggState`, `DuaHauEggState`,
//! `LuckyRoundService`, and `GiftCodeService` extracted from [`super`].

use super::{UseItemCtx, UseItemEffect, UseItemError};
use crate::inventory::{ContainerKind, Inventory, InventoryError, get_opt};
use nro_consts::const_item;
use nro_core::{GameRng, ItemInstance, ItemOption, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};
use strum::Display;

pub(super) fn consume_pea(
    inv: &mut Inventory,
    bag_slot: usize,
    snap: &ItemInstance,
    ctx: &mut UseItemCtx<'_>,
) -> Result<UseItemEffect, UseItemError> {
    if ctx.stats.hp <= 0 {
        return Err(UseItemError::PreconditionFailed("dead"));
    }
    if ctx.now_ms.saturating_sub(*ctx.last_pea_ms) < 1_000 {
        return Err(UseItemError::Cooldown("pea 1s cd"));
    }
    inv.sub_from_bag_slot(bag_slot, 1)?;
    *ctx.last_pea_ms = ctx.now_ms;
    let heal = get_opt(snap, 2)
        .map(|v| i64::from(v).saturating_mul(1_000))
        .or_else(|| get_opt(snap, 48).map(i64::from))
        .unwrap_or(500);
    ctx.stats.hp = ctx.stats.hp.saturating_add(heal).min(ctx.stats.hp_max);
    ctx.stats.mp = ctx.stats.mp.saturating_add(heal).min(ctx.stats.mp_max);
    Ok(UseItemEffect::Applied)
}

/// `MagicTree.java` Lv1–10 upgrade & pea regeneration state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagicTreeState {
    /// Tree level (`1..=10`).
    pub level: u8,
    /// Current harvestable peas (`0..=max_peas()`).
    pub curr_peas: u8,
    /// True if currently upgrading to `level + 1`.
    pub is_upgrading: bool,
    /// Epoch (`ms`) of last pea tick or upgrade completion target.
    pub timer_ms: i64,
}

impl MagicTreeState {
    const UP_GOLD: [u64; 9] = [5_000, 10_000, 100_000, 1_000_000, 10_000_000, 20_000_000, 50_000_000, 100_000_000, 300_000_000];
    const UP_MS: [i64; 9] = [600_000, 6_000_000, 60_000_000, 597_600_000, 1_198_800_000, 2_397_600_000, 4_798_800_000, 5_997_600_000, 9_000_000_000];
    const FAST_UP_GEM: [u32; 10] = [20, 50, 120, 300, 800, 1500, 3000, 6000, 7500, 10_000];
    const FAST_HV_GEM: [u32; 10] = [1, 2, 5, 7, 9, 12, 15, 20, 25, 30];
    /// Canonical Lv1–10 Senzu bean template IDs (`MagicTree.PEA_TEMP`).
    pub const PEA_IDS: [i16; 10] = [13, 60, 61, 62, 63, 64, 65, 295, 523, 595];

    /// Maximum pea capacity (`5 + (level - 1) * 2`, i.e., `5..=23`).
    #[must_use]
    pub const fn max_peas(&self) -> u8 {
        5 + self.level.saturating_sub(1) * 2
    }

    /// Advances regeneration or completes a due upgrade at `now_ms`.
    pub fn tick(&mut self, now_ms: i64) {
        if self.is_upgrading && now_ms >= self.timer_ms {
            self.is_upgrading = false;
            self.level = (self.level + 1).min(10);
            self.curr_peas = self.max_peas();
            self.timer_ms = now_ms;
        } else if !self.is_upgrading && self.curr_peas < self.max_peas() {
            let g = (now_ms.saturating_sub(self.timer_ms) / (i64::from(self.level.max(1)) * 60_000)).max(0) as u8;
            if g > 0 {
                self.curr_peas = (self.curr_peas + g).min(self.max_peas());
                self.timer_ms = now_ms;
            }
        }
    }

    /// Harvests available peas into `inv.bag`, encoding `Opt 2` (`+#000`) for `lv <= 3` and `Opt 48` for `lv >= 4`.
    pub fn harvest(&mut self, inv: &mut Inventory, now_ms: i64) -> Result<u8, UseItemError> {
        self.tick(now_ms);
        if self.curr_peas == 0 {
            return Ok(0);
        }
        let lv = self.level.clamp(1, 10);
        let (pid, cnt) = (Self::PEA_IDS[(lv - 1) as usize], self.curr_peas);
        let opt = if lv <= 3 {
            ItemOption { id: 2, param: i32::from(lv) }
        } else {
            ItemOption { id: 48, param: 1_000 << (lv - 2) }
        };
        inv.add_to_bag(
            ItemInstance {
                template_id: TemplateId(pid),
                quantity: u32::from(cnt),
                options: smallvec![opt],
                create_time_ms: now_ms,
            },
            true,
        )?;
        self.curr_peas = 0;
        self.timer_ms = now_ms;
        Ok(cnt)
    }

    /// Starts upgrading `level -> level + 1` (`Lv1..=9`).
    pub fn start_upgrade(&mut self, inv: &mut Inventory, now_ms: i64) -> Result<(), UseItemError> {
        if self.is_upgrading || self.level >= 10 {
            return Err(UseItemError::PreconditionFailed("cannot upgrade tree"));
        }
        let idx = (self.level - 1) as usize;
        inv.sub_gold(Self::UP_GOLD[idx])?;
        self.is_upgrading = true;
        self.timer_ms = now_ms + Self::UP_MS[idx];
        Ok(())
    }

    /// Completes active upgrade or refills peas instantly using Gems.
    pub fn fast_finish(&mut self, inv: &mut Inventory, now_ms: i64) -> Result<(), UseItemError> {
        let idx = (self.level.clamp(1, 10) - 1) as usize;
        if self.is_upgrading {
            inv.sub_gem(Self::FAST_UP_GEM[idx])?;
            self.is_upgrading = false;
            self.level = (self.level + 1).min(10);
        } else {
            inv.sub_gem(Self::FAST_HV_GEM[idx])?;
        }
        self.curr_peas = self.max_peas();
        self.timer_ms = now_ms;
        Ok(())
    }
}

/// 10-day `MabuEgg` (`864_000_000ms`, instant hatch `1,000,000,000` Gold).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MabuEggState {
    /// Completion timestamp (`ms`).
    pub ready_at_ms: i64,
}
impl MabuEggState {
    /// Plants a new 10-day Mabu egg.
    #[must_use]
    pub const fn plant(now_ms: i64) -> Self { Self { ready_at_ms: now_ms + 864_000_000 } }
    /// Pays `1,000,000,000` Gold (`QuaTrung.COST_AP_TRUNG_NHANH`) to finish incubation immediately.
    pub fn fast_hatch(&mut self, inv: &mut Inventory, now_ms: i64) -> Result<(), UseItemError> { inv.sub_gold(1_000_000_000)?; self.ready_at_ms = now_ms; Ok(()) }
}

/// 24-hour `DuaHauEgg` (`86_400_000ms`, 6-hour growth icons `4669..=4672`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuaHauEggState {
    /// Planted timestamp (`ms`).
    pub planted_at_ms: i64,
}
impl DuaHauEggState {
    /// Current growth sprite icon (`4669..=4672`).
    #[must_use]
    pub fn stage_icon(&self, now_ms: i64) -> u16 { 4669 + (now_ms.saturating_sub(self.planted_at_ms) / 21_600_000).clamp(0, 3) as u16 }
}

/// `LuckyRound` spin mode (`4` Gem / `821` ticket vs. `250,000,000` Gold VIP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum LuckyRoundMode {
    /// Standard spin (`4` Gem or `821` Vé quay ngọc vàng).
    StandardGem,
    /// VIP spin (`250,000,000` Gold per roll).
    VipGold,
}

/// `LuckyRound.java` spin processor depositing into `inv.lucky_round_box` (cap `100`).
pub struct LuckyRoundService;
impl LuckyRoundService {
    /// Executes `spins` (`1..=7`) and deposits rolled prizes into `lucky_round_box`.
    pub fn spin(inv: &mut Inventory, mode: LuckyRoundMode, spins: u8, rng: &mut GameRng) -> Result<u8, UseItemError> {
        let n = spins.clamp(1, 7);
        if inv.lucky_round_box.len() + usize::from(n) > 100 { return Err(InventoryError::ContainerFull(ContainerKind::LuckyRoundBox).into()); }
        for _ in 0..n {
            if mode == LuckyRoundMode::VipGold { inv.sub_gold(250_000_000)?; }
            else if inv.count_template_in_bag(TemplateId(const_item::VE_QUAY_NGOC_VANG as i16)) > 0 { inv.sub_template_from_bag(TemplateId(const_item::VE_QUAY_NGOC_VANG as i16), 1)?; }
            else { inv.sub_gem(4)?; }
            inv.lucky_round_box.push(Some(ItemInstance { template_id: TemplateId([17i16, 18, 19, 20, 220, 441, 381][rng.next_range(0, 6) as usize]), quantity: 1, options: smallvec![], create_time_ms: 0 }));
        }
        Ok(n)
    }
}

/// Server giftcode record (`GiftCode.java` — fixes `timeCode()` to compare `now_ms > expires_at_ms`!).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftCodeDef {
    /// Case-exact giftcode string (e.g. `"tanthu"`).
    pub code: String,
    /// Remaining global redemptions.
    pub count_left: u32,
    /// Expiration epoch (`ms`).
    pub expires_at_ms: i64,
    /// Currency & item rewards (`-1` Gold cap `2B`, `-2` Gem cap `200M`, `-3` Ruby cap `200M`).
    pub items: Vec<(i16, u32, SmallVec<[ItemOption; 4]>)>,
    /// Players who already redeemed this code.
    pub redeemed_by: Vec<PlayerId>,
}

/// Hardened `GiftCodeService` enforcing wall-clock expiration and per-player uniqueness.
pub struct GiftCodeService;
impl GiftCodeService {
    /// Redeems `def` for `player_id` at `now_ms`.
    pub fn redeem(def: &mut GiftCodeDef, inv: &mut Inventory, player_id: PlayerId, now_ms: i64) -> Result<(), UseItemError> {
        if now_ms > def.expires_at_ms || def.count_left == 0 || def.redeemed_by.contains(&player_id) { return Err(UseItemError::GiftCodeRejected("expired or unavailable")); }
        if inv.empty_bag_slots() < def.items.iter().filter(|&&(id, _, _)| id >= 0).count() { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
        for (id, qty, opts) in &def.items {
            match *id {
                -1 => inv.add_gold_clamped(u64::from(*qty).min(2_000_000_000)), -2 => inv.gem = inv.gem.saturating_add((*qty).min(200_000_000)), -3 => inv.ruby = inv.ruby.saturating_add((*qty).min(200_000_000)),
                rid => inv.add_to_bag(ItemInstance { template_id: TemplateId(rid), quantity: *qty, options: opts.iter().copied().collect(), create_time_ms: now_ms }, true)?,
            }
        }
        def.count_left -= 1; def.redeemed_by.push(player_id); Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::*;
    use crate::BuiltinCatalog;
    use nro_consts::const_player;
    use nro_core::{CoreStats, Gender};

    #[test]
    fn capsule_193_defers_deduction_and_pea_enforces_alive_and_opt2_scaling() {
        let mut inv = Inventory::new(8, 8, 12);
        inv.set_bag_slot(0, Some(UseItemService::mk(193, 2, &[]))).unwrap();
        let mut tree_lv2 = MagicTreeState { level: 2, curr_peas: 3, is_upgrading: false, timer_ms: 0 };
        let mut tree_lv4 = MagicTreeState { level: 4, curr_peas: 2, is_upgrading: false, timer_ms: 0 };
        assert_eq!(tree_lv2.harvest(&mut inv, 1_000).unwrap(), 3);
        assert_eq!(tree_lv4.harvest(&mut inv, 1_000).unwrap(), 2);

        let mut stats = CoreStats { hp: 0, mp: 100, hp_max: 10_000, mp_max: 10_000, power: 10_000, ..CoreStats::default() };
        let (mut day, mut pea, mut buffs, mut fus, mut pt, mut egg) = (0, 0, TimedBuffState::default(), const_player::NON_FUSION, 0, None);
        let mut rng = GameRng::seeded(1);
        let mut ctx = UseItemCtx {
            now_ms: 2_000, current_day: 1, last_wood_chest_day: &mut day, last_pea_ms: &mut pea,
            gender: Gender::TraiDat, stats: &mut stats, buffs: &mut buffs, fusion_mode: &mut fus,
            has_pet: false, pet_type: &mut pt, pet_power: 0, pet_unlocked_skills: 0,
            zone_satellites: 0, hungry_soi_in_zone: false, mabu_egg: &mut egg,
        };
        assert_eq!(UseItemService::use_bag_item(&mut inv, 0, &mut ctx, &BuiltinCatalog, &mut rng), Ok(UseItemEffect::OpenCapsuleUi(false)));
        assert_eq!(inv.count_template_in_bag(TemplateId(193)), 2);
        assert_eq!(UseItemService::use_bag_item(&mut inv, 1, &mut ctx, &BuiltinCatalog, &mut rng), Err(UseItemError::PreconditionFailed("dead")));

        ctx.stats.hp = 500;
        assert_eq!(UseItemService::use_bag_item(&mut inv, 1, &mut ctx, &BuiltinCatalog, &mut rng), Ok(UseItemEffect::Applied));
        assert_eq!(ctx.stats.hp, 2_500);
        ctx.now_ms = 4_000;
        assert_eq!(UseItemService::use_bag_item(&mut inv, 2, &mut ctx, &BuiltinCatalog, &mut rng), Ok(UseItemEffect::Applied));
        assert_eq!(ctx.stats.hp, 6_500);
    }
}
