/**
 * @file src/ui/MiniMapPanel.ts
 * @description Bản đồ thu nhỏ tương tác thời gian thực (`#minimap-panel` / `#minimap-canvas`)
 * đặt tại góc trên bên phải HUD cho M8 (#13).
 *
 * Tính năng:
 * - Lưu đệm địa hình tĩnh (`terrainCache`) theo `mapId` để không phải vẽ lại từng ô gạch mỗi khung hình.
 * - Hiển thị vị trí & hướng nhìn 4 chiều của Lữ Khách, cổng chuyển khu vực (Portals),
 *   Đèn Báo An Toàn (Checkpoints), NPC, điểm mục tiêu nhiệm vụ đang theo dõi (Tracked Target)
 *   và đường dẫn A* nét đứt vàng đồng tránh vật cản (`TrackedRouteState.path.points`).
 * - Hỗ trợ phím tắt `[N]` hoặc nút thu gọn/mở rộng (`#btn-toggle-minimap`) và nhấp vào bản đồ thu nhỏ
 *   để mở Sơ Đồ Liên Thông Khu Vực (`WorldMapModal` — phím `[M]`).
 */

import type {
  Facing4,
  NormalizedMapData,
  RomanMilestoneId,
  Vec2,
} from '../types/game';
import type { TrackedRouteState } from '../world/GridPathfinder';

export interface MiniMapRenderParams {
  map: NormalizedMapData;
  playerX: number;
  playerY: number;
  playerFacing: Facing4;
  routeState: TrackedRouteState | null;
  unlockedCheckpoints: ReadonlySet<string>;
  collectedMilestones: ReadonlySet<RomanMilestoneId>;
  discoveredMaps: ReadonlySet<string>;
  timeSec: number;
  reducedMotion?: boolean;
}

export interface MiniMapCallbacks {
  onToggleMinimap?: (visible: boolean) => void;
  onOpenWorldMap?: () => void;
}

export class MiniMapPanel {
  private readonly callbacks: MiniMapCallbacks;
  private isCollapsed = false;
  private readonly terrainCache = new Map<string, HTMLCanvasElement>();
  private lastRouteSummary = '';
  private lastZoneTitle = '';
  private lastRenderedPointCount = 0;

  constructor(callbacks: MiniMapCallbacks = {}) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.syncVisibilityDOM();
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    const toggleBtn = document.getElementById('btn-toggle-minimap');
    toggleBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      this.toggle();
    });

    const openMapBtn = document.getElementById('btn-minimap-open-map');
    openMapBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      this.callbacks.onOpenWorldMap?.();
    });

    const canvas = document.getElementById('minimap-canvas');
    canvas?.addEventListener('click', () => {
      this.callbacks.onOpenWorldMap?.();
    });
  }

  public isVisible(): boolean {
    return !this.isCollapsed;
  }

  public setVisible(visible: boolean, notify = false): void {
    this.isCollapsed = !visible;
    this.syncVisibilityDOM();
    if (notify) {
      this.callbacks.onToggleMinimap?.(visible);
    }
  }

  public toggle(force?: boolean): boolean {
    const nextVisible = force ?? this.isCollapsed;
    this.setVisible(nextVisible, true);
    return nextVisible;
  }

  public getLastRenderedPointCount(): number {
    return this.lastRenderedPointCount;
  }

  private syncVisibilityDOM(): void {
    if (typeof document === 'undefined') return;
    const panel = document.getElementById('minimap-panel');
    const body = document.getElementById('minimap-body');
    const toggleBtn = document.getElementById('btn-toggle-minimap');

    if (panel) {
      panel.classList.toggle('is-collapsed', this.isCollapsed);
      panel.dataset.minimapVisible = String(!this.isCollapsed);
    }
    if (body) {
      body.hidden = this.isCollapsed;
      if (this.isCollapsed) {
        body.setAttribute('hidden', '');
      } else {
        body.removeAttribute('hidden');
      }
    }
    if (toggleBtn) {
      toggleBtn.textContent = this.isCollapsed ? 'Mở [N]' : 'Ẩn [N]';
      toggleBtn.setAttribute('aria-expanded', String(!this.isCollapsed));
    }
  }

  public getActiveTheme(): 'light' | 'dark' {
    if (typeof document !== 'undefined') {
      const attr =
        document.documentElement.dataset.theme || document.body?.dataset.theme;
      if (attr === 'light') return 'light';
    }
    return 'dark';
  }

  private getPalette(theme: 'light' | 'dark') {
    if (theme === 'light') {
      return {
        bgFrame: '#D8CBB0',
        ground: '#EFE7D6',
        shelterFill: 'rgba(49, 107, 74, 0.30)',
        obstacleFill: '#A89172',
        obstacleStroke: '#786042',
        borderStroke: '#7A4E22',
        portalFill: '#1B6B85',
        checkpointUnlocked: '#2D7A47',
        checkpointLocked: '#8A5A2B',
        npcFill: '#3B2E22',
        routeStroke: '#8F2918',
        targetFill: '#9E3321',
        targetPortalFill: '#1B6B85',
        targetStroke: '#221C15',
        playerFill: '#8F2918',
        playerStroke: '#FBF8F1',
      };
    }
    return {
      bgFrame: '#191410',
      ground: '#2A221B',
      shelterFill: 'rgba(66, 122, 91, 0.38)',
      obstacleFill: '#4A3828',
      obstacleStroke: '#6E5239',
      borderStroke: '#8A5A2B',
      portalFill: '#63B3ED',
      checkpointUnlocked: '#68D391',
      checkpointLocked: '#8A5A2B',
      npcFill: '#F2E6D0',
      routeStroke: '#E8B86D',
      targetFill: '#C84B31',
      targetPortalFill: '#63B3ED',
      targetStroke: '#F6EAD4',
      playerFill: '#F6EAD4',
      playerStroke: '#9E3321',
    };
  }

  private getOrCreateTerrainCanvas(
    map: NormalizedMapData,
    width: number,
    height: number,
    theme: 'light' | 'dark'
  ): HTMLCanvasElement | null {
    if (typeof document === 'undefined') return null;
    const key = `${theme}:${map.id}:${width}x${height}:${map.obstacles.length}:${map.shelters.length}`;
    const existing = this.terrainCache.get(key);
    if (existing) {
      return existing;
    }

    const offscreen = document.createElement('canvas');
    offscreen.width = width;
    offscreen.height = height;
    const ctx = offscreen.getContext('2d');
    if (!ctx) {
      return null;
    }

    const palette = this.getPalette(theme);
    const sx = width / Math.max(1, map.widthPx);
    const sy = height / Math.max(1, map.heightPx);

    // Khung nền bản đồ theo chế độ Sáng (Giấy Dó) / Tối (Sơn Mài)
    ctx.fillStyle = palette.bgFrame;
    ctx.fillRect(0, 0, width, height);

    // Lưới mặt đất đi được
    ctx.fillStyle = palette.ground;
    ctx.fillRect(2, 2, width - 4, height - 4);

    // Vùng ẩn nấp an toàn (Shelters)
    ctx.fillStyle = palette.shelterFill;
    for (const shelter of map.shelters) {
      if (shelter.bounds) {
        ctx.fillRect(
          Math.round(shelter.bounds.x * sx),
          Math.round(shelter.bounds.y * sy),
          Math.max(2, Math.round(shelter.bounds.width * sx)),
          Math.max(2, Math.round(shelter.bounds.height * sy))
        );
      }
    }

    // Khối kiến trúc / vật cản (Obstacles)
    ctx.fillStyle = palette.obstacleFill;
    ctx.strokeStyle = palette.obstacleStroke;
    ctx.lineWidth = 1;
    for (const obs of map.obstacles) {
      const ox = Math.round(obs.x * sx);
      const oy = Math.round(obs.y * sy);
      const ow = Math.max(2, Math.round(obs.width * sx));
      const oh = Math.max(2, Math.round(obs.height * sy));
      ctx.fillRect(ox, oy, ow, oh);
    }

    // Khung viền đồng cổ
    ctx.strokeStyle = palette.borderStroke;
    ctx.lineWidth = 1;
    ctx.strokeRect(0.5, 0.5, width - 1, height - 1);

    this.terrainCache.set(key, offscreen);
    return offscreen;
  }

  public render(params: MiniMapRenderParams): void {
    if (typeof document === 'undefined') return;

    const {
      map,
      playerX,
      playerY,
      playerFacing,
      routeState,
      unlockedCheckpoints,
      timeSec,
      reducedMotion,
    } = params;

    const titleEl = document.getElementById('minimap-zone-title');
    if (titleEl && this.lastZoneTitle !== map.title) {
      this.lastZoneTitle = map.title;
      titleEl.textContent = map.title;
    }

    const bannerEl = document.getElementById('minimap-route-banner');
    const summaryText =
      routeState?.routeStepBanner ||
      routeState?.resolution.nextActionText ||
      'Nhấp mục tiêu trong Nhật ký [J] để dẫn đường';
    if (bannerEl && this.lastRouteSummary !== summaryText) {
      this.lastRouteSummary = summaryText;
      bannerEl.textContent = summaryText;
    }

    this.lastRenderedPointCount = routeState?.path.points.length ?? 0;

    if (this.isCollapsed) {
      return;
    }

    const canvas = document.getElementById(
      'minimap-canvas'
    ) as HTMLCanvasElement | null;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const theme = this.getActiveTheme();
    const palette = this.getPalette(theme);
    const width = canvas.width || 168;
    const height = canvas.height || 124;
    const sx = width / Math.max(1, map.widthPx);
    const sy = height / Math.max(1, map.heightPx);

    ctx.clearRect(0, 0, width, height);

    const cachedTerrain = this.getOrCreateTerrainCanvas(map, width, height, theme);
    if (cachedTerrain) {
      try {
        ctx.drawImage(cachedTerrain, 0, 0);
      } catch {
        ctx.fillStyle = palette.bgFrame;
        ctx.fillRect(0, 0, width, height);
      }
    } else {
      ctx.fillStyle = palette.bgFrame;
      ctx.fillRect(0, 0, width, height);
    }

    // 1. Vẽ Cổng chuyển cảnh (Portals)
    for (const portal of map.portals) {
      const px = Math.round((portal.x + portal.width * 0.5) * sx);
      const py = Math.round((portal.y + portal.height * 0.5) * sy);
      ctx.fillStyle = palette.portalFill;
      ctx.fillRect(px - 2, py - 2, 5, 5);
    }

    // 2. Vẽ Đèn Báo An Toàn (Checkpoints)
    for (const cp of map.checkpoints) {
      const cx = Math.round(cp.x * sx);
      const cy = Math.round(cp.y * sy);
      const isUnlocked =
        unlockedCheckpoints.has(cp.id) || unlockedCheckpoints.has(map.id);
      ctx.fillStyle = isUnlocked
        ? palette.checkpointUnlocked
        : palette.checkpointLocked;
      ctx.fillRect(cx - 2, cy - 2, 4, 4);
    }

    // 3. Vẽ NPC & Tư liệu trong khu vực
    for (const npc of map.npcs) {
      const nx = Math.round(npc.x * sx);
      const ny = Math.round(npc.y * sy);
      ctx.fillStyle = palette.npcFill;
      ctx.fillRect(nx - 1, ny - 1, 3, 3);
    }

    // 4. Vẽ đường dẫn A* nét đứt từ Lữ Khách tới mục tiêu đang theo dõi
    const routePoints: readonly Vec2[] = routeState?.path.points ?? [];
    if (routePoints.length >= 2) {
      ctx.save();
      ctx.strokeStyle = palette.routeStroke;
      ctx.lineWidth = 2;
      if (typeof ctx.setLineDash === 'function') {
        ctx.setLineDash([4, 2]);
      }
      ctx.beginPath();
      ctx.moveTo(playerX * sx, playerY * sy);
      for (let i = 1; i < routePoints.length; i++) {
        ctx.lineTo(routePoints[i].x * sx, routePoints[i].y * sy);
      }
      ctx.stroke();
      ctx.restore();
    }

    // 5. Vẽ Điểm Mục Tiêu Nhiệm Vụ (Tracked Objective Marker)
    const targetPos = routeState?.resolution.position;
    if (targetPos) {
      const tx = Math.round(targetPos.x * sx);
      const ty = Math.round(targetPos.y * sy);
      const pulse = reducedMotion ? 1 : 1 + Math.sin(timeSec * 5) * 0.25;
      const r = Math.max(3, Math.round(4 * pulse));

      ctx.save();
      ctx.translate(tx, ty);
      ctx.rotate(Math.PI / 4);
      ctx.fillStyle = targetPos.isPortal
        ? palette.targetPortalFill
        : palette.targetFill;
      ctx.strokeStyle = palette.targetStroke;
      ctx.lineWidth = 1;
      ctx.fillRect(-r * 0.5, -r * 0.5, r, r);
      ctx.strokeRect(-r * 0.5, -r * 0.5, r, r);
      ctx.restore();
    }

    // 6. Vẽ Vị trí & Mũi tên hướng nhìn của Lữ Khách
    const plX = Math.max(4, Math.min(width - 4, playerX * sx));
    const plY = Math.max(4, Math.min(height - 4, playerY * sy));

    ctx.save();
    ctx.translate(plX, plY);
    const angle =
      playerFacing === 'right'
        ? 0
        : playerFacing === 'down'
          ? Math.PI * 0.5
          : playerFacing === 'left'
            ? Math.PI
            : -Math.PI * 0.5;
    ctx.rotate(angle);

    ctx.fillStyle = palette.playerFill;
    ctx.strokeStyle = palette.playerStroke;
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(4.5, 0);
    ctx.lineTo(-3.5, -3.2);
    ctx.lineTo(-2, 0);
    ctx.lineTo(-3.5, 3.2);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.restore();
  }
}
