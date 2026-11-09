/**
 * @file tests/unit/m1_runtime.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest) cho Cột mốc M1 — Phaser 3 + Tiled Runtime:
 * 1. Chuẩn hóa vector di chuyển 8 hướng (cardinal speed = 1.0, diagonal speed = 1.0 với ±1/√2).
 * 2. CameraController bám mượt (smooth follow), vùng trễ trung tâm (dead-zone) và kẹp biên nghiêm ngặt
 *    tại (0, 0) và (1024, 768) không bao giờ lộ khoảng đen (void).
 * 3. Sắp xếp chiều sâu theo trục Y (computeEntityDepth(footY) & PhaserRuntime.getSortedRenderEntities()):
 *    nhân vật đứng phía bắc NPC có depth nhỏ hơn (vẽ sau lưng), đi xuống phía nam NPC có depth lớn hơn (vẽ trước mặt),
 *    và lớp Foreground (10000) luôn nằm trên mọi thực thể.
 * 4. Chuyển cảnh hai chiều qua SceneTransition & PhaserRuntime:
 *    đi vào exit_harbor_to_warehouse đặt nhân vật tại warehouse_south_door trong warehouse_interior,
 *    kích hoạt portalCooldown & requiresStepOffPortal chống hiện tượng dội cổng (portal ping-pong),
 *    và khi bước ra / hết cooldown đi vào exit_warehouse_to_harbor quay về đúng from_warehouse tại harbor_square.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('phaser', () => ({
  default: {
    CANVAS: 1,
    AUTO: 0,
    Scene: class {},
    Game: class {
      public registry = { set: () => {} };
      public destroy(): void {}
    },
  },
}));

import { InputManager } from '../../src/core/InputManager';
import { CameraController } from '../../src/runtime/CameraController';
import { getAllBuiltInMaps } from '../../src/runtime/MapLoader';
import {
  computeEntityDepth,
  LAYER_DEPTHS,
  PhaserRuntime,
} from '../../src/runtime/PhaserRuntime';
import { SceneTransition } from '../../src/runtime/SceneTransition';

describe('M1 Top-Down Runtime — Movement, Camera, Y-Sort & Portal Transition Suite', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  describe('1. 8-direction movement vector normalization', () => {
    it('maintains unit speed (1.0) across all 4 cardinal and 4 diagonal directions', () => {
      const input = new InputManager();

      const cases: Array<{
        keys: string[];
        expectedX: number;
        expectedY: number;
      }> = [
        { keys: ['w'], expectedX: 0, expectedY: -1 },
        { keys: ['s'], expectedX: 0, expectedY: 1 },
        { keys: ['a'], expectedX: -1, expectedY: 0 },
        { keys: ['d'], expectedX: 1, expectedY: 0 },
        { keys: ['w', 'd'], expectedX: Math.SQRT1_2, expectedY: -Math.SQRT1_2 },
        { keys: ['w', 'a'], expectedX: -Math.SQRT1_2, expectedY: -Math.SQRT1_2 },
        { keys: ['s', 'd'], expectedX: Math.SQRT1_2, expectedY: Math.SQRT1_2 },
        { keys: ['s', 'a'], expectedX: -Math.SQRT1_2, expectedY: Math.SQRT1_2 },
      ];

      for (const c of cases) {
        input.clearKeys();
        for (const k of c.keys) {
          window.dispatchEvent(new KeyboardEvent('keydown', { key: k }));
        }
        const vec = input.getMoveVector(true);
        expect(vec.x).toBeCloseTo(c.expectedX, 5);
        expect(vec.y).toBeCloseTo(c.expectedY, 5);
        expect(Math.hypot(vec.x, vec.y)).toBeCloseTo(1.0, 5);
      }

      // Kiểm tra tích hợp với PhaserRuntime.step(): đi chéo 1 giây dịch chuyển quãng đường bằng đúng đi thẳng 1 giây
      const rtCardinal = new PhaserRuntime({
        initialMapId: 'harbor_square',
        moveSpeed: 96,
      });
      const startPos1 = rtCardinal.getPlayerPosition();
      rtCardinal.step(0.1, { x: 1, y: 0 }, false);
      const endPos1 = rtCardinal.getPlayerPosition();
      const cardinalDist = Math.hypot(endPos1.x - startPos1.x, endPos1.y - startPos1.y);

      const rtDiagonal = new PhaserRuntime({
        initialMapId: 'harbor_square',
        moveSpeed: 96,
      });
      const startPos2 = rtDiagonal.getPlayerPosition();
      rtDiagonal.step(0.1, { x: Math.SQRT1_2, y: Math.SQRT1_2 }, false);
      const endPos2 = rtDiagonal.getPlayerPosition();
      const diagonalDist = Math.hypot(endPos2.x - startPos2.x, endPos2.y - startPos2.y);

      expect(cardinalDist).toBeCloseTo(9.6, 3);
      expect(diagonalDist).toBeCloseTo(cardinalDist, 3);

      input.destroy();
      rtCardinal.destroy();
      rtDiagonal.destroy();
    });
  });

  describe('2. CameraController smooth follow, dead-zone, and strict bounds clamping', () => {
    it('clamps camera strictly within [0, worldWidth - viewportWidth] and [0, worldHeight - viewportHeight] and respects dead-zone', () => {
      const cam = new CameraController({
        worldWidth: 1024,
        worldHeight: 768,
        viewportWidth: 320,
        viewportHeight: 180,
        deadZoneWidth: 24,
        deadZoneHeight: 16,
        lerpSpeed: 10,
      });

      // Kẹp biên góc trên-trái (0, 0) khi nhân vật đứng sát góc (10, 10)
      const topLeft = cam.snapTo(10, 10);
      expect(topLeft).toEqual({ x: 0, y: 0 });

      // Kẹp biên góc dưới-phải (1024 - 320 = 704, 768 - 180 = 588) khi nhân vật ở (1020, 760)
      const bottomRight = cam.snapTo(1020, 760);
      expect(bottomRight).toEqual({ x: 704, y: 588 });

      // Đặt camera giữa bản đồ tại (512, 384) -> top-left là (352, 294)
      cam.snapTo(512, 384);
      expect(cam.getPosition()).toEqual({ x: 352, y: 294 });

      // Nhích nhẹ nhân vật 4px trong vùng dead-zone (24x16 -> bán kính 12x8) không làm camera giật
      const afterMicroMove = cam.update(516, 388, 1 / 60);
      expect(afterMicroMove).toEqual({ x: 352, y: 294 });

      // Khi nhân vật bước ra ngoài dead-zone tới (600, 460), camera bám mượt về phía mục tiêu nhưng không vượt biên
      const afterLargeMove = cam.update(600, 460, 1 / 60);
      expect(afterLargeMove.x).toBeGreaterThan(352);
      expect(afterLargeMove.y).toBeGreaterThan(294);
      expect(afterLargeMove.x).toBeLessThanOrEqual(704);
      expect(afterLargeMove.y).toBeLessThanOrEqual(588);
    });
  });

  describe('3. Y-depth sorting (computeEntityDepth & Foreground occlusion)', () => {
    it('orders player behind NPC when player footY < NPC footY, in front when player footY > NPC footY, and keeps Foreground (10000) above all entities', () => {
      const runtime = new PhaserRuntime({ initialMapId: 'harbor_square' });
      const courier = runtime.getCurrentMap().npcs.find((n) => n.id === 'npc_courier')!;
      expect(courier).toBeDefined();
      const courierFootY = courier.y + 16; // 368 + 16 = 384
      const courierDepth = computeEntityDepth(courierFootY);

      // Đặt nhân vật ở phía bắc (phía trên màn hình) so với Giao Liên: playerPos.y = 350 -> footY = 358 < 384
      runtime.playerPos = { x: courier.x, y: 350 };
      const playerNorthDepth = runtime.getPlayerDepth();
      expect(playerNorthDepth).toBeLessThan(courierDepth);

      const sortedNorth = runtime.getSortedRenderEntities();
      const playerNorthIdx = sortedNorth.findIndex((e) => e.id === 'player');
      const courierNorthIdx = sortedNorth.findIndex((e) => e.id === 'npc_courier');
      expect(playerNorthIdx).toBeLessThan(courierNorthIdx);

      // Đặt nhân vật đi xuống phía nam (phía dưới màn hình) so với Giao Liên: playerPos.y = 392 -> footY = 400 > 384
      runtime.playerPos = { x: courier.x, y: 392 };
      const playerSouthDepth = runtime.getPlayerDepth();
      expect(playerSouthDepth).toBeGreaterThan(courierDepth);

      const sortedSouth = runtime.getSortedRenderEntities();
      const playerSouthIdx = sortedSouth.findIndex((e) => e.id === 'player');
      const courierSouthIdx = sortedSouth.findIndex((e) => e.id === 'npc_courier');
      expect(playerSouthIdx).toBeGreaterThan(courierSouthIdx);

      // Lớp Foreground (mái hiên, tán cây = 10000) luôn cao hơn mọi thực thể trên bản đồ
      expect(LAYER_DEPTHS.Foreground).toBe(10000);
      expect(LAYER_DEPTHS.Foreground).toBeGreaterThan(playerSouthDepth);
      expect(LAYER_DEPTHS.Foreground).toBeGreaterThan(courierDepth);

      runtime.destroy();
    });
  });

  describe('4. SceneTransition two-way portal round-trip & anti-ping-pong guard', () => {
    it('transitions from harbor_square (exit_harbor_to_warehouse) to warehouse_interior (warehouse_south_door), blocks immediate ping-pong, and returns to from_warehouse', () => {
      const maps = getAllBuiltInMaps();
      const transition = new SceneTransition('harbor_square', 'default_spawn', 0.65);

      const harborPortals = maps.harbor_square.portals;
      const warehousePortals = maps.warehouse_interior.portals;

      // 1. Nhân vật bước vào cổng exit_harbor_to_warehouse tại (512, 400, 32x16)
      const enterFootRect = { x: 516, y: 402, width: 12, height: 8 };
      const enterRes = transition.tryPortalTransition(
        enterFootRect,
        harborPortals,
        maps
      );

      expect(enterRes.transitioned).toBe(true);
      expect(enterRes.reason).toBe('ok');
      expect(enterRes.targetMapId).toBe('warehouse_interior');
      expect(enterRes.targetSpawnId).toBe('warehouse_south_door');
      expect(enterRes.spawnPos).toEqual({ x: 272, y: 384 });
      expect(transition.getCurrentMapId()).toBe('warehouse_interior');
      expect(transition.getCooldownRemaining()).toBeGreaterThan(0);
      expect(transition.requiresStepOffPortal).toBe(true);

      // 2. Ngay ở frame tiếp theo, nếu hộp va chạm chạm vùng cổng exit_warehouse_to_harbor (272, 416)
      // thì requiresStepOffPortal và cooldown PHẢI chặn không cho dội ngược về harbor_square (anti ping-pong)
      const immediateOverlapFoot = { x: 276, y: 418, width: 12, height: 8 };
      const blockedStepOff = transition.tryPortalTransition(
        immediateOverlapFoot,
        warehousePortals,
        maps
      );
      expect(blockedStepOff.transitioned).toBe(false);
      expect(blockedStepOff.reason).toBe('step_off_required');
      expect(transition.getCurrentMapId()).toBe('warehouse_interior');

      // 3. Nhân vật đứng tại điểm spawn (272, 384) ngoài cổng và chờ hết thời gian cooldown (0.7s)
      const safeSpawnFoot = { x: 272, y: 384, width: 12, height: 8 };
      transition.update(0.7, safeSpawnFoot, warehousePortals);
      expect(transition.requiresStepOffPortal).toBe(false);
      expect(transition.getCooldownRemaining()).toBe(0);

      // 4. Nhân vật chủ động đi xuống cửa nam exit_warehouse_to_harbor tại (272, 416) -> quay về đúng from_warehouse (512, 432) ở harbor_square
      const exitRes = transition.tryPortalTransition(
        immediateOverlapFoot,
        warehousePortals,
        maps
      );
      expect(exitRes.transitioned).toBe(true);
      expect(exitRes.reason).toBe('ok');
      expect(exitRes.targetMapId).toBe('harbor_square');
      expect(exitRes.targetSpawnId).toBe('from_warehouse');
      expect(exitRes.spawnPos).toEqual({ x: 512, y: 432 });
      expect(transition.getCurrentMapId()).toBe('harbor_square');
    });
  });
});
