//! `NpcMenuRouter::confirm_menu` state machine across all 66 [`NpcKind`] variants.

use super::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx};
use crate::combine::CombineOp;
use crate::inventory::{get_opt, opt_id, set_opt};
use nro_consts::{const_item, const_npc};
use nro_core::{Gender, ItemInstance, ItemOption, MapId, TemplateId};
use smallvec::smallvec;

#[path = "npc_menus_confirm_ext.rs"]
mod npc_menus_confirm_ext;
#[cfg(test)]
#[path = "npc_menus_tests.rs"]
mod tests;

impl NpcMenuRouter {
    /// Dispatches `confirmMenu(player, select)` across all 66 [`NpcKind`] state machines.
    pub fn confirm_menu(
        npc: NpcKind,
        menu_index: i32,
        select: u8,
        ctx: &mut NpcPlayerCtx<'_>,
    ) -> Result<NpcEffect, NpcMenuError> {
        let map = ctx.location.map_id.0;
        match npc {
            NpcKind::ConMeo => match (menu_index, select) {
                (const_npc::INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC, 0) => {
                    Ok(NpcEffect::LaunchActivity("OPEN_INTRINSIC"))
                }
                (const_npc::CONFIRM_OPEN_INTRINSIC_VIP, 0) => {
                    Ok(NpcEffect::LaunchActivity("OPEN_INTRINSIC_VIP"))
                }
                (const_npc::CONFIRM_LEAVE_CLAN, 0) => Ok(NpcEffect::LaunchActivity("LEAVE_CLAN")),
                (const_npc::CONFIRM_DISSOLUTION_CLAN, 0) => {
                    Ok(NpcEffect::LaunchActivity("DISSOLVE_CLAN"))
                }
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Bulma if ctx.gender != Gender::TraiDat => Err(NpcMenuError::GuardFailed(
                "Ta chỉ bán đồ cho người Trái Đất",
            )),
            NpcKind::Bulma => Ok(if select == 0 {
                NpcEffect::OpenShop("BUNMA")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Dende if ctx.gender != Gender::Namec => {
                Err(NpcMenuError::GuardFailed("Ta chỉ bán đồ cho người Namếc"))
            }
            NpcKind::Dende => Ok(match select {
                0 => NpcEffect::OpenShop("DENDE"),
                1 => NpcEffect::LaunchActivity("SUMMON_PORUNGA"),
                _ => NpcEffect::NoOp,
            }),
            NpcKind::Appule if ctx.gender != Gender::Xayda => Err(NpcMenuError::GuardFailed(
                "Ta chỉ bán đồ cho chiến binh Xayda",
            )),
            NpcKind::Appule => Ok(if select == 0 {
                NpcEffect::OpenShop("APPULE")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::BulmaTuongLai => Ok(if select == 0 {
                NpcEffect::OpenShop(if map == 102 { "BUNMA_FUTURE" } else { "KARIN" })
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Panchy => Ok(if select == 0 {
                NpcEffect::OpenShop("PANCHY")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Cui if map == 19 => match select {
                0 => Ok(NpcEffect::ChangeMap(MapId(109), 250, 336)),
                1 => Ok(NpcEffect::ChangeMap(MapId(68), 250, 336)),
                2 => Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Cui if map == 68 => Ok(if select == 0 {
                NpcEffect::ChangeMap(MapId(19), 1080, 360)
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Cargo | NpcKind::DrDrief | NpcKind::Cui => Ok(match select {
                0 => NpcEffect::ChangeMap(MapId(24), 250, 336),
                1 => NpcEffect::ChangeMap(MapId(25), 250, 336),
                2 => NpcEffect::ChangeMap(MapId(26), 250, 336),
                3 => NpcEffect::ChangeMap(MapId(84), 250, 336),
                _ => NpcEffect::NoOp,
            }),
            NpcKind::Calick => Ok(if select == 0 {
                NpcEffect::ChangeMap(MapId(if map == 102 { 27 } else { 102 }), 250, 336)
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::GokuSSJ | NpcKind::Jaco | NpcKind::Tapion | NpcKind::Kibit => {
                if select != 0 {
                    return Ok(NpcEffect::NoOp);
                }
                let dst = match npc {
                    NpcKind::GokuSSJ => {
                        if map == 80 {
                            131
                        } else {
                            80
                        }
                    }
                    NpcKind::Jaco => {
                        if map == 24 {
                            139
                        } else {
                            24
                        }
                    }
                    NpcKind::Tapion => {
                        if map == 19 {
                            126
                        } else {
                            19
                        }
                    }
                    _ => {
                        if map == 50 {
                            42
                        } else {
                            50
                        }
                    }
                };
                Ok(NpcEffect::ChangeMap(MapId(dst), 250, 336))
            }
            NpcKind::GokuSSJ2 => {
                if select != 0 {
                    return Ok(NpcEffect::NoOp);
                }
                let s = ctx
                    .inventory
                    .find_bag_slot(TemplateId(const_item::BI_KIEP as i16))
                    .ok_or(NpcMenuError::GuardFailed("no 590"))?;
                let p = get_opt(ctx.inventory.bag_item(s)?, opt_id::QUANTITY_PARAM).unwrap_or(0);
                if p < 9_999 {
                    return Err(NpcMenuError::GuardFailed("need Opt 31 >= 9999"));
                }
                let mut staged = ctx.inventory.clone();
                if p == 9_999 {
                    staged.sub_from_bag_slot(s, 1)?;
                } else if let Some(it) = &mut staged.bag[s] {
                    set_opt(it, opt_id::QUANTITY_PARAM, p - 9_999);
                }
                let out_id = 592
                    + match ctx.gender {
                        Gender::TraiDat => 0,
                        Gender::Namec => 1,
                        Gender::Xayda => 2,
                    };
                staged.add_to_bag(
                    ItemInstance {
                        template_id: TemplateId(out_id),
                        quantity: 1,
                        options: smallvec![ItemOption { id: 30, param: 0 }],
                        create_time_ms: 0,
                    },
                    false,
                )?;
                *ctx.inventory = staged;
                Ok(NpcEffect::Tutorial("Đã nhận võ phục Yardrat!"))
            }
            NpcKind::NgoKhong => Ok(if select == 0 {
                NpcEffect::Tutorial("Thu thập chữ Giải Khai Phong Ấn tại Ngũ Hành Sơn!")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::DuongTang => Ok(if select == 0 {
                let home = 21 + i16::from(i8::from(ctx.gender));
                NpcEffect::ChangeMap(
                    MapId(if (122..=124).contains(&map) {
                        home
                    } else {
                        122
                    }),
                    250,
                    336,
                )
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Berry | NpcKind::RuongSuuTam => Ok(NpcEffect::NoOp),
            NpcKind::XeNuocMia => Ok(if matches!(select, 0 | 1) {
                NpcEffect::LaunchActivity("PRESS_SUGARCANE")
            } else {
                NpcEffect::NoOp
            }),
            NpcKind::Santa => {
                let pgg = ctx.inventory.count_template_in_bag(TemplateId(459)) > 0;
                if select > if pgg { 6 } else { 5 } {
                    return Err(NpcMenuError::InvalidSelection(menu_index, select));
                }
                let adj = usize::from(if !pgg && select >= 1 {
                    select + 1
                } else {
                    select
                });
                if adj == 3 {
                    Ok(NpcEffect::OpenInput("GIFT_CODE"))
                } else {
                    Ok(NpcEffect::OpenShop(
                        [
                            "SANTA",
                            "SANTA_GIAM_GIA_1",
                            "SANTA_MO_RONG_HANH_TRANG",
                            "",
                            "SANTA_HAN_SU_DUNG",
                            "SANTA_HEAD",
                            "SANTA_DANH_HIEU",
                        ][adj],
                    ))
                }
            }
            NpcKind::BaHatMit if map == 5 => match (menu_index, select) {
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 0) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::TaoDaHematite))
                }
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 1) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::CheTaoDuiDuc))
                }
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 2) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::TaoDaMai))
                }
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 3) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::PhanRaDoThanLinh))
                }
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 4) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::LamPhepNhapDa))
                }
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, _) => Ok(NpcEffect::NoOp),
                (_, 9) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_PHAN_RA_DO_THAN_LINH,
                    say: "Chọn công thức chế tác đá & phân rã:",
                    options: smallvec![
                        "Tạo đá Hematite",
                        "Chế tạo Dùi đục",
                        "Tạo Đá mài",
                        "Phân rã Thần Linh",
                        "Làm phép nhập đá",
                        "Đóng"
                    ],
                })),
                (_, s) if s <= 8 => Ok(NpcEffect::OpenCombine(
                    [
                        CombineOp::EpSaoTrangBi,
                        CombineOp::PhaLeHoaTrangBi,
                        CombineOp::PhanRaTrangBiKichHoat,
                        CombineOp::TaiTaoCapsuleKichHoat,
                        CombineOp::NangCapSaoPhaLe,
                        CombineOp::DanhBongSaoPhaLe,
                        CombineOp::CuongHoaLoSaoPhaLe,
                        CombineOp::ChuyenHoaTrangBiVang,
                        CombineOp::ChuyenHoaTrangBiNgoc,
                    ][usize::from(s)],
                )),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::BaHatMit => match (menu_index, select) {
                (const_npc::BASE_MENU, 0) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_OPTION_SHOP_BUA,
                    say: "Bùa tăng sức mạnh, hãy chọn loại bùa con muốn mua:",
                    options: smallvec!["Bùa 1 giờ", "Bùa 8 giờ", "Bùa 1 tháng", "Đóng"],
                })),
                (const_npc::MENU_OPTION_SHOP_BUA, 0) => Ok(NpcEffect::OpenShop("BUA_1H")),
                (const_npc::MENU_OPTION_SHOP_BUA, 1) => Ok(NpcEffect::OpenShop("BUA_8H")),
                (const_npc::MENU_OPTION_SHOP_BUA, 2) => Ok(NpcEffect::OpenShop("BUA_1M")),
                (const_npc::MENU_SACH_TUYET_KY, 0) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::CheTaoCuonSachCu))
                }
                (const_npc::MENU_SACH_TUYET_KY, 1) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKy))
                }
                (const_npc::MENU_SACH_TUYET_KY, 2) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKySealed))
                }
                (const_npc::MENU_SACH_TUYET_KY, 3) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::GiamDinhSach))
                }
                (const_npc::MENU_SACH_TUYET_KY, 4) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::TaySach))
                }
                (const_npc::MENU_SACH_TUYET_KY, 5) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::NangCapSachTuyetKy))
                }
                (const_npc::MENU_SACH_TUYET_KY, 6) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::HoiPhucSach))
                }
                (const_npc::MENU_SACH_TUYET_KY, 7) => {
                    Ok(NpcEffect::OpenCombine(CombineOp::PhanRaSach))
                }
                (const_npc::MENU_OPTION_SHOP_BUA | const_npc::MENU_SACH_TUYET_KY, _) => {
                    Ok(NpcEffect::NoOp)
                }
                (_, 7) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_SACH_TUYET_KY,
                    say: "Chọn thao tác Sách Tuyệt Kỹ:",
                    options: smallvec![
                        "Chế tạo sách cũ",
                        "Đổi Sách Tuyệt Kỹ",
                        "Đổi STK (Con dấu)",
                        "Giám định sách",
                        "Tẩy sách",
                        "Nâng cấp STK",
                        "Hồi phục sách",
                        "Phân rã sách",
                        "Đóng"
                    ],
                })),
                (_, 8) => Ok(NpcEffect::OpenCombine(CombineOp::LamPhepNhapDa)),
                (_, 9) => Ok(NpcEffect::OpenCombine(CombineOp::PhanRaDoThanLinh)),
                (_, s) if (1..=6).contains(&s) => Ok(NpcEffect::OpenCombine(
                    [
                        CombineOp::NangCapVatPham,
                        CombineOp::NangCapBongTai,
                        CombineOp::NangChiSoBongTai,
                        CombineOp::NangCapBongTai3,
                        CombineOp::NangChiSoBongTai3,
                        CombineOp::NhapNgocRong,
                    ][usize::from(s - 1)],
                )),
                _ => Ok(NpcEffect::NoOp),
            },
            _ => Self::confirm_menu_ext(npc, menu_index, select, ctx),
        }
    }
}
