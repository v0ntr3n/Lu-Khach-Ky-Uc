//! Shop tabs (`TabShop*`), `BuyBackService`, and `ConsignShop` (`shop_ky_gui`).
//! Eliminates `ShopTab.java`'s `Runtime.getRuntime().halt(0)` IP DRM kill-switch, repairs Ruby and
//! SpecIcon Gem (`77`) deductions, and fixes `ConsignShopService` so Gold sales pay `90%` raw Gold.

use crate::ItemCatalog;
use crate::inventory::{ContainerKind, Inventory, InventoryError, opt_id, set_opt};
use nro_consts::const_item;
use nro_core::{GameRng, Gender, ItemInstance, ItemOption, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::time::{SystemTime, UNIX_EPOCH};
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

#[path = "shops_consign.rs"]
mod shops_consign;
pub use shops_consign::*;

/// Minimum buyer power (`17B`) required to purchase from the consignment market.
pub const CONSIGN_MIN_BUYER_POWER: i64 = 17_000_000_000;
/// Maximum recent sold items retained per player in [`BuyBackLedger`].
pub const MAX_BUYBACK_HISTORY: usize = 10;

fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Shop transaction pricing modes (`shop.type` in `nro1.sql`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShopType {
    Normal = 0,
    SkillLearn = 1,
    SpecIcon = 3,
    LuckyRoundBox = 4,
    BuyBack = 8,
}

/// Specialized `TabShop*` filter and purchase behaviors from `models/shop/`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize,
)]
pub enum ShopTabKind {
    Standard,
    Uron,
    HangDocHair,
    SantaMain,
    MuaAvatar,
    HocKyNang,
    DanhHieuUnowned,
    DanhHieuOwned,
    Discount80,
    ChipiExchange,
    EventPoint,
    ClanCapsule,
    BillHuyDiet,
}

/// Currency selector on a [`ShopItemListing`] (`ItemShop.typeSell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum ShopCurrency {
    Gold,
    Gem,
    Ruby,
    ItemSpec(TemplateId),
}

/// One purchasable entry inside a [`ShopTabDef`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopItemListing {
    pub template_id: TemplateId,
    pub quantity: u32,
    pub cost: u64,
    pub currency: ShopCurrency,
    pub gender: Option<Gender>,
    pub options: SmallVec<[ItemOption; 4]>,
}

/// Definition of a single shop tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopTabDef {
    pub tab_id: u16,
    pub kind: ShopTabKind,
    pub items: Vec<ShopItemListing>,
}

/// Definition of a complete NPC shop (`sql/nro1.sql` `shop`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopDef {
    pub tag_name: &'static str,
    pub shop_type: ShopType,
    pub tabs: Vec<ShopTabDef>,
}

/// Errors emitted by [`ShopService`], [`BuyBackLedger`], or [`ConsignShop`].
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ShopError {
    #[error("shop listing not found")]
    ListingNotFound,
    #[error("shop requirement not met: {0}")]
    RequirementNotMet(&'static str),
    #[error("invalid consignment listing: {0}")]
    InvalidConsignment(&'static str),
    #[error("caller does not own consignment listing {0}")]
    NotListingOwner(u64),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Per-player 10-item FIFO vendor sell/repurchase ledger (`BuyBackService.java`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyBackLedger {
    items: Vec<ItemInstance>,
}

impl BuyBackLedger {
    /// Borrows the retained vendor buyback history (`0..=10`).
    #[must_use]
    pub fn items(&self) -> &[ItemInstance] {
        &self.items
    }

    /// Sells `qty` from `bag_slot` (`37M` Gold per `457` Thỏi vàng, else `base_gold / 4`); excludes `457` from buyback.
    pub fn sell_from_bag(
        &mut self,
        inv: &mut Inventory,
        bag_slot: usize,
        qty: u32,
        catalog: &dyn ItemCatalog,
    ) -> Result<u64, ShopError> {
        let sold = inv.sub_from_bag_slot(bag_slot, qty)?;
        let is_thoi_vang = sold.template_id.0 == const_item::THOI_VANG as i16;
        let payout = if is_thoi_vang {
            u64::from(qty).saturating_mul(37_000_000)
        } else {
            (catalog.meta_of(sold.template_id).base_gold_value / 4)
                .max(1)
                .saturating_mul(u64::from(qty))
        };
        inv.add_gold_clamped(payout);
        if !is_thoi_vang {
            if self.items.len() >= MAX_BUYBACK_HISTORY {
                self.items.remove(0);
            }
            self.items.push(sold);
        }
        Ok(payout)
    }

    /// Repurchases `history_idx` at `gem / 2`, `gold / 2`, or `quantity * 100` Gold fallback.
    pub fn repurchase(
        &mut self,
        inv: &mut Inventory,
        history_idx: usize,
        catalog: &dyn ItemCatalog,
    ) -> Result<ItemInstance, ShopError> {
        let item = self
            .items
            .get(history_idx)
            .ok_or(ShopError::ListingNotFound)?;
        let meta = catalog.meta_of(item.template_id);
        let gem_cost = (meta.base_gem_value > 0).then(|| {
            (meta.base_gem_value / 2)
                .max(1)
                .saturating_mul(item.quantity)
        });
        let gold_cost = if gem_cost.is_some() {
            0
        } else if meta.base_gold_value > 0 {
            (meta.base_gold_value / 2)
                .max(1)
                .saturating_mul(u64::from(item.quantity))
        } else {
            u64::from(item.quantity).saturating_mul(100)
        };
        if let Some(gc) = gem_cost
            && inv.gem < gc
        {
            return Err(InventoryError::InsufficientCurrency(
                "gem",
                u64::from(gc),
                u64::from(inv.gem),
            )
            .into());
        }
        if gem_cost.is_none() && inv.gold < gold_cost {
            return Err(InventoryError::InsufficientCurrency("gold", gold_cost, inv.gold).into());
        }
        inv.add_to_bag(item.clone(), meta.stackable)?;
        if let Some(gc) = gem_cost {
            inv.sub_gem(gc)?;
        } else {
            inv.sub_gold(gold_cost)?;
        }
        Ok(self.items.remove(history_idx))
    }
}

/// Stateless shop filter & purchase processor.
pub struct ShopService;

impl ShopService {
    /// Filters visible listings for a [`ShopTabDef`] according to `TabShop*` rules.
    #[must_use]
    pub fn visible_items<'a>(
        tab: &'a ShopTabDef,
        inv: &Inventory,
        gender: Gender,
        magic_tree_lv: u8,
        learned_skills: &[i16],
    ) -> Vec<&'a ShopItemListing> {
        tab.items
            .iter()
            .filter(|it| match tab.kind {
                ShopTabKind::Uron => {
                    it.gender.is_none_or(|g| g == gender)
                        && (!matches!(it.template_id.0, 453 | 454)
                            || inv.find_bag_slot(it.template_id).is_none())
                        && (it.template_id.0 != 13 || magic_tree_lv >= 1)
                }
                ShopTabKind::SantaMain => {
                    !(it.template_id.0 == 517 && inv.bag.len() >= 80
                        || it.template_id.0 == 518 && inv.box_items.len() >= 100)
                }
                ShopTabKind::HocKyNang => !learned_skills.contains(&it.template_id.0),
                ShopTabKind::MuaAvatar => inv.find_bag_slot(it.template_id).is_none(),
                _ => true,
            })
            .collect()
    }

    /// Executes a purchase from `(shop_type, tab_kind, listing)`, stamping wall-clock `now_ms`.
    pub fn buy_listing(
        inv: &mut Inventory,
        shop_type: ShopType,
        tab_kind: ShopTabKind,
        listing: &ShopItemListing,
        event_points: &mut u32,
        clan_capsules: &mut u32,
        wearing_full_than_linh: bool,
        catalog: &dyn ItemCatalog,
        rng: &mut GameRng,
    ) -> Result<Option<ItemInstance>, ShopError> {
        Self::buy_listing_at(
            inv,
            shop_type,
            tab_kind,
            listing,
            event_points,
            clan_capsules,
            wearing_full_than_linh,
            catalog,
            rng,
            unix_now_ms(),
        )
    }

    /// Executes a purchase from `(shop_type, tab_kind, listing)` stamping explicit `now_ms`.
    pub fn buy_listing_at(
        inv: &mut Inventory,
        _shop_type: ShopType,
        tab_kind: ShopTabKind,
        listing: &ShopItemListing,
        event_points: &mut u32,
        clan_capsules: &mut u32,
        wearing_full_than_linh: bool,
        catalog: &dyn ItemCatalog,
        rng: &mut GameRng,
        now_ms: i64,
    ) -> Result<Option<ItemInstance>, ShopError> {
        if tab_kind == ShopTabKind::HangDocHair {
            return Ok(None);
        }
        if tab_kind == ShopTabKind::ChipiExchange {
            let mut chipi = ItemInstance {
                template_id: listing.template_id,
                quantity: 1,
                options: listing.options.clone(),
                create_time_ms: now_ms,
            };
            if rng.is_true(5, 100) {
                set_opt(&mut chipi, opt_id::PERM_MARKER, 0);
            } else {
                set_opt(&mut chipi, opt_id::EXPIRY_DAYS, rng.next_range(3, 7));
            }
            inv.add_to_bag(chipi.clone(), false)?;
            return Ok(Some(chipi));
        }
        let (mut staged, mut ep, mut cc) = (inv.clone(), *event_points, *clan_capsules);
        if tab_kind == ShopTabKind::Discount80 {
            staged.sub_template_from_bag(TemplateId(const_item::PHIEU_GIAM_GIA as i16), 1)?;
        }
        if tab_kind == ShopTabKind::BillHuyDiet {
            if !wearing_full_than_linh {
                return Err(ShopError::RequirementNotMet("must wear 5 Thần Linh pieces"));
            }
            let fid = (663..=667)
                .map(TemplateId)
                .find(|&id| staged.count_template_in_bag(id) >= 99)
                .ok_or(ShopError::RequirementNotMet(
                    "requires 99× God food (663..=667)",
                ))?;
            staged.sub_template_from_bag(fid, 99)?;
        }
        let cost = if tab_kind == ShopTabKind::Discount80 {
            u64::try_from((u128::from(listing.cost) * 20) / 100)
                .unwrap_or(u64::MAX)
                .max(1)
        } else {
            listing.cost
        };
        let c32 = u32::try_from(cost).unwrap_or(u32::MAX);
        match (tab_kind, listing.currency) {
            (ShopTabKind::EventPoint, _) => {
                if ep < c32 {
                    return Err(ShopError::RequirementNotMet("insufficient event points"));
                }
                ep -= c32;
            }
            (ShopTabKind::ClanCapsule, _) => {
                if cc < c32 {
                    return Err(ShopError::RequirementNotMet("insufficient clan capsules"));
                }
                cc -= c32;
            }
            (_, ShopCurrency::Gold) => staged.sub_gold(cost)?,
            (_, ShopCurrency::Gem) => staged.sub_gem(c32)?,
            (_, ShopCurrency::Ruby) => staged.sub_ruby(c32)?,
            (_, ShopCurrency::ItemSpec(s)) if s.0 == const_item::NGOC as i16 => {
                staged.sub_gem(c32)?
            }
            (_, ShopCurrency::ItemSpec(s)) => staged.sub_template_from_bag(s, c32)?,
        }
        if listing.template_id.0 == 517 {
            staged.expand_slots(ContainerKind::Bag)?;
            (*inv, *event_points, *clan_capsules) = (staged, ep, cc);
            return Ok(None);
        }
        if listing.template_id.0 == 518 {
            staged.expand_slots(ContainerKind::Box)?;
            (*inv, *event_points, *clan_capsules) = (staged, ep, cc);
            return Ok(None);
        }
        let mut granted = ItemInstance {
            template_id: listing.template_id,
            quantity: listing.quantity.max(1),
            options: listing.options.clone(),
            create_time_ms: now_ms,
        };
        if tab_kind == ShopTabKind::BillHuyDiet {
            let bonus = rng.next_range(0, 15);
            for o in &mut granted.options {
                if matches!(o.id, 0 | 6 | 7 | 22 | 23 | 47) {
                    let p = i64::from(o.param);
                    o.param = (p + p * i64::from(bonus) / 100).clamp(1, i64::from(i32::MAX)) as i32;
                }
            }
        }
        staged.add_to_bag(
            granted.clone(),
            catalog.meta_of(granted.template_id).stackable,
        )?;
        (*inv, *event_points, *clan_capsules) = (staged, ep, cc);
        Ok(Some(granted))
    }
}
