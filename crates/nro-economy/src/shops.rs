//! Shop tabs (`TabShop*`), `BuyBackService`, and `ConsignShop` (`shop_ky_gui`).
//!
//! Eliminates `ShopTab.java`'s `Runtime.getRuntime().halt(0)` IP DRM kill-switch, repairs Ruby and
//! SpecIcon Gem (`77`) deductions, and fixes the `ConsignShopService` payout exploit so Gold sales
//! pay `90%` in raw `Inventory::gold` rather than minting `TemplateId(457)` (`Thỏi vàng`).

use crate::inventory::{ContainerKind, Inventory, InventoryError, get_opt, is_untradable, opt_id, set_opt};
use crate::ItemCatalog;
use nro_consts::const_item;
use nro_core::{GameRng, Gender, ItemInstance, ItemOption, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

/// Minimum buyer power (`17B`) required to purchase from the consignment market.
pub const CONSIGN_MIN_BUYER_POWER: i64 = 17_000_000_000;
/// Maximum recent sold items retained per player in [`BuyBackLedger`].
pub const MAX_BUYBACK_HISTORY: usize = 10;

/// Shop transaction pricing modes (`shop.type` in `nro1.sql`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShopType {
    /// Standard Gold / Gem / Ruby shop (`type = 0`).
    Normal = 0,
    /// Master potential skill-study shop (`type = 1`, `QUY_LAO`).
    SkillLearn = 1,
    /// Special item / icon exchange shop (`type = 3`).
    SpecIcon = 3,
    /// Secondary CrackBall prize withdrawal (`type = 4`, `ITEMS_LUCKY_ROUND`).
    LuckyRoundBox = 4,
    /// Vendor repurchase tab (`type = 8`, `BuyBackService`).
    BuyBack = 8,
}

/// Specialized `TabShop*` filter and purchase behaviors from `models/shop/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize)]
pub enum ShopTabKind {
    /// Standard unfiltered tab (`TabShop`).
    Standard,
    /// `TabShopUron` (`tabs 10..=12`): filters gender, scales pea, hides owned `453`/`454`.
    Uron,
    /// `TabShopHangDoc` (`tab 13`, `SANTA_HEAD`): free hair/head avatar switch.
    HangDocHair,
    /// `TabShopSanta` (`tab 17`): hides bag/box expansion items once at `80`/`100` cap.
    SantaMain,
    /// `TabShopMuaAvatar` (`tab 19`): hides avatars already owned by player.
    MuaAvatar,
    /// `TabShopHocKynang` (`tabs 41..=43`): hides learned skills and starts potential timer.
    HocKyNang,
    /// `TabShopDanhHieu` (`tab 44`): unowned badges with `Opt 220` (`Hoàn thành #%`).
    DanhHieuUnowned,
    /// `TabShopSoHuu` (`tab 45`): owned badges with `Opt 93` remaining days.
    DanhHieuOwned,
    /// `SANTA_GIAM_GIA_1` (`tab 50`) / `tab 30`: consumes `1× 459` (`Phiếu giảm giá` 80%).
    Discount80,
    /// `QDDN` (`tab 49`): free Chipi claim (`5%` perm / `95%` `3..=7` days).
    ChipiExchange,
    /// `SHOP_DOI_DIEM` (`tab 59`): spends `event_points`.
    EventPoint,
    /// `SHOP_CLAN` (`tabs 60..=62`): spends `clan_capsules`.
    ClanCapsule,
    /// `BILL` (`tabs 21..=23`): Hủy Diệt gear requiring 5 Thần Linh pieces + `99×` food (`663..=667`).
    BillHuyDiet,
}

/// Currency selector on a [`ShopItemListing`] (`ItemShop.typeSell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum ShopCurrency {
    /// Raw gold (`typeSell = 0`).
    Gold,
    /// Green gem (`typeSell = 1`).
    Gem,
    /// Pink ruby (`typeSell = 3`).
    Ruby,
    /// Material or token exchange (`typeSell = 4`, resolved by `TemplateId`).
    ItemSpec(TemplateId),
}

/// One purchasable entry inside a [`ShopTabDef`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopItemListing {
    /// Output item template ID.
    pub template_id: TemplateId,
    /// Unit quantity granted per purchase.
    pub quantity: u32,
    /// Price per purchase.
    pub cost: u64,
    /// Payment currency.
    pub currency: ShopCurrency,
    /// Optional planet restriction.
    pub gender: Option<Gender>,
    /// Base options attached on purchase.
    pub options: SmallVec<[ItemOption; 4]>,
}

/// Definition of a single shop tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopTabDef {
    /// SQL `tab_shop.id`.
    pub tab_id: u16,
    /// Specialized filter/purchase behavior.
    pub kind: ShopTabKind,
    /// Listings inside this tab.
    pub items: Vec<ShopItemListing>,
}

/// Definition of a complete NPC shop (`sql/nro1.sql` `shop`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopDef {
    /// Shop tag name (e.g. `"BUNMA"`, `"URON"`, `"SANTA"`, `"BILL"`).
    pub tag_name: &'static str,
    /// Pricing mode.
    pub shop_type: ShopType,
    /// Ordered tabs.
    pub tabs: Vec<ShopTabDef>,
}

/// Errors emitted by [`ShopService`], [`BuyBackLedger`], or [`ConsignShop`].
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ShopError {
    /// Listing or tab index not found.
    #[error("shop listing not found")]
    ListingNotFound,
    /// Player did not meet prerequisite.
    #[error("shop requirement not met: {0}")]
    RequirementNotMet(&'static str),
    /// Consignment listing parameters violated bounds or untradable rules.
    #[error("invalid consignment listing: {0}")]
    InvalidConsignment(&'static str),
    /// Unauthorized action by non-owner on a consignment listing.
    #[error("caller does not own consignment listing {0}")]
    NotListingOwner(u64),
    /// Underlying inventory error.
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Per-player 10-item FIFO vendor sell/repurchase ledger (`BuyBackService.java`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyBackLedger {
    items: Vec<ItemInstance>,
}

impl BuyBackLedger {
    /// Sells `qty` from `bag_slot` (`37,000,000` Gold per `457` Thỏi vàng, else `base_gold / 4`).
    pub fn sell_from_bag(&mut self, inv: &mut Inventory, bag_slot: usize, qty: u32, catalog: &dyn ItemCatalog) -> Result<u64, ShopError> {
        let sold = inv.sub_from_bag_slot(bag_slot, qty)?;
        let meta = catalog.meta_of(sold.template_id);
        let payout = if sold.template_id.0 == const_item::THOI_VANG as i16 {
            u64::from(qty) * 37_000_000
        } else {
            (meta.base_gold_value / 4).max(1) * u64::from(qty)
        };
        inv.add_gold_clamped(payout);
        if self.items.len() >= MAX_BUYBACK_HISTORY { self.items.remove(0); }
        self.items.push(sold);
        Ok(payout)
    }

    /// Repurchases `history_idx` at `gem / 2`, `gold / 2`, or `quantity * 100` Gold fallback.
    pub fn repurchase(&mut self, inv: &mut Inventory, history_idx: usize, catalog: &dyn ItemCatalog) -> Result<ItemInstance, ShopError> {
        let item = self.items.get(history_idx).ok_or(ShopError::ListingNotFound)?;
        let meta = catalog.meta_of(item.template_id);
        if meta.base_gem_value > 0 {
            inv.sub_gem((meta.base_gem_value / 2).max(1) * item.quantity)?;
        } else {
            let cost = if meta.base_gold_value > 0 { (meta.base_gold_value / 2).max(1) * u64::from(item.quantity) } else { u64::from(item.quantity) * 100 };
            inv.sub_gold(cost)?;
        }
        let reclaimed = self.items.remove(history_idx);
        inv.add_to_bag(reclaimed.clone(), meta.stackable)?;
        Ok(reclaimed)
    }
}

/// Stateless shop filter & purchase processor.
pub struct ShopService;

impl ShopService {
    /// Filters visible listings for a [`ShopTabDef`] according to `TabShop*` rules.
    #[must_use]
    pub fn visible_items<'a>(tab: &'a ShopTabDef, inv: &Inventory, gender: Gender, magic_tree_lv: u8, learned_skills: &[i16]) -> Vec<&'a ShopItemListing> {
        tab.items.iter().filter(|it| match tab.kind {
            ShopTabKind::Uron => {
                it.gender.is_none_or(|g| g == gender)
                    && (!matches!(it.template_id.0, 453 | 454) || inv.find_bag_slot(it.template_id).is_none())
                    && (it.template_id.0 != 13 || magic_tree_lv >= 1)
            }
            ShopTabKind::SantaMain => !(it.template_id.0 == 517 && inv.bag.len() >= 80 || it.template_id.0 == 518 && inv.box_items.len() >= 100),
            ShopTabKind::HocKyNang => !learned_skills.contains(&it.template_id.0),
            ShopTabKind::MuaAvatar => inv.find_bag_slot(it.template_id).is_none(),
            _ => true,
        }).collect()
    }

    /// Executes a purchase from `(shop_type, tab_kind, listing)` with full currency validation.
    pub fn buy_listing(
        inv: &mut Inventory, _shop_type: ShopType, tab_kind: ShopTabKind, listing: &ShopItemListing,
        event_points: &mut u32, clan_capsules: &mut u32, wearing_full_than_linh: bool,
        catalog: &dyn ItemCatalog, rng: &mut GameRng,
    ) -> Result<Option<ItemInstance>, ShopError> {
        if tab_kind == ShopTabKind::HangDocHair { return Ok(None); }
        if tab_kind == ShopTabKind::ChipiExchange {
            let mut chipi = ItemInstance { template_id: listing.template_id, quantity: 1, options: listing.options.clone(), create_time_ms: 0 };
            if rng.is_true(5, 100) { set_opt(&mut chipi, opt_id::PERM_MARKER, 0); } else { set_opt(&mut chipi, opt_id::EXPIRY_DAYS, rng.next_range(3, 7)); }
            inv.add_to_bag(chipi.clone(), false)?;
            return Ok(Some(chipi));
        }
        if tab_kind == ShopTabKind::Discount80 { inv.sub_template_from_bag(TemplateId(const_item::PHIEU_GIAM_GIA as i16), 1)?; }
        if tab_kind == ShopTabKind::BillHuyDiet {
            if !wearing_full_than_linh { return Err(ShopError::RequirementNotMet("must wear 5 Thần Linh pieces")); }
            let fid = (663..=667).map(TemplateId).find(|&id| inv.count_template_in_bag(id) >= 99)
                .ok_or(ShopError::RequirementNotMet("requires 99× God food (663..=667)"))?;
            inv.sub_template_from_bag(fid, 99)?;
        }
        let cost = if tab_kind == ShopTabKind::Discount80 { (listing.cost * 20 / 100).max(1) } else { listing.cost };
        let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
        match (tab_kind, listing.currency) {
            (ShopTabKind::EventPoint, _) => { if *event_points < c32 { return Err(ShopError::RequirementNotMet("insufficient event points")); } *event_points -= c32; }
            (ShopTabKind::ClanCapsule, _) => { if *clan_capsules < c32 { return Err(ShopError::RequirementNotMet("insufficient clan capsules")); } *clan_capsules -= c32; }
            (_, ShopCurrency::Gold) => inv.sub_gold(cost)?,
            (_, ShopCurrency::Gem) => inv.sub_gem(c32)?,
            (_, ShopCurrency::Ruby) => inv.sub_ruby(c32)?,
            (_, ShopCurrency::ItemSpec(s)) if s.0 == const_item::NGOC as i16 => inv.sub_gem(c32)?,
            (_, ShopCurrency::ItemSpec(s)) => inv.sub_template_from_bag(s, c32)?,
        }
        if listing.template_id.0 == 517 { inv.expand_slots(ContainerKind::Bag)?; return Ok(None); }
        if listing.template_id.0 == 518 { inv.expand_slots(ContainerKind::Box)?; return Ok(None); }
        let mut granted = ItemInstance { template_id: listing.template_id, quantity: listing.quantity.max(1), options: listing.options.clone(), create_time_ms: 0 };
        if tab_kind == ShopTabKind::BillHuyDiet {
            let bonus = rng.next_range(0, 15);
            for o in &mut granted.options { if matches!(o.id, 0 | 6 | 7 | 22 | 23 | 47) { o.param += o.param * bonus / 100; } }
        }
        inv.add_to_bag(granted.clone(), catalog.meta_of(granted.template_id).stackable)?;
        Ok(Some(granted))
    }
}

/// Consignment tab classification (`ConsignShopService.getTabKiGui`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize)]
pub enum ConsignTab {
    /// `type 0..=2` (`Áo Quần`).
    AoQuan = 0,
    /// `type 3..=4` (`Găng Tay`).
    GangTay = 1,
    /// `type 29` (`Phụ Kiện`).
    PhuKien = 2,
    /// Dragon balls, stones, beans, misc (`Linh tinh`).
    LinhTinh = 3,
    /// Seller's own management view.
    SellerManage = 4,
}

/// Currency denomination on a consignment listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum ConsignCurrency {
    /// Priced in raw Gold (`moneyType == 0`).
    Gold,
    /// Priced in green Gems (`moneyType == 1`).
    Gem,
}

/// Single consignment market listing (`ConsignItem.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsignListing {
    /// Monotonic listing identifier.
    pub id: u64,
    /// Seller player ID.
    pub seller_id: PlayerId,
    /// Escrowed item stack.
    pub item: ItemInstance,
    /// Assigned market tab.
    pub tab: ConsignTab,
    /// Sale currency.
    pub currency: ConsignCurrency,
    /// Total sale price for the listed stack.
    pub price: u64,
    /// True once purchased and awaiting seller claim.
    pub sold: bool,
}

/// Hardened consignment market (`shop_ky_gui` / packet `-100`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConsignShop {
    next_id: u64,
    listings: Vec<ConsignListing>,
}

impl ConsignShop {
    /// Classifies an item into its [`ConsignTab`].
    #[must_use]
    pub const fn tab_for(item_type: u8) -> ConsignTab {
        match item_type { 0..=2 => ConsignTab::AoQuan, 3..=4 => ConsignTab::GangTay, 29 => ConsignTab::PhuKien, _ => ConsignTab::LinhTinh }
    }

    /// Returns active unsold listings in `tab`.
    #[must_use]
    pub fn listings_in_tab(&self, tab: ConsignTab) -> Vec<&ConsignListing> {
        self.listings.iter().filter(|l| !l.sold && l.tab == tab).collect()
    }

    /// Returns all listings owned by `seller_id` (both active unsold and sold awaiting claim).
    #[must_use]
    pub fn seller_listings(&self, seller_id: PlayerId) -> Vec<&ConsignListing> {
        self.listings.iter().filter(|l| l.seller_id == seller_id).collect()
    }

    /// Action `0` (`KiGui`): validates eligibility and caps *before* deducting `1× 457` listing fee.
    pub fn list_item(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, bag_slot: usize, currency: ConsignCurrency, price: u64, quantity: u32) -> Result<u64, ShopError> {
        if !(1..=99).contains(&quantity) { return Err(ShopError::InvalidConsignment("quantity must be in 1..=99")); }
        let ok_p = match currency { ConsignCurrency::Gold => (10_000..=10_000_000_000).contains(&price), ConsignCurrency::Gem => (10..=1_000_000).contains(&price) };
        if !ok_p { return Err(ShopError::InvalidConsignment("listing price out of bounds")); }
        let cand = seller_inv.bag_item(bag_slot)?;
        if is_untradable(cand) { return Err(ShopError::InvalidConsignment("item carries Opt 30 untradable lock")); }
        let meta = crate::ItemMeta::classify_builtin(cand.template_id);
        if !(get_opt(cand, opt_id::CONSIGN_GOLD).is_some() || get_opt(cand, opt_id::CONSIGN_GEM).is_some() || matches!(meta.item_type, 6 | 14 | 15) || (14..=20).contains(&cand.template_id.0)) {
            return Err(ShopError::InvalidConsignment("item not eligible for consignment"));
        }
        if seller_inv.count_template_in_bag(TemplateId(const_item::THOI_VANG as i16)) < 1 {
            return Err(ShopError::RequirementNotMet("requires 1 Thỏi vàng (457) listing fee"));
        }
        let escrowed = seller_inv.sub_from_bag_slot(bag_slot, quantity)?;
        seller_inv.sub_template_from_bag(TemplateId(const_item::THOI_VANG as i16), 1)?;
        self.next_id += 1;
        self.listings.push(ConsignListing { id: self.next_id, seller_id, tab: Self::tab_for(meta.item_type), item: escrowed, currency, price, sold: false });
        Ok(self.next_id)
    }

    /// Action `3` (`buyItem`): requires `buyer_power >= 17B`, deducts raw Gold or Gem, transfers item.
    pub fn buy_item(&mut self, buyer_inv: &mut Inventory, buyer_id: PlayerId, buyer_power: i64, listing_id: u64) -> Result<(), ShopError> {
        if buyer_power < CONSIGN_MIN_BUYER_POWER { return Err(ShopError::RequirementNotMet("requires >= 17B power to buy")); }
        let idx = self.listings.iter().position(|l| l.id == listing_id && !l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id == buyer_id { return Err(ShopError::InvalidConsignment("cannot buy own listing")); }
        if buyer_inv.empty_bag_slots() == 0 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
        let price = self.listings[idx].price;
        match self.listings[idx].currency {
            ConsignCurrency::Gold => buyer_inv.sub_gold(price)?,
            ConsignCurrency::Gem => buyer_inv.sub_gem(u32::try_from(price).unwrap_or(u32::MAX))?,
        }
        let item = self.listings[idx].item.clone();
        buyer_inv.add_to_bag(item.clone(), crate::ItemMeta::classify_builtin(item.template_id).stackable)?;
        self.listings[idx].sold = true;
        Ok(())
    }

    /// Action `1` (`claimOrDel` cancel): returns unsold escrowed item to seller's bag.
    pub fn cancel_listing(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64) -> Result<(), ShopError> {
        let idx = self.listings.iter().position(|l| l.id == listing_id && !l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id { return Err(ShopError::NotListingOwner(listing_id)); }
        let item = self.listings[idx].item.clone();
        seller_inv.add_to_bag(item.clone(), crate::ItemMeta::classify_builtin(item.template_id).stackable)?;
        self.listings.remove(idx);
        Ok(())
    }

    /// Action `2` (`claimOrDel` claim): pays `90%` net in **raw Gold** (`inventory.gold`) or **Gem**!
    pub fn claim_proceeds(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64) -> Result<u64, ShopError> {
        let idx = self.listings.iter().position(|l| l.id == listing_id && l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id { return Err(ShopError::NotListingOwner(listing_id)); }
        let listing = self.listings.remove(idx);
        let net = (listing.price * 90) / 100;
        match listing.currency {
            ConsignCurrency::Gold => seller_inv.add_gold_clamped(net),
            ConsignCurrency::Gem => seller_inv.gem = seller_inv.gem.saturating_add(u32::try_from(net).unwrap_or(u32::MAX)),
        }
        Ok(net)
    }

    /// Action `5` (`upItemToTop`): deducts `2× 457` (`Thỏi vàng`) and moves listing to index `0`.
    pub fn promote_to_top(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64) -> Result<(), ShopError> {
        let idx = self.listings.iter().position(|l| l.id == listing_id && !l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id { return Err(ShopError::NotListingOwner(listing_id)); }
        seller_inv.sub_template_from_bag(TemplateId(const_item::THOI_VANG as i16), 2)?;
        let entry = self.listings.remove(idx);
        self.listings.insert(0, entry);
        Ok(())
    }
}
