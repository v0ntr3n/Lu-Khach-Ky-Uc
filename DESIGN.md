# DESIGN.md — Hệ Thống Thiết Kế "Sơn Mài & Giấy Dó" HD-2D (*Lữ Khách Ký Ức: 1930 – 2045*)

Tựa game **Lữ Khách Ký Ức (1930 – 2045)** kế thừa vũ trụ thị giác của [`v0ntr3n/SPST`](https://github.com/v0ntr3n/SPST) và tiêu chuẩn biên tập từ [`pbakaus/impeccable`](https://github.com/pbakaus/impeccable), vận hành như một **trải nghiệm RPG Pixel-Art Toàn Màn Hình (`100vw × 100dvh`, Dynamic Integer-Scaled)** với bảng màu **12 Sắc Độ Sơn Mài & Giấy Dó**.

---

## 1. Vòng Đời Trò Chơi & Mở Màn Nứt Vỡ Thời Gian (`BOOT → MAIN_MENU → LOADING → OPENING → PLAYING`)

Ứng dụng khởi động trực tiếp vào **Màn Hình Menu Chính Điện Ảnh (`#main-menu-screen`)** tái hiện đêm mưa bến cảng Cửu Long (Hương Cảng 1930) với chiều sâu kiến trúc, ánh đèn lồng vàng đồng, làn sương mỏng và điều hướng bàn phím đầy đủ (`BẮT ĐẦU`, `TIẾP TỤC`, `MULTIPLAYER — Sắp ra mắt`, `HƯỚNG DẪN`, `CÀI ĐẶT`). Thế giới trò chơi chỉ được nạp sau khi người chơi chọn `BẮT ĐẦU` hoặc `TIẾP TỤC`.

Sau màn nạp theo từng giai đoạn (`LoadingScreen`), Lữ Khách bước qua **Vết Nứt Không Gian – Thời Gian (`#rift-opening-overlay`, `4.8s` cho hành trình mới / `1.8s` khi tiếp tục)** gồm 5 pha:
1. **Phase A — True Black (`0.0s – 0.6s`)**: Khung nhìn phủ `#000000` tuyệt đối, khóa thao tác.
2. **Phase B — First Fracture (`0.6s – 1.5s`)**: Vết nứt thời gian mảnh sắc xuất hiện tại tâm màn hình, hé lộ ánh sáng vàng đồng cổ (`#D4A55D`) từ chiều không gian lịch sử bên dưới.
3. **Phase C — Fracture Propagation (`1.5s – 2.9s`)**: Các nhánh nứt thứ cấp lan ra 4 góc theo bộ sinh số giả ngẫu nhiên cố định hạt giống (`mulberry32`), chia màng đen thành 16 mảnh đa giác răng cưa.
4. **Phase D — Catastrophic Tear (`2.9s – 4.4s`)**: 16 mảnh đa giác tách rời từ tâm văng ra 4 phía với gia tốc phi tuyến, để lộ trực tiếp khung cảnh Pixel HD-2D.
5. **Phase E — Complete Reveal (`4.4s – 4.8s`)**: Ẩn lớp phủ và trao quyền điều khiển Lữ Khách 2045.

---

## 2. Hệ Thống Bảng Màu Pixel 12 Sắc Độ (`oklch()` — Sơn Mài & Giấy Dó)

Tuyệt đối loại bỏ màu neon tím/xanh dương AI slop, thẻ web chung chung hoặc hiệu ứng kính mờ nặng nề. Toàn bộ ma trận điểm ảnh (`SpriteAtlas`), bản đồ Tiled `.tmj` 5 lớp, giao diện DOM độ phân giải cao và ánh sáng Chiaroscuro chỉ sử dụng 12 sắc độ:

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

## 3. Khung Vỏ Toàn Màn Hình (`GameShell`), Tỷ Lệ Nhân Vật & Kiểu Chữ Tiếng Việt

1. **Khung Vỏ Toàn Màn Hình (`#game-shell` — `100vw × 100dvh`)**:
   - Loại bỏ hoàn toàn thanh điều hướng kiểu trang web, thanh bên cố định, biểu ngữ chương cỡ lớn bên ngoài khung vẽ và thanh nút dưới đáy trên máy tính.
   - `GameShell` tính toán hệ số phóng đại số nguyên (`pixelScale >= 2`, `image-rendering: pixelated`, `imageSmoothingEnabled = false`) theo 3 mức thu phóng Camera (`wide` / `normal` / `close`). Ở màn hình `1920×1080` mặc định (`normal`, `4×`), nhân vật chính (`16×22` px logic) hiển thị cao **88 CSS px** sắc nét, dễ đọc với 4 hướng đi bộ, nhịp thở nghỉ, tư thế cúi nấp và bóng tiếp đất rõ ràng; đồng thời giữ nguyên hộp va chạm chân (`12×8`) và tọa độ thế giới.
   - `FullscreenManager` hỗ trợ API Toàn màn hình trình duyệt (`F` / nút Cài đặt) từ thao tác người dùng kèm cơ chế dự phòng tràn khung nhìn an toàn (`body.is-viewport-fullscreen`).
2. **HUD Ngữ Cảnh Tối Giản & Kiểu Chữ DOM Sắc Nét**:
   - Mọi văn bản tiếng Việt đều dựng trên lớp phủ DOM/CSS độ phân giải cao (tuyệt đối không vẽ chữ 6–8px nhòe trên Canvas): cỡ chữ giao diện `>= 16px`, lời thoại NPC `18–22px` trên máy tính.
   - **Góc trên trái**: Chương & khu vực hiện tại gọn nhẹ, kèm huy hiệu ẩn nấp / tuần tra chỉ xuất hiện khi liên quan.
   - **Góc trên phải**: Một mục tiêu nhiệm vụ ngắn gọn (`#hud-objective-tracker`) + nút Tạm dừng `[Esc]`.
   - **Nhãn tương tác nổi (`#world-interaction-prompt`)**: Ánh xạ từ tọa độ thế giới sang màn hình (`E · Trò chuyện` / `E · Tương tác`).
   - **Hộp thoại dưới đáy (`#dialogue-modal`)**: Chỉ xuất hiện khi đang đối thoại, phân biệt minh bạch `historical_fact` (Triện Đỏ Son), `fictional_npc` (Giấy Dó) và `speculative_2045` (Đồng Cổ 2045).
   - **Các sổ tay theo yêu cầu**: Nhật ký & Biên Niên Sử La Mã I–XII (`JournalPanel` — `J`), Túi Tư liệu (`InventoryPanel` — `I`), Sơ đồ 14 khu vực (`WorldMapPanel` — `M`), Cố vấn Sử học (`Q`), Tạm dừng (`Esc`/`Tab`) và Cài đặt hệ thống (`SettingsMenu`).
   - **Cô lập Chế độ Giảng viên (`#presenter-drawer`)**: Công cụ chuyển chương, dịch chuyển cột mốc, mở khóa cô lập 12 mốc và lớp phủ kiểm định (`F3`) được tách riêng hoàn toàn, chỉ hiển thị khi bật tường minh trong `CÀI ĐẶT`, tham số `?presenter=1` hoặc tổ hợp `Ctrl+Shift+P`.
3. **Đáp Ứng Đa Thiết Bị & Trợ Năng**:
   - Tương thích từ `1366×768`, `1920×1080`, `2560×1440` đến điện thoại xoay ngang (`844×390`, chỉ hiện cụm điều khiển cảm ứng trên thiết bị cảm ứng / mobile landscape), hỗ trợ Giảm chuyển động (`body.reduced-motion`), Chất lượng hiệu ứng (`full` / `subtle`) và Cỡ chữ lớn (`body.text-scale-large`).
