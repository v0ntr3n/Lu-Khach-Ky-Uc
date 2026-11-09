# DESIGN.md — Hệ Thống Thiết Kế "Sơn Mài & Giấy Dó" HD-2D (*Lữ Khách Ký Ức: 1930 – 2045*)

> **Ghi chú chuyển giao kiến trúc (10/11/2026 — đang lập kế hoạch, chưa triển khai):** Runtime hiện tại vẫn là **Canvas 2D side-scroller**, người chơi đi trái/phải, viewport logic `320×180` và HUD kiểu `1/5 + 4/5`. **Mục tiêu mới** là RPG Exploration **top-down 2D góc nhìn 3/4**, di chuyển XY 8 hướng, xây map bằng **Tiled**, có NPC, tương tác theo vùng, va chạm và ra/vào nhà; **WorkAdventure chỉ là tham khảo thiết kế bản đồ**, không phải runtime hay dịch vụ multiplayer. M1 sẽ là **thử nghiệm Phaser 3 + Tiled cô lập**, chỉ thay runtime sau khi build và playtest đạt tiêu chí.
>
> **Phạm vi tài liệu:** Các mô tả side-scrolling, `1/5 + 4/5`, parallax đi ngang, phím `A/D` và quy trình Climax trong các mục bên dưới là **đặc tả/baseline cũ**, không phải UI/gameplay mục tiêu. **Giữ lại** hệ màu Sơn Mài & Giấy Dó, hiệu ứng OpeningFracture, định hướng kể chuyện và nguyên tắc nguồn sử liệu; **UI đích** là game world toàn màn hình với HUD theo ngữ cảnh, nhật ký/bản đồ mở riêng. Quy ước thư mục đích **chỉ có một nguồn chuẩn** tại [WORKADVENTURE_TOPDOWN.md — Canonical target directory layout](docs/plans/WORKADVENTURE_TOPDOWN.md#canonical-target-directory-layout), nhất quán với [RPG_EXPLORATION.md](docs/plans/RPG_EXPLORATION.md).

Tựa game **Lữ Khách Ký Ức (1930 – 2045)** kế thừa trực tiếp vũ trụ thị giác của [`v0ntr3n/SPST`](https://github.com/v0ntr3n/SPST) và tiêu chuẩn biên tập từ [`pbakaus/impeccable`](https://github.com/pbakaus/impeccable), chuyển hóa thành kiến trúc **Game Đi Cảnh Giải Đố & Điện Ảnh Pixel HD-2D (320×180 Integer-Scaled)** với bảng màu **12 Sắc Độ Sơn Mài & Giấy Dó**.

---

## 1. Kiến Trúc Mở Màn Nứt Vỡ Thời Gian (`0.0s → 4.8s`)

Từ năm **2045** (Kỷ niệm 100 năm thành lập nước), Lữ Khách bước qua **Vết Nứt Không Gian – Thời Gian (`#rift-opening-overlay`)** gồm 5 pha:
1. **Phase A — True Black (`0.0s – 0.6s`)**: Khung nhìn phủ `#000000` tuyệt đối, khóa thao tác.
2. **Phase B — First Fracture (`0.6s – 1.5s`)**: Vết nứt thời gian mảnh sắc xuất hiện tại tâm màn hình, hé lộ ánh sáng vàng đồng cổ (`#D4A55D`) từ chiều không gian lịch sử bên dưới.
3. **Phase C — Fracture Propagation (`1.5s – 2.9s`)**: Các nhánh nứt thứ cấp lan ra 4 góc theo bộ sinh số giả ngẫu nhiên cố định hạt giống (`mulberry32`), chia màng đen thành 16 mảnh đa giác răng cưa.
4. **Phase D — Catastrophic Tear (`2.9s – 4.4s`)**: 16 mảnh đa giác tách rời từ tâm văng ra 4 phía với gia tốc phi tuyến, để lộ trực tiếp khung cảnh Pixel HD-2D.
5. **Phase E — Complete Reveal (`4.4s – 4.8s`)**: Gỡ bỏ hoàn toàn lớp phủ và trao quyền điều khiển Lữ Khách 2045.

---

## 2. Hệ Thống Bảng Màu Pixel 12 Sắc Độ (`oklch()` — Sơn Mài & Giấy Dó)

Tuyệt đối loại bỏ màu neon tím/xanh dương AI slop. Toàn bộ ma trận điểm ảnh (`SpriteAtlas`), nền Parallax 4 lớp và ánh sáng Chiaroscuro chỉ sử dụng 12 sắc độ:

| Mã Token | Hex | Vai trò trong Khung Hình Pixel HD-2D & UI |
| :--- | :--- | :--- |
| `--lacquer-void` | `#110E0B` | Nền đêm Sơn Mài sâu thẳm, viền bóng tối nhân vật |
| `--lacquer-deep` | `#1B1612` | Lớp kiến trúc nền xa (Parallax Layer 1) |
| `--lacquer-elevated` | `#241D17` | Lớp mái ngói, hầm hào, mặt đường (Parallax Layer 2) |
| `--ink-primary` | `#232116` | Chữ chính trên nền Sổ tay Giấy Dó (Chuẩn AAA) |
| `--ink-secondary` | `#3E3A2C` | Chữ phụ, bóng đổ trung gian |
| `--bronze-muted` | `#6E4720` | Gỗ cổ, thân cây đa Tân Trào, bánh xe đạp thồ |
| `--bronze-antique` | `#8A5A2B` | Viền khung đồng cổ, chi tiết trang phục |
| `--bronze-bright` | `#D4A55D` | Ánh đèn dầu Cửu Long, tia sáng Chiaroscuro, sao vàng |
| `--crimson-seal` | `#9E3321` | Đỏ Son Triện, sắc cờ Cách mạng, dấu xác thực sử liệu |
| `--crimson-vermilion` | `#DF6B57` | Điểm sáng Chu Sa, tia lửa lịch sử |
| `--parchment-warm` | `#E6E5DC` | Nền Sổ tay Giấy Dó, sương mù, trang sách cổ |
| `--parchment-light` | `#F4EFE4` | Điểm sáng cao nhất (Highlight), chữ trên nền Sơn Mài |

---

## 3. Bố Cục Điện Ảnh `1/5 + 4/5` & Cơ Chế Gameplay (Mức 1 + Climax A+B)

- **Cột Trái Cố Định `1/5` (`#codex-sidebar`)**: Hiển thị **12 Cột Mốc La Mã (`I – XII`)** tương ứng với 12 di vật lịch sử cần khôi phục qua 4 chương. Bấm vào bất kỳ mốc nào sẽ dịch chuyển Lữ Khách tới ngay tọa độ thời gian đó.
- **Sân Khấu Chính `4/5` (`#stage-viewport`)**:
  - **Khung Hình Pixel `320×180` (`#pixel-canvas`)**: Dựng cảnh đi ngang (Side-scrolling) với 4 lớp Parallax, hiệu ứng đèn rọi tuần tra (1930), dây thông tin (1945), xe đạp thồ vượt đèo (1954–1975) và kiến thiết đô thị số (1986–2045).
  - **Độ Khó Mức 1 (Story-Driven / Không có Game Over)**: Khi người chơi bước vào vùng đèn tuần tra hoặc pháo sáng, màn hình chỉ gợn sóng nhẹ (*"Dòng thời gian chao đảo — Lùi về điểm ẩn nấp gần nhất"*) và đưa Lữ Khách lùi lại vài bước ngay tại chỗ.
  - **Khoảnh Khắc Quyết Định Cuối Chương (Climax C = A + B)**:
    - **Bước A (Quyết Sách Biện Chứng)**: Ghép đúng văn kiện / chọn đúng đường lối chiến lược kèm số trang *Giáo trình Lịch sử Đảng Cộng sản Việt Nam 2021*.
    - **Bước B (Đại Cảnh Tương Tác Bùng Nổ)**: Kích hoạt đại cảnh điện ảnh ngay trên canvas (Thắp đuốc Cương lĩnh 1930, Kéo cờ Ba Đình 2/9/1945, Cờ tung bay Điện Biên & Xe tăng tiến vào Dinh Độc Lập 30/4/1975, Đại đô thị số 2045).
