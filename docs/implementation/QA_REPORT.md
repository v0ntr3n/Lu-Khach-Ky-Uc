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
