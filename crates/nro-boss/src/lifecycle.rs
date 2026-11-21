//! Deterministic `Boss` 9-state lifecycle machine and `AppearType` wave coordinator.
//!
//! Reproduces every state transition in `Boss.java` (`REST`, `RESPAWN`, `JOIN_MAP`, `CHAT_S`,
//! `ACTIVE`, `DIE`, `CHAT_E`, `LEAVE_MAP`, `AFK`) and all four `AppearType` modes while fixing
//! sequential wave deadlocks, orphaned-minion freezes, and `Boss.die()`'s premature `LEAVE_MAP`
//! overwrite when running under [`ParityMode::Fixed`].

use crate::{BossFlags, BossTypeId, EngineEffect, GameRng, ParityMode, PkMode, PlayerId, SkillKind};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumIter, EnumString};

/// Exact 9-state boss lifecycle mirroring `nro.models.boss.BossStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Display, EnumIter, EnumString, Serialize, Deserialize)]
pub enum BossStatus {
    /// Waiting out respawn cooldown (`secondsRest`) or awaiting parent/summoner trigger.
    #[default]
    Rest,
    /// Reinitializing stage stats (`hpMax`, `dame`) and clearing status ailments.
    Respawn,
    /// Selecting target map/zone and entering world coordinates.
    JoinMap,
    /// Speaking intro lines (`textS`) sequentially before engaging in combat.
    ChatStart,
    /// Actively executing movement, skill selection, and mid-combat lines (`textM`).
    Active,
    /// Killed; triggering loot drops, task hooks, and transition to `ChatEnd` or next stage.
    Die,
    /// Speaking defeat lines (`textE`) sequentially before leaving or transforming.
    ChatEnd,
    /// Exiting map (or evolving in-place on multi-stage `AnotherLevel` bosses).
    LeaveMap,
    /// Parked on-map in `NonPk` waiting for wave wakeup, revival timer, or escort target.
    Afk,
}

/// Boss spawn/grouping mode mirroring `nro.models.boss.AppearType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Display, EnumIter, EnumString, Serialize, Deserialize)]
pub enum AppearType {
    /// Autonomous timer-driven spawn after `rest_seconds` elapses in `Rest`.
    #[default]
    DefaultAppear,
    /// Co-spawned alongside its `DefaultAppear` parent (`JoinMap` triggers `Respawn`).
    AppearWithAnother,
    /// Subsequent transformation stage (`current_stage >= 1`) of a multi-form `Boss`.
    AnotherLevel,
    /// Held dormant in `Rest` until explicitly summoned (`Android13`, `XenCon`, `DeathBeam`).
    CallByAnother,
}

/// Single skill entry `(skill_kind, level, cooldown_ms)` inside a [`StageConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSlot {
    /// Canonical skill identifier.
    pub kind: SkillKind,
    /// Skill point level (`1..=7`).
    pub level: u8,
    /// Minimum cooldown between casts in milliseconds.
    pub cooldown_ms: u32,
}

/// Per-form configuration mirroring an entry in `Boss.data[]` (`BossData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageConfig {
    /// Display name for this form (e.g., `"Fide đại ca 1"`, `"Siêu Bọ Hung"`).
    pub name: String,
    /// Six-slot outfit array `[head, body, leg, flag_bag, aura, eff_front]`.
    pub outfit: [i16; 6],
    /// Base attack stat (`dame`).
    pub base_dame: i64,
    /// Candidate max-HP pool (`hp_pool`); one entry is chosen randomly on `Respawn`.
    pub hp_pool: SmallVec<[i64; 4]>,
    /// Available combat skills for this stage.
    pub skills: SmallVec<[SkillSlot; 8]>,
    /// Intro dialogue spoken during `ChatStart` (`textS`).
    pub chat_start: Vec<String>,
    /// Periodic combat taunt lines spoken during `Active` (`textM`).
    pub chat_mid: Vec<String>,
    /// Defeat dialogue spoken during `ChatEnd` (`textE`).
    pub chat_end: Vec<String>,
    /// Respawn cooldown in seconds (`secondsRest`).
    pub rest_seconds: u32,
    /// Stage `AppearType` (`DefaultAppear` on stage 0, `AnotherLevel` on stages `>= 1`).
    pub appear_type: AppearType,
}

/// Wave coordination role governing `Afk` parking and sequential wakeup chains.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WaveRole {
    /// Fights immediately upon finishing `ChatStart`.
    #[default]
    Independent,
    /// Sequential wave fighter (`Poc->Pic->KingKong`, `SO4->SO3->SO2+SO1->TDT`, `Saibamen`).
    SequentialMember {
        /// Zero-based order index within the wave (`0` fights first).
        order: u8,
        /// Companion wave order indices awakened when this member finishes `ChatEnd`.
        wake_orders_on_death: SmallVec<[u8; 4]>,
        /// Whether this member parks in `Afk` (`NonPk`) right after `ChatStart`.
        start_afk: bool,
    },
    /// Parent boss parked in `Afk` until all `AppearWithAnother` minions die (`Bojack`, `TDT`).
    MinionGuardParent,
    /// Summoned mid-fight via `CallByAnother` (`Android13`, `XenCon 1..7`, `DeathBeam 1..5`).
    SummonedChild,
}

/// Mutable per-boss lifecycle state machine context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleState {
    /// Current state in the 9-state machine.
    pub status: BossStatus,
    /// Primary spawn mode of this boss entity.
    pub appear_type: AppearType,
    /// Active form index into `stages` (`0..stages.len()`).
    pub current_stage: usize,
    /// Current PK stance (`NonPk`, `PkPvp`, `PkAll`).
    pub pk_mode: PkMode,
    /// Timestamp (ms) when `Rest` started.
    pub last_rest_ms: u64,
    /// Timestamp (ms) when the boss entered the map (`JoinMap`).
    pub joined_map_ms: u64,
    /// Timestamp (ms) when at least one human player was last observed in the zone.
    pub last_player_seen_ms: u64,
    /// Next dialogue index within `chat_start` or `chat_end`.
    pub chat_index: usize,
    /// Timestamp (ms) when the current chat line finishes displaying.
    pub chat_ready_at_ms: u64,
    /// Timestamp (ms) for next periodic `chat_mid` taunt.
    pub next_mid_chat_ms: u64,
    /// Optional timestamp (ms) when an `Afk` in-place revival completes (`BuiBui`, `Yacon`).
    pub afk_revive_at_ms: Option<u64>,
    /// Optional parent runtime `PlayerId` for companion/summoned bosses.
    pub parent_id: Option<PlayerId>,
    /// Wave coordination role.
    pub wave_role: WaveRole,
}

/// High-level action signal returned by [`LifecycleState::step`] to `BossRuntime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    /// No structural map/stage change occurred on this tick.
    None,
    /// Reinitialize stats (`hpMax`, `dame`) for `current_stage`.
    InitStageStats,
    /// Place boss into its target zone and wake `AppearWithAnother` companions if stage 0.
    EnterZone { wake_companions: bool },
    /// Execute one tick of combat AI (`BossArchetype` + skills).
    RunCombatTick,
    /// Trigger loot roll and death hooks (`BossStatus::Die`).
    ProcessDeath,
    /// Advance in-place to `current_stage + 1` (`AppearType::AnotherLevel`) with full heal.
    TransformNextStage,
    /// Remove boss from map, wake successor wave members, and enter `Rest` (or dispose).
    ExitZoneAndRest,
    /// Revive in-place from `Afk` at full HP (`BuiBui`, `BuiBui2`, `Yacon`, `Goku`, `Cadic`).
    ReviveFromAfk,
}

impl LifecycleState {
    /// Creates a new `LifecycleState` initialized in `BossStatus::Rest`.
    #[must_use]
    pub fn new(appear_type: AppearType, wave_role: WaveRole, now_ms: u64) -> Self {
        Self {
            status: BossStatus::Rest,
            appear_type,
            current_stage: 0,
            pk_mode: PkMode::NonPk,
            last_rest_ms: now_ms,
            joined_map_ms: now_ms,
            last_player_seen_ms: now_ms,
            chat_index: 0,
            chat_ready_at_ms: now_ms,
            next_mid_chat_ms: now_ms + 5_000,
            afk_revive_at_ms: None,
            parent_id: None,
            wave_role,
        }
    }

    /// Transitions cleanly to `next`, resetting per-phase dialogue cursors as needed.
    pub fn transition_to(&mut self, next: BossStatus, now_ms: u64) {
        self.status = next;
        match next {
            BossStatus::Rest => {
                self.last_rest_ms = now_ms;
                self.pk_mode = PkMode::NonPk;
                self.current_stage = 0;
                self.afk_revive_at_ms = None;
            }
            BossStatus::ChatStart | BossStatus::ChatEnd => {
                self.chat_index = 0;
                self.chat_ready_at_ms = now_ms;
            }
            BossStatus::JoinMap => {
                self.joined_map_ms = now_ms;
                self.last_player_seen_ms = now_ms;
            }
            _ => {}
        }
    }

    /// Advances the 9-state lifecycle machine by one tick at `now_ms`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        entity_id: PlayerId,
        stages: &[StageConfig],
        flags: BossFlags,
        players_in_zone: usize,
        can_spawn_window: bool,
        parity: ParityMode,
        rng: &mut GameRng,
        now_ms: u64,
        effects: &mut Vec<EngineEffect>,
    ) -> LifecycleAction {
        let Some(stage) = stages.get(self.current_stage).or_else(|| stages.last()) else { return LifecycleAction::None; };
        if players_in_zone > 0 {
            self.last_player_seen_ms = now_ms;
        }
        match self.status {
            BossStatus::Rest => {
                if self.appear_type == AppearType::DefaultAppear && can_spawn_window {
                    let wait_ms = u64::from(stage.rest_seconds) * 1_000;
                    if now_ms.saturating_sub(self.last_rest_ms) >= wait_ms {
                        self.transition_to(BossStatus::Respawn, now_ms);
                    }
                }
                LifecycleAction::None
            }
            BossStatus::Respawn => {
                self.transition_to(BossStatus::JoinMap, now_ms);
                LifecycleAction::InitStageStats
            }
            BossStatus::JoinMap => {
                let wake_companions = self.current_stage == 0;
                self.transition_to(BossStatus::ChatStart, now_ms);
                LifecycleAction::EnterZone { wake_companions }
            }
            BossStatus::ChatStart => {
                if step_dialogue(entity_id, &stage.chat_start, &mut self.chat_index, &mut self.chat_ready_at_ms, now_ms, effects) {
                    let park_afk = matches!(
                        self.wave_role,
                        WaveRole::SequentialMember { start_afk: true, .. } | WaveRole::MinionGuardParent
                    );
                    if park_afk && self.current_stage == 0 {
                        self.pk_mode = PkMode::NonPk;
                        self.transition_to(BossStatus::Afk, now_ms);
                    } else {
                        self.pk_mode = PkMode::PkAll;
                        effects.push(EngineEffect::ChangePk { entity: entity_id, mode: PkMode::PkAll });
                        self.transition_to(BossStatus::Active, now_ms);
                    }
                }
                LifecycleAction::None
            }
            BossStatus::Active => {
                if !can_spawn_window || self.should_auto_leave(flags, players_in_zone, parity, now_ms) {
                    self.afk_revive_at_ms = None;
                    self.transition_to(BossStatus::LeaveMap, now_ms);
                    return LifecycleAction::None;
                }
                if !stage.chat_mid.is_empty() && now_ms >= self.next_mid_chat_ms {
                    let idx = rng.next_int(0, (stage.chat_mid.len() - 1) as i32) as usize;
                    effects.push(EngineEffect::Chat { speaker: entity_id, text: stage.chat_mid[idx].clone() });
                    self.next_mid_chat_ms = now_ms + rng.next_int(3_000, 12_000) as u64;
                }
                LifecycleAction::RunCombatTick
            }
            BossStatus::Die => {
                if parity == ParityMode::Legacy {
                    self.status = BossStatus::LeaveMap;
                }
                self.pk_mode = PkMode::NonPk;
                self.transition_to(BossStatus::ChatEnd, now_ms);
                LifecycleAction::ProcessDeath
            }
            BossStatus::ChatEnd => {
                if step_dialogue(entity_id, &stage.chat_end, &mut self.chat_index, &mut self.chat_ready_at_ms, now_ms, effects) {
                    if self.current_stage + 1 < stages.len() {
                        self.current_stage += 1;
                        self.transition_to(BossStatus::Respawn, now_ms);
                        return LifecycleAction::TransformNextStage;
                    }
                    self.transition_to(BossStatus::LeaveMap, now_ms);
                }
                LifecycleAction::None
            }
            BossStatus::LeaveMap => {
                self.transition_to(BossStatus::Rest, now_ms);
                LifecycleAction::ExitZoneAndRest
            }
            BossStatus::Afk => {
                if !can_spawn_window {
                    self.afk_revive_at_ms = None;
                    self.transition_to(BossStatus::LeaveMap, now_ms);
                    return LifecycleAction::None;
                }
                if let Some(revive_at) = self.afk_revive_at_ms
                    && now_ms >= revive_at
                {
                    self.afk_revive_at_ms = None;
                    self.pk_mode = PkMode::PkAll;
                    self.transition_to(BossStatus::Active, now_ms);
                    return LifecycleAction::ReviveFromAfk;
                }
                if self.current_stage > 0
                    && matches!(self.wave_role, WaveRole::Independent | WaveRole::MinionGuardParent)
                {
                    self.pk_mode = PkMode::PkAll;
                    self.transition_to(BossStatus::Active, now_ms);
                }
                LifecycleAction::None
            }
        }
    }

    fn should_auto_leave(&self, flags: BossFlags, players: usize, parity: ParityMode, now_ms: u64) -> bool {
        if flags.contains(BossFlags::LEGACY_UNCOND_DESPAWN) {
            let anchor = if parity == ParityMode::Legacy { self.joined_map_ms } else { self.last_player_seen_ms };
            return now_ms.saturating_sub(anchor) >= 900_000;
        }
        if flags.contains(BossFlags::DISPOSE_ON_LEAVE) && players == 0 && now_ms.saturating_sub(self.last_player_seen_ms) >= 300_000 {
            return true;
        }
        flags.contains(BossFlags::EMPTY_ZONE_DESPAWN)
            && players == 0
            && now_ms.saturating_sub(self.last_player_seen_ms) >= 900_000
    }
}

fn step_dialogue(
    speaker: PlayerId,
    lines: &[String],
    index: &mut usize,
    ready_at_ms: &mut u64,
    now_ms: u64,
    effects: &mut Vec<EngineEffect>,
) -> bool {
    if lines.is_empty() || *index >= lines.len() {
        return now_ms >= *ready_at_ms;
    }
    if now_ms < *ready_at_ms {
        return false;
    }
    let line = &lines[*index];
    effects.push(EngineEffect::Chat { speaker, text: line.clone() });
    *index += 1;
    *ready_at_ms = now_ms + ((line.len() as u64) * 100).clamp(500, 2_000);
    false
}

/// Reconciles companion/parent `Afk` states to guarantee zero sequential wave deadlocks.
pub fn reconcile_wave_deadlocks(
    members: &mut [(PlayerId, BossTypeId, &mut LifecycleState, bool)],
    now_ms: u64,
    effects: &mut Vec<EngineEffect>,
) {
    let any_fighting = members.iter().any(|(_, _, st, alive)| {
        *alive && matches!(st.status, BossStatus::Active | BossStatus::ChatStart | BossStatus::Die | BossStatus::ChatEnd)
    });
    if any_fighting {
        return;
    }
    let best_order = members
        .iter()
        .filter_map(|(_, _, st, alive)| match (&st.wave_role, *alive && st.status == BossStatus::Afk) {
            (WaveRole::SequentialMember { order, .. }, true) => Some(*order),
            _ => None,
        })
        .min();

    if let Some(target_order) = best_order {
        for (id, _, st, alive) in members.iter_mut() {
            if *alive
                && st.status == BossStatus::Afk
                && matches!(st.wave_role, WaveRole::SequentialMember { order, .. } if order == target_order)
            {
                st.pk_mode = PkMode::PkAll;
                st.transition_to(BossStatus::Active, now_ms);
                effects.push(EngineEffect::ChangePk { entity: *id, mode: PkMode::PkAll });
            }
        }
        return;
    }
    let any_minion = members.iter().any(|(_, _, st, alive)| {
        *alive && matches!(st.appear_type, AppearType::AppearWithAnother | AppearType::CallByAnother) && st.status != BossStatus::Rest
    });
    if !any_minion {
        for (id, _, st, alive) in members.iter_mut() {
            if *alive && st.status == BossStatus::Afk && st.wave_role == WaveRole::MinionGuardParent {
                st.pk_mode = PkMode::PkAll;
                st.transition_to(BossStatus::Active, now_ms);
                effects.push(EngineEffect::ChangePk { entity: *id, mode: PkMode::PkAll });
            }
        }
    }
}
