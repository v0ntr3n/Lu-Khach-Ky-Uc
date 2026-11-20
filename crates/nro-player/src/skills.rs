//! Active, passive, and Tier-5 special skills (`SkillService`, `EffectSkillService`, `SkillUtil`).
//! Covers all Earth, Namek, Saiyan, Common, and Tier-5 skills (`Super Kame`, `Liên Hoàn Chưởng`, `Ma Phong Ba`).

use bitflags::bitflags;
use nro_consts as consts;
use nro_core::{GameRng, Gender, ParityMode, PlayerId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::npoint::NPoint;
use crate::set_clothes::SetClothes;
use crate::{PlayerError, gender_index, rng_ratio};

/// Melee range threshold (`Skill.RANGE_ATTACK_CHIEU_DAM = 100`).
pub const RANGE_MELEE_PX: i32 = 100;
/// Ranged beam distance (`Skill.RANGE_ATTACK_CHIEU_CHUONG = 300`).
pub const RANGE_BEAM_PX: i32 = 300;
/// Special skill charge phase duration (`NewSkill.TIME_GONG = 2000` ms).
pub const SPECIAL_GONG_MS: u64 = 2_000;
/// Mafuba jar base seal duration (`11,000` ms; `22,000` ms with `Nồi Cơm Điện`).
pub const MAFUBA_BASE_SEAL_MS: u64 = 11_000;

/// Canonical NRO skill template IDs (`nro.models.skill.Skill`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum SkillKind {
    /// Earth basic melee (`0`).
    Dragon = 0,
    /// Earth basic beam (`1`).
    Kamejoko = 1,
    /// Namek basic melee (`2`).
    Demon = 2,
    /// Namek basic beam (`3`).
    Masenko = 3,
    /// Saiyan basic melee (`4`).
    Galick = 4,
    /// Saiyan basic beam (`5`).
    Antomic = 5,
    /// Earth AoE solar flare stun (`6`).
    ThaiDuongHaSan = 6,
    /// Namek single/AoE heal & revive (`7`).
    TriThuong = 7,
    /// Saiyan channeled HP/MP charge (`8`).
    TaiTaoNangLuong = 8,
    /// Earth HP-sacrificing melee burst (`9`).
    Kaioken = 9,
    /// Earth 2-stage spirit bomb (`10`).
    QuaCauKenhKhi = 10,
    /// Namek 2-stage piercing special beam cannon (`11`).
    Makankosappo = 11,
    /// Namek summon `MobMe` egg familiar (`12`).
    DeTrung = 12,
    /// Saiyan Great Ape transformation (`13`).
    BienKhi = 13,
    /// Saiyan 2-stage self-destruct explosion (`14`).
    TuSat = 14,
    /// Namek rapid melee flurry (`17`).
    LienHoan = 17,
    /// Common chocolate polymorph (`18`).
    Socola = 18,
    /// Common PvP energy shield (`19`).
    KhienNangLuong = 19,
    /// Earth instant transmission + blind + guaranteed crit (`20`).
    DichChuyenTucThoi = 20,
    /// Saiyan AoE max-HP whistle buff & Namek HP penalty (`21`).
    HuytSao = 21,
    /// Earth hypnosis sleep (`22`).
    ThoiMien = 22,
    /// Saiyan single-target energy bind (`23`).
    Troi = 23,
    /// Earth Tier-5 special beam (`24`).
    SuperKame = 24,
    /// Saiyan Tier-5 special flurry beam (`25`).
    LienHoanChuong = 25,
    /// Namek Tier-5 Mafuba containment wave (`26`).
    MaPhongBa = 26,
}
impl SkillKind {
    /// Returns `true` for melee skills (`SkillUtil.isUseSkillDam`).
    #[inline] pub const fn is_melee(self) -> bool { matches!(self, Self::Dragon | Self::Demon | Self::Galick | Self::Kaioken | Self::LienHoan) }
    /// Returns `true` for standard beam skills (`SkillUtil.isUseSkillChuong`).
    #[inline] pub const fn is_beam(self) -> bool { matches!(self, Self::Kamejoko | Self::Masenko | Self::Antomic) }
    /// Returns `true` for skills subject to accuracy/dodge and `Giáp Xên` (`Player.injured`).
    #[inline] pub const fn allows_dodge_and_gx(self) -> bool { self.is_melee() || self.is_beam() || matches!(self, Self::QuaCauKenhKhi | Self::Makankosappo | Self::DichChuyenTucThoi) }
    /// Resolves a skill book item template ID (`66..=515`) to [`SkillKind`] (`SkillUtil.getTempSkillSkillByItemID`).
    pub const fn from_skill_book_item(item_id: u16) -> Option<Self> {
        const BOOKS: [(u16, u16, SkillKind); 23] = [
            (66, 72, SkillKind::Dragon), (79, 84, SkillKind::Demon), (86, 86, SkillKind::Demon), (87, 93, SkillKind::Galick),
            (94, 100, SkillKind::Kamejoko), (101, 107, SkillKind::Masenko), (108, 114, SkillKind::Antomic), (115, 121, SkillKind::ThaiDuongHaSan),
            (122, 128, SkillKind::TriThuong), (129, 135, SkillKind::TaiTaoNangLuong), (300, 306, SkillKind::Kaioken), (307, 313, SkillKind::QuaCauKenhKhi),
            (314, 320, SkillKind::BienKhi), (321, 327, SkillKind::TuSat), (328, 334, SkillKind::Makankosappo), (335, 341, SkillKind::DeTrung),
            (434, 440, SkillKind::KhienNangLuong), (474, 480, SkillKind::Socola), (481, 487, SkillKind::LienHoan), (488, 494, SkillKind::DichChuyenTucThoi),
            (495, 501, SkillKind::ThoiMien), (502, 508, SkillKind::Troi), (509, 515, SkillKind::HuytSao),
        ];
        let mut i = 0;
        while i < BOOKS.len() { if item_id >= BOOKS[i].0 && item_id <= BOOKS[i].1 { return Some(BOOKS[i].2); } i += 1; }
        None
    }
}

/// Mana consumption formula mode (`SkillTemplate.manaUseType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ManaUseType {
    /// Flat MP cost (`mana_use`).
    Flat = 0,
    /// Percentage of `mp_max` (`mp_max * mana_use / 100`).
    PercentMax = 1,
    /// Requires `mp > 0` and drains all remaining MP (`mp = 0`).
    DrainAll = 2,
}

/// Learned skill instance stored in a player's or pet's skill bar (`Skill.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSlot {
    /// Skill archetype.
    pub kind: SkillKind,
    /// Protocol `skillId` (`-1` when unlearned).
    pub skill_id: i16,
    /// Skill level (`1..=7`).
    pub point: u8,
    /// Base cooldown (ms).
    pub cool_down_ms: u32,
    /// Mana cost parameter.
    pub mana_use: u32,
    /// Mana consumption mode.
    pub mana_type: ManaUseType,
    /// Skill damage `%`.
    pub damage_pct: u16,
    /// Tier-5 mastery counter (`0..=1000`).
    pub curr_level: u16,
    /// Timestamp (`ms`) of last cast.
    pub last_used_ms: u64,
}
impl SkillSlot {
    /// Checks whether the skill cooldown has elapsed with NRO's `50ms` grace window.
    #[inline] pub const fn is_off_cooldown(&self, now_ms: u64) -> bool { now_ms.saturating_sub(self.last_used_ms) >= (self.cool_down_ms.saturating_sub(50) as u64) }
    /// Computes MP required to cast this skill given `mp` and `mp_max`.
    pub const fn mana_cost(&self, mp: i32, mp_max: i32) -> Option<i32> {
        let cost = match self.mana_type {
            ManaUseType::Flat => self.mana_use as i32,
            ManaUseType::PercentMax => ((mp_max as i64) * (self.mana_use as i64) / 100) as i32,
            ManaUseType::DrainAll => return if mp > 0 { Some(mp) } else { None },
        };
        if mp >= cost { Some(cost) } else { None }
    }
}

bitflags! {
    /// Crowd-control locks matching `EffectSkill.isHaveEffectSkill()`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CcFlags: u16 {
        /// Solar flare stun (`Thái Dương Hạ San`).
        const STUN = 1 << 0;
        /// Instant transmission blind (`Dịch Chuyển Tức Thời`).
        const BLIND_DCTT = 1 << 1;
        /// Bound by `Trói`.
        const AN_TROI = 1 << 2;
        /// Asleep from `Thôi Miên`.
        const THOI_MIEN = 1 << 3;
        /// Petrified by Medusa/Stone aura (`Hóa Đá`).
        const STONE = 1 << 4;
        /// Trapped in Majin Buu cocoon (`MaBuHold`).
        const MABU_HOLD = 1 << 5;
        /// Rooted for `1,500` ms while transforming into Great Ape (`isUseSkillMonkey`).
        const MONKEY_CAST = 1 << 6;
    }
}

/// Active timed skill/crowd-control state (`EffectSkill.java` + `PlayerSkill` 2-stage flags).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EffectSkillState {
    /// Active action-blocking CC flags.
    pub cc: CcFlags,
    /// Expiry timestamps (`ms`) for `[stun, blind_dctt, an_troi, thoi_mien, stone, monkey_cast]`.
    pub cc_until_ms: [u64; 6],
    /// Energy shield expiry timestamp (`ms`).
    pub shield_until_ms: u64,
    /// Active Monkey skill level (`1..=7`, `0` if inactive) and expiry timestamp (`ms`).
    pub monkey_level: u8,
    /// Monkey expiry timestamp (`ms`).
    pub monkey_until_ms: u64,
    /// Active `Huýt Sáo` bonus HP `%` and expiry timestamp (`ms`).
    pub huyt_sao_pct: i32,
    /// `Huýt Sáo` expiry timestamp (`ms`).
    pub huyt_sao_until_ms: u64,
    /// Secondary expiry timestamps (`ms`): `(socola_until_ms, use_troi_until_ms)`.
    pub aux_until_ms: (u64, u64),
    /// Mafuba jar (`isBinh`): `(caster_id, caster_hp_max, point, type_binh, sealed_at_ms, until_ms, last_dot_ms)`.
    pub mafuba_jar: Option<(PlayerId, i32, u8, u8, u64, u64, u64)>,
    /// `Tái Tạo Năng Lượng` channel `(is_charging, ticks)` and 2-stage `(qckk, laze, tu_sat)`.
    pub charging: (bool, u8),
    /// 2-stage skill preparation timestamps `(qckk, makankosappo, tu_sat)`.
    pub stage_prep: (Option<u64>, Option<u64>, Option<u64>),
    /// Summoned `MobMe` familiar from `Đẻ Trứng`.
    pub mob_me: Option<MobMeFamiliar>,
}
impl EffectSkillState {
    /// Returns `true` if any action-blocking CC is active (`EffectSkill.isHaveEffectSkill`).
    #[inline] pub const fn is_locked(&self) -> bool { !self.cc.is_empty() }
    /// Returns `true` if sealed in a Mafuba jar within the initial `3,000` ms damage-suppression window.
    #[inline]
    pub const fn is_mafuba_suppressed(&self, now_ms: u64) -> bool {
        matches!(self.mafuba_jar, Some((_, _, _, _, start, until, _)) if now_ms < until && now_ms.saturating_sub(start) < 3_000)
    }

    /// Advances all effect timers (`EffectSkill.update` + `EffectSkin.updateMaPhongBa`), returning Mafuba piercing DoT damage.
    pub fn tick(&mut self, now_ms: u64, current_hp: i32) -> i32 {
        const FLAGS: [CcFlags; 6] = [CcFlags::STUN, CcFlags::BLIND_DCTT, CcFlags::AN_TROI, CcFlags::THOI_MIEN, CcFlags::STONE, CcFlags::MONKEY_CAST];
        for (i, flag) in FLAGS.into_iter().enumerate() {
            if self.cc.contains(flag) && now_ms >= self.cc_until_ms[i] { self.cc.remove(flag); }
        }
        for t in [&mut self.shield_until_ms, &mut self.aux_until_ms.0, &mut self.aux_until_ms.1] {
            if *t > 0 && now_ms >= *t { *t = 0; }
        }
        if self.monkey_level > 0 && now_ms >= self.monkey_until_ms { self.monkey_level = 0; }
        if self.huyt_sao_pct != 0 && now_ms >= self.huyt_sao_until_ms { self.huyt_sao_pct = 0; }
        if let Some(m) = &self.mob_me && !m.infinite && now_ms >= m.expires_at_ms { self.mob_me = None; }

        let Some((_, c_hp_max, pt, kind_binh, _, until, last_dot)) = self.mafuba_jar else { return 0; };
        if now_ms >= until { self.mafuba_jar = None; return 0; }
        if now_ms.saturating_sub(last_dot) >= 500 {
            let mut sub = (i64::from(c_hp_max) * i64::from(pt) * if kind_binh == 0 { 1 } else { 2 } / 100) as i32;
            if sub >= current_hp { sub = (current_hp - 100).abs(); }
            if let Some(jar) = &mut self.mafuba_jar { jar.6 = now_ms; }
            return sub.max(0);
        }
        0
    }

    /// Strips non-persistent effects upon death (`EffectSkill.removeSkillEffectWhenDie`).
    pub fn clear_on_death(&mut self) {
        (self.cc, self.shield_until_ms, self.monkey_level, self.aux_until_ms.1) = (CcFlags::empty(), 0, 0, 0);
        (self.mafuba_jar, self.charging, self.stage_prep, self.mob_me) = (None, (false, 0), (None, None, None), None);
    }
}

/// Summoned `MobMe` egg familiar (`MobMe.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobMeFamiliar {
    /// Monster template ID (`[8, 11, 32, 25, 43, 49, 50]`).
    pub temp_id: u16,
    /// Current HP (`injured` caps incoming hits at `max_hp / 20`).
    pub hp: i32,
    /// Maximum HP (`30%..=90%` of owner `hp_max`).
    pub max_hp: i32,
    /// Familiar attack damage (`30%..=90%` of owner `getDameAttack`, `4x` with 5pc Pikkoro Daimao).
    pub dame: i32,
    /// Expiry timestamp (`ms`).
    pub expires_at_ms: u64,
    /// `true` when 5pc Pikkoro Daimao grants permanent survival & bypasses `5%` HP PvP safety floor.
    pub infinite: bool,
}
impl MobMeFamiliar {
    /// Spawns a new `MobMe` for `point` (`1..=7`), owner `hp_max`, and `owner_dame_de_trung`.
    pub fn spawn(point: u8, owner_hp_max: i32, owner_dame_de_trung: i32, pikkoro_5pc: bool, now_ms: u64) -> Self {
        const TEMPS: [u16; 7] = [8, 11, 32, 25, 43, 49, 50];
        let idx = usize::from(point.clamp(1, 7) - 1);
        let pct = 30 + (idx as i64) * 10;
        let max_hp = ((i64::from(owner_hp_max) * pct / 100).clamp(1, i32::MAX as i64)) as i32;
        let dame = ((i64::from(owner_dame_de_trung) * pct / 100).clamp(1, i32::MAX as i64)) as i32;
        Self { temp_id: TEMPS[idx], hp: max_hp, max_hp, dame, expires_at_ms: now_ms + monkey_duration_ms(point, false) * 2, infinite: pikkoro_5pc }
    }

    /// Applies capped damage (`min(raw, max_hp / 20)`) to `MobMe`, returning `true` if killed.
    pub fn injure(&mut self, raw_damage: i32) -> bool {
        self.hp = self.hp.saturating_sub(raw_damage.max(0).min((self.max_hp / 20).max(1)));
        self.hp == 0
    }
}

/// Tier-5 Special Skill state machine (`NewSkill.java` + `SkillService.updateSkillSpecial`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpecialSkillState {
    /// Active special skill (`SuperKame`, `LienHoanChuong`, or `MaPhongBa`).
    pub active_skill: Option<SkillKind>,
    /// Step (`0` = `2,000` ms charge, `1` = `2,000` ms active beam).
    pub step: u8,
    /// Facing direction (`-1` left, `1` right).
    pub dir: i8,
    /// Item modifier (`0` normal `11s`, `2` electric rice cooker `22s`).
    pub type_item: u8,
    /// Hit box `(reach_x, reach_y)`.
    pub reach: (i16, i16),
    /// Phase start timestamp (`ms`).
    pub phase_start_ms: u64,
}

/// Outcome emitted when advancing [`SpecialSkillState::tick_250ms`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecialTickEvent {
    /// Still charging (`step == 0`).
    Charging,
    /// `SuperKame` / `LienHoanChuong` beam tick: `(dir, reach_x, reach_y)`.
    BeamDamageTick(i8, i16, i16),
    /// `MaPhongBa` capture: `(radius_px, seal_duration_ms, caster_exhausted_ko)`.
    MafubaCapture(i32, u64, bool),
    /// Special skill completed and closed.
    Finished,
}
impl SpecialSkillState {
    /// Starts a Tier-5 special skill (`NewSkill.setSkillSpecial`).
    pub fn start(&mut self, slot: &mut SkillSlot, dir: i8, target_dx: i16, target_y: i16, use_ncd: bool, is_human: bool, now_ms: u64) {
        if is_human && slot.curr_level < 1000 { slot.curr_level += 1; }
        let max_dx = i32::from(slot.point) + 400;
        let rx = if slot.kind == SkillKind::MaPhongBa { 75 } else if max_dx < i32::from(target_dx.abs()) || target_dx.abs() < 100 { max_dx as i16 } else { target_dx.abs() };
        *self = Self { active_skill: Some(slot.kind), step: 0, dir: if dir < 0 { -1 } else { 1 }, type_item: if use_ncd { 2 } else { 0 }, reach: (rx, target_y.abs()), phase_start_ms: now_ms };
    }

    /// Advances the `250ms` special skill timer (`SkillService.updateSkillSpecial`), fixing Java's `=` bug.
    pub fn tick_250ms(&mut self, is_human: bool, now_ms: u64, rng: &mut GameRng) -> SpecialTickEvent {
        let Some(kind) = self.active_skill else { return SpecialTickEvent::Finished; };
        let elapsed = now_ms.saturating_sub(self.phase_start_ms);
        if kind == SkillKind::MaPhongBa {
            return if elapsed >= SPECIAL_GONG_MS {
                let dur = MAFUBA_BASE_SEAL_MS * if self.type_item == 0 { 1 } else { 2 };
                self.active_skill = None;
                SpecialTickEvent::MafubaCapture(500, dur, is_human && rng_ratio(rng, 1, 50))
            } else { SpecialTickEvent::Charging };
        }
        if self.step == 0 {
            if elapsed >= SPECIAL_GONG_MS {
                (self.step, self.phase_start_ms) = (1, now_ms);
                SpecialTickEvent::BeamDamageTick(self.dir, self.reach.0, self.reach.1)
            } else { SpecialTickEvent::Charging }
        } else if elapsed < SPECIAL_GONG_MS {
            SpecialTickEvent::BeamDamageTick(self.dir, self.reach.0, self.reach.1)
        } else {
            self.active_skill = None;
            SpecialTickEvent::Finished
        }
    }
}

/// Validates Kaioken HP cost, deducts MP/HP, and records post-cast intrinsic CDR & `dame_after`.
pub fn commit_skill_cast(npoint: &mut NPoint, slot: &mut SkillSlot, sets: &SetClothes, intrinsic: (u8, i32), now_ms: u64, parity: ParityMode) -> Result<u32, PlayerError> {
    let _ = consts::PER100;
    if slot.kind == SkillKind::Kaioken {
        let gate_pct = if matches!(parity, ParityMode::Fixed) { sets.kaioken_hp_cost_pct() } else { 10 };
        let (gate_hp, actual_hp) = ((i64::from(npoint.hp_max) * gate_pct / 100) as i32, (i64::from(npoint.hp_max) * sets.kaioken_hp_cost_pct() / 100) as i32);
        if npoint.hp <= gate_hp || npoint.hp <= actual_hp { return Err(PlayerError::InsufficientHpForKaioken); }
        npoint.sub_hp(i64::from(actual_hp));
    }
    let mp_cost = slot.mana_cost(npoint.mp, npoint.mp_max).ok_or(PlayerError::InsufficientMana)?;
    npoint.add_mp(-i64::from(mp_cost));
    if matches!((slot.kind, intrinsic.0), (SkillKind::DichChuyenTucThoi, 6) | (SkillKind::ThoiMien, 7) | (SkillKind::Socola, 14) | (SkillKind::Troi, 22)) {
        npoint.dame_after = intrinsic.1;
    }
    let mut cdr = if matches!((slot.kind, intrinsic.0), (SkillKind::TriThuong, 10) | (SkillKind::ThaiDuongHaSan, 3) | (SkillKind::QuaCauKenhKhi, 4) | (SkillKind::KhienNangLuong, 5 | 15 | 20) | (SkillKind::Makankosappo, 11) | (SkillKind::DeTrung, 12) | (SkillKind::TuSat, 19) | (SkillKind::HuytSao, 21) | (SkillKind::Masenko, 9)) {
        intrinsic.1.max(0) as u32
    } else { 0 };
    if slot.kind == SkillKind::Masenko { cdr += if sets.nail >= 5 { 50 } else if sets.nail == 4 { 20 } else { 0 }; }
    let sub_ms = (u64::from(slot.cool_down_ms) * u64::from(cdr.min(95))) / 100;
    slot.last_used_ms = now_ms.saturating_sub(1 + sub_ms);
    Ok(slot.cool_down_ms.saturating_sub(sub_ms as u32))
}

/// Applies `Huýt Sáo` to a same-flag humanoid (`+(point+3)*10%` HP for Earth/Saiyan; `-10% hp_max` non-lethal for Namek).
pub fn apply_huyt_sao_target(target_npoint: &mut NPoint, target_eff: &mut EffectSkillState, target_gender: Gender, point: u8, now_ms: u64) {
    if gender_index(target_gender) == 1 {
        let pen = (i64::from(target_npoint.hp_max) * 10 / 100) as i32;
        if pen < target_npoint.hp { target_npoint.sub_hp(i64::from(pen)); }
    } else {
        let pct = (i32::from(point) + 3) * 10;
        (target_eff.huyt_sao_pct, target_eff.huyt_sao_until_ms) = (pct, now_ms + 30_000);
        target_npoint.hp = (i64::from(target_npoint.hp) + i64::from(target_npoint.hp) * i64::from(pct) / 100).min(i32::MAX as i64) as i32;
    }
}

/// Applies `Trị Thương` heal/revive (`(point + 9) * 5%` of target `hp_max` & `mp_max`).
pub fn apply_tri_thuong_target(target: &mut NPoint, point: u8) {
    let pct = (i64::from(point) + 9) * 5;
    if target.hp <= 0 { target.hp = 1; }
    target.add_hp(i64::from(target.hp_max) * pct / 100);
    target.add_mp(i64::from(target.mp_max) * pct / 100);
}

/// Computes `Tự Sát` explosion damage against `targets_are_boss`, honoring [`ParityMode`] for loop mutation.
pub fn compute_tu_sat_damages(caster_hp: i32, caster_hp_max: i32, is_monkey: bool, sets: &SetClothes, targets_are_boss: &[bool], parity: ParityMode) -> SmallVec<[i32; 8]> {
    let base = (i64::from(caster_hp) + i64::from(caster_hp_max) * sets.tu_sat_bonus_hp_max_pct() / 100).clamp(0, i32::MAX as i64);
    let mut running = base;
    targets_are_boss.iter().map(|&boss| {
        let cur = if matches!(parity, ParityMode::Fixed) { base } else { running };
        let hit = if boss { if is_monkey { cur / 3 } else { cur / 2 } } else { cur };
        if !matches!(parity, ParityMode::Fixed) { running = hit; }
        hit as i32
    }).collect()
}

/// Monkey duration (`(point + 5) * 10_000` ms, `* 5` with 5pc Cadic).
#[inline] pub const fn monkey_duration_ms(point: u8, cadic_5pc: bool) -> u64 { (point as u64 + 5) * 10_000 * if cadic_5pc { 5 } else { 1 } }
/// Solar flare (`TDHS`) stun duration (`(point + 2) * 1_000` ms, `* 2` with 5pc Thiên Xin Hăng) and range (`120 + 30 * point`).
#[inline] pub const fn tdhs_stun_ms_and_range(point: u8, txh_5pc: bool) -> (u64, i32) { ((point as u64 + 2) * 1_000 * if txh_5pc { 2 } else { 1 }, 120 + (point as i32) * 30) }
/// Energy shield (`Khiên Năng Lượng`) duration (`(point + 2) * 5_000` ms).
#[inline] pub const fn shield_duration_ms(point: u8) -> u64 { (point as u64 + 2) * 5_000 }
/// `(dctt_blind_ms, thoi_mien_sleep_ms, troi_hold_ms)` for `point` (`1..=7`).
#[inline] pub const fn cc_durations_ms(point: u8, is_baby_or_robber_boss: bool) -> (u64, u64, u64) { ((point as u64 + 1) * 500, (point as u64 + 4) * 1_000, if is_baby_or_robber_boss { 5_000 } else { point as u64 * 5_000 }) }
/// `(tri_thuong_pct, ttnl_tick_pct, qckk_range_px, tu_sat_range_px)` for `point` (`1..=7`).
#[inline] pub const fn utility_skill_params(point: u8) -> (i32, i32, i32, i32) { let p = point as i32; ((p + 9) * 5, p + 3, 350 + p * 30, 400 + p * 30) }
