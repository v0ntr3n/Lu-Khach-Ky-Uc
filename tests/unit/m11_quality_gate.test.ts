import { describe, expect, it } from 'vitest';
import { CHAPTERS } from '../../src/data/chronicles.ts';
import { PixelRenderer } from '../../src/engine/PixelRenderer.ts';
import { getAllBuiltInMaps } from '../../src/runtime/MapLoader.ts';
import { resolveObjectiveTarget } from '../../src/runtime/PhaserRuntime.ts';
import { QUEST_CATALOG } from '../../src/systems/QuestSystem.ts';
import { GridPathfinder, RoutePlanner } from '../../src/world/GridPathfinder.ts';
import { runMapValidationCli } from '../../tools/validate_maps.ts';

function createMockCanvas(): HTMLCanvasElement {
  const ctx = {
    fillStyle: '#000000',
    strokeStyle: '#000000',
    lineWidth: 1,
    globalAlpha: 1,
    imageSmoothingEnabled: false,
    fillRect: () => {},
    strokeRect: () => {},
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    arc: () => {},
    fill: () => {},
    stroke: () => {},
    save: () => {},
    restore: () => {},
    translate: () => {},
    scale: () => {},
    clearRect: () => {},
    createRadialGradient: () => ({
      addColorStop: () => {}
    })
  } as unknown as CanvasRenderingContext2D;

  return {
    width: 480,
    height: 270,
    getContext: () => ctx
  } as unknown as HTMLCanvasElement;
}

describe('M11: Gameplay 2.0 Quality Gate — Navigation, Accessibility, Performance & 14-Map QA (#16)', () => {
  it('validates all 14 Tiled maps, WorldGraph routes, QuestTargetRef links, and A* paths with 0 errors', async () => {
    const report = await runMapValidationCli(true);
    expect(report.zoneCount).toBe(14);
    expect(report.errors).toEqual([]);
    expect(report.valid).toBe(true);
  });

  it('maintains A* pathfinding and RoutePlanner average latency < 1.5ms across all 14 maps and 15 objectives', () => {
    const allMaps = getAllBuiltInMaps();
    const planner = new RoutePlanner();
    const flags = new Set<string>(['ch1_met_liaison', 'ch1_dock_permit', 'ch1_pier_schedule']);

    let totalMs = 0;
    let count = 0;

    for (const quest of QUEST_CATALOG) {
      for (const obj of quest.objectives) {
        const map = allMaps[obj.targetRef?.zoneId ?? obj.zoneId];
        expect(map).toBeDefined();
        const spawn = map.spawns.find((s) => s.id === map.defaultSpawnId) ?? map.spawns[0];
        const resolution = resolveObjectiveTarget(map, obj, { questFlags: flags });

        const start = performance.now();
        const route = planner.computeTrackedRoute(
          map,
          spawn.x,
          spawn.y,
          obj,
          resolution,
          flags
        );
        const elapsed = performance.now() - start;

        totalMs += elapsed;
        count++;

        expect(route.path.reachable, `Objective ${obj.id} should be reachable`).toBe(true);
        expect(route.path.points.length).toBeGreaterThanOrEqual(2);
      }
    }

    const avgMs = totalMs / Math.max(1, count);
    expect(avgMs).toBeLessThan(1.5);
  });

  it('renders all 14 zones across Chapters I–IV with crisp pixel art and full accessibility settings', () => {
    const allMaps = getAllBuiltInMaps();
    const canvas = createMockCanvas();
    const renderer = new PixelRenderer(canvas);

    for (const map of Object.values(allMaps)) {
      const chapter = CHAPTERS.find((c) => c.id === map.eraId) ?? CHAPTERS[0];
      const spawn = map.spawns.find((s) => s.id === map.defaultSpawnId) ?? map.spawns[0];
      const pf = new GridPathfinder(map);
      const firstNpc = map.npcs[0];
      const firstInter = map.interactables[0];
      const targetPos = firstNpc
        ? { x: firstNpc.x, y: firstNpc.y }
        : firstInter
          ? { x: firstInter.x, y: firstInter.y }
          : { x: spawn.x + 32, y: spawn.y };
      const path = pf.findPath({ x: spawn.x, y: spawn.y }, targetPos);

      expect(() =>
        renderer.render({
          chapter,
          playerX: spawn.x,
          playerY: spawn.y,
          cameraX: Math.max(0, spawn.x - 160),
          cameraY: Math.max(0, spawn.y - 90),
          facingRight: true,
          playerFacing: 'down',
          isMoving: true,
          isSheltered: false,
          collectedMilestones: new Set(),
          nearbyMilestone: null,
          nearClimaxGate: false,
          climaxBurstProgress: 0,
          timeSec: 1.2,
          mapData: map,
          trackedTargetPos: targetPos,
          trackedTargetId: firstNpc?.npcId ?? firstInter?.id ?? null,
          activeQuestTargetIds: new Set(firstNpc ? [firstNpc.npcId] : []),
          optionalTargetIds: new Set(firstInter ? [firstInter.id] : []),
          routePoints: path.points,
          showHints: true,
          reducedMotion: false,
          effectsQuality: 'full'
        })
      ).not.toThrow();

      expect(() =>
        renderer.render({
          chapter,
          playerX: spawn.x,
          playerY: spawn.y,
          cameraX: Math.max(0, spawn.x - 160),
          cameraY: Math.max(0, spawn.y - 90),
          facingRight: false,
          playerFacing: 'left',
          isMoving: false,
          isSheltered: true,
          collectedMilestones: new Set(['I', 'IV', 'VII', 'X']),
          nearbyMilestone: null,
          nearClimaxGate: false,
          climaxBurstProgress: 0,
          timeSec: 3.4,
          mapData: map,
          trackedTargetPos: targetPos,
          trackedTargetId: null,
          routePoints: path.points,
          showHints: false,
          reducedMotion: true,
          effectsQuality: 'subtle'
        })
      ).not.toThrow();
    }
  });
});
