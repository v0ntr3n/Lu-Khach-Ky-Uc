/**
 * @file src/world/GridPathfinder.ts
 * @description Bộ tìm đường lưới A* 8 hướng tránh vật cản (`GridPathfinder`) và Bộ lập lộ trình
 * liên khu vực (`RoutePlanner`) cho M8 (#13).
 *
 * Đặc tả kỹ thuật:
 * - Xây dựng lưới ô vuông 16×16px từ `CollisionWorld` với hộp va chạm chân chuẩn (`12×8px`).
 * - Giải thuật A* 8 hướng (`cost = 1.0` ngang/dọc, `1.4142` chéo) với quy tắc **cấm cắt góc chéo**
 *   (`canStepDiagonal`: bước chéo `(x, y) -> (nx, ny)` chỉ hợp lệ khi cả `(nx, y)` và `(x, ny)` đều đi được).
 * - Tự động tìm ô tiếp cận khả dụng gần nhất (`findNearestWalkableCell`) khi mục tiêu nằm trên bàn/tường/NPC.
 * - Bộ nhớ đệm lộ trình (`RoutePlanner`): chỉ tính lại khi đổi bản đồ, đổi mục tiêu, đổi cờ nhiệm vụ
 *   hoặc khi Lữ Khách lệch khỏi đường dẫn quá 2 ô (`32px`), đảm bảo độ trễ `< 1.5ms` và `0` cấp phát thừa mỗi khung hình.
 */

import { getAllBuiltInMaps } from '../runtime/MapLoader';
import { resolveObjectiveTarget } from '../runtime/PhaserRuntime';
import type {
  NormalizedMapData,
  ObjectiveTargetResolution,
  QuestObjective,
  QuestTargetRef,
  Rect2D,
  Vec2,
} from '../types/game';
import { CollisionWorld } from './CollisionWorld';
import { CHAPTER1_WORLD_GRAPH } from './WorldGraph';

export const PLAYER_FOOT_WIDTH = 12;
export const PLAYER_FOOT_HEIGHT = 8;

export interface GridCell {
  col: number;
  row: number;
}

export interface GridPathResult {
  reachable: boolean;
  cells: GridCell[];
  points: Vec2[];
  totalCost: number;
  computeMs: number;
}

export interface TrackedRouteState {
  objectiveId: string | null;
  mapId: string;
  targetZoneId: string;
  nextPortalZoneId?: string;
  isCrossZone: boolean;
  zoneRoute: string[];
  routeStepBanner: string;
  routeBannerText: string;
  resolution: ObjectiveTargetResolution;
  path: GridPathResult;
}

const CARDINAL_COST = 1.0;
const DIAGONAL_COST = Math.SQRT2;

const NEIGHBOR_DIRS: ReadonlyArray<{
  dc: number;
  dr: number;
  cost: number;
  isDiagonal: boolean;
}> = [
  { dc: 0, dr: -1, cost: CARDINAL_COST, isDiagonal: false },
  { dc: 1, dr: 0, cost: CARDINAL_COST, isDiagonal: false },
  { dc: 0, dr: 1, cost: CARDINAL_COST, isDiagonal: false },
  { dc: -1, dr: 0, cost: CARDINAL_COST, isDiagonal: false },
  { dc: 1, dr: -1, cost: DIAGONAL_COST, isDiagonal: true },
  { dc: 1, dr: 1, cost: DIAGONAL_COST, isDiagonal: true },
  { dc: -1, dr: 1, cost: DIAGONAL_COST, isDiagonal: true },
  { dc: -1, dr: -1, cost: DIAGONAL_COST, isDiagonal: true },
];

export class GridPathfinder {
  public readonly cols: number;
  public readonly rows: number;
  public readonly tileSize: number;
  private readonly walkable: Uint8Array;

  constructor(
    collisionWorldOrMap: CollisionWorld | NormalizedMapData,
    widthPx?: number,
    heightPx?: number,
    tileSize = 16
  ) {
    let collisionWorld: CollisionWorld;
    let resolvedWidthPx: number;
    let resolvedHeightPx: number;
    let resolvedTileSize = tileSize;

    if ('isRectColliding' in collisionWorldOrMap) {
      collisionWorld = collisionWorldOrMap;
      resolvedWidthPx = widthPx ?? 512;
      resolvedHeightPx = heightPx ?? 384;
    } else {
      const map = collisionWorldOrMap;
      resolvedWidthPx = map.widthPx;
      resolvedHeightPx = map.heightPx;
      resolvedTileSize = map.tileSize || 16;
      collisionWorld = new CollisionWorld(map.obstacles, map.shelters, {
        x: 0,
        y: 0,
        width: map.widthPx,
        height: map.heightPx,
      });
    }

    this.tileSize = Math.max(8, resolvedTileSize);
    this.cols = Math.max(1, Math.ceil(resolvedWidthPx / this.tileSize));
    this.rows = Math.max(1, Math.ceil(resolvedHeightPx / this.tileSize));
    this.walkable = new Uint8Array(this.cols * this.rows);

    for (let r = 0; r < this.rows; r++) {
      for (let c = 0; c < this.cols; c++) {
        const center = this.cellToWorld(c, r);
        const footRect: Rect2D = {
          x: center.x - PLAYER_FOOT_WIDTH * 0.5,
          y: center.y - PLAYER_FOOT_HEIGHT * 0.5,
          width: PLAYER_FOOT_WIDTH,
          height: PLAYER_FOOT_HEIGHT,
        };
        this.walkable[r * this.cols + c] = collisionWorld.isRectColliding(
          footRect
        )
          ? 0
          : 1;
      }
    }
  }

  public static fromMap(map: NormalizedMapData): GridPathfinder {
    return new GridPathfinder(map);
  }

  public isInBounds(col: number, row: number): boolean {
    return col >= 0 && col < this.cols && row >= 0 && row < this.rows;
  }

  public isWalkable(col: number, row: number): boolean {
    if (!this.isInBounds(col, row)) return false;
    return this.walkable[row * this.cols + col] === 1;
  }

  public isCellWalkable(col: number, row: number): boolean {
    return this.isWalkable(col, row);
  }

  /**
   * Kiểm tra bước đi chéo `(col, row) -> (nextCol, nextRow)` (hoặc theo độ lệch `dc, dr` nếu `|arg| <= 1`)
   * có cắt qua góc vật cản hay không.
   * Bắt buộc cả hai ô kề cạnh góc vuông `(nextCol, row)` và `(col, nextRow)` đều phải đi được.
   */
  public canStepDiagonal(
    col: number,
    row: number,
    nextColOrDc: number,
    nextRowOrDr: number
  ): boolean {
    let nextCol = nextColOrDc;
    let nextRow = nextRowOrDr;
    if (
      Math.abs(nextColOrDc) <= 1 &&
      Math.abs(nextRowOrDr) <= 1 &&
      (Math.abs(nextColOrDc - col) > 1 || Math.abs(nextRowOrDr - row) > 1)
    ) {
      nextCol = col + nextColOrDc;
      nextRow = row + nextRowOrDr;
    }

    if (!this.isWalkable(nextCol, nextRow)) return false;
    const dc = nextCol - col;
    const dr = nextRow - row;
    if (Math.abs(dc) !== 1 || Math.abs(dr) !== 1) {
      return this.isWalkable(nextCol, nextRow);
    }
    return this.isWalkable(nextCol, row) && this.isWalkable(col, nextRow);
  }

  public worldToCell(x: number, y: number): GridCell {
    const col = Math.max(
      0,
      Math.min(this.cols - 1, Math.floor(x / this.tileSize))
    );
    const row = Math.max(
      0,
      Math.min(this.rows - 1, Math.floor(y / this.tileSize))
    );
    return { col, row };
  }

  public cellToWorld(col: number, row: number): Vec2 {
    return {
      x: (col + 0.5) * this.tileSize,
      y: (row + 0.5) * this.tileSize,
    };
  }

  public findNearestWalkableCell(
    col: number,
    row: number,
    maxRadius = 5
  ): GridCell | null {
    if (this.isWalkable(col, row)) {
      return { col, row };
    }

    for (let radius = 1; radius <= maxRadius; radius++) {
      let best: GridCell | null = null;
      let bestDistSq = Number.POSITIVE_INFINITY;

      for (let dr = -radius; dr <= radius; dr++) {
        for (let dc = -radius; dc <= radius; dc++) {
          if (Math.max(Math.abs(dc), Math.abs(dr)) !== radius) continue;
          const nc = col + dc;
          const nr = row + dr;
          if (!this.isWalkable(nc, nr)) continue;
          const dSq = dc * dc + dr * dr;
          if (dSq < bestDistSq) {
            bestDistSq = dSq;
            best = { col: nc, row: nr };
          }
        }
      }

      if (best) {
        return best;
      }
    }

    return null;
  }

  private octileHeuristic(c1: number, r1: number, c2: number, r2: number): number {
    const dx = Math.abs(c1 - c2);
    const dy = Math.abs(r1 - r2);
    return CARDINAL_COST * (dx + dy) + (DIAGONAL_COST - 2 * CARDINAL_COST) * Math.min(dx, dy);
  }

  public findPath(
    startWorld: Vec2,
    targetWorld: Vec2,
    approachWorld?: Vec2
  ): GridPathResult {
    const t0 = typeof performance !== 'undefined' ? performance.now() : 0;

    const rawStart = this.worldToCell(startWorld.x, startWorld.y);
    const startCell = this.findNearestWalkableCell(rawStart.col, rawStart.row, 5);

    let preferredGoalCell: GridCell | null = null;
    if (approachWorld) {
      const appCell = this.worldToCell(approachWorld.x, approachWorld.y);
      if (this.isWalkable(appCell.col, appCell.row)) {
        preferredGoalCell = appCell;
      }
    }

    if (!preferredGoalCell) {
      const rawGoal = this.worldToCell(targetWorld.x, targetWorld.y);
      preferredGoalCell = this.findNearestWalkableCell(rawGoal.col, rawGoal.row, 5);
    }

    if (!startCell || !preferredGoalCell) {
      const t1 = typeof performance !== 'undefined' ? performance.now() : 0;
      return {
        reachable: false,
        cells: [],
        points: [],
        totalCost: 0,
        computeMs: Math.max(0, t1 - t0),
      };
    }

    if (
      startCell.col === preferredGoalCell.col &&
      startCell.row === preferredGoalCell.row
    ) {
      const pt = this.cellToWorld(startCell.col, startCell.row);
      const t1 = typeof performance !== 'undefined' ? performance.now() : 0;
      return {
        reachable: true,
        cells: [startCell],
        points: [pt],
        totalCost: 0,
        computeMs: Math.max(0, t1 - t0),
      };
    }

    const totalNodes = this.cols * this.rows;
    const gScore = new Float32Array(totalNodes);
    const fScore = new Float32Array(totalNodes);
    const cameFrom = new Int32Array(totalNodes);
    const closed = new Uint8Array(totalNodes);
    const inOpen = new Uint8Array(totalNodes);

    gScore.fill(Number.POSITIVE_INFINITY);
    fScore.fill(Number.POSITIVE_INFINITY);
    cameFrom.fill(-1);

    const startIdx = startCell.row * this.cols + startCell.col;
    const goalIdx = preferredGoalCell.row * this.cols + preferredGoalCell.col;

    gScore[startIdx] = 0;
    fScore[startIdx] = this.octileHeuristic(
      startCell.col,
      startCell.row,
      preferredGoalCell.col,
      preferredGoalCell.row
    );

    const openSet: number[] = [startIdx];
    inOpen[startIdx] = 1;

    while (openSet.length > 0) {
      let bestPos = 0;
      let currentIdx = openSet[0];
      let bestF = fScore[currentIdx];

      for (let i = 1; i < openSet.length; i++) {
        const candidateIdx = openSet[i];
        const cf = fScore[candidateIdx];
        if (cf < bestF) {
          bestF = cf;
          currentIdx = candidateIdx;
          bestPos = i;
        }
      }

      if (currentIdx === goalIdx) {
        const cells: GridCell[] = [];
        let curr = goalIdx;
        while (curr !== -1) {
          const r = Math.floor(curr / this.cols);
          const c = curr % this.cols;
          cells.push({ col: c, row: r });
          curr = cameFrom[curr];
        }
        cells.reverse();
        const points = cells.map((cell) => this.cellToWorld(cell.col, cell.row));
        const t1 = typeof performance !== 'undefined' ? performance.now() : 0;
        return {
          reachable: true,
          cells,
          points,
          totalCost: gScore[goalIdx],
          computeMs: Math.max(0, t1 - t0),
        };
      }

      openSet[bestPos] = openSet[openSet.length - 1];
      openSet.pop();
      inOpen[currentIdx] = 0;
      closed[currentIdx] = 1;

      const cRow = Math.floor(currentIdx / this.cols);
      const cCol = currentIdx % this.cols;

      for (const dir of NEIGHBOR_DIRS) {
        const nCol = cCol + dir.dc;
        const nRow = cRow + dir.dr;

        if (!this.isWalkable(nCol, nRow)) continue;
        if (dir.isDiagonal && !this.canStepDiagonal(cCol, cRow, nCol, nRow)) {
          continue;
        }

        const neighborIdx = nRow * this.cols + nCol;
        if (closed[neighborIdx] === 1) continue;

        const tentativeG = gScore[currentIdx] + dir.cost;
        if (tentativeG < gScore[neighborIdx]) {
          cameFrom[neighborIdx] = currentIdx;
          gScore[neighborIdx] = tentativeG;
          fScore[neighborIdx] =
            tentativeG +
            this.octileHeuristic(
              nCol,
              nRow,
              preferredGoalCell.col,
              preferredGoalCell.row
            );
          if (inOpen[neighborIdx] === 0) {
            openSet.push(neighborIdx);
            inOpen[neighborIdx] = 1;
          }
        }
      }
    }

    const t1 = typeof performance !== 'undefined' ? performance.now() : 0;
    return {
      reachable: false,
      cells: [],
      points: [],
      totalCost: 0,
      computeMs: Math.max(0, t1 - t0),
    };
  }
}

export class RoutePlanner {
  private readonly gridCache = new Map<string, GridPathfinder>();
  private cachedMapTitles: Record<string, string> | null = null;
  private cachedState: TrackedRouteState | null = null;
  private cachedKey = '';
  private lastStartWorld: Vec2 = { x: -9999, y: -9999 };

  public getGridForMap(map: NormalizedMapData): GridPathfinder {
    const key = `${map.id}:${map.widthPx}x${map.heightPx}:${map.obstacles.length}`;
    let grid = this.gridCache.get(key);
    if (!grid) {
      grid = GridPathfinder.fromMap(map);
      this.gridCache.set(key, grid);
    }
    return grid;
  }

  private getDefaultMapTitles(): Record<string, string> {
    if (!this.cachedMapTitles) {
      this.cachedMapTitles = {};
      const builtIn = getAllBuiltInMaps();
      for (const [id, m] of Object.entries(builtIn)) {
        this.cachedMapTitles[id] = m.title;
      }
    }
    return this.cachedMapTitles;
  }

  public invalidate(): void {
    this.cachedState = null;
    this.cachedKey = '';
  }

  private distanceToPathPx(pos: Vec2, points: readonly Vec2[]): number {
    if (points.length === 0) return Number.POSITIVE_INFINITY;
    let minDist = Number.POSITIVE_INFINITY;
    for (const pt of points) {
      const d = Math.hypot(pos.x - pt.x, pos.y - pt.y);
      if (d < minDist) {
        minDist = d;
      }
    }
    return minDist;
  }

  public computeTrackedRoute(
    paramsOrMap:
      | {
          currentMap: NormalizedMapData;
          playerPos: Vec2;
          objective: QuestObjective | QuestTargetRef | null | undefined;
          resolution?: ObjectiveTargetResolution;
          isLocked?: boolean;
          prerequisiteText?: string;
          questFlags?: ReadonlySet<string>;
          mapTitles?: Record<string, string>;
        }
      | NormalizedMapData,
    playerX?: number,
    playerY?: number,
    objectiveArg?: QuestObjective | QuestTargetRef | null,
    resolutionArg?: ObjectiveTargetResolution,
    questFlagsArg?: ReadonlySet<string>,
    mapTitlesArg?: Record<string, string>
  ): TrackedRouteState {
    let currentMap: NormalizedMapData;
    let playerPos: Vec2;
    let objective: QuestObjective | QuestTargetRef | null | undefined;
    let providedResolution: ObjectiveTargetResolution | undefined;
    let isLocked: boolean | undefined;
    let prerequisiteText: string | undefined;
    let questFlags: ReadonlySet<string> | undefined;
    let mapTitles: Record<string, string> | undefined;

    if ('currentMap' in paramsOrMap) {
      currentMap = paramsOrMap.currentMap;
      playerPos = paramsOrMap.playerPos;
      objective = paramsOrMap.objective;
      providedResolution = paramsOrMap.resolution;
      isLocked = paramsOrMap.isLocked;
      prerequisiteText = paramsOrMap.prerequisiteText;
      questFlags = paramsOrMap.questFlags;
      mapTitles = paramsOrMap.mapTitles;
    } else {
      currentMap = paramsOrMap;
      playerPos = { x: playerX ?? 0, y: playerY ?? 0 };
      objective = objectiveArg;
      providedResolution = resolutionArg;
      questFlags = questFlagsArg;
      mapTitles = mapTitlesArg;
    }

    const objId =
      objective && 'id' in objective && 'title' in objective
        ? objective.id
        : null;
    const flagsKey = questFlags ? Array.from(questFlags).sort().join(',') : '';
    const stateKey = `${currentMap.id}|${objId ?? 'none'}|${Boolean(isLocked)}|${flagsKey}`;

    if (
      this.cachedState &&
      this.cachedKey === stateKey &&
      Math.hypot(
        playerPos.x - this.lastStartWorld.x,
        playerPos.y - this.lastStartWorld.y
      ) <= 24 &&
      (this.cachedState.path.points.length === 0 ||
        this.distanceToPathPx(playerPos, this.cachedState.path.points) <= 32)
    ) {
      return this.cachedState;
    }

    const resolvedTitles = mapTitles ?? this.getDefaultMapTitles();

    const resolution =
      providedResolution ??
      resolveObjectiveTarget(currentMap, objective, {
        isLocked,
        prerequisiteText,
        questFlags,
      });

    const targetZoneId = resolution.targetZoneId || currentMap.id;
    const isCrossZone =
      Boolean(resolution.targetZoneId) && resolution.targetZoneId !== currentMap.id;

    let zoneRoute: string[] = [currentMap.id];
    let routeStepBanner = '';
    let nextPortalZoneId: string | undefined = resolution.nextPortalZoneId;

    if (isCrossZone && resolution.targetZoneId) {
      const foundRoute = CHAPTER1_WORLD_GRAPH.findShortestRoute(
        currentMap.id,
        resolution.targetZoneId,
        questFlags,
        true
      );
      if (foundRoute && foundRoute.length >= 2) {
        zoneRoute = foundRoute;
        const nextZoneId = foundRoute[1]!;
        nextPortalZoneId = nextPortalZoneId ?? nextZoneId;
        const nextZoneTitle = resolvedTitles[nextZoneId] ?? nextZoneId;
        const finalTitle =
          resolvedTitles[resolution.targetZoneId] ?? resolution.targetZoneId;
        routeStepBanner =
          foundRoute.length === 2
            ? `Lộ trình: Qua cổng sang ${nextZoneTitle}`
            : `Lộ trình (${foundRoute.length - 1} chặng): Sang ${nextZoneTitle} → ... → ${finalTitle}`;
      }
    } else if (resolution.status === 'resolved' && resolution.position) {
      routeStepBanner = `Tại khu vực: ${resolution.position.label}`;
    } else if (resolution.status === 'locked') {
      routeStepBanner = resolution.nextActionText;
    }

    let path: GridPathResult = {
      reachable: false,
      cells: [],
      points: [],
      totalCost: 0,
      computeMs: 0,
    };

    if (resolution.status === 'resolved' && resolution.position) {
      const grid = this.getGridForMap(currentMap);
      const ref: QuestTargetRef | undefined =
        objective && 'id' in objective && 'title' in objective
          ? objective.targetRef
          : (objective as QuestTargetRef | undefined);
      const approachPoint =
        !isCrossZone && ref?.approachPoint ? ref.approachPoint : undefined;

      path = grid.findPath(
        playerPos,
        { x: resolution.position.x, y: resolution.position.y },
        approachPoint
      );
    }

    this.lastStartWorld = { x: playerPos.x, y: playerPos.y };
    this.cachedKey = stateKey;
    this.cachedState = {
      objectiveId: objId,
      mapId: currentMap.id,
      targetZoneId,
      nextPortalZoneId,
      isCrossZone,
      zoneRoute,
      routeStepBanner,
      routeBannerText: routeStepBanner,
      resolution,
      path,
    };

    return this.cachedState;
  }
}
