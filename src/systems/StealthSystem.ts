/**
 * @file src/systems/StealthSystem.ts
 * @description Hệ thống AI Tuần tra & Ẩn nấp 2D Top-Down (Stealth & Vision Cone System)
 * cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Quy tắc P0:
 * - Cúi người (phím S) ngoài vùng ẩn nấp (ShelterZone) KHÔNG bao giờ cấp quyền miễn nhiễm.
 * - Chỉ khi đứng bên trong ShelterZone hoặc bị che khuất đường nhìn (Line-of-Sight blocked bởi tường/occluder)
 *   người chơi mới tránh được nón tầm nhìn của lính tuần tra.
 */

import type {
  NormalizedMapData,
  PatrolActorRenderState,
  PatrolWaypoint,
  Rect2D,
  StealthAlertState,
  Vec2,
} from '../types/game';
import type { CollisionWorld } from '../world/CollisionWorld';

export interface PatrolRuntimeState {
  id: string;
  label: string;
  warningToast: string;
  x: number;
  y: number;
  speed: number;
  waypoints: PatrolWaypoint[];
  waypointIndex: number;
  waitRemainingSec: number;
  facingAngleRad: number;
  coneLength: number;
  coneAngleRad: number;
  alertState: StealthAlertState;
  suspicionLevel: number;
}

function normalizeAngleDiff(a: number, b: number): number {
  let diff = a - b;
  while (diff > Math.PI) diff -= Math.PI * 2;
  while (diff < -Math.PI) diff += Math.PI * 2;
  return Math.abs(diff);
}

export class StealthSystem {
  private guards: PatrolRuntimeState[] = [];

  public initForMap(map: NormalizedMapData): void {
    this.guards = map.patrols.map((p) => {
      const startWp = p.waypoints[0] ?? { x: 256, y: 256 };
      const nextWp = p.waypoints[1] ?? startWp;
      const initialAngle = Math.atan2(
        nextWp.y - startWp.y,
        nextWp.x - startWp.x
      );

      return {
        id: p.id,
        label: p.label,
        warningToast: p.warningToast,
        x: startWp.x,
        y: startWp.y,
        speed: p.speed || 44,
        waypoints: p.waypoints.map((wp) => ({ ...wp })),
        waypointIndex: p.waypoints.length > 1 ? 1 : 0,
        waitRemainingSec: 0,
        facingAngleRad: initialAngle,
        coneLength: p.coneLength || 96,
        coneAngleRad: p.coneAngleRad || Math.PI / 3,
        alertState: 'calm',
        suspicionLevel: 0,
      };
    });
  }

  public getGuards(): readonly PatrolRuntimeState[] {
    return this.guards;
  }

  public isPointInVisionCone(
    px: number,
    py: number,
    guardX: number,
    guardY: number,
    facingAngleRad: number,
    coneLength: number,
    coneAngleRad: number
  ): boolean {
    const dx = px - guardX;
    const dy = py - guardY;
    const dist = Math.hypot(dx, dy);
    if (dist > coneLength) {
      return false;
    }
    if (dist < 1e-4) {
      return true;
    }
    const targetAngle = Math.atan2(dy, dx);
    const angleDiff = normalizeAngleDiff(targetAngle, facingAngleRad);
    return angleDiff <= coneAngleRad * 0.5;
  }

  public isPlayerDetected(
    playerPos: Vec2,
    isSheltered: boolean,
    collisionWorld: CollisionWorld,
    occluders: Rect2D[],
    guard: PatrolRuntimeState,
    hasScheduleBonus = false
  ): boolean {
    if (isSheltered) {
      return false;
    }

    const effectiveConeLength = hasScheduleBonus
      ? guard.coneLength * 0.78
      : guard.coneLength;

    const inCone = this.isPointInVisionCone(
      playerPos.x,
      playerPos.y,
      guard.x,
      guard.y,
      guard.facingAngleRad,
      effectiveConeLength,
      guard.coneAngleRad
    );
    if (!inCone) {
      return false;
    }

    return collisionWorld.hasLineOfSight(
      { x: guard.x, y: guard.y },
      playerPos,
      occluders
    );
  }

  public update(
    dtSec: number,
    playerPos: Vec2,
    isSheltered: boolean,
    map: NormalizedMapData,
    collisionWorld: CollisionWorld,
    hasScheduleBonus = false
  ): {
    triggeredAlertGuard: PatrolRuntimeState | null;
    highestAlertState: StealthAlertState;
    maxSuspicion: number;
  } {
    let triggeredAlertGuard: PatrolRuntimeState | null = null;
    let maxSuspicion = 0;
    let highestAlertState: StealthAlertState = 'calm';

    for (const guard of this.guards) {
      if (dtSec > 0 && guard.waypoints.length > 1) {
        if (guard.waitRemainingSec > 0) {
          guard.waitRemainingSec = Math.max(0, guard.waitRemainingSec - dtSec);
        } else {
          const target = guard.waypoints[guard.waypointIndex % guard.waypoints.length];
          if (target) {
            const dx = target.x - guard.x;
            const dy = target.y - guard.y;
            const dist = Math.hypot(dx, dy);
            const step = guard.speed * dtSec;

            if (dist > 0.01) {
              guard.facingAngleRad = Math.atan2(dy, dx);
            }

            if (dist <= step || dist < 0.5) {
              guard.x = target.x;
              guard.y = target.y;
              guard.waitRemainingSec = target.waitSec ?? 0.8;
              guard.waypointIndex = (guard.waypointIndex + 1) % guard.waypoints.length;
              const nextWp = guard.waypoints[guard.waypointIndex];
              if (nextWp) {
                guard.facingAngleRad = Math.atan2(
                  nextWp.y - guard.y,
                  nextWp.x - guard.x
                );
              }
            } else {
              guard.x += (dx / dist) * step;
              guard.y += (dy / dist) * step;
            }
          }
        }
      }

      const detected = this.isPlayerDetected(
        playerPos,
        isSheltered,
        collisionWorld,
        map.occluders,
        guard,
        hasScheduleBonus
      );

      if (detected) {
        const gainRate = hasScheduleBonus ? 0.85 : 1.35;
        guard.suspicionLevel = Math.min(1, guard.suspicionLevel + dtSec * gainRate);
      } else {
        guard.suspicionLevel = Math.max(0, guard.suspicionLevel - dtSec * 0.65);
      }

      if (guard.suspicionLevel >= 1) {
        guard.alertState = 'alert';
        if (!triggeredAlertGuard) {
          triggeredAlertGuard = { ...guard };
        }
        highestAlertState = 'alert';
        maxSuspicion = 1;
        guard.suspicionLevel = 0;
        guard.alertState = 'calm';
      } else if (guard.suspicionLevel >= 0.25) {
        guard.alertState = 'suspicious';
        if (highestAlertState === 'calm') {
          highestAlertState = 'suspicious';
        }
      } else {
        guard.alertState = 'calm';
      }

      if (guard.suspicionLevel > maxSuspicion) {
        maxSuspicion = guard.suspicionLevel;
      }
    }

    if (triggeredAlertGuard) {
      highestAlertState = 'alert';
      maxSuspicion = 1;
    }

    return {
      triggeredAlertGuard,
      highestAlertState,
      maxSuspicion,
    };
  }

  public getRenderStates(hasScheduleBonus = false): PatrolActorRenderState[] {
    return this.guards.map((g) => ({
      id: g.id,
      label: g.label,
      x: g.x,
      y: g.y,
      facingAngleRad: g.facingAngleRad,
      coneLength: hasScheduleBonus ? g.coneLength * 0.78 : g.coneLength,
      coneAngleRad: g.coneAngleRad,
      alertState: g.alertState,
      suspicionLevel: g.suspicionLevel,
    }));
  }

  public resetSuspicion(): void {
    for (const g of this.guards) {
      g.suspicionLevel = 0;
      g.alertState = 'calm';
    }
  }
}
