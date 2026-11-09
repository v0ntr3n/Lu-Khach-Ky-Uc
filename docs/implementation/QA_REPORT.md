# Báo Cáo Kiểm Thử & Nghiệm Thu (QA Report) — Lữ Khách Ký Ức

Tài liệu này cập nhật liên tục kết quả kiểm thử tự động (Unit, Map Validation, Playwright E2E), hiệu năng và danh sách lỗi đã phát hiện/xử lý qua từng cột mốc M0 → M6.

---

## Milestone M0 — Gameplay Stabilization (Hoàn tất — PR #3)

- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 24.27 kB, `dist/assets/index-*.js` 66.12 kB / gzip 22.39 kB).
- **Unit & Integration Tests (`npm run test:unit`)**: `12 passed (12)` trong `tests/unit/m0_stabilization.test.ts`.
- **Browser E2E Tests (`npm run test:e2e`)**: `5 passed (5)` trên Chromium trong `tests/playwright/m0_stabilization.spec.ts`.

---

## Milestone M1 — Phaser 3 + Tiled Top-Down Runtime (Hoàn tất)

### 1. Hạng mục đã kiểm chứng

1. **Hai bản đồ Tiled chuẩn hóa (`public/maps/ch1/`)**:
   - `harbor_square.tmj` (`64×48` ô = `1024×768` px, `tileSize = 16`): Đầy đủ các lớp `Ground`, `GroundDetails`, `LowProps`, `Structures`, `Foreground`, `Collision`, `LightOccluders`, `NPCs`, `Interactables`, `Entrances`, `PatrolPaths`, `Checkpoints`.
   - `warehouse_interior.tmj` (`36×28` ô = `576×448` px, `tileSize = 16`): Cửa ra/vào 2 chiều nối với `harbor_square`, bàn tư liệu `evidence_shipping_ledger` và lối đi BFS thông suốt.
2. **Di chuyển 8 hướng chuẩn hóa (`1/√2`) & Va chạm bàn chân (`footRect`)**:
   - Vector chéo `W+D`, `W+A`, `S+D`, `S+A` được chuẩn hóa độ dài bằng `1.0`.
   - Hộp va chạm chân `12×8` trượt mượt dọc theo tường kho hàng (`CollisionWorld.resolveMovement`), chỉ bị chặn ở gốc cây (`16×16`) và đi xuyên dưới tán cây/mái hiên (`Foreground` depth `10000`).
3. **Sắp xếp chiều sâu theo trục Y (`footY`) & Camera kẹp biên**:
   - `computeEntityDepth(footY) = 100 + Math.round(footY)` đảm bảo nhân vật đứng phía bắc (`footY` nhỏ hơn) vẽ phía sau NPC/vật thể và khi đi xuống phía nam (`footY` lớn hơn) vẽ phía trước.
   - `CameraController` có vùng đệm `24×16` chống rung 1px và kẹp chặt trong biên `[0, widthPx - 320] × [0, heightPx - 180]`.
4. **Chuyển cảnh 2 chiều qua cửa có tên (`SceneTransition`) & Chống Portal Ping-Pong**:
   - Đi qua `exit_harbor_to_warehouse` đưa người chơi tới đúng `warehouse_south_door` (`272, 384`, hướng `'up'`), kích hoạt `portalCooldownSec = 0.65s` và cờ `requiresStepOffPortal` ngăn hiện tượng dội ngược lại bản đồ ngoài trời.
   - Đi qua `exit_warehouse_to_harbor` đưa người chơi trở lại `from_warehouse` (`512, 432`, hướng `'down'`).

### 2. Lỗi phát hiện qua Visual QA (Screenshot Audit) & Đã sửa trong M1

| ID | Mức độ | Phát hiện qua | Mô tả lỗi | Giải pháp đã sửa |
| :--- | :--- | :--- | :--- | :--- |
| **M1-V01** | **P1** | Ảnh chụp `m1_warehouse_interior.png` & `m1_harbor_square.png` | `MapLoader` dùng mã `tileId` `21..27` cho nội thất và `9..13` cho tường/mái hiên/tán cây ngoài trời, nhưng `drawTopDownTile` trong `SpriteAtlas.ts` chỉ xử lý `1..8` ở nội thất nên rơi vào nhánh `default` (vẽ đèn dầu lặp trên sàn). | Đồng bộ toàn bộ bảng mã `tileId` `1..13` (ngoài trời) và `1..9`, `21..30` (nội thất) trong `SpriteAtlas.ts`, đồng thời vẽ khung ngưỡng cửa đỏ son + vàng kim cho `mapData.portals` trong `PixelRenderer.ts`. |
| **M1-V02** | **P2** | Ảnh chụp `m1_harbor_square.png` | Nhãn phụ `.codex-sub` bên trái bị sát vào bộ đếm `0 / 12 CỘT MỐC`. | Thêm `gap: 8px; align-items: center;` cho `.codex-sub` và `white-space: nowrap;` cho `#codex-progress-count` trong `index.html`. |

### 3. Kết quả thực thi kiểm thử M1

- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 24.56 kB, `dist/assets/index-*.js` 113.00 kB / gzip 34.46 kB).
- **Unit & Map Validation Tests (`npm run test:unit`)**: **20/20 passed** (`m0_stabilization.test.ts`: 12, `m1_runtime.test.ts`: 4, `m1_maps.test.ts`: 4).
- **Browser E2E Tests (`npm run test:e2e`)**: **9/9 passed** trên Chromium (`m0_stabilization.spec.ts`: 5, `m1_topdown.spec.ts`: 4).

---

## Milestone M2 — World Exploration (Hoàn tất)

### 1. Hạng mục đã kiểm chứng

1. **Đồ thị thế giới 8 khu vực liên thông (`src/world/WorldGraph.ts` & `public/maps/ch1/*.tmj`)**:
   - Đầy đủ 8 bản đồ Chương I (Hương Cảng 1930): `harbor_square` (`64×48`), `warehouse_interior` (`36×28`), `old_street` (`48×40`), `small_pier` (`40×30`), `safehouse_interior` (`32×24`), `shadow_alley` (`44×32`), `archive_courtyard` (`44×34`), `secret_archive` (`32×24`).
   - Hỗ trợ cả tuyến đường chính (`harbor_square -> old_street -> shadow_alley -> archive_courtyard -> secret_archive`) và tuyến bến thuyền phụ (`harbor_square -> small_pier -> shadow_alley -> archive_courtyard -> secret_archive`).
2. **Công cụ CLI kiểm định bản đồ (`tools/validate_maps.ts` / `npm run validate:maps`)**:
   - Kiểm tra 8/8 file `.tmj` và `getAllBuiltInMaps()`, xác nhận 100% điểm spawn nằm ngoài vật cản, cách cổng dịch chuyển tối thiểu `20px`, và BFS lưới `16×16` tiếp cận được mọi cổng/mục tiêu.
3. **Bản đồ Thế giới trong game (`src/ui/WorldMapPanel.ts`, phím `[M]`) & Fast Travel có kiểm soát**:
   - Mở sơ đồ liên thông khóa hoàn toàn điều khiển thế giới (`mode === 'map'`).
   - Khu vực chưa tới hiển thị `KHU VỰC ẨN — Chưa khám phá` (không lộ nội dung cốt truyện).
   - Chỉ cho phép dịch chuyển nhanh giữa các khu vực đã khám phá VÀ đã kích hoạt Đèn Báo An Toàn (`checkpoint`), tuyệt đối không tự mở khóa cột mốc hay bằng chứng khi dịch chuyển.
4. **Lộ trình NPC (`stepNpcAlongRoute`) & Lớp phủ Gỡ lỗi (`[F3]` / `drawDebugOverlay`)**:
   - NPC tuần tra mượt qua các điểm `route` (`NpcRoutePoint[]`), tự cập nhật 4 hướng nhìn và dừng nghỉ `waitSec`.
   - Phím `F3` / nút `📐 Debug [F3]` hiển thị trực quan hộp va chạm đỏ, vùng ẩn nấp xanh ngọc, cổng xanh lá, tọa độ spawn/checkpoint xanh lam, bán kính tương tác vàng kim và đường đi NPC.

### 2. Lỗi phát hiện qua Hồi quy E2E & Đã sửa trong M2

| ID | Mức độ | Phát hiện qua | Mô tả lỗi | Giải pháp đã sửa |
| :--- | :--- | :--- | :--- | :--- |
| **M2-R01** | **P1** | Chạy hồi quy `m1_topdown.spec.ts` Test 2 | Cổng `portal_to_warehouse` mới thêm tại `x: 480, y: 400` chồng lên điểm kiểm thử va chạm tường phía nam nhà kho `(485, 412)` khiến nhân vật bị chuyển cảnh thay vì đứng lại trước tường. | Dời `portal_to_warehouse` về đúng ô cửa `x: 512, y: 400` trong `MapLoader.ts` và `harbor_square.tmj`, giữ nguyên đoạn tường đặc `x: 464..512`. |
| **M2-R02** | **P2** | Chạy hồi quy `m1_topdown.spec.ts` Test 3 | Đổi `title` của `warehouse_interior` thành `'Kho Hàng Số 4 — Cửu Long (1930)'` làm lệch kiểm tra nhãn `'Kho Hàng Bến Cảng (Nội Thất)'` của M1. | Chuẩn hóa tiêu đề thành `'Kho Hàng Số 4 — Kho Hàng Bến Cảng (Nội Thất)'` trong `MapLoader.ts`, `WorldGraph.ts` và `warehouse_interior.tmj`. |

### 3. Kết quả thực thi kiểm thử M2

- **Map & WorldGraph Validator (`npm run validate:maps`)**: **8/8 zones PASS** (0 lỗi).
- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 29.91 kB, `dist/assets/index-*.js` 162.67 kB / gzip 44.44 kB).
- **Unit & Map Validation Tests (`npm run test:unit`)**: **24/24 passed** (`m0_stabilization.test.ts`: 12, `m1_runtime.test.ts`: 4, `m1_maps.test.ts`: 4, `m2_world_graph.test.ts`: 4).
- **Browser E2E Tests (`npm run test:e2e`)**: **12/12 passed** trên Chromium (`m0_stabilization.spec.ts`: 5, `m1_topdown.spec.ts`: 4, `m2_exploration.spec.ts`: 3).

