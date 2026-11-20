//! Zero-MySQL streaming importer for `database team2026.sql` and `SRC/sql/nro1.sql`.

pub mod dump_parser;
pub mod importer;
pub(crate) mod template_builder;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use nro_core::ParityMode;
use serde::{Deserialize, Serialize};

use crate::codec::decode_val;
use crate::engine::{FjallEngine, IMPORT_MANIFEST_KEY, TEMPLATES_SNAPSHOT_KEY};
use crate::templates::GameTemplates;
use crate::world_repos::SuperRankRepo;
use crate::DbResult;

pub use importer::import_mysqldump_str;

#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    pub parity_mode: ParityMode,
    pub upgrade_passwords: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            parity_mode: ParityMode::Fixed,
            upgrade_passwords: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportManifest {
    pub source_bytes: u64,
    pub total_rows: u64,
    pub table_counts: BTreeMap<String, u64>,
    pub imported_at_utc: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub manifest: Option<ImportManifest>,
    pub item_templates: usize,
    pub map_templates: usize,
    pub shop_templates: usize,
    pub accounts_count: usize,
    pub players_count: usize,
    pub super_rank_highest: i32,
}

pub fn import_mysqldump_file(
    engine: &FjallEngine,
    sql_path: &Path,
    opts: ImportOptions,
) -> DbResult<ImportManifest> {
    let raw_sql = fs::read_to_string(sql_path)?;
    import_mysqldump_str(engine, &raw_sql, opts)
}

pub async fn verify_fjall_store(engine: &FjallEngine) -> DbResult<VerificationReport> {
    let manifest: Option<ImportManifest> = engine
        .ks()
        .meta
        .get(IMPORT_MANIFEST_KEY)?
        .map(|b| decode_val("manifest", &b))
        .transpose()?;
    let has_tpl = engine.ks().templates.contains_key(TEMPLATES_SNAPSHOT_KEY)?;
    let templates = if has_tpl {
        GameTemplates::load_all(engine).await?
    } else {
        GameTemplates::default()
    };
    let accounts_count = engine.ks().accounts.len()?;
    let players_count = engine.ks().players.len()?;
    let super_rank_highest = SuperRankRepo::new(engine.clone()).highest_rank().await?;

    Ok(VerificationReport {
        manifest,
        item_templates: templates.items.len(),
        map_templates: templates.maps.len(),
        shop_templates: templates.shops.len(),
        accounts_count,
        players_count,
        super_rank_highest,
    })
}
