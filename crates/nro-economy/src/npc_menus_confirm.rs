//! `NpcMenuRouter::confirm_menu` state machine across all 66 [`NpcKind`] variants.

use super::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcMenuRouter, NpcPlayerCtx};
use crate::combine::CombineOp;
use crate::inventory::{get_opt, opt_id, set_opt};
use nro_consts::{const_item, const_npc};
use nro_core::{Gender, ItemInstance, ItemOption, MapId, TemplateId};
use smallvec::smallvec;

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
            NpcKind::Bulma => Ok(if select == 0 { NpcEffect::OpenShop("BUNMA") } else { NpcEffect::NoOp }),
            NpcKind::Dende => Ok(match select {
                0 => NpcEffect::OpenShop("DENDE"),
                1 => NpcEffect::LaunchActivity("SUMMON_PORUNGA"),
                _ => NpcEffect::NoOp,
            }),
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
                if p == 9_999 { ctx.inventory.sub_from_bag_slot(s, 1)?; } else if let Some(it) = &mut ctx.inventory.bag[s] { set_opt(it, opt_id::QUANTITY_PARAM, p - 9_999); }
                let out_id = 592 + match ctx.gender { Gender::TraiDat => 0, Gender::Namec => 1, Gender::Xayda => 2 };
                ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(out_id), quantity: 1, options: smallvec![ItemOption { id: 30, param: 0 }], create_time_ms: 0 }, false)?;
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
            NpcKind::BaHatMit if map == 5 => Ok(NpcEffect::OpenCombine([CombineOp::EpSaoTrangBi, CombineOp::PhaLeHoaTrangBi, CombineOp::PhanRaTrangBiKichHoat, CombineOp::TaiTaoCapsuleKichHoat, CombineOp::NangCapSaoPhaLe, CombineOp::DanhBongSaoPhaLe, CombineOp::CuongHoaLoSaoPhaLe, CombineOp::ChuyenHoaTrangBiVang, CombineOp::ChuyenHoaTrangBiNgoc][usize::from(select.min(8))])),
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
                (const_npc::MENU_OPTION_SHOP_BUA, _) => Ok(NpcEffect::NoOp),
                (_, s) if s >= 1 => Ok(NpcEffect::OpenCombine([
                    CombineOp::NangCapVatPham,
                    CombineOp::NangCapBongTai,
                    CombineOp::NangChiSoBongTai,
                    CombineOp::NangCapBongTai3,
                    CombineOp::NangChiSoBongTai3,
                    CombineOp::NhapNgocRong,
                    CombineOp::GiamDinhSach,
                ][usize::from((s - 1).min(6))])),
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::BoMong if menu_index == const_npc::BASE_MENU && select == 3 => { ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: 100, options: smallvec![], create_time_ms: 0 }, true)?; ctx.inventory.gem = ctx.inventory.gem.saturating_add(10_000); Ok(NpcEffect::ClaimedDailyCheckIn) }
            NpcKind::KyGui => Ok(if select == 1 { NpcEffect::OpenConsignShop } else { NpcEffect::Tutorial("Ký gửi bằng Thỏi vàng") }),
            NpcKind::QuyLaoKame | NpcKind::TruongLaoGuru | NpcKind::VuaVegeta => {
                if menu_index == 13 && select == 0 { return Ok(NpcEffect::CancelledSkillStudy); }
                if npc == NpcKind::QuyLaoKame && ((menu_index == const_npc::BASE_MENU && select == 4) || (menu_index == 1 && select == 0)) {
                    ctx.inventory.sub_template_from_bag(TemplateId(const_item::RUA_CON as i16), 1)?;
                    ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: 5, options: smallvec![], create_time_ms: 0 }, true)?;
                    return Ok(NpcEffect::Tutorial("Đã nhận quà Rùa con!"));
                }
                if npc == NpcKind::QuyLaoKame && matches!(select, 2 | 3) {
                    if select == 3 && ctx.inventory.count_template_in_bag(TemplateId(1825)) == 0 { return Err(NpcMenuError::GuardFailed("need VIP ticket 1825")); }
                    return Ok(NpcEffect::ClaimedKolStage(select == 3, if select == 3 { ctx.kol_vip_stage } else { ctx.kol_stage }));
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
                (const_npc::OPEN_POWER_PET, 0) => { ctx.inventory.sub_gold(50_000_000)?; Ok(NpcEffect::UnlockedPowerLimit(true)) }
                _ => Ok(NpcEffect::NoOp),
            },
            NpcKind::Osin => match map {
                50 => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(48), 300, 312)) } else if select == 1 { Ok(NpcEffect::ChangeMap(MapId(154), 300, 312)) } else { Ok(NpcEffect::NoOp) },
                154 | 155 => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(if map == 154 { 155 } else { 154 }), 200, 312)) } else { Ok(NpcEffect::NoOp) },
                165 => if select == 1 { Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")) } else if select == 0 { Ok(NpcEffect::ChangeMap(MapId(21 + i16::from(ctx.gender)), 250, 336)) } else { Ok(NpcEffect::NoOp) },
                114..=120 => if select == 0 { Ok(NpcEffect::Tutorial("Hạ quái tích điểm xuống tầng dưới")) } else if select == 1 && !ctx.is_use_gtpt { Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")) } else if ctx.mabu_points >= 10 && map < 120 { Ok(NpcEffect::ChangeMap(MapId(map + 1), 250, 312)) } else { Err(NpcMenuError::GuardFailed("insufficient Mabu points")) },
                127 => if select == 0 && !ctx.is_phu_ho_mabu { Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")) } else { Ok(NpcEffect::ChangeMap(MapId(52), 200, 336)) },
                _ => if select == 0 { Ok(NpcEffect::ChangeMap(MapId(114), 250, 312)) } else if select == 1 { Ok(NpcEffect::ChangeMap(MapId(165), 250, 312)) } else { Ok(NpcEffect::NoOp) },
            },
            NpcKind::Babiday => if select == 1 && !ctx.is_use_gtpt { Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!")) } else if select == 2 { Ok(NpcEffect::ChangeMap(MapId(52), 200, 336)) } else { Ok(NpcEffect::Tutorial("Chiến đấu cho Babiđây!")) },
            NpcKind::Bill => if map == 48 && menu_index == 100 && select == 0 { Ok(NpcEffect::OpenShop("BILL")) } else if select == 0 { Ok(NpcEffect::ChangeMap(MapId(50), 300, 312)) } else { Ok(NpcEffect::NoOp) },
            NpcKind::Whis => match select {
                0 => Ok(NpcEffect::OpenShop("THIEN_SU")),
                1 => if ctx.wearing_full_destroyer { Ok(NpcEffect::OpenCombine(CombineOp::CheTaoTrangBiThienSu)) } else { Err(NpcMenuError::GuardFailed("need 5 Hủy Diệt")) },
                2 => Ok(NpcEffect::OpenCombine(CombineOp::HocTuyetKy)), _ => Ok(NpcEffect::LaunchActivity("TRAIN_WHIS")),
            },
            NpcKind::Rong1Sao | NpcKind::Rong2Sao | NpcKind::Rong3Sao | NpcKind::Rong4Sao | NpcKind::Rong5Sao | NpcKind::Rong6Sao | NpcKind::Rong7Sao => if select <= 4 {
                Err(NpcMenuError::GuardFailed("Tính năng đang được phát triển!"))
            } else { Ok(NpcEffect::ChangeMap(MapId(21 + i16::from(ctx.gender)), 250, 336)) },
            NpcKind::ToriBot => {
                if ctx.vip_purchases >= 4 || select > 3 || ctx.vnd_balance < u32::from(select + 1) * 50_000 { return Err(NpcMenuError::GuardFailed("VIP cap or insufficient VND")); }
                ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(457), quantity: [200u32, 500, 700, 1_000][usize::from(select)], options: smallvec![], create_time_ms: 0 }, true)?;
                ctx.inventory.add_to_bag(ItemInstance { template_id: TemplateId(if select == 3 { 1772 } else { 1254 }), quantity: 1, options: smallvec![], create_time_ms: 0 }, false)?;
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
}
