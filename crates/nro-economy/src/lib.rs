//! Safe, deterministic economy, crafting, trading, shop, consignment, item-use, and NPC menu
//! engine for the Teamobi2026 NRO server (`#![forbid(unsafe_code)]`, Rust 2024).
//!
//! # Security & Audit Hardening Highlights
//! - **DRM Removal:** Completely eliminates the obfuscated `ShopTab.java` IP kill-switch
//!   (`Runtime.getRuntime().halt(0)`).
//! - **Consignment Payout Fix:** `ConsignShop::claim_proceeds` pays `gold_sell * 90 / 100` in raw
//!   `Inventory::gold` instead of minting `TemplateId(457)` (`Thỏi vàng`), closing the 33.3M×
//!   Gold-to-Gold-Bar duplication exploit in `ConsignShopService.java:400-412`.
//! - **Trade Escrow Hardening:** Fixes `Trade.java:111` (missing early `return` on `Rương Gỗ`
//!   `570`), supports `Panel.cs:671` `0 -> 1` single-item offer coercion, and restores unstaged
//!   slots cleanly via `TradeSession::remove_item_offer`.
//! - **Combine Aliasing & Premature-Craft Fixes:** Blocks duplicate `[slot, slot]` packet indices,
//!   separates `CheTaoTrangBiThienSu` preview from execution, deducts `5,000` Gem on
//!   `ChuyenHoaTrangBiNgoc` (`995`), and deducts `10,000,000` Gold on `PhanRaSach` (`108`).
//! - **Reachable NPC Menus & Wall-Clock GiftCodes:** Un-nests `Osin` Majin Buu floor (`114..=120`)
//!   and Map `127` menus, enforces `Babiday` 1-Gem GTPT cost, repairs `Bill` Map `48` shop access,
//!   removes `Santa`'s out-of-bounds `SHOP_VIP` spoof vector, and checks `now_ms > expires_at_ms`
//!   in `GiftCodeService`.

#![forbid(unsafe_code)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

/// Data-driven and rule-based crafting engine covering all 32 `models/combine/` recipes.
pub mod combine;
/// Player inventory containers (`Body`, `Bag`, `Box`, `LuckyRoundBox`), options, and slot guards.
pub mod inventory;
/// Declarative and stateful router for all 66 `models/npc_list/` NPC classes.
pub mod npc_menus;
/// Shop tabs (`TabShop*`), `BuyBackLedger`, and hardened `ConsignShop` (`shop_ky_gui`).
pub mod shops;
/// Hardened two-party `TradeSession` escrow state machine.
pub mod trade;
/// Functional item dispatcher (`UseItem.java`), `MagicTree`, eggs, `LuckyRound`, and `GiftCodeService`.
pub mod use_item;

pub use combine::{
    CombineContext, CombineError, CombineOp, CombineOutcome, CombinePreview, CombineService,
};
pub use inventory::{ContainerKind, Inventory, InventoryError, opt_id};
pub use npc_menus::{
    MenuPrompt, NpcEffect, NpcError, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx,
};
pub use shops::{
    BuyBackLedger, ConsignCurrency, ConsignListing, ConsignShop, ConsignTab, ShopCurrency, ShopDef,
    ShopError, ShopItemListing, ShopService, ShopTabKind, ShopType,
};
pub use trade::{
    MAX_GOLD_TRADE, MAX_TRADE_STACK_CHUNK, TRADE_COOLDOWN_MS, TRADE_TIMEOUT_MS, TradeError,
    TradeParticipant, TradeSession, TradeState, WOOD_CHEST_TEMPLATE_ID,
};
pub use use_item::{
    BuffSlot, DuaHauEggState, GiftCodeDef, GiftCodeService, LuckyRoundMode, LuckyRoundService,
    MabuEggState, MagicTreeState, TimedBuffState, UseItemCtx, UseItemEffect, UseItemError,
    UseItemService,
};

use nro_core::{Gender, TemplateId, ZoneId};
use serde::{Deserialize, Serialize};

/// Canonical item category metadata resolved from `item_template` (`sql/nro1.sql`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemMeta {
    /// Template identifier (`item_template.id`).
    pub template_id: TemplateId,
    /// Raw NRO `item_template.type` (`0..=4` equip, `6` pea, `7` skill book, `12` dragon ball, etc.).
    pub item_type: u8,
    /// Optional planet restriction (`None` = universal `gender == 3`).
    pub gender: Option<Gender>,
    /// Minimum power (`strRequire`) required to use or equip.
    pub power_require: i64,
    /// Base shop gold value (`item_template.gold`) used for 25% vendor sellback.
    pub base_gold_value: u64,
    /// Base shop gem value (`item_template.gem`).
    pub base_gem_value: u32,
    /// Whether multiple units stack inside a single bag/box slot (`isUpToUp`).
    pub stackable: bool,
}

impl ItemMeta {
    /// Classifies common NRO template IDs directly when no external SQL row is needed.
    #[must_use]
    pub fn classify_builtin(id: TemplateId) -> Self {
        let raw = id.0;
        let (item_type, stackable) = match raw {
            0..=12 | 21..=59 | 136..=187 | 230..=281 | 555..=567 | 650..=662 | 1048..=1062 => {
                let eq_ty = match raw {
                    0..=5
                    | 33
                    | 34
                    | 41
                    | 42
                    | 49
                    | 50
                    | 136..=139
                    | 152..=155
                    | 168..=171
                    | 230..=241
                    | 555
                    | 557
                    | 559
                    | 650
                    | 652
                    | 654
                    | 1048..=1050 => 0,
                    6..=11
                    | 35
                    | 36
                    | 43
                    | 44
                    | 51
                    | 52
                    | 140..=143
                    | 156..=159
                    | 172..=175
                    | 242..=253
                    | 556
                    | 558
                    | 560
                    | 651
                    | 653
                    | 655
                    | 1051..=1053 => 1,
                    21..=26
                    | 37
                    | 38
                    | 45
                    | 46
                    | 53
                    | 54
                    | 144..=147
                    | 160..=163
                    | 176..=179
                    | 254..=265
                    | 562
                    | 564
                    | 566
                    | 657
                    | 659
                    | 661
                    | 1054..=1056 => 2,
                    27..=32
                    | 39
                    | 40
                    | 47
                    | 48
                    | 55
                    | 56
                    | 148..=151
                    | 164..=167
                    | 180..=183
                    | 266..=277
                    | 563
                    | 565
                    | 567
                    | 658
                    | 660
                    | 662
                    | 1057..=1059 => 3,
                    12 | 57..=59 | 184..=187 | 278..=281 | 561 | 656 | 1060..=1062 => 4,
                    _ => (raw.rem_euclid(5)) as u8,
                };
                (eq_ty, false)
            }
            13 | 60..=65 | 295 | 523 | 595 => (6, true),
            14..=20 => (12, true),
            220..=224 => (14, true),
            225 => (15, true),
            226 => (16, true),
            342..=345 => (22, true),
            1252 | 1554 => (23, false),
            361 | 1044 | 1211 | 1212 | 1278..=1280 => (25, false),
            379
            | 381..=385
            | 579
            | 663..=667
            | 764
            | 880..=882
            | 1150..=1154
            | 1233
            | 1532
            | 1614..=1616
            | 1628
            | 1635
            | 1758..=1760
            | 1775..=1777 => (29, true),
            441..=447 | 1416..=1422 | 1426..=1434 => (30, true),
            956 | 1204 => (33, true),
            _ => (27, !matches!(raw, 454 | 570 | 584 | 921 | 1557 | 1819)),
        };
        let gender =
            match raw {
                555 | 556 | 562 | 563 | 650 | 651 | 657 | 658 | 1048 | 1051 | 1054 | 1057
                | 1060 => Some(Gender::TraiDat),
                557 | 558 | 564 | 565 | 652 | 653 | 659 | 660 | 1049 | 1052 | 1055 | 1058
                | 1061 => Some(Gender::Namec),
                559 | 560 | 566 | 567 | 654 | 655 | 661 | 662 | 1050 | 1053 | 1056 | 1059
                | 1062 => Some(Gender::Xayda),
                _ => None,
            };
        Self {
            template_id: id,
            item_type,
            gender,
            power_require: 0,
            base_gold_value: if raw == 457 { 148_000_000 } else { 4_000 },
            base_gem_value: 0,
            stackable,
        }
    }
}

/// Read-only lookup trait for item metadata tables.
pub trait ItemCatalog {
    /// Resolves metadata for `id`, falling back to [`ItemMeta::classify_builtin`].
    fn meta_of(&self, id: TemplateId) -> ItemMeta {
        ItemMeta::classify_builtin(id)
    }
}

/// Default zero-allocation catalog backed by [`ItemMeta::classify_builtin`].
#[derive(Clone, Copy, Debug, Default)]
pub struct BuiltinCatalog;

impl ItemCatalog for BuiltinCatalog {}

/// Helper tagging an economy event to a specific `(ZoneId)` instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneEconomyStamp {
    /// Zone identifier where the transaction occurred.
    pub zone_id: ZoneId,
    /// Monotonic server millisecond timestamp.
    pub at_ms: i64,
}

#[cfg(test)]
#[path = "economy_tests.rs"]
mod tests;
