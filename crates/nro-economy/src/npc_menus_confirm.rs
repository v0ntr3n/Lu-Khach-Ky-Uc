//! `NpcMenuRouter::confirm_menu` state machine across all 66 [`NpcKind`] variants.

use super::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx};
use crate::combine::CombineOp;
use crate::inventory::{get_opt, opt_id, set_opt};
use nro_consts::{const_item, const_npc};
use nro_core::{Gender, ItemInstance, ItemOption, MapId, TemplateId};
use smallvec::smallvec;

const fn req_for_stage(stage: u8) -> u32 {
    match stage { 1 => 10, 2 => 3, 3 => 5, 4 => 10, 5 => 5, 6 => 1_000, 7 => 5_000, _ => u32::MAX }
}

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
                (const_npc::INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC, 0) => Ok(NpcEffect::LaunchActivity("OPEN_INTRINSIC")),
                (const_npc::CONFIRM_OPEN_INTRINSIC_VIP, 0) => Ok(NpcEffect::LaunchActivity("OPEN_INTRINSIC_VIP")),
                (const_npc::CONFIRM_LEAVE_CLAN, 0) => Ok(NpcEffect::LaunchActivity("LEAVE_CLAN")),
                (const_npc::CONFIRM_DISSOLUTION_CLAN, 0) => Ok(NpcEffect::LaunchActivity("DISSOLVE_CLAN")),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Bulma if ctx.gender != Gender::TraiDat => Err(NpcMenuError::GuardFailed("Ta chỉ bán đồ cho người Trái Đất")),
            NpcKind::Bulma => Ok(if select == 0 { NpcEffect::OpenShop("BUNMA") } else { NpcEffect::NoOp }),
            NpcKind::Dende if ctx.gender != Gender::Namec => Err(NpcMenuError::GuardFailed("Ta chỉ bán đồ cho người Namếc")),
            NpcKind::Dende => Ok(match select {
                0 => NpcEffect::OpenShop("DENDE"),
                1 => NpcEffect::LaunchActivity("SUMMON_PORUNGA"),
                _ => NpcEffect::NoOp,
            }),
            NpcKind::Appule if ctx.gender != Gender::Xayda => Err(NpcMenuError::GuardFailed("Ta chỉ bán đồ cho chiến binh Xayda")),
            NpcKind::Appule => Ok(if select == 0 { NpcEffect::OpenShop("APPULE") } else { NpcEffect::NoOp }),
            NpcKind::BulmaTuongLai => Ok(if select == 0 { NpcEffect::OpenShop(if map == 102 { "BUNMA_FUTURE" } else { "KARIN" }) } else { NpcEffect::NoOp }),
            NpcKind::Panchy => Ok(if select == 0 { NpcEffect::OpenShop("PANCHY") } else { NpcEffect::NoOp }),
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
                    NpcKind::GokuSSJ => if map == 80 { 131 } else { 80 },
                    NpcKind::Jaco => if map == 24 { 139 } else { 24 },
                    NpcKind::Tapion => if map == 19 { 126 } else { 19 },
                    _ => if map == 50 { 42 } else { 50 },
                };
                Ok(NpcEffect::ChangeMap(MapId(dst), 250, 336))
            }
            NpcKind::GokuSSJ2 => {
                if select != 0 {
                    return Ok(NpcEffect::NoOp);
                }
                let s = ctx.inventory.find_bag_slot(TemplateId(const_item::BI_KIEP as i16)).ok_or(NpcMenuError::GuardFailed("no 590"))?;
                let p = get_opt(ctx.inventory.bag_item(s)?, opt_id::QUANTITY_PARAM).unwrap_or(0);
                if p < 9_999 { return Err(NpcMenuError::GuardFailed("need Opt 31 >= 9999")); }
                let mut staged = ctx.inventory.clone();
                if p == 9_999 { staged.sub_from_bag_slot(s, 1)?; } else if let Some(it) = &mut staged.bag[s] { set_opt(it, opt_id::QUANTITY_PARAM, p - 9_999); }
                let out_id = 592 + match ctx.gender { Gender::TraiDat => 0, Gender::Namec => 1, Gender::Xayda => 2 };
                staged.add_to_bag(ItemInstance { template_id: TemplateId(out_id), quantity: 1, options: smallvec![ItemOption { id: 30, param: 0 }], create_time_ms: 0 }, false)?;
                *ctx.inventory = staged;
                Ok(NpcEffect::Tutorial("Đã nhận võ phục Yardrat!"))
            }
            NpcKind::NgoKhong => Ok(if select == 0 { NpcEffect::Tutorial("Thu thập chữ Giải Khai Phong Ấn tại Ngũ Hành Sơn!") } else { NpcEffect::NoOp }),
            NpcKind::DuongTang => Ok(if select == 0 { NpcEffect::ChangeMap(MapId(if (122..=124).contains(&map) { 0 } else { 122 }), 250, 336) } else { NpcEffect::NoOp }),
            NpcKind::Berry | NpcKind::RuongSuuTam => Ok(NpcEffect::NoOp),
            NpcKind::XeNuocMia => Ok(if matches!(select, 0 | 1) { NpcEffect::LaunchActivity("PRESS_SUGARCANE") } else { NpcEffect::NoOp }),
            NpcKind::Santa => {
                let pgg = ctx.inventory.count_template_in_bag(TemplateId(459)) > 0;
                if select > if pgg { 6 } else { 5 } { return Err(NpcMenuError::InvalidSelection(menu_index, select)); }
                let adj = usize::from(if !pgg && select >= 1 { select + 1 } else { select });
                if adj == 3 { Ok(NpcEffect::OpenInput("GIFT_CODE")) } else { Ok(NpcEffect::OpenShop(["SANTA", "SANTA_GIAM_GIA_1", "SANTA_MO_RONG_HANH_TRANG", "", "SANTA_HAN_SU_DUNG", "SANTA_HEAD", "SANTA_DANH_HIEU"][adj])) }
            }
            NpcKind::BaHatMit if map == 5 => match (menu_index, select) {
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 0) => Ok(NpcEffect::OpenCombine(CombineOp::TaoDaHematite)),
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 1) => Ok(NpcEffect::OpenCombine(CombineOp::CheTaoDuiDuc)),
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 2) => Ok(NpcEffect::OpenCombine(CombineOp::TaoDaMai)),
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 3) => Ok(NpcEffect::OpenCombine(CombineOp::PhanRaDoThanLinh)),
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, 4) => Ok(NpcEffect::OpenCombine(CombineOp::LamPhepNhapDa)),
                (const_npc::MENU_PHAN_RA_DO_THAN_LINH, _) => Ok(NpcEffect::NoOp),
                (_, 9) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_PHAN_RA_DO_THAN_LINH,
                    say: "Chọn công thức chế tác đá & phân rã:",
                    options: smallvec!["Tạo đá Hematite", "Chế tạo Dùi đục", "Tạo Đá mài", "Phân rã Thần Linh", "Làm phép nhập đá", "Đóng"],
                })),
                (_, s) if s <= 8 => Ok(NpcEffect::OpenCombine([
                    CombineOp::EpSaoTrangBi, CombineOp::PhaLeHoaTrangBi, CombineOp::PhanRaTrangBiKichHoat,
                    CombineOp::TaiTaoCapsuleKichHoat, CombineOp::NangCapSaoPhaLe, CombineOp::DanhBongSaoPhaLe,
                    CombineOp::CuongHoaLoSaoPhaLe, CombineOp::ChuyenHoaTrangBiVang, CombineOp::ChuyenHoaTrangBiNgoc,
                ][usize::from(s)])),
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
                (const_npc::MENU_SACH_TUYET_KY, 0) => Ok(NpcEffect::OpenCombine(CombineOp::CheTaoCuonSachCu)),
                (const_npc::MENU_SACH_TUYET_KY, 1) => Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKy)),
                (const_npc::MENU_SACH_TUYET_KY, 2) => Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKySealed)),
                (const_npc::MENU_SACH_TUYET_KY, 3) => Ok(NpcEffect::OpenCombine(CombineOp::GiamDinhSach)),
                (const_npc::MENU_SACH_TUYET_KY, 4) => Ok(NpcEffect::OpenCombine(CombineOp::TaySach)),
                (const_npc::MENU_SACH_TUYET_KY, 5) => Ok(NpcEffect::OpenCombine(CombineOp::NangCapSachTuyetKy)),
                (const_npc::MENU_SACH_TUYET_KY, 6) => Ok(NpcEffect::OpenCombine(CombineOp::HoiPhucSach)),
                (const_npc::MENU_SACH_TUYET_KY, 7) => Ok(NpcEffect::OpenCombine(CombineOp::PhanRaSach)),
                (const_npc::MENU_OPTION_SHOP_BUA | const_npc::MENU_SACH_TUYET_KY, _) => Ok(NpcEffect::NoOp),
                (_, 7) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_SACH_TUYET_KY,
                    say: "Chọn thao tác Sách Tuyệt Kỹ:",
                    options: smallvec![
                        "Chế tạo sách cũ", "Đổi Sách Tuyệt Kỹ", "Đổi STK (Con dấu)",
                        "Giám định sách", "Tẩy sách", "Nâng cấp STK", "Hồi phục sách", "Phân rã sách", "Đóng"
                    ],
                })),
                (_, 8) => Ok(NpcEffect::OpenCombine(CombineOp::LamPhepNhapDa)),
                (_, 9) => Ok(NpcEffect::OpenCombine(CombineOp::PhanRaDoThanLinh)),
                (_, s) if (1..=6).contains(&s) => Ok(NpcEffect::OpenCombine([
                    CombineOp::NangCapVatPham,
                    CombineOp::NangCapBongTai,
                    CombineOp::NangChiSoBongTai,
                    CombineOp::NangCapBongTai3,
                    CombineOp::NangChiSoBongTai3,
                    CombineOp::NhapNgocRong,
                ][usize::from(s - 1)])),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::BoMong if menu_index == const_npc::BASE_MENU && select == 3 => { ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: 100, options: smallvec![], create_time_ms: 0 }, true)?; ctx.inventory.gem = ctx.inventory.gem.saturating_add(10_000); Ok(NpcEffect::ClaimedDailyCheckIn) }
            NpcKind::KyGui if !ctx.activated => Err(NpcMenuError::GuardFailed("not activated")),
            NpcKind::KyGui => Ok(if select == 1 { NpcEffect::OpenConsignShop } else { NpcEffect::Tutorial("Ký gửi bằng Thỏi vàng") }),
            NpcKind::TruongLaoGuru if ctx.gender != Gender::Namec => Err(NpcMenuError::GuardFailed("Về hành tinh của mình đi")),
            NpcKind::VuaVegeta if ctx.gender != Gender::Xayda => Err(NpcMenuError::GuardFailed("Về hành tinh của mình đi")),
            NpcKind::QuyLaoKame | NpcKind::TruongLaoGuru | NpcKind::VuaVegeta => {
                if menu_index == 13 && select == 0 { return Ok(NpcEffect::CancelledSkillStudy); }
                if npc == NpcKind::QuyLaoKame && ((menu_index == const_npc::BASE_MENU && select == 4) || (menu_index == 1 && select == 0)) {
                    let mut staged = ctx.inventory.clone();
                    staged.sub_template_from_bag(TemplateId(const_item::RUA_CON as i16), 1)?;
                    staged.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: 5, options: smallvec![], create_time_ms: 0 }, true)?;
                    *ctx.inventory = staged;
                    return Ok(NpcEffect::Tutorial("Đã nhận quà Rùa con!"));
                }
                if npc == NpcKind::QuyLaoKame && menu_index == const_npc::BASE_MENU && matches!(select, 2 | 3) {
                    let stage = if select == 3 { ctx.kol_vip_stage } else { ctx.kol_stage };
                    if ctx.kol_progress < req_for_stage(stage) { return Err(NpcMenuError::GuardFailed("Chưa đủ tiến độ nhiệm vụ KOL!")); }
                    if select == 3 && ctx.inventory.count_template_in_bag(TemplateId(1825)) == 0 { return Err(NpcMenuError::GuardFailed("need VIP ticket 1825")); }
                    return Ok(NpcEffect::ClaimedKolStage(select == 3, stage));
                }
                if matches!(npc, NpcKind::TruongLaoGuru | NpcKind::VuaVegeta) {
                    return Ok(match select {
                        0 => NpcEffect::Tutorial("Hãy hoàn thành nhiệm vụ chính tuyến để trưởng thành hơn!"),
                        1 => NpcEffect::OpenShop("QUY_LAO"),
                        _ => NpcEffect::NoOp,
                    });
                }
                Ok(if select == 1 { NpcEffect::OpenShop("SHOP_DOI_DIEM") } else { NpcEffect::OpenShop("QUY_LAO") })
            }
            NpcKind::QuocVuong | NpcKind::ToSuKaio => match (menu_index, select) {
                (const_npc::MENU_NANG_GIOI_HAN, 0) => Ok(NpcEffect::ShowMenu(MenuPrompt { npc, menu_index: const_npc::OPEN_POWER_MYSEFT, say: "Mở giới hạn bản thân (50tr vàng)?", options: smallvec!["Đồng ý", "Đóng"] })),
                (const_npc::MENU_NANG_GIOI_HAN, 1) => if !ctx.has_pet { Err(NpcMenuError::GuardFailed("no pet")) } else { Ok(NpcEffect::ShowMenu(MenuPrompt { npc, menu_index: const_npc::OPEN_POWER_PET, say: "Mở giới hạn đệ tử (50tr vàng)?", options: smallvec!["Đồng ý", "Đóng"] })) },
                (const_npc::OPEN_POWER_MYSEFT, 0) => {
                    if ctx.stats.limit_power >= 9 {
                        return Err(NpcMenuError::GuardFailed("Giới hạn sức mạnh đã đạt tối đa!"));
                    }
                    ctx.inventory.sub_gold(50_000_000)?;
                    ctx.stats.limit_power = (ctx.stats.limit_power + 1).min(9);
                    Ok(NpcEffect::UnlockedPowerLimit(false))
                }
                (const_npc::OPEN_POWER_PET, 0) => {
                    if !ctx.has_pet { return Err(NpcMenuError::GuardFailed("no pet")); }
                    ctx.inventory.sub_gold(50_000_000)?;
                    Ok(NpcEffect::UnlockedPowerLimit(true))
                }
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Osin => match map {
                50 => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(48), 300, 312)) } else if select == 1 { Ok(NpcEffect::ChangeMap(MapId(154), 300, 312)) } else { Ok(NpcEffect::NoOp) },
                154 | 155 => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(if map == 154 { 155 } else { 154 }), 200, 312)) } else { Ok(NpcEffect::NoOp) },
                165 => if select == 1 { Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")) } else if select == 0 { Ok(NpcEffect::ChangeMap(MapId(21 + i16::from(ctx.gender)), 250, 336)) } else { Ok(NpcEffect::NoOp) },
                114..=120 if ctx.c_flag != 9 => Err(NpcMenuError::GuardFailed("Khác phe!")),
                114..=120 => match select {
                    0 => Ok(NpcEffect::Tutorial("Hạ quái tích điểm xuống tầng dưới")),
                    1 => if ctx.is_use_gtpt {
                        Err(NpcMenuError::GuardFailed("Giải trừ phép thuật đang có hiệu lực!"))
                    } else {
                        ctx.inventory.sub_gem(1)?;
                        Ok(NpcEffect::ApplyGtpt)
                    },
                    2 => {
                        if ctx.mabu_points < 10 {
                            return Err(NpcMenuError::GuardFailed("insufficient Mabu points"));
                        }
                        let next_map = match map {
                            114 => 115, 115 => 117, 117 => 118, 118 => 119, 119 => 120,
                            _ => return Err(NpcMenuError::GuardFailed("already at bottom floor")),
                        };
                        Ok(NpcEffect::ChangeMap(MapId(next_map), 250, 312))
                    }
                    _ => Ok(NpcEffect::NoOp),
                },
                127 => if select == 0 && !ctx.is_phu_ho_mabu { ctx.inventory.sub_gem(10)?; Ok(NpcEffect::ApplyMabuBlessing) } else { Ok(NpcEffect::ChangeMap(MapId(52), 200, 336)) },
                _ => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(114), 250, 312)) } else if select == 1 { Ok(NpcEffect::ChangeMap(MapId(165), 250, 312)) } else { Ok(NpcEffect::NoOp) },
            },
            NpcKind::Babiday if ctx.c_flag != 10 => Err(NpcMenuError::GuardFailed("Khác phe!")),
            NpcKind::Babiday => if select == 1 && !ctx.is_use_gtpt { ctx.inventory.sub_gem(1)?; Ok(NpcEffect::ApplyGtpt) } else if select == 2 { Ok(NpcEffect::ChangeMap(MapId(52), 200, 336)) } else { Ok(NpcEffect::Tutorial("Chiến đấu cho Babiđây!")) },
            NpcKind::Bill => if map == 48 && menu_index == 100 && select == 0 { Ok(NpcEffect::OpenShop("BILL")) } else if select == 0 { Ok(NpcEffect::ChangeMap(MapId(50), 300, 312)) } else { Ok(NpcEffect::NoOp) },
            NpcKind::Whis => match (menu_index, select) {
                (const_npc::MENU_SACH_TUYET_KY, 0) => Ok(NpcEffect::OpenCombine(CombineOp::CheTaoCuonSachCu)),
                (const_npc::MENU_SACH_TUYET_KY, 1) => Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKy)),
                (const_npc::MENU_SACH_TUYET_KY, 2) => Ok(NpcEffect::OpenCombine(CombineOp::DoiSachTuyetKySealed)),
                (const_npc::MENU_SACH_TUYET_KY, _) => Ok(NpcEffect::NoOp),
                (_, 0) => Ok(NpcEffect::OpenShop("THIEN_SU")),
                (_, 1) => if ctx.wearing_full_destroyer { Ok(NpcEffect::OpenCombine(CombineOp::CheTaoTrangBiThienSu)) } else { Err(NpcMenuError::GuardFailed("need 5 Hủy Diệt")) },
                (_, 2) => Ok(NpcEffect::OpenCombine(CombineOp::HocTuyetKy)),
                (_, 3) => Ok(NpcEffect::LaunchActivity("TRAIN_WHIS")),
                (_, 4) => Ok(NpcEffect::ShowMenu(MenuPrompt {
                    npc,
                    menu_index: const_npc::MENU_SACH_TUYET_KY,
                    say: "Ngươi muốn chế tạo hay đổi Sách Tuyệt Kỹ?",
                    options: smallvec!["Chế tạo sách cũ", "Đổi Sách Tuyệt Kỹ", "Đổi STK (Con dấu)", "Đóng"],
                })),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Rong1Sao | NpcKind::Rong2Sao | NpcKind::Rong3Sao | NpcKind::Rong4Sao | NpcKind::Rong5Sao | NpcKind::Rong6Sao | NpcKind::Rong7Sao => if select <= 4 {
                Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!"))
            } else { Ok(NpcEffect::ChangeMap(MapId(21 + i16::from(ctx.gender)), 250, 336)) },
            NpcKind::ToriBot => {
                if ctx.vip_purchases >= 4 || select > 3 { return Err(NpcMenuError::GuardFailed("VIP cap or insufficient VND")); }
                let cost = u32::from(select + 1) * 50_000;
                if ctx.vnd_balance < cost { return Err(NpcMenuError::GuardFailed("VIP cap or insufficient VND")); }
                let mut staged = ctx.inventory.clone();
                staged.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: [200u32, 500, 700, 1_000][usize::from(select)], options: smallvec![], create_time_ms: 0 }, true)?;
                staged.add_to_bag(ItemInstance { template_id: TemplateId(if select == 3 { 1772 } else { 1254 }), quantity: 1, options: smallvec![], create_time_ms: 0 }, false)?;
                *ctx.inventory = staged;
                ctx.vnd_balance = ctx.vnd_balance.saturating_sub(cost);
                ctx.vip_purchases = ctx.vip_purchases.saturating_add(1);
                Ok(NpcEffect::PurchasedVip(select + 1))
            }
            NpcKind::LinhCanh | NpcKind::DocNhan | NpcKind::MrPoPo => Ok(if select == 0 { NpcEffect::LaunchActivity("NPC_INTERACTION") } else { NpcEffect::NoOp }),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::inventory::Inventory;
    use nro_core::{AccountId, CoreStats, Location, PlayerId, ZoneId};

    #[test]
    fn verifies_cui_dende_calick_close_buttons_and_quy_lao_turtle() {
        let mut inv = Inventory::new(10, 10, 12);
        inv.gold = 100_000_000;
        inv.add_to_bag(ItemInstance { template_id: TemplateId(const_item::RUA_CON as i16), quantity: 1, options: smallvec![], create_time_ms: 0 }, true).unwrap();
        let mut stats = CoreStats::default();
        let mut ctx = NpcPlayerCtx {
            account_id: AccountId(1), player_id: PlayerId(1),
            location: Location { map_id: MapId(19), zone_id: ZoneId(0), x: 100, y: 100 },
            gender: Gender::TraiDat, c_flag: 0, activated: true, has_clan: false, is_clan_leader: false,
            clan_days: 0, has_pet: false, wearing_full_destroyer: false, is_use_gtpt: false,
            is_phu_ho_mabu: false, mabu_points: 0, vnd_balance: 0, vip_purchases: 0,
            kol_stage: 1, kol_vip_stage: 1, kol_progress: 0, stats: &mut stats, inventory: &mut inv,
        };
        assert_eq!(NpcMenuRouter::open_base_menu(NpcKind::Dende, &ctx).unwrap(), NpcEffect::Tutorial("Ta chỉ bán đồ cho người Namếc"));
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Bulma, const_npc::BASE_MENU, 1, &mut ctx).unwrap(), NpcEffect::NoOp);
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::GokuSSJ, const_npc::BASE_MENU, 1, &mut ctx).unwrap(), NpcEffect::NoOp);
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 0, &mut ctx).unwrap(), NpcEffect::ChangeMap(MapId(109), 250, 336));
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 1, &mut ctx).unwrap(), NpcEffect::ChangeMap(MapId(68), 250, 336));
        ctx.location.map_id = MapId(68);
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 0, &mut ctx).unwrap(), NpcEffect::ChangeMap(MapId(19), 1080, 360));
        ctx.location.map_id = MapId(102);
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Calick, const_npc::BASE_MENU, 0, &mut ctx).unwrap(), NpcEffect::ChangeMap(MapId(27), 250, 336));
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Calick, const_npc::BASE_MENU, 1, &mut ctx).unwrap(), NpcEffect::NoOp);
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 4, &mut ctx).unwrap(), NpcEffect::Tutorial("Đã nhận quà Rùa con!"));
        assert_eq!(ctx.inventory.count_template_in_bag(TemplateId(457)), 5);
    }

    #[test]
    fn enforces_kol_progress_confirm_guards_and_toribot_vnd_deduction() {
        let mut inv = Inventory::new(10, 10, 12);
        inv.gold = 100_000_000;
        let mut stats = CoreStats::default();
        let mut ctx = NpcPlayerCtx {
            account_id: AccountId(1), player_id: PlayerId(1),
            location: Location { map_id: MapId(114), zone_id: ZoneId(0), x: 100, y: 100 },
            gender: Gender::TraiDat, c_flag: 0, activated: false, has_clan: false, is_clan_leader: false,
            clan_days: 0, has_pet: false, wearing_full_destroyer: false, is_use_gtpt: false,
            is_phu_ho_mabu: false, mabu_points: 0, vnd_balance: 150_000, vip_purchases: 0,
            kol_stage: 1, kol_vip_stage: 1, kol_progress: 0, stats: &mut stats, inventory: &mut inv,
        };
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 2, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::KyGui, const_npc::BASE_MENU, 1, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::Dende, const_npc::BASE_MENU, 0, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::Appule, const_npc::BASE_MENU, 0, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::TruongLaoGuru, const_npc::BASE_MENU, 1, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::VuaVegeta, const_npc::BASE_MENU, 1, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::GO_UPSTAIRS_MENU, 0, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::Babiday, const_npc::GO_UPSTAIRS_MENU, 0, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::QuocVuong, const_npc::OPEN_POWER_PET, 0, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
        ctx.kol_progress = 10;
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 2, &mut ctx).unwrap(), NpcEffect::ClaimedKolStage(false, 1));
        assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::ToriBot, 0, 1, &mut ctx).unwrap(), NpcEffect::PurchasedVip(2));
        assert_eq!(ctx.vnd_balance, 50_000);
        assert_eq!(ctx.vip_purchases, 1);
    }

    #[test]
    fn osin_mabu_descent_skips_116_and_guards_bottom_floor() {
        let mut inv = Inventory::new(10, 10, 12);
        let mut stats = CoreStats::default();
        let mut ctx = NpcPlayerCtx {
            account_id: AccountId(1), player_id: PlayerId(1),
            location: Location { map_id: MapId(114), zone_id: ZoneId(0), x: 100, y: 100 },
            gender: Gender::TraiDat, c_flag: 9, activated: true, has_clan: false, is_clan_leader: false,
            clan_days: 0, has_pet: false, wearing_full_destroyer: false, is_use_gtpt: false,
            is_phu_ho_mabu: false, mabu_points: 10, vnd_balance: 0, vip_purchases: 0,
            kol_stage: 1, kol_vip_stage: 1, kol_progress: 0, stats: &mut stats, inventory: &mut inv,
        };
        for (cur, expected) in [(114, 115), (115, 117), (117, 118), (118, 119), (119, 120)] {
            ctx.location.map_id = MapId(cur);
            assert_eq!(NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::BASE_MENU, 2, &mut ctx).unwrap(), NpcEffect::ChangeMap(MapId(expected), 250, 312));
        }
        ctx.location.map_id = MapId(120);
        assert!(matches!(NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::BASE_MENU, 2, &mut ctx), Err(NpcMenuError::GuardFailed(_))));
    }
}
