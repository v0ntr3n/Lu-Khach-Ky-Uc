/**
 * @file tests/maps/m2_world_graph.test.ts
 * @description Bộ kiểm thử đồ thị thế giới 8 khu vực Chương I (Hương Cảng 1930) & lộ trình NPC cho Cột mốc M2:
 * 1. Kiểm định đầy đủ 8 khu vực trong CHAPTER1_WORLD_GRAPH và getAllBuiltInMaps() đều đạt validateMapData & validateWorldGraph.
 * 2. Kiểm tra đa tuyến đường (Main Route qua old_street & Optional Route qua small_pier) từ harbor_square tới secret_archive.
 * 3. Kiểm tra bảo mật Dịch Chuyển Nhanh (Fast Travel): chặn khu vực chưa khám phá hoặc chưa mở Checkpoint, bảo toàn tiến trình SaveDataV2.
 * 4. Kiểm tra thuật toán stepNpcAlongRoute di chuyển NPC qua các mốc NpcRoutePoint[], cập nhật hướng nhìn 4 chiều và chờ waitSec không sinh NaN.
 */

import { describe, expect, it, vi } from 'vitest';

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

import { GameStore } from '../../src/core/GameStore';
import { CHAPTERS } from '../../src/data/chronicles';
import { getAllBuiltInMaps, toValidatableMap } from '../../src/runtime/MapLoader';
import { stepNpcAlongRoute } from '../../src/runtime/PhaserRuntime';
import type { NpcMapPlacement } from '../../src/types/game';
import { CollisionWorld } from '../../src/world/CollisionWorld';
import { type ValidatableMap, validateMapData } from '../../src/world/MapValidator';
import { CHAPTER1_WORLD_GRAPH } from '../../src/world/WorldGraph';

const EXPECTED_ZONE_IDS = [
  'harbor_square',
  'warehouse_interior',
  'old_street',
  'small_pier',
  'safehouse_interior',
  'shadow_alley',
  'archive_courtyard',
  'secret_archive',
] as const;

describe('M2 World Exploration — 8-Zone WorldGraph, Multi-Route, Fast Travel & NPC Route Suite', () => {
  it('1. validates 8-zone Chapter I WorldGraph completeness and passes validateMapData & validateWorldGraph for all 8 maps', () => {
    const zones = CHAPTER1_WORLD_GRAPH.getAllZones();
    expect(zones).toHaveLength(8);
    expect(zones.map((z) => z.id)).toEqual(expect.arrayContaining([...EXPECTED_ZONE_IDS]));

    const allBuiltInMaps = getAllBuiltInMaps();
    expect(Object.keys(allBuiltInMaps)).toEqual(
      expect.arrayContaining([...EXPECTED_ZONE_IDS])
    );

    const validatableMaps: Record<string, ValidatableMap> = {};
    for (const [id, normMap] of Object.entries(allBuiltInMaps)) {
      validatableMaps[id] = toValidatableMap(normMap);
    }

    for (const zoneId of EXPECTED_ZONE_IDS) {
      const vMap = validatableMaps[zoneId];
      expect(vMap).toBeDefined();

      const res = validateMapData(vMap, validatableMaps);
      expect(res.errors).toEqual([]);
      expect(res.valid).toBe(true);

      // Đảm bảo không có điểm spawn nào chồng lấn lên vùng cổng (portal) gây dội cổng tức thì
      for (const spawn of vMap.spawns) {
        const spawnFoot = { x: spawn.x, y: spawn.y, width: 8, height: 8 };
        for (const portal of vMap.portals) {
          const portalRect = {
            x: portal.x,
            y: portal.y,
            width: portal.width,
            height: portal.height,
          };
          expect(CollisionWorld.rectsOverlap(spawnFoot, portalRect)).toBe(false);
        }
      }
    }

    const graphValidation = CHAPTER1_WORLD_GRAPH.validateWorldGraph(allBuiltInMaps);
    expect(graphValidation.errors).toEqual([]);
    expect(graphValidation.valid).toBe(true);
  });

  it('2. verifies multiple route reachability: main route via old_street and optional shortcut via small_pier', () => {
    // Tuyến chính (includeOptional = false): harbor_square -> old_street -> shadow_alley -> archive_courtyard -> secret_archive
    const mainRoute = CHAPTER1_WORLD_GRAPH.findShortestRoute(
      'harbor_square',
      'secret_archive',
      new Set(),
      false
    );
    expect(mainRoute).toEqual([
      'harbor_square',
      'old_street',
      'shadow_alley',
      'archive_courtyard',
      'secret_archive',
    ]);

    // Tuyến tính cả nhánh phụ (includeOptional = true)
    const anyRoute = CHAPTER1_WORLD_GRAPH.findShortestRoute(
      'harbor_square',
      'secret_archive',
      new Set(),
      true
    );
    expect(anyRoute).not.toBeNull();
    expect(anyRoute).toHaveLength(5);
    expect(anyRoute?.[0]).toBe('harbor_square');
    expect(anyRoute?.[4]).toBe('secret_archive');

    // Kiểm tra nhánh phụ từ small_pier tới secret_archive: small_pier -> shadow_alley -> archive_courtyard -> secret_archive
    const pierRoute = CHAPTER1_WORLD_GRAPH.findShortestRoute(
      'small_pier',
      'secret_archive',
      new Set(),
      true
    );
    expect(pierRoute).toEqual([
      'small_pier',
      'shadow_alley',
      'archive_courtyard',
      'secret_archive',
    ]);
  });

  it('3. enforces Fast Travel gating (requires both discoveredMaps and unlockedCheckpoints) and persists in SaveDataV2', () => {
    const discovered = new Set<string>(['harbor_square']);
    const checkpoints = new Set<string>(['cp_harbor_lantern']);

    // Chặn dịch chuyển nhanh tới khu vực chưa khám phá (old_street, secret_archive)
    expect(
      CHAPTER1_WORLD_GRAPH.canFastTravel(
        'harbor_square',
        'old_street',
        discovered,
        checkpoints,
        false
      ).allowed
    ).toBe(false);

    expect(
      CHAPTER1_WORLD_GRAPH.canFastTravel(
        'harbor_square',
        'secret_archive',
        discovered,
        checkpoints,
        false
      ).allowed
    ).toBe(false);

    // Đã khám phá old_street nhưng chưa mở khóa checkpoint -> vẫn chặn
    discovered.add('old_street');
    expect(
      CHAPTER1_WORLD_GRAPH.canFastTravel(
        'harbor_square',
        'old_street',
        discovered,
        checkpoints,
        false
      ).allowed
    ).toBe(false);

    // Sau khi mở khóa checkpoint cp_old_street_tea -> cho phép dịch chuyển nhanh
    checkpoints.add('cp_old_street_tea');
    const allowedRes = CHAPTER1_WORLD_GRAPH.canFastTravel(
      'harbor_square',
      'old_street',
      discovered,
      checkpoints,
      false
    );
    expect(allowedRes.allowed).toBe(true);
    expect(allowedRes.destinationSpawn).toBe('from_harbor');

    // Kiểm tra tích hợp với GameStore & lưu trữ SaveDataV2
    const store = new GameStore();
    expect(store.canFastTravelToZone('old_street', 'cp_old_street_tea', 'from_harbor')).toBe(
      false
    );

    store.unlockCheckpoint('cp_old_street_tea', 'old_street');
    expect(store.canFastTravelToZone('old_street', 'cp_old_street_tea', 'from_harbor')).toBe(
      true
    );

    const saved = store.toSaveDataV2('1930', 200, 'right', 384);
    expect(saved.discoveredMaps).toContain('old_street');
    expect(saved.unlockedCheckpoints).toContain('cp_old_street_tea');

    const restoredStore = new GameStore();
    restoredStore.restoreFromSave(saved, CHAPTERS);
    expect(
      restoredStore.canFastTravelToZone('old_street', 'cp_old_street_tea', 'from_harbor')
    ).toBe(true);
    expect(restoredStore.collectedMilestones.size).toBe(0);
  });

  it('4. steps NPCs along NpcRoutePoint[] waypoints, updates facing, and honors waitSec without producing NaN', () => {
    const npc: NpcMapPlacement = {
      id: 'npc_courier',
      npcId: 'npc_courier',
      name: 'Lâm Văn Bính',
      role: 'Giao liên bí mật',
      x: 100,
      y: 200,
      facing: 'down',
      dialogueId: 'ch1_courier',
      fictionTag: 'fictional_npc',
      approachRadius: 36,
      moveSpeed: 50,
      route: [
        { x: 100, y: 200, waitSec: 1.0, facing: 'down' },
        { x: 150, y: 200, waitSec: 1.5, facing: 'right' },
        { x: 150, y: 250, waitSec: 1.0, facing: 'down' },
      ],
    };

    const routeState = { waypointIndex: 0, waitRemainingSec: 0 };

    // Bước 1: NPC đang ở sẵn điểm 0 (100, 200), sẽ tự động hướng sang điểm 1 (150, 200) theo chiều 'right'
    const step1 = stepNpcAlongRoute(npc, routeState, 0.5);
    expect(Number.isFinite(step1.x)).toBe(true);
    expect(Number.isFinite(step1.y)).toBe(true);
    expect(step1.isMoving).toBe(true);
    expect(step1.facing).toBe('right');
    expect(step1.x).toBeCloseTo(125, 3);
    expect(step1.y).toBeCloseTo(200, 3);

    // Bước 2: Đi thêm 0.6s nữa sẽ chạm tới điểm 1 (150, 200) và bắt đầu chờ waitSec = 1.5s
    const step2 = stepNpcAlongRoute(npc, routeState, 0.6);
    expect(step2.x).toBe(150);
    expect(step2.y).toBe(200);
    expect(step2.isMoving).toBe(false);
    expect(routeState.waitRemainingSec).toBeCloseTo(1.5, 3);

    // Bước 3: Trong thời gian chờ (dt = 0.5s < 1.5s), NPC đứng yên tại (150, 200)
    const step3 = stepNpcAlongRoute(npc, routeState, 0.5);
    expect(step3.x).toBe(150);
    expect(step3.y).toBe(200);
    expect(step3.isMoving).toBe(false);
    expect(routeState.waitRemainingSec).toBeCloseTo(1.0, 3);

    // Bước 4: Sau khi hết 1.0s chờ + đi thêm 0.4s (tổng dt = 1.4s), NPC đi xuống điểm 2 (150, 250) với hướng 'down'
    const step4 = stepNpcAlongRoute(npc, routeState, 1.4);
    expect(step4.isMoving).toBe(true);
    expect(step4.facing).toBe('down');
    expect(step4.x).toBe(150);
    expect(step4.y).toBeCloseTo(220, 3);
  });
});
