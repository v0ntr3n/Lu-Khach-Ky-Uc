# Báo Cáo Kiểm Thử & Nghiệm Thu (QA Report) — Lữ Khách Ký Ức

Tài liệu này cập nhật liên tục kết quả kiểm thử tự động (Unit, Map Validation, Playwright E2E), hiệu năng và danh sách lỗi đã phát hiện/xử lý qua từng cột mốc M0 → M6.

---

## Milestone M0 — Gameplay Stabilization (Hoàn tất)

### 1. Danh sách lỗi P0 đã phát hiện và sửa chữa

| ID | Mức độ | Tập tin & Vị trí | Nguyên nhân gốc | Giải pháp đã triển khai | Kiểm thử hồi quy |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **M0-01** | **P0** | `src/main.ts` (`teleportToMilestone`) | Bấm vào cột mốc ở sidebar tự động thêm vào `collectedMilestones` và dịch chuyển người chơi. | Tách biệt `isPresenterMode` và `presenterCollectedMilestones` trong `GameStore`. Ở chế độ thường, bấm sidebar chỉ xem tư liệu đã mở hoặc hiện thông báo khóa. | `m0_stabilization.test.ts` #1, `m0_stabilization.spec.ts` #2, #5 |
| **M0-02** | **P0** | `src/main.ts` (`handleInteractAction`) | Mở Cổng Thời Gian Climax không kiểm tra xem đã thu thập đủ các cột mốc trong chương hay chưa. | Bổ sung `GameStore.canTriggerChapterClimax(chapter)` kiểm tra đủ 3 mốc của chương trước khi mở `#climax-modal`. | `m0_stabilization.test.ts` #2, `m0_stabilization.spec.ts` #4 |
| **M0-03** | **P0** | `src/main.ts` (`isPlayerInShelter`) | `if (this.isCrouching) return true;` làm người chơi giữ phím `S` ngoài đường trống trở nên bất tử trước đèn tuần tra. | `GameStore.isPlayerInShelter()` và `CollisionWorld.isPointInShelter()` chỉ trả về `true` khi tọa độ nằm trong `ShelterZone`; thêm `hazardCooldown = 1.0s`. | `m0_stabilization.test.ts` #3 |
| **M0-04** | **P0** | `src/main.ts` (`updatePhysics`) | Vòng lặp vật lý vẫn đọc phím di chuyển khi đang mở `#climax-modal`, `#lookup-drawer` hoặc `#rift-opening-overlay`. | Áp dụng máy trạng thái `GameMode` (`opening`, `exploring`, `climax`, `lookup`, ...); chỉ cho phép di chuyển khi `mode === 'exploring'`. | `m0_stabilization.test.ts` #4, `m0_stabilization.spec.ts` #1, #3 |
| **M0-05** | **P0** | `index.html` (`#climax-step-b`) & `OpeningFracture.ts` | Quy tắc CSS `#climax-step-b { display: flex }` ghi đè thuộc tính HTML `hidden`, làm lộ Bước B ngay khi mở modal; Phase A mở màn vẽ `#0D0A08` thay vì `#000000`. | Thêm `[hidden], #climax-step-b[hidden] { display: none !important; }` và vẽ `#000000` ở Phase A (`t < 0.6s`) ngay từ frame 0. | `m0_stabilization.spec.ts` #4 |
| **M0-06** | **P0** | `src/core/InputManager.ts` | Mất tiêu điểm cửa sổ (`blur`, `visibilitychange`) hoặc hủy cảm ứng (`pointercancel`, `lostpointercapture`) làm kẹt phím di chuyển. | Gắn listener xóa toàn bộ `keysPressed` khi `blur`, `visibilitychange`, `pointercancel`, `lostpointercapture`. | `m0_stabilization.test.ts` #5 |
| **M0-07** | **P0** | `src/core/GameApp.ts` (`triggerClimaxStepBSetPiece`) | `setTimeout` 1600ms chuyển chương sau Climax B không bị hủy nếu người chơi chuyển chương/cảnh trước khi hết giờ. | Bổ sung bộ đếm đơn điệu `transitionToken` (`bumpTransitionToken()`) để vô hiệu hóa mọi callback bất đồng bộ cũ. | `m0_stabilization.test.ts` #6 |
| **M0-08** | **P0** | `src/core/InputManager.ts` (`handleKeyDown`) | Giữ phím `E` / `Space` kích hoạt lặp lại hành động tương tác do sự kiện `e.repeat`. | Chặn `e.repeat === true` và khóa trong `consumedActionKeys` cho đến khi nhận sự kiện `keyup`. | `m0_stabilization.test.ts` #7 |

### 2. Kết quả thực thi kiểm thử M0

- **Build & Typecheck (`npm run build`)**: Pass (`dist/index.html` 24.27 kB, `dist/assets/index-*.js` 66.12 kB / gzip 22.39 kB).
- **Unit & Integration Tests (`npm run test:unit`)**: `12 passed (12)` trong `tests/unit/m0_stabilization.test.ts`.
- **Browser E2E Tests (`npm run test:e2e`)**: `5 passed (5)` trên Chromium trong `tests/playwright/m0_stabilization.spec.ts`.
