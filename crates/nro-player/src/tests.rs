//! Unit and `proptest` property tests for `crates/nro-player`.

#[path = "tests_prop.rs"]
mod tests_prop;

use nro_core::{
    AccountId, CoreStats, GameRng, Gender, ItemInstance, ItemOption, Location, MapId, ParityMode,
    PlayerId, TemplateId, ZoneId,
};
use smallvec::smallvec;

use crate::Player;
use crate::npoint::{AttackCalcRequest, StatUpgradeKind, TnsmContext};
use crate::pet::{Pet, PetKind};
use crate::progression::TaskDifficulty;
use crate::set_clothes::{DISGUISE_SLOT, TierSetFlags, pirate_crew_bonus_pct};
use crate::skills::{
    ManaUseType, MobMeFamiliar, SkillKind, SkillSlot, SpecialSkillState, SpecialTickEvent,
    apply_huyt_sao_target, commit_skill_cast, compute_tu_sat_damages,
};

pub(super) fn sample_core(hp: i64, mp: i64, dame: i64) -> CoreStats {
    CoreStats {
        hpg: hp as _,
        mpg: mp as _,
        hp,
        mp,
        dameg: dame as _,
        defg: 100,
        critg: 5,
        power: 20_000_000_000,
        tiem_nang: 500_000_000,
        limit_power: 5,
        stamina: 1000,
        max_stamina: 1000,
        ..CoreStats::default()
    }
}

pub(super) fn loc(map: u16, x: i16, y: i16) -> Location {
    Location {
        map_id: MapId(map as _),
        zone_id: ZoneId(0),
        x,
        y,
    }
}

#[test]
fn set_clothes_activates_planetary_seal_tier_dhd_and_gogeta_sets() {
    let mut pl = Player::new(
        PlayerId(1),
        AccountId(1),
        Gender::Xayda,
        sample_core(100_000, 100_000, 10_000),
        loc(0, 100, 100),
    );
    let mut rng = GameRng::from_seed(7);
    let mut pet = Pet::spawn(PlayerId(1), PetKind::Uub, Gender::Xayda, &mut rng);
    for slot in 0..5 {
        pl.inventory.items_body[slot] = Some(ItemInstance {
            template_id: TemplateId(1048),
            quantity: 1,
            options: smallvec![
                ItemOption { id: 135, param: 1 },
                ItemOption { id: 21, param: 80 },
                ItemOption { id: 34, param: 1 },
                ItemOption { id: 35, param: 1 },
                ItemOption { id: 36, param: 1 },
                ItemOption { id: 211, param: 1 },
            ],
            create_time_ms: 0,
        });
    }
    pl.inventory.items_body[DISGUISE_SLOT] = Some(ItemInstance {
        template_id: TemplateId(2133),
        quantity: 1,
        options: smallvec![],
        create_time_ms: 0,
    });
    pet.items_body[DISGUISE_SLOT] = Some(ItemInstance {
        template_id: TemplateId(2134),
        quantity: 1,
        options: smallvec![],
        create_time_ms: 0,
    });
    pl.pet = Some(pet);

    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_angel());
    assert!(pl.sets.tiers.contains(TierSetFlags::GOGETA_PAIR));
    assert_eq!(pl.sets.nappa, 5);
    assert_eq!(pl.sets.set_dhd, 5);
    assert!(
        pl.npoint.seals.has_tinh_an()
            && pl.npoint.seals.has_nguyet_an()
            && pl.npoint.seals.has_nhat_an()
    );
    assert!(pl.npoint.seals.has_linh_thuy_danh_bac());
    assert_eq!(pirate_crew_bonus_pct(Some(618), &[618, 619, 620]), 9);
}

#[test]
fn parity_mode_toggles_picolo_kaio_tnsm_and_tu_sat_quirks() {
    let mut pl = Player::new(
        PlayerId(2),
        AccountId(2),
        Gender::Namec,
        sample_core(100_000, 200_000, 10_000),
        loc(0, 0, 0),
    );
    for slot in 0..5 {
        pl.inventory.items_body[slot] = Some(ItemInstance {
            template_id: TemplateId(557),
            quantity: 1,
            options: smallvec![ItemOption { id: 130, param: 1 }],
            create_time_ms: 0,
        });
    }
    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_god());
    assert_eq!(pl.sets.picolo, 5);

    let mut rng = GameRng::from_seed(42);
    let mut req = AttackCalcRequest {
        skill: SkillKind::Makankosappo,
        skill_damage_pct: 200,
        skill_point: 7,
        intrinsic: (0, 0),
        sets: pl.sets,
        combat_flags: (true, false, false),
        sexy_dame_buff_pct: 0,
        qckk_surrounding_hp: (0, 0),
        parity: ParityMode::Fixed,
    };
    assert_eq!(pl.npoint.get_dame_attack(&req, &mut rng), 600_000);
    if !matches!(ParityMode::default(), ParityMode::Fixed) {
        req.parity = ParityMode::default();
        assert_eq!(pl.npoint.get_dame_attack(&req, &mut rng), 400_000);
    }

    for slot in 0..5 {
        pl.inventory.items_body[slot] = Some(ItemInstance {
            template_id: TemplateId(650),
            quantity: 1,
            options: smallvec![ItemOption { id: 245, param: 1 }],
            create_time_ms: 0,
        });
    }
    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_des());
    assert_eq!(pl.npoint.crit, 15);
    pl.npoint.hp = 5_000;
    let mut kk = SkillSlot {
        kind: SkillKind::Kaioken,
        skill_id: 9,
        point: 7,
        cool_down_ms: 500,
        mana_use: 10,
        mana_type: ManaUseType::Flat,
        damage_pct: 150,
        curr_level: 0,
        last_used_ms: 0,
    };
    assert!(
        commit_skill_cast(
            &mut pl.npoint,
            &mut kk,
            &pl.sets,
            (0, 0),
            2_000,
            ParityMode::Fixed
        )
        .is_ok()
    );

    let hits_fixed = compute_tu_sat_damages(
        100_000,
        100_000,
        false,
        &pl.sets,
        &[true, true],
        ParityMode::Fixed,
    );
    assert_eq!(hits_fixed.as_slice(), &[50_000, 50_000]);
    assert_eq!(
        TaskDifficulty::Hell.side_task_reward(ParityMode::Fixed),
        (500_000, 704)
    );

    let ctx = TnsmContext {
        charms_tri_tue: (false, false, true),
        world_bonus: (false, 0, 1),
        parity: ParityMode::Fixed,
        ..Default::default()
    };
    assert_eq!(pl.npoint.cal_suc_manh_tiem_nang(1_000, &ctx), 5_000);
}

#[test]
fn crit_upgrade_uses_v250_cost_table() {
    let mut pl = Player::new(
        PlayerId(9),
        AccountId(9),
        Gender::TraiDat,
        sample_core(100_000, 100_000, 10_000),
        loc(0, 0, 0),
    );
    pl.npoint.core.critg = 3;
    pl.npoint.core.limit_power = 9;
    pl.npoint.core.tiem_nang = 100_000_000_000;
    assert_eq!(
        pl.npoint
            .increase_point(StatUpgradeKind::Crit, 2, ParityMode::Fixed),
        Ok(20_000_000_000)
    );
    assert_eq!(pl.npoint.core.critg, 5);
    assert_eq!(
        pl.npoint
            .increase_point(StatUpgradeKind::Crit, 1, ParityMode::Fixed),
        Ok(30_000_000_000)
    );
}

#[test]
fn skills_mafuba_super_kame_huyt_sao_and_mob_me_behave_deterministically() {
    let mut rng = GameRng::from_seed(11);
    let mut spec = SpecialSkillState::default();
    let mut mpb = SkillSlot {
        kind: SkillKind::MaPhongBa,
        skill_id: 26,
        point: 5,
        cool_down_ms: 30_000,
        mana_use: 20,
        mana_type: ManaUseType::PercentMax,
        damage_pct: 200,
        curr_level: 10,
        last_used_ms: 0,
    };
    spec.start(&mut mpb, 1, 250, 80, true, true, 1_000);
    assert_eq!(mpb.curr_level, 11);
    assert_eq!(
        spec.tick_250ms(true, 2_500, &mut rng),
        SpecialTickEvent::Charging
    );
    match spec.tick_250ms(true, 3_000, &mut rng) {
        SpecialTickEvent::MafubaCapture(radius_px, seal_duration_ms, _) => {
            assert_eq!((radius_px, seal_duration_ms), (500, 22_000));
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let mut saiyan = Player::new(
        PlayerId(3),
        AccountId(3),
        Gender::Xayda,
        sample_core(50_000, 50_000, 5_000),
        loc(5, 0, 0),
    );
    let mut namek = Player::new(
        PlayerId(4),
        AccountId(4),
        Gender::Namec,
        sample_core(50_000, 50_000, 5_000),
        loc(5, 0, 0),
    );
    apply_huyt_sao_target(
        &mut saiyan.npoint,
        &mut saiyan.effects,
        Gender::Xayda,
        7,
        1_000,
    );
    apply_huyt_sao_target(
        &mut namek.npoint,
        &mut namek.effects,
        Gender::Namec,
        7,
        1_000,
    );
    assert_eq!(saiyan.effects.huyt_sao_pct, 100);
    assert_eq!(namek.npoint.hp, 45_000);

    let mut egg = MobMeFamiliar::spawn(7, 100_000, 40_000, true, 1_000);
    assert!(egg.infinite && !egg.injure(999_999));
    assert_eq!(egg.hp, 90_000 - 4_500);
}
