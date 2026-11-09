/**
 * @file src/scenes/ZoneExplorationScene.ts
 * @description Scene khám phá đa khu vực (Multi-Zone Top-Down Scene) cho Lữ Khách Ký Ức (M3+).
 * Có thể nạp bất kỳ khu vực nào trong 8 bản đồ Chương I (`getAllBuiltInMaps()`),
 * khởi tạo `CollisionWorld`, `CameraController`, `SceneTransition`, cập nhật tuần tra NPC
 * (`stepNpcAlongRoute`), hiển thị chỉ dẫn mục tiêu nhiệm vụ (`getObjectiveTargetPosition`),
 * chuyển đổi khu vực (`switchZone`) và xuất bản chụp trạng thái (`getSnapshot`).
 */

import type {
  Facing4,
  NormalizedMapData,
  NpcMapPlacement,
  Vec2,
} from '../types/game';
import { CameraController } from '../runtime/CameraController';
import { getAllBuiltInMaps, getBuiltInMap } from '../runtime/MapLoader';
import {
  BaseTopDownScene,
  computeEntityDepth,
  getObjectiveTargetPosition,
  LAYER_DEPTHS,
  type NpcRouteState,
  type ObjectiveTargetPosition,
  PLAYER_FOOT_HEIGHT,
  PLAYER_FOOT_WIDTH,
  type SceneDisplayRect,
  stepNpcAlongRoute,
} from '../runtime/PhaserRuntime';
import { SceneTransition } from '../runtime/SceneTransition';
import { CollisionWorld } from '../world/CollisionWorld';

export { computeEntityDepth, getObjectiveTargetPosition, LAYER_DEPTHS };

export interface ZoneExplorationSceneInitData {
  zoneId?: string;
  spawnId?: string;
  activeObjectiveZoneId?: string | null;
  transition?: SceneTransition;
}

export interface ZoneExplorationSnapshot {
  zoneId: string;
  title: string;
  districtType?: NormalizedMapData['districtType'];
  playerPos: Vec2;
  playerFacing: Facing4;
  playerDepth: number;
  cameraPos: Vec2;
  npcs: Array<{
    id: string;
    x: number;
    y: number;
    facing: Facing4;
    depth: number;
  }>;
  objectiveTarget: ObjectiveTargetPosition | null;
  portalCount: number;
  interactableCount: number;
}

export class ZoneExplorationScene extends BaseTopDownScene {
  public static readonly KEY = 'ZoneExplorationScene';

  public zoneId: string;
  public spawnId: string;
  public mapData: NormalizedMapData;
  public collisionWorld: CollisionWorld;
  public cameraController: CameraController;
  public transition: SceneTransition;

  public playerPos: Vec2 = { x: 160, y: 384 };
  public playerFacing: Facing4 = 'right';
  public playerDepth: number = computeEntityDepth(384 + PLAYER_FOOT_HEIGHT);
  public activeObjectiveZoneId: string | null = null;

  public readonly npcPlacements: NpcMapPlacement[] = [];
  public readonly npcRouteStates = new Map<string, NpcRouteState>();

  public playerGraphic?: SceneDisplayRect;
  public objectiveMarkerGraphic?: SceneDisplayRect;
  public readonly npcGraphics = new Map<string, SceneDisplayRect>();

  constructor(
    zoneIdOrConfig: string | ZoneExplorationSceneInitData = 'harbor_square',
    initialSpawnId?: string
  ) {
    super({ key: ZoneExplorationScene.KEY });

    const initZoneId =
      typeof zoneIdOrConfig === 'string'
        ? zoneIdOrConfig
        : zoneIdOrConfig.zoneId ?? 'harbor_square';
    const initSpawnId =
      typeof zoneIdOrConfig === 'string'
        ? initialSpawnId
        : zoneIdOrConfig.spawnId;

    const allMaps = getAllBuiltInMaps();
    this.mapData = allMaps[initZoneId] ?? getBuiltInMap('harbor_square');
    this.zoneId = this.mapData.id;
    this.spawnId = initSpawnId ?? this.mapData.defaultSpawnId;

    if (typeof zoneIdOrConfig === 'object' && zoneIdOrConfig !== null) {
      this.activeObjectiveZoneId =
        zoneIdOrConfig.activeObjectiveZoneId ?? null;
    }

    this.collisionWorld = new CollisionWorld(
      this.mapData.obstacles,
      this.mapData.shelters,
      { x: 0, y: 0, width: this.mapData.widthPx, height: this.mapData.heightPx }
    );
    this.cameraController = new CameraController({
      worldWidth: this.mapData.widthPx,
      worldHeight: this.mapData.heightPx,
      viewportWidth: 320,
      viewportHeight: 180,
      deadZoneWidth: 24,
      deadZoneHeight: 16,
    });
    this.transition = new SceneTransition(this.zoneId, this.spawnId, 0.65);

    this.applyMapAndSpawn(this.mapData, this.spawnId);
  }

  private applyMapAndSpawn(map: NormalizedMapData, spawnId?: string): void {
    this.mapData = map;
    this.zoneId = map.id;

    const resolvedSpawn =
      (spawnId ? map.spawns.find((s) => s.id === spawnId) : undefined) ??
      map.spawns.find((s) => s.id === map.defaultSpawnId) ??
      map.spawns[0] ?? { id: 'default_spawn', x: 160, y: 384, facing: 'down' };

    this.spawnId = resolvedSpawn.id;
    this.playerPos = { x: resolvedSpawn.x, y: resolvedSpawn.y };
    this.playerFacing = resolvedSpawn.facing ?? 'down';
    this.playerDepth = computeEntityDepth(this.playerPos.y + PLAYER_FOOT_HEIGHT);

    this.collisionWorld = new CollisionWorld(map.obstacles, map.shelters, {
      x: 0,
      y: 0,
      width: map.widthPx,
      height: map.heightPx,
    });

    this.cameraController.setBounds(map.widthPx, map.heightPx);
    this.cameraController.snapTo(this.playerPos.x, this.playerPos.y);

    this.npcPlacements.length = 0;
    this.npcRouteStates.clear();
    for (const n of map.npcs) {
      this.npcPlacements.push({
        ...n,
        route: n.route ? n.route.map((pt) => ({ ...pt })) : undefined,
      });
      this.npcRouteStates.set(n.id, {
        waypointIndex: 0,
        waitRemainingSec: 0,
      });
    }
  }

  public override init(data?: ZoneExplorationSceneInitData): void {
    if (!data) return;
    if (data.transition) {
      this.transition = data.transition;
    }
    if (data.activeObjectiveZoneId !== undefined) {
      this.activeObjectiveZoneId = data.activeObjectiveZoneId;
    }
    const targetZoneId = data.zoneId ?? this.zoneId;
    const allMaps = getAllBuiltInMaps();
    const targetMap = allMaps[targetZoneId] ?? this.mapData;
    this.applyMapAndSpawn(targetMap, data.spawnId);
  }

  public override create(): void {
    this.displayList.length = 0;
    this.cameras.main.setBounds(
      0,
      0,
      this.mapData.widthPx,
      this.mapData.heightPx
    );

    // 1. Lớp nền (Ground)
    const groundBg = this.add.rectangle(
      0,
      0,
      this.mapData.widthPx,
      this.mapData.heightPx,
      0x1c1510
    );
    groundBg.setOrigin(0, 0);
    groundBg.setDepth(LAYER_DEPTHS.Ground);

    // 2. Lớp mái hiên ẩn nấp (LowProps / Shelters)
    for (const shelter of this.mapData.shelters) {
      if (!shelter.bounds) continue;
      const sRect = this.add.rectangle(
        shelter.bounds.x,
        shelter.bounds.y,
        shelter.bounds.width,
        shelter.bounds.height,
        0x2e241b,
        0.55
      );
      sRect.setOrigin(0, 0);
      sRect.setDepth(LAYER_DEPTHS.LowProps);
    }

    // 3. Lớp vật cản kiến trúc (Structures)
    for (const obs of this.mapData.obstacles) {
      const rect = this.add.rectangle(
        obs.x,
        obs.y,
        obs.width,
        obs.height,
        0x38261b
      );
      rect.setOrigin(0, 0);
      rect.setDepth(LAYER_DEPTHS.Structures);
    }

    // 4. Lớp cổng chuyển cảnh (Portals)
    for (const portal of this.mapData.portals) {
      const pRect = this.add.rectangle(
        portal.x,
        portal.y,
        portal.width,
        portal.height,
        0xd4a017,
        0.35
      );
      pRect.setOrigin(0, 0);
      pRect.setDepth(LAYER_DEPTHS.LowProps + 5);
    }

    // 5. Thực thể tương tác (Evidence, Checkpoints, Climax Gate)
    for (const item of this.mapData.interactables) {
      if (item.kind === 'npc') continue;
      const footY = item.y + (item.height ?? 16);
      const color =
        item.kind === 'climax_gate'
          ? 0xb83a2a
          : item.kind === 'checkpoint'
            ? 0x52b788
            : 0xd4a017;
      const prop = this.add.rectangle(
        item.x,
        item.y,
        item.width ?? 16,
        item.height ?? 16,
        color
      );
      prop.setOrigin(0, 0);
      prop.setDepth(computeEntityDepth(footY));
    }

    // 6. Thực thể NPC
    this.npcGraphics.clear();
    for (const npc of this.npcPlacements) {
      const npcFootY = npc.y + 16;
      const npcSprite = this.add.rectangle(npc.x, npc.y - 8, 14, 24, 0xc89b3c);
      npcSprite.setOrigin(0, 0);
      npcSprite.setDepth(computeEntityDepth(npcFootY));
      this.npcGraphics.set(npc.id, npcSprite);
    }

    // 7. Nhân vật Lữ Khách
    this.playerGraphic = this.add.rectangle(
      this.playerPos.x,
      this.playerPos.y - 16,
      14,
      24,
      0x8c2d19
    );
    this.playerGraphic.setOrigin(0, 0);
    this.playerGraphic.setDepth(this.playerDepth);

    // 8. Chỉ dẫn nhiệm vụ không gian (Objective Guidance Marker)
    const objTarget = this.getObjectiveTarget();
    if (objTarget) {
      this.objectiveMarkerGraphic = this.add.rectangle(
        objTarget.x - 4,
        objTarget.y - 4,
        8,
        8,
        0xf2c94c,
        0.95
      );
      this.objectiveMarkerGraphic.setOrigin(0, 0);
      this.objectiveMarkerGraphic.setDepth(LAYER_DEPTHS.Foreground + 50);
    } else {
      this.objectiveMarkerGraphic = undefined;
    }
  }

  public setActiveObjectiveZone(zoneId: string | null): void {
    this.activeObjectiveZoneId = zoneId;
    const objTarget = this.getObjectiveTarget();
    if (objTarget) {
      if (!this.objectiveMarkerGraphic) {
        this.objectiveMarkerGraphic = this.add.rectangle(
          objTarget.x - 4,
          objTarget.y - 4,
          8,
          8,
          0xf2c94c,
          0.95
        );
        this.objectiveMarkerGraphic.setOrigin(0, 0);
        this.objectiveMarkerGraphic.setDepth(LAYER_DEPTHS.Foreground + 50);
      } else {
        this.objectiveMarkerGraphic.setPosition(
          objTarget.x - 4,
          objTarget.y - 4
        );
      }
    }
  }

  public getObjectiveTarget(): ObjectiveTargetPosition | null {
    return getObjectiveTargetPosition(this.mapData, this.activeObjectiveZoneId);
  }

  /**
   * Chuyển đổi sang khu vực `targetZoneId` tại điểm xuất phát `spawnId`.
   */
  public switchZone(targetZoneId: string, spawnId?: string): boolean {
    const allMaps = getAllBuiltInMaps();
    const targetMap = allMaps[targetZoneId];
    if (!targetMap) {
      return false;
    }

    const resolvedSpawnId = spawnId ?? targetMap.defaultSpawnId;
    this.transition.bumpTransitionToken();
    this.transition.currentMapId = targetMap.id;
    this.transition.currentSpawnId = resolvedSpawnId;
    this.transition.cooldownRemainingSec = this.transition.portalCooldownSec;
    this.transition.requiresStepOffPortal = true;

    this.applyMapAndSpawn(targetMap, resolvedSpawnId);
    this.create();
    return true;
  }

  /**
   * Bước cập nhật di chuyển nhân vật, tuần tra NPC, Camera và chuyển cảnh qua cổng.
   */
  public stepMovement(
    dtSec: number,
    moveVec: Vec2 = { x: 0, y: 0 },
    isCrouching = false
  ): void {
    const footRect = {
      x: this.playerPos.x,
      y: this.playerPos.y,
      width: PLAYER_FOOT_WIDTH,
      height: PLAYER_FOOT_HEIGHT,
    };

    this.transition.update(dtSec, footRect, this.mapData.portals);

    // Cập nhật tuần tra NPC theo lộ trình (route)
    for (const npc of this.npcPlacements) {
      if (npc.route && npc.route.length > 0) {
        let routeState = this.npcRouteStates.get(npc.id);
        if (!routeState) {
          routeState = { waypointIndex: 0, waitRemainingSec: 0 };
          this.npcRouteStates.set(npc.id, routeState);
        }
        stepNpcAlongRoute(npc, routeState, dtSec);
        const g = this.npcGraphics.get(npc.id);
        if (g) {
          g.setPosition(npc.x, npc.y - 8);
          g.setDepth(computeEntityDepth(npc.y + 16));
        }
      }
    }

    let nx = moveVec.x;
    let ny = moveVec.y;
    const mag = Math.hypot(nx, ny);
    if (mag > 0) {
      if (mag > 1.0001 || (nx !== 0 && ny !== 0 && Math.abs(mag - 1) > 1e-4)) {
        nx /= mag;
        ny /= mag;
      }
      const speed = isCrouching ? 48 : 96;
      const nextPos = this.collisionWorld.resolveMovement(
        footRect,
        nx * speed * dtSec,
        ny * speed * dtSec
      );
      this.playerPos = nextPos;

      if (Math.abs(nx) >= Math.abs(ny)) {
        this.playerFacing = nx > 0 ? 'right' : 'left';
      } else {
        this.playerFacing = ny > 0 ? 'down' : 'up';
      }
    }

    this.playerDepth = computeEntityDepth(this.playerPos.y + PLAYER_FOOT_HEIGHT);
    if (this.playerGraphic) {
      this.playerGraphic.setPosition(this.playerPos.x, this.playerPos.y - 16);
      this.playerGraphic.setDepth(this.playerDepth);
    }

    const cam = this.cameraController.update(
      this.playerPos.x,
      this.playerPos.y,
      dtSec
    );
    this.cameras.main.scrollX = Math.round(cam.x);
    this.cameras.main.scrollY = Math.round(cam.y);

    const portalCheck = this.transition.tryPortalTransition(
      {
        x: this.playerPos.x,
        y: this.playerPos.y,
        width: PLAYER_FOOT_WIDTH,
        height: PLAYER_FOOT_HEIGHT,
      },
      this.mapData.portals,
      getAllBuiltInMaps()
    );

    if (
      portalCheck.transitioned &&
      portalCheck.targetMapId &&
      portalCheck.targetSpawnId
    ) {
      this.switchZone(portalCheck.targetMapId, portalCheck.targetSpawnId);
    }
  }

  public override update(_time: number, deltaMs: number): void {
    const dtSec = Math.min(0.05, Math.max(0, deltaMs * 0.001));
    this.stepMovement(dtSec, { x: 0, y: 0 }, false);
  }

  /**
   * Trả về bản chụp trạng thái hiện tại của Scene phục vụ kiểm thử và đồng bộ UI.
   */
  public getSnapshot(): ZoneExplorationSnapshot {
    return {
      zoneId: this.zoneId,
      title: this.mapData.title,
      districtType: this.mapData.districtType,
      playerPos: { x: this.playerPos.x, y: this.playerPos.y },
      playerFacing: this.playerFacing,
      playerDepth: this.playerDepth,
      cameraPos: this.cameraController.getPosition(),
      npcs: this.npcPlacements.map((n) => ({
        id: n.id,
        x: n.x,
        y: n.y,
        facing: n.facing,
        depth: computeEntityDepth(n.y + 16),
      })),
      objectiveTarget: this.getObjectiveTarget(),
      portalCount: this.mapData.portals.length,
      interactableCount: this.mapData.interactables.length,
    };
  }
}
