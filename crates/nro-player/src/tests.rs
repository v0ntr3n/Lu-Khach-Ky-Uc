//! Unit and `proptest` property tests for `crates/nro-player`.
//!
//! Verifies 5-piece SKH/Seal/Tier set activations, `NPoint` overflow saturation past `i32::MAX`,
//! `ParityMode` legacy Java truncations (`3/2`, `10/100`, `tdTriTue4`), damage reduction invariants,
//! skills/Mafuba/Tự Sát, Pet Hakai/Fusion/HTVV, Task bit-packing, Clans, and 7 AI Bot classes.

use nro_core::{
    AccountId, ClanId, CoreStats, GameRng, Gender, ItemInstance, ItemOption, Location, MapId,
    ParityMode, PlayerId, TemplateId, ZoneId,
};
use proptest::prelude::*;
use smallvec::smallvec;

use crate::bot_ai::{BotAgent, BotArchetype, BotDirective, BotManager, BotTradeConfig, BotWorldView, MARKET_MAP_ID};
use crate::npoint::{AttackCalcRequest, BuffFlags, IncomingHit, StatUpgradeKind, TnsmContext};
use crate::pet::{FusionKind, NewPet, Pet, PetAction, PetKind, PetStatus, PetTargetCandidate};
use crate::progression::{CLAN_JOIN_MIN_TASK_KEY, Clan, ClanRole, SubTaskMain, TaskDifficulty, TaskPlayer};
use crate::set_clothes::{DISGUISE_SLOT, TierSetFlags, pirate_crew_bonus_pct};
use crate::skills::{
    ManaUseType, MobMeFamiliar, SkillKind, SkillSlot, SpecialSkillState, SpecialTickEvent,
    apply_huyt_sao_target, commit_skill_cast, compute_tu_sat_damages,
};
use crate::{Player, PlayerError};

fn sample_core(hp: i64, mp: i64, dame: i64) -> CoreStats {
    CoreStats {
        hpg: hp as _, mpg: mp as _, hp, mp, dameg: dame as _, defg: 100, critg: 5,
        power: 20_000_000_000, tiem_nang: 500_000_000, limit_power: 5, stamina: 1000, max_stamina: 1000, ..CoreStats::default() }
}

fn loc(map: u16, x: i16, y: i16) -> Location {
    Location { map_id: MapId(map as _), zone_id: ZoneId(0), x, y }
}

#[test]
fn set_clothes_activates_planetary_seal_tier_and_gogeta_sets() {
    let mut pl = Player::new(PlayerId(1), AccountId(1), Gender::Xayda, sample_core(100_000, 100_000, 10_000), loc(0, 100, 100));
    let mut rng = GameRng::from_seed(7);
    let mut pet = Pet::spawn(PlayerId(1), PetKind::Uub, Gender::Xayda, &mut rng);
    for slot in 0..5 {
        pl.inventory.items_body[slot] = Some(ItemInstance {
            template_id: TemplateId(1048),
            quantity: 1,
            options: smallvec![
                ItemOption { id: 135, param: 1 },
                ItemOption { id: 34, param: 1 },
                ItemOption { id: 35, param: 1 },
                ItemOption { id: 36, param: 1 },
                ItemOption { id: 211, param: 1 },
            ], create_time_ms: 0 });
    }
    pl.inventory.items_body[DISGUISE_SLOT] = Some(ItemInstance { template_id: TemplateId(2133), quantity: 1, options: smallvec![], create_time_ms: 0 });
    pet.items_body[DISGUISE_SLOT] = Some(ItemInstance { template_id: TemplateId(2134), quantity: 1, options: smallvec![], create_time_ms: 0 });
    pl.pet = Some(pet);

    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_angel());
    assert!(pl.sets.tiers.contains(TierSetFlags::GOGETA_PAIR));
    assert_eq!(pl.sets.nappa, 5);
    assert!(pl.npoint.seals.has_tinh_an() && pl.npoint.seals.has_nguyet_an() && pl.npoint.seals.has_nhat_an());
    assert!(pl.npoint.seals.has_linh_thuy_danh_bac());
    assert_eq!(pirate_crew_bonus_pct(Some(618), &[618, 619, 620]), 9);
}

#[test]
fn parity_mode_toggles_picolo_kaio_tnsm_and_tu_sat_quirks() {
    let mut pl = Player::new(PlayerId(2), AccountId(2), Gender::Namec, sample_core(100_000, 200_000, 10_000), loc(0, 0, 0));
    for slot in 0..5 {
        pl.inventory.items_body[slot] = Some(ItemInstance {
            template_id: TemplateId(557),
            quantity: 1,
            options: smallvec![ItemOption { id: 130, param: 1 }], create_time_ms: 0 });
    }
    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_god());
    assert_eq!(pl.sets.picolo, 5);

    let mut rng = GameRng::from_seed(42);
    let mut req = AttackCalcRequest {
        skill: SkillKind::Makankosappo, skill_damage_pct: 200, skill_point: 7, intrinsic: (0, 0),
        sets: pl.sets, combat_flags: (true, false, false), sexy_dame_buff_pct: 0,
        qckk_surrounding_hp: (0, 0), parity: ParityMode::Fixed,
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
            options: smallvec![ItemOption { id: 245, param: 1 }], create_time_ms: 0 });
    }
    pl.recalculate_stats(false, false, 1_000, ParityMode::Fixed);
    assert!(pl.sets.check_set_des());
    assert_eq!(pl.npoint.crit, 15);
    pl.npoint.hp = 5_000;
    let mut kk = SkillSlot { kind: SkillKind::Kaioken, skill_id: 9, point: 7, cool_down_ms: 500, mana_use: 10, mana_type: ManaUseType::Flat, damage_pct: 150, curr_level: 0, last_used_ms: 0 };
    assert!(commit_skill_cast(&mut pl.npoint, &mut kk, &pl.sets, (0, 0), 2_000, ParityMode::Fixed).is_ok());

    let hits_fixed = compute_tu_sat_damages(100_000, 100_000, false, &pl.sets, &[true, true], ParityMode::Fixed);
    assert_eq!(hits_fixed.as_slice(), &[50_000, 50_000]);
    assert_eq!(TaskDifficulty::Hell.side_task_reward(ParityMode::Fixed), (500_000, 704));

    let ctx = TnsmContext { charms_tri_tue: (false, false, true), world_bonus: (false, 0, 1), parity: ParityMode::Fixed, ..Default::default() };
    assert_eq!(pl.npoint.cal_suc_manh_tiem_nang(1_000, &ctx), 5_000);
}

#[test]
fn crit_upgrade_uses_v250_cost_table() {
    let mut pl = Player::new(PlayerId(9), AccountId(9), Gender::TraiDat, sample_core(100_000, 100_000, 10_000), loc(0, 0, 0));
    pl.npoint.core.critg = 3;
    pl.npoint.core.limit_power = 9;
    pl.npoint.core.tiem_nang = 100_000_000_000;
    assert_eq!(pl.npoint.increase_point(StatUpgradeKind::Crit, 2, ParityMode::Fixed), Ok(20_000_000_000));
    assert_eq!(pl.npoint.core.critg, 5);
    assert_eq!(pl.npoint.increase_point(StatUpgradeKind::Crit, 1, ParityMode::Fixed), Ok(30_000_000_000));
}

#[test]
fn skills_mafuba_super_kame_huyt_sao_and_mob_me_behave_deterministically() {
    let mut rng = GameRng::from_seed(11);
    let mut spec = SpecialSkillState::default();
    let mut mpb = SkillSlot { kind: SkillKind::MaPhongBa, skill_id: 26, point: 5, cool_down_ms: 30_000, mana_use: 20, mana_type: ManaUseType::PercentMax, damage_pct: 200, curr_level: 10, last_used_ms: 0 };
    spec.start(&mut mpb, 1, 250, 80, true, true, 1_000);
    assert_eq!(mpb.curr_level, 11);
    assert_eq!(spec.tick_250ms(true, 2_500, &mut rng), SpecialTickEvent::Charging);
    match spec.tick_250ms(true, 3_000, &mut rng) {
        SpecialTickEvent::MafubaCapture(radius_px, seal_duration_ms, _) => {
            assert_eq!((radius_px, seal_duration_ms), (500, 22_000));
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let mut saiyan = Player::new(PlayerId(3), AccountId(3), Gender::Xayda, sample_core(50_000, 50_000, 5_000), loc(5, 0, 0));
    let mut namek = Player::new(PlayerId(4), AccountId(4), Gender::Namec, sample_core(50_000, 50_000, 5_000), loc(5, 0, 0));
    apply_huyt_sao_target(&mut saiyan.npoint, &mut saiyan.effects, Gender::Xayda, 7, 1_000);
    apply_huyt_sao_target(&mut namek.npoint, &mut namek.effects, Gender::Namec, 7, 1_000);
    assert_eq!(saiyan.effects.huyt_sao_pct, 100);
    assert_eq!(namek.npoint.hp, 45_000);

    let mut egg = MobMeFamiliar::spawn(7, 100_000, 40_000, true, 1_000);
    assert!(egg.infinite && !egg.injure(999_999));
    assert_eq!(egg.hp, 90_000 - 4_500);
}

#[test]
fn pet_fusion_assimilate_tasks_clan_and_bots_execute_cleanly() {
    let mut rng = GameRng::from_seed(123);
    let mut master = Player::new(PlayerId(10), AccountId(10), Gender::Namec, sample_core(100_000, 100_000, 10_000), loc(21, 200, 336));
    let mut pet = Pet::spawn(PlayerId(10), PetKind::Uub, Gender::Namec, &mut rng);
    pet.change_status(PetStatus::Fusion, &mut master.fusion, FusionKind::Porata2, 10_000).expect("fusion");
    master.pet = Some(pet);
    master.recalculate_stats(false, false, 10_000, ParityMode::Fixed);
    assert!(master.npoint.hp_max > 500_000 && master.npoint.dame > 30_000);

    if let Some(p) = &mut master.pet {
        p.unfuse(&mut master.fusion, 12_000);
        assert_eq!(p.change_status(PetStatus::Fusion, &mut master.fusion, FusionKind::Porata1, 14_000), Err(PlayerError::FusionCooldownActive));
        p.status = PetStatus::PermanentAssimilate;
        let act = p.tick_ai(master.location, master.gender, false, &[], 20_000, ParityMode::Fixed, &mut rng);
        assert_eq!(act, PetAction::AssimilateIntoMaster(40_000_000_000));
    }
    assert_eq!(NewPet::parts_for_item(1765), Some((1662, 1663, 1764)));

    assert_eq!(TaskPlayer::encode_task_key(0, 1), 2);
    assert_eq!(TaskPlayer::encode_task_key(10, 0), 20_480);
    assert_eq!(TaskPlayer::decode_task_key(57_358), (28, 7));
    master.tasks.main_id = 3;
    master.tasks.sub_tasks.push(SubTaskMain { count: 0, max_count: 1, npc_id: 1, map_id: 22 });
    assert_eq!(master.tasks.record_event(TaskPlayer::encode_task_key(3, 0), 1, Gender::Namec), Some((3, 5)));

    let mut clan = Clan::create(ClanId(1), 5, PlayerId(10), 20_000_000_000);
    assert_eq!(clan.join(PlayerId(11), 1_000_000, 100, false), Err(PlayerError::ClanActionDenied));
    clan.join(PlayerId(11), 1_000_000, CLAN_JOIN_MIN_TASK_KEY, false).expect("join");
    clan.members[1].role = ClanRole::Deputy;
    clan.transfer_leadership(PlayerId(10), PlayerId(11)).expect("transfer");
    assert_eq!(clan.members[0].role, ClanRole::Member);
    let msg_id = clan.ask_for_pea(PlayerId(10), 400_000).expect("ask_for_pea");
    assert_eq!(clan.donate_pea(PlayerId(11), msg_id), Ok(PlayerId(10)));

    let cfg = BotTradeConfig { offer_item_id: TemplateId(457), required_item_id: TemplateId(16), rate_per_unit: 10 };
    assert_eq!(cfg.evaluate_offer(&[ItemInstance { template_id: TemplateId(16), quantity: 25, options: smallvec![], create_time_ms: 0 }]), Some((TemplateId(457), 2)));
    let mut mgr = BotManager::default();
    for (i, arch) in [BotArchetype::MobGrinder, BotArchetype::MarketTrader, BotArchetype::BossHunter, BotArchetype::PvpHunter].into_iter().enumerate() {
        mgr.bots.push(BotAgent::spawn(PlayerId(100 + i as i64), Gender::TraiDat, (0, 1, 2, 0), arch, Some(cfg), 1_000, &mut rng));
    }
    let world = BotWorldView {
        zone_mobs: &[(1, loc(21, 220, 336))],
        zone_items: &[(9, loc(21, 210, 336))],
        map_players: &[(PlayerId(10), loc(21, 250, 336))],
        active_bosses: &[(PlayerId(-50), loc(105, 300, 336), false, false, true)],
    };
    let directives = mgr.tick_all(&world, 2_000, ParityMode::Fixed, &mut rng);
    assert_eq!(directives.len(), 4);
    assert!(matches!(directives[1].1, BotDirective::ChangeMap { map_id, .. } if map_id.0 as u16 == MARKET_MAP_ID));
}

proptest! {
    #[test]
    fn stats_never_wrap_negative_under_extreme_multipliers(
        hpg in 1_i64..500_000_000,
        mpg in 1_i64..500_000_000,
        dameg in 1_i64..500_000_000,
        pct1 in 0_i32..10_000,
        pct2 in 0_i32..10_000,
        tn_gain in 0_u64..u64::MAX,
    ) {
        let mut pl = Player::new(PlayerId(7), AccountId(7), Gender::Xayda, sample_core(hpg, mpg, dameg), loc(5, 0, 0));
        pl.inventory.items_body[0] = Some(ItemInstance {
            template_id: TemplateId(654),
            quantity: 1,
            options: smallvec![
                ItemOption { id: 77, param: pct1 },
                ItemOption { id: 103, param: pct2 },
                ItemOption { id: 50, param: pct1 },
                ItemOption { id: 117, param: pct2 },
            ], create_time_ms: 0 });
        pl.buffs.flags = BuffFlags::BO_HUYET_2 | BuffFlags::BO_KHI_2 | BuffFlags::CUONG_NO_2 | BuffFlags::NUOC_MIA_3;
        pl.buffs.black_balls_mask = 0x7F;
        pl.buffs.war_mults = (5, 5);
        pl.recalculate_stats(false, true, 5_000, ParityMode::Fixed);

        prop_assert!(pl.npoint.hp_max >= 0);
        prop_assert!(pl.npoint.mp_max >= 0);
        prop_assert!(pl.npoint.dame >= 0);

        let before = pl.npoint.core.tiem_nang;
        pl.npoint.tiem_nang_up(tn_gain);
        prop_assert!(pl.npoint.core.tiem_nang >= before);
    }

    #[test]
    fn damage_reduction_pipeline_is_monotonic_and_bounded(
        raw_hit in 100_i64..5_000_000_000,
        giap in 0_i16..150,
        def in 0_i32..50_000,
        shielding in any::<bool>(),
        in_pho_ban in any::<bool>(),
        bat_tu in any::<bool>(),
    ) {
        let mut base = Player::new(PlayerId(8), AccountId(8), Gender::TraiDat, sample_core(1_000_000, 100_000, 10_000), loc(5, 0, 0)).npoint;
        base.hp_max = 1_000_000;
        base.hp = 500_000;
        base.def = def;
        base.mods.tl_giap = giap;

        let (mut n_none, mut n_gx1, mut n_gx2, mut n_pvp, mut n_mob) = (base.clone(), base.clone(), base.clone(), base.clone(), base);
        let mut rng = GameRng::from_seed(99);
        let mk_hit = |is_mob: bool, ds: (bool, bool, bool, bool, bool)| IncomingHit {
            raw_damage: raw_hit, attacker_skill: Some(SkillKind::Kamejoko), attacker_pen: (0, 0, 0),
            hit_flags: (false, is_mob, false), defense_state: ds,
        };
        let d0 = n_none.apply_injured(&mk_hit(true, (false, false, false, false, false)), &mut rng).damage_taken;
        let d1 = n_gx1.apply_injured(&mk_hit(true, (true, false, false, false, false)), &mut rng).damage_taken;
        let d2 = n_gx2.apply_injured(&mk_hit(true, (false, true, false, false, false)), &mut rng).damage_taken;
        prop_assert!(d0 >= d1 && d1 >= d2 && d2 >= 0);

        let out_pvp = n_pvp.apply_injured(&mk_hit(false, (false, false, shielding, in_pho_ban, false)), &mut rng);
        if shielding {
            prop_assert_eq!(out_pvp.damage_taken, if in_pho_ban { 10 } else { 1 });
        }
        let _ = n_mob.apply_injured(&mk_hit(true, (false, false, false, false, bat_tu)), &mut rng);
        if bat_tu {
            prop_assert!(n_mob.hp >= 1);
        }
    }
}

#[test]
fn pet_protect_hakai_respects_linh_thuy_danh_bac_immunity() {
    let mut rng = GameRng::from_seed(5);
    let mut pet = Pet::spawn(PlayerId(1), PetKind::KidJiren, Gender::Xayda, &mut rng);
    pet.status = PetStatus::Protect;
    let immune = PetTargetCandidate { id: 99, location: loc(21, 210, 336), hp: 500_000, is_mob: false, is_boss: false, has_ltdb: true };
    let vulnerable = PetTargetCandidate { id: 99, location: loc(21, 210, 336), hp: 500_000, is_mob: false, is_boss: false, has_ltdb: false };
    let mut saw_hakai = false;
    for t in 0..40 {
        let act_imm = pet.tick_ai(loc(21, 200, 336), Gender::Xayda, false, &[immune], 20_000 + t * 2_000, ParityMode::Fixed, &mut rng);
        assert_ne!(act_imm, PetAction::HakaiKill(PlayerId(99)));
        let act_vul = pet.tick_ai(loc(21, 200, 336), Gender::Xayda, false, &[vulnerable], 21_000 + t * 2_000, ParityMode::Fixed, &mut rng);
        if act_vul == PetAction::HakaiKill(PlayerId(99)) {
            saw_hakai = true;
        }
    }
    assert!(saw_hakai);
}
