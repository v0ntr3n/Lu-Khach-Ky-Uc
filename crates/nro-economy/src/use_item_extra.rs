//! Template-specific item handlers, timed buffs, `Rương Gỗ`, and mystery boxes for [`UseItemService`].

use crate::inventory::{ContainerKind, Inventory, InventoryError, get_opt, opt_id, set_opt};
use nro_consts::const_player;
use nro_core::{GameRng, ItemInstance, TemplateId};

use super::{BuffSlot, MabuEggState, UseItemCtx, UseItemEffect, UseItemError, UseItemService};

/// Toggles Porata fusion (`454`, `921`, `1819`), rejecting fusion entry when `pet_hp <= 0`.
pub fn apply_porata_fusion(
    id: i16,
    fusion_mode: &mut i8,
    has_pet: bool,
    pet_hp: i64,
) -> Result<UseItemEffect, UseItemError> {
    if !has_pet {
        return Err(UseItemError::PreconditionFailed("pet required"));
    }
    if *fusion_mode == const_player::NON_FUSION && pet_hp <= 0 {
        return Err(UseItemError::PreconditionFailed(
            "Đệ tử đã chết, không thể hợp thể!",
        ));
    }
    let m = match id {
        454 => const_player::HOP_THE_PORATA,
        921 => const_player::HOP_THE_PORATA2,
        _ => const_player::HOP_THE_PORATA3,
    };
    *fusion_mode = if *fusion_mode == const_player::NON_FUSION {
        m
    } else {
        const_player::NON_FUSION
    };
    Ok(UseItemEffect::Applied)
}

impl UseItemService {
    pub(in crate::use_item) fn use_bag_item_by_id(
        inv: &mut Inventory,
        bag_slot: usize,
        id: i16,
        snap: &ItemInstance,
        ctx: &mut UseItemCtx<'_>,
        pet_hp: i64,
        rng: &mut GameRng,
    ) -> Result<UseItemEffect, UseItemError> {
        match id {
            193 => Ok(UseItemEffect::OpenCapsuleUi(false)),
            194 => Ok(UseItemEffect::OpenCapsuleUi(true)),
            211 | 212 => {
                if ctx.stats.stamina > ctx.stats.max_stamina / 2 {
                    Err(UseItemError::PreconditionFailed("stamina > 50%"))
                } else {
                    inv.sub_from_bag_slot(bag_slot, 1)?;
                    ctx.stats.stamina = ctx
                        .stats
                        .stamina
                        .saturating_add(if id == 211 {
                            ctx.stats.max_stamina
                        } else {
                            ctx.stats.max_stamina / 5
                        })
                        .min(ctx.stats.max_stamina);
                    Ok(UseItemEffect::Applied)
                }
            }
            342..=345 | 361 | 1758..=1760 => Err(UseItemError::PreconditionFailed(
                "Tính năng đang được phát triển!",
            )),
            380 => {
                if inv.empty_bag_slots() == 0 {
                    return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                if rng.next_range(0, 8) <= 3 {
                    inv.add_gold_clamped(rng.next_range(5_000, 20_000) as u64);
                } else {
                    inv.add_to_bag(
                        Self::mk(rng.next_range(381, 385) as i16, 1, &[(73, 0)], ctx.now_ms),
                        true,
                    )?;
                }
                Ok(UseItemEffect::Applied)
            }
            379
            | 381..=385
            | 521
            | 579
            | 638
            | 663..=667
            | 764
            | 880..=882
            | 1045
            | 1150..=1154
            | 1233
            | 1532
            | 1614..=1616
            | 1628
            | 1635 => Self::apply_buff(inv, bag_slot, id, snap, ctx),
            454 | 921 | 1819 => apply_porata_fusion(id, ctx.fusion_mode, ctx.has_pet, pet_hp),
            460 => {
                if !ctx.hungry_soi_in_zone {
                    return Err(UseItemError::PreconditionFailed("no SoiHecQuyn"));
                }
                if inv.empty_bag_slots() == 0 {
                    return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                if rng.next_range(0, 3) < 3 {
                    let r = rng.next_range(0, 6);
                    inv.add_to_bag(
                        Self::mk(
                            (441 + r) as i16,
                            1,
                            &[(95 + r as u16, if r == 3 || r == 4 { 3 } else { 5 })],
                            ctx.now_ms,
                        ),
                        true,
                    )?;
                } else {
                    inv.add_to_bag(Self::mk(459, 1, &[(112, 80), (93, 90)], ctx.now_ms), true)?;
                }
                Ok(UseItemEffect::Applied)
            }
            568 => {
                if ctx.mabu_egg.is_some() {
                    Err(UseItemError::PreconditionFailed("egg exists"))
                } else {
                    inv.sub_from_bag_slot(bag_slot, 1)?;
                    *ctx.mabu_egg = Some(MabuEggState::plant(ctx.now_ms));
                    Ok(UseItemEffect::Applied)
                }
            }
            727 | 728 => {
                inv.sub_from_bag_slot(bag_slot, 1)?;
                if rng.is_true(90, 100) {
                    ctx.stats.hp = 0;
                    Ok(UseItemEffect::PoisonedDeath)
                } else {
                    Ok(UseItemEffect::BonusTnsm(if id == 727 {
                        10_000_000
                    } else {
                        50_000_000
                    }))
                }
            }
            992 => Ok(UseItemEffect::TeleportMap(102)),
            1560 => {
                if inv.count_template_in_bag(TemplateId(1561)) == 0 {
                    return Err(UseItemError::PreconditionFailed("need key 1561"));
                }
                if inv.empty_bag_slots() == 0 {
                    return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                inv.sub_template_from_bag(TemplateId(1561), 1)?;
                let r = rng.next_range(0, 100);
                let ball = if r < 85 {
                    rng.next_range(17, 20) as i16
                } else if r < 95 {
                    16
                } else {
                    rng.next_range(14, 15) as i16
                };
                inv.add_to_bag(Self::mk(ball, 1, &[], ctx.now_ms), true)?;
                Ok(UseItemEffect::Applied)
            }
            1575 | 1576 => {
                inv.sub_from_bag_slot(bag_slot, 1)?;
                inv.add_gold_clamped(if id == 1575 {
                    rng.next_range(5_000, 20_000) as u64
                } else {
                    rng.next_range(500_000, 2_000_000) as u64
                });
                Ok(UseItemEffect::Applied)
            }
            1655 => Ok(UseItemEffect::PromptSkhChoice),
            1775 => {
                if inv.empty_bag_slots() == 0 {
                    return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                let pool = [
                    555i16, 556, 562, 563, 561, 557, 558, 564, 565, 561, 559, 560, 566, 567, 561,
                ];
                inv.add_to_bag(
                    Self::mk(
                        pool[rng.next_range(0, 14) as usize],
                        1,
                        &[(30, 1)],
                        ctx.now_ms,
                    ),
                    false,
                )?;
                Ok(UseItemEffect::Applied)
            }
            1787 => Ok(UseItemEffect::TeleportMap(164)),
            1795 => {
                if *ctx.fusion_mode != const_player::NON_FUSION {
                    return Err(UseItemError::PreconditionFailed(
                        "Vui lòng tách hợp thể trước!",
                    ));
                }
                if !ctx.has_pet
                    || *ctx.pet_type != 1
                    || ctx.pet_power < 40_000_000_000
                    || get_opt(snap, opt_id::KILIS).unwrap_or(0) < 3_000
                {
                    return Err(UseItemError::PreconditionFailed(
                        "need Mabu >= 40B & 3000 Kilis",
                    ));
                }
                inv.sub_from_bag_slot(bag_slot, 1)?;
                *ctx.pet_type = rng.next_range(2, 4) as u8;
                Ok(UseItemEffect::Applied)
            }
            1798..=1802 => {
                if inv.empty_bag_slots() == 0 || snap.quantity < 99 {
                    return Err(UseItemError::PreconditionFailed("need 99 God food"));
                }
                inv.sub_from_bag_slot(bag_slot, 99)?;
                inv.add_to_bag(Self::mk(1805, 1, &[], ctx.now_ms), true)?;
                Ok(UseItemEffect::Applied)
            }
            648
            | 736
            | 962
            | 963
            | 1170
            | 1171
            | 1569
            | 1591
            | 1592
            | 1594
            | 1608
            | 1609
            | 1757
            | 1776
            | 1777
            | 1821..=1823
            | 1840 => Self::open_box(inv, bag_slot, id, ctx.now_ms, rng),
            _ => Err(UseItemError::PreconditionFailed("unsupported item")),
        }
    }

    fn apply_buff(
        inv: &mut Inventory,
        bag_slot: usize,
        id: i16,
        snap: &ItemInstance,
        ctx: &mut UseItemCtx<'_>,
    ) -> Result<UseItemEffect, UseItemError> {
        if (880..=882).contains(&id)
            && ctx
                .buffs
                .has_active(ctx.now_ms, |s| matches!(s, BuffSlot::MealTier2(_)))
        {
            return Err(UseItemError::PreconditionFailed("MealTier2 active"));
        }
        if id == 521 {
            inv.sub_from_bag_slot(bag_slot, 1)?;
            ctx.buffs.activate(
                BuffSlot::AutoTrain,
                ctx.now_ms,
                i64::from(get_opt(snap, 1).unwrap_or(60).max(1)) * 60_000,
            );
            return Ok(UseItemEffect::Applied);
        }
        let (slot, dur) = match id {
            379 => (BuffSlot::MayDoCapsule, 1_800_000),
            1532 => (BuffSlot::MayDoKhoBau, 1_800_000),
            381 | 1150 => (BuffSlot::CuongNo(id == 1150), 600_000),
            382 | 1152 => (BuffSlot::BoHuyet(id == 1152), 600_000),
            383 | 1151 => (BuffSlot::BoKhi(id == 1151), 600_000),
            384 | 1153 => (BuffSlot::GiapXen(id == 1153), 600_000),
            385 | 1154 => (BuffSlot::AnDanh(id == 1154), 600_000),
            579 | 1045 => (BuffSlot::DuoiKhi, 600_000),
            638 => (BuffSlot::Commeson, 600_000),
            663..=667 => (BuffSlot::MealTier1(id), 600_000),
            880..=882 => (BuffSlot::MealTier2(id), 600_000),
            764 => (BuffSlot::KhauTrang, 600_000),
            1233 => (BuffSlot::NoiComDien, 600_000),
            1614..=1616 => (BuffSlot::NuocMia((id - 1613) as u8), 1_800_000),
            1628 => (BuffSlot::X2PetExp, 1_800_000),
            _ => (BuffSlot::CoBonLa, 1_800_000),
        };
        inv.sub_from_bag_slot(bag_slot, 1)?;
        ctx.buffs.activate(slot, ctx.now_ms, dur);
        Ok(UseItemEffect::Applied)
    }

    pub(in crate::use_item) fn open_ruong_go(
        inv: &mut Inventory,
        bag_slot: usize,
        ctx: &mut UseItemCtx<'_>,
        rng: &mut GameRng,
    ) -> Result<UseItemEffect, UseItemError> {
        if *ctx.last_wood_chest_day >= ctx.current_day {
            return Err(UseItemError::Cooldown("opened today"));
        }
        let lv = get_opt(inv.bag_item(bag_slot)?, opt_id::LEVEL)
            .unwrap_or(0)
            .clamp(0, 12);
        if inv.empty_bag_slots() < 4 {
            return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
        }
        inv.sub_from_bag_slot(bag_slot, 1)?;
        *ctx.last_wood_chest_day = ctx.current_day;
        if lv == 0 {
            inv.add_gold_clamped(1_000);
            return Ok(UseItemEffect::Applied);
        }
        inv.add_gold_clamped(((100 * lv * (100 + rng.next_range(-15, 15)) / 100) as u64) * 1_000);
        if lv >= 9 {
            inv.gem = inv.gem.saturating_add((100 + (lv - 9) * 20) as u32);
        }
        inv.add_to_bag(
            Self::mk(
                (441 + rng.next_range(0, 6)) as i16,
                rng.next_range(1, 3) as u32,
                &[],
                ctx.now_ms,
            ),
            true,
        )?;
        inv.add_to_bag(
            Self::mk(
                (220 + rng.next_range(0, 4)) as i16,
                rng.next_range(1, lv * 2) as u32,
                &[],
                ctx.now_ms,
            ),
            true,
        )?;
        Ok(UseItemEffect::Applied)
    }

    fn open_box(
        inv: &mut Inventory,
        bag_slot: usize,
        id: i16,
        now_ms: i64,
        rng: &mut GameRng,
    ) -> Result<UseItemEffect, UseItemError> {
        if inv.empty_bag_slots() == 0 {
            return Err(InventoryError::ContainerFull(ContainerKind::Bag).into());
        }
        inv.sub_from_bag_slot(bag_slot, 1)?;
        let (pool, st, hid, bps, days): (&[i16], i32, i32, u32, i32) = match id {
            1608 => (&[1807, 1599, 1600, 1601, 1602], 23, 4, 5_000, 15),
            1609 => (&[1804], 25, 0, 50, 15),
            1569 => (
                &[618, 619, 620, 621, 622, 623, 624, 625, 626],
                13,
                2,
                50,
                15,
            ),
            1822 => (&[1579, 1580, 1581, 1582, 1583, 1584, 1585], 14, 0, 50, 15),
            1823 => (&[1586, 1809, 1810, 1811, 1812, 1813, 1814], 16, 3, 50, 15),
            1821 => (&[1765, 1766, 1767, 1768, 1769, 1770, 1771], 15, 0, 50, 15),
            1757 => (&[1741, 1742, 1743, 1744, 1745, 1746], 27, 4, 50, 3),
            1840 => (&[1587, 1588, 1589, 1590, 1593, 1595], 27, 4, 500, 3),
            1592 => (&[1587, 1588, 1589, 1590, 1593, 1595], 25, 4, 50, 3),
            1591 | 1594 => (&[1587, 1588, 1589, 1590, 1593, 1595], 20, 2, 50, 3),
            1777 => (&[1761, 1731, 1732], 25, 0, 50, 3),
            1776 => (&[1772, 1773], 10, 0, 50, 3),
            _ => (&[1087, 1088, 1089, 1090, 1091], 18, 0, 200, 5),
        };
        let mut rw = Self::mk(
            pool[rng.next_range(0, (pool.len() - 1) as i32) as usize],
            1,
            &[(50, st), (77, st), (103, st)],
            now_ms,
        );
        if hid > 0 {
            set_opt(&mut rw, opt_id::HIDDEN_LINES, hid);
        }
        if !rng.is_true(bps, 10_000) {
            set_opt(&mut rw, opt_id::EXPIRY_DAYS, days);
        }
        inv.add_to_bag(rw, false)?;
        Ok(UseItemEffect::Applied)
    }
}
