# Lữ Khách Ký Ức: Biên Niên Sử Sơn Mài (1930 – 2045)

Tựa game **2D Top-Down Pixel-Art RPG Toàn Màn Hình & Giải Đố Lịch Sử (`100vw × 100dvh`, Dynamic Integer-Scaled)** lấy cảm hứng từ cấu trúc bản đồ liên thông của *WorkAdventure* và phong cách thị giác **Sơn Mài & Giấy Dó** (`pbakaus/impeccable`), bám sát toàn văn **Giáo trình Lịch sử Đảng Cộng sản Việt Nam** (Bộ GD&ĐT, NXB Chính trị quốc gia Sự thật, 2021 — 440 trang).

---

## 1. Bối Cảnh & 4 Chương Lịch Sử (14 Bản Đồ Tiled `.tmj` Liên Thông)

Khởi đầu từ **Menu Chính Điện Ảnh** trong đêm mưa bến cảng Hương Cảng (1930), một **Lữ Khách Ký Ức** từ năm **2045** bước qua **Vết Nứt Không Gian – Thời Gian (16-Shard Tectonic Rift)** để khám phá 14 khu vực liên thông qua 4 thời kỳ lịch sử trọng đại và khôi phục **12 Cột Mốc La Mã (`I – XII`)**:

| Chương | Thời Kỳ & Cụm Bản Đồ Tiled (`.tmj`) | Cơ Chế RPG & Giải Đố Lịch Sử | Climax Cuối Chương (Bước A + Bước B) |
| :--- | :--- | :--- | :--- |
| **Chương I** | **1930 · Đêm Mưa Cửu Long (Hương Cảng)**<br>8 bản đồ: `harbor_square`, `warehouse_interior`, `old_street`, `small_pier`, `safehouse_interior`, `shadow_alley`, `archive_courtyard`, `secret_archive` | Khám phá đa khu vực, đối thoại NPC đa lựa chọn, thu thập 6 tư liệu mật, ẩn nấp dưới mái hiên tránh góc nhìn nón tuần tra (`StealthSystem`), và đối chiếu 3 mảnh ghép *Cương lĩnh chính trị đầu tiên* trên Bàn Kiểm Định Hồ Sơ (`PuzzlePanel`). | **A:** Xác lập *Cương lĩnh chính trị đầu tiên* (`tr. 36–43`).<br>**B:** Đại cảnh thắp đuốc Cương lĩnh & bão táp Xô Viết Nghệ Tĩnh. |
| **Chương II** | **1945 · Tân Trào → Quảng Trường Ba Đình**<br>2 bản đồ: `tan_trao_banyan`, `ba_dinh_square` | Khôi phục các mốc chuyển hướng chiến lược (1936–1939, Trung ương 8 năm 1941) và phát lệnh Tổng khởi nghĩa Tháng Tám từ Cây đa Tân Trào về Ba Đình. | **A:** Giải mã *Chỉ thị Nhật – Pháp bắn nhau* & Thời cơ Tháng Tám (`tr. 101–124`).<br>**B:** Đại cảnh kéo Quốc kỳ tại Quảng trường Ba Đình 2/9/1945. |
| **Chương III** | **1954–1975 · Đèo Pha Đin → Mùa Xuân 1975**<br>2 bản đồ: `pha_din_pass`, `spring_1975_corridor` | Kết nối tuyến lửa xe đạp thồ Điện Biên Phủ (1954), Nghị quyết 15 (1959) và bản đồ tác chiến 5 cánh quân thần tốc mùa xuân 1975. | **A:** Quyết sách *"Đánh chắc, tiến chắc"* & *Đại thắng Mùa Xuân 1975* (`tr. 172–237`).<br>**B:** Cờ chiến thắng tung bay & Xe tăng tiến vào Dinh Độc Lập 30/4/1975. |
| **Chương IV** | **1986–2045 · Đại Lộ Đổi Mới → Kỷ Nguyên Số**<br>2 bản đồ: `doi_moi_avenue`, `nexus_2045_plaza` | Khám phá bước ngoặt Đổi Mới Đại hội VI (1986), Cương lĩnh 1991/2011 và hoàn thiện hệ thống lưu trữ quốc gia hướng tới mục tiêu 2030 – 2045. | **A:** Đường lối *Đổi Mới toàn diện* Đại hội VI–XIII (`tr. 256–395`).<br>**B:** Đại cảnh đoàn tàu cao tốc & bầu trời pháo hoa Kỷ nguyên vươn mình 2045. |

---

## 2. Bảng Điều Khiển & Phím Tắt

| Phím / Thao tác | Chức năng |
| :--- | :--- |
| `W / A / S / D` hoặc `↑ / ← / ↓ / →` | Di chuyển 8 hướng chuẩn hóa vận tốc chéo (`1 / √2`) trong thế giới hoặc điều hướng Menu Chính |
| `Shift` (hoặc đứng dưới mái hiên / tán đa) | Ẩn mình trong vùng bóng tối (`ShelterZone`) tránh nón tầm nhìn tuần tra |
| `E` hoặc `Space` (Nhấn 1 lần) | Tương tác ngữ cảnh: Trò chuyện NPC, Thu thập Tư liệu, Mở Cổng/Cửa, Kích hoạt Đèn An Toàn, Mở Bàn Kiểm Định |
| `J` | Mở / Đóng **Nhật Ký Nhiệm Vụ & Biên Niên Sử La Mã I–XII** (`#journal-modal`) |
| `I` | Mở / Đóng **Túi Tư Liệu & Bằng Chứng Lịch Sử** (`#inventory-modal`) |
| `M` | Mở / Đóng **Sơ Đồ Liên Thông Khu Vực & Dịch Chuyển Nhanh** (`#world-map-modal`) |
| `Q` | Mở / Đóng **Bàn Cố Vấn Sử Học** kèm số trang Giáo trình 2021 (`#lookup-drawer`) |
| `F` (hoặc `F11`) | Bật / Tắt **Chế độ Toàn màn hình** (`FullscreenManager` + dự phòng tràn khung nhìn) |
| `H` | Bật / Tắt nhanh dòng gợi ý dẫn đường trên HUD (`#hud-active-objective-hint`) |
| `Esc` hoặc `Tab` | Mở **Tạm Dừng** (`#pause-modal`: Tiếp tục, Cài đặt, Lưu, Về Menu Chính) / Đóng cửa sổ đang mở |
| `1 .. 4` | Chọn nhanh nhánh hội thoại NPC hoặc phương án Quyết sách Climax |
| `Ctrl + Shift + P` (hoặc `?presenter=1`) | Mở khóa & bật **Ngăn Điều Khiển Giảng Viên / Kiểm Định (`F3`)** độc lập, không ghi đè tiến trình lưu của người chơi |

---

## 3. Cài Đặt, Kiểm Định Bản Đồ & Chạy Kiểm Thử

```bash
npm install

# 1. Chạy máy chủ phát triển Vite (http://127.0.0.1:5173)
npm run dev

# 2. Kiểm tra kiểu TypeScript nghiêm ngặt
npm run typecheck

# 3. Kiểm định toàn bộ 14 bản đồ Tiled (.tmj) và đồ thị liên thông
npm run validate:maps

# 4. Đóng gói bản dựng sản xuất (dist/)
npm run build

# 5. Chạy bộ kiểm thử đơn vị Vitest (M0 -> M7)
npm run test:unit

# 6. Chạy bộ kiểm thử End-to-End Playwright Chromium trên trình duyệt thực
npm run test:e2e
```

---

## 4. Nguyên Tắc Minh Bạch Nguồn Sử Liệu & Tài Nguyên Đồ Họa

1. **Phân định rõ ràng 3 cấp độ nguồn dữ liệu (`ProvenanceType`)**:
   - `historical_fact` (**Sử liệu xác thực**): Sự kiện, ngày tháng, văn kiện và ý nghĩa lịch sử trích dẫn trực tiếp kèm số trang từ *Giáo trình Lịch sử Đảng Cộng sản Việt Nam (Bộ GD&ĐT, NXB Chính trị quốc gia Sự thật, 2021)*.
   - `fictional_npc` (**Nhân vật / tình tiết hư cấu dẫn chuyện**): Các nhân vật giao liên, công nhân bến cảng, học giả lưu trữ được gắn nhãn minh bạch trên hộp thoại và nhật ký.
   - `speculative_2045` (**Khung viễn tưởng Kỷ nguyên số 2045**): Bối cảnh Lữ Khách Lưu Trữ Quốc Gia năm 2045 kỷ niệm 100 năm lập nước.
2. **Tài nguyên Pixel Art & Sprite Atlas (`src/data/provenance/sprite_atlas_manifest.ts`)**:
   - Toàn bộ nhân vật 4 hướng (`16×22`, `down`, `up`, `left`, `right`) và tĩnh vật `16×16` tuân thủ nghiêm ngặt bảng 12 sắc độ **Sơn Mài & Giấy Dó** (`DESIGN.md`) và quy tắc dựng điểm ảnh nguyên (`image-rendering: pixelated`, `imageSmoothingEnabled = false`).
