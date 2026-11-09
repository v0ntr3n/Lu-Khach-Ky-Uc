/**
 * @file src/runtime/PhaserRuntime.ts
 * @description Bộ điều phối Runtime Top-Down 2D (Phaser 3 + Tiled TMJ) cho Lữ Khách Ký Ức (M1+).
 * Cung cấp:
 * - Quy tắc phân lớp chiều sâu Y-sorting (`LAYER_DEPTHS`, `computeEntityDepth`)
 * - Lớp cơ sở `BaseTopDownScene` tương thích vòng đời `Phaser.Scene` (an toàn trong cả Browser WebGL/Canvas và Node/Vitest)
 * - Vòng lặp mô phỏng tất định (`step`) hỗ trợ di chuyển 8 hướng chuẩn hóa (1/√2),
 *   va chạm chân AABB (`CollisionWorld`), Camera kẹp biên (`CameraController`),
 *   và chuyển cảnh hai chiều qua cổng có tên (`SceneTransition`).
 * - Kết xuất trực tiếp lên Canvas 2D (`#pixel-canvas`) và tích hợp Phaser 3 (`BootScene`, `HarborScene`, `WarehouseInteriorScene`).
 */

import type Phaser from 'phaser';
import type {
  Facing4,
  InteractableDefinition,
  NormalizedMapData,
  NpcMapPlacement,
  Rect2D,
  Vec2,
} from '../types/game';
import { CollisionWorld } from '../world/CollisionWorld';
import { CameraController } from './CameraController';
import { getAllBuiltInMaps, getBuiltInMap } from './MapLoader';
import { SceneTransition, type SceneTransitionResult } from './SceneTransition';

export const LAYER_DEPTHS = {
  Ground: 10,
  GroundDetails: 20,
  LowProps: 50,
  Structures: 90,
  EntitiesBase: 100,
  Foreground: 10000,
  Weather: 12000,
} as const;

/**
 * Tính toán độ sâu hiển thị (Z-depth) của thực thể dựa trên tọa độ chân `footY`.
 * Đảm bảo thực thể có `footY` nhỏ hơn (đứng phía bắc/sau) vẽ trước thực thể có `footY` lớn hơn (đứng phía nam/trước),
 * nhưng luôn nằm dưới lớp `Foreground` (mái nhà, tán cây = 10000).
 */
export function computeEntityDepth(footY: number): number {
  return LAYER_DEPTHS.EntitiesBase + Math.round(footY);
}

export interface NpcRouteState {
  waypointIndex: number;
  waitRemainingSec: number;
}

/**
 * Cập nhật vị trí và hướng nhìn của NPC di chuyển tuần tra theo danh sách `npc.route`.
 * Tự động dừng nghỉ `waitSec` tại mỗi điểm mốc và đổi hướng nhìn 4 chiều (`Facing4`).
 */
export function stepNpcAlongRoute(
  npc: NpcMapPlacement,
  state: NpcRouteState,
  dtSec: number
): { x: number; y: number; facing: Facing4; isMoving: boolean } {
  const route = npc.route;
  if (!route || route.length === 0 || dtSec <= 0 || !Number.isFinite(dtSec)) {
    return {
      x: npc.x,
      y: npc.y,
      facing: npc.facing,
      isMoving: false,
    };
  }

  state.waypointIndex =
    ((state.waypointIndex % route.length) + route.length) % route.length;

  let remainingDt = dtSec;

  if (state.waitRemainingSec > 0) {
    if (remainingDt < state.waitRemainingSec) {
      state.waitRemainingSec -= remainingDt;
      return {
        x: npc.x,
        y: npc.y,
        facing: npc.facing,
        isMoving: false,
      };
    }
    remainingDt -= state.waitRemainingSec;
    state.waitRemainingSec = 0;
    if (remainingDt <= 1e-6) {
      return {
        x: npc.x,
        y: npc.y,
        facing: npc.facing,
        isMoving: false,
      };
    }
  }

  let target = route[state.waypointIndex];
  let dx = target.x - npc.x;
  let dy = target.y - npc.y;
  let dist = Math.hypot(dx, dy);

  // Nếu đang đứng sẵn tại waypoint hiện tại và có nhiều hơn 1 điểm, chuyển sang điểm tiếp theo
  if (dist < 0.5 && route.length > 1) {
    state.waypointIndex = (state.waypointIndex + 1) % route.length;
    target = route[state.waypointIndex];
    dx = target.x - npc.x;
    dy = target.y - npc.y;
    dist = Math.hypot(dx, dy);
  }

  if (dist < 0.5) {
    npc.x = target.x;
    npc.y = target.y;
    if (target.facing) {
      npc.facing = target.facing;
    }
    return {
      x: npc.x,
      y: npc.y,
      facing: npc.facing,
      isMoving: false,
    };
  }

  const speed = npc.moveSpeed && npc.moveSpeed > 0 ? npc.moveSpeed : 36;
  const maxStep = speed * remainingDt;

  const moveFacing: Facing4 =
    Math.abs(dx) >= Math.abs(dy)
      ? dx >= 0
        ? 'right'
        : 'left'
      : dy >= 0
        ? 'down'
        : 'up';

  if (maxStep >= dist) {
    npc.x = target.x;
    npc.y = target.y;
    npc.facing = target.facing ?? moveFacing;
    state.waitRemainingSec = Math.max(0, target.waitSec ?? 0);
    state.waypointIndex = (state.waypointIndex + 1) % route.length;
    return {
      x: npc.x,
      y: npc.y,
      facing: npc.facing,
      isMoving: false,
    };
  }

  npc.x += (dx / dist) * maxStep;
  npc.y += (dy / dist) * maxStep;
  npc.facing = moveFacing;

  return {
    x: npc.x,
    y: npc.y,
    facing: npc.facing,
    isMoving: true,
  };
}

/**
 * Thực thể đồ họa nhẹ tương thích API của Phaser GameObject phục vụ kiểm thử Scene và kết xuất.
 */
export class SceneDisplayRect {
  public x: number;
  public y: number;
  public width: number;
  public height: number;
  public fillColor: number;
  public alpha: number;
  public originX = 0.5;
  public originY = 0.5;
  public depth = 0;

  constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    fillColor = 0xffffff,
    alpha = 1
  ) {
    this.x = x;
    this.y = y;
    this.width = width;
    this.height = height;
    this.fillColor = fillColor;
    this.alpha = alpha;
  }

  public setOrigin(ox: number, oy = ox): this {
    this.originX = ox;
    this.originY = oy;
    return this;
  }

  public setDepth(depth: number): this {
    this.depth = depth;
    return this;
  }

  public setPosition(x: number, y: number): this {
    this.x = x;
    this.y = y;
    return this;
  }
}

/**
 * Lớp cơ sở tương thích vòng đời `Phaser.Scene` (`init`, `preload`, `create`, `update`),
 * hoạt động ổn định trong cả trình duyệt (WebGL/Canvas2D) lẫn môi trường kiểm thử Node/jsdom.
 */
export class BaseTopDownScene {
  public readonly sceneKey: string;
  public readonly displayList: SceneDisplayRect[] = [];
  public readonly registryData = new Map<string, unknown>();
  public activeSceneTarget: { key: string; data?: unknown } | null = null;

  public readonly registry = {
    set: (k: string, v: unknown): void => {
      this.registryData.set(k, v);
    },
    get: (k: string): unknown => this.registryData.get(k),
  };

  public readonly cameras = {
    main: {
      scrollX: 0,
      scrollY: 0,
      bounds: { x: 0, y: 0, width: 1024, height: 768 },
      setBounds: (x: number, y: number, width: number, height: number): void => {
        this.cameras.main.bounds = { x, y, width, height };
      },
    },
  };

  public readonly add = {
    rectangle: (
      x: number,
      y: number,
      width: number,
      height: number,
      fillColor = 0xffffff,
      alpha = 1
    ): SceneDisplayRect => {
      const rect = new SceneDisplayRect(x, y, width, height, fillColor, alpha);
      this.displayList.push(rect);
      return rect;
    },
  };

  public readonly scene = {
    key: '',
    start: (targetKey: string, data?: unknown): void => {
      this.activeSceneTarget = { key: targetKey, data };
    },
  };

  constructor(config: { key: string }) {
    this.sceneKey = config.key;
    this.scene.key = config.key;
  }

  public init(_data?: unknown): void {
    // Override in subclass
  }

  public preload(): void {
    // Override in subclass
  }

  public create(): void {
    // Override in subclass
  }

  public update(_time: number, _deltaMs: number): void {
    // Override in subclass
  }
}

export interface PhaserRuntimeConfig {
  canvas?: HTMLCanvasElement | null;
  parentElement?: HTMLElement | string | null;
  initialMapId?: string;
  initialSpawnId?: string;
  viewportWidth?: number;
  viewportHeight?: number;
  moveSpeed?: number;
  crouchSpeed?: number;
  portalCooldownSec?: number;
  debugCollision?: boolean;
  onMapChanged?: (map: NormalizedMapData, spawnId: string) => void;
  onInteractableTriggered?: (item: InteractableDefinition) => void;
}

export interface RuntimeStepSummary {
  mapId: string;
  playerPos: Vec2;
  playerFootRect: Rect2D;
  playerFacing: Facing4;
  playerDepth: number;
  cameraPos: Vec2;
  isMoving: boolean;
  isCrouching: boolean;
  isInShelter: boolean;
  nearbyInteractable: InteractableDefinition | null;
  transitionResult?: SceneTransitionResult;
}

export const PLAYER_FOOT_WIDTH = 12;
export const PLAYER_FOOT_HEIGHT = 8;

export class PhaserRuntime {
  public readonly maps: Record<string, NormalizedMapData>;
  public readonly transition: SceneTransition;
  public readonly camera: CameraController;
  public collisionWorld: CollisionWorld;

  public playerPos: Vec2 = { x: 160, y: 384 };
  public playerFacing: Facing4 = 'right';
  public isMoving = false;
  public isCrouching = false;
  public debugCollision = false;
  public readonly questFlags = new Set<string>();

  public readonly moveSpeed: number;
  public readonly crouchSpeed: number;
  private walkAnimTime = 0;
  private elapsedTimeSec = 0;

  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private phaserGame: Phaser.Game | null = null;

  private readonly onMapChanged?: (map: NormalizedMapData, spawnId: string) => void;
  private readonly onInteractableTriggered?: (item: InteractableDefinition) => void;

  constructor(config: PhaserRuntimeConfig = {}) {
    this.maps = getAllBuiltInMaps();
    const initialMapId = config.initialMapId ?? 'harbor_square';
    const initialMap = this.maps[initialMapId] ?? getBuiltInMap('harbor_square');
    const initialSpawnId = config.initialSpawnId ?? initialMap.defaultSpawnId;

    this.moveSpeed = config.moveSpeed ?? 96;
    this.crouchSpeed = config.crouchSpeed ?? 48;
    this.debugCollision = Boolean(config.debugCollision);
    this.onMapChanged = config.onMapChanged;
    this.onInteractableTriggered = config.onInteractableTriggered;

    this.transition = new SceneTransition(
      initialMap.id,
      initialSpawnId,
      config.portalCooldownSec ?? 0.65
    );

    const viewportW = config.viewportWidth ?? 320;
    const viewportH = config.viewportHeight ?? 180;

    this.camera = new CameraController({
      worldWidth: initialMap.widthPx,
      worldHeight: initialMap.heightPx,
      viewportWidth: viewportW,
      viewportHeight: viewportH,
      deadZoneWidth: 24,
      deadZoneHeight: 16,
      lerpSpeed: 10,
    });

    this.collisionWorld = new CollisionWorld(
      initialMap.obstacles,
      initialMap.shelters,
      { x: 0, y: 0, width: initialMap.widthPx, height: initialMap.heightPx }
    );

    this.applySpawnPosition(initialMap, initialSpawnId);

    if (config.canvas) {
      this.attachCanvas(config.canvas);
    }
  }

  public attachCanvas(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    try {
      this.ctx = canvas.getContext('2d');
      if (this.ctx) {
        this.ctx.imageSmoothingEnabled = false;
      }
    } catch {
      this.ctx = null;
    }
  }

  private applySpawnPosition(map: NormalizedMapData, spawnId: string): void {
    const spawn =
      map.spawns.find((s) => s.id === spawnId) ??
      map.spawns.find((s) => s.id === map.defaultSpawnId) ??
      map.spawns[0] ?? { id: 'default_spawn', x: 160, y: 384, facing: 'down' };

    this.playerPos = { x: spawn.x, y: spawn.y };
    this.playerFacing = spawn.facing ?? 'down';
    this.camera.setBounds(map.widthPx, map.heightPx);
    this.camera.snapTo(this.playerPos.x, this.playerPos.y);
  }

  public getCurrentMapId(): string {
    return this.transition.getCurrentMapId();
  }

  public getCurrentMap(): NormalizedMapData {
    return (
      this.maps[this.transition.getCurrentMapId()] ??
      getBuiltInMap('harbor_square')
    );
  }

  public getPlayerPosition(): Vec2 {
    return { x: this.playerPos.x, y: this.playerPos.y };
  }

  public getPlayerFootRect(pos: Vec2 = this.playerPos): Rect2D {
    return {
      x: pos.x,
      y: pos.y,
      width: PLAYER_FOOT_WIDTH,
      height: PLAYER_FOOT_HEIGHT,
    };
  }

  public getPlayerFootY(): number {
    return this.playerPos.y + PLAYER_FOOT_HEIGHT;
  }

  public getPlayerDepth(): number {
    return computeEntityDepth(this.getPlayerFootY());
  }

  public getCameraPosition(): Vec2 {
    return this.camera.getPosition();
  }

  public isPlayerInShelter(): boolean {
    const foot = this.getPlayerFootRect();
    const centerX = foot.x + foot.width * 0.5;
    const centerY = foot.y + foot.height * 0.5;
    return this.collisionWorld.isPointInShelter(centerX, centerY);
  }

  public addQuestFlag(flag: string): void {
    this.questFlags.add(flag);
  }

  public hasQuestFlag(flag: string): boolean {
    return this.questFlags.has(flag);
  }

  /**
   * Dịch chuyển tức thời tới một bản đồ và điểm xuất phát có tên (dùng cho khởi tạo hoặc kiểm thử).
   */
  public teleportToSpawn(mapId: string, spawnId: string): boolean {
    const targetMap = this.maps[mapId];
    if (!targetMap) return false;
    const spawn = targetMap.spawns.find((s) => s.id === spawnId);
    if (!spawn) return false;

    this.transition.bumpTransitionToken();
    this.transition.currentMapId = targetMap.id;
    this.transition.currentSpawnId = spawn.id;
    this.transition.cooldownRemainingSec = this.transition.portalCooldownSec;
    this.transition.requiresStepOffPortal = true;

    this.collisionWorld = new CollisionWorld(
      targetMap.obstacles,
      targetMap.shelters,
      { x: 0, y: 0, width: targetMap.widthPx, height: targetMap.heightPx }
    );
    this.applySpawnPosition(targetMap, spawn.id);
    this.onMapChanged?.(targetMap, spawn.id);
    return true;
  }

  /**
   * Tìm đối tượng tương tác (NPC, chứng cứ, cổng Climax...) gần nhất trong bán kính tương tác.
   */
  public findNearbyInteractable(): InteractableDefinition | null {
    const map = this.getCurrentMap();
    const pCenterX = this.playerPos.x + PLAYER_FOOT_WIDTH * 0.5;
    const pCenterY = this.playerPos.y + PLAYER_FOOT_HEIGHT * 0.5;

    let best: InteractableDefinition | null = null;
    let bestDist = Infinity;

    for (const item of map.interactables) {
      const iCenterX = item.x + (item.width ?? 16) * 0.5;
      const iCenterY = item.y + (item.height ?? 16) * 0.5;
      const dist = Math.hypot(pCenterX - iCenterX, pCenterY - iCenterY);
      const radius = item.radius ?? 36;
      if (dist <= radius && dist < bestDist) {
        bestDist = dist;
        best = item;
      }
    }

    return best;
  }

  /**
   * Kích hoạt tương tác phím [E] với thực thể gần nhất.
   */
  public triggerInteract(): InteractableDefinition | null {
    const nearby = this.findNearbyInteractable();
    if (!nearby) return null;
    if (nearby.grantsFlag) {
      this.questFlags.add(nearby.grantsFlag);
    }
    this.onInteractableTriggered?.(nearby);
    return nearby;
  }

  /**
   * Bước mô phỏng vật lý, di chuyển 8 hướng chuẩn hóa, chuyển cảnh qua cổng và cập nhật Camera.
   */
  public step(
    dtSec: number,
    moveInput: Vec2 = { x: 0, y: 0 },
    isCrouching = false
  ): RuntimeStepSummary {
    const totalDt = Math.max(0, Number.isFinite(dtSec) ? dtSec : 0);
    this.isCrouching = isCrouching;

    let nx = moveInput.x;
    let ny = moveInput.y;
    const mag = Math.hypot(nx, ny);
    const hasMove = mag > 0;
    if (hasMove) {
      if (mag > 1.0001 || (nx !== 0 && ny !== 0 && Math.abs(mag - 1) > 1e-4)) {
        nx /= mag;
        ny /= mag;
      }
      this.isMoving = true;
      if (Math.abs(nx) >= Math.abs(ny)) {
        this.playerFacing = nx > 0 ? 'right' : 'left';
      } else {
        this.playerFacing = ny > 0 ? 'down' : 'up';
      }
    } else {
      this.isMoving = false;
    }

    const maxSlice = 0.05;
    let remaining = totalDt;
    let lastPortalRes: SceneTransitionResult | undefined;

    do {
      const sliceDt = remaining > maxSlice ? maxSlice : remaining;
      remaining = Math.max(0, remaining - sliceDt);
      this.elapsedTimeSec += sliceDt;

      const currentMap = this.getCurrentMap();
      this.transition.update(
        sliceDt,
        this.getPlayerFootRect(),
        currentMap.portals
      );

      if (hasMove && sliceDt > 0) {
        this.walkAnimTime += sliceDt * (isCrouching ? 4 : 8);
        const speed = isCrouching ? this.crouchSpeed : this.moveSpeed;
        const dx = nx * speed * sliceDt;
        const dy = ny * speed * sliceDt;

        const resolved = this.collisionWorld.resolveMovement(
          this.getPlayerFootRect(),
          dx,
          dy
        );
        this.playerPos = resolved;
      }

      const portalRes = this.transition.tryPortalTransition(
        this.getPlayerFootRect(),
        currentMap.portals,
        this.maps,
        this.questFlags
      );
      lastPortalRes = portalRes;

      if (
        portalRes.transitioned &&
        portalRes.targetMapId &&
        portalRes.targetSpawnId
      ) {
        const nextMap = this.maps[portalRes.targetMapId];
        if (nextMap) {
          this.collisionWorld = new CollisionWorld(
            nextMap.obstacles,
            nextMap.shelters,
            { x: 0, y: 0, width: nextMap.widthPx, height: nextMap.heightPx }
          );
          this.applySpawnPosition(nextMap, portalRes.targetSpawnId);
          this.onMapChanged?.(nextMap, portalRes.targetSpawnId);
        }
        break;
      } else {
        this.camera.update(this.playerPos.x, this.playerPos.y, sliceDt);
      }
    } while (remaining > 1e-6);

    if (this.ctx && this.canvas) {
      this.renderToCanvas(this.ctx, this.canvas.width, this.canvas.height);
    }

    return {
      mapId: this.getCurrentMapId(),
      playerPos: this.getPlayerPosition(),
      playerFootRect: this.getPlayerFootRect(),
      playerFacing: this.playerFacing,
      playerDepth: this.getPlayerDepth(),
      cameraPos: this.getCameraPosition(),
      isMoving: this.isMoving,
      isCrouching: this.isCrouching,
      isInShelter: this.isPlayerInShelter(),
      nearbyInteractable: this.findNearbyInteractable(),
      transitionResult: lastPortalRes,
    };
  }

  /**
   * Trả về danh sách thực thể đã sắp xếp theo chiều sâu Y (`footY`) để kiểm thử thứ tự vẽ.
   */
  public getSortedRenderEntities(): Array<{
    id: string;
    kind: 'player' | 'npc' | 'prop' | 'tree_trunk';
    footY: number;
    depth: number;
  }> {
    const map = this.getCurrentMap();
    const entities: Array<{
      id: string;
      kind: 'player' | 'npc' | 'prop' | 'tree_trunk';
      footY: number;
      depth: number;
    }> = [];

    for (const npc of map.npcs) {
      const footY = npc.y + 16;
      entities.push({
        id: npc.id,
        kind: 'npc',
        footY,
        depth: computeEntityDepth(footY),
      });
    }

    for (const item of map.interactables) {
      if (item.kind === 'npc') continue;
      const footY = item.y + (item.height ?? 16);
      entities.push({
        id: item.id,
        kind: 'prop',
        footY,
        depth: computeEntityDepth(footY),
      });
    }

    for (let i = 0; i < map.obstacles.length; i++) {
      const obs = map.obstacles[i];
      if (obs.width === 16 && obs.height === 16) {
        const footY = obs.y + obs.height;
        entities.push({
          id: `tree_trunk_${i}`,
          kind: 'tree_trunk',
          footY,
          depth: computeEntityDepth(footY),
        });
      }
    }

    const playerFootY = this.getPlayerFootY();
    entities.push({
      id: 'player',
      kind: 'player',
      footY: playerFootY,
      depth: computeEntityDepth(playerFootY),
    });

    entities.sort((a, b) => a.depth - b.depth);
    return entities;
  }

  /**
   * Kết xuất bản đồ Top-Down 3/4 theo bảng màu Sơn Mài & Giấy Dó lên Canvas 2D.
   */
  public renderToCanvas(
    ctx: CanvasRenderingContext2D,
    viewportW = this.camera.viewportWidth,
    viewportH = this.camera.viewportHeight
  ): void {
    const map = this.getCurrentMap();
    const camX = Math.round(this.camera.x);
    const camY = Math.round(this.camera.y);
    const ts = map.tileSize;

    // 1. Nền Sơn Mài Đen (#0D0A08)
    ctx.fillStyle = '#0D0A08';
    ctx.fillRect(0, 0, viewportW, viewportH);

    ctx.save();
    ctx.translate(-camX, -camY);

    const startCol = Math.max(0, Math.floor(camX / ts));
    const endCol = Math.min(
      map.widthInTiles - 1,
      Math.ceil((camX + viewportW) / ts)
    );
    const startRow = Math.max(0, Math.floor(camY / ts));
    const endRow = Math.min(
      map.heightInTiles - 1,
      Math.ceil((camY + viewportH) / ts)
    );

    const groundLayer = map.tileLayers['Ground'] ?? [];
    const detailsLayer = map.tileLayers['GroundDetails'] ?? [];
    const lowPropsLayer = map.tileLayers['LowProps'] ?? [];
    const structuresLayer = map.tileLayers['Structures'] ?? [];
    const foregroundLayer = map.tileLayers['Foreground'] ?? [];

    // 2. Lớp Ground & GroundDetails & LowProps
    for (let r = startRow; r <= endRow; r++) {
      for (let c = startCol; c <= endCol; c++) {
        const idx = r * map.widthInTiles + c;
        const gx = c * ts;
        const gy = r * ts;

        const gVal = groundLayer[idx] ?? 1;
        if (gVal === 2) {
          ctx.fillStyle = '#2B2118';
        } else if (gVal === 3) {
          ctx.fillStyle = '#3A2818';
        } else if (gVal === 4) {
          ctx.fillStyle = '#101C26';
        } else if (gVal >= 20) {
          ctx.fillStyle = gVal === 22 ? '#3C2218' : '#241A14';
        } else {
          ctx.fillStyle = (r + c) % 2 === 0 ? '#1C1510' : '#201812';
        }
        ctx.fillRect(gx, gy, ts, ts);

        const dVal = detailsLayer[idx] ?? 0;
        if (dVal > 0) {
          ctx.fillStyle = '#4A3828';
          ctx.fillRect(gx + 2, gy + 2, ts - 4, ts - 4);
        }

        const lpVal = lowPropsLayer[idx] ?? 0;
        if (lpVal > 0) {
          ctx.fillStyle = '#5C3D24';
          ctx.fillRect(gx + 1, gy + 3, ts - 2, ts - 4);
        }
      }
    }

    // 3. Lớp Structures (Tường nhà, kiện hàng, gốc cây)
    for (let r = startRow; r <= endRow; r++) {
      for (let c = startCol; c <= endCol; c++) {
        const idx = r * map.widthInTiles + c;
        const sVal = structuresLayer[idx] ?? 0;
        if (sVal <= 0) continue;
        const gx = c * ts;
        const gy = r * ts;

        if (sVal === 11) {
          ctx.fillStyle = '#3B2313';
          ctx.fillRect(gx + 3, gy + 2, 10, 14);
        } else if (sVal === 10 || sVal === 26) {
          ctx.fillStyle = '#4E321F';
          ctx.fillRect(gx, gy, ts, ts);
          ctx.strokeStyle = '#2A180D';
          ctx.strokeRect(gx + 0.5, gy + 0.5, ts - 1, ts - 1);
        } else {
          ctx.fillStyle = '#38261B';
          ctx.fillRect(gx, gy, ts, ts);
        }
      }
    }

    // Vẽ cổng chuyển cảnh (Portals)
    for (const portal of map.portals) {
      ctx.fillStyle = 'rgba(212, 160, 23, 0.32)';
      ctx.fillRect(portal.x, portal.y, portal.width, portal.height);
      ctx.strokeStyle = '#D4A017';
      ctx.strokeRect(portal.x + 0.5, portal.y + 0.5, portal.width - 1, portal.height - 1);
    }

    // 4. Thực thể sắp xếp theo chiều sâu Y (footY): NPCs, Interactables, Player
    interface DrawableEntity {
      depth: number;
      draw: () => void;
    }
    const drawables: DrawableEntity[] = [];

    for (const item of map.interactables) {
      if (item.kind === 'npc') continue;
      const footY = item.y + (item.height ?? 16);
      drawables.push({
        depth: computeEntityDepth(footY),
        draw: () => {
          if (item.kind === 'climax_gate') {
            ctx.fillStyle = '#B83A2A';
            ctx.fillRect(item.x, item.y, item.width ?? 32, item.height ?? 32);
            ctx.strokeStyle = '#F2C94C';
            ctx.strokeRect(item.x + 2, item.y + 2, (item.width ?? 32) - 4, (item.height ?? 32) - 4);
          } else {
            ctx.fillStyle = '#D4A017';
            ctx.fillRect(item.x + 2, item.y + 2, (item.width ?? 16) - 4, (item.height ?? 16) - 4);
          }
        },
      });
    }

    for (const npc of map.npcs) {
      const footY = npc.y + 16;
      drawables.push({
        depth: computeEntityDepth(footY),
        draw: () => this.drawCharacterSprite(ctx, npc.x, npc.y, npc.facing, false, '#C89B3C', '#2B4C5E'),
      });
    }

    const playerFootY = this.getPlayerFootY();
    drawables.push({
      depth: computeEntityDepth(playerFootY),
      draw: () =>
        this.drawCharacterSprite(
          ctx,
          Math.round(this.playerPos.x),
          Math.round(this.playerPos.y),
          this.playerFacing,
          this.isMoving,
          '#F3E5AB',
          '#8C2D19'
        ),
    });

    drawables.sort((a, b) => a.depth - b.depth);
    for (const d of drawables) {
      d.draw();
    }

    // 5. Lớp Foreground (Mái hiên, tán cây cổ thụ phủ phía trên nhân vật)
    for (let r = startRow; r <= endRow; r++) {
      for (let c = startCol; c <= endCol; c++) {
        const idx = r * map.widthInTiles + c;
        const fVal = foregroundLayer[idx] ?? 0;
        if (fVal <= 0) continue;
        const gx = c * ts;
        const gy = r * ts;

        if (fVal === 13) {
          ctx.fillStyle = 'rgba(34, 62, 40, 0.88)';
          ctx.fillRect(gx, gy, ts, ts);
        } else {
          ctx.fillStyle = 'rgba(72, 34, 24, 0.92)';
          ctx.fillRect(gx, gy, ts, ts);
        }
      }
    }

    // 6. Debug Collision Overlay (nếu bật)
    if (this.debugCollision) {
      ctx.strokeStyle = 'rgba(255, 64, 64, 0.85)';
      for (const obs of map.obstacles) {
        ctx.strokeRect(obs.x + 0.5, obs.y + 0.5, obs.width - 1, obs.height - 1);
      }
      const foot = this.getPlayerFootRect();
      ctx.strokeStyle = '#00FF88';
      ctx.strokeRect(foot.x, foot.y, foot.width, foot.height);
    }

    ctx.restore();
  }

  private drawCharacterSprite(
    ctx: CanvasRenderingContext2D,
    footX: number,
    footY: number,
    facing: Facing4,
    isMoving: boolean,
    hatColor: string,
    coatColor: string
  ): void {
    const bob = isMoving ? Math.round(Math.sin(this.walkAnimTime) * 1.2) : 0;
    const spriteX = footX - 2;
    const spriteY = footY - 16 + bob;

    ctx.fillStyle = 'rgba(0, 0, 0, 0.45)';
    ctx.fillRect(footX, footY + 2, PLAYER_FOOT_WIDTH, 5);

    ctx.fillStyle = coatColor;
    ctx.fillRect(spriteX + 2, spriteY + 8, 12, 12);

    ctx.fillStyle = '#E6C699';
    ctx.fillRect(spriteX + 3, spriteY + 3, 10, 6);
    ctx.fillStyle = hatColor;
    ctx.fillRect(spriteX + 1, spriteY + 1, 14, 3);

    ctx.fillStyle = '#1A110B';
    if (facing === 'down') {
      ctx.fillRect(spriteX + 5, spriteY + 5, 2, 2);
      ctx.fillRect(spriteX + 9, spriteY + 5, 2, 2);
    } else if (facing === 'left') {
      ctx.fillRect(spriteX + 3, spriteY + 5, 2, 2);
    } else if (facing === 'right') {
      ctx.fillRect(spriteX + 11, spriteY + 5, 2, 2);
    }
  }

  /**
   * Khởi tạo thực thể Phaser.Game bất đồng bộ khi chạy trên trình duyệt thực.
   */
  public async mountPhaserGame(
    parent?: HTMLElement | string
  ): Promise<Phaser.Game | null> {
    if (typeof window === 'undefined' || typeof document === 'undefined') {
      return null;
    }
    if (this.phaserGame) {
      return this.phaserGame;
    }

    try {
      const PhaserMod = await import('phaser');
      const PhaserLib = PhaserMod.default ?? PhaserMod;
      this.phaserGame = new PhaserLib.Game({
        type: PhaserLib.CANVAS,
        width: this.camera.viewportWidth,
        height: this.camera.viewportHeight,
        parent: parent ?? undefined,
        canvas: this.canvas ?? undefined,
        pixelArt: true,
        backgroundColor: '#0D0A08',
        audio: { noAudio: true },
        banner: false,
      });
      this.phaserGame.registry.set('phaserRuntime', this);
      return this.phaserGame;
    } catch {
      return null;
    }
  }

  public destroy(): void {
    if (this.phaserGame) {
      this.phaserGame.destroy(false);
      this.phaserGame = null;
    }
  }
}

export type { NpcMapPlacement };
