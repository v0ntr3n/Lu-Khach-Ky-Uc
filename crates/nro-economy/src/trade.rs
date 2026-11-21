//! Hardened two-party `TradeSession` with deterministic escrow and conservation-of-mass guarantees.
//!
//! Fixes `Trade.java:111` (missing early return on untradeable `Rương Gỗ` `570`), supports
//! `Panel.cs:671` `quantity == 0` -> `1` coercion on occupied slots, and restores unstaged items
//! via [`TradeSession::remove_item_offer`].

use crate::inventory::{ContainerKind, Inventory, InventoryError, LIMIT_GOLD, is_untradable};
use crate::{ItemCatalog, ItemMeta};
use nro_consts::const_item;
use nro_core::{ItemInstance, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;

/// Maximum gold transferable per trade (`Trade.MAX_GOLD_TRADE_PER_TIME`).
pub const MAX_GOLD_TRADE: u64 = 10_000_000;
/// Maximum stack chunk per trade slot (`Trade.java:120`).
pub const MAX_TRADE_STACK_CHUNK: u32 = 99;
/// Minimum cooldown between trade invites (`TransactionService.TIME_DELAY_TRADE`).
pub const TRADE_COOLDOWN_MS: i64 = 10_000;
/// Maximum lifetime of an open trade session (`180_000ms`).
pub const TRADE_TIMEOUT_MS: i64 = 180_000;
/// Untradeable Wood Chest template ID (`ConstItem.RUONG_GO = 570`).
pub const WOOD_CHEST_TEMPLATE_ID: TemplateId = TemplateId(const_item::RUONG_GO as i16);

/// Lifecycle state of a [`TradeSession`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum TradeState {
    /// Both parties can add items and update gold offers.
    Open,
    /// Player 1 locked their offer; awaiting Player 2 lock.
    LockedP1,
    /// Player 2 locked their offer; awaiting Player 1 lock.
    LockedP2,
    /// Both parties locked; awaiting final accept confirmations.
    LockedBoth,
    /// Player 1 accepted; awaiting Player 2 accept.
    AcceptedP1,
    /// Player 2 accepted; awaiting Player 1 accept.
    AcceptedP2,
    /// Escrow atomically swapped and committed.
    Completed,
    /// Trade aborted and rolled back to pre-trade snapshots.
    Cancelled,
}

/// Account & cooldown metadata for one trading player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeParticipant {
    /// Unique player identifier.
    pub player_id: PlayerId,
    /// Whether `session.actived` is enabled.
    pub activated: bool,
    /// Whether `player.baovetaikhoan` security lock is active.
    pub security_locked: bool,
    /// Timestamp (`ms`) of previous completed/invited trade.
    pub last_trade_ms: i64,
}

/// Errors produced during trade initiation, offer staging, locking, or atomic commit.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TradeError {
    /// Unactivated account (`!session.actived`).
    #[error("account must be activated before trading")]
    AccountNotActivated,
    /// Account protection (`baovetaikhoan`) is enabled.
    #[error("account security lock is enabled")]
    SecurityLockActive,
    /// Trade invite sent before `10,000ms` cooldown elapsed.
    #[error("trade cooldown active ({0}ms remaining)")]
    CooldownActive(i64),
    /// Session exceeded `180,000ms` timeout.
    #[error("trade session timed out")]
    SessionTimedOut,
    /// Action is invalid in current [`TradeState`].
    #[error("action invalid in trade state {0}")]
    InvalidState(TradeState),
    /// Gold offer exceeded [`MAX_GOLD_TRADE`] or available player gold.
    #[error("invalid gold offer {offered} (balance {balance}, cap {MAX_GOLD_TRADE})")]
    InvalidGoldOffer { offered: u64, balance: u64 },
    /// Packet exceeded available slot quantity.
    #[error("invalid trade quantity {requested} (available {available})")]
    ZeroOrExcessQuantity { requested: u32, available: u32 },
    /// Item is `Rương Gỗ` (`570`), carries `Opt 30`, or has an untradeable template type.
    #[error("item template {0:?} cannot be traded")]
    UntradeableItem(TemplateId),
    /// Recipient lacks enough empty bag slots to receive offered items.
    #[error("recipient lacks bag space (needed {needed}, empty {empty})")]
    InsufficientRecipientBagSpace { needed: usize, empty: usize },
    /// Recipient gold would exceed [`LIMIT_GOLD`].
    #[error("recipient gold balance would overflow LIMIT_GOLD")]
    GoldCapOverflow,
    /// Underlying inventory slot error.
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Deterministic two-player escrow state machine.
#[derive(Clone, Debug)]
pub struct TradeSession {
    baseline_1: Inventory,
    baseline_2: Inventory,
    working_1: Inventory,
    working_2: Inventory,
    escrow_items_1: Vec<ItemInstance>,
    escrow_items_2: Vec<ItemInstance>,
    escrow_slots_1: Vec<usize>,
    escrow_slots_2: Vec<usize>,
    escrow_gold_1: u64,
    escrow_gold_2: u64,
    started_at_ms: i64,
    state: TradeState,
}

impl TradeSession {
    /// Opens a validated trade session from both players' baseline inventories.
    pub fn open(p1: TradeParticipant, inv1: Inventory, p2: TradeParticipant, inv2: Inventory, now_ms: i64) -> Result<Self, TradeError> {
        for part in [p1, p2] {
            if !part.activated { return Err(TradeError::AccountNotActivated); }
            if part.security_locked { return Err(TradeError::SecurityLockActive); }
            let elapsed = now_ms.saturating_sub(part.last_trade_ms);
            if part.last_trade_ms > 0 && elapsed < TRADE_COOLDOWN_MS {
                return Err(TradeError::CooldownActive(TRADE_COOLDOWN_MS - elapsed));
            }
        }
        Ok(Self {
            working_1: inv1.clone(), working_2: inv2.clone(), baseline_1: inv1, baseline_2: inv2,
            escrow_items_1: Vec::new(), escrow_items_2: Vec::new(),
            escrow_slots_1: Vec::new(), escrow_slots_2: Vec::new(),
            escrow_gold_1: 0, escrow_gold_2: 0, started_at_ms: now_ms, state: TradeState::Open,
        })
    }

    /// Returns current session [`TradeState`].
    #[must_use]
    pub const fn state(&self) -> TradeState { self.state }

    /// Returns staged item slices `(offer_1, offer_2)`.
    #[must_use]
    pub fn escrow_offers(&self) -> (&[ItemInstance], &[ItemInstance]) { (&self.escrow_items_1, &self.escrow_items_2) }

    const fn can_edit_offer(&self, is_p1: bool) -> bool {
        matches!((self.state, is_p1), (TradeState::Open, _) | (TradeState::LockedP2, true) | (TradeState::LockedP1, false))
    }

    fn ensure_open_for(&mut self, is_p1: bool, now_ms: i64) -> Result<(), TradeError> {
        if now_ms.saturating_sub(self.started_at_ms) > TRADE_TIMEOUT_MS {
            self.state = TradeState::Cancelled;
            return Err(TradeError::SessionTimedOut);
        }
        if !self.can_edit_offer(is_p1) { return Err(TradeError::InvalidState(self.state)); }
        Ok(())
    }

    /// Updates gold offer (`0..=MAX_GOLD_TRADE`) while in `Open` or unilateral lock state.
    pub fn set_gold_offer(&mut self, is_p1: bool, offered: u64, now_ms: i64) -> Result<(), TradeError> {
        self.ensure_open_for(is_p1, now_ms)?;
        let balance = if is_p1 { self.working_1.gold } else { self.working_2.gold };
        if offered > MAX_GOLD_TRADE || offered > balance {
            return Err(TradeError::InvalidGoldOffer { offered, balance });
        }
        if is_p1 { self.escrow_gold_1 = offered; } else { self.escrow_gold_2 = offered; }
        Ok(())
    }

    /// Returns `true` if `item` is forbidden from player-to-player trade (`isItemCannotTran` + `570`).
    #[must_use]
    pub fn is_item_untradeable(item: &ItemInstance, meta: ItemMeta) -> bool {
        item.template_id == WOOD_CHEST_TEMPLATE_ID
            || is_untradable(item)
            || matches!(meta.item_type, 5 | 11 | 22 | 23 | 24 | 33 | 72)
            || matches!(item.template_id.0, 194 | 454 | 568 | 921 | 992 | 1819)
    }

    /// Stages `quantity` units (`0` coerced to `1` per `Panel.cs:671`) from `bag_slot` into escrow.
    pub fn add_item_offer(
        &mut self, is_p1: bool, bag_slot: usize, quantity: u32, catalog: &dyn ItemCatalog, now_ms: i64,
    ) -> Result<(), TradeError> {
        self.ensure_open_for(is_p1, now_ms)?;
        let (bag, escrow, slots) = if is_p1 {
            (&mut self.working_1, &mut self.escrow_items_1, &mut self.escrow_slots_1)
        } else {
            (&mut self.working_2, &mut self.escrow_items_2, &mut self.escrow_slots_2)
        };
        let slot_ref = bag.bag_item(bag_slot)?;
        if Self::is_item_untradeable(slot_ref, catalog.meta_of(slot_ref.template_id)) {
            return Err(TradeError::UntradeableItem(slot_ref.template_id));
        }
        let available = slot_ref.quantity;
        let qty = quantity.max(1);
        if qty > available {
            return Err(TradeError::ZeroOrExcessQuantity { requested: quantity, available });
        }
        let removed = bag.sub_from_bag_slot(bag_slot, qty)?;
        for _ in 0..(qty / MAX_TRADE_STACK_CHUNK) {
            let mut chunk = removed.clone();
            chunk.quantity = MAX_TRADE_STACK_CHUNK;
            escrow.push(chunk);
            slots.push(bag_slot);
        }
        if !qty.is_multiple_of(MAX_TRADE_STACK_CHUNK) {
            let mut tail = removed;
            tail.quantity = qty % MAX_TRADE_STACK_CHUNK;
            escrow.push(tail);
            slots.push(bag_slot);
        }
        Ok(())
    }

    /// Removes staged item chunks originating from `bag_slot` and restores them to `working_1`/`working_2`.
    pub fn remove_item_offer(&mut self, is_p1: bool, bag_slot: usize, now_ms: i64) -> Result<(), TradeError> {
        self.ensure_open_for(is_p1, now_ms)?;
        let (bag, escrow, slots) = if is_p1 {
            (&mut self.working_1, &mut self.escrow_items_1, &mut self.escrow_slots_1)
        } else {
            (&mut self.working_2, &mut self.escrow_items_2, &mut self.escrow_slots_2)
        };
        let (mut restored_qty, mut sample, mut i) = (0_u32, None, 0);
        while i < escrow.len() {
            if slots[i] == bag_slot {
                let chunk = escrow.remove(i);
                slots.remove(i);
                restored_qty = restored_qty.saturating_add(chunk.quantity);
                if sample.is_none() { sample = Some(chunk); }
            } else {
                i += 1;
            }
        }
        let Some(mut item) = sample else {
            return Err(InventoryError::EmptySlot(ContainerKind::Bag, bag_slot).into());
        };
        item.quantity = restored_qty;
        let len = bag.bag.len();
        let dst = bag.bag.get_mut(bag_slot).ok_or(InventoryError::OutOfBounds(ContainerKind::Bag, bag_slot, len))?;
        match dst {
            None => *dst = Some(item),
            Some(cur) if cur.template_id == item.template_id && cur.options == item.options => {
                cur.quantity = cur.quantity.saturating_add(restored_qty);
            }
            Some(_) => bag.add_to_bag(item, true)?,
        }
        Ok(())
    }

    /// Locks one side's trade offer (`LOCK_TRADE`).
    pub fn lock(&mut self, is_p1: bool, now_ms: i64) -> Result<TradeState, TradeError> {
        if now_ms.saturating_sub(self.started_at_ms) > TRADE_TIMEOUT_MS {
            self.state = TradeState::Cancelled;
            return Err(TradeError::SessionTimedOut);
        }
        self.state = match (self.state, is_p1) {
            (TradeState::Open, true) => TradeState::LockedP1,
            (TradeState::Open, false) => TradeState::LockedP2,
            (TradeState::LockedP1, false) | (TradeState::LockedP2, true) => TradeState::LockedBoth,
            (s, _) => return Err(TradeError::InvalidState(s)),
        };
        Ok(self.state)
    }

    /// Accepts a locked trade; once both sides accept, atomically commits escrow or rolls back.
    pub fn accept(&mut self, is_p1: bool, catalog: &dyn ItemCatalog, now_ms: i64) -> Result<Option<(Inventory, Inventory)>, TradeError> {
        if now_ms.saturating_sub(self.started_at_ms) > TRADE_TIMEOUT_MS {
            self.state = TradeState::Cancelled;
            return Err(TradeError::SessionTimedOut);
        }
        match (self.state, is_p1) {
            (TradeState::LockedBoth, true) => { self.state = TradeState::AcceptedP1; return Ok(None); }
            (TradeState::LockedBoth, false) => { self.state = TradeState::AcceptedP2; return Ok(None); }
            (TradeState::AcceptedP1, false) | (TradeState::AcceptedP2, true) => {}
            (s, _) => return Err(TradeError::InvalidState(s)),
        }
        if self.working_1.empty_bag_slots() < self.escrow_items_2.len() {
            self.state = TradeState::Cancelled;
            return Err(TradeError::InsufficientRecipientBagSpace { needed: self.escrow_items_2.len(), empty: self.working_1.empty_bag_slots() });
        }
        if self.working_2.empty_bag_slots() < self.escrow_items_1.len() {
            self.state = TradeState::Cancelled;
            return Err(TradeError::InsufficientRecipientBagSpace { needed: self.escrow_items_1.len(), empty: self.working_2.empty_bag_slots() });
        }
        let next_g1 = (self.working_1.gold - self.escrow_gold_1).saturating_add(self.escrow_gold_2);
        let next_g2 = (self.working_2.gold - self.escrow_gold_2).saturating_add(self.escrow_gold_1);
        if next_g1 > LIMIT_GOLD || next_g2 > LIMIT_GOLD {
            self.state = TradeState::Cancelled;
            return Err(TradeError::GoldCapOverflow);
        }
        let (mut out_1, mut out_2) = (self.working_1.clone(), self.working_2.clone());
        out_1.gold = next_g1;
        out_2.gold = next_g2;
        for it in &self.escrow_items_2 { out_1.add_to_bag(it.clone(), catalog.meta_of(it.template_id).stackable)?; }
        for it in &self.escrow_items_1 { out_2.add_to_bag(it.clone(), catalog.meta_of(it.template_id).stackable)?; }
        self.state = TradeState::Completed;
        Ok(Some((out_1, out_2)))
    }

    /// Cancels the trade and restores untouched baseline inventories `(baseline_1, baseline_2)`.
    #[must_use]
    pub fn cancel(mut self) -> (Inventory, Inventory) {
        self.state = TradeState::Cancelled;
        (self.baseline_1, self.baseline_2)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::BuiltinCatalog;
    use proptest::prelude::*;
    use smallvec::smallvec;

    const fn part(id: i64) -> TradeParticipant {
        TradeParticipant { player_id: PlayerId(id), activated: true, security_locked: false, last_trade_ms: 0 }
    }

    #[test]
    fn rejects_wood_chest_570_coerces_zero_qty_to_one_and_restores_removed_offer() {
        let mut b1 = Inventory::new(8, 8, 12);
        b1.set_bag_slot(0, Some(ItemInstance { template_id: WOOD_CHEST_TEMPLATE_ID, quantity: 1, options: smallvec![], create_time_ms: 0 })).unwrap();
        b1.set_bag_slot(1, Some(ItemInstance { template_id: TemplateId(457), quantity: 10, options: smallvec![], create_time_ms: 0 })).unwrap();
        let mut sess = TradeSession::open(part(1), b1, part(2), Inventory::new(8, 8, 12), 20_000).unwrap();
        assert_eq!(sess.add_item_offer(true, 0, 1, &BuiltinCatalog, 20_100), Err(TradeError::UntradeableItem(WOOD_CHEST_TEMPLATE_ID)));
        assert_eq!(sess.add_item_offer(true, 1, 11, &BuiltinCatalog, 20_100), Err(TradeError::ZeroOrExcessQuantity { requested: 11, available: 10 }));
        sess.add_item_offer(true, 1, 0, &BuiltinCatalog, 20_100).unwrap();
        assert_eq!(sess.escrow_offers().0.len(), 1);
        assert_eq!(sess.escrow_offers().0[0].quantity, 1);
        sess.remove_item_offer(true, 1, 20_200).unwrap();
        assert_eq!(sess.escrow_offers().0.len(), 0);
        assert_eq!(sess.working_1.bag_item(1).unwrap().quantity, 10);
        assert_eq!(sess.remove_item_offer(true, 1, 20_000 + TRADE_TIMEOUT_MS + 1), Err(TradeError::SessionTimedOut));
    }

    proptest! {
        #[test]
        fn prop_trade_conserves_total_gold_and_items(
            qty1 in 1u32..400u32, offer1 in 1u32..400u32,
            g1 in 0u64..10_000_000u64, g2 in 0u64..10_000_000u64,
        ) {
            let offer1 = offer1.min(qty1);
            let mut b1 = Inventory::new(12, 8, 12);
            b1.gold = g1 + 5_000;
            b1.set_bag_slot(0, Some(ItemInstance { template_id: TemplateId(457), quantity: qty1, options: smallvec![], create_time_ms: 0 })).unwrap();
            let mut b2 = Inventory::new(12, 8, 12);
            b2.gold = g2 + 7_000;
            let gold_before = b1.gold + b2.gold;

            let mut sess = TradeSession::open(part(1), b1, part(2), b2, 50_000).unwrap();
            sess.add_item_offer(true, 0, offer1, &BuiltinCatalog, 50_100).unwrap();
            sess.set_gold_offer(true, g1, 50_100).unwrap();
            sess.set_gold_offer(false, g2, 50_100).unwrap();
            sess.lock(true, 50_200).unwrap();
            sess.lock(false, 50_200).unwrap();
            assert!(sess.accept(true, &BuiltinCatalog, 50_300).unwrap().is_none());
            let (a1, a2) = sess.accept(false, &BuiltinCatalog, 50_300).unwrap().unwrap();
            prop_assert_eq!(a1.gold + a2.gold, gold_before);
            prop_assert_eq!(a1.count_template_in_bag(TemplateId(457)) + a2.count_template_in_bag(TemplateId(457)), qty1);
        }
    }
}
