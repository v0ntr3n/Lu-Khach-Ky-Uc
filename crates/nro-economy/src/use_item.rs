//! Complete `UseItem` dispatcher (`UseItem.java`), buffs, `Pet2`, chests, `MagicTree`, eggs, `LuckyRound`, and `GiftCodeService`.

use crate::inventory::{ContainerKind, Inventory, InventoryError, get_opt, opt_id, set_opt};
use crate::ItemCatalog;
use nro_consts::{const_item, const_player};
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, ItemOption, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter};
use thiserror::Error;

#[path = "use_item_pea.rs"]
mod use_item_pea;

pub use use_item_pea::{
    DuaHauEggState, GiftCodeDef, GiftCodeService, LuckyRoundMode, LuckyRoundService, MabuEggState,
    MagicTreeState,
};

/// All 68 companion `Pet2` item IDs mapped to `(head, body, leg)` sprite triplets (`UseItem.java:304-648`).
pub const PET2_SPRITES: &[(i16, (i16, i16, i16))] = &[
    (892, (882, 883, 884)), (893, (885, 886, 887)), (908, (891, 892, 893)), (909, (894, 895, 896)), (910, (897, 898, 899)), (916, (925, 926, 927)), (917, (928, 929, 930)), (918, (931, 932, 933)), (919, (934, 935, 936)), (936, (718, 719, 720)), (942, (966, 967, 968)), (943, (969, 970, 971)),
    (944, (972, 973, 974)), (967, (1050, 1051, 1052)), (1008, (1074, 1075, 1076)), (1039, (1089, 1090, 1091)), (1040, (1092, 1093, 1094)), (1046, (-1, -1, -1)), (1107, (1155, 1156, 1157)), (1114, (1158, 1159, 1160)), (1188, (1183, 1184, 1185)), (1202, (1201, 1202, 1203)), (1203, (1201, 1202, 1203)), (1207, (1077, 1078, 1079)),
    (1224, (1227, 1228, 1229)), (1225, (1233, 1234, 1235)), (1226, (1230, 1231, 1232)), (1243, (1245, 1246, 1247)), (1244, (1248, 1249, 1250)), (1256, (1267, 1268, 1269)), (1318, (1299, 1300, 1301)), (1347, (1302, 1303, 1304)), (1414, (1341, 1342, 1343)), (1435, (1347, 1348, 1349)), (1452, (1365, 1366, 1367)), (1458, (1368, 1369, 1370)),
    (1482, (1398, 1399, 1400)), (1497, (1401, 1402, 1403)), (1550, (1428, 1429, 1430)), (1551, (1425, 1426, 1427)), (1564, (1437, 1438, 1439)), (1568, (1443, 1444, 1445)), (1573, (1446, 1447, 1448)), (1596, (1473, 1474, 1475)), (1597, (1473, 1474, 1475)), (1611, (1488, 1494, 1495)), (1620, (1496, 1497, 1498)), (1621, (1496, 1497, 1498)),
    (1622, (1488, 1489, 1490)), (1629, (1505, 1506, 1507)), (1630, (1508, 1509, 1510)), (1631, (1513, 1516, 1517)), (1633, (1523, 1524, 1525)), (1654, (1526, 1529, 1530)), (1668, (1550, 1551, 1552)), (1682, (1558, 1559, 1560)), (1683, (1561, 1562, 1563)), (1686, (1572, 1573, 1574)), (1727, (1616, 1617, 1618)), (1729, (1621, 1622, 1623)),
    (1750, (1464, 1465, 1466)), (1765, (1662, 1663, 1664)), (1766, (1665, 1666, 1667)), (1767, (1668, 1669, 1670)), (1768, (1671, 1672, 1673)), (1769, (1674, 1675, 1676)), (1770, (1677, 1678, 1679)), (1771, (1680, 1681, 1682)), (1789, (1724, 1725, 1726)),
];

/// Timed buff channels (`ItemTime.java` + fixed `1635`, `1615`, `1616` expiration timestamps).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, Serialize, Deserialize)]
pub enum BuffSlot {
    /// `379` Máy dò Capsule (`30m`).
    MayDoCapsule,
    /// `1532` Rađa kho báu (`30m`).
    MayDoKhoBau,
    /// `381` (`false`) / `1150` (`true`) Cuồng nộ 1/2 (`10m`).
    CuongNo(bool),
    /// `382` (`false`) / `1152` (`true`) Bổ huyết 1/2 (`10m`).
    BoHuyet(bool),
    /// `383` (`false`) / `1151` (`true`) Bổ khí 1/2 (`10m`).
    BoKhi(bool),
    /// `384` (`false`) / `1153` (`true`) Giáp Xên 1/2 (`10m`).
    GiapXen(bool),
    /// `385` (`false`) / `1154` (`true`) Ẩn danh 1/2 (`10m`).
    AnDanh(bool),
    /// `579` / `1045` Đuôi khỉ (`10m`).
    DuoiKhi,
    /// `638` Bình chứa Commeson (`10m`).
    Commeson,
    /// `663..=667` Whis food (`10m`).
    MealTier1(i16),
    /// `880..=882` Seafood meal (`10m`, non-overwriting).
    MealTier2(i16),
    /// `764` Khẩu trang (`10m`).
    KhauTrang,
    /// `1233` Nồi cơm điện (`10m`).
    NoiComDien,
    /// `1614..=1616` Nước mía 1/2/3 (`30m` — fixes `1615`/`1616` instant-expiry bug!).
    NuocMia(u8),
    /// `1628` Bùa x2 TNSM đệ tử (`30m`).
    X2PetExp,
    /// `1635` Cỏ bốn lá (`30m` — fixes `lastTime = 0` instant-expiry bug!).
    CoBonLa,
    /// `521` Tự động luyện tập (`Opt 1` minutes).
    AutoTrain,
}

/// Active player buff state tracking expiration epochs (`ms`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedBuffState {
    /// Active `(slot, expires_at_ms)` pairs.
    pub active: SmallVec<[(BuffSlot, i64); 8]>,
}

impl TimedBuffState {
    /// Returns `true` if `pred` matches an unexpired buff at `now_ms`.
    #[must_use]
    pub fn has_active(&self, now_ms: i64, pred: impl Fn(BuffSlot) -> bool) -> bool {
        self.active.iter().any(|&(s, exp)| exp > now_ms && pred(s))
    }

    /// Inserts or refreshes `slot` until `now_ms + duration_ms`.
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
    /// Timestamp (`ms`).
    pub now_ms: i64,
    /// Current calendar day index (`now_ms / 86_400_000`) for daily limits (`Rương Gỗ`).
    pub current_day: i64,
    /// Last day `570` Rương Gỗ was opened.
    pub last_wood_chest_day: &'a mut i64,
    /// Last pea eat timestamp (`1,000ms` cooldown).
    pub last_pea_ms: &'a mut i64,
    /// Player planet.
    pub gender: Gender,
    /// Player core stats.
    pub stats: &'a mut CoreStats,
    /// Active timed buffs.
    pub buffs: &'a mut TimedBuffState,
    /// Current fusion mode (`const_player::NON_FUSION`, `HOP_THE_PORATA`, etc.).
    pub fusion_mode: &'a mut i8,
    /// True if player owns a combat pet.
    pub has_pet: bool,
    /// Pet type (`0 = normal, 1 = Mabu, 2 = Uub, 3 = KidBeer, 4 = Jiren`).
    pub pet_type: &'a mut u8,
    /// Pet power (`pet.nPoint.power`).
    pub pet_power: i64,
    /// Bitmask of unlocked pet skill slots (`bit 0..=3`).
    pub pet_unlocked_skills: u8,
    /// Number of satellites (`type 22`) currently placed in the zone.
    pub zone_satellites: u8,
    /// True if `SoiHecQuyn` boss is alive and hungry in the zone (`460` Cục xương).
    pub hungry_soi_in_zone: bool,
    /// Player Mabu Egg state (`None` if unplanted).
    pub mabu_egg: &'a mut Option<MabuEggState>,
}

/// Side-effect returned to the zone/session layer after [`UseItemService::use_bag_item`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseItemEffect {
    /// Item consumed/applied with no extra UI modal needed.
    Applied,
    /// Equipped companion `Pet2` with `(head, body, leg)` sprites.
    SpawnedPet2(i16, i16, i16),
    /// Opens capsule teleport UI (`true` when using reusable `194`, `false` for deferred `193`).
    OpenCapsuleUi(bool),
    /// Summons Shenron (`true` for Earth `14..=20`, `false` for Namek `353`).
    SummonDragon(bool),
    /// Drops zone satellite (`342..=345`).
    DropSatellite(TemplateId),
    /// Opens SKH equipment slot picker (`1655`).
    PromptSkhChoice,
    /// Teleports player to target map ID (`1787` Private Map `164` or `992` Spacetime `102`).
    TeleportMap(i32),
    /// Player died from `727`/`728` Siêu thần thủy poison (`90%`).
    PoisonedDeath,
    /// Granted `+tnsm` from `727`/`728` Siêu thần thủy (`10%`).
    BonusTnsm(i64),
    /// Reroll pet skill slot (`2`, `3`, or `4`).
    RerolledPetSkill(u8),
    /// Learned skill book template.
    LearnedSkill(TemplateId),
    /// Updated radar card collection (`type 33`).
    UpdatedRadarCard(TemplateId),
}

/// Errors produced by [`UseItemService`], [`MagicTreeState`], [`LuckyRoundService`], or [`GiftCodeService`].
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum UseItemError {
    /// Player power below `strRequire` or item precondition failed.
    #[error("cannot use item: {0}")]
    PreconditionFailed(&'static str),
    /// Cooldown or daily limit still active.
    #[error("cooldown active: {0}")]
    Cooldown(&'static str),
    /// Invalid or expired giftcode.
    #[error("giftcode rejected: {0}")]
    GiftCodeRejected(&'static str),
    /// Underlying inventory error.
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
        let (id, meta) = (snap.template_id.0, catalog.meta_of(snap.template_id));
        if ctx.stats.power < meta.power_require {
            return Err(UseItemError::PreconditionFailed("insufficient power"));
        }
        if id == const_item::RUONG_GO as i16 {
            return Self::open_ruong_go(inv, bag_slot, ctx, rng);
        }
        if let Some(&(_, (h, b, l))) = PET2_SPRITES.iter().find(|&&(pid, _)| pid == id) {
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
                inv.bag_to_body(bag_slot, (if meta.item_type == 11 { 8 } else { 9 }).min(inv.body.len().saturating_sub(1)), rng)?;
                return Ok(UseItemEffect::Applied);
            }
            12 => {
                return if (14..=16).contains(&id) && (14..=20).all(|b| inv.count_template_in_bag(TemplateId(b)) >= 1) {
                    Ok(UseItemEffect::SummonDragon(true))
                } else {
                    Err(UseItemError::PreconditionFailed("missing balls"))
                };
            }
            22 => return Err(UseItemError::PreconditionFailed("Tính năng đang được phát triển!")),
            25 if id != 361 => {
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
            193 => Ok(UseItemEffect::OpenCapsuleUi(false)),
            194 => Ok(UseItemEffect::OpenCapsuleUi(true)),
            211 | 212 => if ctx.stats.stamina > ctx.stats.max_stamina / 2 { Err(UseItemError::PreconditionFailed("stamina > 50%")) } else { inv.sub_from_bag_slot(bag_slot, 1)?; ctx.stats.stamina = ctx.stats.stamina.saturating_add(if id == 211 { ctx.stats.max_stamina } else { ctx.stats.max_stamina / 5 }).min(ctx.stats.max_stamina); Ok(UseItemEffect::Applied) },
            342..=345 | 361 | 401..=404 | 759 | 1758..=1760 => Err(UseItemError::PreconditionFailed("Tính năng đang được phát triển!")),
            380 => {
                if inv.empty_bag_slots() == 0 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
                inv.sub_from_bag_slot(bag_slot, 1)?; if rng.next_range(0, 8) <= 3 { inv.add_gold_clamped(rng.next_range(5_000, 20_000) as u64); } else { inv.add_to_bag(Self::mk(rng.next_range(381, 385) as i16, 1, &[(73, 0)]), true)?; }
                Ok(UseItemEffect::Applied)
            }
            379 | 381..=385 | 521 | 579 | 638 | 663..=667 | 764 | 880..=882 | 1045 | 1150..=1154 | 1233 | 1532 | 1614..=1616 | 1628 | 1635 => Self::apply_buff(inv, bag_slot, id, &snap, ctx),
            454 | 921 | 1819 => if !ctx.has_pet { Err(UseItemError::PreconditionFailed("pet required")) } else { let m = match id { 454 => const_player::HOP_THE_PORATA, 921 => const_player::HOP_THE_PORATA2, _ => const_player::HOP_THE_PORATA3 }; *ctx.fusion_mode = if *ctx.fusion_mode == const_player::NON_FUSION { m } else { const_player::NON_FUSION }; Ok(UseItemEffect::Applied) },
            460 => {
                if !ctx.hungry_soi_in_zone { return Err(UseItemError::PreconditionFailed("no SoiHecQuyn")); }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                if rng.next_range(0, 3) < 3 { let r = rng.next_range(0, 6); inv.add_to_bag(Self::mk((441 + r) as i16, 1, &[(95 + r as u16, if r == 3 || r == 4 { 3 } else { 5 })]), true)?; } else { inv.add_to_bag(Self::mk(459, 1, &[(112, 80), (93, 90)]), true)?; }
                Ok(UseItemEffect::Applied)
            }
            568 => if ctx.mabu_egg.is_some() { Err(UseItemError::PreconditionFailed("egg exists")) } else { inv.sub_from_bag_slot(bag_slot, 1)?; *ctx.mabu_egg = Some(MabuEggState::plant(ctx.now_ms)); Ok(UseItemEffect::Applied) },
            727 | 728 => { inv.sub_from_bag_slot(bag_slot, 1)?; if rng.is_true(90, 100) { ctx.stats.hp = 0; Ok(UseItemEffect::PoisonedDeath) } else { Ok(UseItemEffect::BonusTnsm(if id == 727 { 10_000_000 } else { 50_000_000 })) } }
            992 => Ok(UseItemEffect::TeleportMap(102)),
            1560 => {
                if inv.count_template_in_bag(TemplateId(1561)) == 0 { return Err(UseItemError::PreconditionFailed("need key 1561")); }
                inv.sub_from_bag_slot(bag_slot, 1)?; inv.sub_template_from_bag(TemplateId(1561), 1)?; let r = rng.next_range(0, 100);
                inv.add_to_bag(Self::mk(if r < 85 { rng.next_range(17, 20) as i16 } else if r < 95 { 16 } else { rng.next_range(14, 15) as i16 }, 1, &[]), true)?; Ok(UseItemEffect::Applied)
            }
            1575 | 1576 => { inv.sub_from_bag_slot(bag_slot, 1)?; inv.add_gold_clamped(if id == 1575 { rng.next_range(5_000, 20_000) as u64 } else { rng.next_range(500_000, 2_000_000) as u64 }); Ok(UseItemEffect::Applied) }
            1655 => Ok(UseItemEffect::PromptSkhChoice),
            1775 => { if inv.empty_bag_slots() == 0 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); } inv.sub_from_bag_slot(bag_slot, 1)?; inv.add_to_bag(Self::mk([555i16, 556, 562, 563, 561, 557, 558, 564, 565, 561, 559, 560, 566, 567, 561][rng.next_range(0, 14) as usize], 1, &[(30, 1)]), false)?; Ok(UseItemEffect::Applied) }
            1787 => Ok(UseItemEffect::TeleportMap(164)),
            1795 => { if !ctx.has_pet || *ctx.pet_type != 1 || ctx.pet_power < 40_000_000_000 || get_opt(&snap, opt_id::KILIS).unwrap_or(0) < 3_000 { return Err(UseItemError::PreconditionFailed("need Mabu >= 40B & 3000 Kilis")); } inv.sub_from_bag_slot(bag_slot, 1)?; *ctx.pet_type = rng.next_range(2, 4) as u8; Ok(UseItemEffect::Applied) }
            1798..=1802 => { if inv.empty_bag_slots() == 0 || snap.quantity < 99 { return Err(UseItemError::PreconditionFailed("need 99 God food")); } inv.sub_from_bag_slot(bag_slot, 99)?; inv.add_to_bag(Self::mk(1805, 1, &[]), true)?; Ok(UseItemEffect::Applied) }
            648 | 736 | 962 | 963 | 1170 | 1171 | 1569 | 1591 | 1592 | 1594 | 1608 | 1609 | 1757 | 1776 | 1777 | 1821..=1823 | 1840 => Self::open_box(inv, bag_slot, id, rng),
            _ => Err(UseItemError::PreconditionFailed("unsupported item")),
        }
    }

    fn apply_buff(inv: &mut Inventory, bag_slot: usize, id: i16, snap: &ItemInstance, ctx: &mut UseItemCtx<'_>) -> Result<UseItemEffect, UseItemError> {
        if (880..=882).contains(&id) && ctx.buffs.has_active(ctx.now_ms, |s| matches!(s, BuffSlot::MealTier2(_))) { return Err(UseItemError::PreconditionFailed("MealTier2 active")); }
        if id == 521 { ctx.buffs.activate(BuffSlot::AutoTrain, ctx.now_ms, i64::from(get_opt(snap, 1).unwrap_or(60).max(1)) * 60_000); return Ok(UseItemEffect::Applied); }
        let (slot, dur) = match id {
            379 => (BuffSlot::MayDoCapsule, 1_800_000), 1532 => (BuffSlot::MayDoKhoBau, 1_800_000), 381 | 1150 => (BuffSlot::CuongNo(id == 1150), 600_000),
            382 | 1152 => (BuffSlot::BoHuyet(id == 1152), 600_000), 383 | 1151 => (BuffSlot::BoKhi(id == 1151), 600_000), 384 | 1153 => (BuffSlot::GiapXen(id == 1153), 600_000),
            385 | 1154 => (BuffSlot::AnDanh(id == 1154), 600_000), 579 | 1045 => (BuffSlot::DuoiKhi, 600_000), 638 => (BuffSlot::Commeson, 600_000),
            663..=667 => (BuffSlot::MealTier1(id), 600_000), 880..=882 => (BuffSlot::MealTier2(id), 600_000), 764 => (BuffSlot::KhauTrang, 600_000),
            1233 => (BuffSlot::NoiComDien, 600_000), 1614..=1616 => (BuffSlot::NuocMia((id - 1613) as u8), 1_800_000), 1628 => (BuffSlot::X2PetExp, 1_800_000), _ => (BuffSlot::CoBonLa, 1_800_000),
        };
        inv.sub_from_bag_slot(bag_slot, 1)?; ctx.buffs.activate(slot, ctx.now_ms, dur); Ok(UseItemEffect::Applied)
    }

    fn open_ruong_go(inv: &mut Inventory, bag_slot: usize, ctx: &mut UseItemCtx<'_>, rng: &mut GameRng) -> Result<UseItemEffect, UseItemError> {
        if *ctx.last_wood_chest_day >= ctx.current_day { return Err(UseItemError::Cooldown("opened today")); }
        let lv = get_opt(inv.bag_item(bag_slot)?, opt_id::LEVEL).unwrap_or(0).clamp(0, 12);
        if inv.empty_bag_slots() < 4 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
        inv.sub_from_bag_slot(bag_slot, 1)?; *ctx.last_wood_chest_day = ctx.current_day;
        if lv == 0 { inv.add_gold_clamped(1_000); return Ok(UseItemEffect::Applied); }
        inv.add_gold_clamped(((100 * lv * (100 + rng.next_range(-15, 15)) / 100) as u64) * 1_000); if lv >= 9 { inv.gem = inv.gem.saturating_add((100 + (lv - 9) * 20) as u32); }
        inv.add_to_bag(Self::mk((441 + rng.next_range(0, 6)) as i16, rng.next_range(1, 3) as u32, &[]), true)?;
        inv.add_to_bag(Self::mk((220 + rng.next_range(0, 4)) as i16, rng.next_range(1, lv * 2) as u32, &[]), true)?; Ok(UseItemEffect::Applied)
    }

    fn open_box(inv: &mut Inventory, bag_slot: usize, id: i16, rng: &mut GameRng) -> Result<UseItemEffect, UseItemError> {
        if inv.empty_bag_slots() == 0 { return Err(InventoryError::ContainerFull(ContainerKind::Bag).into()); }
        inv.sub_from_bag_slot(bag_slot, 1)?;
        let (pool, st, hid, bps, days): (&[i16], i32, i32, u32, i32) = match id {
            1608 => (&[1807, 1599, 1600, 1601, 1602], 23, 4, 5_000, 15), 1609 => (&[1804], 25, 0, 50, 15), 1569 => (&[618, 619, 620, 621, 622, 623, 624, 625, 626], 13, 2, 50, 15),
            1822 => (&[1579, 1580, 1581, 1582, 1583, 1584, 1585], 14, 0, 50, 15), 1823 => (&[1586, 1809, 1810, 1811, 1812, 1813, 1814], 16, 3, 50, 15),
            1821 => (&[1765, 1766, 1767, 1768, 1769, 1770, 1771], 15, 0, 50, 15), 1757 => (&[1741, 1742, 1743, 1744, 1745, 1746], 27, 4, 50, 3),
            1840 => (&[1587, 1588, 1589, 1590, 1593, 1595], 27, 4, 500, 3), 1592 => (&[1587, 1588, 1589, 1590, 1593, 1595], 25, 4, 50, 3),
            1591 | 1594 => (&[1587, 1588, 1589, 1590, 1593, 1595], 20, 2, 50, 3), 1777 => (&[1761, 1731, 1732], 25, 0, 50, 3), 1776 => (&[1772, 1773], 10, 0, 50, 3),
            _ => (&[1087, 1088, 1089, 1090, 1091], 18, 0, 200, 5),
        };
        let mut rw = Self::mk(pool[rng.next_range(0, (pool.len() - 1) as i32) as usize], 1, &[(50, st), (77, st), (103, st)]);
        if hid > 0 { set_opt(&mut rw, opt_id::HIDDEN_LINES, hid); }
        if !rng.is_true(bps, 10_000) { set_opt(&mut rw, opt_id::EXPIRY_DAYS, days); } inv.add_to_bag(rw, false)?; Ok(UseItemEffect::Applied)
    }

    pub(super) fn mk(id: i16, qty: u32, opts: &[(u16, i32)]) -> ItemInstance {
        ItemInstance { template_id: TemplateId(id), quantity: qty, options: opts.iter().map(|&(i, p)| ItemOption { id: i, param: p }).collect(), create_time_ms: 0 }
    }
}
