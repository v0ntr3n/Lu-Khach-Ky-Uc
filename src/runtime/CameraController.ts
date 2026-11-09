/**
 * @file src/runtime/CameraController.ts
 * @description Bộ điều khiển Camera 2D cho góc nhìn Top-Down 3/4:
 * - Bám mượt theo nhân vật (smooth follow lerp)
 * - Vùng trễ trung tâm (dead-zone 24x16 px) chống rung giật khi nhích 1 pixel
 * - Kẹp biên nghiêm ngặt trong [0, max(0, worldWidth - viewportWidth)] và [0, max(0, worldHeight - viewportHeight)]
 *   để không bao giờ lộ khoảng trống đen (void) ngoài rìa bản đồ.
 */

import type { Rect2D, Vec2 } from '../types/game';

export interface CameraConfig {
  worldWidth: number;
  worldHeight: number;
  viewportWidth?: number;
  viewportHeight?: number;
  deadZoneWidth?: number;
  deadZoneHeight?: number;
  lerpSpeed?: number;
}

export class CameraController {
  public x = 0;
  public y = 0;
  public worldWidth: number;
  public worldHeight: number;
  public viewportWidth: number;
  public viewportHeight: number;
  public deadZoneWidth: number;
  public deadZoneHeight: number;
  public lerpSpeed: number;

  constructor(
    configOrWorldWidth: CameraConfig | number = 1024,
    worldHeight = 768,
    viewportWidth = 320,
    viewportHeight = 180,
    deadZoneWidth = 24,
    deadZoneHeight = 16,
    lerpSpeed = 10
  ) {
    if (typeof configOrWorldWidth === 'object' && configOrWorldWidth !== null) {
      this.worldWidth = Math.max(0, configOrWorldWidth.worldWidth);
      this.worldHeight = Math.max(0, configOrWorldWidth.worldHeight);
      this.viewportWidth = Math.max(1, configOrWorldWidth.viewportWidth ?? 320);
      this.viewportHeight = Math.max(1, configOrWorldWidth.viewportHeight ?? 180);
      this.deadZoneWidth = Math.max(0, configOrWorldWidth.deadZoneWidth ?? 24);
      this.deadZoneHeight = Math.max(0, configOrWorldWidth.deadZoneHeight ?? 16);
      this.lerpSpeed = Math.max(0.1, configOrWorldWidth.lerpSpeed ?? 10);
    } else {
      this.worldWidth = Math.max(0, configOrWorldWidth);
      this.worldHeight = Math.max(0, worldHeight);
      this.viewportWidth = Math.max(1, viewportWidth);
      this.viewportHeight = Math.max(1, viewportHeight);
      this.deadZoneWidth = Math.max(0, deadZoneWidth);
      this.deadZoneHeight = Math.max(0, deadZoneHeight);
      this.lerpSpeed = Math.max(0.1, lerpSpeed);
    }
  }

  /**
   * Cập nhật lại kích thước biên bản đồ và khung nhìn.
   */
  public setBounds(
    worldWidth: number,
    worldHeight: number,
    viewportWidth = this.viewportWidth,
    viewportHeight = this.viewportHeight
  ): void {
    this.worldWidth = Math.max(0, worldWidth);
    this.worldHeight = Math.max(0, worldHeight);
    this.viewportWidth = Math.max(1, viewportWidth);
    this.viewportHeight = Math.max(1, viewportHeight);
    const clamped = this.clamp(this.x, this.y);
    this.x = clamped.x;
    this.y = clamped.y;
  }

  /**
   * Kẹp tọa độ góc trên-trái của camera vào biên hợp lệ của bản đồ.
   */
  public clamp(camX: number, camY: number): Vec2 {
    const maxX = Math.max(0, this.worldWidth - this.viewportWidth);
    const maxY = Math.max(0, this.worldHeight - this.viewportHeight);
    const x = Math.max(0, Math.min(maxX, Number.isFinite(camX) ? camX : 0));
    const y = Math.max(0, Math.min(maxY, Number.isFinite(camY) ? camY : 0));
    return { x, y };
  }

  /**
   * Đặt tức thời vị trí camera căn giữa vào mục tiêu `(targetX, targetY)` có kẹp biên.
   */
  public snapTo(targetX: number, targetY: number): Vec2 {
    const desiredX = targetX - this.viewportWidth * 0.5;
    const desiredY = targetY - this.viewportHeight * 0.5;
    const clamped = this.clamp(desiredX, desiredY);
    this.x = clamped.x;
    this.y = clamped.y;
    return { x: this.x, y: this.y };
  }

  /**
   * Cập nhật vị trí camera bám mượt theo `(targetX, targetY)` có tính vùng dead-zone 24x16.
   */
  public update(targetX: number, targetY: number, dtSec = 1 / 60): Vec2 {
    const currentCenterX = this.x + this.viewportWidth * 0.5;
    const currentCenterY = this.y + this.viewportHeight * 0.5;

    const halfDeadX = this.deadZoneWidth * 0.5;
    const halfDeadY = this.deadZoneHeight * 0.5;

    let desiredCenterX = currentCenterX;
    let desiredCenterY = currentCenterY;

    if (targetX < currentCenterX - halfDeadX) {
      desiredCenterX = targetX + halfDeadX;
    } else if (targetX > currentCenterX + halfDeadX) {
      desiredCenterX = targetX - halfDeadX;
    }

    if (targetY < currentCenterY - halfDeadY) {
      desiredCenterY = targetY + halfDeadY;
    } else if (targetY > currentCenterY + halfDeadY) {
      desiredCenterY = targetY - halfDeadY;
    }

    const desiredTopLeft = this.clamp(
      desiredCenterX - this.viewportWidth * 0.5,
      desiredCenterY - this.viewportHeight * 0.5
    );

    const alpha = dtSec <= 0 ? 1 : Math.min(1, Math.max(0, dtSec * this.lerpSpeed));

    let nextX = this.x + (desiredTopLeft.x - this.x) * alpha;
    let nextY = this.y + (desiredTopLeft.y - this.y) * alpha;

    if (Math.abs(desiredTopLeft.x - nextX) < 0.05) {
      nextX = desiredTopLeft.x;
    }
    if (Math.abs(desiredTopLeft.y - nextY) < 0.05) {
      nextY = desiredTopLeft.y;
    }

    const clamped = this.clamp(nextX, nextY);
    this.x = clamped.x;
    this.y = clamped.y;

    return { x: this.x, y: this.y };
  }

  /**
   * Bí danh tương đương `update()` cho các scene gọi `cameraController.follow(...)`.
   */
  public follow(targetX: number, targetY: number, dtSec = 1 / 60): Vec2 {
    return this.update(targetX, targetY, dtSec);
  }

  public getPosition(): Vec2 {
    return { x: this.x, y: this.y };
  }

  public getDeadZoneRect(): Rect2D {
    return {
      x: this.x + (this.viewportWidth - this.deadZoneWidth) * 0.5,
      y: this.y + (this.viewportHeight - this.deadZoneHeight) * 0.5,
      width: this.deadZoneWidth,
      height: this.deadZoneHeight,
    };
  }

  public getViewportRect(): Rect2D {
    return {
      x: Math.round(this.x),
      y: Math.round(this.y),
      width: this.viewportWidth,
      height: this.viewportHeight,
    };
  }
}
