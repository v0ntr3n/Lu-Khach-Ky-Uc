/**
 * @file src/runtime/SceneTransition.ts
 * @description Quản lý chuyển cảnh hai chiều qua cổng (portals) với điểm xuất phát có tên (named spawns),
 * thời gian miễn nhiễm sau khi dịch chuyển (`portalCooldownSec = 0.65`), cờ `requiresStepOffPortal`
 * chống hiện tượng dội cổng liên tục (portal ping-pong), và `transitionToken` chống race condition bất đồng bộ.
 */

import type {
  Facing4,
  NormalizedMapData,
  PortalDefinition,
  Rect2D,
  SpawnPointDefinition,
  Vec2,
} from '../types/game';
import { CollisionWorld } from '../world/CollisionWorld';

export interface SceneTransitionResult {
  transitioned: boolean;
  reason?:
    | 'ok'
    | 'no_overlap'
    | 'cooldown'
    | 'step_off_required'
    | 'missing_flag'
    | 'missing_map'
    | 'missing_spawn';
  portal?: PortalDefinition;
  blockedByFlag?: string;
  targetMapId?: string;
  targetSpawnId?: string;
  spawnPos?: Vec2;
  spawnFacing?: Facing4;
  transitionToken: number;
}

function hasFlag(
  activeFlags: Set<string> | string[] | undefined,
  flag: string
): boolean {
  if (!flag) return true;
  if (!activeFlags) return false;
  if (activeFlags instanceof Set) {
    return activeFlags.has(flag);
  }
  return activeFlags.includes(flag);
}

export class SceneTransition {
  public readonly portalCooldownSec: number;
  public cooldownRemainingSec = 0;
  public requiresStepOffPortal = false;
  public transitionToken = 0;
  public currentMapId: string;
  public currentSpawnId: string;

  constructor(
    initialMapId = 'harbor_square',
    initialSpawnId = 'default_spawn',
    portalCooldownSec = 0.65
  ) {
    this.currentMapId = initialMapId;
    this.currentSpawnId = initialSpawnId;
    this.portalCooldownSec = Math.max(0, portalCooldownSec);
  }

  public getCurrentMapId(): string {
    return this.currentMapId;
  }

  public getCurrentSpawnId(): string {
    return this.currentSpawnId;
  }

  public getTransitionToken(): number {
    return this.transitionToken;
  }

  public bumpTransitionToken(): number {
    this.transitionToken += 1;
    return this.transitionToken;
  }

  public isTokenValid(token: number): boolean {
    return token === this.transitionToken;
  }

  public getCooldownRemaining(): number {
    return this.cooldownRemainingSec;
  }

  /**
   * Cập nhật bộ đếm thời gian miễn nhiễm cổng và tự động gỡ cờ `requiresStepOffPortal`
   * khi hộp va chạm chân của nhân vật đã bước ra khỏi mọi vùng cổng.
   */
  public update(
    dtSec: number,
    playerFootRect?: Rect2D,
    portals?: PortalDefinition[]
  ): void {
    if (dtSec > 0 && this.cooldownRemainingSec > 0) {
      this.cooldownRemainingSec = Math.max(0, this.cooldownRemainingSec - dtSec);
    }

    if (this.requiresStepOffPortal && playerFootRect && portals) {
      const overlappingAny = portals.some((portal) =>
        CollisionWorld.rectsOverlap(playerFootRect, {
          x: portal.x,
          y: portal.y,
          width: portal.width,
          height: portal.height,
        })
      );
      if (!overlappingAny) {
        this.requiresStepOffPortal = false;
      }
    }
  }

  /**
   * Kiểm tra xem nhân vật có thể kích hoạt cổng ở thời điểm hiện tại hay không.
   */
  public canTriggerPortal(
    portal?: PortalDefinition,
    activeFlags?: Set<string> | string[]
  ): boolean {
    if (this.cooldownRemainingSec > 0 || this.requiresStepOffPortal) {
      return false;
    }
    if (portal?.requiresFlag && !hasFlag(activeFlags, portal.requiresFlag)) {
      return false;
    }
    return true;
  }

  /**
   * Kiểm tra va chạm giữa `playerFootRect` và danh sách `portals` của bản đồ hiện tại.
   * Nếu đủ điều kiện, thực hiện chuyển cảnh sang `destinationMap` tại đúng `destinationSpawn`.
   */
  public tryPortalTransition(
    playerFootRect: Rect2D,
    portals: PortalDefinition[],
    maps: Record<string, NormalizedMapData>,
    activeFlags?: Set<string> | string[]
  ): SceneTransitionResult {
    const overlappingPortal = portals.find((portal) =>
      CollisionWorld.rectsOverlap(playerFootRect, {
        x: portal.x,
        y: portal.y,
        width: portal.width,
        height: portal.height,
      })
    );

    if (!overlappingPortal) {
      this.requiresStepOffPortal = false;
      return {
        transitioned: false,
        reason: 'no_overlap',
        transitionToken: this.transitionToken,
      };
    }

    if (this.requiresStepOffPortal) {
      return {
        transitioned: false,
        reason: 'step_off_required',
        portal: overlappingPortal,
        transitionToken: this.transitionToken,
      };
    }

    if (this.cooldownRemainingSec > 0) {
      return {
        transitioned: false,
        reason: 'cooldown',
        portal: overlappingPortal,
        transitionToken: this.transitionToken,
      };
    }

    if (
      overlappingPortal.requiresFlag &&
      !hasFlag(activeFlags, overlappingPortal.requiresFlag)
    ) {
      return {
        transitioned: false,
        reason: 'missing_flag',
        portal: overlappingPortal,
        blockedByFlag: overlappingPortal.requiresFlag,
        transitionToken: this.transitionToken,
      };
    }

    const res = this.transitionTo(
      overlappingPortal.destinationMap,
      overlappingPortal.destinationSpawn,
      maps,
      overlappingPortal.requiresFlag,
      activeFlags
    );

    return {
      ...res,
      portal: overlappingPortal,
    };
  }

  /**
   * Thực thi chuyển cảnh trực tiếp tới `destinationMapId` và `destinationSpawnId`.
   */
  public transitionTo(
    destinationMapId: string,
    destinationSpawnId: string,
    maps: Record<string, NormalizedMapData>,
    requiresFlag?: string,
    activeFlags?: Set<string> | string[]
  ): SceneTransitionResult {
    if (requiresFlag && !hasFlag(activeFlags, requiresFlag)) {
      return {
        transitioned: false,
        reason: 'missing_flag',
        blockedByFlag: requiresFlag,
        transitionToken: this.transitionToken,
      };
    }

    const targetMap =
      maps[destinationMapId] ??
      Object.values(maps).find((m) => m.id === destinationMapId);

    if (!targetMap) {
      return {
        transitioned: false,
        reason: 'missing_map',
        transitionToken: this.transitionToken,
      };
    }

    const targetSpawn: SpawnPointDefinition | undefined =
      targetMap.spawns.find((s) => s.id === destinationSpawnId) ??
      targetMap.spawns.find((s) => s.id === targetMap.defaultSpawnId) ??
      targetMap.spawns[0];

    if (
      !targetSpawn ||
      !targetMap.spawns.some((s) => s.id === destinationSpawnId)
    ) {
      return {
        transitioned: false,
        reason: 'missing_spawn',
        transitionToken: this.transitionToken,
      };
    }

    const token = this.bumpTransitionToken();
    this.currentMapId = targetMap.id;
    this.currentSpawnId = targetSpawn.id;
    this.cooldownRemainingSec = this.portalCooldownSec;
    this.requiresStepOffPortal = true;

    return {
      transitioned: true,
      reason: 'ok',
      targetMapId: targetMap.id,
      targetSpawnId: targetSpawn.id,
      spawnPos: { x: targetSpawn.x, y: targetSpawn.y },
      spawnFacing: targetSpawn.facing ?? 'down',
      transitionToken: token,
    };
  }
}
