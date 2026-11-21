//! Outgoing attack (`get_dame_attack`), incoming mitigation (`apply_injured`), `TNSM`, and stat upgrades.
//!
//! Implements the combat and progression methods on [`NPoint`] (`getDameAttack`, `Player.injured`,
//! `subDameInjureWithDeff`, `calSucManhTiemNang`, `calSubTNSM`, `increasePoint`), re-exported by
//! [`crate::npoint`] so callers can import everything from `nro_player::npoint`.

use nro_core::{GameRng, ParityMode};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::npoint::{LIMIT_TIERS, NPoint, add_pct, clamp_i32};
use crate::set_clothes::SetClothes;
use crate::skills::SkillKind;
use crate::{PlayerError, rng_one_sign, rng_range, rng_ratio};

/// Base stat categories upgradeable with Potential (`NPoint.increasePoint`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum StatUpgradeKind {
    /// Base HP (`+20` `hpg` per point).
    Hp = 0,
    /// Base MP/KI (`+20` `mpg` per point).
    Mp = 1,
    /// Base Attack Damage (`+1` `dameg` per point).
    Dame = 2,
    /// Base Defense (`+1` `defg` per point).
    Def = 3,
    /// Base Critical Chance (`+1` `critg` per point).
    Crit = 4,
}

/// Request payload for [`NPoint::get_dame_attack`].
pub struct AttackCalcRequest {
    /// Selected skill.
    pub skill: SkillKind,
    /// Skill template damage percentage (`skillSelect.damage`).
    pub skill_damage_pct: i32,
    /// Skill level (`1..=7`).
    pub skill_point: u8,
    /// Active intrinsic `(id, param1)`.
    pub intrinsic: (u8, i32),
    /// Computed `SetClothes`.
    pub sets: SetClothes,
    /// Combat flags: `(is_attack_mob, pet_charm_active, is_monkey)`.
    pub combat_flags: (bool, bool, bool),
    /// Active `EffectSkill` sexy-dame buff percentage (`tileDameBuff`).
    pub sexy_dame_buff_pct: i32,
    /// Sum of `(mob_hp, humanoid_hp)` within `rangeQCKK` for `QuaCauKenhKhi`.
    pub qckk_surrounding_hp: (i64, i64),
    /// Arithmetic parity mode.
    pub parity: ParityMode,
}

/// Incoming hit descriptor for [`NPoint::apply_injured`].
pub struct IncomingHit {
    /// Raw incoming damage before mitigation.
    pub raw_damage: i64,
    /// Attacker's skill (when hit by a player/boss skill).
    pub attacker_skill: Option<SkillKind>,
    /// Attacker's `(tl_chinh_xac, tl_xgc, tl_xgcc)` penetration stats.
    pub attacker_pen: (i32, i32, i32),
    /// Flags: `(piercing, is_mob_attack, cap_mabu_5pct)`.
    pub hit_flags: (bool, bool, bool),
    /// Defender active states: `(giap_xen_1, giap_xen_2, is_shielding, in_pho_ban, td_bat_tu)`.
    pub defense_state: (bool, bool, bool, bool, bool),
}

/// Result of [`NPoint::apply_injured`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InjuryOutcome {
    /// Final HP damage applied (`0` if dodged or absorbed).
    pub damage_taken: i32,
    /// MP restored via Option `3` (`vo_hieu_chuong`) beam absorption.
    pub mp_absorbed: i32,
    /// Pre-shield damage recorded for `Phản Sát Thương` (`damePST`).
    pub reflect_base_damage: i32,
    /// `true` if `Khiên Năng Lượng` shattered (`damage > hp_max`).
    pub shield_broken: bool,
    /// `true` if the attack missed due to dodge or Xinbato aura.
    pub dodged: bool,
}

/// Multipliers & environment flags for [`NPoint::cal_suc_manh_tiem_nang`].
#[derive(Clone, Copy, Debug, Default)]
pub struct TnsmContext {
    /// Active charms `(tri_tue_x2, tri_tue_x3, tri_tue_x4)`.
    pub charms_tri_tue: (bool, bool, bool),
    /// Active VIP flags `(timevip, session_vip)`.
    pub vip_active: (bool, bool),
    /// Bonus flags: `(chibi_x2, duoi_khi, kho_bau_x2, ve_tinh_tri_tue_20pct)`.
    pub bonus_flags: (bool, bool, bool, bool),
    /// Intrinsic `(id, param1)` (`id == 24` grants `+param1%` TNSM).
    pub intrinsic: (u8, i32),
    /// Master's `(bua_santa, tl_tnsm_pet)` when calculating TNSM for a `Pet`.
    pub pet_master_bonus: Option<(bool, i32)>,
    /// World modifiers `(in_bdkb, c_flag, server_exp_rate)`.
    pub world_bonus: (bool, u8, u8),
    /// Arithmetic parity mode.
    pub parity: ParityMode,
}

impl NPoint {
    pub(crate) fn eval_dame_attack(&mut self, req: &AttackCalcRequest, rng: &mut GameRng) -> i32 {
        self.combat_state.0 = if req.intrinsic.0 == 25 && self.curr_percent_hp() <= req.intrinsic.1 { true }
        else if self.combat_state.1 { self.combat_state.1 = false; true }
        else { rng_ratio(rng, self.crit.max(0) as u32, 100) };
        if req.skill != SkillKind::DichChuyenTucThoi && self.combat_state.2 {
            (self.combat_state.0, self.combat_state.2) = (true, false);
        }
        let (mut d, mut p_int, mut p_sk, mut p_xd) = (i64::from(self.dame), 0_i64, i64::from(req.skill_damage_pct), 0_i64);
        match req.skill {
            SkillKind::Makankosappo => {
                let mut lz = i64::from(self.mp_max).saturating_mul(p_sk) / 100;
                if req.sets.picolo >= 5 && matches!(req.parity, ParityMode::Fixed) { lz = lz.saturating_mul(3) / 2; }
                return clamp_i32(lz);
            }
            SkillKind::QuaCauKenhKhi => {
                let mut q = (req.qckk_surrounding_hp.0.saturating_mul(10) / 100).saturating_add(req.qckk_surrounding_hp.1.saturating_mul(10) / 100).saturating_add(d * 10);
                if req.sets.kirin >= 5 { q = q.saturating_mul(2); }
                return clamp_i32(add_pct(q, rng_range(rng, -5, 5)));
            }
            SkillKind::DeTrung => return clamp_i32(if req.sets.pikkoro_daimao >= 5 { d.saturating_mul(4) } else { d }),
            SkillKind::Dragon if req.intrinsic.0 == 1 => p_int = i64::from(req.intrinsic.1),
            SkillKind::Kamejoko => {
                if req.intrinsic.0 == 2 { p_int = i64::from(req.intrinsic.1); }
                if req.sets.songoku >= 5 { p_xd = 100; }
            }
            SkillKind::Galick => {
                if req.intrinsic.0 == 16 { p_int = i64::from(req.intrinsic.1); }
                if req.sets.kakarot >= 5 { p_xd = 100; }
            }
            SkillKind::Antomic if req.intrinsic.0 == 17 => p_int = i64::from(req.intrinsic.1),
            SkillKind::Demon if req.intrinsic.0 == 8 => p_int = i64::from(req.intrinsic.1),
            SkillKind::Masenko => {
                let b = (if req.intrinsic.0 == 9 { i64::from(req.intrinsic.1) } else { 0 }) + if req.sets.nail >= 5 { 80 } else { 0 };
                p_sk += b;
                if !matches!(req.parity, ParityMode::Fixed) { p_xd = i64::from(b as i8); }
            }
            SkillKind::LienHoan => {
                if req.intrinsic.0 == 13 { p_int = i64::from(req.intrinsic.1); }
                if req.sets.oc_tieu >= 5 { p_xd = 100; }
            }
            SkillKind::Kaioken if req.sets.than_vu_tru_kaio >= 5 => p_xd = 30,
            SkillKind::DichChuyenTucThoi => {
                (self.combat_state.0, self.combat_state.2, p_sk) = (true, true, 0);
                d = rng_range(rng, d.saturating_sub(d / 100 * 5), d.saturating_add(d / 100 * 5));
            }
            _ => {}
        }
        if req.intrinsic.0 == 18 && req.combat_flags.2 { p_int = i64::from(req.intrinsic.1); }
        if p_sk != 0 { d = d.saturating_mul(p_sk) / 100; }
        d = add_pct(add_pct(d, p_int), i64::from(self.dame_after));
        self.dame_after = 0;
        if req.sexy_dame_buff_pct > 0 && self.mods.tl_sexy_dame == 0 { d = add_pct(d, i64::from(req.sexy_dame_buff_pct)); }
        if req.combat_flags.0 {
            for &tl in &self.mods.tl_dame_att_mob { d = add_pct(d, i64::from(tl)); }
            if req.combat_flags.1 { d = d.saturating_mul(2); }
        }
        if self.combat_state.0 { d = add_pct(d.saturating_mul(2), i64::from(self.mods.tl_sdcm)); }
        d = add_pct(d, p_xd);
        let vcap = (d / 100 * 5).max(1);
        d = d.saturating_add(rng_one_sign(rng) * rng_range(rng, 0, vcap - 1) + 1).max(0);
        if self.combat_state.3 && req.skill.is_beam() && self.mods.utils[4] > 0 {
            d = d.saturating_mul(i64::from(self.mods.utils[4]));
            (self.combat_state.3, self.combat_state.4) = (false, true);
        }
        clamp_i32(d)
    }

    /// Applies incoming damage mitigation (`Player.injured` + `subDameInjureWithDeff` + `subHP`).
    pub fn apply_injured(&mut self, hit: &IncomingHit, rng: &mut GameRng) -> InjuryOutcome {
        let (piercing, is_mob, cap_mabu) = hit.hit_flags;
        let mut d = hit.raw_damage.max(0);
        if cap_mabu { d = d.min(i64::from(self.hp_max / 20).max(1)); }
        if !piercing && let Some(sk) = hit.attacker_skill && sk.is_beam() && self.mods.vo_hieu_chuong > 0 {
            let mp_gain = clamp_i32(d.saturating_mul(i64::from(self.mods.vo_hieu_chuong)) / 100);
            self.add_mp(i64::from(mp_gain));
            return InjuryOutcome { damage_taken: 0, mp_absorbed: mp_gain, reflect_base_damage: 0, shield_broken: false, dodged: false };
        }
        let (mut ne, mut xb, mut giap) = (i32::from(self.mods.tl_ne_don), i32::from(self.mods.tl_ne_don_xinbato), i32::from(self.mods.tl_giap));
        if !is_mob && let Some(sk) = hit.attacker_skill {
            if sk.allows_dodge_and_gx() { ne -= hit.attacker_pen.0; } else { ne = 0; xb = 0; }
            if sk.is_beam() { giap = (giap - hit.attacker_pen.1).max(0); } else if sk.is_melee() { giap = (giap - hit.attacker_pen.2).max(0); }
        }
        if piercing { (ne, xb, giap) = (0, 0, 0); }
        let (ne, xb, giap) = (ne.clamp(0, 90), xb.clamp(0, 90), giap.clamp(0, 86));
        if !piercing && (rng_ratio(rng, ne as u32, 100) || (xb > 0 && rng_ratio(rng, xb as u32, 100))) {
            return InjuryOutcome { damage_taken: 0, mp_absorbed: 0, reflect_base_damage: 0, shield_broken: false, dodged: true };
        }
        d = d.saturating_sub((d / 100).saturating_mul(i64::from(giap)));
        if !piercing { d = (d - i64::from(self.def)).max(1); }
        let (gx1, gx2, shielding, pho_ban, bat_tu) = hit.defense_state;
        if !piercing && (is_mob || hit.attacker_skill.is_some_and(SkillKind::allows_dodge_and_gx)) {
            if gx2 { d = (d / 100).saturating_mul(40); } else if gx1 { d /= 2; }
        }
        let (mut shield_broken, mut reflect_base) = (false, 0_i32);
        if !piercing && shielding && !is_mob {
            reflect_base = clamp_i32(d);
            shield_broken = d > i64::from(self.hp_max);
            d = if pho_ban { 10 } else { 1 };
        }
        if is_mob && bat_tu && d >= i64::from(self.hp) { d = i64::from(self.hp - 1).max(0); }
        let final_d = clamp_i32(d);
        if reflect_base == 0 { reflect_base = final_d; }
        self.sub_hp(i64::from(final_d));
        InjuryOutcome { damage_taken: final_d, mp_absorbed: 0, reflect_base_damage: reflect_base, shield_broken, dodged: false }
    }

    /// Computes final Potential/Power gain (`NPoint.calSucManhTiemNang` + `calSubTNSM`).
    pub fn cal_suc_manh_tiem_nang(&self, base_tn: u64, ctx: &TnsmContext) -> u64 {
        if self.core.power >= self.power_limit() { return 10; }
        let mut tn = base_tn.min(i64::MAX as u64) as i64;
        for &tl in &self.mods.tl_tnsm { tn = add_pct(tn, i64::from(tl)); }
        let snap = tn;
        let t4 = if matches!(ctx.parity, ParityMode::Fixed) { 4 } else { 8 };
        for (cond, mult) in [(ctx.charms_tri_tue.0, 1), (ctx.charms_tri_tue.1, 3), (ctx.charms_tri_tue.2, t4), (ctx.vip_active.0, 3), (ctx.bonus_flags.0, 2), (ctx.vip_active.1, 3), (ctx.bonus_flags.1, 2), (ctx.bonus_flags.2, 2)] {
            if cond { tn = tn.saturating_add(snap.saturating_mul(mult)); }
        }
        if ctx.bonus_flags.3 { tn = tn.saturating_add(snap / 5); }
        if ctx.intrinsic.0 == 24 { tn = add_pct(tn, i64::from(ctx.intrinsic.1)); }
        if self.core.power >= 60_000_000_000 { tn = tn.saturating_sub((tn / 100).saturating_mul(80)); }
        if let Some((santa, tl_pet)) = ctx.pet_master_bonus {
            if santa { tn = tn.saturating_add(snap.saturating_mul(2)); }
            if tl_pet > 0 { tn = tn.saturating_add((snap / 100).saturating_mul(i64::from(tl_pet) + 100)); }
        }
        if ctx.world_bonus.0 { tn = tn.saturating_mul(3) / 2; }
        if ctx.world_bonus.1 != 0 { tn = add_pct(tn, if ctx.world_bonus.1 == 8 { 10 } else { 5 }); }
        tn = tn.saturating_mul(i64::from(ctx.world_bonus.2.max(1)));
        cal_sub_tnsm(self.core.power, tn.max(1) as u64).max(1)
    }

    /// Upgrades a base stat using `tiem_nang` (`NPoint.increasePoint`).
    pub fn increase_point(&mut self, kind: StatUpgradeKind, points: u16, parity: ParityMode) -> Result<u64, PlayerError> {
        const CRIT_COSTS: [i64; 14] = [
            50_000_000, 250_000_000, 1_250_000_000, 5_000_000_000, 15_000_000_000,
            30_000_000_000, 45_000_000_000, 60_000_000_000, 75_000_000_000, 90_000_000_000,
            110_000_000_000, 130_000_000_000, 150_000_000_000, 170_000_000_000,
        ];
        if points == 0 || points > 1000 { return Err(PlayerError::InvalidUpgradeAmount(points)); }
        let tier = LIMIT_TIERS.get(usize::from(self.core.limit_power)).ok_or(PlayerError::PowerLimitReached)?;
        let p = i64::from(points);
        let cost = match kind {
            StatUpgradeKind::Hp | StatUpgradeKind::Mp => {
                let base = if kind == StatUpgradeKind::Hp { self.core.hpg } else { self.core.mpg };
                if base.saturating_add(p.saturating_mul(20)) > i64::from(tier.1) { return Err(PlayerError::PowerLimitReached); }
                let b = base.max(0);
                p * (2 * (b + 1000) + p * 20 - 20) / 2
            }
            StatUpgradeKind::Dame => {
                if self.core.dameg.saturating_add(p) > i64::from(tier.2) { return Err(PlayerError::PowerLimitReached); }
                let b = self.core.dameg.max(0);
                p * (2 * b + p - 1) / 2 * 100
            }
            StatUpgradeKind::Def => {
                let d = i64::from(self.core.defg);
                if d + p > i64::from(tier.3) { return Err(PlayerError::PowerLimitReached); }
                let db = d.max(0);
                if matches!(parity, ParityMode::Fixed) { p * (2 * (db + 5) + p - 1) * 100_000 / 2 } else { (db + 5) * 100_000 }
            }
            StatUpgradeKind::Crit => {
                let c = usize::from(self.core.critg);
                if c as i64 + p > i64::from(tier.4) { return Err(PlayerError::PowerLimitReached); }
                if matches!(parity, ParityMode::Fixed) {
                    (0..usize::from(points)).fold(0_i64, |acc, i| acc.saturating_add(CRIT_COSTS.get(c + i).copied().unwrap_or(170_000_000_000)))
                } else {
                    CRIT_COSTS.get(c).copied().unwrap_or(170_000_000_000)
                }
            }
        };
        let cost = cost.max(0);
        if self.core.tiem_nang < cost { return Err(PlayerError::InsufficientPotential { required: cost as u64, available: self.core.tiem_nang.max(0) as u64 }); }
        self.core.tiem_nang = self.core.tiem_nang.saturating_sub(cost);
        match kind {
            StatUpgradeKind::Hp => self.core.hpg += p * 20,
            StatUpgradeKind::Mp => self.core.mpg += p * 20,
            StatUpgradeKind::Dame => self.core.dameg += p,
            StatUpgradeKind::Def => self.core.defg = (i64::from(self.core.defg) + p) as i32,
            StatUpgradeKind::Crit => self.core.critg = (i64::from(self.core.critg) + p) as u8,
        }
        Ok(cost as u64)
    }
}

/// High-power TNSM soft-cap divisor (`NPoint.calSubTNSM`).
#[inline]
pub const fn cal_sub_tnsm(power: i64, tn: u64) -> u64 {
    if power >= 90_000_000_000 { tn / 100 } else if power >= 80_000_000_000 { tn / 90 }
    else if power >= 60_000_000_000 { tn / 50 } else if power >= 50_000_000_000 { tn / 40 }
    else if power >= 40_000_000_000 { tn / 30 } else { tn }
}

/// Maps `power` to caption level `0..=19` (`Service.getCurrLevel`).
pub const fn power_caption_level(power: i64) -> u8 {
    const CAPS: [i64; 19] = [3_000, 15_000, 40_000, 90_000, 170_000, 340_000, 700_000, 1_500_000, 15_000_000, 150_000_000, 1_500_000_000, 5_000_000_000, 10_000_000_000, 40_000_000_000, 50_010_000_000, 60_010_000_000, 70_010_000_000, 80_010_000_000, 90_010_000_000];
    let mut i = 0;
    while i < CAPS.len() { if power < CAPS[i] { return i as u8; } i += 1; }
    19
}
