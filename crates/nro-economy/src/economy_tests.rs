#![allow(clippy::unwrap_used)]

use super::*;
use nro_consts::{const_item, const_map, const_player};
use nro_core::{
    AccountId, CoreStats, GameRng, ItemInstance, ItemOption, Location, MapId, PlayerId,
};
use proptest::prelude::*;
use smallvec::smallvec;
use strum::EnumCount;

fn make_item(id: i16, qty: u32, opts: &[(u16, i32)]) -> ItemInstance {
    ItemInstance {
        template_id: TemplateId(id),
        quantity: qty,
        options: opts
            .iter()
            .map(|&(oid, p)| ItemOption { id: oid, param: p })
            .collect(),
        create_time_ms: 1_700_000_000_000,
    }
}

#[test]
fn verifies_66_npcs_and_disjoint_combine_slot_aliasing_guard() {
    assert_eq!(NpcKind::COUNT, 66);
    let mut inv = Inventory::new(8, 8, 12);
    inv.set_bag_slot(2, Some(make_item(441, 10, &[]))).unwrap();
    assert_eq!(
        inv.validate_disjoint_bag_slots(&[2, 2]),
        Err(InventoryError::DuplicateSlotIndex(2))
    );
}

#[test]
fn consignment_gold_sale_pays_raw_gold_and_shop_spec_77_deducts_gem() {
    let mut seller_inv = Inventory::new(20, 20, 12);
    seller_inv
        .add_to_bag(make_item(const_item::THOI_VANG as i16, 2, &[]), true)
        .unwrap();
    seller_inv
        .add_to_bag(make_item(14, 5, &[(opt_id::CONSIGN_GOLD, 0)]), true)
        .unwrap();

    let mut market = ConsignShop::default();
    let lid = market
        .list_item(
            &mut seller_inv,
            PlayerId(10),
            1,
            ConsignCurrency::Gold,
            100_000,
            3,
        )
        .unwrap();
    assert_eq!(seller_inv.count_template_in_bag(TemplateId(457)), 1);

    let mut buyer_inv = Inventory::new(20, 20, 12);
    buyer_inv.gold = 500_000;
    buyer_inv.gem = 100;
    market
        .buy_item(&mut buyer_inv, PlayerId(20), 20_000_000_000, lid)
        .unwrap();
    assert_eq!(buyer_inv.gold, 400_000);
    assert_eq!(buyer_inv.count_template_in_bag(TemplateId(14)), 3);

    market
        .claim_proceeds(&mut seller_inv, PlayerId(10), lid)
        .unwrap();
    assert_eq!(seller_inv.gold, 90_000);
    assert_eq!(seller_inv.count_template_in_bag(TemplateId(457)), 1);

    let listing = ShopItemListing {
        template_id: TemplateId(381),
        quantity: 1,
        cost: 25,
        currency: ShopCurrency::ItemSpec(TemplateId(const_item::NGOC as i16)),
        gender: None,
        options: smallvec![],
    };
    let (mut ep, mut cc, mut rng) = (0, 0, GameRng::seeded(9));
    ShopService::buy_listing(
        &mut buyer_inv,
        ShopType::SpecIcon,
        ShopTabKind::Standard,
        &listing,
        &mut ep,
        &mut cc,
        false,
        &BuiltinCatalog,
        &mut rng,
    )
    .unwrap();
    assert_eq!(buyer_inv.gem, 75);
}

#[test]
fn combine_chuyen_hoa_ngoc_deducts_5000_gem_and_phan_ra_sach_deducts_10m_gold() {
    let mut inv = Inventory::new(20, 20, 12);
    inv.gold = 50_000_000;
    inv.gem = 6_000;
    inv.set_bag_slot(
        0,
        Some(make_item(232, 1, &[(opt_id::ATK, 100), (opt_id::LEVEL, 5)])),
    )
    .unwrap();
    inv.set_bag_slot(1, Some(make_item(555, 1, &[(opt_id::ATK, 1_000)])))
        .unwrap();
    let mut rng = GameRng::seeded(42);
    let ctx = CombineContext::default_hardened();
    let out = CombineService::execute(
        &mut inv,
        CombineOp::ChuyenHoaTrangBiNgoc,
        &[0, 1],
        &ctx,
        &BuiltinCatalog,
        &mut rng,
    )
    .unwrap();
    assert!(out.success);
    assert_eq!(inv.gem, 1_000);
    assert_eq!(inv.gold, 50_000_000);

    inv.set_bag_slot(2, Some(make_item(1044, 1, &[(opt_id::DURABILITY, 1_000)])))
        .unwrap();
    CombineService::execute(
        &mut inv,
        CombineOp::PhanRaSach,
        &[2],
        &ctx,
        &BuiltinCatalog,
        &mut rng,
    )
    .unwrap();
    assert_eq!(inv.gold, 40_000_000);
    assert_eq!(inv.count_template_in_bag(TemplateId(1283)), 5);
}

#[test]
fn use_item_co_bon_la_1635_tayaki_1798_osin_bill_and_giftcode_work() {
    let mut inv = Inventory::new(12, 10, 12);
    inv.gem = 20;
    inv.set_bag_slot(0, Some(make_item(1635, 1, &[]))).unwrap();
    inv.set_bag_slot(1, Some(make_item(1798, 99, &[]))).unwrap();
    let mut stats = CoreStats {
        power: 65_000_000_000,
        tiem_nang: 0,
        hp: 1000,
        mp: 1000,
        hp_max: 1000,
        mp_max: 1000,
        dame: 500,
        def: 10,
        crit: 5,
        stamina: 10_000,
        max_stamina: 10_000,
        limit_power: 5,
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
    let mut rng = GameRng::seeded(11);
    {
        let mut uctx = UseItemCtx {
            now_ms: 5_000,
            current_day: 2,
            last_wood_chest_day: &mut day,
            last_pea_ms: &mut pea,
            gender: Gender::TraiDat,
            stats: &mut stats,
            buffs: &mut buffs,
            fusion_mode: &mut fus,
            has_pet: true,
            pet_type: &mut pt,
            pet_power: 50_000_000_000,
            pet_unlocked_skills: 0b1111,
            zone_satellites: 0,
            hungry_soi_in_zone: false,
            mabu_egg: &mut egg,
        };
        UseItemService::use_bag_item(&mut inv, 0, &mut uctx, &BuiltinCatalog, &mut rng).unwrap();
        assert!(uctx.buffs.has_active(1_800_000, |s| s == BuffSlot::CoBonLa));
        UseItemService::use_bag_item(&mut inv, 1, &mut uctx, &BuiltinCatalog, &mut rng).unwrap();
        assert_eq!(inv.count_template_in_bag(TemplateId(1805)), 1);
    }
    let mut ctx = NpcPlayerCtx {
        account_id: AccountId(1),
        player_id: PlayerId(1),
        location: Location {
            map_id: MapId(114),
            zone_id: ZoneId(0),
            x: 200,
            y: 312,
        },
        gender: Gender::TraiDat,
        c_flag: 9,
        activated: true,
        has_clan: true,
        is_clan_leader: true,
        clan_days: 5,
        has_pet: true,
        wearing_full_destroyer: true,
        is_use_gtpt: false,
        is_phu_ho_mabu: false,
        mabu_points: 10,
        vnd_balance: 500_000,
        vip_purchases: 0,
        kol_stage: 1,
        kol_vip_stage: 1,
        kol_progress: 200,
        stats: &mut stats,
        inventory: &mut inv,
    };
    assert_eq!(
        NpcMenuRouter::confirm_menu(NpcKind::Osin, 10_000, 1, &mut ctx).unwrap(),
        NpcEffect::ApplyGtpt
    );
    assert_eq!(ctx.inventory.gem, 19);
    ctx.location.map_id = MapId(const_map::HANH_TINH_KAIO as i16);
    if let NpcEffect::ShowMenu(p) = NpcMenuRouter::open_base_menu(NpcKind::Bill, &ctx).unwrap() {
        assert_eq!(
            NpcMenuRouter::confirm_menu(NpcKind::Bill, p.menu_index, 0, &mut ctx).unwrap(),
            NpcEffect::OpenShop("BILL")
        );
    }
    let mut expired = GiftCodeDef {
        code: "tanthu".into(),
        count_left: 5,
        expires_at_ms: 1_000,
        items: vec![],
        redeemed_by: vec![],
    };
    assert!(matches!(
        GiftCodeService::redeem(&mut expired, ctx.inventory, PlayerId(1), 2_000),
        Err(UseItemError::GiftCodeRejected(_))
    ));
}

proptest! {
    #[test]
    fn prop_combine_nhap_ngoc_rong_and_hematite_conserve_mass(
        qty in 7u32..500u32, star_id in 15i16..=20i16, spl_qty in 5u32..500u32,
    ) {
        let mut inv = Inventory::new(10, 10, 12);
        inv.gold = 100_000_000;
        inv.set_bag_slot(0, Some(ItemInstance { template_id: TemplateId(star_id), quantity: qty, options: smallvec![], create_time_ms: 0 })).unwrap();
        inv.set_bag_slot(1, Some(ItemInstance { template_id: TemplateId(441), quantity: spl_qty, options: smallvec![], create_time_ms: 0 })).unwrap();
        let mut rng = GameRng::seeded(7);
        let ctx = CombineContext::default_hardened();
        CombineService::execute(&mut inv, CombineOp::NhapNgocRong, &[0], &ctx, &BuiltinCatalog, &mut rng).unwrap();
        CombineService::execute(&mut inv, CombineOp::TaoDaHematite, &[1], &ctx, &BuiltinCatalog, &mut rng).unwrap();
        prop_assert_eq!(inv.count_template_in_bag(TemplateId(star_id)), qty - 7);
        prop_assert_eq!(inv.count_template_in_bag(TemplateId(star_id - 1)), 1);
        prop_assert_eq!(inv.count_template_in_bag(TemplateId(441)), spl_qty - 5);
        prop_assert_eq!(inv.count_template_in_bag(TemplateId(1423)), 1);
    }
}
