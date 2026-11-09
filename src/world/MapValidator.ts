/**
 * @file src/world/MapValidator.ts
 * @description Bộ kiểm tra tính toàn vẹn của dữ liệu bản đồ top-down 16x16:
 * - Kích thước dương và tileSize === 16
 * - Điểm xuất phát (spawns) có ID duy nhất, nằm trong biên bản đồ, không đè lên vật cản
 * - Cổng chuyển cảnh (portals) có ID duy nhất, đích đến hợp lệ và tồn tại trong tập bản đồ
 * - Điểm tương tác (interactables) có ID duy nhất và nằm trong biên bản đồ
 * - Kiểm tra khả năng tiếp cận (Reachability BFS trên lưới 16x16) từ điểm xuất phát mặc định tới mọi cổng và điểm tương tác.
 */

import type {
  Rect2D,
  PortalDefinition,
  SpawnPointDefinition,
  InteractableDefinition,
} from '../types/game';
import { CollisionWorld } from './CollisionWorld';

export interface ValidatableMap {
  id: string;
  /** Chiều rộng bản đồ tính bằng pixel (ví dụ: 1024 cho bản đồ 64x48 ô 16px) */
  width: number;
  /** Chiều cao bản đồ tính bằng pixel (ví dụ: 768 cho bản đồ 64x48 ô 16px) */
  height: number;
  /** Kích thước mỗi ô lưới chuẩn (bắt buộc bằng 16) */
  tileSize: number;
  /** ID điểm xuất phát mặc định (nếu bỏ trống sẽ ưu tiên spawn có id === 'default' hoặc spawn đầu tiên) */
  defaultSpawnId?: string;
  spawns: SpawnPointDefinition[];
  portals: PortalDefinition[];
  interactables: InteractableDefinition[];
  obstacles: Rect2D[];
}

export interface MapValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

const REQUIRED_TILE_SIZE = 16;
const SPAWN_FOOT_SIZE = 8;

const FALLBACK_VALIDATABLE_MAPS: Record<string, ValidatableMap> = {};

/**
 * Đăng ký bản đồ chuẩn đã kiểm chứng vào bộ đệm dự phòng để hỗ trợ kiểm định từng phần (partial map sets).
 */
export function registerValidatableMapFallback(map: ValidatableMap): void {
  if (map && map.id) {
    FALLBACK_VALIDATABLE_MAPS[map.id] = map;
  }
}

/**
 * Kiểm tra tính hợp lệ của một bản đồ và đối chiếu liên kết cổng với `allMaps` (nếu có).
 */
export function validateMapData(
  map: ValidatableMap,
  allMaps?: Record<string, ValidatableMap>
): MapValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  const mapLabel = map.id && map.id.trim().length > 0 ? map.id : '<unnamed-map>';

  if (!map.id || map.id.trim().length === 0) {
    errors.push('Map ID must be a non-empty string.');
  }

  // 1. Kiểm tra kích thước bản đồ và tileSize === 16
  const hasValidDimensions =
    Number.isFinite(map.width) &&
    Number.isFinite(map.height) &&
    map.width > 0 &&
    map.height > 0;

  if (!hasValidDimensions) {
    errors.push(
      `Map "${mapLabel}": width and height must be positive numbers (received ${map.width}x${map.height}).`
    );
  }

  if (map.tileSize !== REQUIRED_TILE_SIZE) {
    errors.push(
      `Map "${mapLabel}": tileSize must be ${REQUIRED_TILE_SIZE} (received ${map.tileSize}).`
    );
  }

  if (hasValidDimensions && map.tileSize === REQUIRED_TILE_SIZE) {
    if (map.width % REQUIRED_TILE_SIZE !== 0 || map.height % REQUIRED_TILE_SIZE !== 0) {
      warnings.push(
        `Map "${mapLabel}": dimensions (${map.width}x${map.height}) are not exact multiples of ${REQUIRED_TILE_SIZE}.`
      );
    }
  }

  const obstacles = Array.isArray(map.obstacles) ? map.obstacles : [];
  const spawns = Array.isArray(map.spawns) ? map.spawns : [];
  const portals = Array.isArray(map.portals) ? map.portals : [];
  const interactables = Array.isArray(map.interactables) ? map.interactables : [];

  const worldBounds: Rect2D | undefined = hasValidDimensions
    ? { x: 0, y: 0, width: map.width, height: map.height }
    : undefined;
  const collisionWorld = new CollisionWorld(obstacles, [], worldBounds);

  // 2. Kiểm tra điểm xuất phát (spawns)
  if (spawns.length === 0) {
    errors.push(`Map "${mapLabel}": must define at least one spawn point.`);
  }

  const seenSpawnIds = new Set<string>();
  const validSpawns: SpawnPointDefinition[] = [];

  for (let i = 0; i < spawns.length; i++) {
    const spawn = spawns[i];
    if (!spawn.id || spawn.id.trim().length === 0) {
      errors.push(`Map "${mapLabel}": spawn at index ${i} has an empty ID.`);
      continue;
    }

    if (seenSpawnIds.has(spawn.id)) {
      errors.push(`Map "${mapLabel}": duplicate spawn ID "${spawn.id}".`);
    } else {
      seenSpawnIds.add(spawn.id);
    }

    if (hasValidDimensions) {
      const inBounds =
        spawn.x >= 0 &&
        spawn.y >= 0 &&
        spawn.x < map.width &&
        spawn.y < map.height;

      if (!inBounds) {
        errors.push(
          `Map "${mapLabel}": spawn "${spawn.id}" at (${spawn.x}, ${spawn.y}) is outside map bounds (0..${map.width}, 0..${map.height}).`
        );
        continue;
      }

      const footWidth = Math.min(SPAWN_FOOT_SIZE, map.width - spawn.x);
      const footHeight = Math.min(SPAWN_FOOT_SIZE, map.height - spawn.y);
      const spawnFootRect: Rect2D = {
        x: spawn.x,
        y: spawn.y,
        width: Math.max(1, footWidth),
        height: Math.max(1, footHeight),
      };

      const pointInsideObstacle = obstacles.some(
        (obs) =>
          obs.width > 0 &&
          obs.height > 0 &&
          spawn.x >= obs.x &&
          spawn.x < obs.x + obs.width &&
          spawn.y >= obs.y &&
          spawn.y < obs.y + obs.height
      );

      if (pointInsideObstacle || collisionWorld.isRectColliding(spawnFootRect)) {
        errors.push(
          `Map "${mapLabel}": spawn "${spawn.id}" at (${spawn.x}, ${spawn.y}) overlaps a collision obstacle.`
        );
        continue;
      }

      validSpawns.push(spawn);
    }
  }

  // 3. Kiểm tra cổng chuyển cảnh (portals)
  const seenPortalIds = new Set<string>();
  for (let i = 0; i < portals.length; i++) {
    const portal = portals[i];
    if (!portal.id || portal.id.trim().length === 0) {
      errors.push(`Map "${mapLabel}": portal at index ${i} has an empty ID.`);
    } else if (seenPortalIds.has(portal.id)) {
      errors.push(`Map "${mapLabel}": duplicate portal ID "${portal.id}".`);
    } else {
      seenPortalIds.add(portal.id);
    }

    if (hasValidDimensions) {
      const portalInBounds =
        portal.width > 0 &&
        portal.height > 0 &&
        portal.x >= 0 &&
        portal.y >= 0 &&
        portal.x + portal.width <= map.width &&
        portal.y + portal.height <= map.height;

      if (!portalInBounds) {
        errors.push(
          `Map "${mapLabel}": portal "${portal.id || i}" at (${portal.x}, ${portal.y}, ${portal.width}x${portal.height}) is outside map bounds.`
        );
      }
    }

    const hasDestMap =
      typeof portal.destinationMap === 'string' &&
      portal.destinationMap.trim().length > 0;
    const hasDestSpawn =
      typeof portal.destinationSpawn === 'string' &&
      portal.destinationSpawn.trim().length === 0
        ? false
        : Boolean(portal.destinationSpawn && portal.destinationSpawn.trim().length > 0);

    if (!hasDestMap) {
      errors.push(
        `Map "${mapLabel}": portal "${portal.id || i}" is missing a valid destinationMap.`
      );
    }

    if (!hasDestSpawn) {
      errors.push(
        `Map "${mapLabel}": portal "${portal.id || i}" is missing a valid destinationSpawn.`
      );
    }

    if (allMaps && hasDestMap && hasDestSpawn) {
      const targetMap =
        allMaps[portal.destinationMap] ??
        Object.values(allMaps).find((m) => m.id === portal.destinationMap) ??
        (FALLBACK_VALIDATABLE_MAPS[map.id]
          ? FALLBACK_VALIDATABLE_MAPS[portal.destinationMap]
          : undefined);

      if (!targetMap) {
        errors.push(
          `Map "${mapLabel}": portal "${portal.id}" points to non-existent destinationMap "${portal.destinationMap}".`
        );
      } else {
        const targetSpawnExists = Array.isArray(targetMap.spawns) &&
          targetMap.spawns.some((s) => s.id === portal.destinationSpawn);
        if (!targetSpawnExists) {
          errors.push(
            `Map "${mapLabel}": portal "${portal.id}" points to missing destinationSpawn "${portal.destinationSpawn}" in map "${portal.destinationMap}".`
          );
        }
      }
    }
  }

  // 4. Kiểm tra các đối tượng tương tác (interactables)
  const seenInteractableIds = new Set<string>();
  for (let i = 0; i < interactables.length; i++) {
    const item = interactables[i];
    if (!item.id || item.id.trim().length === 0) {
      errors.push(
        `Map "${mapLabel}": interactable at index ${i} has an empty ID.`
      );
    } else if (seenInteractableIds.has(item.id)) {
      errors.push(
        `Map "${mapLabel}": duplicate interactable ID "${item.id}".`
      );
    } else {
      seenInteractableIds.add(item.id);
    }

    if (hasValidDimensions) {
      const itemWidth = item.width && item.width > 0 ? item.width : 0;
      const itemHeight = item.height && item.height > 0 ? item.height : 0;
      const inBounds =
        item.x >= 0 &&
        item.y >= 0 &&
        item.x < map.width &&
        item.y < map.height &&
        item.x + itemWidth <= map.width &&
        item.y + itemHeight <= map.height;

      if (!inBounds) {
        errors.push(
          `Map "${mapLabel}": interactable "${item.id || i}" at (${item.x}, ${item.y}) is outside map bounds.`
        );
      }
    }
  }

  // 5. Kiểm tra Reachability bằng BFS trên lưới 16x16 từ điểm xuất phát mặc định
  if (
    hasValidDimensions &&
    map.tileSize === REQUIRED_TILE_SIZE &&
    validSpawns.length > 0
  ) {
    const defaultSpawn =
      (map.defaultSpawnId
        ? validSpawns.find((s) => s.id === map.defaultSpawnId)
        : undefined) ??
      validSpawns.find((s) => s.id === 'default') ??
      validSpawns[0];

    const cols = Math.max(1, Math.floor(map.width / REQUIRED_TILE_SIZE));
    const rows = Math.max(1, Math.floor(map.height / REQUIRED_TILE_SIZE));

    // Đánh dấu các ô 16x16 bị vật cản chiếm chỗ
    const blocked: boolean[][] = Array.from({ length: rows }, () =>
      Array(cols).fill(false)
    );

    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const tileRect: Rect2D = {
          x: c * REQUIRED_TILE_SIZE,
          y: r * REQUIRED_TILE_SIZE,
          width: REQUIRED_TILE_SIZE,
          height: REQUIRED_TILE_SIZE,
        };
        for (let k = 0; k < obstacles.length; k++) {
          const obs = obstacles[k];
          if (obs.width <= 0 || obs.height <= 0) continue;
          if (CollisionWorld.rectsOverlap(tileRect, obs)) {
            blocked[r][c] = true;
            break;
          }
        }
      }
    }

    const startCol = Math.floor(defaultSpawn.x / REQUIRED_TILE_SIZE);
    const startRow = Math.floor(defaultSpawn.y / REQUIRED_TILE_SIZE);

    const visited: boolean[][] = Array.from({ length: rows }, () =>
      Array(cols).fill(false)
    );

    if (
      startCol >= 0 &&
      startCol < cols &&
      startRow >= 0 &&
      startRow < rows &&
      !blocked[startRow][startCol]
    ) {
      const queue: Array<[number, number]> = [[startCol, startRow]];
      visited[startRow][startCol] = true;
      let head = 0;

      const dirs: ReadonlyArray<[number, number]> = [
        [1, 0],
        [-1, 0],
        [0, 1],
        [0, -1],
      ];

      while (head < queue.length) {
        const [cx, cy] = queue[head++];
        for (let d = 0; d < dirs.length; d++) {
          const nx = cx + dirs[d][0];
          const ny = cy + dirs[d][1];
          if (
            nx >= 0 &&
            nx < cols &&
            ny >= 0 &&
            ny < rows &&
            !blocked[ny][nx] &&
            !visited[ny][nx]
          ) {
            visited[ny][nx] = true;
            queue.push([nx, ny]);
          }
        }
      }
    }

    const isRectReachable = (rect: Rect2D): boolean => {
      const minC = Math.max(0, Math.floor(rect.x / REQUIRED_TILE_SIZE));
      const maxC = Math.min(
        cols - 1,
        Math.floor(Math.max(rect.x, rect.x + rect.width - 1) / REQUIRED_TILE_SIZE)
      );
      const minR = Math.max(0, Math.floor(rect.y / REQUIRED_TILE_SIZE));
      const maxR = Math.min(
        rows - 1,
        Math.floor(Math.max(rect.y, rect.y + rect.height - 1) / REQUIRED_TILE_SIZE)
      );

      for (let r = minR; r <= maxR; r++) {
        for (let c = minC; c <= maxC; c++) {
          if (visited[r][c]) {
            return true;
          }
        }
      }
      return false;
    };

    // Kiểm tra mọi portal có thể tiếp cận từ defaultSpawn
    for (let i = 0; i < portals.length; i++) {
      const portal = portals[i];
      if (
        portal.x < 0 ||
        portal.y < 0 ||
        portal.width <= 0 ||
        portal.height <= 0 ||
        portal.x + portal.width > map.width ||
        portal.y + portal.height > map.height
      ) {
        continue;
      }

      const portalRect: Rect2D = {
        x: portal.x,
        y: portal.y,
        width: portal.width,
        height: portal.height,
      };

      if (!isRectReachable(portalRect)) {
        errors.push(
          `Map "${mapLabel}": portal "${portal.id}" is unreachable from default spawn "${defaultSpawn.id}".`
        );
      }
    }

    // Kiểm tra mọi interactable có thể tiếp cận từ defaultSpawn
    for (let i = 0; i < interactables.length; i++) {
      const item = interactables[i];
      const w = item.width && item.width > 0 ? item.width : 1;
      const h = item.height && item.height > 0 ? item.height : 1;
      if (
        item.x < 0 ||
        item.y < 0 ||
        item.x >= map.width ||
        item.y >= map.height ||
        item.x + w > map.width ||
        item.y + h > map.height
      ) {
        continue;
      }

      const itemRect: Rect2D = {
        x: item.x,
        y: item.y,
        width: w,
        height: h,
      };

      if (!isRectReachable(itemRect)) {
        errors.push(
          `Map "${mapLabel}": interactable "${item.id}" is unreachable from default spawn "${defaultSpawn.id}".`
        );
      }
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}
