//! Player inventory containers (`Body`, `Bag`, `Box`, `LuckyRoundBox`), option mutators,
//! stack merging, equip hidden-stat expansion (`Opt 210`/`231`), and disjoint slot validation.

use nro_core::{GameRng, ItemInstance, ItemOption, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter};
use thiserror::Error;

/// Maximum raw gold balance (`Inventory.LIMIT_GOLD` = 200 billion).
pub const LIMIT_GOLD: u64 = 200_000_000_000;
/// Hard ceiling on player bag slots (`80`).
pub const MAX_BAG_SLOTS: usize = 80;
/// Hard ceiling on player storage box slots (`100`).
pub const MAX_BOX_SLOTS: usize = 100;
/// Hard ceiling on secondary Lucky Round prize box slots (`100`).
pub const MAX_LUCKY_BOX_SLOTS: usize = 100;
/// Maximum stack quantity inside a single stackable slot (`9,999`).
pub const MAX_STACK_QTY: u32 = 9_999;

/// Canonical NRO `item_option_template` IDs from `sql/nro1.sql`.
pub mod opt_id {
    /// Flat attack (`Tấn công+#`).
    pub const ATK: u16 = 0;
    /// Critical strike damage (`+#% sức đánh chí mạng`).
    pub const CRIT_DMG: u16 = 5;
    /// Flat HP (`HP+#`).
    pub const HP: u16 = 6;
    /// Flat KI (`KI+#`).
    pub const KI: u16 = 7;
    /// Critical chance (`Chí mạng+#%`).
    pub const CRIT: u16 = 14;
    /// Melee damage reflection (`Phản đòn cận chiến+#`).
    pub const REFLECT: u16 = 15;
    /// Power requirement in billions (`Yêu cầu sức mạnh # tỉ`).
    pub const POWER_REQ_B: u16 = 21;
    /// Flat HP in thousands (`HP+#K`).
    pub const HP_K: u16 = 22;
    /// Flat KI in thousands (`KI+#K`).
    pub const KI_K: u16 = 23;
    /// Flat HP regen per 30s (`+# HP/30s`).
    pub const HP_REGEN: u16 = 27;
    /// Flat KI regen per 30s (`+# KI/30s`).
    pub const KI_REGEN: u16 = 28;
    /// Untradable lock (`Không thể giao dịch`).
    pub const UNTRADABLE: u16 = 30;
    /// Embedded shard/stack quantity counter (`Số lượng #`).
    pub const QUANTITY_PARAM: u16 = 31;
    /// Flat defense (`Giáp+#`).
    pub const DEF: u16 = 47;
    /// Percentage attack bonus (`Sức đánh+#%`).
    pub const DAME_PCT: u16 = 50;
    /// Upgrade tier `+1..=+8` or Porata tier (`Cấp #`).
    pub const LEVEL: u16 = 72;
    /// Permanent marker (`Opt 73`).
    pub const PERM_MARKER: u16 = 73;
    /// Percentage HP bonus (`HP+#%`).
    pub const HP_PCT: u16 = 77;
    /// Percentage HP regen per 30s (`HP+#%/30s`).
    pub const HP_REGEN_PCT: u16 = 80;
    /// Percentage KI regen per 30s (`KI+#%/30s`).
    pub const KI_REGEN_PCT: u16 = 81;
    /// Gold consignment eligibility (`Ký gửi vàng`).
    pub const CONSIGN_GOLD: u16 = 86;
    /// Gem consignment eligibility (`Ký gửi ngọc`).
    pub const CONSIGN_GEM: u16 = 87;
    /// Expiration countdown in days (`Hạn sử dụng # ngày`).
    pub const EXPIRY_DAYS: u16 = 93;
    /// Percentage damage reduction (`Giảm #% sát thương`).
    pub const DMG_REDUCE: u16 = 94;
    /// Potential/power gain bonus (`+#% tiềm năng, sức mạnh`).
    pub const TNSM_PCT: u16 = 101;
    /// Currently socketed crystal stars (`# Sao Pha Lê`).
    pub const SOCKETED_STARS: u16 = 102;
    /// Percentage KI bonus (`KI +#%`).
    pub const KI_PCT: u16 = 103;
    /// Cold immunity (`Không ảnh hưởng bởi cái lạnh`).
    pub const COLD_IMMUNE: u16 = 106;
    /// Drilled crystal star socket capacity `0..=9` (`# Sao Pha Lê`).
    pub const MAX_STAR_SOCKETS: u16 = 107;
    /// Dodge percentage (`#% Né đòn`).
    pub const DODGE_PCT: u16 = 108;
    /// Aura attack bonus (`Đẹp +#% SĐ`).
    pub const BEAUTY_AURA: u16 = 117;
    /// Blindness duration reduction (`Giảm #% thời gian bị mù`).
    pub const BLIND_REDUCE: u16 = 175;
    /// Upgrade downgrade counter (`Bị rớt cấp # lần`).
    pub const DROP_LEVEL_COUNT: u16 = 209;
    /// Unrolled hidden stat line count (`# Dòng chỉ số ẩn`).
    pub const HIDDEN_LINES: u16 = 210;
    /// Book durability out of 1000 (`Độ bền #/1000`).
    pub const DURABILITY: u16 = 212;
    /// Ultimate skill range bonus (`Phạm vi tuyệt kỹ +#%`).
    pub const SKILL_RANGE_PCT: u16 = 214;
    /// Ultimate skill damage bonus (`Tuyệt kỹ +#% sát thương`).
    pub const SKILL_DMG_PCT: u16 = 216;
    /// Unappraised book option (`- Chưa giám định`).
    pub const UNAPPRAISED: u16 = 217;
    /// Reinforced star socket divider (`---------`).
    pub const SOCKET_DIVIDER: u16 = 218;
    /// Remaining book reset charges (`Số lần tẩy #`).
    pub const RESET_COUNT: u16 = 219;
    /// Title badge completion percentage (`Hoàn thành #%`).
    pub const BADGE_PROGRESS: u16 = 220;
    /// Reinforced star socket ceiling `8` or `9` (`Cường hóa tới ô sao pha lê #`).
    pub const REINFORCED_STAR_SLOT: u16 = 228;
    /// Deferred expiry roll on equip (`Hạn sử dụng hoặc vĩnh viễn`).
    pub const ROLL_EXPIRY_ON_EQUIP: u16 = 231;
    /// Luck bonus (`+#% May mắn`).
    pub const LUCKY_PCT: u16 = 236;
    /// Absorbed Kilis energy (`# Kilis`).
    pub const KILIS: u16 = 250;
}

/// Identifies one of the four item storage containers on a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize)]
pub enum ContainerKind {
    /// Equipped items worn on character body (`itemsBody`).
    Body,
    /// Active backpack slots (`itemsBag`).
    Bag,
    /// Home / NPC storage chest (`itemsBox`).
    Box,
    /// Secondary CrackBall / Lucky Round prize box (`itemsBoxCrackBall`).
    LuckyRoundBox,
}

/// Errors produced by inventory slot, stack, or currency operations.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InventoryError {
    /// Slot index `(index)` exceeded `(len)` in `(container)`.
    #[error("{0} slot {1} out of bounds (len {2})")]
    OutOfBounds(ContainerKind, usize, usize),
    /// Referenced slot `(index)` in `(container)` was empty.
    #[error("{0} slot {1} is empty")]
    EmptySlot(ContainerKind, usize),
    /// Combine packet repeated the same slot index (`[slot, slot]` aliasing exploit).
    #[error("duplicate bag slot index {0} in multi-slot operation")]
    DuplicateSlotIndex(usize),
    /// Requested quantity was zero or exceeded available stack count `(requested, available)`.
    #[error("insufficient item quantity: requested {0}, available {1}")]
    InsufficientQuantity(u32, u32),
    /// Container had no free slots for incoming item.
    #[error("{0} is full")]
    ContainerFull(ContainerKind),
    /// Container capacity expansion reached hard server maximum.
    #[error("{0} already at max capacity {1}")]
    MaxCapacityReached(ContainerKind, usize),
    /// Insufficient `(currency, required, available)` balance.
    #[error("insufficient {0}: required {1}, available {2}")]
    InsufficientCurrency(&'static str, u64, u64),
}

/// Reads parameter of option `id` on `item`, if present.
#[must_use]
pub fn get_opt(item: &ItemInstance, id: u16) -> Option<i32> {
    item.options.iter().find(|o| o.id == id).map(|o| o.param)
}

/// Sets option `id` to `param`, appending a new [`ItemOption`] if absent.
pub fn set_opt(item: &mut ItemInstance, id: u16, param: i32) {
    if let Some(existing) = item.options.iter_mut().find(|o| o.id == id) { existing.param = param; }
    else { item.options.push(ItemOption { id, param }); }
}

/// Adds `delta` to option `id` (inserting `delta` if missing).
pub fn add_opt_param(item: &mut ItemInstance, id: u16, delta: i32) {
    if let Some(existing) = item.options.iter_mut().find(|o| o.id == id) { existing.param = existing.param.saturating_add(delta); }
    else { item.options.push(ItemOption { id, param: delta }); }
}

/// Removes all occurrences of option `id`.
pub fn remove_opt(item: &mut ItemInstance, id: u16) {
    item.options.retain(|o| o.id != id);
}

/// Returns `true` if `item` carries `Opt 30` (`Không thể giao dịch`).
#[must_use]
pub fn is_untradable(item: &ItemInstance) -> bool {
    item.options.iter().any(|o| o.id == opt_id::UNTRADABLE)
}

/// Returns `true` if `item` carries any Set Kích Hoạt (SKH) option (`127..=135`, `233`, `237`, `241`, `245`).
#[must_use]
pub fn has_skh_option(item: &ItemInstance) -> bool {
    item.options.iter().any(|o| matches!(o.id, 127..=135 | 233 | 237 | 241 | 245))
}

/// Expands `Opt 210` (hidden stat lines) and `Opt 231` (random expiry) upon equipping to `Body`.
pub fn expand_equip_triggers(item: &mut ItemInstance, rng: &mut GameRng) {
    const POOL: [u16; 7] = [50, 77, 103, 5, 94, 108, 80];
    if let Some(lines) = get_opt(item, opt_id::HIDDEN_LINES) {
        remove_opt(item, opt_id::HIDDEN_LINES);
        for _ in 0..(lines.clamp(0, 6) as usize) {
            add_opt_param(item, POOL[rng.next_range(0, 6) as usize], rng.next_range(1, 5));
        }
    }
    if get_opt(item, opt_id::ROLL_EXPIRY_ON_EQUIP).is_some() {
        remove_opt(item, opt_id::ROLL_EXPIRY_ON_EQUIP);
        if rng.is_true(5, 100) { set_opt(item, opt_id::PERM_MARKER, 0); }
        else { set_opt(item, opt_id::EXPIRY_DAYS, rng.next_range(3, 15)); }
    }
}

/// Complete player inventory holding currencies and all four item containers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    /// Gold balance (`0..=LIMIT_GOLD`).
    pub gold: u64,
    /// Green gem (`ngọc xanh`) balance.
    pub gem: u32,
    /// Pink ruby (`hồng ngọc`) balance.
    pub ruby: u32,
    /// Shop event coupon points.
    pub coupon: u32,
    /// Equipped body slots (`itemsBody`).
    pub body: Vec<Option<ItemInstance>>,
    /// Active bag slots (`itemsBag`).
    pub bag: Vec<Option<ItemInstance>>,
    /// Storage box slots (`itemsBox`).
    pub box_items: Vec<Option<ItemInstance>>,
    /// Secondary Lucky Round prize box (`itemsBoxCrackBall`).
    pub lucky_round_box: Vec<Option<ItemInstance>>,
}

impl Inventory {
    /// Creates an empty [`Inventory`] with bounded initial container capacities.
    #[must_use]
    pub fn new(bag_slots: usize, box_slots: usize, body_slots: usize) -> Self {
        Self {
            gold: 0, gem: 0, ruby: 0, coupon: 0,
            body: vec![None; body_slots],
            bag: vec![None; bag_slots.min(MAX_BAG_SLOTS)],
            box_items: vec![None; box_slots.min(MAX_BOX_SLOTS)],
            lucky_round_box: Vec::new(),
        }
    }
    /// Adds `amount` gold clamped cleanly at [`LIMIT_GOLD`].
    pub fn add_gold_clamped(&mut self, amount: u64) { self.gold = self.gold.saturating_add(amount).min(LIMIT_GOLD); }
    /// Deducts `amount` gold or returns [`InventoryError::InsufficientCurrency`].
    pub fn sub_gold(&mut self, amount: u64) -> Result<(), InventoryError> {
        if self.gold < amount { return Err(InventoryError::InsufficientCurrency("gold", amount, self.gold)); }
        self.gold -= amount; Ok(())
    }
    /// Deducts `amount` green gems or returns [`InventoryError::InsufficientCurrency`].
    pub fn sub_gem(&mut self, amount: u32) -> Result<(), InventoryError> {
        if self.gem < amount { return Err(InventoryError::InsufficientCurrency("gem", u64::from(amount), u64::from(self.gem))); }
        self.gem -= amount; Ok(())
    }
    /// Deducts `amount` pink rubies or returns [`InventoryError::InsufficientCurrency`].
    pub fn sub_ruby(&mut self, amount: u32) -> Result<(), InventoryError> {
        if self.ruby < amount { return Err(InventoryError::InsufficientCurrency("ruby", u64::from(amount), u64::from(self.ruby))); }
        self.ruby -= amount; Ok(())
    }
    /// Expands bag or box capacity by 1 slot up to [`MAX_BAG_SLOTS`] / [`MAX_BOX_SLOTS`].
    pub fn expand_slots(&mut self, container: ContainerKind) -> Result<(), InventoryError> {
        match container {
            ContainerKind::Bag if self.bag.len() < MAX_BAG_SLOTS => { self.bag.push(None); Ok(()) }
            ContainerKind::Bag => Err(InventoryError::MaxCapacityReached(container, MAX_BAG_SLOTS)),
            ContainerKind::Box if self.box_items.len() < MAX_BOX_SLOTS => { self.box_items.push(None); Ok(()) }
            ContainerKind::Box => Err(InventoryError::MaxCapacityReached(container, MAX_BOX_SLOTS)),
            _ => Err(InventoryError::MaxCapacityReached(container, 0)),
        }
    }
    /// Counts unoccupied slots in `bag`.
    #[must_use]
    pub fn empty_bag_slots(&self) -> usize { self.bag.iter().filter(|s| s.is_none()).count() }
    /// Directly sets `bag[index]`.
    pub fn set_bag_slot(&mut self, index: usize, item: Option<ItemInstance>) -> Result<(), InventoryError> {
        let len = self.bag.len();
        let slot = self.bag.get_mut(index).ok_or(InventoryError::OutOfBounds(ContainerKind::Bag, index, len))?;
        *slot = item.filter(|it| it.quantity > 0); Ok(())
    }
    /// Borrows an occupied bag slot.
    pub fn bag_item(&self, index: usize) -> Result<&ItemInstance, InventoryError> {
        let len = self.bag.len();
        self.bag.get(index).ok_or(InventoryError::OutOfBounds(ContainerKind::Bag, index, len))?
            .as_ref().ok_or(InventoryError::EmptySlot(ContainerKind::Bag, index))
    }
    /// Finds the first bag slot index containing `template_id`.
    #[must_use]
    pub fn find_bag_slot(&self, template_id: TemplateId) -> Option<usize> {
        self.bag.iter().position(|s| s.as_ref().is_some_and(|i| i.template_id == template_id))
    }
    /// Sums total quantity of `template_id` across all bag slots.
    #[must_use]
    pub fn count_template_in_bag(&self, template_id: TemplateId) -> u32 {
        self.bag.iter().flatten().filter(|i| i.template_id == template_id).map(|i| i.quantity).sum()
    }
    /// Inserts `incoming` into `bag` atomically, merging into option-identical stacks when `stackable`.
    pub fn add_to_bag(&mut self, mut incoming: ItemInstance, stackable: bool) -> Result<(), InventoryError> {
        if incoming.quantity == 0 { return Ok(()); }
        let empty_slots = self.empty_bag_slots();
        let capacity: u64 = if stackable {
            let merge_cap: u64 = self.bag.iter().flatten()
                .filter(|s| s.template_id == incoming.template_id && s.options == incoming.options)
                .map(|s| u64::from(MAX_STACK_QTY.saturating_sub(s.quantity)))
                .sum();
            merge_cap.saturating_add((empty_slots as u64).saturating_mul(u64::from(MAX_STACK_QTY)))
        } else if empty_slots > 0 { u64::from(incoming.quantity) } else { 0 };
        if u64::from(incoming.quantity) > capacity {
            return Err(InventoryError::ContainerFull(ContainerKind::Bag));
        }
        if stackable {
            for slot in self.bag.iter_mut().flatten() {
                if slot.template_id == incoming.template_id && slot.options == incoming.options && slot.quantity < MAX_STACK_QTY {
                    let take = incoming.quantity.min(MAX_STACK_QTY - slot.quantity);
                    slot.quantity += take; incoming.quantity -= take;
                    if incoming.quantity == 0 { return Ok(()); }
                }
            }
        }
        while incoming.quantity > 0 {
            let empty = self.bag.iter_mut().find(|s| s.is_none()).ok_or(InventoryError::ContainerFull(ContainerKind::Bag))?;
            let chunk = if stackable { incoming.quantity.min(MAX_STACK_QTY) } else { incoming.quantity };
            let mut placed = incoming.clone(); placed.quantity = chunk;
            *empty = Some(placed); incoming.quantity -= chunk;
            if !stackable { break; }
        }
        Ok(())
    }
    /// Subtracts `qty` from `bag[index]`, clearing the slot when depleted to `0`.
    pub fn sub_from_bag_slot(&mut self, index: usize, qty: u32) -> Result<ItemInstance, InventoryError> {
        if qty == 0 { return Err(InventoryError::InsufficientQuantity(0, 0)); }
        let len = self.bag.len();
        let slot = self.bag.get_mut(index).ok_or(InventoryError::OutOfBounds(ContainerKind::Bag, index, len))?;
        let cur = slot.as_mut().ok_or(InventoryError::EmptySlot(ContainerKind::Bag, index))?;
        if cur.quantity < qty { return Err(InventoryError::InsufficientQuantity(qty, cur.quantity)); }
        let mut taken = cur.clone(); taken.quantity = qty; cur.quantity -= qty;
        if cur.quantity == 0 { *slot = None; }
        Ok(taken)
    }
    /// Subtracts `total_qty` of `template_id` across one or more bag stacks atomically.
    pub fn sub_template_from_bag(&mut self, template_id: TemplateId, mut total_qty: u32) -> Result<(), InventoryError> {
        let avail = self.count_template_in_bag(template_id);
        if avail < total_qty { return Err(InventoryError::InsufficientQuantity(total_qty, avail)); }
        for slot in &mut self.bag {
            if total_qty == 0 { break; }
            if let Some(it) = slot && it.template_id == template_id {
                let take = it.quantity.min(total_qty);
                it.quantity -= take; total_qty -= take;
                if it.quantity == 0 { *slot = None; }
            }
        }
        Ok(())
    }
    /// Equips `bag[bag_idx]` into `body[body_idx]` atomically, expanding `Opt 210`/`231` and swapping any worn item.
    pub fn bag_to_body(&mut self, bag_idx: usize, body_idx: usize, rng: &mut GameRng) -> Result<(), InventoryError> {
        if body_idx >= self.body.len() { return Err(InventoryError::OutOfBounds(ContainerKind::Body, body_idx, self.body.len())); }
        let bag_qty = self.bag_item(bag_idx)?.quantity;
        if self.body[body_idx].is_some() && bag_qty > 1 && self.empty_bag_slots() == 0 {
            return Err(InventoryError::ContainerFull(ContainerKind::Bag));
        }
        let mut item = self.sub_from_bag_slot(bag_idx, 1)?;
        expand_equip_triggers(&mut item, rng);
        let prev = self.body[body_idx].take();
        self.body[body_idx] = Some(item);
        if let Some(old) = prev { self.add_to_bag(old, false)?; }
        Ok(())
    }
    /// Moves `body[body_idx]` into `box_items` (fixing `InventoryService.itemBodyToBox` bounds bug).
    pub fn body_to_box(&mut self, body_idx: usize) -> Result<(), InventoryError> {
        let len = self.body.len();
        let slot = self.body.get_mut(body_idx).ok_or(InventoryError::OutOfBounds(ContainerKind::Body, body_idx, len))?;
        let item = slot.take().ok_or(InventoryError::EmptySlot(ContainerKind::Body, body_idx))?;
        if let Some(empty_box) = self.box_items.iter_mut().find(|s| s.is_none()) { *empty_box = Some(item); Ok(()) }
        else { *slot = Some(item); Err(InventoryError::ContainerFull(ContainerKind::Box)) }
    }
    /// Validates disjoint bag slot indices, blocking `[slot, slot]` aliasing in `CombineService`.
    pub fn validate_disjoint_bag_slots(&self, indices: &[usize]) -> Result<SmallVec<[&ItemInstance; 4]>, InventoryError> {
        let mut seen: SmallVec<[usize; 8]> = SmallVec::new();
        let mut out: SmallVec<[&ItemInstance; 4]> = SmallVec::new();
        for &idx in indices {
            if seen.contains(&idx) { return Err(InventoryError::DuplicateSlotIndex(idx)); }
            seen.push(idx); out.push(self.bag_item(idx)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_to_bag_and_bag_to_body_are_atomic_when_full() {
        let mut inv = Inventory::new(1, 1, 1);
        let item = |id, qty| ItemInstance { template_id: TemplateId(id), quantity: qty, options: SmallVec::new(), create_time_ms: 0 };
        inv.add_to_bag(item(10, MAX_STACK_QTY - 5), true).expect("seed");
        assert_eq!(inv.add_to_bag(item(10, 10), true), Err(InventoryError::ContainerFull(ContainerKind::Bag)));
        assert_eq!(inv.count_template_in_bag(TemplateId(10)), MAX_STACK_QTY - 5);
        inv.body[0] = Some(item(99, 1));
        assert_eq!(inv.bag_to_body(0, 0, &mut GameRng::from_seed(1)), Err(InventoryError::ContainerFull(ContainerKind::Bag)));
        assert_eq!(inv.body[0].as_ref().map(|i| i.template_id), Some(TemplateId(99)));
    }
}
