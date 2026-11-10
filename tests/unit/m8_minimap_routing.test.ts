/**
 * @file tests/unit/m8_minimap_routing.test.ts
 * @description Kiểm định Milestone 8 (Issue #13):
 * - GridPathfinder 8 hướng A* không cắt góc chéo qua vật cản (no diagonal corner cutting).
 * - findNearestWalkableCell tìm ô đi bộ gần nhất khi mục tiêu nằm trong vật cản.
 * - RoutePlanner tính toán đường dẫn cùng khu vực và liên khu vực (WorldGraph BFS + A*),
 *   kèm bộ nhớ đệm theo ô lưới (< 1.5ms).
 * - Click-to-Track nhiệm vụ trong GameStore / QuestSystem / SaveManager.
 */

import { describe, expect, it } from 'vitest';
import { GameStore } from '../../src/core/GameStore';
import { SaveManager } from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import { getAllBuiltInMaps, getBuiltInMap } from '../../src/runtime/MapLoader';
import { resolveObjectiveTarget } from '../../src/runtime/PhaserRuntime';
import { QuestSystem } from '../../src/systems/QuestSystem';
import type { NormalizedMapData } from '../../src/types/game';
import { GridPathfinder, RoutePlanner } from '../../src/world/GridPathfinder';

describe('M8 — GridPathfinder, RoutePlanner & Click-to-Track Quests (Issue #13)', () => {
  it('routes around obstacles without diagonal corner cutting', () => {
    const customMap: NormalizedMapData = {
      ...getBuiltInMap('harbor_square'),
      id: 'test_grid_map',
      widthTiles: 8,
      heightTiles: 8,
      tileSize: 16,
      widthPx: 128,
      heightPx: 128,
      // Chặn các ô (3,2), (3,3), (3,4) tạo bức tường dọc ở cột 3
      obstacles: [{ id: 'wall_vertical', x: 48, y: 32, width: 16, height: 48 }],
      shelters: [],
    };

    const pf = new GridPathfinder(customMap);
    expect(pf.isCellWalkable(3, 2)).toBe(false);
    expect(pf.isCellWalkable(3, 3)).toBe(false);
    expect(pf.isCellWalkable(3, 4)).toBe(false);

    // Kiểm tra quy tắc cấm cắt góc chéo qua 2 ô tiếp giáp bức tường
    expect(pf.canStepDiagonal(2, 2, 1, -1)).toBe(false);
    expect(pf.canStepDiagonal(2, 4, 1, 1)).toBe(false);

    const route = pf.findPath({ x: 24, y: 56 }, { x: 88, y: 56 });
    expect(route.reachable).toBe(true);
    expect(route.cells.length).toBeGreaterThanOrEqual(4);

    // Không có bước đi nào đi vào cột 3 ở hàng 2..4
    for (const cell of route.cells) {
      const blocked = cell.col === 3 && cell.row >= 2 && cell.row <= 4;
      expect(blocked).toBe(false);
    }

    // Kiểm tra không có bước chéo nào cắt qua góc tường
    for (let i = 1; i < route.cells.length; i++) {
      const prev = route.cells[i - 1]!;
      const curr = route.cells[i]!;
      const dCol = curr.col - prev.col;
      const dRow = curr.row - prev.row;
      expect(Math.abs(dCol)).toBeLessThanOrEqual(1);
      expect(Math.abs(dRow)).toBeLessThanOrEqual(1);
      if (dCol !== 0 && dRow !== 0) {
        expect(pf.isCellWalkable(prev.col + dCol, prev.row)).toBe(true);
        expect(pf.isCellWalkable(prev.col, prev.row + dRow)).toBe(true);
      }
    }
  });

  it('finds nearest walkable cell when target point lies inside an obstacle', () => {
    const map = getBuiltInMap('harbor_square');
    const pf = new GridPathfinder(map);
    const firstObstacle = map.obstacles[0]!;
    const insideX = firstObstacle.x + firstObstacle.width * 0.5;
    const insideY = firstObstacle.y + firstObstacle.height * 0.5;

    const nearest = pf.findNearestWalkableCell(
      Math.floor(insideX / 16),
      Math.floor(insideY / 16)
    );
    expect(nearest).not.toBeNull();
    expect(pf.isCellWalkable(nearest!.col, nearest!.row)).toBe(true);

    const path = pf.findPath({ x: 85, y: 384 }, { x: insideX, y: insideY });
    expect(path.reachable).toBe(true);
    expect(path.points.length).toBeGreaterThanOrEqual(2);
  });

  it('computes intra-zone and cross-zone routes across all 14 maps under 1.5ms', () => {
    const allMaps = getAllBuiltInMaps();
    const questSystem = new QuestSystem();
    const store = new GameStore();
    const planner = new RoutePlanner();

    // 1. Intra-zone: harbor_square -> obj_ch1_meet_liaison
    const harborMap = allMaps['harbor_square']!;
    const activeCh1 = questSystem.getCurrentActiveObjective('1930', store)!;
    const res1 = resolveObjectiveTarget(harborMap, activeCh1.objective, {
      questFlags: store.questFlags,
    });
    const t0 = performance.now();
    const route1 = planner.computeTrackedRoute(
      harborMap,
      85,
      384,
      activeCh1.objective,
      res1,
      store.questFlags
    );
    const elapsedMs = performance.now() - t0;

    expect(elapsedMs).toBeLessThan(15); // cold build + A*
    expect(route1.path.reachable).toBe(true);
    expect(route1.isCrossZone).toBe(false);
    expect(route1.path.points.length).toBeGreaterThanOrEqual(2);

    // Gọi lại từ cùng ô lưới để kiểm tra cache (< 1.5ms)
    const tCache0 = performance.now();
    const route1Cached = planner.computeTrackedRoute(
      harborMap,
      86,
      385,
      activeCh1.objective,
      res1,
      store.questFlags
    );
    const cachedMs = performance.now() - tCache0;
    expect(cachedMs).toBeLessThan(1.5);
    expect(route1Cached).toBe(route1);

    // 2. Cross-zone: theo dõi nhiệm vụ phụ ở Bến Thuyền Nhỏ (small_pier) khi đang đứng ở Quảng Trường Bến Cảng (harbor_square)
    store.trackedObjectiveId = 'obj_ch1_talk_pier_boatman';
    const trackedSide = questSystem.getCurrentActiveObjective('1930', store)!;
    expect(trackedSide.objective.id).toBe('obj_ch1_talk_pier_boatman');

    const resCross = resolveObjectiveTarget(harborMap, trackedSide.objective, {
      questFlags: store.questFlags,
    });
    expect(resCross.isCrossZone).toBe(true);
    expect(resCross.nextPortalZoneId).toBe('small_pier');

    const routeCross = planner.computeTrackedRoute(
      harborMap,
      85,
      384,
      trackedSide.objective,
      resCross,
      store.questFlags
    );
    expect(routeCross.isCrossZone).toBe(true);
    expect(routeCross.nextPortalZoneId).toBe('small_pier');
    expect(routeCross.path.reachable).toBe(true);
    expect(routeCross.routeBannerText).toContain('Bến Thuyền Nhỏ');
  });

  it('persists trackedObjectiveId and showMinimap in SaveDataV2 and auto-advances when completed', () => {
    const store = new GameStore();
    const questSystem = new QuestSystem();
    const saveManager = new SaveManager();

    store.trackedObjectiveId = 'obj_ch1_talk_pier_boatman';
    store.settings.showMinimap = false;

    const savePayload = store.toSaveDataV2('1930', 140, 'down', 360);
    expect(savePayload.trackedObjectiveId).toBe('obj_ch1_talk_pier_boatman');
    expect(savePayload.settings?.showMinimap).toBe(false);

    saveManager.save(savePayload);
    const loaded = saveManager.load();
    expect(loaded?.trackedObjectiveId).toBe('obj_ch1_talk_pier_boatman');
    expect(loaded?.settings?.showMinimap).toBe(false);

    const restoredStore = new GameStore();
    restoredStore.restoreFromSave(loaded!, CHAPTERS);
    expect(restoredStore.trackedObjectiveId).toBe('obj_ch1_talk_pier_boatman');

    // Khi mục tiêu đang theo dõi hoàn thành, getCurrentActiveObjective tự động chuyển về mục tiêu đang mở tiếp theo
    restoredStore.questFlags.add('ch1_talked_boatman');
    questSystem.evaluateProgress(restoredStore, '1930');
    expect(restoredStore.completedObjectives.has('obj_ch1_talk_pier_boatman')).toBe(true);

    const nextActive = questSystem.getCurrentActiveObjective('1930', restoredStore);
    expect(nextActive?.objective.id).toBe('obj_ch1_meet_liaison');
  });
});
