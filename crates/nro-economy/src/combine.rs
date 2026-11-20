//! Data-driven and rule-based crafting engine porting all 32 `models/combine/` files.
//!
//! Covers all 25 `CombineService` tab operations, 3 NPC-menu combines (`CheTaoCuonSachCu`,
//! `DoiSachTuyetKy` unsealed/sealed, `HocTuyetKy`), unwired `LamPhepNhapDa` (`512`) and
//! `PhanRaDoThanLinh` (`514`), plus `Combine`, `CombineSystem`, and `RandomCollection` curves.

use crate::inventory::{Inventory, InventoryError, add_opt_param, get_opt, has_skh_option, opt_id, set_opt};
use crate::ItemCatalog;
use nro_consts::{const_item, const_task_badges};
use nro_core::{GameRng, Gender, ItemInstance, ItemOption, ParityMode, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

/// Maximum crystal star sockets (`CombineService.MAX_STAR_ITEM`).
pub const MAX_STAR_ITEM: i32 = 9;
/// Maximum equipment upgrade level (`CombineService.MAX_LEVEL_ITEM`).
pub const MAX_LEVEL_ITEM: i32 = 8;

const PHA_LE_GOLD: [u64; 9] = [5_000_000, 10_000_000, 20_000_000, 40_000_000, 60_000_000, 90_000_000, 120_000_000, 200_000_000, 300_000_000];
const PHA_LE_UI_BPS: [u32; 9] = [8000, 4000, 3000, 2000, 1000, 500, 300, 200, 100];
const PHA_LE_REAL_BPS: [u32; 9] = [5000, 2000, 1000, 500, 100, 70, 50, 10, 10];
const NANG_CAP_STONES: [u32; 8] = [3, 7, 11, 17, 23, 35, 50, 70];
const NANG_CAP_GOLD: [u64; 8] = [10_000, 70_000, 300_000, 1_500_000, 7_000_000, 23_000_000, 100_000_000, 250_000_000];
const NANG_CAP_BPS: [u32; 8] = [8000, 5000, 2000, 1000, 700, 500, 100, 30];
const PORATA_POOL: [u16; 7] = [77, 80, 81, 103, 50, 94, 5];
const BOOK_POOL: [u16; 12] = [77, 103, 50, 108, 94, 14, 80, 81, 175, 5, 214, 216];

struct FixedRecipe(CombineOp, (i16, i16, u32, u32), Option<(i16, u32)>, u64, u32, (i16, i16, u32, bool));

const FIXED_RECIPES: &[FixedRecipe] = &[
    FixedRecipe(CombineOp::TaoDaHematite, (441, 447, 5, 5), None, 50_000_000, 10_000, (1423, 1423, 1, false)),
    FixedRecipe(CombineOp::CheTaoDuiDuc, (1423, 1423, 5, 5), None, 50_000_000, 10_000, (1438, 1438, 1, false)),
    FixedRecipe(CombineOp::TaoDaMai, (1438, 1438, 5, 5), None, 50_000_000, 10_000, (1439, 1439, 1, false)),
    FixedRecipe(CombineOp::LamPhepNhapDa, (225, 225, 10, 10), Some((226, 1)), 10_000_000, 8_000, (220, 224, 1, false)),
    FixedRecipe(CombineOp::PhanRaDoThanLinh, (555, 567, 1, 1), None, 10_000_000, 10_000, (1423, 1423, 1, false)),
    FixedRecipe(CombineOp::TaiTaoCapsuleKichHoat, (1656, 1656, 3, 3), Some((1634, 1)), 2_000_000_000, 10_000, (1655, 1655, 1, true)),
    FixedRecipe(CombineOp::CheTaoCuonSachCu, (1281, 1281, 9_999, 99), Some((1282, 1)), 0, 2_000, (1283, 1283, 1, true)),
];

/// Every combine recipe operation from `models/combine/` (`repr(i32)` matches `CombineService` IDs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, FromRepr, Serialize, Deserialize)]
#[repr(i32)]
pub enum CombineOp {
    /// `NangCapSaoPhaLe` (`100`): SPL Lv1 + Hematite -> SPL Lv2 (`50%`).
    NangCapSaoPhaLe = 100,
    /// `DanhBongSaoPhaLe` (`101`): 2× SPL Lv2 + Đá mài -> SPL Lấp lánh (`100%`).
    DanhBongSaoPhaLe = 101,
    /// `CuongHoaLoSaoPhaLe` (`102`): Reinforce 8th/9th star socket (`Opt 228`).
    CuongHoaLoSaoPhaLe = 102,
    /// `TaoDaHematite` (`103`): 5× SPL Lv1 -> 1× `1423` Hematite.
    TaoDaHematite = 103,
    /// `GiamDinhSach` (`104`): Appraise `Opt 217` on Sách Tuyệt Kỹ using `1284`.
    GiamDinhSach = 104,
    /// `TaySach` (`105`): Reset appraised book options using `Opt 219` charge.
    TaySach = 105,
    /// `NangCapSachTuyetKy` (`106`): STK 1 + 10× `1285` -> STK 2 (`10%`).
    NangCapSachTuyetKy = 106,
    /// `HoiPhucSach` (`107`): Restore book durability (`Opt 212` -> `1000`).
    HoiPhucSach = 107,
    /// `PhanRaSach` (`108`): Disassemble STK 1/2 -> 5× `1283` (deducts `10M` Gold).
    PhanRaSach = 108,
    /// `CheTaoDuiDuc` (`109`): 5× `1423` Hematite -> 1× `1438` Dùi đục.
    CheTaoDuiDuc = 109,
    /// `TaoDaMai` (`110`): 5× `1438` Dùi đục -> 1× `1439` Đá mài.
    TaoDaMai = 110,
    /// `NangCapBongTai3` (`455`): `921` + `1820[Opt 31>=20000]` -> `1819` Porata Lv3 (`50%`).
    NangCapBongTai3 = 455,
    /// `NangChiSoBongTai3` (`457`): `1819` + `934[Opt 31>=99]` + `935` -> 2 stats (`30%`).
    NangChiSoBongTai3 = 457,
    /// `EpSaoTrangBi` (`500`): Socket gem/SPL into drilled equipment (`10` Gem).
    EpSaoTrangBi = 500,
    /// `PhaLeHoaTrangBi` (`501`): Drill empty star socket `0..=9` (`Opt 107`).
    PhaLeHoaTrangBi = 501,
    /// `NangCapVatPham` (`510`): Upgrade equipment `+0 -> +8` (`Opt 72`).
    NangCapVatPham = 510,
    /// `NangCapBongTai` (`511`): `454` + `933[Opt 31>=9999]` -> `921` Porata Lv2 (`50%`).
    NangCapBongTai = 511,
    /// `LamPhepNhapDa` (`512`): `10× 225` + `1× 226` -> random stone `220..=224` (`80%`).
    LamPhepNhapDa = 512,
    /// `NhapNgocRong` (`513`): `7×` Dragon Ball `15..=20` -> `1×` `(id - 1)`.
    NhapNgocRong = 513,
    /// `PhanRaDoThanLinh` (`514`): Disassemble clean Thần Linh (`555..=567`) -> `1× 1423`.
    PhanRaDoThanLinh = 514,
    /// `CheTaoTrangBiThienSu` (`515`): Craft Angel gear `1048..=1062` (never runs in preview!).
    CheTaoTrangBiThienSu = 515,
    /// `NangChiSoBongTai` (`517`): `921` + `934[Opt 31>=200]` + `935` -> 1 stat (`45%`).
    NangChiSoBongTai = 517,
    /// `ChuyenHoaTrangBiVang` (`994`): Transfer `+4..=+8` onto clean Thần Linh for `2B` Gold.
    ChuyenHoaTrangBiVang = 994,
    /// `ChuyenHoaTrangBiNgoc` (`995`): Transfer `+4..=+8` onto clean Thần Linh for `5,000` Gem.
    ChuyenHoaTrangBiNgoc = 995,
    /// `PhanRaTrangBiKichHoat` (`996`): Disassemble SKH gear -> `1× 1656` (`2B` Gold).
    PhanRaTrangBiKichHoat = 996,
    /// `TaiTaoCapsuleKichHoat` (`997`): `3× 1656` + `1× 1634` -> `1× 1655` (`2B` Gold).
    TaiTaoCapsuleKichHoat = 997,
    /// `HocTuyetKy` (`9990`): Learn/upgrade Whis Ultimate Skill (`1229` ×9999 / ×999).
    HocTuyetKy = 9990,
    /// `CheTaoCuonSachCu` (`19787`): `9999× 1281` + `1× 1282` -> `1× 1283` (`20%`).
    CheTaoCuonSachCu = 19787,
    /// `DoiSachTuyetKy` (`28777`): `10× 1283` + `1× 1285` -> STK 1 (`20%`).
    DoiSachTuyetKy = 28777,
    /// `DoiSachTuyetKySealed` (`28778`): `10× 1283` + `1× 1285` + `1× 1794` Con dấu -> STK 1 (`100%`).
    DoiSachTuyetKySealed = 28778,
}

/// Player state context needed by recipe validators and Whis/Angel gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombineContext {
    /// Player planet.
    pub gender: Gender,
    /// Player power (`nPoint.power`).
    pub power: i64,
    /// True if wearing all 5 Hủy Diệt pieces (`checkSetDes()`).
    pub wearing_full_destroyer: bool,
    /// Current special skill level (`0..=9`) for `HocTuyetKy`.
    pub skill_point: u8,
    /// Current special skill mastery (`0..=1000`) for `HocTuyetKy`.
    pub skill_mastery: u16,
    /// Controls legacy probability quirks vs. hardened fixes.
    pub parity: ParityMode,
}

impl CombineContext {
    /// Default hardened context for tests and standalone combine checks.
    #[must_use]
    pub const fn default_hardened() -> Self {
        Self { gender: Gender::TraiDat, power: 100_000_000_000, wearing_full_destroyer: true, skill_point: 0, skill_mastery: 1_000, parity: ParityMode::Fixed }
    }
}

/// Read-only quote produced by [`CombineService::preview`] (never mutates inventory!).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinePreview {
    /// Operation quoted.
    pub op: CombineOp,
    /// Gold required.
    pub gold_cost: u64,
    /// Gems required.
    pub gem_cost: u32,
    /// UI-displayed success rate in basis points (`10_000 = 100.00%`).
    pub ui_rate_bps: u32,
    /// Server execution success rate in basis points (`10_000 = 100.00%`).
    pub server_rate_bps: u32,
}

/// Result of executing a combine recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombineOutcome {
    /// Whether the RNG roll (or deterministic craft) succeeded.
    pub success: bool,
    /// Number of attempts executed (`1` normally, `1..=100` in `execute_vip_batch`).
    pub attempts: u32,
    /// Optional badge ID incremented (e.g. `THANH_DAP_DO_7` on `+7 -> +8`).
    pub badge_awarded: Option<i8>,
    /// Optional new skill point level on `HocTuyetKy`.
    pub new_skill_point: Option<u8>,
}

/// Errors returned when combine inputs, caps, or balances fail validation.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CombineError {
    /// Recipe inputs did not match required item templates, types, or option predicates.
    #[error("invalid recipe inputs for {0}: {1}")]
    InvalidInputs(CombineOp, &'static str),
    /// Target item already reached star/level/skill cap.
    #[error("{0} reached maximum cap")]
    CapReached(CombineOp),
    /// Player already owns unique output item (e.g. `921` in bag during `NangCapBongTai`).
    #[error("unique output item {0:?} already present in bag")]
    AlreadyOwned(TemplateId),
    /// Underlying inventory slot, space, or currency error.
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Stateless combine recipe preview and execution service.
pub struct CombineService;

impl CombineService {
    /// Computes a non-mutating [`CombinePreview`] after verifying disjoint bag slot indices.
    pub fn preview(inv: &Inventory, op: CombineOp, slots: &[usize], ctx: &CombineContext, catalog: &dyn ItemCatalog) -> Result<CombinePreview, CombineError> {
        let it = inv.validate_disjoint_bag_slots(slots)?;
        if let Some(fr) = FIXED_RECIPES.iter().find(|r| r.0 == op) {
            let ok1 = it.iter().any(|x| (fr.1.0..=fr.1.1).contains(&x.template_id.0) && x.quantity >= fr.1.2);
            let ok2 = fr.2.is_none_or(|(id2, q2)| it.iter().any(|x| x.template_id.0 == id2 && x.quantity >= q2));
            if it.len() != (if fr.2.is_some() { 2 } else { 1 }) || !ok1 || !ok2 { return Err(CombineError::InvalidInputs(op, "fixed recipe mismatch")); }
            return Ok(CombinePreview { op, gold_cost: fr.3, gem_cost: 0, ui_rate_bps: fr.4, server_rate_bps: fr.4 });
        }
        let is_eq = |i: &ItemInstance| { let t = catalog.meta_of(i.template_id).item_type; t < 5 || t == 32 };
        let has = |id: i16, q: u32| it.iter().any(|x| x.template_id.0 == id && x.quantity >= q);
        let is_stk = |id: i16| matches!(id, 1044 | 1211 | 1212 | 1278..=1280);
        let bad = |msg| CombineError::InvalidInputs(op, msg);

        let (gold_cost, gem_cost, ui_rate_bps, server_rate_bps) = match op {
            CombineOp::EpSaoTrangBi => {
                let eq = *it.iter().find(|x| it.len() == 2 && is_eq(x)).ok_or(bad("need equip + gem"))?;
                let gm = *it.iter().find(|x| !std::ptr::eq(**x, eq)).ok_or(bad("no gem"))?;
                let (s, m) = (get_opt(eq, opt_id::SOCKETED_STARS).unwrap_or(0), get_opt(eq, opt_id::MAX_STAR_SOCKETS).unwrap_or(0));
                if !(catalog.meta_of(gm.template_id).item_type == 30 || (14..=20).contains(&gm.template_id.0)) || (s >= 7 && get_opt(eq, opt_id::REINFORCED_STAR_SLOT).unwrap_or(0) <= s) { return Err(bad("socket ineligible")); }
                if s >= m || s >= MAX_STAR_ITEM { return Err(CombineError::CapReached(op)); }
                (0, 10, 10_000, 10_000)
            }
            CombineOp::PhaLeHoaTrangBi => {
                if it.len() != 1 || !is_eq(it[0]) { return Err(bad("need 1 equip")); }
                let st = get_opt(it[0], 107).unwrap_or(0);
                if st >= MAX_STAR_ITEM { return Err(CombineError::CapReached(op)); }
                let u = st.max(0) as usize; (PHA_LE_GOLD[u], (u as u32) + 1, PHA_LE_UI_BPS[u], PHA_LE_REAL_BPS[u])
            }
            CombineOp::NangCapVatPham => {
                let eq = *it.iter().find(|x| (2..=3).contains(&it.len()) && catalog.meta_of(x.template_id).item_type < 5).ok_or(bad("no equip"))?;
                let lv = get_opt(eq, 72).unwrap_or(0);
                if lv >= MAX_LEVEL_ITEM { return Err(CombineError::CapReached(op)); }
                let u = lv.max(0) as usize;
                if !has([223i16, 222, 224, 221, 220][catalog.meta_of(eq.template_id).item_type as usize], NANG_CAP_STONES[u]) { return Err(bad("missing stones")); }
                (NANG_CAP_GOLD[u], 0, NANG_CAP_BPS[u], NANG_CAP_BPS[u])
            }
            CombineOp::NangCapBongTai | CombineOp::NangCapBongTai3 => {
                let (b, sh, req) = if op == CombineOp::NangCapBongTai { (454, 933, 9_999) } else { (921, 1820, 20_000) };
                if op == CombineOp::NangCapBongTai && inv.find_bag_slot(TemplateId(const_item::BONG_TAI_PORATA_CAP_2 as i16)).is_some() { return Err(CombineError::AlreadyOwned(TemplateId(921))); }
                if it.len() != 2 || !has(b, 1) || !it.iter().any(|x| x.template_id.0 == sh && get_opt(x, 31).unwrap_or(0) >= req) { return Err(bad("porata inputs")); }
                (200_000_000, 1_000, 5_000, 5_000)
            }
            CombineOp::NangChiSoBongTai | CombineOp::NangChiSoBongTai3 => {
                let (e, req, bps) = if op == CombineOp::NangChiSoBongTai { (921, 200, 4_500) } else { (1819, 99, 3_000) };
                if it.len() != 3 || !has(e, 1) || !has(935, 1) || !it.iter().any(|x| x.template_id.0 == 934 && get_opt(x, 31).unwrap_or(0) >= req) { return Err(bad("porata stat")); }
                (0, 1_000, bps, bps)
            }
            CombineOp::NhapNgocRong => if it.len() == 1 && (15..=20).contains(&it[0].template_id.0) && it[0].quantity >= 7 { (0, 0, 10_000, 10_000) } else { return Err(bad("need 7× ball")); },
            CombineOp::CheTaoTrangBiThienSu => {
                if !ctx.wearing_full_destroyer || !(2..=4).contains(&it.len()) || !it.iter().any(|x| matches!(x.template_id.0, 1071..=1073 | 1084..=1086)) || !it.iter().any(|x| (1066..=1070).contains(&x.template_id.0) && x.quantity >= 999) { return Err(bad("angel")); }
                let b = it.iter().find(|x| (1074..=1078).contains(&x.template_id.0)).map_or(0, |x| ((x.template_id.0 - 1073) as u32) * 100);
                (10_000_000, 0, 9_000 + b, 9_000 + b)
            }
            CombineOp::ChuyenHoaTrangBiVang | CombineOp::ChuyenHoaTrangBiNgoc => {
                let dst = *it.iter().find(|x| it.len() == 2 && (555..=567).contains(&x.template_id.0) && get_opt(x, 72).is_none() && get_opt(x, 102).is_none()).ok_or(bad("no clean Thần Linh"))?;
                let src = *it.iter().find(|x| !std::ptr::eq(**x, dst)).ok_or(bad("no source"))?;
                let (lv, ms, md) = (get_opt(src, 72).unwrap_or(0), catalog.meta_of(src.template_id), catalog.meta_of(dst.template_id));
                if !(4..=8).contains(&lv) || has_skh_option(src) || ms.item_type != md.item_type || (ms.gender.is_some() && ms.gender != md.gender) || (op == CombineOp::ChuyenHoaTrangBiNgoc && (555..=567).contains(&src.template_id.0)) { return Err(bad("transfer pair")); }
                if op == CombineOp::ChuyenHoaTrangBiVang { (2_000_000_000, 0, 10_000, 10_000) } else { (0, 5_000, 10_000, 10_000) }
            }
            CombineOp::PhanRaTrangBiKichHoat => if it.len() == 1 && has_skh_option(it[0]) { (2_000_000_000, 0, 10_000, 10_000) } else { return Err(bad("need SKH")); },
            CombineOp::NangCapSaoPhaLe => if it.len() == 2 && has(1423, 1) && it.iter().any(|x| (441..=447).contains(&x.template_id.0)) { (200_000_000, 10, 5_000, 5_000) } else { return Err(bad("SPL1+1423")); },
            CombineOp::DanhBongSaoPhaLe => if it.len() == 2 && has(1439, 1) && it.iter().any(|x| (1416..=1422).contains(&x.template_id.0) && x.quantity >= 2) { (100_000_000, 0, 10_000, 10_000) } else { return Err(bad("2×SPL2+1439")); },
            CombineOp::CuongHoaLoSaoPhaLe => {
                let eq = *it.iter().find(|x| it.len() == 3 && has(1423, 1) && has(1438, 1) && !matches!(x.template_id.0, 1423 | 1438)).ok_or(bad("need equip+1423+1438"))?;
                let (ms, sk, rf) = (get_opt(eq, 107).unwrap_or(0), get_opt(eq, 102).unwrap_or(0), get_opt(eq, 228).unwrap_or(0));
                if !is_eq(eq) || !(8..=9).contains(&ms) || sk < 7 || rf >= ms { return Err(bad("socket ineligible")); }
                (500_000_000, 0, 5_000, match (ctx.parity, rf == 8) { (ParityMode::Legacy, false) => 10_000, (ParityMode::Legacy, true) => 2_500, (ParityMode::Fixed, _) => 5_000 })
            }
            CombineOp::DoiSachTuyetKy | CombineOp::DoiSachTuyetKySealed => {
                let s = op == CombineOp::DoiSachTuyetKySealed;
                if it.len() != (if s { 3 } else { 2 }) || !has(1283, 10) || !has(1285, 1) || (s && !has(1794, 1)) { return Err(bad("book exchange")); }
                let r = if s { 10_000 } else { 2_000 }; (0, 0, r, r)
            }
            CombineOp::GiamDinhSach => if it.len() == 2 && has(1284, 1) && it.iter().any(|x| is_stk(x.template_id.0) && get_opt(x, 217).is_some()) { (0, 0, 10_000, 10_000) } else { return Err(bad("appraise")); },
            CombineOp::TaySach => if it.len() == 1 && is_stk(it[0].template_id.0) && get_opt(it[0], 219).unwrap_or(0) > 0 && get_opt(it[0], 217).is_none() { (0, 0, 10_000, 10_000) } else { return Err(bad("reset")); },
            CombineOp::NangCapSachTuyetKy => if it.len() == 2 && has(1285, 10) && it.iter().any(|x| matches!(x.template_id.0, 1044 | 1211 | 1212)) { (0, 0, 1_000, 1_000) } else { return Err(bad("book upgrade")); },
            CombineOp::HoiPhucSach => {
                if it.len() != 1 || !is_stk(it[0].template_id.0) { return Err(bad("need STK")); }
                let db = get_opt(it[0], 212).unwrap_or(1_000);
                if db >= 1_000 { return Err(CombineError::CapReached(op)); }
                (0, (((1_000 - db) * 50) / 1_000).max(5) as u32, 10_000, 10_000)
            }
            CombineOp::PhanRaSach => if it.len() == 1 && is_stk(it[0].template_id.0) { (10_000_000, 0, 10_000, 10_000) } else { return Err(bad("need STK")); },
            CombineOp::HocTuyetKy => {
                if ctx.skill_point >= 9 { return Err(CombineError::CapReached(op)); }
                if ctx.power < 60_000_000_000 || (ctx.skill_point > 0 && ctx.skill_mastery < 1_000) || it.len() != 1 || !has(1229, if ctx.skill_point == 0 { 9_999 } else { 999 }) { return Err(bad("whis skill")); }
                let r = if ctx.skill_point == 0 { 10_000 } else { 333 }; (10_000_000, 99, r, r)
            }
            _ => unreachable!("covered by FIXED_RECIPES"),
        };
        Ok(CombinePreview { op, gold_cost, gem_cost, ui_rate_bps, server_rate_bps })
    }

    /// Executes batch `PhaLeHoaTrangBi` (`startCombineVip` `x1`, `x10`, `x100`), stopping on first star gain.
    pub fn execute_vip_batch(inv: &mut Inventory, slot: usize, max_rolls: u8, ctx: &CombineContext, catalog: &dyn ItemCatalog, rng: &mut GameRng) -> Result<CombineOutcome, CombineError> {
        let mut attempts = 0;
        for _ in 0..max_rolls.clamp(1, 100) {
            attempts += 1;
            if Self::execute(inv, CombineOp::PhaLeHoaTrangBi, &[slot], ctx, catalog, rng)?.success { return Ok(CombineOutcome { success: true, attempts, badge_awarded: None, new_skill_point: None }); }
        }
        Ok(CombineOutcome { success: false, attempts, badge_awarded: None, new_skill_point: None })
    }

    /// Executes a validated combine recipe and mutates `inv` atomically.
    pub fn execute(inv: &mut Inventory, op: CombineOp, slots: &[usize], ctx: &CombineContext, catalog: &dyn ItemCatalog, rng: &mut GameRng) -> Result<CombineOutcome, CombineError> {
        let q = Self::preview(inv, op, slots, ctx, catalog)?;
        if inv.gold < q.gold_cost { return Err(InventoryError::InsufficientCurrency("gold", q.gold_cost, inv.gold).into()); }
        if inv.gem < q.gem_cost { return Err(InventoryError::InsufficientCurrency("gem", u64::from(q.gem_cost), u64::from(inv.gem)).into()); }
        if op != CombineOp::PhaLeHoaTrangBi { inv.sub_gold(q.gold_cost)?; inv.sub_gem(q.gem_cost)?; }
        let ok = rng.is_true(q.server_rate_bps, 10_000);
        let slot_id = |iv: &Inventory, id: i16| *slots.iter().find(|&&s| iv.bag[s].as_ref().is_some_and(|i| i.template_id.0 == id)).expect("v");

        if let Some(fr) = FIXED_RECIPES.iter().find(|r| r.0 == op) {
            if let Some((id2, q2)) = fr.2 { let s2 = slot_id(inv, id2); inv.sub_from_bag_slot(s2, q2)?; inv.sub_from_bag_slot(if slots[0] == s2 { slots[1] } else { slots[0] }, if ok { fr.1.2 } else { fr.1.3 })?; }
            else { inv.sub_from_bag_slot(slots[0], if ok { fr.1.2 } else { fr.1.3 })?; }
            if ok { let oid = rng.next_range(i32::from(fr.5.0), i32::from(fr.5.1)) as i16; inv.add_to_bag(Self::mk(oid, fr.5.2, if fr.5.3 { &[(30, 0)] } else { &[] }), oid != 1655)?; }
            return Ok(CombineOutcome { success: ok, attempts: 1, badge_awarded: None, new_skill_point: None });
        }

        let (mut badge_awarded, mut new_skill_point) = (None, None);
        match op {
            CombineOp::EpSaoTrangBi => {
                let t0 = catalog.meta_of(inv.bag_item(slots[0])?.template_id).item_type;
                let (es, gs) = if t0 < 5 || t0 == 32 { (slots[0], slots[1]) } else { (slots[1], slots[0]) };
                let (gm, eq) = (inv.sub_from_bag_slot(gs, 1)?, inv.bag[es].as_mut().expect("v"));
                let cur = get_opt(eq, 102).unwrap_or(0);
                if let Some(&fo) = gm.options.first() { if cur >= 7 { eq.options.push(fo); } else { add_opt_param(eq, fo.id, fo.param); } }
                set_opt(eq, 102, cur + 1);
            }
            CombineOp::PhaLeHoaTrangBi => { inv.sub_gold(q.gold_cost)?; if ok { inv.sub_gem(q.gem_cost)?; add_opt_param(inv.bag[slots[0]].as_mut().expect("v"), 107, 1); } }
            CombineOp::NangCapVatPham => {
                let es = *slots.iter().find(|&&s| catalog.meta_of(inv.bag[s].as_ref().expect("v").template_id).item_type < 5).expect("v");
                let (lv, ss) = (get_opt(inv.bag[es].as_ref().expect("v"), 72).unwrap_or(0), *slots.iter().find(|&&s| s != es && inv.bag[s].as_ref().is_some_and(|i| i.template_id.0 != 987)).expect("v"));
                let ps = slots.iter().copied().find(|&s| inv.bag[s].as_ref().is_some_and(|i| i.template_id.0 == 987));
                inv.sub_from_bag_slot(ss, NANG_CAP_STONES[lv.max(0) as usize])?; if let Some(p) = ps { inv.sub_from_bag_slot(p, 1)?; }
                let eq = inv.bag[es].as_mut().expect("v");
                if ok {
                    for o in &mut eq.options { if matches!(o.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) { o.param += (o.param * 10 / 100).max(1); } }
                    set_opt(eq, 72, lv + 1); if lv == 7 { badge_awarded = Some(const_task_badges::THANH_DAP_DO_7); }
                } else if ps.is_none() && matches!(lv, 2 | 4 | 6) {
                    for o in &mut eq.options { if matches!(o.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) { o.param = (o.param - (o.param * 11 / 100).max(1)).max(1); } }
                    set_opt(eq, 72, lv - 1); add_opt_param(eq, 209, 1);
                }
            }
            CombineOp::NangCapBongTai | CombineOp::NangCapBongTai3 => {
                let (b, sh, full, fail, out, lv) = if op == CombineOp::NangCapBongTai { (454, 933, 9_999, 99, 921, 2) } else { (921, 1820, 20_000, 200, 1819, 3) };
                let (bs, ss) = (slot_id(inv, b), slot_id(inv, sh)); Self::sub_opt31(inv, ss, if ok { full } else { fail });
                if ok { inv.sub_from_bag_slot(bs, 1)?; inv.add_to_bag(Self::mk(out, 1, &[(72, lv)]), false)?; }
            }
            CombineOp::NangChiSoBongTai | CombineOp::NangChiSoBongTai3 => {
                let (e, sc, lines, lv) = if op == CombineOp::NangChiSoBongTai { (921, 200, 1, 2) } else { (1819, 99, 2, 3) };
                let (es, ss, bls) = (slot_id(inv, e), slot_id(inv, 934), slot_id(inv, 935)); Self::sub_opt31(inv, ss, sc); inv.sub_from_bag_slot(bls, 1)?;
                if ok { let ear = inv.bag[es].as_mut().expect("v"); ear.options.clear(); for _ in 0..lines { ear.options.push(ItemOption { id: PORATA_POOL[rng.next_range(0, 6) as usize], param: rng.next_range(5, 15) }); } ear.options.push(ItemOption { id: 72, param: lv }); }
            }
            CombineOp::NhapNgocRong => { let id = inv.bag_item(slots[0])?.template_id.0; inv.sub_from_bag_slot(slots[0], 7)?; inv.add_to_bag(Self::mk(id - 1, 1, &[]), true)?; }
            CombineOp::CheTaoTrangBiThienSu => {
                let (mut frag_id, mut luck) = (1066i16, 0i32);
                for &s in slots { let id = inv.bag_item(s)?.template_id.0; if (1066..=1070).contains(&id) { frag_id = id; inv.sub_from_bag_slot(s, 999)?; } else { if (1079..=1083).contains(&id) { luck = i32::from(id - 1078); } inv.sub_from_bag_slot(s, 1)?; } }
                if ok {
                    let mut opts = smallvec![ItemOption { id: 0, param: 2_400 }];
                    if luck > 0 && rng.next_range(0, 50) <= 5 + 5 * luck { opts.push(ItemOption { id: 15, param: rng.next_range(1, 3) }); for _ in 0..rng.next_range(1, 3) { opts.push(ItemOption { id: [50u16, 77, 103, 94, 5][rng.next_range(0, 4) as usize], param: rng.next_range(1, 3) }); } }
                    let g = match ctx.gender { Gender::TraiDat => 0, Gender::Namec => 1, Gender::Xayda => 2 };
                    inv.add_to_bag(ItemInstance { template_id: TemplateId(1048 + (frag_id - 1066) * 3 + g), quantity: 1, options: opts, create_time_ms: 0 }, false)?;
                }
            }
            CombineOp::ChuyenHoaTrangBiVang | CombineOp::ChuyenHoaTrangBiNgoc => {
                let (si, di) = if (555..=567).contains(&inv.bag_item(slots[1])?.template_id.0) && get_opt(inv.bag_item(slots[1])?, 72).is_none() { (slots[0], slots[1]) } else { (slots[1], slots[0]) };
                let (src, mut dst) = (inv.sub_from_bag_slot(si, 1)?, inv.sub_from_bag_slot(di, 1)?);
                let (lv, dr) = (get_opt(&src, 72).unwrap_or(4), get_opt(&src, 209).unwrap_or(0));
                for o in &mut dst.options { if matches!(o.id, 0 | 6 | 7 | 14 | 22 | 23 | 27 | 28 | 47) { let mut v = f64::from(o.param); for _ in 0..lv { v *= 1.1; } for _ in 0..dr { v *= 0.9; } o.param = (v as i32).max(1); } }
                for c in &src.options { if matches!(c.id, 72 | 95..=102 | 107 | 218 | 228) { set_opt(&mut dst, c.id, c.param); } }
                set_opt(&mut dst, 30, 0); inv.add_to_bag(dst, false)?;
            }
            CombineOp::PhanRaTrangBiKichHoat => { inv.sub_from_bag_slot(slots[0], 1)?; inv.add_to_bag(Self::mk(1656, 1, &[]), true)?; }
            CombineOp::NangCapSaoPhaLe => { let hs = slot_id(inv, 1423); inv.sub_from_bag_slot(hs, 1)?; if ok { let mut b = inv.sub_from_bag_slot(if slots[0] == hs { slots[1] } else { slots[0] }, 1)?; b.template_id.0 += 975; inv.add_to_bag(b, true)?; } }
            CombineOp::DanhBongSaoPhaLe => { let ms = slot_id(inv, 1439); inv.sub_from_bag_slot(ms, 1)?; let mut b = inv.sub_from_bag_slot(if slots[0] == ms { slots[1] } else { slots[0] }, 2)?; b.quantity = 1; b.template_id.0 += 10; if let Some(o) = b.options.first_mut() { o.param += 1; } inv.add_to_bag(b, true)?; }
            CombineOp::CuongHoaLoSaoPhaLe => {
                let (hs, ds) = (slot_id(inv, 1423), slot_id(inv, 1438)); let es = *slots.iter().find(|&&s| s != hs && s != ds).expect("v");
                inv.sub_from_bag_slot(hs, 1)?; inv.sub_from_bag_slot(ds, 1)?;
                if ok { let eq = inv.bag[es].as_mut().expect("v"); let nr = if get_opt(eq, 228) == Some(8) { 9 } else { 8 }; set_opt(eq, 218, 0); set_opt(eq, 228, nr); }
            }
            CombineOp::DoiSachTuyetKy | CombineOp::DoiSachTuyetKySealed => {
                let (os, ks) = (slot_id(inv, 1283), slot_id(inv, 1285)); inv.sub_from_bag_slot(ks, 1)?;
                if op == CombineOp::DoiSachTuyetKySealed { let ss = slot_id(inv, 1794); inv.sub_from_bag_slot(ss, 1)?; }
                inv.sub_from_bag_slot(os, if ok { 10 } else { 5 })?;
                if ok {
                    let n = if op == CombineOp::DoiSachTuyetKySealed && rng.is_true(20, 100) { rng.next_range(1, 3) } else { 1 };
                    let mut opts: SmallVec<[ItemOption; 4]> = (0..n).map(|_| ItemOption { id: 217, param: 0 }).collect();
                    for &(id, p) in &[(21, 40), (30, 0), (87, 0), (219, 5), (212, 1_000)] { opts.push(ItemOption { id, param: p }); }
                    inv.add_to_bag(ItemInstance { template_id: TemplateId([1044i16, 1211, 1212][rng.next_range(0, 2) as usize]), quantity: 1, options: opts, create_time_ms: 0 }, false)?;
                }
            }
            CombineOp::GiamDinhSach => { let bs = slot_id(inv, 1284); inv.sub_from_bag_slot(bs, 1)?; for o in &mut inv.bag[if slots[0] == bs { slots[1] } else { slots[0] }].as_mut().expect("v").options { if o.id == 217 { o.id = BOOK_POOL[rng.next_range(0, 11) as usize]; let div = rng.next_range(1, 3); o.param = rng.next_range(1, (10 / div).max(1)); } } }
            CombineOp::TaySach => { let bk = inv.bag[slots[0]].as_mut().expect("v"); for o in &mut bk.options { if o.id == 21 { break; } o.id = 217; o.param = 0; } add_opt_param(bk, 219, -1); }
            CombineOp::NangCapSachTuyetKy => { let cs = slot_id(inv, 1285); inv.sub_from_bag_slot(cs, 10)?; if ok { let bk = inv.bag[if slots[0] == cs { slots[1] } else { slots[0] }].as_mut().expect("v"); bk.template_id.0 = match bk.template_id.0 { 1044 => 1278, 1211 => 1279, _ => 1280 }; } }
            CombineOp::HoiPhucSach => set_opt(inv.bag[slots[0]].as_mut().expect("v"), 212, 1_000),
            CombineOp::PhanRaSach => { inv.sub_from_bag_slot(slots[0], 1)?; inv.add_to_bag(Self::mk(1283, 5, &[(30, 0)]), true)?; }
            CombineOp::HocTuyetKy => { inv.sub_from_bag_slot(slots[0], match (ctx.skill_point == 0, ok) { (true, _) => 9_999, (false, true) => 999, (false, false) => 99 })?; if ok { new_skill_point = Some(ctx.skill_point + 1); } }
            _ => unreachable!("covered by FIXED_RECIPES"),
        }
        Ok(CombineOutcome { success: ok, attempts: 1, badge_awarded, new_skill_point })
    }

    fn sub_opt31(inv: &mut Inventory, slot: usize, delta: i32) {
        if let Some(it) = &mut inv.bag[slot] { let rem = get_opt(it, 31).unwrap_or(0) - delta; if rem > 0 { set_opt(it, 31, rem); } else { inv.bag[slot] = None; } }
    }

    fn mk(id: i16, qty: u32, opts: &[(u16, i32)]) -> ItemInstance {
        ItemInstance { template_id: TemplateId(id), quantity: qty, options: opts.iter().map(|&(i, p)| ItemOption { id: i, param: p }).collect(), create_time_ms: 0 }
    }
}
