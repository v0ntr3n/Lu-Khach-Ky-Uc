#![forbid(unsafe_code)]
//! Library surface of `nro-server`, exposing `ServerRuntime`, `ServerConfig`, and `AdminControl`
//! for both the headless `nro-server` CLI binary and the embedded `nro-admin-ui` desktop panel.

pub mod admin_control;
pub mod config;
pub mod data_game;
pub mod data_game_tpl;
pub mod pgo_bench;
pub mod router;
pub mod runtime;
pub mod state;
