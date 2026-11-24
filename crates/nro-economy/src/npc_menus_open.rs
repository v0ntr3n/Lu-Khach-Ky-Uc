//! `NpcMenuRouter::open_base_menu` initial menu builder across all 66 [`NpcKind`] variants.

use super::{MenuPrompt, NpcEffect, NpcKind, NpcMenuError, NpcPlayerCtx};
use nro_consts::const_npc;
use nro_core::{Gender, TemplateId};

/// Unified 66-NPC `openBaseMenu` and `confirmMenu` dispatcher.
pub struct NpcMenuRouter;

impl NpcMenuRouter {
    /// Builds initial `openBaseMenu` effect for any of the 66 [`NpcKind`] variants.
    pub fn open_base_menu(npc: NpcKind, ctx: &NpcPlayerCtx<'_>) -> Result<NpcEffect, NpcMenuError> {
        let map = ctx.location.map_id.0;
        let menu = |idx: i32, say: &'static str, opts: &[&'static str]| {
            Ok(NpcEffect::ShowMenu(MenuPrompt {
                npc,
                menu_index: idx,
                say,
                options: opts.iter().copied().collect(),
            }))
        };
        match npc {
            NpcKind::OngGohan | NpcKind::OngMoori | NpcKind::OngParagus => Ok(NpcEffect::Tutorial(
                "Con cố gắng học thành tài, đừng lo cho ta.",
            )),
            NpcKind::RuongDo | NpcKind::RuongSuuTam => Ok(NpcEffect::OpenItemBox),
            NpcKind::DauThan => Ok(NpcEffect::OpenMagicTree),
            NpcKind::ConMeo => menu(const_npc::IGNORE_MENU, "Xin chào!", &["Đóng"]),
            NpcKind::QuaTrung | NpcKind::DuaHau => {
                Ok(NpcEffect::OpenHomePlant(npc == NpcKind::DuaHau))
            }
            NpcKind::Uron => Ok(NpcEffect::OpenShop("URON")),
            NpcKind::Bulma if ctx.gender != Gender::TraiDat => {
                Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho người Trái Đất"))
            }
            NpcKind::Dende if ctx.gender != Gender::Namec => {
                Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho người Namếc"))
            }
            NpcKind::Appule if ctx.gender != Gender::Xayda => {
                Ok(NpcEffect::Tutorial("Ta chỉ bán đồ cho chiến binh Xayda"))
            }
            NpcKind::Bulma | NpcKind::Appule | NpcKind::BulmaTuongLai | NpcKind::Panchy => menu(
                const_npc::BASE_MENU,
                "Cậu cần trang bị gì?",
                &["Cửa hàng", "Đóng"],
            ),
            NpcKind::Dende => menu(
                const_npc::BASE_MENU,
                "Cậu cần gì?",
                &["Cửa hàng", "Gọi Rồng Thần Namếc"],
            ),
            NpcKind::Cui if map == 19 => menu(
                const_npc::BASE_MENU,
                "Tàu vũ trụ!",
                &["Đến Cold", "Đến Nappa", "Tìm Boss 50tr vàng", "Đóng"],
            ),
            NpcKind::Cui if map == 68 => menu(
                const_npc::BASE_MENU,
                "Tàu vũ trụ!",
                &["Về Thành phố Vegeta", "Đóng"],
            ),
            NpcKind::Cargo | NpcKind::DrDrief | NpcKind::Cui => menu(
                const_npc::BASE_MENU,
                "Tàu vũ trụ!",
                &["Trái Đất", "Namếc", "Xayda", "Siêu thị"],
            ),
            NpcKind::Calick => menu(
                const_npc::BASE_MENU,
                "Cỗ máy thời gian!",
                if map == 102 {
                    &["Về Quá Khứ", "Đóng"]
                } else {
                    &["Đi Tương Lai", "Đóng"]
                },
            ),
            NpcKind::GokuSSJ | NpcKind::Jaco | NpcKind::Tapion | NpcKind::Kibit => menu(
                const_npc::BASE_MENU,
                "Di chuyển ngay?",
                &["Đồng ý", "Từ chối"],
            ),
            NpcKind::GokuSSJ2 => menu(
                const_npc::BASE_MENU,
                "Đổi 9999 Bí kiếp lấy võ phục Yardrat?",
                &["Đổi võ phục", "Đóng"],
            ),
            NpcKind::NgoKhong => menu(
                const_npc::BASE_MENU,
                "Ngũ Hành Sơn!",
                &["Hướng dẫn", "Đóng"],
            ),
            NpcKind::DuongTang => menu(
                const_npc::BASE_MENU,
                "A di đà phật!",
                if (122..=124).contains(&map) {
                    &["Về nhà", "Đóng"]
                } else {
                    &["Đến Ngũ Hành Sơn", "Đóng"]
                },
            ),
            NpcKind::Santa => menu(
                const_npc::BASE_MENU,
                "Xin chào!",
                if ctx.inventory.count_template_in_bag(TemplateId(459)) > 0 {
                    &[
                        "Cửa hàng",
                        "Giảm giá 80%",
                        "Mở rộng",
                        "Giftcode",
                        "Hạn sử dụng",
                        "Hớt tóc",
                        "Danh hiệu",
                    ]
                } else {
                    &[
                        "Cửa hàng",
                        "Mở rộng",
                        "Giftcode",
                        "Hạn sử dụng",
                        "Hớt tóc",
                        "Danh hiệu",
                    ]
                },
            ),
            NpcKind::BaHatMit if map == 5 => menu(
                const_npc::BASE_MENU,
                "Bà Hạt Mít Đảo Kame",
                &[
                    "Ép sao",
                    "Pha lê hóa",
                    "Phân rã SKH",
                    "Tái tạo SKH",
                    "Nâng SPL",
                    "Đánh bóng SPL",
                    "Cường hóa lỗ",
                    "Chuyển hóa Vàng",
                    "Chuyển hóa Ngọc",
                    "Đá & Phân rã",
                ],
            ),
            NpcKind::BaHatMit => menu(
                const_npc::BASE_MENU,
                "Bà Hạt Mít",
                &[
                    "Cửa hàng\nBùa",
                    "Nâng cấp đồ",
                    "Porata 2",
                    "Chỉ số BTC2",
                    "Porata 3",
                    "Chỉ số BTC3",
                    "Nhập Ngọc Rồng",
                    "Sách Tuyệt Kỹ",
                    "Làm phép nhập đá",
                    "Phân rã\nThần Linh",
                ],
            ),
            NpcKind::BoMong => menu(
                const_npc::BASE_MENU,
                "Nhiệm vụ & Điểm danh",
                &["Nhiệm vụ ngày", "Thành tích", "Quy đổi", "Điểm danh"],
            ),
            NpcKind::KyGui => {
                if ctx.activated {
                    menu(
                        const_npc::BASE_MENU,
                        "Ký gửi!",
                        &["Hướng dẫn", "Cửa hàng ký gửi"],
                    )
                } else {
                    Err(NpcMenuError::GuardFailed("not activated"))
                }
            }
            NpcKind::QuyLaoKame => menu(
                const_npc::BASE_MENU,
                "Con muốn hỏi gì?",
                &[
                    "Nói chuyện",
                    "Đổi điểm",
                    "Quà KOL",
                    "Quà KOL VIP",
                    "Giao Rùa con",
                ],
            ),
            NpcKind::TruongLaoGuru if ctx.gender != Gender::Namec => {
                Ok(NpcEffect::Tutorial("Về hành tinh của mình đi"))
            }
            NpcKind::VuaVegeta if ctx.gender != Gender::Xayda => {
                Ok(NpcEffect::Tutorial("Về hành tinh của mình đi"))
            }
            NpcKind::TruongLaoGuru | NpcKind::VuaVegeta => menu(
                const_npc::BASE_MENU,
                "Chào con!",
                &["Nhiệm vụ", "Học Kỹ năng"],
            ),
            NpcKind::QuocVuong | NpcKind::ToSuKaio => menu(
                const_npc::MENU_NANG_GIOI_HAN,
                "Nâng giới hạn sức mạnh?",
                &["Bản thân", "Đệ tử", "Đóng"],
            ),
            NpcKind::Karin | NpcKind::ThuongDe | NpcKind::ThanVuTru => menu(
                const_npc::BASE_MENU,
                "Thánh địa tập luyện",
                &["Tập tự động", "Tập luyện", "Thách đấu", "Vòng quay"],
            ),
            NpcKind::Osin => match map {
                50 => menu(
                    const_npc::BASE_MENU,
                    "Đi đâu?",
                    &["Đến Kaio", "Hành tinh Bill", "Từ chối"],
                ),
                154 | 155 => menu(const_npc::BASE_MENU, "Di chuyển?", &["Đồng ý", "Từ chối"]),
                165 => menu(
                    const_npc::BASE_MENU,
                    "Hút Kilis!",
                    &["Về nhà", "Bùa hỗ trợ 5 ngọc", "Từ chối"],
                ),
                114..=120 => {
                    if ctx.c_flag != 9 {
                        Ok(NpcEffect::Tutorial("Khác phe!"))
                    } else {
                        menu(
                            const_npc::GO_UPSTAIRS_MENU,
                            "Tầng Ma Bư",
                            &["Hướng dẫn", "Giải trừ 1 ngọc", "Xuống Tầng dưới"],
                        )
                    }
                }
                127 => menu(
                    const_npc::BASE_MENU,
                    "Phù hộ Kaiô!",
                    &["Phù hộ 10 ngọc", "Về Đại Hội"],
                ),
                _ => menu(
                    const_npc::MENU_OPEN_MMB_WITH_JAR,
                    "Ngăn chặn Ma Bư!",
                    &["Vào Map Ma Bư", "Bình hút năng lượng", "Từ chối"],
                ),
            },
            NpcKind::Babiday => {
                if ctx.c_flag != 10 {
                    Ok(NpcEffect::Tutorial("Khác phe!"))
                } else {
                    menu(
                        const_npc::GO_UPSTAIRS_MENU,
                        "Babidi!",
                        &["Hướng dẫn", "Giải trừ 1 ngọc", "Về Đại Hội"],
                    )
                }
            }
            NpcKind::Bill => {
                if map == 48 {
                    menu(100, "Đổi đồ Hủy Diệt?", &["Shop Hủy Diệt", "Từ chối"])
                } else {
                    menu(const_npc::BASE_MENU, "Về Kaio?", &["Đồng ý", "Từ chối"])
                }
            }
            NpcKind::Whis => menu(
                const_npc::BASE_MENU,
                "Hành tinh Thiên Sứ",
                &[
                    "Shop thiên sứ",
                    "Chế tạo Thiên Sứ",
                    "Học tuyệt kỹ",
                    "Tập luyện",
                    "Sách Tuyệt Kỹ",
                ],
            ),
            NpcKind::Rong1Sao
            | NpcKind::Rong2Sao
            | NpcKind::Rong3Sao
            | NpcKind::Rong4Sao
            | NpcKind::Rong5Sao
            | NpcKind::Rong6Sao
            | NpcKind::Rong7Sao => menu(
                const_npc::MENU_OPTION_PHU_HP,
                "Phù hộ Sao Đen!",
                &["x3 HP", "x5 HP", "x7 HP", "x3 SD", "x5 SD", "Về nhà"],
            ),
            NpcKind::RongOmega => menu(
                const_npc::MENU_OPEN_BDW,
                "Ngọc rồng sao đen!",
                &["Hướng dẫn", "Tham gia", "Nhận thưởng"],
            ),
            NpcKind::GhiDanh | NpcKind::TrongTai => menu(
                const_npc::BASE_MENU,
                "Đại Hội Võ Thuật!",
                &["Thi đấu", "Top 100", "DHVT23"],
            ),
            NpcKind::LinhCanh | NpcKind::DocNhan | NpcKind::MrPoPo => menu(
                const_npc::BASE_MENU,
                "Phó bản Bang!",
                &["Vào phó bản", "Đóng"],
            ),
            NpcKind::GiuMaDauBo => menu(
                const_npc::BASE_MENU,
                "Bang Hội!",
                &["Gọi Boss", "Capsule Bang", "Shop Bang"],
            ),
            NpcKind::LyTieuNuong => menu(
                const_npc::BASE_MENU,
                "Minigame!",
                &["Kéo Búa Bao", "Con Số May Mắn", "Chọn Ai Đây"],
            ),
            NpcKind::Potage => menu(
                const_npc::BASE_MENU,
                "Nhân Bản Commeson!",
                &["Hướng dẫn", "Gọi Nhân Bản", "Từ chối"],
            ),
            NpcKind::ToriBot => menu(
                0,
                "Mùa VIP (tối đa 4 lần)!",
                &["Vip 1", "Vip 2", "Vip 3", "Vip 4"],
            ),
            NpcKind::ChiChi => menu(
                const_npc::BASE_MENU,
                "Sự kiện!",
                &["Top Sự Kiện", "Shop ChiChi"],
            ),
            NpcKind::XeNuocMia => menu(
                const_npc::BASE_MENU,
                "Nước mía giải khát!",
                &["Ép 1 ly (5tr vàng)", "Ép 10 ly (50tr vàng)", "Đóng"],
            ),
            NpcKind::Berry
            | NpcKind::Bardock
            | NpcKind::DaiThienSu
            | NpcKind::DrMyuu
            | NpcKind::Vados => Ok(NpcEffect::Tutorial("Hãy tiếp tục rèn luyện!")),
        }
    }
}
