//! Complete `NPoint` stat calculator (`cal_point`), outgoing damage (`get_dame_attack`), `sub_hp`, and `tiem_nang_up`.
//! Uses `i64`/`u64` saturating math capped at `i32::MAX` (`2,147,483,647`) with [`ParityMode`] toggles.

use bitflags::bitflags;
use nro_consts as consts;
use nro_core::{CoreStats, GameRng, ItemInstance, ItemOption, ParityMode};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub use crate::combat::{AttackCalcRequest, IncomingHit, InjuryOutcome, StatUpgradeKind, TnsmContext, cal_sub_tnsm, power_caption_level};
use crate::pet::{FusionKind, PetKind};
use crate::set_clothes::{SealSetCounts, SetClothes, TRAIN_ARMOR_SLOT, TierSetFlags, train_armor_percent};
use crate::tpl_raw;

/// Maximum signed 32-bit stat ceiling enforced by the NRO protocol (`2,147,483,647`).
pub const MAX_STAT_I64: i64 = i32::MAX as i64;

/// `(power_cap, hp_mp_cap, dame_cap, def_cap, crit_cap)` indexed by `limit_power` (`0..=9`).
pub const LIMIT_TIERS: [(i64, i32, i32, i16, u8); 10] = [
    (17_999_999_999, 220_000, 11_000, 550, 1), (19_999_999_999, 240_000, 12_000, 600, 2),
    (24_999_999_999, 300_000, 15_000, 700, 3), (29_999_999_999, 350_000, 18_000, 800, 4),
    (39_999_999_999, 400_000, 20_000, 1_000, 5), (50_010_000_000, 450_000, 22_000, 1_200, 6),
    (60_010_000_000, 500_000, 24_000, 1_400, 7), (70_010_000_000, 525_000, 24_500, 1_500, 8),
    (80_010_000_000, 550_000, 25_000, 1_600, 9), (90_010_000_000, 575_000, 26_000, 1_800, 10),
];

bitflags! {
    /// Passive equipment & radar option flags populated during [`NPoint::cal_point`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct PassiveOptionFlags: u16 {
        /// Option `33` or Yardrat armor (`592..=594`): allows instant teleport to players.
        const TELEPORT = 1 << 0;
        /// Option `105`: invisible when out of combat for `5,000` ms.
        const WEARING_VO_HINH = 1 << 1;
        /// Option `106`: immune to Cold Map `50%` HP & Dame penalty.
        const KHONG_LANH = 1 << 2;
        /// Option `116`: immune to `Thái Dương Hạ San` stun.
        const KHANG_TDHS = 1 << 3;
        /// Option `24`: slows nearby players (`speed = 1` for `5s` every `10s`).
        const AURA_LAM_CHAM = 1 << 4;
        /// Option `25`: periodic invisibility (`1.5s` every `5s`).
        const AURA_TAN_HINH = 1 << 5;
        /// Option `26`: petrifies nearby players (`6s` every `30s`).
        const AURA_HOA_DA = 1 << 6;
        /// Option `110`: crystal radar detection (`isDoSPL`).
        const DO_SPL = 1 << 7;
        /// Currently wearing Training Armor in `itemsBody[6]`.
        const WEARING_TRAIN_ARMOR = 1 << 8;
        /// Option `111`: Xinbato dodge aura source.
        const HAS_XINBATO = 1 << 9;
    }

    /// Timed consumable (`ItemTime`), meal, and map blessing flags read by [`NPoint`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct BuffFlags: u32 {
        /// `Bổ Huyết` (`2x` `hp_max`).
        const BO_HUYET = 1 << 0;
        /// `Bổ Huyết Cấp 2` (`2.2x` `hp_max`).
        const BO_HUYET_2 = 1 << 1;
        /// `Bổ Khí` (`2x` `mp_max`).
        const BO_KHI = 1 << 2;
        /// `Bổ Khí Cấp 2` (`2.2x` `mp_max`).
        const BO_KHI_2 = 1 << 3;
        /// `Cuồng Nộ` (`2x` `dame`).
        const CUONG_NO = 1 << 4;
        /// `Cuồng Nộ Cấp 2` (`2.2x` `dame`).
        const CUONG_NO_2 = 1 << 5;
        /// `Giáp Xên` (`50%` incoming DR).
        const GIAP_XEN = 1 << 6;
        /// `Giáp Xên Cấp 2` (`60%` incoming DR).
        const GIAP_XEN_2 = 1 << 7;
        /// Sugarcane juice #1 (`+10%` HP).
        const NUOC_MIA_1 = 1 << 8;
        /// Sugarcane juice #2 (`+10%` HP, `+10` crit).
        const NUOC_MIA_2 = 1 << 9;
        /// Sugarcane juice #3 (`+10%` HP, `+10%` dame, `+10%` def, `+10%` crit dame).
        const NUOC_MIA_3 = 1 << 10;
        /// `Rồng Xương` (`+10%` HP, MP, Dame).
        const RONG_XUONG = 1 << 11;
        /// Standard `Thức Ăn` (`+10%` Dame, shared with Pet).
        const EAT_MEAL = 1 << 12;
        /// `Đuôi Khỉ` (`+2x` base TNSM).
        const DUOI_KHI = 1 << 13;
        /// `Kho Báu X2` (`+2x` base TNSM on maps `135..=138`).
        const KHO_BAU_X2 = 1 << 14;
        /// `Bùa Santa` (`+2x` base Pet TNSM).
        const BUA_SANTA = 1 << 15;
        /// `Nồi Cơm Điện` (`2x` Mafuba seal & DoT).
        const USE_NCD = 1 << 16;
        /// MaBu 2H map blessing (`+1,000,000` HP/MP, `+10,000` Dame).
        const PHU_HO_MABU = 1 << 17;
    }
}

/// Active consumables, Black Ball Star bitmask (`bit 0..=6` for 1..=7 stars), and war multipliers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveBuffs {
    /// Consumable & map blessing bitflags.
    pub flags: BuffFlags,
    /// Bitmask of active Black Ball Star rewards (`bit 0` = 1-Star `+21%` dame .. `bit 6` = 7-Star `+14%` dodge).
    pub black_balls_mask: u8,
    /// Secondary meal icon (`8060` `+5%` Dame, `8061` `+5%` Crit Dame, `8062` `+5%` HP).
    pub eat_meal_2_icon: u16,
    /// Black Ball War priest multipliers `(x_hp_ki, x_dame)` (default `(1, 1)`).
    pub war_mults: (u8, u8),
}

/// Snapshot of external context needed to run [`NPoint::cal_point`].
pub struct StatContext<'a> {
    /// Equipped body items (`itemsBody`).
    pub body: &'a [Option<ItemInstance>],
    /// Active radar card options (`Used == 1`, filtered by card level).
    pub radar_options: &'a [ItemOption],
    /// Active badge options (`BagesTemplate`).
    pub badge_options: &'a [ItemOption],
    /// Optional `(porata_earring_921_or_1819, cached_train_armor)` from bag.
    pub bag_gear: (Option<&'a ItemInstance>, Option<&'a ItemInstance>),
    /// Computed `SetClothes`.
    pub sets: SetClothes,
    /// Active consumables & Black Ball buffs.
    pub buffs: ActiveBuffs,
    /// `true` if human player (`isPl()`).
    pub is_human_player: bool,
    /// Pet kind & fusion status when this `NPoint` belongs to a `Pet`.
    pub self_pet_info: Option<(PetKind, FusionKind, bool)>,
    /// Fused pet's calculated `(hp_max, mp_max, dame)` added when Master is fused.
    pub fused_pet_stats: Option<(i32, i32, i32)>,
    /// Active `(monkey_level, huyt_sao_pct, chibi_type)`.
    pub form_state: (u8, i32, Option<u8>),
    /// Map flags `(in_cold_map, in_black_ball_war)`.
    pub map_env: (bool, bool),
    /// Arithmetic parity mode (`ParityMode::Fixed` vs Java legacy bugs).
    pub parity: ParityMode,
}

/// Derived percentage & proc modifiers rebuilt on every [`NPoint::cal_point`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OptionModifiers {
    /// Sequential HP `%` bonuses (`Option 77`).
    pub tl_hp: SmallVec<[i32; 12]>,
    /// Sequential MP `%` bonuses (`Option 103`).
    pub tl_mp: SmallVec<[i32; 12]>,
    /// Sequential Dame `%` bonuses (`Options 49, 50, 147`).
    pub tl_dame: SmallVec<[i32; 12]>,
    /// Sequential vs-Mob Dame `%` bonuses (`Option 19`).
    pub tl_dame_att_mob: SmallVec<[i32; 8]>,
    /// Sequential TNSM `%` bonuses (`Options 88, 101, 156`).
    pub tl_tnsm: SmallVec<[i32; 8]>,
    /// Crit damage `%` (`Option 5`).
    pub tl_sdcm: i32,
    /// Option `3` beam damage absorbed as MP (`%`).
    pub vo_hieu_chuong: i32,
    /// Max Option `117 / 226` Sexy Dame (`%`).
    pub tl_sexy_dame: i32,
    /// Option `162` Cute MP regen (`%`).
    pub mp_hoi_cute: i32,
    /// Percent armor (`<= 86`, `Option 94`).
    pub tl_giap: i16,
    /// Base dodge `%` (`<= 90`, `Options 108, 111`).
    pub tl_ne_don: i16,
    /// Xinbato temporary dodge `%`.
    pub tl_ne_don_xinbato: i16,
    /// Penetration `(chinh_xac_18, xgc_beam_98, xgcc_melee_99)`.
    pub pen_stats: (i16, i16, i16),
    /// Drain `%`: `[hut_hp, hut_mp, hut_hp_mob, hut_hp_mp_xq, hp_giam_odo]`.
    pub drains: [i16; 5],
    /// Utility `%`: `[pst_reflect, gold_bonus, bom_explode, tnsm_pet, x_chuong, sub_sd]`.
    pub utils: [i16; 6],
}

/// Complete port of `nro.models.player.NPoint`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NPoint {
    /// Persisted base stats (`hpg, mpg, dameg, defg, critg, power, tiem_nang, limit_power, stamina`).
    pub core: CoreStats,
    /// Current HP (`0..=hp_max`).
    pub hp: i32,
    /// Derived maximum HP (`<= 2,147,483,647`).
    pub hp_max: i32,
    /// Current MP/KI (`0..=mp_max`).
    pub mp: i32,
    /// Derived maximum MP/KI (`<= 2,147,483,647`).
    pub mp_max: i32,
    /// Derived base attack damage (`<= 2,147,483,647`).
    pub dame: i32,
    /// Derived flat defense (`defg * 4 + def_add`).
    pub def: i32,
    /// Derived critical strike chance (`%`).
    pub crit: i32,
    /// Permanent Shenron wish crit bonus (`critdragon`).
    pub crit_dragon: i32,
    /// Movement speed (`8` base).
    pub speed: u8,
    /// Porata earring upgrade tier (`levelBT`).
    pub level_bt: u8,
    /// 30-second `(hp_hoi, mp_hoi)` regeneration amounts.
    pub regen: (i32, i32),
    /// Recomputed option percentages.
    pub mods: OptionModifiers,
    /// Active 5-piece Seal (`Ấn`) counters.
    pub seals: SealSetCounts,
    /// Active passive option bitflags.
    pub flags: PassiveOptionFlags,
    /// Transient combat flags: `(last_crit, is_crit_100, is_crit_tele, is_x_chuong_ready, is_x_dame_boss_nerf)`.
    pub combat_state: (bool, bool, bool, bool, bool),
    /// Post-CC intrinsic bonus damage `%` (`dameAfter`).
    pub dame_after: i32,
    /// Stamina consumption counter (`numAttack`).
    pub num_attack: u16,
}

impl NPoint {
    /// Creates a new `NPoint` from `CoreStats` and initializes HP/MP to clamped base values.
    pub fn new(core: CoreStats) -> Self {
        let (hp, mp) = (clamp_i32(core.hp.max(0)), clamp_i32(core.mp.max(0)));
        let (hp_max, mp_max, crit_dragon) = (clamp_i32(core.hpg.max(1)), clamp_i32(core.mpg.max(1)), i32::from(core.critdragon));
        Self {
            core, hp, hp_max, mp, mp_max, dame: 1, def: 0, crit: 0, crit_dragon,
            speed: 8, level_bt: 0, regen: (0, 0), mods: OptionModifiers::default(),
            seals: SealSetCounts::default(), flags: PassiveOptionFlags::empty(),
            combat_state: (false, false, false, false, false), dame_after: 0, num_attack: 0,
        }
    }

    /// Recomputes all derived stats (`NPoint.calPoint` / `setPointWhenWearClothes`).
    pub fn cal_point(&mut self, ctx: &StatContext<'_>) {
        let _ = consts::PER100;
        self.mods = OptionModifiers { tl_ne_don_xinbato: self.mods.tl_ne_don_xinbato, ..Default::default() };
        (self.seals, self.flags, self.level_bt) = (SealSetCounts::default(), PassiveOptionFlags::empty(), 0);
        let (mut flat, mut tl_reg, mut tl_spd) = ([0_i64; 7], [0_i32; 4], 0_i32);
        let bf = ctx.buffs.flags;
        let bb = |star: u8| (ctx.buffs.black_balls_mask & (1 << (star - 1))) != 0;
        if bb(3) { self.mods.drains[0] = self.mods.drains[0].saturating_add(35); }
        if bb(4) { self.mods.utils[0] = self.mods.utils[0].saturating_add(35); }
        if bb(5) { self.mods.tl_sdcm += 35; }
        if bb(7) { self.mods.tl_ne_don = self.mods.tl_ne_don.saturating_add(14); }
        for o in ctx.radar_options { self.apply_option(o, false, &mut flat, &mut tl_reg, &mut tl_spd); }
        if let Some(bt) = ctx.bag_gear.0 {
            for o in &bt.options {
                self.apply_option(o, true, &mut flat, &mut tl_reg, &mut tl_spd);
                if o.id == 72 { self.level_bt = o.param.clamp(0, 255) as u8; }
            }
        }
        for it in ctx.body.iter().flatten() {
            if (592..=594).contains(&tpl_raw(it.template_id)) { self.flags.insert(PassiveOptionFlags::TELEPORT); }
            for o in &it.options { self.apply_option(o, true, &mut flat, &mut tl_reg, &mut tl_spd); }
        }
        for o in ctx.badge_options {
            if o.id == 108 { self.mods.tl_ne_don = self.mods.tl_ne_don.saturating_add((i64::from(self.mods.tl_ne_don) * i64::from(o.param) / 100) as i16); }
        }
        if let Some(Some(gtl)) = ctx.body.get(TRAIN_ARMOR_SLOT) {
            self.flags.insert(PassiveOptionFlags::WEARING_TRAIN_ARMOR);
            self.mods.utils[5] = self.mods.utils[5].saturating_add(train_armor_percent(gtl.template_id) as i16);
        } else if let Some(c) = ctx.bag_gear.1 && c.options.iter().any(|o| o.id == 9 && o.param > 0) {
            self.mods.tl_dame.push(train_armor_percent(c.template_id));
        }

        let gogeta = ctx.sets.tiers.contains(TierSetFlags::GOGETA_PAIR);
        let pet_fused = ctx.self_pet_info.is_some_and(|(_, _, f)| f);
        let div_porata = ctx.self_pet_info.is_some_and(|(k, mf, _)| mf.is_porata() && matches!(k, PetKind::Uub | PetKind::KidBeer | PetKind::KidJiren));
        let (prev_mp, (monkey_lvl, huyt_sao_pct, chibi)) = (i64::from(self.mp_max), ctx.form_state);

        let mut hp_m = (self.core.hpg as i64).saturating_add(flat[0]);
        for &tl in &self.mods.tl_hp { hp_m = add_pct(hp_m, i64::from(tl)); }
        for (c, p) in [(ctx.sets.nappa >= 5, 80), (ctx.sets.cadic_m >= 2, 20), (ctx.sets.worldcup == 2, 10), (bf.contains(BuffFlags::RONG_XUONG), 10), (self.seals.has_nhat_an(), 15), (bb(2), 35), (monkey_lvl > 0 && !pet_fused, (i64::from(monkey_lvl) + 3) * 10), (div_porata, 20)] {
            if c { hp_m = add_pct(hp_m, p); }
        }
        if ctx.map_env.1 && ctx.buffs.war_mults.0 > 1 { hp_m = hp_m.saturating_mul(i64::from(ctx.buffs.war_mults.0)); }
        if ctx.buffs.eat_meal_2_icon == 8062 { hp_m = add_pct(hp_m, 5); }
        if gogeta { hp_m = add_pct(hp_m, 10); }
        if bf.contains(BuffFlags::PHU_HO_MABU) { hp_m = hp_m.saturating_add(1_000_000); }
        if let Some((p_hp, _, _)) = ctx.fused_pet_stats { hp_m = hp_m.saturating_add(i64::from(p_hp)); }
        if bf.contains(BuffFlags::BO_HUYET) && !bf.contains(BuffFlags::BO_HUYET_2) { hp_m = hp_m.saturating_mul(2); }
        for f in [BuffFlags::NUOC_MIA_3, BuffFlags::NUOC_MIA_2, BuffFlags::NUOC_MIA_1] { if bf.contains(f) { hp_m = hp_m.saturating_add(hp_m / 10); } }
        if bf.contains(BuffFlags::BO_HUYET_2) { hp_m = scale_2_2x(hp_m, ctx.parity); }
        if !pet_fused && huyt_sao_pct != 0 { hp_m = add_pct(hp_m, i64::from(huyt_sao_pct)); }
        if chibi == Some(3) { hp_m = hp_m.saturating_mul(2); }
        if ctx.map_env.0 && !self.flags.contains(PassiveOptionFlags::KHONG_LANH) { hp_m /= 2; }
        for o in ctx.badge_options { if o.id == 77 { hp_m = add_pct(hp_m, i64::from(o.param)); } }
        (self.hp_max, self.hp) = (clamp_i32(hp_m.max(1)), self.hp.clamp(0, clamp_i32(hp_m.max(1))));

        let mut mp_m = (self.core.mpg as i64).saturating_add(flat[1]);
        for &tl in &self.mods.tl_mp { mp_m = add_pct(mp_m, i64::from(tl)); }
        if self.seals.has_nguyet_an() { mp_m = add_pct(mp_m, 15); }
        if bb(6) { mp_m = add_pct(mp_m, 40); }
        for (c, p) in [(ctx.sets.worldcup == 2, 10_i64), (div_porata, 20_i64)] {
            if c { mp_m = mp_m.saturating_add((if matches!(ctx.parity, ParityMode::Fixed) { mp_m } else { prev_mp }) * p / 100); }
        }
        if ctx.map_env.1 && ctx.buffs.war_mults.0 > 1 { mp_m = mp_m.saturating_mul(i64::from(ctx.buffs.war_mults.0)); }
        if gogeta { mp_m = add_pct(mp_m, 10); }
        if bf.contains(BuffFlags::PHU_HO_MABU) { mp_m = mp_m.saturating_add(1_000_000); }
        if bf.contains(BuffFlags::RONG_XUONG) { mp_m = add_pct(mp_m, 10); }
        if let Some((_, p_mp, _)) = ctx.fused_pet_stats { mp_m = mp_m.saturating_add(i64::from(p_mp)); }
        if bf.contains(BuffFlags::BO_KHI) && !bf.contains(BuffFlags::BO_KHI_2) { mp_m = mp_m.saturating_mul(2); }
        if bf.contains(BuffFlags::BO_KHI_2) { mp_m = scale_2_2x(mp_m, ctx.parity); }
        for o in ctx.badge_options { if o.id == 103 { mp_m = add_pct(mp_m, i64::from(o.param)); } }
        (self.mp_max, self.mp) = (clamp_i32(mp_m.max(1)), self.mp.clamp(0, clamp_i32(mp_m.max(1))));

        let mut d = (self.core.dameg as i64).saturating_add(flat[2]);
        for &tl in &self.mods.tl_dame { d = add_pct(d, i64::from(tl)); }
        if ctx.self_pet_info.is_some_and(|(k, mf, _)| mf.is_porata() && k == PetKind::KidBeer) && !matches!(ctx.parity, ParityMode::Fixed) { d = add_pct(d, 20); }
        if div_porata { d = add_pct(d, 20); }
        for (c, p) in [(self.seals.has_tinh_an(), 15), (bf.contains(BuffFlags::EAT_MEAL), 10), (ctx.buffs.eat_meal_2_icon == 8060, 5)] { if c { d = add_pct(d, p); } }
        if ctx.sets.nail >= 2 && matches!(ctx.parity, ParityMode::Fixed) { self.mods.tl_sdcm += 10; }
        if ctx.buffs.eat_meal_2_icon == 8061 { self.mods.tl_sdcm += 5; }
        if bf.contains(BuffFlags::NUOC_MIA_3) { self.mods.tl_sdcm += 10; }
        if bf.contains(BuffFlags::CUONG_NO) && !bf.contains(BuffFlags::CUONG_NO_2) { d = d.saturating_mul(2); }
        if bf.contains(BuffFlags::NUOC_MIA_3) { d = d.saturating_add(d / 10); }
        if bf.contains(BuffFlags::CUONG_NO_2) { d = scale_2_2x(d, ctx.parity); }
        for (c, p) in [(bb(1), 21), (ctx.sets.worldcup == 2, 10), (gogeta, 10)] { if c { d = add_pct(d, p); } }
        if bf.contains(BuffFlags::PHU_HO_MABU) { d = d.saturating_add(10_000); }
        if bf.contains(BuffFlags::RONG_XUONG) { d = add_pct(d, 10); }
        if ctx.map_env.1 && ctx.buffs.war_mults.1 > 1 { d = d.saturating_mul(i64::from(ctx.buffs.war_mults.1)); }
        if let Some((_, _, p_d)) = ctx.fused_pet_stats { d = d.saturating_add(i64::from(p_d)); }
        for o in ctx.badge_options {
            if o.id == 50 { d = add_pct(d, i64::from(o.param)); }
            if o.id == 117 { self.mods.tl_sexy_dame = self.mods.tl_sexy_dame.saturating_add(o.param); }
        }
        if monkey_lvl > 0 && !pet_fused { d = add_pct(d, i64::from(monkey_lvl) + 3); }
        d = add_pct(d, i64::from(self.mods.tl_sexy_dame));
        if !matches!(ctx.parity, ParityMode::Fixed) { d = add_pct(d, i64::from(self.mods.tl_sexy_dame)); }
        d = d.saturating_sub(d.saturating_mul(i64::from(self.mods.utils[5])) / 100);
        if ctx.map_env.0 && !self.flags.contains(PassiveOptionFlags::KHONG_LANH) { d /= 2; }
        self.dame = clamp_i32(d);

        let def = (self.core.defg as i64).saturating_mul(4).saturating_add(flat[3]);
        self.def = clamp_i32(if bf.contains(BuffFlags::NUOC_MIA_3) { add_pct(def, 10) } else { def });
        let mut crit = if monkey_lvl > 0 { 110 } else { (self.core.critg as i64) + flat[4] + i64::from(self.crit_dragon) };
        if bf.contains(BuffFlags::NUOC_MIA_2) { crit += 10; }
        if ctx.sets.than_vu_tru_kaio >= 1 { crit += if matches!(ctx.parity, ParityMode::Fixed) { 10 } else { 10 / 100 }; }
        self.crit = clamp_i32(crit);

        let (hm, mm) = (i64::from(self.hp_max), i64::from(self.mp_max));
        self.regen.0 = clamp_i32(hm / 100 + flat[5] + hm * i64::from(tl_reg[0].clamp(0, 100) + tl_reg[2].clamp(0, 100)) / 100);
        self.regen.1 = clamp_i32(mm / 100 + flat[6] + mm * i64::from(tl_reg[1].clamp(0, 100) + tl_reg[3].clamp(0, 100)) / 100);
        if ctx.is_human_player { self.speed = (8 + (8 * tl_spd.max(0)) / 100).clamp(1, 127) as u8; }
    }

    fn apply_option(&mut self, io: &ItemOption, body: bool, flat: &mut [i64; 7], reg: &mut [i32; 4], spd: &mut i32) {
        let (p, ps) = (i64::from(io.param), io.param as i16);
        self.seals.record_option(io.id);
        match io.id {
            0 => flat[2] += p, 2 => { flat[0] += p * 1000; flat[1] += p * 1000; }
            3 => self.mods.vo_hieu_chuong += io.param, 5 => self.mods.tl_sdcm += io.param,
            6 => flat[0] += p, 7 => flat[1] += p, 8 => self.mods.drains[3] = self.mods.drains[3].saturating_add(ps),
            14 => flat[4] += p, 16 | 114 | 148 => *spd += io.param, 18 => self.mods.pen_stats.0 = self.mods.pen_stats.0.saturating_add(ps),
            19 => self.mods.tl_dame_att_mob.push(io.param), 22 => flat[0] += p * 1000, 23 => flat[1] += p * 1000,
            24 if body => self.flags.insert(PassiveOptionFlags::AURA_LAM_CHAM),
            25 if body => self.flags.insert(PassiveOptionFlags::AURA_TAN_HINH),
            26 if body => self.flags.insert(PassiveOptionFlags::AURA_HOA_DA),
            27 => flat[5] += p, 28 => flat[6] += p, 33 => self.flags.insert(PassiveOptionFlags::TELEPORT),
            47 => flat[3] += p, 48 => { flat[0] += p; flat[1] += p; }
            49 | 50 | 147 => self.mods.tl_dame.push(io.param), 77 => self.mods.tl_hp.push(io.param),
            80 => reg[0] += io.param, 81 => reg[1] += io.param, 88 | 101 => self.mods.tl_tnsm.push(io.param),
            94 => self.mods.tl_giap = self.mods.tl_giap.saturating_add(ps),
            95 => self.mods.drains[0] = self.mods.drains[0].saturating_add(ps),
            96 => self.mods.drains[1] = self.mods.drains[1].saturating_add(ps),
            97 => self.mods.utils[0] = self.mods.utils[0].saturating_add(ps),
            98 => self.mods.pen_stats.1 = self.mods.pen_stats.1.saturating_add(ps),
            99 => self.mods.pen_stats.2 = self.mods.pen_stats.2.saturating_add(ps),
            100 => self.mods.utils[1] = self.mods.utils[1].saturating_add(ps),
            103 => self.mods.tl_mp.push(io.param), 104 => self.mods.drains[2] = self.mods.drains[2].saturating_add(ps),
            105 => self.flags.insert(PassiveOptionFlags::WEARING_VO_HINH),
            106 => self.flags.insert(PassiveOptionFlags::KHONG_LANH),
            108 | 111 => { self.mods.tl_ne_don = self.mods.tl_ne_don.saturating_add(ps); if io.id == 111 && body { self.flags.insert(PassiveOptionFlags::HAS_XINBATO); } }
            109 => self.mods.drains[4] = self.mods.drains[4].saturating_add(ps),
            110 if body => self.flags.insert(PassiveOptionFlags::DO_SPL),
            116 => self.flags.insert(PassiveOptionFlags::KHANG_TDHS),
            117 | 226 => self.mods.tl_sexy_dame = self.mods.tl_sexy_dame.max(io.param),
            153 => self.mods.utils[2] = self.mods.utils[2].saturating_add(ps),
            156 => { self.mods.utils[5] += 50; self.mods.tl_tnsm.push(io.param); self.mods.utils[1] = self.mods.utils[1].saturating_add(ps); }
            159 if body => self.mods.utils[4] = ps, 160 if body => self.mods.utils[3] = self.mods.utils[3].saturating_add(ps),
            162 => self.mods.mp_hoi_cute += io.param, 173 => { reg[2] += io.param; reg[3] += io.param; }
            _ => {}
        }
    }

    /// Computes outgoing attack damage (`NPoint.getDameAttack`), toggling `3/2` & `Masenko` quirks via [`ParityMode`].
    #[inline] pub fn get_dame_attack(&mut self, req: &AttackCalcRequest, rng: &mut GameRng) -> i32 { self.eval_dame_attack(req, rng) }
    /// Subtracts `sub` from `hp` with saturating floor at `0` (`NPoint.subHP`).
    #[inline] pub fn sub_hp(&mut self, sub: i64) { self.hp = (i64::from(self.hp).saturating_sub(sub.max(0))).clamp(0, i64::from(self.hp_max)) as i32; }
    /// Adds `amount` to `hp` clamped to `hp_max`.
    #[inline] pub fn add_hp(&mut self, amount: i64) { if amount > 0 { self.hp = (i64::from(self.hp).saturating_add(amount)).min(i64::from(self.hp_max)) as i32; } }
    /// Adds signed `delta` to `mp` clamped to `[0, mp_max]`.
    #[inline] pub fn add_mp(&mut self, delta: i64) { self.mp = (i64::from(self.mp).saturating_add(delta)).clamp(0, i64::from(self.mp_max)) as i32; }
    /// Current HP percentage (`0..=100`).
    #[inline] pub const fn curr_percent_hp(&self) -> i32 { if self.hp_max <= 0 { 100 } else { ((self.hp as i64) * 100 / (self.hp_max as i64)) as i32 } }
    /// Adds `amount` to `tiem_nang` with saturating `i64` cap (`NPoint.tiemNangUp`).
    #[inline] pub fn tiem_nang_up(&mut self, amount: u64) { self.core.tiem_nang = self.core.tiem_nang.saturating_add(amount.min(i64::MAX as u64) as i64); }
    /// Adds `amount` to `power` with saturating `i64` cap (`NPoint.powerUp`).
    #[inline] pub fn power_up(&mut self, amount: u64) { self.core.power = self.core.power.saturating_add(amount.min(i64::MAX as u64) as i64); }
    /// Returns the power cap for the current `limit_power` (`0..=9`).
    #[inline] pub fn power_limit(&self) -> i64 { LIMIT_TIERS.get(usize::from(self.core.limit_power)).map_or(0, |t| t.0) }
}

#[inline] pub(crate) const fn add_pct(v: i64, pct: i64) -> i64 { v.saturating_add(v.saturating_mul(pct) / 100) }
#[inline] fn scale_2_2x(v: i64, parity: ParityMode) -> i64 { if matches!(parity, ParityMode::Fixed) { v.saturating_mul(11) / 5 } else { (v as f64 * 2.2) as i64 } }
#[inline] pub(crate) const fn clamp_i32(v: i64) -> i32 { if v <= 0 { 0 } else if v > MAX_STAT_I64 { i32::MAX } else { v as i32 } }
