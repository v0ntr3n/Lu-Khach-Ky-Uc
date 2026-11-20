//! Server-side AI Bots (`BotManager`, `Bot`, `NewBot`, `BotPemQuai`, `BotSanBoss`, `BotGiaoDich`, `BotAttackPlayer`).
//!
//! Ports all 7 classes from `nro.models.Bot` onto a deterministic `150ms` tick scheduler with
//! zero threads or blocking sleeps, preserving exact spawn profiles, map pools, trade ratios,
//! item magnet (`tdThuHut`), boss-hunting retries, and PvP chase mechanics.

use nro_consts as consts;
use nro_core::{CoreStats, GameRng, Gender, ItemInstance, Location, MapId, ParityMode, PlayerId, TemplateId, ZoneId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::npoint::{NPoint, StatUpgradeKind};
use crate::skills::{ManaUseType, SkillKind, SkillSlot};
use crate::{distance_2d, gender_index, rng_range, rng_ratio, tpl_raw};

/// Global `BotManager` update interval (`150` ms in `BotManager.run`).
pub const BOT_MANAGER_TICK_MS: u64 = 150;
/// `BotAttackplayer` fixed attack cadence (`500` ms).
pub const PVP_BOT_COOLDOWN_MS: u64 = 500;
/// Market map ID (`Siêu Thị = 84`) used by `BotGiaoDich`.
pub const MARKET_MAP_ID: u16 = 84;

/// Earth farming map pool (`Bot.TraiDat`).
pub const TRAI_DAT_BOT_MAPS: [u16; 47] = [1, 2, 3, 4, 6, 29, 30, 28, 27, 42, 110, 103, 106, 105, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 92, 93, 94, 95, 96, 97, 98, 99, 100, 107, 108, 109];
/// Namek farming map pool (`Bot.Namec`).
pub const NAMEC_BOT_MAPS: [u16; 47] = [8, 9, 10, 11, 12, 13, 33, 34, 32, 31, 110, 103, 106, 105, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 92, 93, 94, 95, 96, 97, 98, 99, 100, 107, 108, 109];
/// Saiyan farming map pool (`Bot.XayDa`).
pub const XAYDA_BOT_MAPS: [u16; 47] = [15, 16, 17, 18, 19, 20, 37, 36, 35, 44, 52, 110, 106, 105, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 92, 93, 94, 95, 96, 97, 98, 99, 100, 107, 108, 109];

/// Server-side AI bot archetypes (`NewBot.runBot(type, ...)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum BotArchetype {
    /// `type = 0`: Mob grinder & ground-item looter (`BotPemQuai`).
    MobGrinder = 0,
    /// `type = 1`: Map `84` automated barter merchant (`BotGiaoDich`).
    MarketTrader = 1,
    /// `type = 2`: World boss hunter (`BotSanBoss`).
    BossHunter = 2,
    /// `type = 3`: Cross-map PvP hunter (`BotAttackplayer`).
    PvpHunter = 3,
}

/// Trade exchange rule for `BotGiaoDich` (`item`, `traodoi`, `slot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotTradeConfig {
    /// Item template offered by the bot (`idItem`).
    pub offer_item_id: TemplateId,
    /// Currency/material item template required from player (`idItTd`).
    pub required_item_id: TemplateId,
    /// Required units of `required_item_id` per `1` `offer_item_id` (`slot`).
    pub rate_per_unit: u32,
}

impl BotTradeConfig {
    /// Evaluates a player's trade items (`BotGiaoDich.CheckTraDe` & `active`), returning `(offer_item_id, qty)`.
    pub fn evaluate_offer(&self, player_items: &[ItemInstance]) -> Option<(TemplateId, u32)> {
        if self.rate_per_unit == 0 { return None; }
        let offered = player_items
            .iter()
            .find(|it| tpl_raw(it.template_id) == tpl_raw(self.required_item_id) && it.quantity >= self.rate_per_unit)
            .map(|it| it.quantity)?;
        Some((self.offer_item_id, ((f64::from(offered) / f64::from(self.rate_per_unit)).round() as u32).max(1)))
    }
}

/// Snapshot of world entities in the bot's zone/server passed to [`BotAgent::tick`].
pub struct BotWorldView<'a> {
    /// Living mobs in current zone: `(mob_id, location)`.
    pub zone_mobs: &'a [(u32, Location)],
    /// Unpicked ground items in current zone: `(item_map_id, location)`.
    pub zone_items: &'a [(u32, Location)],
    /// Living human players in current map: `(player_id, location)`.
    pub map_players: &'a [(PlayerId, Location)],
    /// Active world bosses: `(boss_id, location, in_restricted_dungeon, zone_full, has_mobs)`.
    pub active_bosses: &'a [(PlayerId, Location, bool, bool, bool)],
}

/// Deterministic command emitted by [`BotAgent::tick`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotDirective {
    /// No action this tick.
    Idle,
    /// Revive bot to full HP/MP (`Bot.update`: `hsChar`).
    ReviveFull,
    /// Despawn and remove from `BotManager` (after 20 failed zone searches or 4 failed boss searches).
    Despawn,
    /// Warp to `(map_id, x, y)`.
    ChangeMap {
        /// Destination map ID.
        map_id: MapId,
        /// Destination X coordinate.
        x: i16,
        /// Destination Y coordinate.
        y: i16,
    },
    /// Move to `move_to` and execute `skill` against mob or player/boss.
    UseSkill {
        /// Selected skill.
        skill: SkillKind,
        /// Targeted zone mob ID, if attacking a mob.
        target_mob: Option<u32>,
        /// Targeted player or boss ID, if attacking a humanoid.
        target_player: Option<PlayerId>,
        /// Target `(x, y)` coordinates moved to prior to attack.
        move_to: (i16, i16),
    },
    /// Pick up up to `3` ground `item_map_id`s within `500` px (`BotPemQuai.NhatItem`).
    PickItems(SmallVec<[u32; 3]>),
    /// Broadcast local and/or global trade advertisement (`BotGiaoDich.chat`).
    Advertise {
        /// Emit world-chat trade broadcast (`100..=199` s interval).
        global: bool,
        /// Emit zone-chat trade broadcast (`5..=9` s interval).
        local: bool,
    },
}

/// Complete server-side AI Bot (`Bot.java` + sub-controllers).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotAgent {
    /// Bot `PlayerId`.
    pub id: PlayerId,
    /// Planet class.
    pub gender: Gender,
    /// Outfit `(head, body, leg, flag)`.
    pub appearance: (i16, i16, i16, i16),
    /// AI behavior mode (`0..=3`).
    pub archetype: BotArchetype,
    /// Current spatial location.
    pub location: Location,
    /// Bot `NPoint` stats.
    pub npoint: NPoint,
    /// Learned Lv7 skills.
    pub skills: SmallVec<[SkillSlot; 24]>,
    /// Optional market trade rule (`archetype == MarketTrader`).
    pub trade_cfg: Option<BotTradeConfig>,
    /// Timestamps: `[last_map_change, last_special_skill, last_item_pick, last_global_chat, last_local_chat, last_pvp_attack]`.
    pub timers_ms: [u64; 6],
    /// Item-magnet expiry (`tdThuHut`).
    pub magnet_until_ms: u64,
    /// Consecutive failed `getRandomZone` attempts (`>= 20` despawns bot).
    pub failed_zone_attempts: u8,
    /// Current tracked target `(mob_id, player_or_boss_id)`.
    pub current_target: (Option<u32>, Option<PlayerId>),
}

impl BotAgent {
    /// Spawns a bot with exact `NewBot.runBot` / `BotAttackplayer` profiles.
    pub fn spawn(id: PlayerId, gender: Gender, appearance: (i16, i16, i16, i16), archetype: BotArchetype, trade_cfg: Option<BotTradeConfig>, now_ms: u64, rng: &mut GameRng) -> Self {
        let _ = consts::PER100;
        let (core, skills) = if archetype == BotArchetype::PvpHunter {
            const ALL_24: [SkillKind; 24] = [
                SkillKind::Dragon, SkillKind::Kamejoko, SkillKind::Demon, SkillKind::Galick, SkillKind::Masenko, SkillKind::Antomic,
                SkillKind::TriThuong, SkillKind::ThaiDuongHaSan, SkillKind::LienHoan, SkillKind::Kaioken, SkillKind::QuaCauKenhKhi, SkillKind::Makankosappo,
                SkillKind::DeTrung, SkillKind::BienKhi, SkillKind::TuSat, SkillKind::KhienNangLuong, SkillKind::Socola, SkillKind::HuytSao,
                SkillKind::Troi, SkillKind::DichChuyenTucThoi, SkillKind::ThoiMien, SkillKind::SuperKame, SkillKind::LienHoanChuong, SkillKind::MaPhongBa,
            ];
            let c = CoreStats { hpg: 500_000, mpg: 200_000, dameg: 100_000, defg: 10, critg: 10, power: 50_000_000, tiem_nang: 0, limit_power: 8, stamina: 20_000, max_stamina: 20_000, ..CoreStats::default() };
            (c, ALL_24.into_iter().map(Self::lv7_slot).collect())
        } else {
            let b = rng_range(rng, 0, 49_999_999);
            let c = CoreStats { hpg: 675_000, mpg: 600_000, dameg: 600_000, defg: 10, critg: 10, power: (1_000 + b) as _, tiem_nang: (20_000_000 + b) as _, limit_power: 8, stamina: 20_000, max_stamina: 20_000, ..CoreStats::default() };
            let cls: &[SkillKind] = match gender_index(gender) {
                0 => &[SkillKind::Dragon, SkillKind::Kamejoko, SkillKind::ThaiDuongHaSan, SkillKind::Kaioken, SkillKind::QuaCauKenhKhi, SkillKind::DichChuyenTucThoi, SkillKind::ThoiMien, SkillKind::KhienNangLuong],
                1 => &[SkillKind::Demon, SkillKind::Masenko, SkillKind::TriThuong, SkillKind::Makankosappo, SkillKind::DeTrung, SkillKind::LienHoan, SkillKind::Socola, SkillKind::KhienNangLuong],
                _ => &[SkillKind::Galick, SkillKind::Antomic, SkillKind::TaiTaoNangLuong, SkillKind::BienKhi, SkillKind::TuSat, SkillKind::HuytSao, SkillKind::Troi, SkillKind::KhienNangLuong],
            };
            (c, cls.iter().copied().map(Self::lv7_slot).collect())
        };
        let mut npoint = NPoint::new(core);
        if archetype == BotArchetype::PvpHunter {
            (npoint.dame, npoint.hp_max, npoint.hp, npoint.mp_max, npoint.mp) = (100_000, 500_000, 500_000, 200_000, 200_000);
        } else {
            (npoint.dame, npoint.mp_max, npoint.mp, npoint.hp_max, npoint.hp) = (600_000, 2_000_000_000, 600_000, 675_000, 600_000);
        }
        Self {
            id, gender, appearance, archetype, location: Location { map_id: MapId(21), zone_id: ZoneId(0), x: 200, y: 336 },
            npoint, skills, trade_cfg, timers_ms: [now_ms, 0, 0, 0, 0, 0],
            magnet_until_ms: now_ms.saturating_add(30 * 86_400_000), failed_zone_attempts: 0, current_target: (None, None),
        }
    }

    fn lv7_slot(kind: SkillKind) -> SkillSlot {
        SkillSlot { kind, skill_id: kind as i16, point: 7, cool_down_ms: 1_000, mana_use: 10, mana_type: ManaUseType::Flat, damage_pct: 200, curr_level: 7, last_used_ms: 0 }
    }

    /// Selects a farming map ID based on power (`Bot.MapToPow`), honoring [`ParityMode`] for dead `else if` branches.
    pub fn select_map_for_power(&self, parity: ParityMode, rng: &mut GameRng) -> MapId {
        let p = self.npoint.core.power;
        let m = if p < 100_000_000 {
            let pool = match gender_index(self.gender) { 0 => &TRAI_DAT_BOT_MAPS, 1 => &NAMEC_BOT_MAPS, _ => &XAYDA_BOT_MAPS };
            pool[rng_range(rng, 0, (pool.len() - 1) as i64) as usize]
        } else if matches!(parity, ParityMode::Fixed) && p < 1_000_000_000 {
            62 + rng_range(rng, 0, 14) as u16
        } else if matches!(parity, ParityMode::Fixed) && p < 10_000_000_000 {
            if rng_ratio(rng, 30, 100) { 91 + rng_range(rng, 0, 2) as u16 } else if rng_ratio(rng, 30, 100) { 95 + rng_range(rng, 0, 4) as u16 } else { 103 + rng_range(rng, 0, 1) as u16 }
        } else if rng_ratio(rng, 30, 100) { 104 + rng_range(rng, 0, 5) as u16 } else { 21 };
        MapId(m as i16)
    }

    /// Records a zone-selection attempt (`Bot.getRandomZone`), returning `false` when `>= 20` retries fail.
    pub fn record_zone_lookup(&mut self, found_zone: bool) -> bool {
        if found_zone { self.failed_zone_attempts = 0; true } else { self.failed_zone_attempts = self.failed_zone_attempts.saturating_add(1); self.failed_zone_attempts < 20 }
    }

    /// Advances 1 tick (`150` ms) of the bot's state machine.
    pub fn tick(&mut self, world: &BotWorldView<'_>, now_ms: u64, parity: ParityMode, rng: &mut GameRng) -> BotDirective {
        if self.npoint.hp <= 0 {
            (self.npoint.hp, self.npoint.mp) = (self.npoint.hp_max, self.npoint.mp_max);
            return BotDirective::ReviveFull;
        }
        if self.archetype != BotArchetype::PvpHunter { self.auto_allocate_stats(parity, rng); }
        match self.archetype {
            BotArchetype::MobGrinder => self.tick_pem_quai(world, now_ms, parity, rng),
            BotArchetype::MarketTrader => self.tick_giao_dich(now_ms, rng),
            BotArchetype::BossHunter => self.tick_san_boss(world, now_ms, rng),
            BotArchetype::PvpHunter => self.tick_attack_player(world, now_ms, rng),
        }
    }

    fn auto_allocate_stats(&mut self, parity: ParityMode, rng: &mut GameRng) {
        if rng_ratio(rng, 50, 100) {
            if matches!(parity, ParityMode::Fixed) {
                let _ = self.npoint.increase_point(StatUpgradeKind::Hp, 100, parity);
            } else {
                let cost = 100 * (2 * (self.npoint.core.hpg + 1000) + 2000 - 20) / 2;
                if self.npoint.core.tiem_nang >= cost {
                    self.npoint.core.tiem_nang -= cost;
                    self.npoint.core.hpg += 100;
                    self.npoint.hp_max = self.npoint.hp_max.saturating_add(100);
                }
            }
        } else { let _ = self.npoint.increase_point(StatUpgradeKind::Dame, 10, parity); }
    }

    fn tick_pem_quai(&mut self, world: &BotWorldView<'_>, now_ms: u64, parity: ParityMode, rng: &mut GameRng) -> BotDirective {
        if now_ms.saturating_sub(self.timers_ms[0]) > 150_000 + rng_range(rng, 0, 150_000) as u64 {
            self.timers_ms[0] = now_ms;
            return BotDirective::ChangeMap { map_id: self.select_map_for_power(parity, rng), x: 200, y: 336 };
        }
        if now_ms < self.magnet_until_ms && now_ms.saturating_sub(self.timers_ms[2]) >= 500 {
            let picks: SmallVec<[u32; 3]> = world.zone_items.iter().filter(|(_, loc)| distance_2d(&self.location, loc) < 500).take(3).map(|(id, _)| *id).collect();
            if !picks.is_empty() { self.timers_ms[2] = now_ms; return BotDirective::PickItems(picks); }
        }
        if world.zone_mobs.is_empty() || self.skills.is_empty() { return BotDirective::Idle; }
        let (mob_id, mob_loc) = world.zone_mobs[rng_range(rng, 0, (world.zone_mobs.len() - 1) as i64) as usize];
        let idx = rng_range(rng, 0, (self.skills.len() - 1) as i64) as usize;
        let kind = self.skills[idx].kind;
        if kind == SkillKind::BienKhi {
            if now_ms.saturating_sub(self.timers_ms[1]) >= 300_000 {
                self.timers_ms[1] = now_ms;
                return BotDirective::UseSkill { skill: kind, target_mob: None, target_player: None, move_to: (self.location.x, self.location.y) };
            }
            return BotDirective::Idle;
        }
        if kind.is_beam() || self.skills[idx].is_off_cooldown(now_ms) {
            self.skills[idx].last_used_ms = now_ms;
            (self.location.x, self.location.y) = (mob_loc.x, mob_loc.y);
            return BotDirective::UseSkill { skill: kind, target_mob: Some(mob_id), target_player: None, move_to: (mob_loc.x, mob_loc.y) };
        }
        BotDirective::Idle
    }

    fn tick_giao_dich(&mut self, now_ms: u64, rng: &mut GameRng) -> BotDirective {
        if self.location.map_id.0 as u16 != MARKET_MAP_ID {
            let x = 81 + rng_range(rng, 0, 715) as i16;
            (self.location.map_id, self.location.x, self.location.y) = (MapId(MARKET_MAP_ID as _), x, 336);
            return BotDirective::ChangeMap { map_id: MapId(MARKET_MAP_ID as _), x, y: 336 };
        }
        let g = now_ms.saturating_sub(self.timers_ms[3]) > (100 + rng_range(rng, 0, 99) as u64) * 1_000;
        let l = now_ms.saturating_sub(self.timers_ms[4]) > (5 + rng_range(rng, 0, 4) as u64) * 1_000;
        if g { self.timers_ms[3] = now_ms; }
        if l { self.timers_ms[4] = now_ms; }
        if g || l { BotDirective::Advertise { global: g, local: l } } else { BotDirective::Idle }
    }

    fn tick_san_boss(&mut self, world: &BotWorldView<'_>, now_ms: u64, rng: &mut GameRng) -> BotDirective {
        if world.active_bosses.is_empty() { return BotDirective::Despawn; }
        let chosen = (0..4).find_map(|_| {
            let b = world.active_bosses[rng_range(rng, 0, (world.active_bosses.len() - 1) as i64) as usize];
            (!b.2 && !b.3 && b.4).then_some(b)
        });
        let Some((boss_id, boss_loc, _, _, _)) = chosen else { return BotDirective::Despawn; };
        if self.location.map_id != boss_loc.map_id {
            self.location = boss_loc;
            return BotDirective::ChangeMap { map_id: boss_loc.map_id, x: boss_loc.x, y: boss_loc.y };
        }
        if now_ms.saturating_sub(self.timers_ms[1]) > 50_000 {
            self.timers_ms[1] = now_ms.saturating_sub(rng_range(rng, 0, 149_999) as u64);
            let ult = [SkillKind::QuaCauKenhKhi, SkillKind::Makankosappo, SkillKind::BienKhi][usize::from(gender_index(self.gender))];
            return BotDirective::UseSkill { skill: ult, target_mob: None, target_player: Some(boss_id), move_to: (boss_loc.x, boss_loc.y) };
        }
        let idx = usize::from(!rng_ratio(rng, 50, 100)).min(self.skills.len().saturating_sub(1));
        if self.skills[idx].is_off_cooldown(now_ms) {
            self.skills[idx].last_used_ms = now_ms;
            self.location = boss_loc;
            return BotDirective::UseSkill { skill: self.skills[idx].kind, target_mob: None, target_player: Some(boss_id), move_to: (boss_loc.x, boss_loc.y) };
        }
        BotDirective::Idle
    }

    fn tick_attack_player(&mut self, world: &BotWorldView<'_>, now_ms: u64, rng: &mut GameRng) -> BotDirective {
        let Some(&(target_id, target_loc)) = world.map_players.iter().filter(|(pid, _)| *pid != self.id).min_by_key(|(_, loc)| i32::from(self.location.x - loc.x).abs()) else { return BotDirective::Idle; };
        if self.location.map_id != target_loc.map_id {
            self.location = target_loc;
            return BotDirective::ChangeMap { map_id: target_loc.map_id, x: target_loc.x, y: target_loc.y };
        }
        if now_ms.saturating_sub(self.timers_ms[5]) < PVP_BOT_COOLDOWN_MS || self.skills.is_empty() { return BotDirective::Idle; }
        self.timers_ms[5] = now_ms;
        self.location = target_loc;
        let sk = self.skills[rng_range(rng, 0, (self.skills.len() - 1) as i64) as usize].kind;
        BotDirective::UseSkill { skill: sk, target_mob: None, target_player: Some(target_id), move_to: (target_loc.x, target_loc.y) }
    }
}

/// Global bot registry ticking every `150` ms (`BotManager.java`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BotManager {
    /// Active bots managed by the server.
    pub bots: Vec<BotAgent>,
    /// Last global tick timestamp (`ms`).
    pub last_tick_ms: u64,
}

impl BotManager {
    /// Advances all bots when `>= 150` ms have elapsed, removing despawned bots.
    pub fn tick_all(&mut self, world: &BotWorldView<'_>, now_ms: u64, parity: ParityMode, rng: &mut GameRng) -> Vec<(PlayerId, BotDirective)> {
        if now_ms.saturating_sub(self.last_tick_ms) < BOT_MANAGER_TICK_MS { return Vec::new(); }
        self.last_tick_ms = now_ms;
        let mut actions = Vec::with_capacity(self.bots.len());
        self.bots.retain_mut(|bot| {
            let dir = bot.tick(world, now_ms, parity, rng);
            let keep = dir != BotDirective::Despawn;
            actions.push((bot.id, dir));
            keep
        });
        actions
    }
}
