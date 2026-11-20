//! Tasks, Side/Clan quests, Badges, Radar cards, Charms, Intrinsics, KOL quests, and Clans.
//!
//! Ports `TaskService.java` (with ALL hardcoded IP DRM kill-switches removed and the
//! `(id << 11) | (index << 1)` bit-packing formalized), `SideTask`, `Badges`, `RadarCard`,
//! `Charms`, `Intrinsic`, `KOLProgressData`, `LearnSkill`, and `Clan` / `ClanService`.

use nro_consts as consts;
use nro_core::{ClanId, GameRng, Gender, ItemOption, ParityMode, PlayerId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};

use crate::{PlayerError, gender_index, rng_range};

/// Maximum daily Bò Mộng side quests (`ConstTask.MAX_SIDE_TASK = 10`).
pub const MAX_DAILY_SIDE_TASKS: u8 = 10;
/// Maximum daily Clan quests (`ConstTask.MAX_CLAN_TASK = 5`).
pub const MAX_DAILY_CLAN_TASKS: u8 = 5;
/// Minimum task key required to join or create a Clan (`ConstTask.TASK_10_0 = 20_480`).
pub const CLAN_JOIN_MIN_TASK_KEY: u32 = TaskPlayer::encode_task_key(10, 0);
/// Minimum power required to open or reroll Intrinsics (`10,000,000,000`).
pub const INTRINSIC_MIN_POWER: i64 = 10_000_000_000;
/// Gold cost (in millions) per sequential Intrinsic roll (`IntrinsicService.COST_OPEN`).
pub const INTRINSIC_GOLD_MILLIONS: [u64; 8] = [10, 20, 40, 80, 160, 320, 640, 1280];
/// Skill book study timers (ms) for levels `1..=7` (`15m, 30m, 1h, 1d, 3d, 7d, 15d`).
pub const LEARN_SKILL_DURATIONS_MS: [u64; 7] = [900_000, 1_800_000, 3_600_000, 86_400_000, 259_200_000, 604_800_000, 1_296_000_000];

/// Side & Clan quest difficulty tiers (`ConstTask.EASY..HELL`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskDifficulty {
    /// Easy (`5,000` gold, item `708`).
    #[default]
    Easy = 0,
    /// Normal (`20,000` gold, item `707`).
    Normal = 1,
    /// Hard (`50,000` gold, item `706`).
    Hard = 2,
    /// Very Hard (`200,000` gold, item `705`).
    VeryHard = 3,
    /// Hell (`25` gold in legacy Java typo vs `500,000` in `Fixed` mode, item `704`).
    Hell = 4,
}

impl TaskDifficulty {
    /// Returns `(gold_reward, reward_orb_item_id)` for this difficulty, honoring [`ParityMode`].
    pub const fn side_task_reward(self, parity: ParityMode) -> (u64, u16) {
        match self {
            Self::Easy => (5_000, 708),
            Self::Normal => (20_000, 707),
            Self::Hard => (50_000, 706),
            Self::VeryHard => (200_000, 705),
            Self::Hell => (if matches!(parity, ParityMode::Fixed) { 500_000 } else { 25 }, 704),
        }
    }
}

/// Sub-step within a Main Task (`SubTaskMain.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubTaskMain {
    /// Current progress `count`, target `max_count`, target `npc_id`, and `map_id`.
    pub count: u16,
    /// Target completion count.
    pub max_count: u16,
    /// Target NPC ID.
    pub npc_id: i16,
    /// Target Map ID.
    pub map_id: i16,
}

/// Daily repeatable Side Task or Clan Task tracker (`SideTask.java` / `ClanTask.java`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyQuestState {
    /// Active template ID (`0..=57` mob kill, `58` gold pickup), `count`, and `max_count`.
    pub template_id: Option<u8>,
    /// Current progress count.
    pub count: u32,
    /// Required count for completion.
    pub max_count: u32,
    /// Selected [`TaskDifficulty`], remaining daily quota, and notified-decile bitmask.
    pub difficulty: TaskDifficulty,
    /// Remaining daily quests.
    pub left_today: u8,
    /// Bitmask of notified deciles (`0..=9` for `0%..=90%`).
    pub notified_deciles: u16,
}

impl DailyQuestState {
    /// Increments progress by `delta`, returning `Some(percent)` if a new decile or `100%` was reached.
    pub fn add_progress(&mut self, delta: u32) -> Option<u8> {
        if self.template_id.is_none() || self.max_count == 0 { return None; }
        self.count = self.count.saturating_add(delta).min(self.max_count);
        let pct = ((u64::from(self.count) * 100) / u64::from(self.max_count)) as u8;
        if pct >= 100 { return Some(100); }
        let mask = 1_u16 << (pct / 10);
        if (self.notified_deciles & mask) == 0 { self.notified_deciles |= mask; Some(pct) } else { None }
    }

    /// Returns `true` when `count >= max_count` (`isDone`).
    #[inline] pub const fn is_done(&self) -> bool { self.template_id.is_some() && self.count >= self.max_count }
}

/// Player quest state (`TaskPlayer.java` + `TaskService.java` without any IP DRM checks).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskPlayer {
    /// Current `TaskMain` ID (`0..=28`), active `main_index`, and `sub_tasks`.
    pub main_id: u16,
    /// Active sub-task index (`0..sub_tasks.len()`).
    pub main_index: u16,
    /// Sub-tasks of the active `TaskMain`.
    pub sub_tasks: SmallVec<[SubTaskMain; 8]>,
    /// Bò Mộng daily side task (`10/day`) and daily clan task (`5/day`).
    pub side_task: DailyQuestState,
    /// Daily clan task (`5/day`).
    pub clan_task: DailyQuestState,
}

impl Default for TaskPlayer {
    fn default() -> Self {
        Self {
            main_id: 0, main_index: 0, sub_tasks: SmallVec::new(),
            side_task: DailyQuestState { left_today: MAX_DAILY_SIDE_TASKS, ..Default::default() },
            clan_task: DailyQuestState { left_today: MAX_DAILY_CLAN_TASKS, ..Default::default() },
        }
    }
}

impl TaskPlayer {
    /// Encodes `(main_id, main_index)` into NRO's bit-packed `ConstTask` key: `(id << 11) | (index << 1)`.
    #[inline]
    pub const fn encode_task_key(main_id: u16, main_index: u16) -> u32 {
        ((main_id as u32) << 11) | (((main_index & 0x03FF) as u32) << 1)
    }

    /// Decodes a `ConstTask` key back into `(main_id, main_index)`.
    #[inline]
    pub const fn decode_task_key(key: u32) -> (u16, u16) {
        ((key >> 11) as u16, ((key >> 1) & 0x03FF) as u16)
    }

    /// Returns the player's current bit-packed `ConstTask` key.
    #[inline] pub const fn current_key(&self) -> u32 { Self::encode_task_key(self.main_id, self.main_index) }

    /// Advances the current sub-task if `expected_key` matches, returning `(completed_main_id, next_main_id)`.
    pub fn record_event(&mut self, expected_key: u32, amount: u16, gender: Gender) -> Option<(u16, u16)> {
        let _ = consts::PER100;
        if self.current_key() != expected_key { return None; }
        if let Some(sub) = self.sub_tasks.get_mut(usize::from(self.main_index)) {
            sub.count = sub.count.saturating_add(amount);
            if sub.count < sub.max_count { return None; }
        }
        self.main_index += 1;
        if usize::from(self.main_index) >= self.sub_tasks.len() {
            let done_id = self.main_id;
            self.main_id = match done_id { 3 => 4 + u16::from(gender_index(gender)), 4..=6 => 7, _ => done_id + 1 };
            self.main_index = 0;
            return Some((done_id, self.main_id));
        }
        None
    }

    /// Computes Clan co-op mob-kill credit on Tasks `14` and `15` (`2` if `>= 2` clanmates in zone, `1` if `1`, `0` if clanless).
    #[inline] pub const fn clan_coop_kill_credit(in_clan: bool, same_clan_in_zone: usize) -> u16 { if !in_clan { 0 } else if same_clan_in_zone >= 2 { 2 } else { 1 } }

    /// Computes `(power_only, tn_only, both_smtn, gold)` awarded on completing `TaskMain` `done_id`.
    pub const fn main_task_reward(done_id: u16) -> (u64, u64, u64, u64) {
        let early = match done_id { 0 => 500, 1 => 1_000, 2 => 1_200, 3 => 3_000, 4 => 7_000, 5 => 20_000, _ => 0 };
        let (both, gold) = if done_id > 0 && done_id < 25 {
            (500 * (done_id as u64 + 1), if done_id < 5 { 100_000 * (done_id as u64 + 1) } else { 500_000 })
        } else { (0, 0) };
        (early, early, both, gold)
    }
}

/// Bà Hạt Mít charm expiry timestamps (`nro.models.player.Charms`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Charms {
    /// Expiry timestamps (ms): `[tri_tue, manh_me, da_trau, oai_hung, bat_tu, deo_dai, thu_hut, de_tu, tri_tue_3, tri_tue_4, mabu_1, mabu_2, mabu_3, phu_hp, phu_ki, phu_sd, phu_tnsm]`.
    pub expires_ms: [u64; 17],
}

impl Charms {
    /// Extends the charm corresponding to `item_id` by `minutes` from `max(now_ms, current_expiry)`.
    pub fn add_minutes(&mut self, item_id: u16, minutes: u64, now_ms: u64) -> bool {
        const IDS: [u16; 17] = [213, 214, 215, 216, 217, 218, 219, 522, 671, 672, 2025, 2076, 1387, 3000, 3001, 3002, 3003];
        let Some(idx) = IDS.iter().position(|&id| id == item_id) else { return false; };
        self.expires_ms[idx] = self.expires_ms[idx].max(now_ms).saturating_add(minutes.saturating_mul(60_000));
        true
    }

    /// Checks whether charm slot `idx` is active at `now_ms`.
    #[inline] pub const fn is_active(&self, idx: usize, now_ms: u64) -> bool { idx < self.expires_ms.len() && self.expires_ms[idx] > now_ms }
}

/// Rolled passive Intrinsic (`Intrinsic.java` + `IntrinsicPlayer.java` + `IntrinsicService.java`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntrinsicState {
    /// Intrinsic ID (`0` = none), rolled `param1`, rolled `param2`, and gold open counter (`0..=7`).
    pub id: u8,
    /// Primary rolled percentage parameter.
    pub param1: i16,
    /// Secondary rolled parameter.
    pub param2: i16,
    /// Number of gold opens (`countOpen`, reset to `0` on `100`-gem VIP open).
    pub count_open: u8,
}

impl IntrinsicState {
    /// Rolls a new intrinsic via Gold (`is_vip == false`) or `100` Gems (`is_vip == true`, resets `count_open`).
    pub fn roll(&mut self, power: i64, gold: &mut u64, gems: &mut u32, is_vip: bool, pool: &[(u8, i16, i16, i16, i16)], rng: &mut GameRng) -> Result<(), PlayerError> {
        if power < INTRINSIC_MIN_POWER || pool.is_empty() { return Err(PlayerError::PowerLimitReached); }
        if is_vip {
            if *gems < 100 { return Err(PlayerError::InsufficientCurrency); }
            *gems -= 100;
            self.count_open = 0;
        } else {
            let cost = INTRINSIC_GOLD_MILLIONS[usize::from(self.count_open).min(7)] * 1_000_000;
            if *gold < cost { return Err(PlayerError::InsufficientCurrency); }
            *gold -= cost;
            if self.count_open < 7 { self.count_open += 1; }
        }
        let &(id, f1, t1, f2, t2) = &pool[rng_range(rng, 0, (pool.len() - 1) as i64) as usize];
        *self = Self { id, param1: rng_range(rng, i64::from(f1), i64::from(t1.max(f1))) as i16, param2: rng_range(rng, i64::from(f2), i64::from(t2.max(f2))) as i16, count_open: self.count_open };
        Ok(())
    }
}

/// Radar collection card (`radar/Card.java`) and option (`OptionCard.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarCard {
    /// Card ID, `level` (`-1`..`max`), `(amount, max_amount)`, `used`, `aura_id`, and `(option, active_level)` pairs.
    pub id: u16,
    /// Upgrade level.
    pub level: i8,
    /// `(collected_fragments, max_fragments_per_level)`.
    pub progress: (u8, u8),
    /// Whether this card is the active slot (`Used == 1`) and unlocked `aura_id`.
    pub used: bool,
    /// Visual aura ID unlocked when `level > 1` on aura cards (`956, 1142, 1204, 1791..=1793`).
    pub aura_id: i16,
    /// Card options paired with required activation level (`io.active`).
    pub options: SmallVec<[(ItemOption, i8); 4]>,
}

impl RadarCard {
    /// Collects options active at the card's current `level` (`io.active == level || (level == -1 && io.active == 0)`).
    pub fn active_options(&self) -> SmallVec<[ItemOption; 4]> {
        self.options.iter().filter(|(_, act)| *act == self.level || (self.level == -1 && *act == 0)).map(|(opt, _)| *opt).collect()
    }
}

/// Timed title badge (`BadgesData.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeEntry {
    /// Badge effect ID, expiry timestamp (`ms`), `is_use` toggle, and granted `ItemOption`s.
    pub id_badges: u16,
    /// Expiry timestamp (`ms`).
    pub expires_at_ms: u64,
    /// Currently equipped badge flag.
    pub is_use: bool,
    /// Stat options granted while equipped and unexpired.
    pub options: SmallVec<[ItemOption; 4]>,
}

/// KOL Normal & VIP progression counters (`KOLProgressData.java` + `LearnSkill.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolProgressData {
    /// Normal KOL stage (`>= 1`) and VIP KOL stage (`>= 1`, requires item `1825`).
    pub kol_quest_stage: u16,
    /// Current VIP KOL stage (`>= 1`).
    pub kol_vip_quest_stage: u16,
    /// Activity counters: `[destron_gas_70, tournament_wins, super_hard_daily, boss_baby]`.
    pub activity_counts: [u32; 4],
    /// Auto-train monster kills and optional active skill-book study `(skill_item_id, finish_at_ms)`.
    pub auto_train_kills: u64,
    /// Active skill-book learning timer.
    pub learning_skill: Option<(u16, u64)>,
}

impl Default for KolProgressData {
    fn default() -> Self {
        Self { kol_quest_stage: 1, kol_vip_quest_stage: 1, activity_counts: [0; 4], auto_train_kills: 0, learning_skill: None }
    }
}

/// Guild hierarchy roles (`Clan.LEADER = 0, DEPUTY = 1, MEMBER = 2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumCount, EnumIter, FromRepr, Serialize, Deserialize)]
#[repr(u8)]
pub enum ClanRole {
    /// Guild Leader (`Bang chủ = 0`).
    Leader = 0,
    /// Guild Deputy (`Phó bang = 1`).
    Deputy = 1,
    /// Regular Member (`Thành viên = 2`).
    Member = 2,
}

/// Guild member record (`ClanMember.java`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClanMember {
    /// Member `PlayerId`, `role`, `power_point`, `(donate, receive_donate)`, `(member_point, clan_point)`, and `last_ask_pea_ms`.
    pub player_id: PlayerId,
    /// Hierarchy role.
    pub role: ClanRole,
    /// Cached power point.
    pub power_point: i64,
    /// `(donated_peas, received_peas)`.
    pub pea_stats: (u32, u32),
    /// `(personal_capsules, total_clan_capsules)`.
    pub capsule_points: (u32, u32),
    /// Last `askForPea` timestamp (`5` min cooldown).
    pub last_ask_pea_ms: u64,
}

/// Complete Guild state (`Clan.java` + `ClanService.java`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clan {
    /// Unique `ClanId`, `img_id` flag bag, `level` (`1..=11`), `max_member` (default `10`), and `capsule_clan`.
    pub id: ClanId,
    /// Emblem `FlagBag` ID.
    pub img_id: u8,
    /// Guild level (`1..=11`).
    pub level: u8,
    /// Member cap (default `10`).
    pub max_member: u8,
    /// Accumulated guild capsules, `members` roster, and `20`-entry `pea_requests` ring buffer.
    pub capsule_clan: u32,
    /// Guild roster.
    pub members: SmallVec<[ClanMember; 16]>,
    /// Active pea-request messages `(msg_id, player_id, received_0_to_5)` (capped at `20`).
    pub pea_requests: SmallVec<[(u32, PlayerId, u8); 20]>,
    /// Monotonic message ID counter.
    pub next_msg_id: u32,
}

impl Clan {
    /// Creates a new Level-1 Clan led by `leader_id`.
    pub fn create(id: ClanId, img_id: u8, leader_id: PlayerId, leader_power: i64) -> Self {
        let mut members = SmallVec::new();
        members.push(ClanMember { player_id: leader_id, role: ClanRole::Leader, power_point: leader_power, pea_stats: (0, 0), capsule_points: (0, 0), last_ask_pea_ms: 0 });
        Self { id, img_id, level: 1, max_member: 10, capsule_clan: 0, members, pea_requests: SmallVec::new(), next_msg_id: 1 }
    }

    /// Adds a member if `task_key >= TASK_10_0`, `!holding_black_ball`, and capacity allows.
    pub fn join(&mut self, player_id: PlayerId, power: i64, task_key: u32, holding_black_ball: bool) -> Result<(), PlayerError> {
        if task_key < CLAN_JOIN_MIN_TASK_KEY || holding_black_ball || self.members.len() >= usize::from(self.max_member) || self.members.iter().any(|m| m.player_id == player_id) {
            return Err(PlayerError::ClanActionDenied);
        }
        self.members.push(ClanMember { player_id, role: ClanRole::Member, power_point: power, pea_stats: (0, 0), capsule_points: (0, 0), last_ask_pea_ms: 0 });
        Ok(())
    }

    /// Transfers leadership from `leader_id` to an existing `Deputy` (`ClanService.phongPc`: old leader becomes `Member`).
    pub fn transfer_leadership(&mut self, leader_id: PlayerId, new_leader_id: PlayerId) -> Result<(), PlayerError> {
        if !self.members.iter().any(|m| m.player_id == leader_id && m.role == ClanRole::Leader) || !self.members.iter().any(|m| m.player_id == new_leader_id && m.role == ClanRole::Deputy) {
            return Err(PlayerError::ClanActionDenied);
        }
        for m in &mut self.members {
            if m.player_id == leader_id { m.role = ClanRole::Member; } else if m.player_id == new_leader_id { m.role = ClanRole::Leader; }
        }
        Ok(())
    }

    /// Posts a Pea donation request (`5` minute cooldown, `maxDonate = 5`) (`ClanService.askForPea`).
    pub fn ask_for_pea(&mut self, player_id: PlayerId, now_ms: u64) -> Result<u32, PlayerError> {
        let m = self.members.iter_mut().find(|m| m.player_id == player_id).ok_or(PlayerError::ClanActionDenied)?;
        if m.last_ask_pea_ms > 0 && now_ms.saturating_sub(m.last_ask_pea_ms) < 300_000 { return Err(PlayerError::ClanActionDenied); }
        m.last_ask_pea_ms = now_ms;
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        if self.pea_requests.len() >= 20 { self.pea_requests.remove(0); }
        self.pea_requests.push((id, player_id, 0));
        Ok(id)
    }

    /// Fulfills 1 Pea (`max 5`) on request `msg_id`, crediting donor & recipient (`ClanService.clanDonate`).
    pub fn donate_pea(&mut self, donor_id: PlayerId, msg_id: u32) -> Result<PlayerId, PlayerError> {
        let req = self.pea_requests.iter_mut().find(|r| r.0 == msg_id && r.2 < 5).ok_or(PlayerError::ClanActionDenied)?;
        let receiver_id = req.1;
        if receiver_id == donor_id { return Err(PlayerError::ClanActionDenied); }
        req.2 += 1;
        if let Some(d) = self.members.iter_mut().find(|m| m.player_id == donor_id) { d.pea_stats.0 += 1; }
        if let Some(r) = self.members.iter_mut().find(|m| m.player_id == receiver_id) { r.pea_stats.1 += 1; }
        Ok(receiver_id)
    }

    /// Capsule threshold for `level` `1..=11` (`100, 300, ..., 2100`).
    #[inline] pub const fn required_capsules(level: u8) -> u32 { if level >= 1 && level <= 11 { (level as u32) * 200 - 100 } else { 999_999 } }

    /// Shared `tiem_nang` granted to a same-zone clanmate (`Clan.addSMTNClan`).
    #[inline]
    pub const fn shared_zone_tnsm(origin_param: u64, origin_caption_lvl: u8, mate_caption_lvl: u8) -> u64 {
        let diff = origin_caption_lvl.abs_diff(mate_caption_lvl) as u64;
        origin_param / if diff == 0 { 1 } else { diff }
    }
}
