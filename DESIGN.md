# DESIGN.md — Hệ Thống Thiết Kế "Sơn Mài & Giấy Dó" HD-2D (*Lữ Khách Ký Ức: 1930 – 2045*)

Tựa game **Lữ Khách Ký Ức (1930 – 2045)** kế thừa vũ trụ thị giác của [`v0ntr3n/SPST`](https://github.com/v0ntr3n/SPST) và tiêu chuẩn biên tập từ [`pbakaus/impeccable`](https://github.com/pbakaus/impeccable), vận hành trên kiến trúc **2D Top-Down RPG Exploration (Góc nhìn 3/4, 320×180 Integer-Scaled)** với bảng màu **12 Sắc Độ Sơn Mài & Giấy Dó**.

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

Tuyệt đối loại bỏ màu neon tím/xanh dương AI slop. Toàn bộ ma trận điểm ảnh (`SpriteAtlas`), bản đồ Tiled `.tmj` 5 lớp và ánh sáng Chiaroscuro chỉ sử dụng 12 sắc độ:

| Mã Token | Hex | Vai trò trong Khung Hình Pixel HD-2D & UI |
| :--- | :--- | :--- |
| `--lacquer-void` | `#110E0B` | Nền đêm Sơn Mài sâu thẳm, viền bóng tối nhân vật |
| `--lacquer-deep` | `#1B1612` | Lớp nền giao diện, mặt nước cảng đêm, tường sâu |
| `--lacquer-elevated` | `#241D17` | Lớp mái ngói, mặt đường đá cuội, thẻ HUD nổi |
| `--ink-primary` | `#232116` | Chữ chính trên nền Sổ tay Giấy Dó (Chuẩn AAA) |
| `--ink-secondary` | `#3E3A2C` | Chữ phụ, bóng đổ trung gian |
| `--bronze-muted` | `#6E4720` | Gỗ cổ, thân cây đa Tân Trào, thùng hàng bến cảng |
| `--bronze-antique` | `#8A5A2B` | Viền khung đồng cổ, chi tiết trang phục, bàn gỗ |
| `--bronze-bright` | `#D4A55D` | Ánh đèn dầu Cửu Long, tia sáng Chiaroscuro, sao vàng |
| `--crimson-seal` | `#9E3321` | Đỏ Son Triện, sắc cờ Cách mạng, dấu xác thực sử liệu |
| `--crimson-vermilion` | `#DF6B57` | Điểm sáng Chu Sa, đèn báo an toàn, tia lửa lịch sử |
| `--parchment-warm` | `#E6E5DC` | Nền Sổ tay Giấy Dó, thẻ nhiệm vụ, trang sách cổ |
| `--parchment-light` | `#F4EFE4` | Điểm sáng cao nhất (Highlight), chữ trên nền Sơn Mài |

---

## 3. Kiến Trúc Không Gian Top-Down 2D & Giao Diện Toàn Màn Hình (M1 – M6)

1. **Khung Nhìn Thế Giới Toàn Màn Hình (`.workspace-grid` 100% Width)**:
   - Thay thế cột dọc `1/5` cũ bằng **Dải Biên Niên Sử La Mã Ngang (`#codex-sidebar.codex-sidebar`)** cao tối đa `54px`, giữ hiển thị tiến độ `#codex-progress-count` (`0 / 12 CỘT MỐC`) và 12 nút mốc `I – XII`.
   - Khung dựng hình `#pixel-canvas` (`320×180` Integer-Scaled, `imageSmoothingEnabled = false`) sắp xếp độ sâu động theo trục Y (`footY`) cho nhân vật 4 hướng, NPC, lính tuần tra và lớp mái hiên/tán cây (`Foreground`).
2. **HUD Ngữ Cảnh & Hệ Thống Cửa Sổ Độc Quyền Trạng Thái (`GameMode`)**:
   - **Objective Tracker (`#hud-objective-tracker`)**: Hiển thị nhiệm vụ chính đang kích hoạt, bước mục tiêu hiện hành và dòng gợi ý dẫn đường (`#hud-active-objective-hint`, bật/tắt bằng phím `H`).
   - **Hộp Thoại Đa Nhánh & Nhãn Nguồn Sử Liệu (`DialoguePanel` / `#dialogue-modal`)**: Phân biệt rõ `historical_fact` (Triện Đỏ Son), `fictional_npc` (Giấy Dó) và `speculative_2045` (Đồng Cổ 2045).
   - **Nhật Ký Nhiệm Vụ (`JournalPanel` — `J`)**, **Túi Tư Liệu (`InventoryPanel` — `I`)**, **Sơ Đồ Liên Thông 14 Khu Vực (`WorldMapPanel` — `M`)**, **Bàn Kiểm Định Hồ Sơ (`PuzzlePanel`)**, **Cố Vấn Sử Học (`Q`)** và **Tạm Dừng / Trợ Năng (`PausePanel` — `Tab`/`Esc`)** đều khóa hoàn toàn điều khiển thế giới khi mở và cuộn mượt (`max-height: 90vh; overflow-y: auto`).
3. **Đáp Ứng Đa Thiết Bị & Trợ Năng**:
   - Hỗ trợ đầy đủ cả màn hình máy tính (`1280×800`) và điện thoại xoay ngang (`844×390` qua `@media (max-height: 500px) and (orientation: landscape)`), chế độ Giảm chuyển động (`body.reduced-motion`) và Cỡ chữ lớn (`body.text-scale-large`).
