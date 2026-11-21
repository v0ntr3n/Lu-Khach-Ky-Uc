//! Shop tabs (`TabShop*`), `BuyBackService`, and `ConsignShop` (`shop_ky_gui`).
//!
//! Eliminates `ShopTab.java`'s `Runtime.getRuntime().halt(0)` IP DRM kill-switch, repairs Ruby and
//! SpecIcon Gem (`77`) deductions, and fixes `ConsignShopService` so Gold sales pay `90%` raw Gold.

use crate::inventory::{ContainerKind, Inventory, InventoryError, LIMIT_GOLD, get_opt, is_untradable, opt_id, set_opt};
use crate::{BuiltinCatalog, ItemCatalog};
use nro_consts::const_item;
use nro_core::{GameRng, Gender, ItemInstance, ItemOption, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::time::{SystemTime, UNIX_EPOCH};
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

/// Minimum buyer power (`17B`) required to purchase from the consignment market.
pub const CONSIGN_MIN_BUYER_POWER: i64 = 17_000_000_000;
/// Maximum recent sold items retained per player in [`BuyBackLedger`].
pub const MAX_BUYBACK_HISTORY: usize = 10;

fn unix_now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

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
    /// Sells `qty` from `bag_slot` (`37M` Gold per `457` Thỏi vàng, else `base_gold / 4`); excludes `457` from buyback.
    pub fn sell_from_bag(&mut self, inv: &mut Inventory, bag_slot: usize, qty: u32, catalog: &dyn ItemCatalog) -> Result<u64, ShopError> {
        let sold = inv.sub_from_bag_slot(bag_slot, qty)?;
        let is_thoi_vang = sold.template_id.0 == const_item::THOI_VANG as i16;
        let payout = if is_thoi_vang { u64::from(qty).saturating_mul(37_000_000) } else { (catalog.meta_of(sold.template_id).base_gold_value / 4).max(1).saturating_mul(u64::from(qty)) };
        inv.add_gold_clamped(payout);
        if !is_thoi_vang {
            if self.items.len() >= MAX_BUYBACK_HISTORY { self.items.remove(0); }
            self.items.push(sold);
        }
        Ok(payout)
    }

    /// Repurchases `history_idx` at `gem / 2`, `gold / 2`, or `quantity * 100` Gold fallback.
    pub fn repurchase(&mut self, inv: &mut Inventory, history_idx: usize, catalog: &dyn ItemCatalog) -> Result<ItemInstance, ShopError> {
        let item = self.items.get(history_idx).ok_or(ShopError::ListingNotFound)?;
        let meta = catalog.meta_of(item.template_id);
        let gem_cost = (meta.base_gem_value > 0).then(|| (meta.base_gem_value / 2).max(1).saturating_mul(item.quantity));
        let gold_cost = if gem_cost.is_some() { 0 } else if meta.base_gold_value > 0 { (meta.base_gold_value / 2).max(1).saturating_mul(u64::from(item.quantity)) } else { u64::from(item.quantity).saturating_mul(100) };
        if let Some(gc) = gem_cost && inv.gem < gc { return Err(InventoryError::InsufficientCurrency("gem", u64::from(gc), u64::from(inv.gem)).into()); }
        if gem_cost.is_none() && inv.gold < gold_cost { return Err(InventoryError::InsufficientCurrency("gold", gold_cost, inv.gold).into()); }
        inv.add_to_bag(item.clone(), meta.stackable)?;
        if let Some(gc) = gem_cost { inv.sub_gem(gc)?; } else { inv.sub_gold(gold_cost)?; }
        Ok(self.items.remove(history_idx))
    }
}

/// Stateless shop filter & purchase processor.
pub struct ShopService;

impl ShopService {
    /// Filters visible listings for a [`ShopTabDef`] according to `TabShop*` rules.
    #[must_use]
    pub fn visible_items<'a>(tab: &'a ShopTabDef, inv: &Inventory, gender: Gender, magic_tree_lv: u8, learned_skills: &[i16]) -> Vec<&'a ShopItemListing> {
        tab.items.iter().filter(|it| match tab.kind {
            ShopTabKind::Uron => it.gender.is_none_or(|g| g == gender) && (!matches!(it.template_id.0, 453 | 454) || inv.find_bag_slot(it.template_id).is_none()) && (it.template_id.0 != 13 || magic_tree_lv >= 1),
            ShopTabKind::SantaMain => !(it.template_id.0 == 517 && inv.bag.len() >= 80 || it.template_id.0 == 518 && inv.box_items.len() >= 100),
            ShopTabKind::HocKyNang => !learned_skills.contains(&it.template_id.0),
            ShopTabKind::MuaAvatar => inv.find_bag_slot(it.template_id).is_none(),
            _ => true,
        }).collect()
    }

    /// Executes a purchase from `(shop_type, tab_kind, listing)`, stamping wall-clock `now_ms`.
    pub fn buy_listing(inv: &mut Inventory, shop_type: ShopType, tab_kind: ShopTabKind, listing: &ShopItemListing, event_points: &mut u32, clan_capsules: &mut u32, wearing_full_than_linh: bool, catalog: &dyn ItemCatalog, rng: &mut GameRng) -> Result<Option<ItemInstance>, ShopError> {
        Self::buy_listing_at(inv, shop_type, tab_kind, listing, event_points, clan_capsules, wearing_full_than_linh, catalog, rng, unix_now_ms())
    }

    /// Executes a purchase from `(shop_type, tab_kind, listing)` stamping explicit `now_ms`.
    pub fn buy_listing_at(inv: &mut Inventory, _shop_type: ShopType, tab_kind: ShopTabKind, listing: &ShopItemListing, event_points: &mut u32, clan_capsules: &mut u32, wearing_full_than_linh: bool, catalog: &dyn ItemCatalog, rng: &mut GameRng, now_ms: i64) -> Result<Option<ItemInstance>, ShopError> {
        if tab_kind == ShopTabKind::HangDocHair { return Ok(None); }
        if tab_kind == ShopTabKind::ChipiExchange {
            let mut chipi = ItemInstance { template_id: listing.template_id, quantity: 1, options: listing.options.clone(), create_time_ms: now_ms };
            if rng.is_true(5, 100) { set_opt(&mut chipi, opt_id::PERM_MARKER, 0); } else { set_opt(&mut chipi, opt_id::EXPIRY_DAYS, rng.next_range(3, 7)); }
            inv.add_to_bag(chipi.clone(), false)?;
            return Ok(Some(chipi));
        }
        let (mut staged, mut ep, mut cc) = (inv.clone(), *event_points, *clan_capsules);
        if tab_kind == ShopTabKind::Discount80 { staged.sub_template_from_bag(TemplateId(const_item::PHIEU_GIAM_GIA as i16), 1)?; }
        if tab_kind == ShopTabKind::BillHuyDiet {
            if !wearing_full_than_linh { return Err(ShopError::RequirementNotMet("must wear 5 Thần Linh pieces")); }
            let fid = (663..=667).map(TemplateId).find(|&id| staged.count_template_in_bag(id) >= 99).ok_or(ShopError::RequirementNotMet("requires 99× God food (663..=667)"))?;
            staged.sub_template_from_bag(fid, 99)?;
        }
        let cost = if tab_kind == ShopTabKind::Discount80 { u64::try_from((u128::from(listing.cost) * 20) / 100).unwrap_or(u64::MAX).max(1) } else { listing.cost };
        let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
        match (tab_kind, listing.currency) {
            (ShopTabKind::EventPoint, _) => { if ep < c32 { return Err(ShopError::RequirementNotMet("insufficient event points")); } ep -= c32; }
            (ShopTabKind::ClanCapsule, _) => { if cc < c32 { return Err(ShopError::RequirementNotMet("insufficient clan capsules")); } cc -= c32; }
            (_, ShopCurrency::Gold) => staged.sub_gold(cost)?,
            (_, ShopCurrency::Gem) => staged.sub_gem(c32)?,
            (_, ShopCurrency::Ruby) => staged.sub_ruby(c32)?,
            (_, ShopCurrency::ItemSpec(s)) if s.0 == const_item::NGOC as i16 => staged.sub_gem(c32)?,
            (_, ShopCurrency::ItemSpec(s)) => staged.sub_template_from_bag(s, c32)?,
        }
        if listing.template_id.0 == 517 { staged.expand_slots(ContainerKind::Bag)?; (*inv, *event_points, *clan_capsules) = (staged, ep, cc); return Ok(None); }
        if listing.template_id.0 == 518 { staged.expand_slots(ContainerKind::Box)?; (*inv, *event_points, *clan_capsules) = (staged, ep, cc); return Ok(None); }
        let mut granted = ItemInstance { template_id: listing.template_id, quantity: listing.quantity.max(1), options: listing.options.clone(), create_time_ms: now_ms };
        if tab_kind == ShopTabKind::BillHuyDiet {
            let bonus = rng.next_range(0, 15);
            for o in &mut granted.options { if matches!(o.id, 0 | 6 | 7 | 22 | 23 | 47) { let p = i64::from(o.param); o.param = (p + p * i64::from(bonus) / 100).clamp(1, i64::from(i32::MAX)) as i32; } }
        }
        staged.add_to_bag(granted.clone(), catalog.meta_of(granted.template_id).stackable)?; (*inv, *event_points, *clan_capsules) = (staged, ep, cc);
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
    /// Listing identifier within `1..=i16::MAX`.
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
    pub fn listings_in_tab(&self, tab: ConsignTab) -> Vec<&ConsignListing> { self.listings.iter().filter(|l| !l.sold && l.tab == tab).collect() }
    /// Returns all listings owned by `seller_id` (both active unsold and sold awaiting claim).
    #[must_use]
    pub fn seller_listings(&self, seller_id: PlayerId) -> Vec<&ConsignListing> { self.listings.iter().filter(|l| l.seller_id == seller_id).collect() }

    fn next_free_id(&self) -> Result<u64, ShopError> {
        const MAX_ID: u64 = i16::MAX.cast_unsigned() as u64;
        let mut cand = self.next_id % MAX_ID;
        for _ in 0..MAX_ID {
            cand = (cand % MAX_ID) + 1;
            if !self.listings.iter().any(|l| l.id == cand) { return Ok(cand); }
        }
        Err(ShopError::InvalidConsignment("consignment market full"))
    }

    /// Action `0` (`KiGui`): validates eligibility and caps *before* deducting `1× 457` listing fee.
    pub fn list_item(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, bag_slot: usize, currency: ConsignCurrency, price: u64, quantity: u32) -> Result<u64, ShopError> {
        self.list_item_with_catalog(seller_inv, seller_id, bag_slot, currency, price, quantity, &BuiltinCatalog)
    }

    /// Action `0` (`KiGui`) using an explicit [`ItemCatalog`] for item classification.
    pub fn list_item_with_catalog(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, bag_slot: usize, currency: ConsignCurrency, price: u64, quantity: u32, catalog: &dyn ItemCatalog) -> Result<u64, ShopError> {
        if !(1..=99).contains(&quantity) { return Err(ShopError::InvalidConsignment("quantity must be in 1..=99")); }
        let ok_p = match currency { ConsignCurrency::Gold => (10_000..=u64::from(i32::MAX.cast_unsigned())).contains(&price), ConsignCurrency::Gem => (10..=1_000_000).contains(&price) };
        if !ok_p { return Err(ShopError::InvalidConsignment("listing price out of bounds")); }
        let cand = seller_inv.bag_item(bag_slot)?;
        if is_untradable(cand) { return Err(ShopError::InvalidConsignment("item carries Opt 30 untradable lock")); }
        let meta = catalog.meta_of(cand.template_id);
        if !(get_opt(cand, opt_id::CONSIGN_GOLD).is_some() || get_opt(cand, opt_id::CONSIGN_GEM).is_some() || matches!(meta.item_type, 6 | 14 | 15) || (14..=20).contains(&cand.template_id.0)) { return Err(ShopError::InvalidConsignment("item not eligible for consignment")); }
        let needed_457 = if cand.template_id.0 == const_item::THOI_VANG as i16 { quantity.saturating_add(1) } else { 1 };
        if seller_inv.count_template_in_bag(TemplateId(const_item::THOI_VANG as i16)) < needed_457 { return Err(ShopError::RequirementNotMet("requires 1 Thỏi vàng (457) listing fee")); }
        let id = self.next_free_id()?;
        let escrowed = seller_inv.sub_from_bag_slot(bag_slot, quantity)?;
        seller_inv.sub_template_from_bag(TemplateId(const_item::THOI_VANG as i16), 1)?;
        self.next_id = id;
        self.listings.push(ConsignListing { id, seller_id, tab: Self::tab_for(meta.item_type), item: escrowed, currency, price, sold: false });
        Ok(id)
    }

    /// Action `3` (`buyItem`): requires `buyer_power >= 17B`, deducts raw Gold or Gem, transfers item.
    pub fn buy_item(&mut self, buyer_inv: &mut Inventory, buyer_id: PlayerId, buyer_power: i64, listing_id: u64) -> Result<(), ShopError> {
        self.buy_item_with_catalog(buyer_inv, buyer_id, buyer_power, listing_id, &BuiltinCatalog)
    }

    /// Action `3` (`buyItem`) using an explicit [`ItemCatalog`] for stackability rules.
    pub fn buy_item_with_catalog(&mut self, buyer_inv: &mut Inventory, buyer_id: PlayerId, buyer_power: i64, listing_id: u64, catalog: &dyn ItemCatalog) -> Result<(), ShopError> {
        if buyer_power < CONSIGN_MIN_BUYER_POWER { return Err(ShopError::RequirementNotMet("requires >= 17B power to buy")); }
        let idx = self.listings.iter().position(|l| l.id == listing_id && !l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id == buyer_id { return Err(ShopError::InvalidConsignment("cannot buy own listing")); }
        if buyer_inv.empty_bag_slots() == 0 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
        let (price, item, mut staged) = (self.listings[idx].price, self.listings[idx].item.clone(), buyer_inv.clone());
        match self.listings[idx].currency {
            ConsignCurrency::Gold => staged.sub_gold(price)?,
            ConsignCurrency::Gem => staged.sub_gem(u32::try_from(price).unwrap_or(u32::MAX))?,
        }
        let stackable = catalog.meta_of(item.template_id).stackable;
        staged.add_to_bag(item, stackable)?;
        *buyer_inv = staged; self.listings[idx].sold = true;
        Ok(())
    }

    /// Action `1` (`claimOrDel` cancel): returns unsold escrowed item to seller's bag.
    pub fn cancel_listing(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64) -> Result<(), ShopError> {
        self.cancel_listing_with_catalog(seller_inv, seller_id, listing_id, &BuiltinCatalog)
    }

    /// Action `1` (`claimOrDel` cancel) using an explicit [`ItemCatalog`] for stackability rules.
    pub fn cancel_listing_with_catalog(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64, catalog: &dyn ItemCatalog) -> Result<(), ShopError> {
        let idx = self.listings.iter().position(|l| l.id == listing_id && !l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id { return Err(ShopError::NotListingOwner(listing_id)); }
        let item = self.listings[idx].item.clone();
        seller_inv.add_to_bag(item.clone(), catalog.meta_of(item.template_id).stackable)?;
        self.listings.remove(idx);
        Ok(())
    }

    /// Action `2` (`claimOrDel` claim): pays `90%` net in **raw Gold** (`inventory.gold`) or **Gem**!
    pub fn claim_proceeds(&mut self, seller_inv: &mut Inventory, seller_id: PlayerId, listing_id: u64) -> Result<u64, ShopError> {
        let idx = self.listings.iter().position(|l| l.id == listing_id && l.sold).ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id { return Err(ShopError::NotListingOwner(listing_id)); }
        let net = (self.listings[idx].price * 90) / 100;
        match self.listings[idx].currency {
            ConsignCurrency::Gold if seller_inv.gold.saturating_add(net) > LIMIT_GOLD => return Err(ShopError::RequirementNotMet("gold would exceed LIMIT_GOLD")),
            ConsignCurrency::Gem if u64::from(seller_inv.gem).saturating_add(net) > u64::from(i32::MAX.cast_unsigned()) => return Err(ShopError::RequirementNotMet("gem would exceed max limit")),
            _ => {}
        }
        let listing = self.listings.remove(idx);
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
