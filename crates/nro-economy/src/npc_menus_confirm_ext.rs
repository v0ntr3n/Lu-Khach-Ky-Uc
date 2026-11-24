//! Extended `NpcMenuRouter::confirm_menu` arms (`BoMong`, `QuyLaoKame`, `Osin`, `Whis`, `ToriBot`, etc.).

use super::super::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx};
use crate::combine::CombineOp;
use nro_consts::{const_item, const_npc};
use nro_core::{Gender, ItemInstance, MapId, TemplateId};
use smallvec::smallvec;

const fn req_for_stage(stage: u8) -> u32 {
    match stage {
        1 | 4 => 10,
        2 => 3,
        3 | 5 => 5,
        6 => 1_000,
        7 => 5_000,
        _ => u32::MAX,
    }
}

impl NpcMenuRouter {
    pub(super) fn confirm_menu_ext(
        npc: NpcKind,
        menu_index: i32,
        select: u8,
        ctx: &mut NpcPlayerCtx<'_>,
    ) -> Result<NpcEffect, NpcMenuError> {
        let map = ctx.location.map_id.0;
        match npc {
            NpcKind::BoMong if menu_index == const_npc::BASE_MENU && select == 3 => {
                ctx.inventory.add_to_bag(
                    ItemInstance {
                        template_id: TemplateId(457),
                        quantity: 100,
                        options: smallvec![],
                        create_time_ms: 0,
                    },
                    true,
                )?;
                ctx.inventory.gem = ctx.inventory.gem.saturating_add(10_000);
                Ok(NpcEffect::ClaimedDailyCheckIn)
            }
            NpcKind::KyGui if !ctx.activated => Err(NpcMenuError::GuardFailed("not activated")),
            NpcKind::KyGui => Ok(if select == 1 {
                NpcEffect::OpenConsignShop
            } else {
                NpcEffect::Tutorial("Ký gửi bằng Thỏi vàng")
            }),
            NpcKind::TruongLaoGuru if ctx.gender != Gender::Namec => {
                Err(NpcMenuError::GuardFailed("Về hành tinh của mình đi"))
            }
            NpcKind::VuaVegeta if ctx.gender != Gender::Xayda => {
                Err(NpcMenuError::GuardFailed("Về hành tinh của mình đi"))
            }
            NpcKind::QuyLaoKame | NpcKind::TruongLaoGuru | NpcKind::VuaVegeta => {
                if menu_index == 13 && select == 0 {
                    return Ok(NpcEffect::CancelledSkillStudy);
                }
                if npc == NpcKind::QuyLaoKame
                    && ((menu_index == const_npc::BASE_MENU && select == 4)
                        || (menu_index == 1 && select == 0))
                {
                    let mut staged = ctx.inventory.clone();
                    staged.sub_template_from_bag(TemplateId(const_item::RUA_CON as i16), 1)?;
                    staged.add_to_bag(
                        ItemInstance {
                            template_id: TemplateId(457),
                            quantity: 5,
                            options: smallvec![],
                            create_time_ms: 0,
                        },
                        true,
                    )?;
                    *ctx.inventory = staged;
                    return Ok(NpcEffect::Tutorial("Đã nhận quà Rùa con!"));
                }
                if npc == NpcKind::QuyLaoKame
                    && menu_index == const_npc::BASE_MENU
                    && matches!(select, 2 | 3)
                {
                    let stage = if select == 3 {
                        ctx.kol_vip_stage
                    } else {
                        ctx.kol_stage
                    };
                    if ctx.kol_progress < req_for_stage(stage) {
                        return Err(NpcMenuError::GuardFailed("Chưa đủ tiến độ nhiệm vụ KOL!"));
                    }
                    if select == 3 && ctx.inventory.count_template_in_bag(TemplateId(1825)) == 0 {
                        return Err(NpcMenuError::GuardFailed("need VIP ticket 1825"));
                    }
                    return Ok(NpcEffect::ClaimedKolStage(select == 3, stage));
                }
                if matches!(npc, NpcKind::TruongLaoGuru | NpcKind::VuaVegeta) {
                    return Ok(match select {
                        0 => NpcEffect::Tutorial(
                            "Hãy hoàn thành nhiệm vụ chính tuyến để trưởng thành hơn!",
                        ),
                        1 => NpcEffect::OpenShop("QUY_LAO"),
                        _ => NpcEffect::NoOp,
                    });
                }
                Ok(if select == 1 {
                    NpcEffect::OpenShop("SHOP_DOI_DIEM")
                } else {
                    NpcEffect::OpenShop("QUY_LAO")
                })
            }
            NpcKind::QuocVuong | NpcKind::ToSuKaio => match (menu_index, select) {
                (const_npc::MENU_NANG_GIOI_HAN, 0) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::OPEN_POWER_MYSEFT,
                    say: "Mở giới hạn bản thân (50tr vàng)?",
                    options: smallvec!["Đồng ý", "Đóng"],
                })),
                (const_npc::MENU_NANG_GIOI_HAN, 1) => {
                    if !ctx.has_pet {
                        Err(NpcMenuError::GuardFailed("no pet"))
                    } else {
                        Ok(NpcEffect::ShowMenu(MenuPrompt {
                            npc,
                            menu_index: const_npc::OPEN_POWER_PET,
                            say: "Mở giới hạn đệ tử (50tr vàng)?",
                            options: smallvec!["Đồng ý", "Đóng"],
                        }))
                    }
                }
                (const_npc::OPEN_POWER_MYSEFT, 0) => {
                    if ctx.stats.limit_power >= 9 {
                        return Err(NpcMenuError::GuardFailed(
                            "Giới hạn sức mạnh đã đạt tối đa!",
                        ));
                    }
                    ctx.inventory.sub_gold(50_000_000)?;
                    ctx.stats.limit_power = (ctx.stats.limit_power + 1).min(9);
                    Ok(NpcEffect::UnlockedPowerLimit(false))
                }
                (const_npc::OPEN_POWER_PET, 0) => {
                    if !ctx.has_pet {
                        return Err(NpcMenuError::GuardFailed("no pet"));
                    }
                    ctx.inventory.sub_gold(50_000_000)?;
                    Ok(NpcEffect::UnlockedPowerLimit(true))
                }
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Osin => match map {
                50 => {
                    if select == 0 {
                        Ok(NpcEffect::ChangeMap(MapId(48), 300, 312))
                    } else if select == 1 {
                        Ok(NpcEffect::ChangeMap(MapId(154), 300, 312))
                    } else {
                        Ok(NpcEffect::NoOp)
                    }
                }
                154 | 155 => {
                    if select == 0 {
                        Ok(NpcEffect::ChangeMap(
                            MapId(if map == 154 { 155 } else { 154 }),
                            200,
                            312,
                        ))
                    } else {
                        Ok(NpcEffect::NoOp)
                    }
                }
                165 => {
                    if select == 1 {
                        Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!"))
                    } else if select == 0 {
                        Ok(NpcEffect::ChangeMap(
                            MapId(21 + i16::from(ctx.gender)),
                            250,
                            336,
                        ))
                    } else {
                        Ok(NpcEffect::NoOp)
                    }
                }
                114..=120 if ctx.c_flag != 9 => Err(NpcMenuError::GuardFailed("Khác phe!")),
                114..=120 => match select {
                    0 => Ok(NpcEffect::Tutorial("Hạ quái tích điểm xuống tầng dưới")),
                    1 => {
                        if ctx.is_use_gtpt {
                            Err(NpcMenuError::GuardFailed(
                                "Giải trừ phép thuật đang có hiệu lực!",
                            ))
                        } else {
                            ctx.inventory.sub_gem(1)?;
                            Ok(NpcEffect::ApplyGtpt)
                        }
                    }
                    2 => {
                        if ctx.mabu_points < 10 {
                            return Err(NpcMenuError::GuardFailed("insufficient Mabu points"));
                        }
                        let next_map = match map {
                            114 => 115,
                            115 => 117,
                            117 => 118,
                            118 => 119,
                            119 => 120,
                            _ => return Err(NpcMenuError::GuardFailed("already at bottom floor")),
                        };
                        Ok(NpcEffect::ChangeMap(MapId(next_map), 250, 312))
                    }
                    _ => Ok(NpcEffect::NoOp),
                },
                127 => {
                    if select == 0 && !ctx.is_phu_ho_mabu {
                        ctx.inventory.sub_gem(10)?;
                        Ok(NpcEffect::ApplyMabuBlessing)
                    } else {
                        Ok(NpcEffect::ChangeMap(MapId(52), 200, 336))
                    }
                }
                _ => {
                    if select == 0 {
                        Ok(NpcEffect::ChangeMap(MapId(114), 250, 312))
                    } else if select == 1 {
                        Ok(NpcEffect::ChangeMap(MapId(165), 250, 312))
                    } else {
                        Ok(NpcEffect::NoOp)
                    }
                }
            },
            NpcKind::Babiday if ctx.c_flag != 10 => Err(NpcMenuError::GuardFailed("Khác phe!")),
            NpcKind::Babiday => {
                if select == 1 && !ctx.is_use_gtpt {
                    ctx.inventory.sub_gem(1)?;
                    Ok(NpcEffect::ApplyGtpt)
                } else if select == 2 {
                    Ok(NpcEffect::ChangeMap(MapId(52), 200, 336))
                } else {
                    Ok(NpcEffect::Tutorial("Chiến đấu cho Babiđây!"))
                }
            }
            NpcKind::Bill => {
                if map == 48 && menu_index == 100 && select == 0 {
                    Ok(NpcEffect::OpenShop("BILL"))
                } else if select == 0 {
                    Ok(NpcEffect::ChangeMap(MapId(50), 300, 312))
                } else {
                    Ok(NpcEffect::NoOp)
                }
            }
            NpcKind::Whis => match (menu_index, select) {
                (const_npc::MENU_SACH_TUYET_KY, 0) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::CheTaoCuonSachCu))
                }
                (const_npc::MENU_SACH_TUYET_KY, 1) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKy))
                }
                (const_npc::MENU_SACH_TUYET_KY, 2) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKySealed))
                }
                (const_npc::MENU_SACH_TUYET_KY, _) => Ok(NpcEffect::NoOp),
                (_, 0) => Ok(NpcEffect::OpenShop("THIEN_SU")),
                (_, 1) => {
                    if ctx.wearing_full_destroyer {
                        Ok(NpcEffect::OpenCombine(CombineOp::CheTaoTrangBiThienSu))
                    } else {
                        Err(NpcMenuError::GuardFailed("need 5 Hủy Diệt"))
                    }
                }
                (_, 2) => Ok(NpcEffect::OpenCombine(CombineOp::HocTuyetKy)),
                (_, 3) => Ok(NpcEffect::LaunchActivity("TRAIN_WHIS")),
                (_, 4) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_SACH_TUYET_KY,
                    say: "Ngươi muốn chế tạo hay đổi Sách Tuyệt Kỹ?",
                    options: smallvec![
                        "Chế tạo sách cũ",
                        "Đổi Sách Tuyệt Kỹ",
                        "Đổi STK (Con dấu)",
                        "Đóng"
                    ],
                })),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Rong1Sao
            | NpcKind::Rong2Sao
            | NpcKind::Rong3Sao
            | NpcKind::Rong4Sao
            | NpcKind::Rong5Sao
            | NpcKind::Rong6Sao
            | NpcKind::Rong7Sao => {
                if select <= 4 {
                    Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!"))
                } else {
                    Ok(NpcEffect::ChangeMap(
                        MapId(21 + i16::from(ctx.gender)),
                        250,
                        336,
                    ))
                }
            }
            NpcKind::ToriBot => {
                if ctx.vip_purchases >= 4 || select > 3 {
                    return Err(NpcMenuError::GuardFailed("VIP cap or insufficient VND"));
                }
                let cost = u32::from(select + 1) * 50_000;
                if ctx.vnd_balance < cost {
                    return Err(NpcMenuError::GuardFailed("VIP cap or insufficient VND"));
                }
                let mut staged = ctx.inventory.clone();
                staged.add_to_bag(
                    ItemInstance {
                        template_id: TemplateId(457),
                        quantity: [200u32, 500, 700, 1_000][usize::from(select)],
                        options: smallvec![],
                        create_time_ms: 0,
                    },
                    true,
                )?;
                staged.add_to_bag(
                    ItemInstance {
                        template_id: TemplateId(if select == 3 { 1772 } else { 1254 }),
                        quantity: 1,
                        options: smallvec![],
                        create_time_ms: 0,
                    },
                    false,
                )?;
                *ctx.inventory = staged;
                ctx.vnd_balance = ctx.vnd_balance.saturating_sub(cost);
                ctx.vip_purchases = ctx.vip_purchases.saturating_add(1);
                Ok(NpcEffect::PurchasedVip(select + 1))
            }
            NpcKind::LinhCanh | NpcKind::DocNhan | NpcKind::MrPoPo => Ok(if select == 0 {
                NpcEffect::LaunchActivity("NPC_INTERACTION")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Potage => Ok(match select {
                0 => NpcEffect::Tutorial("Dùng Bình Commeson để gọi Nhân Bản!"),
                1 => NpcEffect::LaunchActivity("NPC_INTERACTION"),
                _ => NpcEffect::NoOp,
            }),
            NpcKind::GiuMaDauBo if select == 2 => Ok(NpcEffect::OpenShop("SHOP_CLAN")),
            NpcKind::ChiChi if select == 1 => Ok(NpcEffect::OpenShop("SHOP_CHI_CHI")),
            _ => Ok(NpcEffect::LaunchActivity("NPC_INTERACTION")),
        }
    }
}
