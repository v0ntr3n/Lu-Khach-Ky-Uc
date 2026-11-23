//! Complete `UseItem` dispatcher (`UseItem.java`), buffs, `Pet2`, chests, `MagicTree`, eggs, `LuckyRound`, and `GiftCodeService`.

use crate::inventory::{Inventory, InventoryError, get_opt, opt_id};
use crate::{ItemCatalog, ItemMeta};
use nro_consts::{const_item, const_player};
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, ItemOption, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter};
use thiserror::Error;

#[path = "use_item_extra.rs"]
mod use_item_extra;
#[path = "use_item_pea.rs"]
mod use_item_pea;

pub use use_item_pea::{
    DuaHauEggState, GiftCodeDef, GiftCodeService, LuckyRoundMode, LuckyRoundService, MabuEggState,
    MagicTreeState,
};

/// All 68 companion `Pet2` item IDs mapped to `(head, body, leg)` sprite triplets (`UseItem.java:304-648`).
pub const PET2_SPRITES: &[(i16, (i16, i16, i16))] = &[
    (892, (882, 883, 884)),
    (893, (885, 886, 887)),
    (908, (891, 892, 893)),
    (909, (894, 895, 896)),
    (910, (897, 898, 899)),
    (916, (925, 926, 927)),
    (917, (928, 929, 930)),
    (918, (931, 932, 933)),
    (919, (934, 935, 936)),
    (936, (718, 719, 720)),
    (942, (966, 967, 968)),
    (943, (969, 970, 971)),
    (944, (972, 973, 974)),
    (967, (1050, 1051, 1052)),
    (1008, (1074, 1075, 1076)),
    (1039, (1089, 1090, 1091)),
    (1040, (1092, 1093, 1094)),
    (1046, (-1, -1, -1)),
    (1107, (1155, 1156, 1157)),
    (1114, (1158, 1159, 1160)),
    (1188, (1183, 1184, 1185)),
    (1202, (1201, 1202, 1203)),
    (1203, (1201, 1202, 1203)),
    (1207, (1077, 1078, 1079)),
    (1224, (1227, 1228, 1229)),
    (1225, (1233, 1234, 1235)),
    (1226, (1230, 1231, 1232)),
    (1243, (1245, 1246, 1247)),
    (1244, (1248, 1249, 1250)),
    (1256, (1267, 1268, 1269)),
    (1318, (1299, 1300, 1301)),
    (1347, (1302, 1303, 1304)),
    (1414, (1341, 1342, 1343)),
    (1435, (1347, 1348, 1349)),
    (1452, (1365, 1366, 1367)),
    (1458, (1368, 1369, 1370)),
    (1482, (1398, 1399, 1400)),
    (1497, (1401, 1402, 1403)),
    (1550, (1428, 1429, 1430)),
    (1551, (1425, 1426, 1427)),
    (1564, (1437, 1438, 1439)),
    (1568, (1443, 1444, 1445)),
    (1573, (1446, 1447, 1448)),
    (1596, (1473, 1474, 1475)),
    (1597, (1473, 1474, 1475)),
    (1611, (1488, 1494, 1495)),
    (1620, (1496, 1497, 1498)),
    (1621, (1496, 1497, 1498)),
    (1622, (1488, 1489, 1490)),
    (1629, (1505, 1506, 1507)),
    (1630, (1508, 1509, 1510)),
    (1631, (1513, 1516, 1517)),
    (1633, (1523, 1524, 1525)),
    (1654, (1526, 1529, 1530)),
    (1668, (1550, 1551, 1552)),
    (1682, (1558, 1559, 1560)),
    (1683, (1561, 1562, 1563)),
    (1686, (1572, 1573, 1574)),
    (1727, (1616, 1617, 1618)),
    (1729, (1621, 1622, 1623)),
    (1750, (1464, 1465, 1466)),
    (1765, (1662, 1663, 1664)),
    (1766, (1665, 1666, 1667)),
    (1767, (1668, 1669, 1670)),
    (1768, (1671, 1672, 1673)),
    (1769, (1674, 1675, 1676)),
    (1770, (1677, 1678, 1679)),
    (1771, (1680, 1681, 1682)),
    (1789, (1724, 1725, 1726)),
];

/// Timed buff channels (`ItemTime.java` + fixed `1635`, `1615`, `1616` expiration timestamps).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize,
)]
pub enum BuffSlot {
    MayDoCapsule,
    MayDoKhoBau,
    CuongNo(bool),
    BoHuyet(bool),
    BoKhi(bool),
    GiapXen(bool),
    AnDanh(bool),
    DuoiKhi,
    Commeson,
    MealTier1(i16),
    MealTier2(i16),
    KhauTrang,
    NoiComDien,
    NuocMia(u8),
    X2PetExp,
    CoBonLa,
    AutoTrain,
}

/// Active player buff state tracking expiration epochs (`ms`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedBuffState {
    pub active: SmallVec<[(BuffSlot, i64); 8]>,
}

impl TimedBuffState {
    #[must_use]
    pub fn has_active(&self, now_ms: i64, pred: impl Fn(BuffSlot) -> bool) -> bool {
        self.active.iter().any(|&(s, exp)| exp > now_ms && pred(s))
    }

    pub fn activate(&mut self, slot: BuffSlot, now_ms: i64, duration_ms: i64) {
        self.active.retain(|&mut (_, exp)| exp > now_ms);
        let exp = now_ms.saturating_add(duration_ms);
        if let Some(e) = self.active.iter_mut().find(|(s, _)| *s == slot) {
            e.1 = exp;
        } else {
            self.active.push((slot, exp));
        }
    }
}

/// Mutable gameplay context passed into [`UseItemService::use_bag_item`].
pub struct UseItemCtx<'a> {
    pub now_ms: i64,
    pub current_day: i64,
    pub last_wood_chest_day: &'a mut i64,
    pub last_pea_ms: &'a mut i64,
    pub gender: Gender,
    pub stats: &'a mut CoreStats,
    pub buffs: &'a mut TimedBuffState,
    pub fusion_mode: &'a mut i8,
    pub has_pet: bool,
    pub pet_type: &'a mut u8,
    pub pet_power: i64,
    pub pet_unlocked_skills: u8,
    pub zone_satellites: u8,
    pub hungry_soi_in_zone: bool,
    pub mabu_egg: &'a mut Option<MabuEggState>,
}

/// Side-effect returned to the zone/session layer after [`UseItemService::use_bag_item`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseItemEffect {
    Applied,
    SpawnedPet2(i16, i16, i16),
    OpenCapsuleUi(bool),
    SummonDragon(bool),
    DropSatellite(TemplateId),
    PromptSkhChoice,
    TeleportMap(i32),
    PoisonedDeath,
    BonusTnsm(i64),
    RerolledNormalPet,
    UpgradedPetSkill(u8),
    RerolledPetSkill(u8),
    LearnedSkill(TemplateId),
    UpdatedRadarCard(TemplateId),
}

/// Errors produced by [`UseItemService`], [`MagicTreeState`], [`LuckyRoundService`], or [`GiftCodeService`].
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum UseItemError {
    #[error("cannot use item: {0}")]
    PreconditionFailed(&'static str),
    #[error("cooldown active: {0}")]
    Cooldown(&'static str),
    #[error("giftcode rejected: {0}")]
    GiftCodeRejected(&'static str),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Complete `UseItem` handler engine.
pub struct UseItemService;

impl UseItemService {
    /// Dispatches item usage at `bag_slot`, deferring `193` deduction until `MapTransport`
    /// and enforcing live-HP & `Opt 2` (`* 1000`) scaling on Senzu beans.
    pub fn use_bag_item(
        inv: &mut Inventory,
        bag_slot: usize,
        ctx: &mut UseItemCtx<'_>,
        catalog: &dyn ItemCatalog,
        rng: &mut GameRng,
    ) -> Result<UseItemEffect, UseItemError> {
        let snap = inv.bag_item(bag_slot)?.clone();
        let (id, meta): (i16, ItemMeta) = (snap.template_id.0, catalog.meta_of(snap.template_id));
        if ctx.stats.power < meta.power_require {
            return Err(UseItemError::PreconditionFailed("insufficient power"));
        }
        if id == const_item::RUONG_GO as i16 {
            return Self::open_ruong_go(inv, bag_slot, ctx, rng);
        }
        if let Some(&(_, (h, b, l))) = PET2_SPRITES.iter().find(|&&(pid, _)| pid == id) {
            Self::check_equip_prereqs(&snap, meta, ctx)?;
            inv.bag_to_body(bag_slot, 7.min(inv.body.len().saturating_sub(1)), rng)?;
            return Ok(UseItemEffect::SpawnedPet2(h, b, l));
        }
        match meta.item_type {
            6 => return use_item_pea::consume_pea(inv, bag_slot, &snap, ctx),
            7 => {
                inv.sub_from_bag_slot(bag_slot, 1)?;
                return Ok(UseItemEffect::LearnedSkill(snap.template_id));
            }
            11 | 23 | 24 => {
                Self::check_equip_prereqs(&snap, meta, ctx)?;
                inv.bag_to_body(
                    bag_slot,
                    (if meta.item_type == 11 { 8 } else { 9 })
                        .min(inv.body.len().saturating_sub(1)),
                    rng,
                )?;
                return Ok(UseItemEffect::Applied);
            }
            12 => {
                return if (14..=16).contains(&id)
                    && (14..=20).all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1)
                {
                    Ok(UseItemEffect::SummonDragon(true))
                } else {
                    Err(UseItemError::PreconditionFailed("missing balls"))
                };
            }
            22 => {
                return Err(UseItemError::PreconditionFailed(
                    "Tính năng đang được phát triển!",
                ));
            }
            25 if id != 361 => {
                Self::check_equip_prereqs(&snap, meta, ctx)?;
                inv.bag_to_body(bag_slot, 4.min(inv.body.len().saturating_sub(1)), rng)?;
                return Ok(UseItemEffect::Applied);
            }
            33 => {
                inv.sub_from_bag_slot(bag_slot, 1)?;
                return Ok(UseItemEffect::UpdatedRadarCard(snap.template_id));
            }
            _ => {}
        }
        match id {
            401 => {
                if !ctx.has_pet {
                    return Err(UseItemError::PreconditionFailed("Bạn chưa có đệ tử!"));
                }
                if *ctx.fusion_mode != const_player::NON_FUSION {
                    return Err(UseItemError::PreconditionFailed(
                        "Vui lòng tách hợp thể trước khi đổi đệ tử!",
                    ));
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                *ctx.pet_type = 0;
                Ok(UseItemEffect::RerolledNormalPet)
            }
            402..=404 | 759 => {
                if !ctx.has_pet {
                    return Err(UseItemError::PreconditionFailed("Bạn chưa có đệ tử!"));
                }
                let slot_idx: u8 = match id {
                    402 => 0,
                    403 => 1,
                    404 => 2,
                    _ => 3,
                };
                if (ctx.pet_unlocked_skills & (1 << slot_idx)) == 0 {
                    return Err(UseItemError::PreconditionFailed(
                        "Kỹ năng đệ tử chưa mở hoặc đã đạt cấp tối đa!",
                    ));
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                Ok(UseItemEffect::UpgradedPetSkill(slot_idx))
            }
            _ => Self::use_bag_item_by_id(inv, bag_slot, id, &snap, ctx, rng),
        }
    }

    fn check_equip_prereqs(
        snap: &ItemInstance,
        meta: ItemMeta,
        ctx: &UseItemCtx<'_>,
    ) -> Result<(), UseItemError> {
        if !meta.gender.is_none_or(|g| g == ctx.gender) {
            return Err(UseItemError::PreconditionFailed("wrong gender"));
        }
        if get_opt(snap, opt_id::POWER_REQ_B)
            .is_some_and(|b| b > 0 && ctx.stats.power < i64::from(b).saturating_mul(1_000_000_000))
        {
            return Err(UseItemError::PreconditionFailed("insufficient power"));
        }
        Ok(())
    }

    pub(super) fn mk(id: i16, qty: u32, opts: &[(u16, i32)], now_ms: i64) -> ItemInstance {
        ItemInstance {
            template_id: TemplateId(id),
            quantity: qty,
            options: opts
                .iter()
                .map(|&(i, p)| ItemOption { id: i, param: p })
                .collect(),
            create_time_ms: now_ms,
        }
    }
}
