//! Declarative and stateful menu router porting all **66 `models/npc_list/`** NPC classes,
//! fixing `Osin` Majin Buu floors, `Babiday` 1-Gem GTPT, `Bill` Map `48` shop, `Dende` planet
//! guard, `Cui` Map `19`/`68` routes, `QuyLaoKame` turtle exchange, and `"Đóng"`/`"Từ chối"` guards.

use crate::combine::CombineOp;
use crate::inventory::{Inventory, InventoryError};
use nro_consts::const_npc;
use nro_core::{AccountId, CoreStats, Gender, Location, MapId, PlayerId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

#[path = "npc_menus_confirm.rs"]
mod npc_menus_confirm;

/// Enumeration of all **66** NPC classes (`NpcKind::COUNT == 66`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, EnumIter, EnumCount, FromRepr, Serialize, Deserialize)]
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

/// Unified 66-NPC `openBaseMenu` and `confirmMenu` dispatcher.
pub struct NpcMenuRouter;

impl NpcMenuRouter {
    /// Builds initial `openBaseMenu` effect for any of the 66 [`NpcKind`] variants.
    pub fn open_base_menu(npc: NpcKind, ctx: &NpcPlayerCtx<'_>) -> Result<NpcEffect, NpcMenuError> {
        let map = ctx.location.map_id.0;
        let menu = |idx: i32, say: &'static str, opts: &[&'static str]| {
            Ok(NpcEffect::ShowMenu(MenuPrompt { npc, menu_index: idx, say, options: opts.iter().copied().collect() }))
        };
        match npc {
            NpcKind::OngGohan | NpcKind::OngMoori | NpcKind::OngParagus => Ok(NpcEffect::Tutorial("Con cố gắng học thành tài, đừng lo cho ta.")),
            NpcKind::RuongDo | NpcKind::RuongSuuTam => Ok(NpcEffect::OpenItemBox),
            NpcKind::DauThan => Ok(NpcEffect::OpenMagicTree),
            NpcKind::ConMeo => menu(const_npc::IGNORE_MENU, "Xin chào!", &["Đóng"]),
            NpcKind::QuaTrung | NpcKind::DuaHau => Ok(NpcEffect::OpenHomePlant(npc == NpcKind::DuaHau)),
            NpcKind::Uron => Ok(NpcEffect::OpenShop("URON")),
            NpcKind::Bulma if ctx.gender != Gender::TraiDat => Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho người Trái Đất")),
            NpcKind::Dende if ctx.gender != Gender::Namec => Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho người Namếc")),
            NpcKind::Appule if ctx.gender != Gender::Xayda => Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho chiến binh Xayda")),
            NpcKind::Bulma | NpcKind::Appule | NpcKind::BulmaTuongLai | NpcKind::Panchy => menu(const_npc::BASE_MENU, "Cậu cần trang bị gì?", &["Cửa hàng", "Đóng"]),
            NpcKind::Dende => menu(const_npc::BASE_MENU, "Cậu cần gì?", &["Cửa hàng", "Gọi Rồng Thần Namếc"]),
            NpcKind::Cui if map == 19 => menu(const_npc::BASE_MENU, "Tàu vũ trụ!", &["Đến Cold", "Đến Nappa", "Tìm Boss 50tr vàng", "Đóng"]),
            NpcKind::Cui if map == 68 => menu(const_npc::BASE_MENU, "Tàu vũ trụ!", &["Về Thành phố Vegeta", "Đóng"]),
            NpcKind::Cargo | NpcKind::DrDrief | NpcKind::Cui => menu(const_npc::BASE_MENU, "Tàu vũ trụ!", &["Trái Đất", "Namếc", "Xayda", "Siêu thị"]),
            NpcKind::Calick => menu(const_npc::BASE_MENU, "Cỗ máy thời gian!", if map == 102 { &["Về Quá Khứ", "Đóng"] } else { &["Đi Tương Lai", "Đóng"] }),
            NpcKind::GokuSSJ | NpcKind::Jaco | NpcKind::Tapion | NpcKind::Kibit => menu(const_npc::BASE_MENU, "Di chuyển ngay?", &["Đồng ý", "Từ chối"]),
            NpcKind::GokuSSJ2 => menu(const_npc::BASE_MENU, "Đổi 9999 Bí kiếp lấy võ phục Yardrat?", &["Đổi võ phục", "Đóng"]),
            NpcKind::NgoKhong => menu(const_npc::BASE_MENU, "Ngũ Hành Sơn!", &["Hướng dẫn", "Đóng"]),
            NpcKind::DuongTang => menu(const_npc::BASE_MENU, "A di đà phật!", if (122..=124).contains(&map) { &["Về nhà", "Đóng"] } else { &["Đến Ngũ Hành Sơn", "Đóng"] }),
            NpcKind::Santa => menu(const_npc::BASE_MENU, "Xin chào!", if ctx.inventory.count_template_in_bag(TemplateId(459)) > 0 { &["Cửa hàng", "Giảm giá 80%", "Mở rộng", "Giftcode", "Hạn sử dụng", "Hớt tóc", "Danh hiệu"] } else { &["Cửa hàng", "Mở rộng", "Giftcode", "Hạn sử dụng", "Hớt tóc", "Danh hiệu"] }),
            NpcKind::BaHatMit if map == 5 => menu(const_npc::BASE_MENU, "Bà Hạt Mít Đảo Kame", &["Ép sao", "Pha lê hóa", "Phân rã SKH", "Tái tạo SKH", "Nâng SPL", "Đánh bóng SPL", "Cường hóa lỗ", "Chuyển hóa Vàng", "Chuyển hóa Ngọc"]),
            NpcKind::BaHatMit => menu(const_npc::BASE_MENU, "Bà Hạt Mít", &["Cửa hàng\nBùa", "Nâng cấp đồ", "Porata 2", "Chỉ số BTC2", "Porata 3", "Chỉ số BTC3", "Nhập Ngọc Rồng", "Sách Tuyệt Kỹ"]),
            NpcKind::BoMong => menu(const_npc::BASE_MENU, "Nhiệm vụ & Điểm danh", &["Nhiệm vụ ngày", "Thành tích", "Quy đổi", "Điểm danh"]),
            NpcKind::KyGui => if ctx.activated { menu(const_npc::BASE_MENU, "Ký gửi!", &["Hướng dẫn", "Cửa hàng ký gửi"]) } else { Err(NpcMenuError::GuardFailed("not activated")) },
            NpcKind::QuyLaoKame => menu(const_npc::BASE_MENU, "Con muốn hỏi gì?", &["Nói chuyện", "Đổi điểm", "Quà KOL", "Quà KOL VIP", "Giao Rùa con"]),
            NpcKind::TruongLaoGuru if ctx.gender != Gender::Namec => Ok(NpcEffect::Tutorial("Về hành tinh của mình đi")),
            NpcKind::VuaVegeta if ctx.gender != Gender::Xayda => Ok(NpcEffect::Tutorial("Về hành tinh của mình đi")),
            NpcKind::TruongLaoGuru | NpcKind::VuaVegeta => menu(const_npc::BASE_MENU, "Chào con!", &["Nhiệm vụ", "Học Kỹ năng"]),
            NpcKind::QuocVuong | NpcKind::ToSuKaio => menu(const_npc::MENU_NANG_GIOI_HAN, "Nâng giới hạn sức mạnh?", &["Bản thân", "Đệ tử", "Đóng"]),
            NpcKind::Karin | NpcKind::ThuongDe | NpcKind::ThanVuTru => menu(const_npc::BASE_MENU, "Thánh địa tập luyện", &["Tập tự động", "Tập luyện", "Thách đấu", "Vòng quay"]),
            NpcKind::Osin => match map {
                50 => menu(const_npc::BASE_MENU, "Đi đâu?", &["Đến Kaio", "Hành tinh Bill", "Từ chối"]),
                154 | 155 => menu(const_npc::BASE_MENU, "Di chuyển?", &["Đồng ý", "Từ chối"]),
                165 => menu(const_npc::BASE_MENU, "Hút Kilis!", &["Về nhà", "Bùa hỗ trợ 5 ngọc", "Từ chối"]),
                114..=120 => if ctx.c_flag != 9 { Ok(NpcEffect::Tutorial("Khác phe!")) } else { menu(const_npc::GO_UPSTAIRS_MENU, "Tầng Ma Bư", &["Hướng dẫn", "Giải trừ 1 ngọc", "Xuống Tầng dưới"]) },
                127 => menu(const_npc::BASE_MENU, "Phù hộ Kaiô!", &["Phù hộ 10 ngọc", "Về Đại Hội"]),
                _ => menu(const_npc::MENU_OPEN_MMB_WITH_JAR, "Ngăn chặn Ma Bư!", &["Vào Map Ma Bư", "Bình hút năng lượng", "Từ chối"]),
            },
            NpcKind::Babiday => if ctx.c_flag != 10 { Ok(NpcEffect::Tutorial("Khác phe!")) } else { menu(const_npc::GO_UPSTAIRS_MENU, "Babidi!", &["Hướng dẫn", "Giải trừ 1 ngọc", "Về Đại Hội"]) },
            NpcKind::Bill => if map == 48 { menu(100, "Đổi đồ Hủy Diệt?", &["Shop Hủy Diệt", "Từ chối"]) } else { menu(const_npc::BASE_MENU, "Về Kaio?", &["Đồng ý", "Từ chối"]) },
            NpcKind::Whis => menu(const_npc::BASE_MENU, "Hành tinh Thiên Sứ", &["Shop thiên sứ", "Chế tạo Thiên Sứ", "Học tuyệt kỹ", "Tập luyện"]),
            NpcKind::Rong1Sao | NpcKind::Rong2Sao | NpcKind::Rong3Sao | NpcKind::Rong4Sao | NpcKind::Rong5Sao | NpcKind::Rong6Sao | NpcKind::Rong7Sao => menu(const_npc::MENU_OPTION_PHU_HP, "Phù hộ Sao Đen!", &["x3 HP", "x5 HP", "x7 HP", "x3 SD", "x5 SD", "Về nhà"]),
            NpcKind::RongOmega => menu(const_npc::MENU_OPEN_BDW, "Ngọc rồng sao đen!", &["Hướng dẫn", "Tham gia", "Nhận thưởng"]),
            NpcKind::GhiDanh | NpcKind::TrongTai => menu(const_npc::BASE_MENU, "Đại Hội Võ Thuật!", &["Thi đấu", "Top 100", "DHVT23"]),
            NpcKind::LinhCanh | NpcKind::DocNhan | NpcKind::MrPoPo => menu(const_npc::BASE_MENU, "Phó bản Bang!", &["Vào phó bản", "Đóng"]),
            NpcKind::GiuMaDauBo => menu(const_npc::BASE_MENU, "Bang Hội!", &["Gọi Boss", "Capsule Bang", "Shop Bang"]),
            NpcKind::LyTieuNuong => menu(const_npc::BASE_MENU, "Minigame!", &["Kéo Búa Bao", "Con Số May Mắn", "Chọn Ai Đây"]),
            NpcKind::Potage => menu(const_npc::BASE_MENU, "Nhân Bản Commeson!", &["Hướng dẫn", "Gọi Nhân Bản", "Từ chối"]),
            NpcKind::ToriBot => menu(0, "Mùa VIP (tối đa 4 lần)!", &["Vip 1", "Vip 2", "Vip 3", "Vip 4"]),
            NpcKind::ChiChi => menu(const_npc::BASE_MENU, "Sự kiện!", &["Top Sự Kiện", "Shop ChiChi"]),
            NpcKind::XeNuocMia => menu(const_npc::BASE_MENU, "Nước mía giải khát!", &["Ép 1 ly (5tr vàng)", "Ép 10 ly (50tr vàng)", "Đóng"]),
            NpcKind::Berry | NpcKind::Bardock | NpcKind::DaiThienSu | NpcKind::DrMyuu | NpcKind::Vados => Ok(NpcEffect::Tutorial("Hãy tiếp tục rèn luyện!")),
        }
    }
}
