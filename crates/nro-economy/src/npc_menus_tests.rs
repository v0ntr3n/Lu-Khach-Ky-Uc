#![allow(clippy::unwrap_used)]

use super::*;
use crate::inventory::Inventory;
use nro_core::{AccountId, CoreStats, Location, PlayerId, ZoneId};

#[test]
fn verifies_cui_dende_calick_duong_tang_home_and_quy_lao_turtle() {
    let mut inv = Inventory::new(10, 10, 12);
    inv.gold = 100_000_000;
    inv.add_to_bag(
        ItemInstance {
            template_id: TemplateId(const_item::RUA_CON as i16),
            quantity: 1,
            options: smallvec![],
            create_time_ms: 0,
        },
        true,
    )
    .unwrap();
    let mut stats = CoreStats::default();
    let mut ctx = NpcPlayerCtx {
        account_id: AccountId(1),
        player_id: PlayerId(1),
        location: Location {
            map_id: MapId(19),
            zone_id: ZoneId(0),
            x: 100,
            y: 100,
        },
        gender: Gender::Namec,
        c_flag: 0,
        activated: true,
        has_clan: false,
        is_clan_leader: false,
        clan_days: 0,
        has_pet: false,
        wearing_full_destroyer: false,
        is_use_gtpt: false,
        is_phu_ho_mabu: false,
        mabu_points: 0,
        vnd_balance: 0,
        vip_purchases: 0,
        kol_stage: 1,
        kol_vip_stage: 1,
        kol_progress: 0,
        stats: &mut stats,
        inventory: &mut inv,
    };
    ctx.location.map_id = MapId(122);
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::DuongTang, const_npc::BASE_MENU, 0, &mut ctx).unwrap(),
        NpcEffect::ChangeMap(MapId(22), 250, 336)
    );
    ctx.gender = Gender::TraiDat;
    ctx.location.map_id = MapId(19);
    assert_eq!(
        NpcMenuRouter::open_base_menu(NpcKind::Dende, &ctx).unwrap(),
        NpcEffect::Tutorial("Ta chỉ bán đồ cho người Namếc")
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Bulma, const_npc::BASE_MENU, 1, &mut ctx).unwrap(),
        NpcEffect::NoOp
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::GokuSSJ, const_npc::BASE_MENU, 1, &mut ctx).unwrap(),
        NpcEffect::NoOp
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 0, &mut ctx).unwrap(),
        NpcEffect::ChangeMap(MapId(109), 250, 336)
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 1, &mut ctx).unwrap(),
        NpcEffect::ChangeMap(MapId(68), 250, 336)
    );
    ctx.location.map_id = MapId(68);
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Cui, const_npc::BASE_MENU, 0, &mut ctx).unwrap(),
        NpcEffect::ChangeMap(MapId(19), 1080, 360)
    );
    ctx.location.map_id = MapId(102);
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Calick, const_npc::BASE_MENU, 0, &mut ctx).unwrap(),
        NpcEffect::ChangeMap(MapId(27), 250, 336)
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Calick, const_npc::BASE_MENU, 1, &mut ctx).unwrap(),
        NpcEffect::NoOp
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 4, &mut ctx)
            .unwrap(),
        NpcEffect::Tutorial("Đã nhận quà Rùa con!")
    );
    assert_eq!(ctx.inventory.count_template_in_bag(TemplateId(457)), 5);
}

#[test]
fn enforces_kol_progress_confirm_guards_and_toribot_vnd_deduction() {
    let mut inv = Inventory::new(10, 10, 12);
    inv.gold = 100_000_000;
    let mut stats = CoreStats::default();
    let mut ctx = NpcPlayerCtx {
        account_id: AccountId(1),
        player_id: PlayerId(1),
        location: Location {
            map_id: MapId(114),
            zone_id: ZoneId(0),
            x: 100,
            y: 100,
        },
        gender: Gender::TraiDat,
        c_flag: 0,
        activated: false,
        has_clan: false,
        is_clan_leader: false,
        clan_days: 0,
        has_pet: false,
        wearing_full_destroyer: false,
        is_use_gtpt: false,
        is_phu_ho_mabu: false,
        mabu_points: 0,
        vnd_balance: 150_000,
        vip_purchases: 0,
        kol_stage: 1,
        kol_vip_stage: 1,
        kol_progress: 0,
        stats: &mut stats,
        inventory: &mut inv,
    };
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 2, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::KyGui, const_npc::BASE_MENU, 1, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::Dende, const_npc::BASE_MENU, 0, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::Appule, const_npc::BASE_MENU, 0, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::TruongLaoGuru, const_npc::BASE_MENU, 1, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::VuaVegeta, const_npc::BASE_MENU, 1, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::GO_UPSTAIRS_MENU, 0, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::Babiday, const_npc::GO_UPSTAIRS_MENU, 0, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::QuocVuong, const_npc::OPEN_POWER_PET, 0, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
    ctx.kol_progress = 10;
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::QuyLaoKame, const_npc::BASE_MENU, 2, &mut ctx)
            .unwrap(),
        NpcEffect::ClaimedKolStage(false, 1)
    );
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::ToriBot, 0, 1, &mut ctx).unwrap(),
        NpcEffect::PurchasedVip(2)
    );
    assert_eq!(ctx.vnd_balance, 50_000);
    assert_eq!(ctx.vip_purchases, 1);
}

#[test]
fn osin_mabu_descent_skips_116_and_guards_bottom_floor() {
    let mut inv = Inventory::new(10, 10, 12);
    let mut stats = CoreStats::default();
    let mut ctx = NpcPlayerCtx {
        account_id: AccountId(1),
        player_id: PlayerId(1),
        location: Location {
            map_id: MapId(114),
            zone_id: ZoneId(0),
            x: 100,
            y: 100,
        },
        gender: Gender::TraiDat,
        c_flag: 9,
        activated: true,
        has_clan: false,
        is_clan_leader: false,
        clan_days: 0,
        has_pet: false,
        wearing_full_destroyer: false,
        is_use_gtpt: false,
        is_phu_ho_mabu: false,
        mabu_points: 10,
        vnd_balance: 0,
        vip_purchases: 0,
        kol_stage: 1,
        kol_vip_stage: 1,
        kol_progress: 0,
        stats: &mut stats,
        inventory: &mut inv,
    };
    for (cur, expected) in [(114, 115), (115, 117), (117, 118), (118, 119), (119, 120)] {
        ctx.location.map_id = MapId(cur);
        assert_eq!(
            NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::BASE_MENU, 2, &mut ctx).unwrap(),
            NpcEffect::ChangeMap(MapId(expected), 250, 312)
        );
    }
    ctx.location.map_id = MapId(120);
    assert!(matches!(
        NpcMenuRouter::confirm_menu(NpcKind::Osin, const_npc::BASE_MENU, 2, &mut ctx),
        Err(NpcMenuError::GuardFailed(_))
    ));
}
