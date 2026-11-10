/**
 * @file src/ui/GameShell.ts
 * @description Bộ điều phối khung nhìn toàn màn hình `100vw × 100dvh` (`#game-shell`)
 * đảm bảo tỷ lệ phóng đại số nguyên (`pixelScale >= 2`), không kéo giãn hay làm mờ điểm ảnh,
 * đồng thời ánh xạ chính xác tọa độ thế giới sang tọa độ màn hình CSS cho HUD ngữ cảnh.
 */

import type { CameraZoomPreset, Vec2, ViewportMetrics } from '../types/game';

export interface GameShellOptions {
  shellSelector?: string;
  canvas?: HTMLCanvasElement | null;
  initialZoom?: CameraZoomPreset;
  onViewportChange?: (metrics: ViewportMetrics) => void;
}

export class GameShell {
  private readonly shellSelector: string;
  private shellEl: HTMLElement | null;
  private canvas: HTMLCanvasElement | null;
  private zoomPreset: CameraZoomPreset;
  private metrics: ViewportMetrics;
  private readonly onViewportChange?: (metrics: ViewportMetrics) => void;
  private resizeObserver: ResizeObserver | null = null;
  private readonly boundHandleViewportEvent: () => void;

  /**
   * Tính toán thông số khung nhìn điểm ảnh thuần túy (pure static helper phục vụ cả runtime và unit test).
   */
  public static computeViewportMetrics(
    cssWidth: number,
    cssHeight: number,
    zoomPreset: CameraZoomPreset = 'normal',
    isTouchDevice = false
  ): ViewportMetrics {
    const safeW = Math.max(320, Math.floor(cssWidth || 1280));
    const safeH = Math.max(180, Math.floor(cssHeight || 720));

    const baseLogicalHeight =
      zoomPreset === 'wide' ? 330 : zoomPreset === 'close' ? 205 : 245;

    const pixelScale =
      safeH <= 500 && safeW > safeH
        ? Math.max(
            2,
            Math.round(
              safeH /
                (zoomPreset === 'close' ? 165 : zoomPreset === 'wide' ? 230 : 195)
            )
          )
        : Math.max(2, Math.round(safeH / baseLogicalHeight));

    const logicalWidth = Math.max(240, Math.ceil(safeW / pixelScale));
    const logicalHeight = Math.max(135, Math.ceil(safeH / pixelScale));
    const isMobileLandscape =
      safeW > safeH && (safeH <= 520 || (isTouchDevice && safeW <= 1024));

    return {
      cssWidth: safeW,
      cssHeight: safeH,
      pixelScale,
      logicalWidth,
      logicalHeight,
      zoomPreset,
      isTouchDevice: Boolean(isTouchDevice),
      isMobileLandscape,
    };
  }

  constructor(options?: GameShellOptions) {
    this.shellSelector = options?.shellSelector ?? '#game-shell';
    this.shellEl =
      typeof document !== 'undefined'
        ? (document.querySelector(this.shellSelector) as HTMLElement | null)
        : null;
    this.canvas =
      options?.canvas ??
      (typeof document !== 'undefined'
        ? (document.getElementById('pixel-canvas') as HTMLCanvasElement | null)
        : null);
    this.zoomPreset = options?.initialZoom ?? 'normal';
    this.onViewportChange = options?.onViewportChange;

    this.metrics = GameShell.computeViewportMetrics(
      1280,
      720,
      this.zoomPreset,
      this.detectTouchDevice()
    );

    this.boundHandleViewportEvent = () => {
      this.refresh();
    };

    this.refresh(true);

    if (typeof window !== 'undefined') {
      window.addEventListener('resize', this.boundHandleViewportEvent);
      window.addEventListener('orientationchange', this.boundHandleViewportEvent);
      window.visualViewport?.addEventListener(
        'resize',
        this.boundHandleViewportEvent
      );
    }

    if (typeof ResizeObserver !== 'undefined' && this.shellEl) {
      this.resizeObserver = new ResizeObserver(() => {
        this.refresh();
      });
      this.resizeObserver.observe(this.shellEl);
    }
  }

  private detectTouchDevice(): boolean {
    if (typeof window === 'undefined') {
      return false;
    }
    const hasTouchPoints =
      typeof navigator !== 'undefined' &&
      typeof navigator.maxTouchPoints === 'number' &&
      navigator.maxTouchPoints > 0;
    const hasCoarsePointer = Boolean(
      typeof window.matchMedia === 'function' &&
        window.matchMedia('(pointer: coarse)').matches
    );
    return 'ontouchstart' in window || hasTouchPoints || hasCoarsePointer;
  }

  private readCssSize(): { width: number; height: number } {
    if (!this.shellEl && typeof document !== 'undefined') {
      this.shellEl = document.querySelector(
        this.shellSelector
      ) as HTMLElement | null;
    }

    if (this.shellEl) {
      const rect = this.shellEl.getBoundingClientRect();
      const rectW = Math.floor(rect.width || this.shellEl.clientWidth || 0);
      const rectH = Math.floor(rect.height || this.shellEl.clientHeight || 0);
      if (rectW > 0 && rectH > 0) {
        return { width: rectW, height: rectH };
      }
    }

    if (typeof window !== 'undefined') {
      const vw = Math.floor(
        window.visualViewport?.width || window.innerWidth || 1280
      );
      const vh = Math.floor(
        window.visualViewport?.height || window.innerHeight || 720
      );
      return { width: vw, height: vh };
    }

    return { width: 1280, height: 720 };
  }

  private applyMetricsToDom(metrics: ViewportMetrics): void {
    if (!this.canvas && typeof document !== 'undefined') {
      this.canvas = document.getElementById(
        'pixel-canvas'
      ) as HTMLCanvasElement | null;
    }

    if (this.canvas) {
      const resizedBuffer =
        this.canvas.width !== metrics.logicalWidth ||
        this.canvas.height !== metrics.logicalHeight;

      this.canvas.width = metrics.logicalWidth;
      this.canvas.height = metrics.logicalHeight;
      this.canvas.style.width = `${metrics.logicalWidth * metrics.pixelScale}px`;
      this.canvas.style.height = `${metrics.logicalHeight * metrics.pixelScale}px`;

      if (resizedBuffer) {
        try {
          const ctx = this.canvas.getContext('2d');
          if (ctx) {
            ctx.imageSmoothingEnabled = false;
          }
        } catch {
          // Ignore under headless canvas mocks without 2d context
        }
      }
    }

    if (typeof document !== 'undefined') {
      document.documentElement?.style.setProperty(
        '--pixel-scale',
        `${metrics.pixelScale}`
      );
      document.body?.classList.toggle(
        'is-touch-controls',
        metrics.isTouchDevice || metrics.isMobileLandscape
      );
    }
  }

  private hasMetricsChanged(
    prev: ViewportMetrics,
    next: ViewportMetrics
  ): boolean {
    return (
      prev.cssWidth !== next.cssWidth ||
      prev.cssHeight !== next.cssHeight ||
      prev.pixelScale !== next.pixelScale ||
      prev.logicalWidth !== next.logicalWidth ||
      prev.logicalHeight !== next.logicalHeight ||
      prev.zoomPreset !== next.zoomPreset ||
      prev.isTouchDevice !== next.isTouchDevice ||
      prev.isMobileLandscape !== next.isMobileLandscape
    );
  }

  public refresh(forceNotify = false): ViewportMetrics {
    const { width, height } = this.readCssSize();
    const nextMetrics = GameShell.computeViewportMetrics(
      width,
      height,
      this.zoomPreset,
      this.detectTouchDevice()
    );

    const changed = this.hasMetricsChanged(this.metrics, nextMetrics);
    this.metrics = nextMetrics;
    this.applyMetricsToDom(nextMetrics);

    if (changed || forceNotify) {
      this.onViewportChange?.(this.getMetrics());
    }

    return this.getMetrics();
  }

  public setZoomPreset(zoom: CameraZoomPreset): ViewportMetrics {
    this.zoomPreset = zoom;
    return this.refresh(true);
  }

  public getZoomPreset(): CameraZoomPreset {
    return this.zoomPreset;
  }

  public getMetrics(): ViewportMetrics {
    return { ...this.metrics };
  }

  /**
   * Chuyển đổi tọa độ thế giới `(worldX, worldY)` sang tọa độ điểm ảnh CSS tương đối với `#game-shell`,
   * có bù trừ độ lệch căn giữa 1–3px giữa `cssWidth` và `logicalWidth * pixelScale`.
   */
  public worldToScreen(
    worldX: number,
    worldY: number,
    cameraX: number,
    cameraY: number
  ): Vec2 {
    const { cssWidth, cssHeight, logicalWidth, logicalHeight, pixelScale } =
      this.metrics;
    const canvasCssW = logicalWidth * pixelScale;
    const canvasCssH = logicalHeight * pixelScale;
    const offsetX = (cssWidth - canvasCssW) * 0.5;
    const offsetY = (cssHeight - canvasCssH) * 0.5;

    return {
      x: Math.round(offsetX + (worldX - cameraX) * pixelScale),
      y: Math.round(offsetY + (worldY - cameraY) * pixelScale),
    };
  }

  public destroy(): void {
    if (typeof window !== 'undefined') {
      window.removeEventListener('resize', this.boundHandleViewportEvent);
      window.removeEventListener(
        'orientationchange',
        this.boundHandleViewportEvent
      );
      window.visualViewport?.removeEventListener(
        'resize',
        this.boundHandleViewportEvent
      );
    }

    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
      this.resizeObserver = null;
    }
  }
}
