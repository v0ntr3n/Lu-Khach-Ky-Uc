/**
 * @file src/scenes/HarborScene.ts
 * @description Scene ngoài trời Chương I — Quảng Trường Bến Cảng Hương Cảng 1930 (`harbor_square`, 64x48 ô 16x16).
 * Tích hợp Phaser 3 Scene lifecycle, MapLoader, CollisionWorld, CameraController, SceneTransition và Y-depth sorting.
 */

import type { Facing4, NormalizedMapData, Vec2 } from '../types/game';
import { CameraController } from '../runtime/CameraController';
import { getAllBuiltInMaps, getBuiltInMap } from '../runtime/MapLoader';
import {
  BaseTopDownScene,
  computeEntityDepth,
  LAYER_DEPTHS,
  PLAYER_FOOT_HEIGHT,
  PLAYER_FOOT_WIDTH,
  type SceneDisplayRect,
} from '../runtime/PhaserRuntime';
import { SceneTransition } from '../runtime/SceneTransition';
import { CollisionWorld } from '../world/CollisionWorld';

export { computeEntityDepth, LAYER_DEPTHS };

export interface HarborSceneData {
  spawnId?: string;
  transition?: SceneTransition;
}

export class HarborScene extends BaseTopDownScene {
  public static readonly KEY = 'HarborScene';
  public static readonly MAP_ID = 'harbor_square';

  public mapData: NormalizedMapData;
  public collisionWorld: CollisionWorld;
  public cameraController: CameraController;
  public transition: SceneTransition;

  public playerPos: Vec2 = { x: 160, y: 384 };
  public playerFacing: Facing4 = 'right';
  public playerDepth: number = computeEntityDepth(384 + PLAYER_FOOT_HEIGHT);

  public playerGraphic?: SceneDisplayRect;
  public npcGraphics: Map<string, SceneDisplayRect> = new Map();

  constructor() {
    super({ key: HarborScene.KEY });
    this.mapData = getBuiltInMap(HarborScene.MAP_ID);
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
    this.transition = new SceneTransition(HarborScene.MAP_ID, 'default_spawn', 0.65);
  }

  public override init(data?: HarborSceneData): void {
    const spawnId = data?.spawnId ?? this.mapData.defaultSpawnId;
    if (data?.transition) {
      this.transition = data.transition;
    }
    const spawn =
      this.mapData.spawns.find((s) => s.id === spawnId) ??
      this.mapData.spawns[0];

    if (spawn) {
      this.playerPos = { x: spawn.x, y: spawn.y };
      this.playerFacing = spawn.facing ?? 'right';
      this.playerDepth = computeEntityDepth(this.playerPos.y + PLAYER_FOOT_HEIGHT);
      this.cameraController.snapTo(this.playerPos.x, this.playerPos.y);
    }
  }

  public override create(): void {
    this.displayList.length = 0;
    this.cameras.main.setBounds(0, 0, this.mapData.widthPx, this.mapData.heightPx);

    const groundBg = this.add.rectangle(
      0,
      0,
      this.mapData.widthPx,
      this.mapData.heightPx,
      0x1c1510
    );
    groundBg.setOrigin(0, 0);
    groundBg.setDepth(LAYER_DEPTHS.Ground);

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

    this.npcGraphics.clear();
    for (const npc of this.mapData.npcs) {
      const npcFootY = npc.y + 16;
      const npcSprite = this.add.rectangle(npc.x, npc.y - 8, 14, 24, 0xc89b3c);
      npcSprite.setOrigin(0, 0);
      npcSprite.setDepth(computeEntityDepth(npcFootY));
      this.npcGraphics.set(npc.id, npcSprite);
    }

    this.playerGraphic = this.add.rectangle(
      this.playerPos.x,
      this.playerPos.y - 16,
      14,
      24,
      0x8c2d19
    );
    this.playerGraphic.setOrigin(0, 0);
    this.playerGraphic.setDepth(this.playerDepth);

    const canopyWest = this.add.rectangle(320, 272, 48, 48, 0x223e28, 0.88);
    canopyWest.setOrigin(0, 0);
    canopyWest.setDepth(LAYER_DEPTHS.Foreground);

    const canopyEast = this.add.rectangle(640, 272, 48, 48, 0x223e28, 0.88);
    canopyEast.setOrigin(0, 0);
    canopyEast.setDepth(LAYER_DEPTHS.Foreground);
  }

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
      portalCheck.targetMapId === 'warehouse_interior'
    ) {
      this.scene.start('WarehouseInteriorScene', {
        spawnId: portalCheck.targetSpawnId,
        transition: this.transition,
      });
    }
  }

  public override update(_time: number, deltaMs: number): void {
    const dtSec = Math.min(0.05, Math.max(0, deltaMs * 0.001));
    this.stepMovement(dtSec, { x: 0, y: 0 }, false);
  }
}
