//! Compatibility re-exports mapping legacy [`DbPool`] and [`DbPoolConfig`] names onto [`crate::engine::FjallEngine`].

pub use crate::engine::{
    DEFAULT_WOMENS_DAY_JSON, FjallConfig as DbPoolConfig, FjallEngine as DbPool,
    SchemaHealthReport,
};
