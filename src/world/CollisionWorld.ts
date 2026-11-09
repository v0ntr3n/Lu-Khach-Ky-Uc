/**
 * @file src/world/CollisionWorld.ts
 * @description Hệ thống kiểm tra va chạm hình học AABB thuần túy, giải quyết di chuyển trượt theo trục X/Y,
 * kiểm tra vùng ẩn nấp (ShelterZone) và tầm nhìn (Line of Sight) cho Lữ Khách Ký Ức.
 * Tuyệt đối không đọc pixel từ sprite và không cho phép trạng thái cúi người (crouch) tự tạo vùng an toàn ngoài trời.
 */

import type { Vec2, Rect2D, ShelterZone } from '../types/game';

export class CollisionWorld {
  private readonly obstacles: Rect2D[];
  private readonly shelters: ShelterZone[];
  private readonly worldBounds?: Rect2D;

  constructor(
    obstacles: Rect2D[] = [],
    shelters: ShelterZone[] = [],
    worldBounds?: Rect2D
  ) {
    this.obstacles = obstacles.map((obs) => ({ ...obs }));
    this.shelters = shelters.map((shelter) => ({
      ...shelter,
      bounds: shelter.bounds ? { ...shelter.bounds } : undefined,
    }));
    this.worldBounds = worldBounds ? { ...worldBounds } : undefined;
  }

  /**
   * Trả về danh sách các vật cản hiện tại (bản sao chỉ đọc).
   */
  public getObstacles(): readonly Rect2D[] {
    return this.obstacles;
  }

  /**
   * Trả về danh sách các vùng ẩn nấp hiện tại (bản sao chỉ đọc).
   */
  public getShelters(): readonly ShelterZone[] {
    return this.shelters;
  }

  /**
   * Trả về giới hạn biên của thế giới (nếu có).
   */
  public getWorldBounds(): Rect2D | undefined {
    return this.worldBounds ? { ...this.worldBounds } : undefined;
  }

  /**
   * Kiểm tra va chạm AABB tất định giữa hình chữ nhật `rect` với `worldBounds` và `obstacles`.
   * Nếu `worldBounds` được cung cấp, `rect` bắt buộc phải nằm trọn bên trong `worldBounds`.
   */
  public isRectColliding(rect: Rect2D): boolean {
    if (rect.width < 0 || rect.height < 0) {
      return true;
    }

    if (this.worldBounds) {
      const minX = this.worldBounds.x;
      const minY = this.worldBounds.y;
      const maxX = this.worldBounds.x + this.worldBounds.width;
      const maxY = this.worldBounds.y + this.worldBounds.height;

      if (
        rect.x < minX ||
        rect.y < minY ||
        rect.x + rect.width > maxX ||
        rect.y + rect.height > maxY
      ) {
        return true;
      }
    }

    for (let i = 0; i < this.obstacles.length; i++) {
      const obs = this.obstacles[i];
      if (obs.width <= 0 || obs.height <= 0) {
        continue;
      }
      if (CollisionWorld.rectsOverlap(rect, obs)) {
        return true;
      }
    }

    return false;
  }

  /**
   * Giải quyết di chuyển tách trục (X trước, Y sau) cho hộp va chạm bàn chân (`footRect`).
   * Cho phép nhân vật trượt dọc theo tường khi đi chéo vào vật cản.
   * @returns Tọa độ `{ x, y }` góc trên-trái mới của `footRect`.
   */
  public resolveMovement(footRect: Rect2D, dx: number, dy: number): Vec2 {
    let resolvedX = footRect.x;
    let resolvedY = footRect.y;

    if (dx !== 0) {
      const candidateX: Rect2D = {
        x: footRect.x + dx,
        y: footRect.y,
        width: footRect.width,
        height: footRect.height,
      };
      if (!this.isRectColliding(candidateX)) {
        resolvedX = candidateX.x;
      }
    }

    if (dy !== 0) {
      const candidateY: Rect2D = {
        x: resolvedX,
        y: footRect.y + dy,
        width: footRect.width,
        height: footRect.height,
      };
      if (!this.isRectColliding(candidateY)) {
        resolvedY = candidateY.y;
      }
    }

    return { x: resolvedX, y: resolvedY };
  }

  /**
   * Kiểm tra một điểm `(x, y)` có nằm trong vùng ẩn nấp hợp lệ (`ShelterZone`) hay không.
   * Quy tắc P0: Tuyệt đối KHÔNG sử dụng trạng thái cúi người (crouch) để cấp quyền ẩn nấp ngoài không gian trống.
   */
  public isPointInShelter(x: number, y?: number): boolean {
    if (!Number.isFinite(x)) {
      return false;
    }

    for (let i = 0; i < this.shelters.length; i++) {
      const shelter = this.shelters[i];
      if (shelter.bounds) {
        const b = shelter.bounds;
        const inX = x >= b.x && x <= b.x + b.width;
        const inY = y === undefined ? true : y >= b.y && y <= b.y + b.height;
        if (inX && inY) {
          return true;
        }
      } else {
        const minX = Math.min(shelter.xStart, shelter.xEnd);
        const maxX = Math.max(shelter.xStart, shelter.xEnd);
        if (x >= minX && x <= maxX) {
          return true;
        }
      }
    }

    return false;
  }

  /**
   * Kiểm tra đường nhìn thẳng (Line of Sight) từ `from` tới `to`.
   * Trả về `true` nếu đoạn thẳng không cắt bất kỳ vật cản nào trong `obstacles` hoặc `extraOccluders`.
   */
  public hasLineOfSight(
    from: Vec2,
    to: Vec2,
    extraOccluders: Rect2D[] = []
  ): boolean {
    for (let i = 0; i < this.obstacles.length; i++) {
      const obs = this.obstacles[i];
      if (obs.width <= 0 || obs.height <= 0) continue;
      if (CollisionWorld.segmentIntersectsRect(from, to, obs)) {
        return false;
      }
    }

    for (let i = 0; i < extraOccluders.length; i++) {
      const occ = extraOccluders[i];
      if (occ.width <= 0 || occ.height <= 0) continue;
      if (CollisionWorld.segmentIntersectsRect(from, to, occ)) {
        return false;
      }
    }

    return true;
  }

  /**
   * Kiểm tra chồng lấn AABB nghiêm ngặt (chạm mép không tính là chồng lấn).
   */
  public static rectsOverlap(a: Rect2D, b: Rect2D): boolean {
    return (
      a.x < b.x + b.width &&
      a.x + a.width > b.x &&
      a.y < b.y + b.height &&
      a.y + a.height > b.y
    );
  }

  /**
   * Thuật toán Liang-Barsky / Slab clipping kiểm tra đoạn thẳng `p0 -> p1` cắt hình chữ nhật `rect`.
   */
  public static segmentIntersectsRect(
    p0: Vec2,
    p1: Vec2,
    rect: Rect2D
  ): boolean {
    const rxMin = rect.x;
    const ryMin = rect.y;
    const rxMax = rect.x + rect.width;
    const ryMax = rect.y + rect.height;

    const dx = p1.x - p0.x;
    const dy = p1.y - p0.y;

    let tMin = 0;
    let tMax = 1;

    // Kiểm tra trục X
    if (dx === 0) {
      if (p0.x < rxMin || p0.x > rxMax) {
        return false;
      }
    } else {
      const invDx = 1 / dx;
      let t1 = (rxMin - p0.x) * invDx;
      let t2 = (rxMax - p0.x) * invDx;
      if (t1 > t2) {
        const tmp = t1;
        t1 = t2;
        t2 = tmp;
      }
      tMin = Math.max(tMin, t1);
      tMax = Math.min(tMax, t2);
      if (tMin > tMax) {
        return false;
      }
    }

    // Kiểm tra trục Y
    if (dy === 0) {
      if (p0.y < ryMin || p0.y > ryMax) {
        return false;
      }
    } else {
      const invDy = 1 / dy;
      let t1 = (ryMin - p0.y) * invDy;
      let t2 = (ryMax - p0.y) * invDy;
      if (t1 > t2) {
        const tmp = t1;
        t1 = t2;
        t2 = tmp;
      }
      tMin = Math.max(tMin, t1);
      tMax = Math.min(tMax, t2);
      if (tMin > tMax) {
        return false;
      }
    }

    return tMin <= tMax;
  }
}
