# Lữ Khách Ký Ức (1930 – 2045) — HD-2D Historical Pixel Game

Tựa game **Đi Cảnh Giải Đố & Điện Ảnh Pixel HD-2D (Side-Scrolling Narrative Puzzle)** lấy cảm hứng từ *Valiant Hearts* và *Octopath Traveler*, được xây dựng trên hệ thống thiết kế **Sơn Mài & Giấy Dó** (`pbakaus/impeccable`) và bám sát **Giáo trình Lịch sử Đảng Cộng sản Việt Nam** (Bộ GD&ĐT, NXB Chính trị quốc gia Sự thật, 2021 — 440 trang).

---

## 1. Cốt Truyện & Lối Chơi ("Lữ Khách Ký Ức 2045")

Năm **2045** (Kỷ niệm 100 năm thành lập nước Việt Nam Dân chủ Cộng hòa / Cộng hòa Xã hội chủ nghĩa Việt Nam), một **Lữ Khách Ký Ức** bước qua **Vết Nứt Không Gian – Thời Gian (16-Shard Tectonic Rift)** để quay ngược về 4 thời kỳ lịch sử trọng đại:

| Chương | Thời Kỳ & Bối Cảnh Pixel HD-2D | Tương Tác Đi Cảnh (Mức 1 — Điện ảnh) | Climax Cuối Chương (A: Quyết Sách + B: Đại Cảnh) |
| :--- | :--- | :--- | :--- |
| **Chương I** | **1930 · Đêm Mưa Cửu Long (Hương Cảng)** | Ẩn mình dưới bóng hiên nhà tránh đèn rọi mật thám để thu thập 3 mảnh văn kiện hợp nhất. | **A:** Xác lập *Cương lĩnh chính trị đầu tiên* (`tr. 36–43`).<br>**B:** Đại cảnh thắp đuốc Cương lĩnh & bão táp Xô Viết Nghệ Tĩnh. |
| **Chương II** | **1945 · Tân Trào → Quảng Trường Ba Đình** | Kích hoạt các trạm truyền tin thần tốc từ Tân Trào về Hà Nội – Huế – Sài Gòn trước khi quân Đồng minh vào. | **A:** Giải mã *Chỉ thị Nhật – Pháp bắn nhau* (`tr. 101–124`).<br>**B:** Đại cảnh kéo Quốc kỳ tại Quảng trường Ba Đình ngày 2/9/1945. |
| **Chương III** | **1945–1975 · Điện Biên Phủ & Mùa Xuân 1975** | Đẩy xe đạp thồ vượt đèo dưới pháo sáng và kết nối 5 cánh quân thần tốc mùa xuân 1975. | **A:** Quyết sách *"Đánh chắc, tiến chắc"* & *Đại thắng 1975* (`tr. 172–237`).<br>**B:** Cờ tung bay trên hầm Đờ Cát & Xe tăng tiến vào Dinh Độc Lập 30/4/1975. |
| **Chương IV** | **1986–2045 · Đại Lộ Đổi Mới → Kỷ Nguyên Số** | Bước dọc con phố năm 1986, kích hoạt các trụ cột Đổi Mới để biến đổi kiến trúc thành siêu đô thị 2045. | **A:** Đường lối *Đổi Mới toàn diện* Đại hội VI–XIII (`tr. 256–395`).<br>**B:** Đại cảnh đoàn tàu cao tốc & bầu trời pháo hoa Kỷ nguyên vươn mình 2045. |

---

## 2. Điều Khiển & Công Cụ Hỗ Trợ Giảng Viên

- **Di chuyển**: Phím `A / D` hoặc `← / →` (hoặc nút cảm ứng trên màn hình).
- **Tương tác / Thu thập Di vật / Trò chuyện NPC**: Phím `E` hoặc `Space`.
- **Ẩn mình vào bóng tối / Trạm trú ẩn**: Phím `S` hoặc `↓` (hoặc đứng dưới mái hiên có biểu tượng bóng râm).
- **Tra cứu Cố vấn Sử học (AI Lookup kèm số trang Giáo trình 2021)**: Phím `Q`.
- **Chế độ Giảng Viên (`⚡ Mở Khóa Toàn Bộ / Tự Động Dẫn Đường`)**: Nút trên thanh Topbar cho phép nhảy tức thì tới bất kỳ mốc nào trong **12 Mốc La Mã (`I – XII`)** hoặc tự động điều khiển Lữ Khách đi qua toàn bộ 4 chương.

---

## 3. Chạy Dự Án & Kiến Trúc Thư Mục (`Vite + TypeScript`)

```bash
npm install
npm run dev      # Chạy máy chủ phát triển Vite
npm run build    # Kiểm tra kiểu TypeScript & đóng gói ra thư mục dist/
```

- `src/types/game.ts`: Định nghĩa cấu trúc dữ liệu Chương, Cột mốc La Mã (`I – XII`), Cổ vật, Vùng tương tác và Đại cảnh Climax.
- `src/data/chronicles.ts`: Toàn bộ dữ liệu lịch sử, câu hỏi quyết sách và trích dẫn số trang *Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)*.
- `src/engine/OpeningFracture.ts`: Hiệu ứng mở màn 4.8 giây nứt vỡ màng đen 16 mảnh đa giác (`mulberry32`).
- `src/engine/SpriteAtlas.ts`: Ma trận điểm ảnh 16-bit vẽ thủ tục cho Lữ Khách 2045, NPC lịch sử, xe đạp thồ, xe tăng 390, tàu điện 2045.
- `src/engine/PixelRenderer.ts`: Bộ dựng hình Canvas 2D `320×180` Integer-Scaled với 4 lớp Parallax và ánh sáng Chiaroscuro Sơn Mài.
- `src/engine/SoundSynth.ts`: Bộ tổng hợp âm thanh Chiptune Ngũ Cung 8-bit bằng Web Audio API (0KB tài nguyên ngoài).
- `src/ui/HudController.ts`: Điều khiển cột mục lục `1/5` (`I – XII`), hộp thoại RPG, Climax Modal và Bàn Cố vấn Sử học.
