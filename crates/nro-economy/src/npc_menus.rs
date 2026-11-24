//! Declarative and stateful menu router porting all **66 `models/npc_list/`** NPC classes,
//! fixing `Osin` Majin Buu floors, `Babiday` 1-Gem GTPT, `Bill` Map `48` shop, `Dende` planet
//! guard, `Cui` Map `19`/`68` routes, `QuyLaoKame` turtle exchange, and `"Đóng"`/`"Từ chối"` guards.

use crate::combine::CombineOp;
use crate::inventory::{Inventory, InventoryError};
use nro_core::{AccountId, CoreStats, Gender, Location, MapId, PlayerId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

#[path = "npc_menus_confirm.rs"]
mod npc_menus_confirm;
#[path = "npc_menus_open.rs"]
mod npc_menus_open;
pub use npc_menus_open::NpcMenuRouter;

/// Enumeration of all **66** NPC classes (`NpcKind::COUNT == 66`).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Display,
    EnumIter,
    EnumCount,
    FromRepr,
    Serialize,
    Deserialize,
)]
#[repr(i16)]
pub enum NpcKind {
    /// `OngGohan` (`0`)
    OngGohan = 0,
    /// `OngParagus` (`1`)
    OngParagus = 1,
    /// `OngMoori` (`2`)
    OngMoori = 2,
    /// `RuongDo` (`3`)
    RuongDo = 3,
    /// `DauThan` (`4`)
    DauThan = 4,
    /// `ConMeo` (`5`)
    ConMeo = 5,
    /// `Bulma` (`7`)
    Bulma = 7,
    /// `Dende` (`8`)
    Dende = 8,
    /// `Appule` (`9`)
    Appule = 9,
    /// `DrDrief` (`10`)
    DrDrief = 10,
    /// `Cargo` (`11`)
    Cargo = 11,
    /// `Cui` (`12`)
    Cui = 12,
    /// `QuyLaoKame` (`13`)
    QuyLaoKame = 13,
    /// `TruongLaoGuru` (`14`)
    TruongLaoGuru = 14,
    /// `VuaVegeta` (`15`)
    VuaVegeta = 15,
    /// `Uron` (`16`)
    Uron = 16,
    /// `BoMong` (`17`)
    BoMong = 17,
    /// `Karin` (`18`)
    Karin = 18,
    /// `ThuongDe` (`19`)
    ThuongDe = 19,
    /// `ThanVuTru` (`20`)
    ThanVuTru = 20,
    /// `BaHatMit` (`21`)
    BaHatMit = 21,
    /// `TrongTai` (`22`)
    TrongTai = 22,
    /// `GhiDanh` (`23`)
    GhiDanh = 23,
    /// `LinhCanh` (`25`)
    LinhCanh = 25,
    /// `DocNhan` (`26`)
    DocNhan = 26,
    /// `KyGui` (`28`)
    KyGui = 28,
    /// `RongOmega` (`29`)
    RongOmega = 29,
    /// `Rong2Sao` (`30`)
    Rong2Sao = 30,
    /// `Rong3Sao` (`31`)
    Rong3Sao = 31,
    /// `Rong4Sao` (`32`)
    Rong4Sao = 32,
    /// `Rong5Sao` (`33`)
    Rong5Sao = 33,
    /// `Rong6Sao` (`34`)
    Rong6Sao = 34,
    /// `Rong7Sao` (`35`)
    Rong7Sao = 35,
    /// `Rong1Sao` (`36`)
    Rong1Sao = 36,
    /// `BulmaTuongLai` (`37`)
    BulmaTuongLai = 37,
    /// `Calick` (`38`)
    Calick = 38,
    /// `Santa` (`39`)
    Santa = 39,
    /// `QuocVuong` (`42`)
    QuocVuong = 42,
    /// `ToSuKaio` (`43`)
    ToSuKaio = 43,
    /// `Osin` (`44`)
    Osin = 44,
    /// `Kibit` (`45`)
    Kibit = 45,
    /// `Babiday` (`46`)
    Babiday = 46,
    /// `GiuMaDauBo` (`47`)
    GiuMaDauBo = 47,
    /// `NgoKhong` (`48`)
    NgoKhong = 48,
    /// `DuongTang` (`49`)
    DuongTang = 49,
    /// `QuaTrung` (`50`)
    QuaTrung = 50,
    /// `DuaHau` (`51`)
    DuaHau = 51,
    /// `Tapion` (`53`)
    Tapion = 53,
    /// `LyTieuNuong` (`54`)
    LyTieuNuong = 54,
    /// `Bill` (`55`)
    Bill = 55,
    /// `Whis` (`56`)
    Whis = 56,
    /// `Vados` (`58`)
    Vados = 58,
    /// `GokuSSJ` (`60`)
    GokuSSJ = 60,
    /// `GokuSSJ2` (`61`)
    GokuSSJ2 = 61,
    /// `Potage` (`62`)
    Potage = 62,
    /// `Jaco` (`63`)
    Jaco = 63,
    /// `DaiThienSu` (`64`)
    DaiThienSu = 64,
    /// `MrPoPo` (`67`)
    MrPoPo = 67,
    /// `Panchy` (`68`)
    Panchy = 68,
    /// `Bardock` (`70`)
    Bardock = 70,
    /// `Berry` (`71`)
    Berry = 71,
    /// `ToriBot` (`74`)
    ToriBot = 74,
    /// `ChiChi` (`81`)
    ChiChi = 81,
    /// `RuongSuuTam` (`82`)
    RuongSuuTam = 82,
    /// `DrMyuu` (`83`)
    DrMyuu = 83,
    /// `XeNuocMia` (`84`)
    XeNuocMia = 84,
}

/// Interactive `createOtherMenu` payload sent to the client (`cmd -32`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuPrompt {
    /// Originating NPC.
    pub npc: NpcKind,
    /// Menu state index (`player.idMark.indexMenu`).
    pub menu_index: i32,
    /// Dialogue body text.
    pub say: &'static str,
    /// Button labels (`0..N`).
    pub options: SmallVec<[&'static str; 8]>,
}

/// Result of opening or confirming an NPC menu option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcEffect {
    /// Displays an interactive menu card.
    ShowMenu(MenuPrompt),
    /// Displays an informational tutorial popup.
    Tutorial(&'static str),
    /// Opens a named `ShopDef` (`tag_name`).
    OpenShop(&'static str),
    /// Opens the combine tab for [`CombineOp`].
    OpenCombine(CombineOp),
    /// Opens the consignment market UI (`KyGui`).
    OpenConsignShop,
    /// Opens player storage box (`RuongDo` / `RuongSuuTam`).
    OpenItemBox,
    /// Opens `MagicTree` management menu (`DauThan`).
    OpenMagicTree,
    /// Opens `MabuEgg` (`false`) or `DuaHauEgg` (`true`) harvest menu.
    OpenHomePlant(bool),
    /// Opens client text input dialog (`GIFT_CODE`, `CHOOSE_LEVEL_BDKB`, etc.).
    OpenInput(&'static str),
    /// Teleports player to `(map_id, x, y)`.
    ChangeMap(MapId, i16, i16),
    /// Activates `Giải trừ phép thuật` (`Osin` / `Babiday`, 1 Gem deducted).
    ApplyGtpt,
    /// Activates Map `127` Kaioshin `+1M HP/KI, +10K Dame` blessing (`10` Gem deducted).
    ApplyMabuBlessing,
    /// Extends Kilis `2×` absorption buff by `+10` minutes (`5` Gem deducted).
    ExtendKilisBuff,
    /// Applies Black Ball War `(hp_mult, dame_mult)` stat multiplier.
    ApplyBlackBallBuff(u8, u8),
    /// Unlocks next power limit tier (`false = Self, true = Pet`).
    UnlockedPowerLimit(bool),
    /// Completed `Bò Mộng` daily check-in (`+10,000` Gem + `100× 457` Thỏi vàng).
    ClaimedDailyCheckIn,
    /// Purchased seasonal VIP tier `1..=4` at `ToriBot`.
    PurchasedVip(u8),
    /// Claimed KOL (`vip = false`) or KOL VIP (`vip = true`) stage reward `(vip, stage)`.
    ClaimedKolStage(bool, u8),
    /// Enters dungeon / arena / minigame activity.
    LaunchActivity(&'static str),
    /// Cancelled ongoing skill study and refunded `50%` potential.
    CancelledSkillStudy,
    /// Intentional no-op / close button.
    NoOp,
}

/// Errors produced during NPC menu validation or payment.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum NpcMenuError {
    /// Player failed planet, clan, activation, or power requirement.
    #[error("NPC condition not met: {0}")]
    GuardFailed(&'static str),
    /// Out-of-range menu index or button `select` index.
    #[error("invalid menu selection (menu {0}, select {1})")]
    InvalidSelection(i32, u8),
    /// Underlying inventory error.
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

/// Canonical alias for [`NpcMenuError`] used by server NPC routers.
pub type NpcError = NpcMenuError;

/// Mutable player context passed into [`NpcMenuRouter`].
pub struct NpcPlayerCtx<'a> {
    /// Account ID (`session.userId`).
    pub account_id: AccountId,
    /// Player ID.
    pub player_id: PlayerId,
    /// Current map & coordinates.
    pub location: Location,
    /// Player planet.
    pub gender: Gender,
    /// Faction flag (`9 = Kaioshin`, `10 = Babidi`).
    pub c_flag: u8,
    /// Account activation status (`session.actived`).
    pub activated: bool,
    /// Clan membership flag.
    pub has_clan: bool,
    /// Clan leader flag.
    pub is_clan_leader: bool,
    /// Days in current clan.
    pub clan_days: u16,
    /// Owns combat pet.
    pub has_pet: bool,
    /// Wearing 5 Hủy Diệt pieces (`Whis`).
    pub wearing_full_destroyer: bool,
    /// `Giải trừ phép thuật` active flag.
    pub is_use_gtpt: bool,
    /// Map `127` Kaioshin blessing active flag.
    pub is_phu_ho_mabu: bool,
    /// Majin Buu floor points (`>= 10` allows descending).
    pub mabu_points: u16,
    /// Seasonal VND balance (`ToriBot`).
    pub vnd_balance: u32,
    /// Seasonal VIP purchase counter (`0..=4`).
    pub vip_purchases: u8,
    /// Current KOL stage (`1..=7`).
    pub kol_stage: u8,
    /// Current KOL VIP stage (`1..=7`).
    pub kol_vip_stage: u8,
    /// Progress counter toward active KOL stage.
    pub kol_progress: u32,
    /// Mutable player stats.
    pub stats: &'a mut CoreStats,
    /// Mutable player inventory.
    pub inventory: &'a mut Inventory,
}
