//! Complete game constants extracted verbatim from `nro/models/consts/*` and `BossID.java`.
#![forbid(unsafe_code)]

pub mod boss_i_d;
pub mod cmd_message;
pub mod const_account;
pub mod const_achievement;
pub mod const_daily_gift;
pub mod const_event;
pub mod const_item;
pub mod const_map;
pub mod const_mob;
pub mod const_npc;
pub mod const_player;
pub mod const_ratio;
pub mod const_task;
pub mod const_task_badges;
pub mod const_tournament;

pub use const_map::MAP_NORMAL;
pub use const_ratio::PER100;

/// Default TeaMobi TCP server port (`ServerManager.PORT = 14445`).
pub const DEFAULT_SERVER_PORT: u16 = 14445;
/// Dirty-flag leaderboard refresh cadence (`3000 ms` in `ServerManager.java`).
pub const LEADERBOARD_REFRESH_MS: u64 = 3_000;
/// Authoritative boss AI & spawn supervisor tick interval (`150 ms`).
pub const BOSS_TICK_MS: u64 = 150;
/// Martial arts tournament (`Đại Hội Võ Thuật`) bracket tick interval (`500 ms`).
pub const TOURNAMENT_TICK_MS: u64 = 500;
/// Mini-game lottery (`Chọn Ai Đây` / `Con Số May Mắn`) tick interval (`1000 ms`).
pub const LOTTERY_TICK_MS: u64 = 1_000;
