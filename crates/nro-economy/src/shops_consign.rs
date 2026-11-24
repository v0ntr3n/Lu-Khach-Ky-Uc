//! Hardened consignment market (`ConsignShop` / `shop_ky_gui` / packet `-100`).

use super::{CONSIGN_MIN_BUYER_POWER, ShopError};
use crate::inventory::{
    ContainerKind, Inventory, InventoryError, LIMIT_GOLD, get_opt, is_untradable, opt_id,
};
use crate::{BuiltinCatalog, ItemCatalog};
use nro_consts::const_item;
use nro_core::{ItemInstance, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumCount, EnumIter};

/// Consignment tab classification (`ConsignShopService.getTabKiGui`).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize,
)]
pub enum ConsignTab {
    AoQuan = 0,
    GangTay = 1,
    PhuKien = 2,
    LinhTinh = 3,
    SellerManage = 4,
}

/// Currency denomination on a consignment listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum ConsignCurrency {
    Gold,
    Gem,
}

/// Single consignment market listing (`ConsignItem.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsignListing {
    pub id: u64,
    pub seller_id: PlayerId,
    pub item: ItemInstance,
    pub tab: ConsignTab,
    pub currency: ConsignCurrency,
    pub price: u64,
    pub sold: bool,
}

/// Hardened consignment market (`shop_ky_gui` / packet `-100`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConsignShop {
    next_id: u64,
    listings: Vec<ConsignListing>,
}

impl ConsignShop {
    #[must_use]
    pub const fn tab_for(item_type: u8) -> ConsignTab {
        match item_type {
            0..=2 => ConsignTab::AoQuan,
            3..=4 => ConsignTab::GangTay,
            29 => ConsignTab::PhuKien,
            _ => ConsignTab::LinhTinh,
        }
    }
    #[must_use]
    pub fn listings_in_tab(&self, tab: ConsignTab) -> Vec<&ConsignListing> {
        self.listings
            .iter()
            .filter(|l| !l.sold && l.tab == tab)
            .collect()
    }
    #[must_use]
    pub fn seller_listings(&self, seller_id: PlayerId) -> Vec<&ConsignListing> {
        self.listings
            .iter()
            .filter(|l| l.seller_id == seller_id)
            .collect()
    }

    fn next_free_id(&self) -> Result<u64, ShopError> {
        const MAX_ID: u64 = i16::MAX.cast_unsigned() as u64;
        let mut cand = self.next_id % MAX_ID;
        for _ in 0..MAX_ID {
            cand = (cand % MAX_ID) + 1;
            if !self.listings.iter().any(|l| l.id == cand) {
                return Ok(cand);
            }
        }
        Err(ShopError::InvalidConsignment("consignment market full"))
    }

    /// Action `0` (`KiGui`): validates eligibility and caps *before* deducting `1× 457` listing fee.
    pub fn list_item(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        bag_slot: usize,
        currency: ConsignCurrency,
        price: u64,
        quantity: u32,
    ) -> Result<u64, ShopError> {
        self.list_item_with_catalog(
            seller_inv,
            seller_id,
            bag_slot,
            currency,
            price,
            quantity,
            &BuiltinCatalog,
        )
    }

    /// Action `0` (`KiGui`) using an explicit [`ItemCatalog`] for item classification.
    pub fn list_item_with_catalog(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        bag_slot: usize,
        currency: ConsignCurrency,
        price: u64,
        quantity: u32,
        catalog: &dyn ItemCatalog,
    ) -> Result<u64, ShopError> {
        if !(1..=99).contains(&quantity) {
            return Err(ShopError::InvalidConsignment("quantity must be in 1..=99"));
        }
        let ok_p = match currency {
            ConsignCurrency::Gold => {
                (10_000..=u64::from(i32::MAX.cast_unsigned())).contains(&price)
            }
            ConsignCurrency::Gem => (10..=1_000_000).contains(&price),
        };
        if !ok_p {
            return Err(ShopError::InvalidConsignment("listing price out of bounds"));
        }
        let cand = seller_inv.bag_item(bag_slot)?;
        if is_untradable(cand) {
            return Err(ShopError::InvalidConsignment(
                "item carries Opt 30 untradable lock",
            ));
        }
        let meta = catalog.meta_of(cand.template_id);
        if !(get_opt(cand, opt_id::CONSIGN_GOLD).is_some()
            || get_opt(cand, opt_id::CONSIGN_GEM).is_some()
            || matches!(meta.item_type, 6 | 14 | 15)
            || (14..=20).contains(&cand.template_id.0))
        {
            return Err(ShopError::InvalidConsignment(
                "item not eligible for consignment",
            ));
        }
        let needed_457 = if cand.template_id.0 == const_item::THOI_VANG as i16 {
            quantity.saturating_add(1)
        } else {
            1
        };
        if seller_inv.count_template_in_bag(TemplateId(const_item::THOI_VANG as i16)) < needed_457 {
            return Err(ShopError::RequirementNotMet(
                "requires 1 Thỏi vàng (457) listing fee",
            ));
        }
        let id = self.next_free_id()?;
        let escrowed = seller_inv.sub_from_bag_slot(bag_slot, quantity)?;
        seller_inv.sub_template_from_bag(TemplateId(const_item::THOI_VANG as i16), 1)?;
        self.next_id = id;
        self.listings.push(ConsignListing {
            id,
            seller_id,
            tab: Self::tab_for(meta.item_type),
            item: escrowed,
            currency,
            price,
            sold: false,
        });
        Ok(id)
    }

    /// Action `3` (`buyItem`): requires `buyer_power >= 17B`, deducts raw Gold or Gem, transfers item.
    pub fn buy_item(
        &mut self,
        buyer_inv: &mut Inventory,
        buyer_id: PlayerId,
        buyer_power: i64,
        listing_id: u64,
    ) -> Result<(), ShopError> {
        self.buy_item_with_catalog(
            buyer_inv,
            buyer_id,
            buyer_power,
            listing_id,
            &BuiltinCatalog,
        )
    }

    /// Action `3` (`buyItem`) using an explicit [`ItemCatalog`] for stackability rules.
    pub fn buy_item_with_catalog(
        &mut self,
        buyer_inv: &mut Inventory,
        buyer_id: PlayerId,
        buyer_power: i64,
        listing_id: u64,
        catalog: &dyn ItemCatalog,
    ) -> Result<(), ShopError> {
        if buyer_power < CONSIGN_MIN_BUYER_POWER {
            return Err(ShopError::RequirementNotMet("requires >= 17B power to buy"));
        }
        let idx = self
            .listings
            .iter()
            .position(|l| l.id == listing_id && !l.sold)
            .ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id == buyer_id {
            return Err(ShopError::InvalidConsignment("cannot buy own listing"));
        }
        if buyer_inv.empty_bag_slots() == 0 {
            return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
        }
        let (price, item, mut staged) = (
            self.listings[idx].price,
            self.listings[idx].item.clone(),
            buyer_inv.clone(),
        );
        match self.listings[idx].currency {
            ConsignCurrency::Gold => staged.sub_gold(price)?,
            ConsignCurrency::Gem => staged.sub_gem(u32::try_from(price).unwrap_or(u32::MAX))?,
        }
        let stackable = catalog.meta_of(item.template_id).stackable;
        staged.add_to_bag(item, stackable)?;
        *buyer_inv = staged;
        self.listings[idx].sold = true;
        Ok(())
    }

    /// Action `1` (`claimOrDel` cancel): returns unsold escrowed item to seller's bag.
    pub fn cancel_listing(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        listing_id: u64,
    ) -> Result<(), ShopError> {
        self.cancel_listing_with_catalog(seller_inv, seller_id, listing_id, &BuiltinCatalog)
    }

    /// Action `1` (`claimOrDel` cancel) using an explicit [`ItemCatalog`] for stackability rules.
    pub fn cancel_listing_with_catalog(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        listing_id: u64,
        catalog: &dyn ItemCatalog,
    ) -> Result<(), ShopError> {
        let idx = self
            .listings
            .iter()
            .position(|l| l.id == listing_id && !l.sold)
            .ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id {
            return Err(ShopError::NotListingOwner(listing_id));
        }
        let item = self.listings[idx].item.clone();
        seller_inv.add_to_bag(item.clone(), catalog.meta_of(item.template_id).stackable)?;
        self.listings.remove(idx);
        Ok(())
    }

    /// Action `2` (`claimOrDel` claim): pays `90%` net in **raw Gold** (`inventory.gold`) or **Gem**!
    pub fn claim_proceeds(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        listing_id: u64,
    ) -> Result<u64, ShopError> {
        let idx = self
            .listings
            .iter()
            .position(|l| l.id == listing_id && l.sold)
            .ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id {
            return Err(ShopError::NotListingOwner(listing_id));
        }
        let net = (self.listings[idx].price * 90) / 100;
        match self.listings[idx].currency {
            ConsignCurrency::Gold if seller_inv.gold.saturating_add(net) > LIMIT_GOLD => {
                return Err(ShopError::RequirementNotMet("gold would exceed LIMIT_GOLD"));
            }
            ConsignCurrency::Gem
                if u64::from(seller_inv.gem).saturating_add(net)
                    > u64::from(i32::MAX.cast_unsigned()) =>
            {
                return Err(ShopError::RequirementNotMet("gem would exceed max limit"));
            }
            _ => {}
        }
        let listing = self.listings.remove(idx);
        match listing.currency {
            ConsignCurrency::Gold => seller_inv.add_gold_clamped(net),
            ConsignCurrency::Gem => {
                seller_inv.gem = seller_inv
                    .gem
                    .saturating_add(u32::try_from(net).unwrap_or(u32::MAX))
            }
        }
        Ok(net)
    }

    /// Action `5` (`upItemToTop`): deducts `2× 457` (`Thỏi vàng`) and moves listing to index `0`.
    pub fn promote_to_top(
        &mut self,
        seller_inv: &mut Inventory,
        seller_id: PlayerId,
        listing_id: u64,
    ) -> Result<(), ShopError> {
        let idx = self
            .listings
            .iter()
            .position(|l| l.id == listing_id && !l.sold)
            .ok_or(ShopError::ListingNotFound)?;
        if self.listings[idx].seller_id != seller_id {
            return Err(ShopError::NotListingOwner(listing_id));
        }
        seller_inv.sub_template_from_bag(TemplateId(const_item::THOI_VANG as i16), 2)?;
        let entry = self.listings.remove(idx);
        self.listings.insert(0, entry);
        Ok(())
    }
}
