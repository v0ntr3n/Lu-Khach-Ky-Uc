//! Safe, deterministic player domain crate (`crates/nro-player`) for Teamobi NRO.
//!
//! Integrates `Player`, `PlayerInventory`, `NPoint`, `SetClothes`, `EffectSkillState`,
//! `SpecialSkillState`, `Pet`, `NewPet`, `FusionState`, `TaskPlayer`, `Clan`, and `BotManager`
//! with `#![forbid(unsafe_code)]` and [`ParityMode`] toggles for legacy Java quirks.

#![forbid(unsafe_code)]

/// Server-side AI Bots (`BotManager`, `BotPemQuai`, `BotSanBoss`, `BotGiaoDich`, `BotAttackPlayer`).
pub mod bot_ai;
/// Damage mitigation (`apply_injured`), `TNSM` potential scaling, and stat upgrades (`increase_point`).
pub mod combat;
/// Complete `NPoint` stat calculator (`cal_point`), outgoing damage (`get_dame_attack`), and `sub_hp`.
pub mod npoint;
/// Disciple (`Pet`, `Mabu`, `Uub`, `KidBeer`, `KidJiren`), `NewPet`, and `Fusion` state machines.
pub mod pet;
/// Tasks, Side/Clan quests, Badges, Radar cards, Charms, Intrinsics, KOL quests, and Clans.
pub mod progression;
/// Planetary 5-piece SKH sets, Seal sets (`Ấn`), Tier sets (`Thần Linh`/`Hủy Diệt`/`Thiên Sứ`), and auras.
pub mod set_clothes;
/// Active, passive, and Tier-5 special skills (`SkillService`, `EffectSkillService`, `NewSkill`).
pub mod skills;

#[cfg(test)]
mod tests;

use nro_consts as consts;
use nro_core::{
    AccountId, ClanId, CoreStats, GameRng, Gender, ItemInstance, ItemOption, Location, MapId,
    ParityMode, PlayerId, TemplateId,
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use thiserror::Error;

use crate::npoint::{
    ActiveBuffs, BuffFlags, LIMIT_TIERS, NPoint, PassiveOptionFlags, StatContext, TnsmContext,
    cal_sub_tnsm, power_caption_level,
};
use crate::pet::{FusionKind, FusionState, NewPet, Pet, PetStatus};
use crate::progression::{BadgeEntry, Charms, IntrinsicState, KolProgressData, RadarCard, TaskPlayer};
use crate::set_clothes::{SetClothes, TRAIN_ARMOR_SLOT, train_armor_percent};
use crate::skills::{EffectSkillState, SpecialSkillState};

/// Maximum player gold (`Inventory.LIMIT_GOLD = 200_000_000_000`).
pub const LIMIT_GOLD: u64 = 200_000_000_000;
/// Maximum bag slots (`Inventory.MAX_ITEMS_BAG = 80`).
pub const MAX_ITEMS_BAG: usize = 80;
/// Maximum storage box slots (`Inventory.MAX_ITEMS_BOX = 100`).
pub const MAX_ITEMS_BOX: usize = 100;
/// Maximum `OpenPowerService.MAX_LIMIT` tier (`9`).
pub const MAX_OPEN_POWER_LIMIT: u8 = 9;
/// Gold cost to unlock the next `limit_power` tier (`OpenPowerService.COST_SPEED_OPEN_LIMIT_POWER = 500_000_000`).
pub const OPEN_POWER_GOLD_COST: u64 = 500_000_000;

/// Domain errors returned across `nro-player` operations.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PlayerError {
    /// Stat upgrade point count was outside `1..=1000`.
    #[error("invalid stat upgrade point count: {0}")]
    InvalidUpgradeAmount(u16),
    /// Current `limit_power` cap or power requirement was not met.
    #[error("power or stat tier limit reached")]
    PowerLimitReached,
    /// Insufficient `tiem_nang` for stat upgrade.
    #[error("insufficient potential: need {required}, have {available}")]
    InsufficientPotential {
        /// Required `tiem_nang` cost.
        required: u64,
        /// Available `tiem_nang` balance.
        available: u64,
    },
    /// Insufficient MP/KI to cast skill.
    #[error("insufficient mana to cast skill")]
    InsufficientMana,
    /// Insufficient HP to sacrifice for Kaioken.
    #[error("insufficient HP for Kaioken")]
    InsufficientHpForKaioken,
    /// Insufficient gold or gems.
    #[error("insufficient currency")]
    InsufficientCurrency,
    /// Fusion is still on its `5,000` ms unfusion cooldown.
    #[error("fusion cooldown active")]
    FusionCooldownActive,
    /// Action blocked by death, CC, or state invariant.
    #[error("player or pet action blocked")]
    ActionBlocked,
    /// Clan permission, task gate, or capacity check failed.
    #[error("clan action denied")]
    ClanActionDenied,
}

/// Player inventory & currencies (`nro.models.player.Inventory`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerInventory {
    /// Equipped body slots (`0..12`).
    pub items_body: SmallVec<[Option<ItemInstance>; 12]>,
    /// Bag items (`<= 80`).
    pub items_bag: Vec<Option<ItemInstance>>,
    /// Storage box items (`<= 100`, used for Clan Pea donations).
    pub items_box: Vec<Option<ItemInstance>>,
    /// Cached unequipped training armor (`Inventory.trainArmor`).
    pub train_armor: Option<ItemInstance>,
    /// Gold (`<= 200B`).
    pub gold: u64,
    /// Green gems (`ngọc xanh`).
    pub gem: u32,
    /// Pink rubies (`hồng ngọc`).
    pub ruby: u32,
    /// Shop coupons.
    pub coupon: u32,
}

impl PlayerInventory {
    /// Adds `amount` gold clamped to [`LIMIT_GOLD`] (`200_000_000_000`).
    #[inline]
    pub fn add_gold(&mut self, amount: u64) {
        self.gold = self.gold.saturating_add(amount).min(LIMIT_GOLD);
    }

    /// Counts distinct permanent dragon pets (`1765..=1771` without expiry option `93`) (`checkAndUpdateMeRongBadges`).
    pub fn permanent_dragon_pet_count(&self) -> u8 {
        let mut mask = 0_u8;
        for item in self.items_body.iter().chain(&self.items_bag).chain(&self.items_box).flatten() {
            let id = tpl_raw(item.template_id);
            if (1765..=1771).contains(&id) && !item.options.iter().any(|o| o.id == 93) {
                mask |= 1 << (id - 1765);
            }
        }
        mask.count_ones() as u8
    }
}

/// Aggregate root for a connected human player (`nro.models.player.Player`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    /// Unique player identifier.
    pub id: PlayerId,
    /// Owning account identifier.
    pub account_id: AccountId,
    /// Current guild membership.
    pub clan_id: Option<ClanId>,
    /// Planet class (`TraiDat`, `Namec`, `Xayda`).
    pub gender: Gender,
    /// PK state (`0` NonPK, `3` PVP, `5` KillAll).
    pub type_pk: u8,
    /// Selected flag ID (`8` = Black Flag `+10%` TNSM).
    pub c_flag: u8,
    /// Spatial location.
    pub location: Location,
    /// Stat & combat calculator.
    pub npoint: NPoint,
    /// Active equipment set bonuses.
    pub sets: SetClothes,
    /// Currencies & items.
    pub inventory: PlayerInventory,
    /// Consumable & Black Ball buffs.
    pub buffs: ActiveBuffs,
    /// Crowd-control & skill timers.
    pub effects: EffectSkillState,
    /// Tier-5 special skill channel state.
    pub special_skill: SpecialSkillState,
    /// Optional Disciple (`Pet`).
    pub pet: Option<Pet>,
    /// Optional slot-7 `NewPet`.
    pub new_pet: Option<NewPet>,
    /// Fusion state.
    pub fusion: FusionState,
    /// Quest progression.
    pub tasks: TaskPlayer,
    /// Bà Hạt Mít charms.
    pub charms: Charms,
    /// Passive intrinsic.
    pub intrinsic: IntrinsicState,
    /// Radar card deck.
    pub radar_cards: Vec<RadarCard>,
    /// Owned title badges.
    pub badges: Vec<BadgeEntry>,
    /// KOL & skill-book study progress.
    pub kol: KolProgressData,
    /// Revival timestamp (`1,500` ms invulnerability).
    pub last_revived_ms: u64,
    /// Tracks `(has_reduced_future_power, original_power)` for `Player.isDie()`.
    pub future_power_guard: (bool, Option<i64>),
}

impl Player {
    /// Constructs a new `Player` with `12` empty body slots and `(1, 1)` default war multipliers.
    pub fn new(id: PlayerId, account_id: AccountId, gender: Gender, core: CoreStats, location: Location) -> Self {
        let _ = consts::PER100;
        let mut inv = PlayerInventory::default();
        inv.items_body.resize(12, None);
        let buffs = ActiveBuffs { war_mults: (1, 1), ..Default::default() };
        Self {
            id, account_id, clan_id: None, gender, type_pk: 0, c_flag: 0, location,
            npoint: NPoint::new(core), sets: SetClothes::default(), inventory: inv, buffs,
            effects: EffectSkillState::default(), special_skill: SpecialSkillState::default(),
            pet: None, new_pet: None, fusion: FusionState::default(), tasks: TaskPlayer::default(),
            charms: Charms::default(), intrinsic: IntrinsicState::default(), radar_cards: Vec::new(),
            badges: Vec::new(), kol: KolProgressData::default(), last_revived_ms: 0, future_power_guard: (false, None),
        }
    }

    /// Recomputes `Pet` stats first (when present) and then `Player` stats (`NPoint.calPoint`).
    pub fn recalculate_stats(&mut self, in_cold_map: bool, in_black_ball_war: bool, now_ms: u64, parity: ParityMode) {
        let pet_body_ref = self.pet.as_ref().map(|p| p.items_body.as_slice());
        self.sets = SetClothes::recompute(&self.inventory.items_body, pet_body_ref, false);

        if let Some(pet) = &mut self.pet {
            let p_sets = SetClothes::recompute(&pet.items_body, None, false);
            let p_buffs = ActiveBuffs { flags: self.buffs.flags & BuffFlags::EAT_MEAL, war_mults: (1, 1), ..Default::default() };
            let p_ctx = StatContext {
                body: &pet.items_body, radar_options: &[], badge_options: &[], bag_gear: (None, None),
                sets: p_sets, buffs: p_buffs, is_human_player: false,
                self_pet_info: Some((pet.kind, self.fusion.kind, pet.status == PetStatus::Fusion)),
                fused_pet_stats: None, form_state: (pet.effects.monkey_level, pet.effects.huyt_sao_pct, None),
                map_env: (in_cold_map, in_black_ball_war), parity,
            };
            pet.npoint.cal_point(&p_ctx);
        }

        if let Some(Some(gtl)) = self.inventory.items_body.get(TRAIN_ARMOR_SLOT) {
            self.inventory.train_armor = Some(gtl.clone());
        }
        let radar_opts: SmallVec<[ItemOption; 4]> = self.radar_cards.iter().find(|c| c.used).map_or_else(SmallVec::new, RadarCard::active_options);
        let badge_opts: Vec<ItemOption> = self.badges.iter().filter(|b| b.is_use && b.expires_at_ms > now_ms).flat_map(|b| b.options.iter().copied()).collect();
        let eid = match self.fusion.kind { FusionKind::Porata2 => Some(921), FusionKind::Porata3 => Some(1819), _ => None };
        let earring = eid.and_then(|id| self.inventory.items_bag.iter().flatten().find(|it| tpl_raw(it.template_id) == id));
        let fused_stats = (self.fusion.kind != FusionKind::None).then(|| self.pet.as_ref().map(|p| (p.npoint.hp_max, p.npoint.mp_max, p.npoint.dame))).flatten();
        let ctx = StatContext {
            body: &self.inventory.items_body, radar_options: &radar_opts, badge_options: &badge_opts,
            bag_gear: (earring, self.inventory.train_armor.as_ref()), sets: self.sets, buffs: self.buffs,
            is_human_player: true, self_pet_info: None, fused_pet_stats: fused_stats,
            form_state: (self.effects.monkey_level, self.effects.huyt_sao_pct, None),
            map_env: (in_cold_map, in_black_ball_war), parity,
        };
        self.npoint.cal_point(&ctx);
    }

    /// Processes death penalties (`Player.isDie` + `Player.setDie`), returning `(dropped_gold_item_id, dropped_gold_amount)`.
    pub fn on_death(&mut self, in_future_map: bool, in_cold_map: bool, rng: &mut GameRng) -> Option<(u16, u64)> {
        if in_future_map && !self.future_power_guard.0 {
            let orig = *self.future_power_guard.1.get_or_insert(self.npoint.core.power);
            self.npoint.core.power = (orig as f64 * 0.99) as i64;
            self.future_power_guard.0 = true;
        }
        if in_future_map || in_cold_map {
            let loss = ((self.npoint.core.power as f64 * 0.001) as i64).max(1);
            if self.npoint.core.power >= loss {
                self.npoint.core.power -= loss;
            }
        }
        let mut gold_drop = None;
        let raw_vang = (self.npoint.core.power / 1_000_000).min(32_000) - rng_range(rng, 10, 100);
        if raw_vang >= 1 && self.inventory.gold >= raw_vang as u64 {
            self.inventory.gold -= raw_vang as u64;
            let dropped = (raw_vang as u64) * 95 / 100;
            gold_drop = Some((if dropped < 10_000 { 189 } else if dropped < 20_000 { 188 } else { 190 }, dropped));
        }
        self.buffs.war_mults = (1, 1);
        self.effects.clear_on_death();
        (self.npoint.hp, self.npoint.mp, self.type_pk) = (0, 0, 0);
        gold_drop
    }

    /// Unlocks the next `limit_power` tier up to `9` (`OpenPowerService.openPowerFast`).
    pub fn open_power_limit(&mut self) -> Result<u8, PlayerError> {
        let cur = self.npoint.core.limit_power;
        if cur >= MAX_OPEN_POWER_LIMIT || self.npoint.core.power < LIMIT_TIERS[usize::from(cur)].0 {
            return Err(PlayerError::PowerLimitReached);
        }
        if self.inventory.gold < OPEN_POWER_GOLD_COST { return Err(PlayerError::InsufficientCurrency); }
        self.inventory.gold -= OPEN_POWER_GOLD_COST;
        self.npoint.core.limit_power += 1;
        Ok(self.npoint.core.limit_power)
    }

    /// Distributes Power/Potential (`Service.addSMTN`), honoring [`ParityMode`] for Pet -> Master double-grant.
    pub fn add_smtn(&mut self, from_pet: bool, smtn_type: u8, amount: u64, parity: ParityMode) -> u64 {
        if from_pet && let Some(pet) = &mut self.pet {
            pet.npoint.power_up(amount);
            pet.npoint.tiem_nang_up(amount);
            let half = cal_sub_tnsm(self.npoint.core.power, amount / 2);
            return self.apply_master_smtn(smtn_type, half.saturating_mul(if matches!(parity, ParityMode::Fixed) { 1 } else { 2 }));
        }
        self.apply_master_smtn(smtn_type, amount)
    }

    fn apply_master_smtn(&mut self, smtn_type: u8, amount: u64) -> u64 {
        let (cur, lim) = (self.npoint.core.power, self.npoint.power_limit());
        if cur >= lim { return 0; }
        let grant = amount.min((lim - cur) as u64);
        if grant == 0 { return 0; }
        match smtn_type {
            1 => self.npoint.tiem_nang_up(grant),
            2 => { self.npoint.power_up(grant); self.npoint.tiem_nang_up(grant); }
            _ => self.npoint.power_up(grant),
        }
        grant
    }

    /// Updates Training Armor option `9` (`+1` min worn in combat up to `1000`; `-1` min/60s in bag/box).
    pub fn tick_train_armor(&mut self, attacked_within_30s: bool) {
        if self.npoint.flags.contains(PassiveOptionFlags::WEARING_TRAIN_ARMOR) && attacked_within_30s
            && let Some(Some(gtl)) = self.inventory.items_body.get_mut(TRAIN_ARMOR_SLOT)
            && let Some(opt) = gtl.options.iter_mut().find(|o| o.id == 9 && o.param < 1000)
        {
            opt.param += 1;
        }
        for item in self.inventory.items_bag.iter_mut().chain(&mut self.inventory.items_box).flatten() {
            if train_armor_percent(item.template_id) > 0 && let Some(opt) = item.options.iter_mut().find(|o| o.id == 9 && o.param > 0) {
                opt.param -= 1;
            }
        }
        let mut clear_cache = false;
        if let Some(cached) = &mut self.inventory.train_armor {
            if let Some(gtl) = self.inventory.items_body.get(TRAIN_ARMOR_SLOT).and_then(Option::as_ref).filter(|it| it.template_id == cached.template_id) {
                *cached = gtl.clone();
            } else if self.inventory.items_bag.iter().flatten().any(|it| it.template_id == cached.template_id) {
                if let Some(opt) = cached.options.iter_mut().find(|o| o.id == 9 && o.param > 0) {
                    opt.param -= 1;
                }
                clear_cache = cached.options.iter().find(|o| o.id == 9).is_none_or(|o| o.param <= 0);
            } else {
                clear_cache = true;
            }
        }
        if clear_cache { self.inventory.train_armor = None; }
    }

    /// Returns the player's current caption level (`0..=19`).
    #[inline] pub fn caption_level(&self) -> u8 { power_caption_level(self.npoint.core.power) }
}

/// Computes Mob potential reward (`Mob.getTiemNangForPlayer`).
pub fn mob_tiem_nang_for_player(player: &Player, mob_level: u8, mob_temp_id: u16, mob_full_hp: i64, damage: i64, tnsm_ctx: &TnsmContext) -> u64 {
    let p_lvl = player.caption_level();
    let diff = p_lvl.abs_diff(mob_level);
    let base = if mob_temp_id == 0 || (diff > 5 && p_lvl > mob_level) {
        1_u64
    } else {
        let raw = (damage.max(0) as f64) + (mob_full_hp.max(0) as f64) * 0.0005;
        (raw / (f64::from((f64::from(diff) * 0.5) as i32) + 1.25)).max(1.0) as u64
    };
    let out = player.npoint.cal_suc_manh_tiem_nang(base, tnsm_ctx);
    if matches!(tnsm_ctx.parity, ParityMode::Fixed) { out } else { (out as i32).max(1) as u64 }
}

#[inline] pub(crate) const fn tpl_raw(id: TemplateId) -> u16 { id.0 as u16 }
#[inline] pub(crate) const fn map_raw(id: MapId) -> u16 { id.0 as u16 }
#[inline] pub(crate) const fn gender_index(g: Gender) -> u8 { match g { Gender::TraiDat => 0, Gender::Namec => 1, Gender::Xayda => 2 } }
#[inline] pub(crate) fn distance_2d(a: &Location, b: &Location) -> i32 { f64::hypot(f64::from(a.x) - f64::from(b.x), f64::from(a.y) - f64::from(b.y)) as i32 }
#[inline] pub(crate) fn rng_range(rng: &mut GameRng, min: i64, max: i64) -> i64 { if min >= max { min } else { rng.next_i64(min, max) } }
#[inline] pub(crate) fn rng_ratio(rng: &mut GameRng, num: u32, denom: u32) -> bool { rng.chance(num, denom) }
#[inline] pub(crate) fn rng_one_sign(rng: &mut GameRng) -> i64 { if rng.chance(1, 2) { 1 } else { -1 } }
