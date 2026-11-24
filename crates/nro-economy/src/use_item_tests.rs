#![allow(clippy::unwrap_used)]

use super::super::super::*;
use crate::{
    BuiltinCatalog, BuyBackLedger, ConsignCurrency, ConsignShop, ShopCurrency, ShopItemListing,
    ShopService, ShopTabKind, ShopType,
};
use nro_consts::const_player;
use nro_core::{CoreStats, Gender, TemplateId};

#[test]
fn capsule_193_defers_deduction_and_pea_restores_hp_mp_stamina() {
    let mut inv = Inventory::new(8, 8, 6);
    inv.set_bag_slot(0, Some(UseItemService::mk(193, 2, &[], 0)))
        .unwrap();
    let mut tree_lv2 = MagicTreeState {
        level: 2,
        curr_peas: 3,
        is_upgrading: false,
        timer_ms: 0,
    };
    let mut tree_lv4 = MagicTreeState {
        level: 4,
        curr_peas: 2,
        is_upgrading: false,
        timer_ms: 0,
    };
    assert_eq!(tree_lv2.harvest(&mut inv, 1_000).unwrap(), 3);
    assert_eq!(tree_lv4.harvest(&mut inv, 1_000).unwrap(), 2);

    let mut stats = CoreStats {
        hp: 0,
        mp: 100,
        hp_max: 10_000,
        mp_max: 10_000,
        stamina: 200,
        max_stamina: 1_000,
        power: 10_000,
        ..CoreStats::default()
    };
    let (mut day, mut pea, mut buffs, mut fus, mut pt, mut egg) = (
        0,
        0,
        TimedBuffState::default(),
        const_player::NON_FUSION,
        0,
        None,
    );
    let mut rng = GameRng::seeded(1);
    let mut ctx = UseItemCtx {
        now_ms: 2_000,
        current_day: 1,
        last_wood_chest_day: &mut day,
        last_pea_ms: &mut pea,
        gender: Gender::TraiDat,
        stats: &mut stats,
        buffs: &mut buffs,
        fusion_mode: &mut fus,
        has_pet: true,
        pet_type: &mut pt,
        pet_power: 50_000_000_000,
        pet_unlocked_skills: 0,
        zone_satellites: 0,
        hungry_soi_in_zone: false,
        mabu_egg: &mut egg,
    };
    assert_eq!(
        UseItemService::use_bag_item(&mut inv, 0, &mut ctx, &BuiltinCatalog, &mut rng),
        Ok(UseItemEffect::OpenCapsuleUi(false))
    );
    assert_eq!(inv.count_template_in_bag(TemplateId(193)), 2);
    assert_eq!(
        UseItemService::use_bag_item(&mut inv, 1, &mut ctx, &BuiltinCatalog, &mut rng),
        Err(UseItemError::PreconditionFailed("dead"))
    );

    ctx.stats.hp = 500;
    assert_eq!(
        UseItemService::use_bag_item(&mut inv, 1, &mut ctx, &BuiltinCatalog, &mut rng),
        Ok(UseItemEffect::Applied)
    );
    assert_eq!((ctx.stats.hp, ctx.stats.stamina), (2_500, 300));
    ctx.now_ms = 4_000;
    assert_eq!(
        UseItemService::use_bag_item(&mut inv, 2, &mut ctx, &BuiltinCatalog, &mut rng),
        Ok(UseItemEffect::Applied)
    );
    assert_eq!((ctx.stats.hp, ctx.stats.stamina), (6_500, 400));

    assert_eq!(
        apply_porata_fusion(454, ctx.fusion_mode, true, 0),
        Err(UseItemError::PreconditionFailed(
            "Đệ tử đã chết, không thể hợp thể!"
        ))
    );
    inv.set_bag_slot(3, Some(UseItemService::mk(892, 1, &[], 0)))
        .unwrap();
    assert_eq!(
        UseItemService::use_bag_item(&mut inv, 3, &mut ctx, &BuiltinCatalog, &mut rng),
        Ok(UseItemEffect::SpawnedPet2(882, 883, 884))
    );
    assert_eq!(inv.body.len(), 8);
    assert!(inv.body[7].is_some());
    inv.set_bag_slot(4, Some(UseItemService::mk(401, 1, &[], 0)))
        .unwrap();
    assert_eq!(
        UseItemService::use_bag_item_with_pet(
            &mut inv,
            4,
            &mut ctx,
            100,
            true,
            &BuiltinCatalog,
            &mut rng
        ),
        Err(UseItemError::PreconditionFailed(
            "Vui lòng tháo hết trang bị của đệ tử trước!"
        ))
    );
}

#[test]
fn magic_tree_costs_and_cancel_upgrade_refund() {
    let mut inv = Inventory::new(8, 8, 12);
    inv.gold = 20_000;
    let mut tree = MagicTreeState {
        level: 1,
        curr_peas: 5,
        is_upgrading: false,
        timer_ms: 0,
    };
    assert_eq!(
        (
            tree.upgrade_gold_cost(),
            tree.fast_harvest_gem_cost(),
            tree.fast_upgrade_gem_cost()
        ),
        (Some(0), 1, 20)
    );
    assert_eq!(
        tree.cancel_upgrade(&mut inv, 1_000),
        Err(UseItemError::PreconditionFailed("tree is not upgrading"))
    );
    tree.start_upgrade(&mut inv, 1_000).unwrap();
    assert!(tree.is_upgrading);
    assert_eq!(tree.cancel_upgrade(&mut inv, 5_000).unwrap(), 0);
    assert!(!tree.is_upgrading);
    inv.gem = 10;
    assert_eq!(
        tree.fast_finish(&mut inv, 6_000),
        Err(UseItemError::PreconditionFailed("peas already full"))
    );
    let mut grow = MagicTreeState {
        level: 1,
        curr_peas: 0,
        is_upgrading: false,
        timer_ms: 0,
    };
    grow.tick(90_000);
    assert_eq!((grow.curr_peas, grow.timer_ms), (1, 60_000));
    grow.tick(120_000);
    assert_eq!((grow.curr_peas, grow.timer_ms), (2, 120_000));
}

#[test]
fn round2_economy_fixes_verified() {
    let mut inv = Inventory::new(8, 8, 12);
    inv.gem = 100;
    inv.lucky_round_box = vec![None; 110];
    let mut rng = GameRng::seeded(5);
    assert_eq!(
        LuckyRoundService::spin(&mut inv, LuckyRoundMode::StandardGem, 3, &mut rng).unwrap(),
        3
    );
    assert_eq!(
        (
            inv.lucky_round_box.len(),
            inv.lucky_round_box.iter().flatten().count()
        ),
        (110, 3)
    );

    let mut ledger = BuyBackLedger::default();
    inv.set_bag_slot(0, Some(UseItemService::mk(457, 1, &[], 1_000)))
        .unwrap();
    assert_eq!(
        ledger
            .sell_from_bag(&mut inv, 0, 1, &BuiltinCatalog)
            .unwrap(),
        37_000_000
    );
    assert!(ledger.items().is_empty());
    assert!(ledger.repurchase(&mut inv, 0, &BuiltinCatalog).is_err());

    let mut seller = Inventory::new(8, 8, 12);
    seller.gold = crate::inventory::LIMIT_GOLD - 1_000;
    seller
        .set_bag_slot(0, Some(UseItemService::mk(457, 2, &[], 1_000)))
        .unwrap();
    seller
        .set_bag_slot(
            1,
            Some(UseItemService::mk(
                14,
                1,
                &[(opt_id::CONSIGN_GOLD, 0)],
                1_000,
            )),
        )
        .unwrap();
    let mut market = ConsignShop::default();
    let lid = market
        .list_item_with_catalog(
            &mut seller,
            nro_core::PlayerId(1),
            1,
            ConsignCurrency::Gold,
            100_000,
            1,
            &BuiltinCatalog,
        )
        .unwrap();
    let mut buyer = Inventory::new(8, 8, 12);
    buyer.gold = 200_000;
    market
        .buy_item_with_catalog(
            &mut buyer,
            nro_core::PlayerId(2),
            20_000_000_000,
            lid,
            &BuiltinCatalog,
        )
        .unwrap();
    assert!(
        market
            .claim_proceeds(&mut seller, nro_core::PlayerId(1), lid)
            .is_err()
    );
    seller.gold = 0;
    assert_eq!(
        market
            .claim_proceeds(&mut seller, nro_core::PlayerId(1), lid)
            .unwrap(),
        90_000
    );

    let listing = ShopItemListing {
        template_id: TemplateId(381),
        quantity: 1,
        cost: 10,
        currency: ShopCurrency::Gold,
        gender: None,
        options: smallvec::smallvec![],
    };
    let (mut ep, mut cc) = (0, 0);
    let bought = ShopService::buy_listing(
        &mut buyer,
        ShopType::Normal,
        ShopTabKind::Standard,
        &listing,
        &mut ep,
        &mut cc,
        false,
        &BuiltinCatalog,
        &mut rng,
    )
    .unwrap()
    .unwrap();
    assert!(bought.create_time_ms > 0);
}
