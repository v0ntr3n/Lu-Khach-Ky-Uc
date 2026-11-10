/**
 * @file tests/unit/m7_fullscreen_presentation.test.ts
 * @description Bộ kiểm thử đơn vị hồi quy (Vitest + jsdom) cho Cột mốc M7 — Fullscreen RPG Presentation:
 * 1. GameShell.computeViewportMetrics & worldToScreen trên 1366×768, 1920×1080, 2560×1440 và Mobile Landscape 844×390
 *    qua 3 mức zoom ('wide', 'normal', 'close')
 * 2. CameraController (setViewport, kẹp biên không để lộ void trên bản đồ lớn, căn giữa âm khi bản đồ nhỏ hơn viewport)
 * 3. SaveManager lưu độc lập SETTINGS_KEY_V1 không tạo bản lưu giả SAVE_KEY_V2, hasSave() & getSaveSummary() chính xác
 * 4. GameStore chuyển đổi AppLifecycleState (BOOT -> MAIN_MENU -> LOADING -> OPENING -> PLAYING -> PAUSED -> MAIN_MENU)
 *    và isWorldInputAllowed() chỉ bật khi lifecycle === 'PLAYING' && mode === 'exploring'
 * 5. FullscreenManager (request, exit, toggle, mock Native Fullscreen + fallback an toàn body.is-viewport-fullscreen)
 * 6. OpeningFractureController tái sử dụng DOM node (#rift-opening-overlay), nhịp 'full' (4.8s) vs 'continue' (1.8s), và cancel()
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GameStore } from '../../src/core/GameStore';
import {
  DEFAULT_GAME_SETTINGS,
  SAVE_KEY_V2,
  SETTINGS_KEY_V1,
  SaveManager,
} from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import { OpeningFractureController } from '../../src/engine/OpeningFracture';
import { CHARACTER_SPRITE_HEIGHT } from '../../src/engine/SpriteAtlas';
import { CameraController } from '../../src/runtime/CameraController';
import type { CameraZoomPreset, GameMode, SaveDataV2 } from '../../src/types/game';
import { FullscreenManager } from '../../src/ui/FullscreenManager';
import { GameShell } from '../../src/ui/GameShell';

function createMockCanvas2DContext(): CanvasRenderingContext2D {
  return {
    fillStyle: '#000000',
    strokeStyle: '#000000',
    lineWidth: 1,
    globalAlpha: 1,
    globalCompositeOperation: 'source-over',
    imageSmoothingEnabled: false,
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
    scale: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

describe('M7 Fullscreen RPG Presentation — Unit Regression Suite', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    document.body.className = '';
    document.documentElement.className = '';
    localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('1. GameShell.computeViewportMetrics & worldToScreen projection', () => {
    const resolutions = [
      { label: '1366×768 Desktop', w: 1366, h: 768, mobile: false },
      { label: '1920×1080 Full HD', w: 1920, h: 1080, mobile: false },
      { label: '2560×1440 QHD', w: 2560, h: 1440, mobile: false },
      { label: '844×390 Mobile Landscape', w: 844, h: 390, mobile: true },
    ] as const;

    const zoomPresets: readonly CameraZoomPreset[] = ['wide', 'normal', 'close'];

    it('computes integer pixelScale >= 2, full viewport coverage with zero aspect distortion, and accurate mobile detection across all resolutions and zoom presets', () => {
      for (const res of resolutions) {
        for (const zoom of zoomPresets) {
          const metrics = GameShell.computeViewportMetrics(res.w, res.h, zoom, res.mobile);

          expect(metrics.cssWidth).toBe(res.w);
          expect(metrics.cssHeight).toBe(res.h);
          expect(metrics.zoomPreset).toBe(zoom);
          expect(Number.isInteger(metrics.pixelScale)).toBe(true);
          expect(metrics.pixelScale).toBeGreaterThanOrEqual(2);

          // Covers 100% of CSS viewport with uniform X/Y integer scale (zero aspect distortion)
          const canvasCssW = metrics.logicalWidth * metrics.pixelScale;
          const canvasCssH = metrics.logicalHeight * metrics.pixelScale;
          expect(canvasCssW).toBeGreaterThanOrEqual(res.w);
          expect(canvasCssH).toBeGreaterThanOrEqual(res.h);
          expect(canvasCssW / metrics.logicalWidth).toBe(canvasCssH / metrics.logicalHeight);

          expect(metrics.isMobileLandscape).toBe(res.mobile);
        }
      }
    });

    it('guarantees pixelScale === 4 and 88 CSS px character height (within 64–96 CSS px) at 1920×1080 normal zoom, plus 3× wide and 5× close', () => {
      const wide = GameShell.computeViewportMetrics(1920, 1080, 'wide', false);
      const normal = GameShell.computeViewportMetrics(1920, 1080, 'normal', false);
      const close = GameShell.computeViewportMetrics(1920, 1080, 'close', false);

      expect(wide.pixelScale).toBe(3);
      expect(normal.pixelScale).toBe(4);
      expect(close.pixelScale).toBe(5);

      expect(CHARACTER_SPRITE_HEIGHT).toBe(22);
      const charCssHeight = CHARACTER_SPRITE_HEIGHT * normal.pixelScale;
      expect(charCssHeight).toBe(88);
      expect(charCssHeight).toBeGreaterThanOrEqual(64);
      expect(charCssHeight).toBeLessThanOrEqual(96);
    });

    it('projects world coordinates to CSS screen coordinates accurately including sub-tile centering offset', () => {
      document.body.innerHTML = `
        <div id="game-shell">
          <canvas id="pixel-canvas"></canvas>
        </div>
      `;
      const shellEl = document.getElementById('game-shell')!;
      const canvasEl = document.getElementById('pixel-canvas') as HTMLCanvasElement;
      vi.spyOn(canvasEl, 'getContext').mockReturnValue(createMockCanvas2DContext());

      // Case A: 1920×1080 normal -> pixelScale=4, logical=480×270 -> exact fit (offset 0, 0)
      vi.spyOn(shellEl, 'getBoundingClientRect').mockReturnValue({
        width: 1920,
        height: 1080,
        top: 0,
        left: 0,
        right: 1920,
        bottom: 1080,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });

      const shell = new GameShell({ shellSelector: '#game-shell', canvas: canvasEl });
      expect(shell.getMetrics().pixelScale).toBe(4);
      expect(shell.worldToScreen(256, 384, 16, 249)).toEqual({
        x: (256 - 16) * 4,
        y: (384 - 249) * 4,
      });

      // Case B: 1366×768 normal -> pixelScale=3, logicalWidth=ceil(1366/3)=456 (456*3=1368 -> offsetX=-1)
      vi.spyOn(shellEl, 'getBoundingClientRect').mockReturnValue({
        width: 1366,
        height: 768,
        top: 0,
        left: 0,
        right: 1366,
        bottom: 768,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
      shell.refresh(true);

      const m1366 = shell.getMetrics();
      expect(m1366.pixelScale).toBe(3);
      expect(m1366.logicalWidth).toBe(456);
      expect(m1366.logicalHeight).toBe(256);
      // offsetX = (1366 - 1368) * 0.5 = -1; offsetY = (768 - 768) * 0.5 = 0
      expect(shell.worldToScreen(100, 200, 50, 120)).toEqual({
        x: Math.round(-1 + (100 - 50) * 3),
        y: Math.round(0 + (200 - 120) * 3),
      });

      shell.destroy();
    });
  });

  describe('2. CameraController dynamic viewport clamping & small-map centering', () => {
    it('clamps strictly within [0, worldSize - viewSize] on large maps so zero void appears outside map bounds', () => {
      const cam = new CameraController({
        worldWidth: 1024,
        worldHeight: 768,
        viewportWidth: 320,
        viewportHeight: 180,
      });

      cam.setViewport(480, 270);
      expect(cam.viewportWidth).toBe(480);
      expect(cam.viewportHeight).toBe(270);

      // Far top-left corner
      expect(cam.snapTo(0, 0)).toEqual({ x: 0, y: 0 });
      expect(cam.clamp(-120, -80)).toEqual({ x: 0, y: 0 });

      // Far bottom-right corner
      expect(cam.snapTo(2000, 2000)).toEqual({
        x: 1024 - 480,
        y: 768 - 270,
      });
      expect(cam.x + cam.viewportWidth).toBeLessThanOrEqual(1024);
      expect(cam.y + cam.viewportHeight).toBeLessThanOrEqual(768);
    });

    it('applies negative centering offset (mapSize - viewSize)/2 when an interior map is smaller than logical viewport', () => {
      // safehouse_interior is 512×384; on a wide QHD logical viewport (640×480), map is smaller than viewport
      const cam = new CameraController({
        worldWidth: 512,
        worldHeight: 384,
        viewportWidth: 480,
        viewportHeight: 270,
      });

      cam.setViewport(640, 480);
      expect(cam.x).toBe(Math.round((512 - 640) * 0.5)); // -64
      expect(cam.y).toBe(Math.round((384 - 480) * 0.5)); // -48

      // Moving target anywhere inside the small interior keeps the map centered
      const afterSnap = cam.snapTo(420, 310);
      expect(afterSnap).toEqual({ x: -64, y: -48 });
      const afterUpdate = cam.update(90, 80, 1 / 60);
      expect(afterUpdate).toEqual({ x: -64, y: -48 });
    });
  });

  describe('3. SaveManager standalone SETTINGS_KEY_V1 persistence & getSaveSummary accuracy', () => {
    it('persists SETTINGS_KEY_V1 without creating a fake SAVE_KEY_V2 save, keeping hasSave() false and getSaveSummary() null', () => {
      const saveManager = new SaveManager();
      expect(saveManager.hasSave()).toBe(false);
      expect(saveManager.getSaveSummary(CHAPTERS)).toBeNull();

      saveManager.saveSettings({
        ...DEFAULT_GAME_SETTINGS,
        muted: true,
        masterVolume: 0.42,
        cameraZoom: 'close',
        reducedMotion: true,
        textScale: 'large',
      });

      expect(localStorage.getItem(SETTINGS_KEY_V1)).not.toBeNull();
      expect(localStorage.getItem(SAVE_KEY_V2)).toBeNull();
      expect(saveManager.hasSave()).toBe(false);
      expect(saveManager.getSaveSummary(CHAPTERS)).toBeNull();

      const loadedSettings = saveManager.loadSettings();
      expect(loadedSettings.muted).toBe(true);
      expect(loadedSettings.masterVolume).toBe(0.42);
      expect(loadedSettings.cameraZoom).toBe('close');
      expect(loadedSettings.reducedMotion).toBe(true);
      expect(loadedSettings.textScale).toBe('large');
    });

    it('returns accurate chapter, map, milestone, and evidence counts after save(), and preserves settings on clearSave()', () => {
      const saveManager = new SaveManager();
      saveManager.saveSettings({
        ...DEFAULT_GAME_SETTINGS,
        cameraZoom: 'wide',
      });

      const realSave: SaveDataV2 = {
        version: 2,
        timestamp: 1762750000000,
        eraId: '1945',
        currentMapId: 'tan_trao_banyan',
        spawnId: 'default_spawn',
        playerPos: { x: 180, y: 260 },
        playerFacing: 'down',
        collectedMilestones: ['I', 'II', 'III', 'IV', 'IV'],
        collectedEvidence: ['ev_ch1_shipping_ledger', 'ev_ch1_cipher_note', 'ev_ch1_cipher_note'],
        questFlags: ['ch1_met_liaison'],
        discoveredMaps: ['harbor_square', 'tan_trao_banyan'],
        unlockedCheckpoints: ['default_spawn'],
        completedChapters: ['1930'],
        activeQuestIds: [],
        completedQuestIds: ['q_ch1_main_unification'],
        completedObjectiveIds: ['obj_ch1_meet_liaison'],
        dialogueHistory: [],
        settings: { ...DEFAULT_GAME_SETTINGS, cameraZoom: 'wide' },
      };

      saveManager.save(realSave);

      expect(saveManager.hasSave()).toBe(true);
      const summary = saveManager.getSaveSummary(CHAPTERS, {
        tan_trao_banyan: 'Cây Đa Tân Trào (08/1945)',
      });
      expect(summary).not.toBeNull();
      expect(summary?.exists).toBe(true);
      expect(summary?.eraId).toBe('1945');
      expect(summary?.chapterLabel).toContain('CHƯƠNG II');
      expect(summary?.mapId).toBe('tan_trao_banyan');
      expect(summary?.mapTitle).toBe('Cây Đa Tân Trào (08/1945)');
      expect(summary?.milestoneCount).toBe(4);
      expect(summary?.evidenceCount).toBe(2);

      // Clearing game save does not wipe standalone settings
      saveManager.clearSave();
      expect(saveManager.hasSave()).toBe(false);
      expect(saveManager.getSaveSummary(CHAPTERS)).toBeNull();
      expect(saveManager.loadSettings().cameraZoom).toBe('wide');
    });
  });

  describe('4. GameStore AppLifecycleState transitions & isWorldInputAllowed gating', () => {
    it('transitions cleanly through BOOT -> MAIN_MENU -> LOADING -> OPENING -> PLAYING -> PAUSED -> MAIN_MENU and gates world input strictly', () => {
      const store = new GameStore();

      store.setMode('boot');
      expect(store.getLifecycle()).toBe('BOOT');
      expect(store.isWorldInputAllowed()).toBe(false);

      store.setMode('main_menu');
      expect(store.getLifecycle()).toBe('MAIN_MENU');
      expect(store.isWorldInputAllowed()).toBe(false);

      store.setMode('loading');
      expect(store.getLifecycle()).toBe('LOADING');
      expect(store.isWorldInputAllowed()).toBe(false);

      store.setMode('opening');
      expect(store.getLifecycle()).toBe('OPENING');
      expect(store.isWorldInputAllowed()).toBe(false);

      store.setMode('exploring');
      expect(store.getLifecycle()).toBe('PLAYING');
      expect(store.isWorldInputAllowed()).toBe(true);

      // Every non-exploring overlay mode blocks world input
      const blockedOverlayModes: readonly GameMode[] = [
        'dialogue',
        'journal',
        'inventory',
        'map',
        'lookup',
        'puzzle',
        'climax',
        'presenter',
        'settings',
      ];
      for (const overlayMode of blockedOverlayModes) {
        store.setMode(overlayMode);
        expect(store.isWorldInputAllowed()).toBe(false);
      }

      store.setMode('exploring');
      expect(store.isWorldInputAllowed()).toBe(true);

      store.setMode('paused');
      expect(store.getLifecycle()).toBe('PAUSED');
      expect(store.isWorldInputAllowed()).toBe(false);

      // Even if lifecycle is explicitly transitioned away from PLAYING while mode was 'exploring', world input stays blocked
      store.setMode('exploring');
      store.transitionLifecycle('PAUSED');
      expect(store.getLifecycle()).toBe('PAUSED');
      expect(store.isWorldInputAllowed()).toBe(false);

      store.setMode('main_menu');
      expect(store.getLifecycle()).toBe('MAIN_MENU');
      expect(store.isWorldInputAllowed()).toBe(false);
    });
  });

  describe('5. FullscreenManager native API mock & graceful viewport fallback', () => {
    it('supports native requestFullscreen / exitFullscreen / toggle when browser allows fullscreen', async () => {
      const shellEl = document.createElement('div');
      shellEl.id = 'game-shell';
      document.body.appendChild(shellEl);

      let currentFsEl: Element | null = null;
      Object.defineProperty(document, 'fullscreenEnabled', {
        configurable: true,
        get: () => true,
      });
      Object.defineProperty(document, 'fullscreenElement', {
        configurable: true,
        get: () => currentFsEl,
      });

      shellEl.requestFullscreen = vi.fn(async () => {
        currentFsEl = shellEl;
        document.dispatchEvent(new Event('fullscreenchange'));
      });
      document.exitFullscreen = vi.fn(async () => {
        currentFsEl = null;
        document.dispatchEvent(new Event('fullscreenchange'));
      });

      const states: boolean[] = [];
      const fsManager = new FullscreenManager(shellEl, (st) => {
        states.push(st.isBrowserFullscreen || st.isViewportFallback);
      });

      expect(fsManager.isFullscreenActive()).toBe(false);

      const entered = await fsManager.request();
      expect(entered).toBe(true);
      expect(fsManager.isFullscreenActive()).toBe(true);
      expect(fsManager.getState().isBrowserFullscreen).toBe(true);
      expect(fsManager.getState().isViewportFallback).toBe(false);
      expect(document.body.classList.contains('is-browser-fullscreen')).toBe(true);

      const afterToggleExit = await fsManager.toggle();
      expect(afterToggleExit).toBe(false);
      expect(fsManager.isFullscreenActive()).toBe(false);
      expect(document.body.classList.contains('is-browser-fullscreen')).toBe(false);
      expect(states.length).toBeGreaterThanOrEqual(2);

      fsManager.destroy();
    });

    it('falls back gracefully to body.is-viewport-fullscreen without throwing when requestFullscreen rejects or is unsupported', async () => {
      const shellEl = document.createElement('div');
      shellEl.id = 'game-shell';
      document.body.appendChild(shellEl);

      Object.defineProperty(document, 'fullscreenEnabled', {
        configurable: true,
        get: () => true,
      });
      Object.defineProperty(document, 'fullscreenElement', {
        configurable: true,
        get: () => null,
      });

      shellEl.requestFullscreen = vi.fn(async () => {
        throw new Error('Permissions check failed');
      });

      const fsManager = new FullscreenManager(shellEl);
      const activated = await fsManager.toggle();
      expect(activated).toBe(true);
      expect(fsManager.isFullscreenActive()).toBe(true);
      expect(fsManager.getState().isBrowserFullscreen).toBe(false);
      expect(fsManager.getState().isViewportFallback).toBe(true);
      expect(document.body.classList.contains('is-viewport-fullscreen')).toBe(true);

      const exited = await fsManager.exit();
      expect(exited).toBe(false);
      expect(fsManager.isFullscreenActive()).toBe(false);
      expect(document.body.classList.contains('is-viewport-fullscreen')).toBe(false);

      // Also verify unsupported environment (no requestFullscreen method)
      delete (shellEl as Partial<HTMLElement>).requestFullscreen;
      const fallbackUnsupported = await fsManager.request();
      expect(fallbackUnsupported).toBe(true);
      expect(document.body.classList.contains('is-viewport-fullscreen')).toBe(true);

      await fsManager.toggle();
      expect(fsManager.isFullscreenActive()).toBe(false);
      fsManager.destroy();
    });
  });

  describe('6. OpeningFractureController reusable DOM lifecycle, full vs continue pacing & cancel()', () => {
    it('hides #rift-opening-overlay instead of removing DOM node by default, supports full (4.8s) vs continue (1.8s) modes, and cancel() halts without firing onComplete', () => {
      document.body.innerHTML = `
        <div id="rift-opening-overlay" hidden>
          <canvas id="rift-opening-canvas"></canvas>
          <button id="skip-opening-btn" type="button">Skip</button>
        </div>
      `;

      const overlayEl = document.getElementById('rift-opening-overlay')!;
      const canvasEl = document.getElementById('rift-opening-canvas') as HTMLCanvasElement;
      vi.spyOn(canvasEl, 'getContext').mockReturnValue(createMockCanvas2DContext());

      let fakeNowMs = 1000;
      vi.spyOn(performance, 'now').mockImplementation(() => fakeNowMs);

      let pendingRaf: FrameRequestCallback | null = null;
      vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) => {
        pendingRaf = cb;
        return 101;
      });
      vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {
        pendingRaf = null;
      });

      const controller = new OpeningFractureController(
        '#rift-opening-overlay',
        '#rift-opening-canvas'
      );

      // 1) mode: 'full' (4.8s)
      const onFullComplete = vi.fn();
      controller.start({ mode: 'full', reducedMotion: false, onComplete: onFullComplete });
      expect(overlayEl.hidden).toBe(false);
      expect(overlayEl.dataset.mode).toBe('full');
      expect(controller.getState()).toBe('initializing');

      // At 2.0s elapsed -> still running ('propagating'), not completed
      fakeNowMs = 3000;
      pendingRaf?.(fakeNowMs);
      expect(controller.getState()).toBe('propagating');
      expect(onFullComplete).not.toHaveBeenCalled();

      // At 4.85s elapsed -> finishes, hides overlay WITHOUT removing it from DOM
      fakeNowMs = 5850;
      pendingRaf?.(fakeNowMs);
      expect(onFullComplete).toHaveBeenCalledTimes(1);
      expect(overlayEl.isConnected).toBe(true);
      expect(overlayEl.hidden).toBe(true);
      expect(overlayEl.style.display).toBe('none');

      // 2) Reuse the exact same DOM node with mode: 'continue' (1.8s)
      const onContinueComplete = vi.fn();
      fakeNowMs = 10000;
      controller.start({
        mode: 'continue',
        reducedMotion: false,
        onComplete: onContinueComplete,
      });
      expect(overlayEl.hidden).toBe(false);
      expect(overlayEl.dataset.mode).toBe('continue');
      expect(controller.getState()).toBe('cracking');

      // At 1.85s elapsed -> completes in continue mode (< 4.8s)
      fakeNowMs = 11850;
      pendingRaf?.(fakeNowMs);
      expect(onContinueComplete).toHaveBeenCalledTimes(1);
      expect(overlayEl.isConnected).toBe(true);
      expect(overlayEl.hidden).toBe(true);

      // 3) cancel() stops active animation and hides overlay WITHOUT calling onComplete
      const onCancelledComplete = vi.fn();
      fakeNowMs = 20000;
      controller.start({
        mode: 'full',
        reducedMotion: false,
        onComplete: onCancelledComplete,
      });
      expect(controller.isRunning()).toBe(true);
      controller.cancel();
      expect(controller.isRunning()).toBe(false);
      expect(overlayEl.hidden).toBe(true);
      expect(onCancelledComplete).not.toHaveBeenCalled();
    });
  });
});
