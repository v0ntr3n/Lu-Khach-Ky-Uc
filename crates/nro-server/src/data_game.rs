//! Binary client asset cache and `DataGame` S2C packet builders (`SRC/data` / `data/`).
//!
//! Ports `DataGame.java`, `Service.setClientType`, and pre-login/in-world binary asset
//! streaming (`-74`, `-87`, `-67`, `-66`, `66`, `-32`, `-77`, `11`, `-82`, `-28`, `-29`, `-41`)
//! so `Mod Local` connects directly to `nro-server` without `20.jar`.

#[path = "data_game_dispatch.rs"]
mod data_game_dispatch;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use dashmap::DashMap;
use nro_db::templates::GameTemplates;
use nro_protocol::{Packet, PacketWriter};
use parking_lot::Mutex;
use tracing::warn;

pub const VS_DATA: i8 = 9;
pub const VS_MAP: i8 = 2;
pub const VS_SKILL: i8 = 1;
pub const VS_ITEM: i8 = 10;
pub const VS_RES: i32 = 1;
pub const MAX_SMALL_VERSION: i16 = 32_767;

/// Maximum UTF-8 byte length written by [`put_utf`], kept under C# signed `short::MAX` (`32,767`).
const MAX_CLIENT_UTF_BYTES: usize = 32_000;

/// Soft byte ceiling for the in-memory binary asset read-through cache (`48 MiB`).
const MAX_ASSET_CACHE_BYTES: usize = 48 * 1024 * 1024;

/// Writes a UTF string into [`PacketWriter`], truncating cleanly to `32,000` bytes
/// so C# `myReader.readUTF()` (signed `short` length) never overflows.
pub(crate) fn put_utf(pw: &mut PacketWriter, text: &str) {
    let slice = if text.len() > MAX_CLIENT_UTF_BYTES {
        let mut end = MAX_CLIENT_UTF_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    } else {
        text
    };
    let _ = pw.write_utf(slice);
}

/// Concurrent bounded read-through cache over `SRC/data` (or `data/`) serving binary client assets.
#[derive(Debug, Clone)]
pub struct DataGameAssets {
    root: PathBuf,
    files: Arc<DashMap<String, Bytes>>,
    files_bytes: Arc<AtomicUsize>,
    cache_evict_lock: Arc<Mutex<()>>,
    res_pack_names: Arc<[Vec<String>; 4]>,
    small_versions: Arc<[Bytes; 4]>,
    bg_item_versions: Arc<[Bytes; 4]>,
}

impl DataGameAssets {
    /// Resolves `configured` (falling back across `SRC/data` and `data`), indexes `res/x1..x4`
    /// filenames without preloading bytes, and precomputes `icon/x1..x4` + `item_bg_temp/x1..x4` version tables.
    #[must_use]
    pub fn open(configured: &Path) -> Self {
        let root = resolve_assets_root(configured);
        let res_pack_names = std::array::from_fn(|idx| load_res_zoom_names(&root, (idx + 1) as u8));
        let small_versions = std::array::from_fn(|idx| build_small_version_table(&root, (idx + 1) as u8));
        let bg_item_versions = std::array::from_fn(|idx| build_bg_item_version_table(&root, (idx + 1) as u8));
        Self {
            root,
            files: Arc::new(DashMap::new()),
            files_bytes: Arc::new(AtomicUsize::new(0)),
            cache_evict_lock: Arc::new(Mutex::new(())),
            res_pack_names: Arc::new(res_pack_names),
            small_versions: Arc::new(small_versions),
            bg_item_versions: Arc::new(bg_item_versions),
        }
    }

    /// Resolved filesystem directory serving binary assets.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns `true` if all 6 `update_data/*` binary blobs required for login & `CreateCharScr` exist.
    #[must_use]
    pub fn has_update_data(&self) -> bool {
        ["dart", "arrow", "effect", "image", "part", "skill"]
            .iter()
            .all(|name| self.root.join("update_data").join(name).is_file())
    }

    /// Returns the number of indexed `res/x{zoom}` files.
    #[must_use]
    pub fn res_pack_count(&self, zoom: u8) -> usize {
        self.res_pack_names[Self::zoom_idx(zoom)].len()
    }

    /// Reads `rel` from the bounded in-memory cache or disk. Never caches `None` misses.
    pub(crate) fn read_cached(&self, rel: &str) -> Option<Bytes> {
        if let Some(hit) = self.files.get(rel) {
            return Some(hit.clone());
        }
        let bytes = Bytes::from(fs::read(self.root.join(rel)).ok()?);
        let sz = bytes.len();
        if sz <= MAX_ASSET_CACHE_BYTES {
            let _guard = self.cache_evict_lock.lock();
            let prev = self.files_bytes.fetch_add(sz, Ordering::Relaxed);
            if prev.saturating_add(sz) > MAX_ASSET_CACHE_BYTES {
                self.files.clear();
                self.files_bytes.store(sz, Ordering::Relaxed);
            }
            if let Some(old) = self.files.insert(rel.to_owned(), bytes.clone()) {
                let _ = self.files_bytes.try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                    Some(cur.saturating_sub(old.len()))
                });
            }
        }
        Some(bytes)
    }

    fn try_read_cached_zoom(&self, zoom: u8, path_for_zoom: &impl Fn(u8) -> String) -> Option<Bytes> {
        let z = zoom.clamp(1, 4);
        let mut tried = [0_u8; 5];
        let mut tried_len = 0_usize;
        for candidate in [z, 2, 1, 3, 4] {
            if tried[..tried_len].contains(&candidate) {
                continue;
            }
            tried[tried_len] = candidate;
            tried_len += 1;
            let rel = path_for_zoom(candidate);
            if let Some(bytes) = self.read_cached(&rel) {
                return Some(bytes);
            }
        }
        None
    }

    /// Reads a zoom-tiered asset trying `[z, 2, 1, 3, 4]` (skipping duplicates) and logs a warning on miss.
    pub(crate) fn read_cached_zoom(
        &self,
        zoom: u8,
        path_for_zoom: impl Fn(u8) -> String,
    ) -> Option<Bytes> {
        if let Some(bytes) = self.try_read_cached_zoom(zoom, &path_for_zoom) {
            return Some(bytes);
        }
        let z = zoom.clamp(1, 4);
        warn!(
            zoom = z,
            primary_path = %path_for_zoom(z),
            assets_dir = %self.root.display(),
            "missing zoomed binary asset across all zoom levels [1..=4]"
        );
        None
    }

    #[inline]
    pub(crate) fn zoom_idx(zoom: u8) -> usize {
        usize::from(zoom.clamp(1, 4) - 1)
    }

    /// `-67`: `DataGame.sendIcon` (`icon/x{zoom}/{id}.png`).
    #[must_use]
    pub fn pkt_icon(&self, zoom: u8, id: i32) -> Option<Packet> {
        let icon = self.read_cached_zoom(zoom, |z| format!("icon/x{z}/{id}.png"))?;
        let mut pw = PacketWriter::with_capacity(8 + icon.len());
        pw.write_i32(id);
        pw.write_i32(icon.len() as i32);
        pw.write_bytes(&icon);
        Some(pw.finish(-67))
    }

    /// `66`: `DataGame.sendImageByName` (`img_by_name/x{zoom}/{safe_name}.png`).
    #[must_use]
    pub fn pkt_img_by_name(&self, tpl: &GameTemplates, zoom: u8, img_name: &str) -> Packet {
        let n_frame = tpl.images_by_name.get(img_name).copied().unwrap_or(0);
        let data = is_safe_asset_stem(img_name)
            .then(|| self.read_cached_zoom(zoom, |z| format!("img_by_name/x{z}/{img_name}.png")))
            .flatten();
        let mut pw = PacketWriter::with_capacity(16 + img_name.len() + data.as_ref().map_or(0, Bytes::len));
        put_utf(&mut pw, img_name);
        pw.write_u8(n_frame);
        match data {
            Some(bytes) => {
                pw.write_i32(bytes.len() as i32);
                pw.write_bytes(&bytes);
            }
            None => pw.write_i32(0),
        }
        pw.finish(66)
    }

    /// `-66`: `DataGame.sendEffectTemplate` (`effdata/x{z}/DataEffect_{id_t}` -> `effdata/DataEffect_{id_t}`
    /// + `effect/x{zoom}/ImgEffect_{id_t}.png`).
    #[must_use]
    pub fn pkt_effect_template(
        &self,
        zoom: u8,
        _client_version: i32,
        eff_id: i16,
        id_temp: i16,
    ) -> Option<Packet> {
        let eff_data = self
            .try_read_cached_zoom(zoom, &|z| format!("effdata/x{z}/DataEffect_{id_temp}"))
            .or_else(|| self.read_cached(&format!("effdata/DataEffect_{id_temp}")));
        let Some(eff_data) = eff_data else {
            warn!(
                zoom = zoom.clamp(1, 4),
                id_temp,
                assets_dir = %self.root.display(),
                "missing effdata asset across zoomed and unzoomed paths"
            );
            return None;
        };
        let eff_img = self.read_cached_zoom(zoom, |z| format!("effect/x{z}/ImgEffect_{id_temp}.png"))?;
        let mut pw = PacketWriter::with_capacity(12 + eff_data.len() + eff_img.len());
        pw.write_i16(eff_id);
        pw.write_i32(eff_data.len() as i32);
        pw.write_bytes(&eff_data);
        pw.write_i8(if id_temp == 60 { 2 } else { 0 });
        pw.write_i32(eff_img.len() as i32);
        pw.write_bytes(&eff_img);
        Some(pw.finish(-66))
    }

    /// `-32`: `DataGame.sendItemBGTemplate` (`item_bg_temp/x{zoom}/{id}.png`).
    #[must_use]
    pub fn pkt_item_bg_template(&self, zoom: u8, id: i16) -> Option<Packet> {
        let bg = self.read_cached_zoom(zoom, |z| format!("item_bg_temp/x{z}/{id}.png"))?;
        let mut pw = PacketWriter::with_capacity(6 + bg.len());
        pw.write_i16(id);
        pw.write_i32(bg.len() as i32);
        pw.write_bytes(&bg);
        Some(pw.finish(-32))
    }

    /// `-77`: `DataGame.sendSmallVersion` populated from actual `icon/x{zoom}/{id}.png` file lengths (`len % 127`).
    #[must_use]
    pub fn pkt_small_version_for_zoom(&self, zoom: u8) -> Packet {
        Packet::new(-77, self.small_versions[Self::zoom_idx(zoom)].clone())
    }

    /// `-93`: `DataGame.sendBgItemVersion` populated from actual `item_bg_temp/x{zoom}/{id}.png` file lengths (`len % 127`).
    #[must_use]
    pub fn pkt_bg_item_version_for_zoom(&self, zoom: u8) -> Packet {
        Packet::new(-93, self.bg_item_versions[Self::zoom_idx(zoom)].clone())
    }

    /// `11`: `DataGame.requestMobTemplate` (`mob/x{zoom}/{mob_id}`).
    #[must_use]
    pub fn pkt_mob_template(&self, zoom: u8, mob_id: i16) -> Option<Packet> {
        let mob = self.read_cached_zoom(zoom, |z| format!("mob/x{z}/{mob_id}"))?;
        let mut pw = PacketWriter::with_capacity(2 + mob.len());
        pw.write_i16(mob_id);
        pw.write_bytes(&mob);
        Some(pw.finish(11))
    }

    /// `-82`: `DataGame.sendTileSetInfo` (`map/tile_set_info`).
    #[must_use]
    pub fn pkt_tile_set_info(&self) -> Option<Packet> {
        self.read_cached("map/tile_set_info").map(|b| Packet::new(-82, b))
    }

    /// `-28` subcommand `10`: `DataGame.sendMapTemp` (`map/tile_map_data/{map_id}`).
    #[must_use]
    pub fn pkt_map_temp(&self, map_id: u8) -> Option<Packet> {
        let data = self.read_cached(&format!("map/tile_map_data/{map_id}"))?;
        let mut pw = PacketWriter::with_capacity(1 + data.len());
        pw.write_i8(10);
        pw.write_bytes(&data);
        Some(pw.finish(-28))
    }

    /// Reads the horizontal pixel width (`tmw * 24`) of `map_id` from `map/tile_map_data/{map_id}`.
    #[must_use]
    pub fn map_pixel_width(&self, map_id: i16) -> i16 {
        if let Ok(id_u8) = u8::try_from(map_id)
            && let Some(data) = self.read_cached(&format!("map/tile_map_data/{id_u8}"))
            && let Some(&tmw) = data.first()
            && tmw > 0
        {
            return i16::from(tmw).saturating_mul(24).max(1_200);
        }
        4_096
    }
}

fn resolve_assets_root(configured: &Path) -> PathBuf {
    for c in [
        configured,
        Path::new("SRC/data"),
        Path::new("Copy of Teamobi2026/SRC/data"),
        Path::new("Teamobi2026/SRC/data"),
        Path::new("../SRC/data"),
        Path::new("../Teamobi2026/SRC/data"),
        Path::new("../Copy of Teamobi2026/SRC/data"),
        Path::new("data"),
    ] {
        if c.join("update_data").is_dir() || c.join("icon").is_dir() {
            return c.to_path_buf();
        }
    }
    configured.to_path_buf()
}

fn is_safe_asset_stem(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\') && !name.contains('\0')
}

fn build_small_version_table(root: &Path, zoom: u8) -> Bytes {
    let mut raw = vec![0_u8; 2 + MAX_SMALL_VERSION as usize];
    raw[..2].copy_from_slice(&MAX_SMALL_VERSION.to_be_bytes());
    if let Ok(rd) = fs::read_dir(root.join(format!("icon/x{zoom}"))) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) != Some("png") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            let Ok(id) = stem.parse::<usize>() else {
                continue;
            };
            if id < MAX_SMALL_VERSION as usize
                && let Ok(meta) = entry.metadata()
            {
                raw[2 + id] = (meta.len() % 127) as u8;
            }
        }
    }
    Bytes::from(raw)
}

fn build_bg_item_version_table(root: &Path, zoom: u8) -> Bytes {
    let mut entries: Vec<(usize, u8)> = Vec::new();
    let mut max_disk_id = 0_usize;
    if let Ok(rd) = fs::read_dir(root.join(format!("item_bg_temp/x{zoom}"))) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) != Some("png") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            let Ok(id) = stem.parse::<usize>() else {
                continue;
            };
            if id < (i16::MAX as usize)
                && let Ok(meta) = entry.metadata()
            {
                max_disk_id = max_disk_id.max(id);
                entries.push((id, (meta.len() % 127) as u8));
            }
        }
    }
    let len = (max_disk_id.max(1_024) + 1).min(i16::MAX as usize);
    let mut raw = vec![0_u8; 2 + len];
    raw[..2].copy_from_slice(&(len as i16).to_be_bytes());
    for (id, ver) in entries {
        if id < len {
            raw[2 + id] = ver;
        }
    }
    Bytes::from(raw)
}

fn load_res_zoom_names(root: &Path, zoom: u8) -> Vec<String> {
    let Ok(rd) = fs::read_dir(root.join(format!("res/x{zoom}"))) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            Some(e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}
