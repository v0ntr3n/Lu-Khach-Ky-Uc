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

pub use combine::{CombineContext, CombineError, CombineOp, CombineOutcome, CombinePreview, CombineService};
pub use inventory::{ContainerKind, Inventory, InventoryError, opt_id};
pub use npc_menus::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx};
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
                    0..=5 | 33 | 34 | 41 | 42 | 49 | 50 | 136..=139 | 152..=155 | 168..=171 | 230..=241 | 555 | 557 | 559 | 650 | 652 | 654 | 1048..=1050 => 0,
                    6..=11 | 35 | 36 | 43 | 44 | 51 | 52 | 140..=143 | 156..=159 | 172..=175 | 242..=253 | 556 | 558 | 560 | 651 | 653 | 655 | 1051..=1053 => 1,
                    21..=26 | 37 | 38 | 45 | 46 | 53 | 54 | 144..=147 | 160..=163 | 176..=179 | 254..=265 | 562 | 564 | 566 | 657 | 659 | 661 | 1054..=1056 => 2,
                    27..=32 | 39 | 40 | 47 | 48 | 55 | 56 | 148..=151 | 164..=167 | 180..=183 | 266..=277 | 563 | 565 | 567 | 658 | 660 | 662 | 1057..=1059 => 3,
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
            379 | 381..=385 | 579 | 663..=667 | 764 | 880..=882 | 1150..=1154 | 1233 | 1532
            | 1614..=1616 | 1628 | 1635 | 1758..=1760 | 1775..=1777 => (29, true),
            441..=447 | 1416..=1422 | 1426..=1434 => (30, true),
            956 | 1204 => (33, true),
            _ => (27, !matches!(raw, 454 | 570 | 584 | 921 | 1557 | 1819)),
        };
        let gender = match raw {
            555 | 556 | 562 | 563 | 650 | 651 | 657 | 658 | 1048 | 1051 | 1054 | 1057 | 1060 => Some(Gender::TraiDat),
            557 | 558 | 564 | 565 | 652 | 653 | 659 | 660 | 1049 | 1052 | 1055 | 1058 | 1061 => Some(Gender::Namec),
            559 | 560 | 566 | 567 | 654 | 655 | 661 | 662 | 1050 | 1053 | 1056 | 1059 | 1062 => Some(Gender::Xayda),
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use nro_consts::{const_item, const_map, const_player};
    use nro_core::{AccountId, CoreStats, GameRng, ItemInstance, ItemOption, Location, MapId, PlayerId};
    use proptest::prelude::*;
    use smallvec::smallvec;
    use strum::EnumCount;

    fn make_item(id: i16, qty: u32, opts: &[(u16, i32)]) -> ItemInstance {
        ItemInstance {
            template_id: TemplateId(id),
            quantity: qty,
            options: opts.iter().map(|&(oid, p)| ItemOption { id: oid, param: p }).collect(),
            create_time_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn verifies_66_npcs_and_disjoint_combine_slot_aliasing_guard() {
        assert_eq!(NpcKind::COUNT, 66);
        let mut inv = Inventory::new(8, 8, 12);
        inv.set_bag_slot(2, Some(make_item(441, 10, &[]))).unwrap();
        assert_eq!(
            inv.validate_disjoint_bag_slots(&[2, 2]),
            Err(InventoryError::DuplicateSlotIndex(2))
        );
    }

    #[test]
    fn consignment_gold_sale_pays_raw_gold_and_shop_spec_77_deducts_gem() {
        let mut seller_inv = Inventory::new(20, 20, 12);
        seller_inv.add_to_bag(make_item(const_item::THOI_VANG as i16, 2, &[]), true).unwrap();
        seller_inv.add_to_bag(make_item(14, 5, &[(opt_id::CONSIGN_GOLD, 0)]), true).unwrap();

        let mut market = ConsignShop::default();
        let lid = market.list_item(&mut seller_inv, PlayerId(10), 1, ConsignCurrency::Gold, 100_000, 3).unwrap();
        assert_eq!(seller_inv.count_template_in_bag(TemplateId(457)), 1);

        let mut buyer_inv = Inventory::new(20, 20, 12);
        buyer_inv.gold = 500_000;
        buyer_inv.gem = 100;
        market.buy_item(&mut buyer_inv, PlayerId(20), 20_000_000_000, lid).unwrap();
        assert_eq!(buyer_inv.gold, 400_000);
        assert_eq!(buyer_inv.count_template_in_bag(TemplateId(14)), 3);

        market.claim_proceeds(&mut seller_inv, PlayerId(10), lid).unwrap();
        assert_eq!(seller_inv.gold, 90_000);
        assert_eq!(seller_inv.count_template_in_bag(TemplateId(457)), 1);

        let listing = ShopItemListing {
            template_id: TemplateId(381), quantity: 1, cost: 25,
            currency: ShopCurrency::ItemSpec(TemplateId(const_item::NGOC as i16)),
            gender: None, options: smallvec![],
        };
        let (mut ep, mut cc, mut rng) = (0, 0, GameRng::seeded(9));
        ShopService::buy_listing(&mut buyer_inv, ShopType::SpecIcon, ShopTabKind::Standard, &listing, &mut ep, &mut cc, false, &BuiltinCatalog, &mut rng).unwrap();
        assert_eq!(buyer_inv.gem, 75);
    }

    #[test]
    fn combine_chuyen_hoa_ngoc_deducts_5000_gem_and_phan_ra_sach_deducts_10m_gold() {
        let mut inv = Inventory::new(20, 20, 12);
        inv.gold = 50_000_000;
        inv.gem = 6_000;
        inv.set_bag_slot(0, Some(make_item(232, 1, &[(opt_id::ATK, 100), (opt_id::LEVEL, 5)]))).unwrap();
        inv.set_bag_slot(1, Some(make_item(555, 1, &[(opt_id::ATK, 1_000)]))).unwrap();
        let mut rng = GameRng::seeded(42);
        let ctx = CombineContext::default_hardened();
        let out = CombineService::execute(&mut inv, CombineOp::ChuyenHoaTrangBiNgoc, &[0, 1], &ctx, &BuiltinCatalog, &mut rng).unwrap();
        assert!(out.success);
        assert_eq!(inv.gem, 1_000);
        assert_eq!(inv.gold, 50_000_000);

        inv.set_bag_slot(2, Some(make_item(1044, 1, &[(opt_id::DURABILITY, 1_000)]))).unwrap();
        CombineService::execute(&mut inv, CombineOp::PhanRaSach, &[2], &ctx, &BuiltinCatalog, &mut rng).unwrap();
        assert_eq!(inv.gold, 40_000_000);
        assert_eq!(inv.count_template_in_bag(TemplateId(1283)), 5);
    }

    #[test]
    fn use_item_co_bon_la_1635_tayaki_1798_osin_bill_and_giftcode_work() {
        let mut inv = Inventory::new(12, 10, 12);
        inv.gem = 20;
        inv.set_bag_slot(0, Some(make_item(1635, 1, &[]))).unwrap();
        inv.set_bag_slot(1, Some(make_item(1798, 99, &[]))).unwrap();
        let mut stats = CoreStats {
            power: 65_000_000_000, tiem_nang: 0, hp: 1000, mp: 1000, hp_max: 1000, mp_max: 1000,
            dame: 500, def: 10, crit: 5, stamina: 10_000, max_stamina: 10_000, limit_power: 5, ..CoreStats::default() };
        let (mut day, mut pea, mut buffs, mut fus, mut pt, mut egg) = (0, 0, TimedBuffState::default(), const_player::NON_FUSION, 0, None);
        let mut rng = GameRng::seeded(11);
        {
            let mut uctx = UseItemCtx {
                now_ms: 5_000, current_day: 2, last_wood_chest_day: &mut day, last_pea_ms: &mut pea,
                gender: Gender::TraiDat, stats: &mut stats, buffs: &mut buffs, fusion_mode: &mut fus,
                has_pet: true, pet_type: &mut pt, pet_power: 50_000_000_000, pet_unlocked_skills: 0b1111,
                zone_satellites: 0, hungry_soi_in_zone: false, mabu_egg: &mut egg,
            };
            UseItemService::use_bag_item(&mut inv, 0, &mut uctx, &BuiltinCatalog, &mut rng).unwrap();
            assert!(uctx.buffs.has_active(1_800_000, |s| s == BuffSlot::CoBonLa));
            UseItemService::use_bag_item(&mut inv, 1, &mut uctx, &BuiltinCatalog, &mut rng).unwrap();
            assert_eq!(inv.count_template_in_bag(TemplateId(1805)), 1);
        }
        let mut ctx = NpcPlayerCtx {
            account_id: AccountId(1), player_id: PlayerId(1),
            location: Location { map_id: MapId(114), zone_id: ZoneId(0), x: 200, y: 312 },
            gender: Gender::TraiDat, c_flag: 9, activated: true, has_clan: true, is_clan_leader: true,
            clan_days: 5, has_pet: true, wearing_full_destroyer: true, is_use_gtpt: false,
            is_phu_ho_mabu: false, mabu_points: 10, vnd_balance: 500_000, vip_purchases: 0,
            kol_stage: 1, kol_vip_stage: 1, kol_progress: 200, stats: &mut stats, inventory: &mut inv,
        };
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Osin, 10_000, 1, &mut ctx).unwrap(), NpcEffect::ApplyGtpt);
        assert_eq!(ctx.inventory.gem, 19);
        ctx.location.map_id = MapId(const_map::HANH_TINH_KAIO as i16);
        if let NpcEffect::ShowMenu(p) = NpcMenuRouter::open_base_menu(NpcKind::Bill, &ctx).unwrap() {
            assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Bill, p.menu_index, 0, &mut ctx).unwrap(), NpcEffect::OpenShop("BILL"));
        }
        let mut expired = GiftCodeDef { code: "tanthu".into(), count_left: 5, expires_at_ms: 1_000, items: vec![], redeemed_by: vec![] };
        assert!(matches!(GiftCodeService::redeem(&mut expired, ctx.inventory, PlayerId(1), 2_000), Err(UseItemError::GiftCodeRejected(_))));
    }

    proptest! {
        #[test]
        fn prop_combine_nhap_ngoc_rong_and_hematite_conserve_mass(
            qty in 7u32..500u32, star_id in 15i16..=20i16, spl_qty in 5u32..500u32,
        ) {
            let mut inv = Inventory::new(10, 10, 12);
            inv.gold = 100_000_000;
            inv.set_bag_slot(0, Some(ItemInstance { template_id: TemplateId(star_id), quantity: qty, options: smallvec![], create_time_ms: 0 })).unwrap();
            inv.set_bag_slot(1, Some(ItemInstance { template_id: TemplateId(441), quantity: spl_qty, options: smallvec![], create_time_ms: 0 })).unwrap();
            let mut rng = GameRng::seeded(7);
            let ctx = CombineContext::default_hardened();
            CombineService::execute(&mut inv, CombineOp::NhapNgocRong, &[0], &ctx, &BuiltinCatalog, &mut rng).unwrap();
            CombineService::execute(&mut inv, CombineOp::TaoDaHematite, &[1], &ctx, &BuiltinCatalog, &mut rng).unwrap();
            prop_assert_eq!(inv.count_template_in_bag(TemplateId(star_id)), qty - 7);
            prop_assert_eq!(inv.count_template_in_bag(TemplateId(star_id - 1)), 1);
            prop_assert_eq!(inv.count_template_in_bag(TemplateId(441)), spl_qty - 5);
            prop_assert_eq!(inv.count_template_in_bag(TemplateId(1423)), 1);
        }
    }
}
