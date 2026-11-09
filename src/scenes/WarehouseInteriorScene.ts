/**
 * @file src/scenes/WarehouseInteriorScene.ts
 * @description Scene nội thất Chương I — Kho Hàng Bến Cảng (`warehouse_interior`, 36x28 ô 16x16).
 * Hỗ trợ khám phá chứng cứ Sổ Vận Đơn Cửu Long (`evidence_shipping_ledger`) và quay lại
 * đúng điểm xuất phát `from_warehouse` ngoài `HarborScene` mà không bị dội cổng (portal ping-pong).
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

export interface WarehouseInteriorSceneData {
  spawnId?: string;
  transition?: SceneTransition;
}

export class WarehouseInteriorScene extends BaseTopDownScene {
  public static readonly KEY = 'WarehouseInteriorScene';
  public static readonly MAP_ID = 'warehouse_interior';

  public mapData: NormalizedMapData;
  public collisionWorld: CollisionWorld;
  public cameraController: CameraController;
  public transition: SceneTransition;

  public playerPos: Vec2 = { x: 272, y: 384 };
  public playerFacing: Facing4 = 'up';
  public playerDepth: number = computeEntityDepth(384 + PLAYER_FOOT_HEIGHT);

  public playerGraphic?: SceneDisplayRect;

  constructor() {
    super({ key: WarehouseInteriorScene.KEY });
    this.mapData = getBuiltInMap(WarehouseInteriorScene.MAP_ID);
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
    this.transition = new SceneTransition(
      WarehouseInteriorScene.MAP_ID,
      'warehouse_south_door',
      0.65
    );
  }

  public override init(data?: WarehouseInteriorSceneData): void {
    const spawnId = data?.spawnId ?? this.mapData.defaultSpawnId;
    if (data?.transition) {
      this.transition = data.transition;
    }
    const spawn =
      this.mapData.spawns.find((s) => s.id === spawnId) ??
      this.mapData.spawns[0];

    if (spawn) {
      this.playerPos = { x: spawn.x, y: spawn.y };
      this.playerFacing = spawn.facing ?? 'up';
      this.playerDepth = computeEntityDepth(this.playerPos.y + PLAYER_FOOT_HEIGHT);
      this.cameraController.snapTo(this.playerPos.x, this.playerPos.y);
    }
  }

  public override create(): void {
    this.displayList.length = 0;
    this.cameras.main.setBounds(0, 0, this.mapData.widthPx, this.mapData.heightPx);

    const floor = this.add.rectangle(
      0,
      0,
      this.mapData.widthPx,
      this.mapData.heightPx,
      0x241a14
    );
    floor.setOrigin(0, 0);
    floor.setDepth(LAYER_DEPTHS.Ground);

    for (const obs of this.mapData.obstacles) {
      const rect = this.add.rectangle(
        obs.x,
        obs.y,
        obs.width,
        obs.height,
        0x4e321f
      );
      rect.setOrigin(0, 0);
      rect.setDepth(LAYER_DEPTHS.Structures);
    }

    for (const item of this.mapData.interactables) {
      const itemFootY = item.y + (item.height ?? 16);
      const prop = this.add.rectangle(
        item.x,
        item.y,
        item.width ?? 16,
        item.height ?? 16,
        0xd4a017
      );
      prop.setOrigin(0, 0);
      prop.setDepth(computeEntityDepth(itemFootY));
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

    const beam = this.add.rectangle(32, 112, 512, 16, 0x38261b, 0.9);
    beam.setOrigin(0, 0);
    beam.setDepth(LAYER_DEPTHS.Foreground);
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
      portalCheck.targetMapId === 'harbor_square'
    ) {
      this.scene.start('HarborScene', {
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
