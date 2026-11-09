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
  - `QuestSystem.ts`: Quản lý chuỗi nhiệm vụ chính/phụ và máy trạng thái phụ thuộc mục tiêu (`locked` -> `active` -> `completed`).
  - `DialogueSystem.ts`: Cây hội thoại đa lựa chọn cho NPC có gắn nhãn xuất xứ (`ProvenanceMetadata`) và phần thưởng cờ/tư liệu/cột mốc.
  - `EvidenceSystem.ts` & `InventorySystem.ts`: Kho tư liệu lịch sử có dẫn nguồn giáo trình và bộ lọc theo thời kỳ.
  - `InteractionSystem.ts`: Phát hiện tương tác ưu tiên theo khoảng cách (Cửa/Portal, NPC, Tư liệu, Cột mốc, Đèn Báo An Toàn, Cổng Thời Gian).
- `src/ui/`:
  - Giao diện DOM toàn màn hình (Fullscreen World Viewport 100%, thanh Chronicle Ribbon gọn nhẹ) theo hệ màu 12 Sắc Độ Sơn Mài & Giấy Dó (`DESIGN.md`), hoàn toàn chỉ đọc trạng thái và gửi hành động qua `GameStore`, không tự ý mở khóa tiến trình.
  - `GameHUD.ts`: Hộp theo dõi mục tiêu nhiệm vụ đang thực hiện (`#hud-objective-tracker`) và gợi ý dẫn đường.
  - `DialoguePanel.ts`: Hộp thoại NPC đa lựa chọn và thẻ xuất xứ tư liệu (`#dialogue-provenance-badge`).
  - `JournalPanel.ts`: Nhật ký nhiệm vụ chính/phụ (`[J]`, `#journal-modal`).
  - `InventoryPanel.ts`: Túi tư liệu & trình tra cứu bằng chứng lịch sử (`[I]`, `#inventory-modal`).
  - `WorldMapPanel.ts`: Sơ đồ liên thông 8 khu vực Chương I (`[M]`, `#world-map-modal`), hiển thị trạng thái Đang đứng / Đã khám phá / Khu vực ẩn và điều khiển Dịch chuyển nhanh giữa các Đèn Báo An Toàn (`checkpoint`) đã mở khóa.
  - `PausePanel.ts`: Menu tạm dừng, lưu/xóa tiến trình và thiết lập trợ năng (`[Tab]`, `#pause-modal`).
- `tools/validate_maps.ts`:
  - Công cụ dòng lệnh (`npm run validate:maps`) chạy trực tiếp trên Node 24 (`--experimental-strip-types`), kiểm định toàn bộ file `.tmj` trong `public/maps/ch1/` cùng đồ thị `CHAPTER1_WORLD_GRAPH` (kiểm tra va chạm spawn, khoảng cách an toàn với portal, và BFS `16×16` tới mọi lối ra/mục tiêu).


