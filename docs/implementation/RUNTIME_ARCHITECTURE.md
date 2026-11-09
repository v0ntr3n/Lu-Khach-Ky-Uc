# Kiến Trúc Runtime Thực Tế — Lữ Khách Ký Ức (1930 – 2045)

Tài liệu này ghi nhận kiến trúc runtime thực tế được triển khai qua các cột mốc M0 → M6.

## 1. Tổng quan phân tầng (`Canonical Directory Layout`)

- `src/core/`:
  - `GameStore.ts`: Kho trạng thái có thẩm quyền duy nhất (Authoritative State Store), tách biệt hoàn toàn tiến trình người chơi chuẩn (`collectedMilestones`, `collectedEvidence`, `questFlags`) và chế độ trình chiếu (`isPresenterMode`, `presenterCollectedMilestones`).
  - `InputManager.ts`: Quản lý bàn phím 8 hướng (chuẩn hóa vector chéo `1/√2`), chặn sự kiện lặp phím (`e.repeat`) cho phím hành động, tự động xóa trạng thái phím khi mất tiêu điểm (`blur`, `visibilitychange`, `pointercancel`, `lostpointercapture`), và khóa thao tác thế giới khi `GameMode !== 'exploring'`.
  - `SaveManager.ts`: Lưu trữ cục bộ có phiên bản (`SaveDataV2`), tự động di trú an toàn từ `SaveDataV1` về điểm xuất phát có tên (`named spawn`), không bao giờ đặt nhân vật vào vùng vật cản.
  - `GameApp.ts`: Điều phối vòng đời game, bộ đếm thế hệ chuyển cảnh (`transitionToken`) chống race condition của các `setTimeout` bất đồng bộ.
- `src/world/`:
  - `CollisionWorld.ts`: Kiểm tra va chạm hình học AABB thuần túy (không đọc pixel từ sprite) và kiểm tra tầm nhìn/vùng ẩn nấp (`ShelterZone` / `LightOccluder`).
  - `MapValidator.ts`: Kiểm tra tính toàn vẹn của bản đồ Tiled (ID duy nhất, điểm spawn hợp lệ không chồng lấn vật cản, liên kết cổng 2 chiều, khả năng tiếp cận mục tiêu).
  - `WorldGraph.ts`: Đồ thị các khu vực liên thông và điều kiện mở khóa.
- `src/runtime/` & `src/scenes/`:
  - Tích hợp Phaser 3 + Tiled TMJ/JSON, điều khiển Camera bám mục tiêu có giới hạn biên, sắp xếp chiều sâu theo trục Y (`footY`), và chuyển cảnh hai chiều có thời gian miễn nhiễm (`portalCooldown`) chống hiệu ứng dội cổng (`portal ping-pong`).
- `src/systems/`:
  - `QuestSystem.ts`, `DialogueSystem.ts`, `InteractionSystem.ts`, `InventorySystem.ts`, `EvidenceSystem.ts`, `StealthSystem.ts`.
- `src/ui/`:
  - Giao diện DOM toàn màn hình theo hệ màu 12 Sắc Độ Sơn Mài & Giấy Dó (`DESIGN.md`), hoàn toàn chỉ đọc trạng thái và gửi hành động qua `GameStore`, không tự ý mở khóa tiến trình.
