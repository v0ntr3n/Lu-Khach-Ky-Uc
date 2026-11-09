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

---

## Milestone M3 — RPG Systems & UI (Hoàn tất)

### 1. Hạng mục đã kiểm chứng

1. **Hệ thống Nhiệm vụ & Phụ thuộc Mục tiêu (`src/systems/QuestSystem.ts`)**:
   - Triển khai chuỗi nhiệm vụ chính Chương I (`q_ch1_main_unification`, 4 bước tuần tự có điều kiện tiên quyết `requiresMilestones` / `requiresEvidence`) và nhiệm vụ phụ (`q_ch1_side_dock_watch`, 2 bước tại `small_pier`), cùng nhiệm vụ chính cho các Chương II, III, IV.
   - Tự động mở khóa bước tiếp theo khi người chơi thu thập tư liệu, khôi phục cột mốc hoặc đối thoại NPC; cô lập hoàn toàn khi bật Chế độ Giảng Viên (`isPresenterMode`).
2. **Hệ thống Đối thoại Đa lựa chọn (`src/systems/DialogueSystem.ts` & `src/ui/DialoguePanel.ts`)**:
   - Cây hội thoại nhiều nút cho toàn bộ 8 NPC Chương I (`dlg_harbor_liaison`, `dlg_old_worker`, `dlg_dock_watchman`, `dlg_warehouse_keeper`, `dlg_street_vendor`, `dlg_liaison_inner`, `dlg_pier_boatman`, `dlg_archive_keeper`).
   - Gắn nhãn xuất xứ tư liệu (`#dialogue-provenance-badge`: `TƯ LIỆU LỊCH SỬ XÁC THỰC` có dẫn trang *Giáo trình Lịch sử Đảng Cộng sản Việt Nam 2021* hoặc `NHÂN VẬT GIAO LIÊN HƯ CẤU`).
3. **Hệ thống Tư liệu, Bằng chứng & Túi đồ (`src/systems/EvidenceSystem.ts`, `src/systems/InventorySystem.ts`, `src/ui/InventoryPanel.ts`)**:
   - Quản lý 8 tư liệu lịch sử có trích dẫn nguyên văn, ý nghĩa lịch sử và chú thích nguồn giáo trình.
   - Giao diện `#inventory-modal` (`[I]`) cho phép tra cứu chi tiết từng tư liệu đã thu thập và ẩn nội dung tư liệu chưa mở khóa.
4. **Bố cục Fullscreen World Viewport & Các Bảng Điều Khiển (`GameHUD.ts`, `JournalPanel.ts`, `PausePanel.ts`)**:
   - Loại bỏ hoàn toàn cột sidebar 1/5 cố định (`260px`), chuyển `.workspace-grid` sang rộng 100% màn hình và biến danh sách mốc thành dải **Chronicle Ribbon Bar** gọn nhẹ phía trên cùng hộp theo dõi mục tiêu `#hud-objective-tracker`.
   - Bổ sung Nhật ký nhiệm vụ `#journal-modal` (`[J]`), Túi tư liệu `#inventory-modal` (`[I]`), và Tạm dừng / Thiết lập `#pause-modal` (`[Tab]`) hỗ trợ Lưu thủ công, Xóa lưu trữ, Giảm chuyển động (`body.reduced-motion`), Cỡ chữ lớn (`body.text-scale-large`) và Bật/tắt gợi ý.

### 2. Kết quả thực thi kiểm thử M3

- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 37.95 kB, `dist/assets/index-*.js` 221.96 kB / gzip 59.40 kB).
- **Unit & Map Validation Tests (`npm run test:unit`)**: **28/28 passed** across 5 test files (`m0_stabilization.test.ts`: 12, `m1_runtime.test.ts`: 4, `m1_maps.test.ts`: 4, `m2_world_graph.test.ts`: 4, `m3_rpg_systems.test.ts`: 4).
- **Browser E2E Tests (`npm run test:e2e`)**: **15/15 passed** trên Chromium (`m0_stabilization.spec.ts`: 5, `m1_topdown.spec.ts`: 4, `m2_exploration.spec.ts`: 3, `m3_systems_ui.spec.ts`: 3).
- **Visual Audit Screenshots**: Đã kiểm tra trực quan `m3_dialogue_choices.png`, `m3_journal_modal.png`, `m3_inventory_modal.png` (bố cục 100% fullscreen, chữ rõ nét, đúng bảng màu Sơn Mài & Giấy Dó).

---

## Milestone M4 — Chapter I Complete Vertical Slice (Hoàn tất)

### 1. Hạng mục đã kiểm chứng

1. **Hệ thống Stealth & Tuần tra 2D (`src/systems/StealthSystem.ts`)**:
   - Nón tầm nhìn hình học (`coneLength`, `coneAngleRad`, `facingAngleRad`) kết hợp kiểm tra mái hiên ẩn nấp (`ShelterZone`) và tường chắn tầm nhìn (`CollisionWorld.hasLineOfSight` với `obstacles` & `occluders`).
   - Giữ phím `S` ngoài khoảng trống tuyệt đối không bất tử.
   - Ba trạng thái cảnh giác (`calm -> suspicious -> alert`): khi bị phát hiện hoàn toàn (`suspicionLevel >= 1`), nhân vật rút lui về Đèn Báo An Toàn (`checkpoint`) gần nhất trên bản đồ hiện tại mà **giữ nguyên 100% tư liệu, cờ nhiệm vụ và cột mốc đã thu thập**.
   - Phần thưởng nhiệm vụ phụ (`ev_ch1_patrol_schedule` tại `small_pier`) giảm 22% tầm quét nón đèn và tốc độ tăng nghi ngờ của lính tuần tra.
2. **Giải đố Đối chiếu Văn kiện Lịch sử (`src/ui/PuzzlePanel.ts` & `CH1_DOSSIER_PUZZLE`)**:
   - Bàn đối chiếu văn kiện Hội nghị Hợp nhất (`puzzle_ch1_unification_dossier` tại `secret_archive`) yêu cầu xác thực 3 luận điểm cốt lõi của *Chánh cương vắn tắt, Sách lược vắn tắt* (02/1930).
   - Tự động hiển thị gợi ý học thuật (`#dossier-progressive-hint`, dẫn nguồn *Giáo trình Lịch sử Đảng Cộng sản Việt Nam 2021, tr. 61–65*) sau 2 lần thử chưa đúng mà không phạt hay xóa tiến trình người chơi.
3. **Kịch bản Chơi Hoàn Chỉnh Chương I từ Đầu đến Cuối Không Dùng Cheat (`m4_chapter1_playthrough.spec.ts`)**:
   - Khởi đầu tại `harbor_square` (`isPresenterMode === false`, `0 / 12 CỘT MỐC`) -> trò chuyện Giao Liên Lâm Văn Bính (Mốc I) -> vào `warehouse_interior` thu thập `ev_ch1_shipping_ledger` -> rẽ sang `small_pier` hoàn thành nhiệm vụ phụ `q_ch1_side_dock_watch` (`npc_pier_boatman` + `ev_ch1_patrol_schedule`) -> qua `old_street` vào `safehouse_interior` giải mã `ev_ch1_cipher_note` (Mốc II) -> vượt `shadow_alley` & `archive_courtyard` tới `secret_archive` -> giải đố `puzzle_ch1_unification_dossier` (Mốc III + `ev_ch1_unification_appeal`) -> mở `climax_gate_1930`, vượt qua Bước A & Bước B (`Thắp Sáng Ngọn Đuốc 1930`).

### 2. Kết quả thực thi kiểm thử M4

- **Map & WorldGraph Validator (`npm run validate:maps`)**: **8/8 zones PASS** (0 lỗi).
- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 45.16 kB, `dist/assets/index-*.js` 238.83 kB / gzip 63.95 kB).
- **Unit & Map Validation Tests (`npm run test:unit`)**: **30/30 passed** across 6 test files (`m0_stabilization`: 12, `m1_runtime`: 4, `m1_maps`: 4, `m2_world_graph`: 4, `m3_rpg_systems`: 4, `m4_vertical_slice`: 2).
- **Browser E2E Tests (`npm run test:e2e`)**: **17/17 passed** trên Chromium (`m0_stabilization`: 5, `m1_topdown`: 4, `m2_exploration`: 3, `m3_systems_ui`: 3, `m4_chapter1_playthrough`: 2).
- **Visual Audit Screenshots**: Đã kiểm tra trực quan `m4_stealth_alley.png`, `m4_dossier_puzzle.png`, `m4_climax_1930.png`.



