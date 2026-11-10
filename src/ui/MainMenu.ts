/**
 * @file src/ui/MainMenu.ts
 * @description Màn hình Menu Chính điện ảnh (#main-menu-screen) của "Lữ Khách Ký Ức (1930 – 2045)".
 * Vẽ khung cảnh Pixel-Art Đêm Mưa Bến Cảng Cửu Long (Hương Cảng 1930) trên #main-menu-bg-canvas
 * theo đúng bảng 12 sắc độ Sơn Mài & Giấy Dó, tự động dừng vòng lặp RAF khi ẩn (0% CPU khi chơi),
 * hỗ trợ điều hướng bàn phím đầy đủ và quản lý các hộp thoại Hướng dẫn / Multiplayer / Xác nhận.
 */

import type { SaveSummary } from '../types/game';

export interface MainMenuCallbacks {
  onStartNewGame: (confirmedOverwrite: boolean) => void;
  onContinueGame: () => void;
  onOpenSettings: () => void;
  onPlayUiSound?: (kind: 'focus' | 'confirm' | 'cancel') => void;
  onToggleFullscreen?: () => void;
}

interface RainDrop {
  x: number;
  y: number;
  speed: number;
  len: number;
}

interface WaterRipple {
  x: number;
  y: number;
  phase: number;
}

interface ShophouseBuilding {
  x: number;
  w: number;
  roofY: number;
  stories: number;
  hasSign: boolean;
  windowSeed: number;
}

const BUFFER_W = 426;
const BUFFER_H = 240;
const WATER_Y = 154;

const PALETTE = {
  lacquerVoid: '#110E0B',
  lacquerDeep: '#1B1612',
  lacquerElevated: '#241D17',
  inkPrimary: '#232116',
  inkSecondary: '#3E3A2C',
  bronzeMuted: '#6E4720',
  bronzeAntique: '#8A5A2B',
  bronzeBright: '#D4A55D',
  crimsonSeal: '#9E3321',
  crimsonVermilion: '#DF6B57',
  parchmentWarm: '#E6E5DC',
  parchmentLight: '#F4EFE4',
} as const;

const MENU_BUTTON_IDS = [
  'btn-menu-start',
  'btn-menu-continue',
  'btn-menu-multiplayer',
  'btn-menu-instructions',
  'btn-menu-settings',
] as const;

export class MainMenu {
  private callbacks: MainMenuCallbacks;
  private saveSummary: SaveSummary | null = null;
  private reducedMotion = false;
  private focusedBtnIndex = 0;
  private rafId: number | null = null;
  private startPerfMs = 0;
  private subModalTriggerEl: HTMLElement | null = null;
  private confirmCallback: (() => void) | null = null;
  private isDestroyed = false;

  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;

  private readonly rainDrops: RainDrop[] = [];
  private readonly ripples: WaterRipple[] = [];
  private readonly buildings: ShophouseBuilding[] = [
    { x: -18, w: 54, roofY: 86, stories: 3, hasSign: false, windowSeed: 1 },
    { x: 38, w: 48, roofY: 96, stories: 2, hasSign: true, windowSeed: 3 },
    { x: 88, w: 62, roofY: 78, stories: 3, hasSign: false, windowSeed: 5 },
    { x: 152, w: 46, roofY: 92, stories: 2, hasSign: true, windowSeed: 7 },
    { x: 200, w: 58, roofY: 84, stories: 3, hasSign: false, windowSeed: 11 },
    { x: 262, w: 44, roofY: 100, stories: 2, hasSign: false, windowSeed: 13 },
    { x: 358, w: 52, roofY: 88, stories: 3, hasSign: true, windowSeed: 17 },
    { x: 412, w: 46, roofY: 96, stories: 2, hasSign: false, windowSeed: 19 },
  ];

  constructor(callbacks: MainMenuCallbacks) {
    this.callbacks = callbacks;
    this.initDeterministicParticles();
    this.initCanvas();
    this.bindEvents();
    this.updateSaveSummary(null);
  }

  private initDeterministicParticles(): void {
    let seed = 19300203;
    const nextRand = (): number => {
      seed = (seed * 1664525 + 1013904223) >>> 0;
      return seed / 4294967296;
    };

    for (let i = 0; i < 64; i++) {
      this.rainDrops.push({
        x: Math.floor(nextRand() * (BUFFER_W + 60)) - 30,
        y: Math.floor(nextRand() * BUFFER_H),
        speed: 135 + nextRand() * 65,
        len: 5 + Math.floor(nextRand() * 5),
      });
    }

    for (let i = 0; i < 18; i++) {
      this.ripples.push({
        x: 24 + Math.floor(nextRand() * (BUFFER_W - 48)),
        y: WATER_Y + 8 + Math.floor(nextRand() * (BUFFER_H - WATER_Y - 16)),
        phase: nextRand() * Math.PI * 2,
      });
    }
  }

  private initCanvas(): void {
    if (typeof document === 'undefined') return;
    this.canvas = document.getElementById('main-menu-bg-canvas') as HTMLCanvasElement | null;
    if (!this.canvas) return;

    this.canvas.width = BUFFER_W;
    this.canvas.height = BUFFER_H;
    try {
      this.ctx = this.canvas.getContext('2d');
      if (this.ctx) {
        this.ctx.imageSmoothingEnabled = false;
      }
    } catch {
      this.ctx = null;
    }
  }

  private getMenuButtons(): HTMLButtonElement[] {
    if (typeof document === 'undefined') return [];
    const list: HTMLButtonElement[] = [];
    for (const id of MENU_BUTTON_IDS) {
      const btn = document.getElementById(id) as HTMLButtonElement | null;
      if (btn) list.push(btn);
    }
    return list;
  }

  private getEnabledMenuButtons(): HTMLButtonElement[] {
    return this.getMenuButtons().filter((btn) => !btn.disabled);
  }

  private focusMenuButton(btn: HTMLButtonElement, playSound = false): void {
    const all = this.getMenuButtons();
    const idx = all.indexOf(btn);
    if (idx !== -1) {
      this.focusedBtnIndex = idx;
    }
    for (const item of all) {
      const active = item === btn;
      item.classList.toggle('is-focused', active);
      item.tabIndex = active ? 0 : -1;
    }
    btn.focus({ preventScroll: true });
    if (playSound) {
      this.callbacks.onPlayUiSound?.('focus');
    }
  }

  private moveFocusByDelta(delta: 1 | -1): void {
    const all = this.getMenuButtons();
    if (all.length === 0) return;

    let idx = this.focusedBtnIndex;
    for (let step = 0; step < all.length; step++) {
      idx = (idx + delta + all.length) % all.length;
      const candidate = all[idx];
      if (candidate && !candidate.disabled) {
        this.focusMenuButton(candidate, true);
        return;
      }
    }
  }

  private bindEvents(): void {
    if (typeof document === 'undefined' || typeof window === 'undefined') return;

    const buttons = this.getMenuButtons();
    buttons.forEach((btn, idx) => {
      btn.addEventListener('mouseenter', () => {
        if (btn.disabled) return;
        if (this.focusedBtnIndex !== idx) {
          this.focusMenuButton(btn, true);
        }
      });
      btn.addEventListener('focus', () => {
        if (btn.disabled) return;
        this.focusedBtnIndex = idx;
        buttons.forEach((b) => b.classList.toggle('is-focused', b === btn));
      });
    });

    const startBtn = document.getElementById('btn-menu-start') as HTMLButtonElement | null;
    startBtn?.addEventListener('click', () => {
      if (this.saveSummary?.exists) {
        this.callbacks.onPlayUiSound?.('focus');
        const detail = this.saveSummary.chapterLabel
          ? `${this.saveSummary.chapterLabel} — ${this.saveSummary.mapTitle} (${this.saveSummary.milestoneCount}/12 cột mốc)`
          : 'tiến trình đã lưu hiện tại';
        this.openConfirmDialog(
          'KHỞI ĐẦU HÀNH TRÌNH MỚI?',
          `Hệ thống phát hiện hồ sơ lưu trữ: ${detail}. Khởi đầu mới sẽ ghi đè dữ liệu này. Bạn có muốn tiếp tục?`,
          'Ghi đè & Bắt đầu',
          () => {
            this.callbacks.onPlayUiSound?.('confirm');
            this.callbacks.onStartNewGame(true);
          }
        );
        return;
      }
      this.callbacks.onPlayUiSound?.('confirm');
      this.callbacks.onStartNewGame(false);
    });

    const continueBtn = document.getElementById(
      'btn-menu-continue'
    ) as HTMLButtonElement | null;
    continueBtn?.addEventListener('click', () => {
      if (!this.saveSummary?.exists) return;
      this.callbacks.onPlayUiSound?.('confirm');
      this.callbacks.onContinueGame();
    });

    const mpBtn = document.getElementById(
      'btn-menu-multiplayer'
    ) as HTMLButtonElement | null;
    mpBtn?.addEventListener('click', () => {
      this.callbacks.onPlayUiSound?.('confirm');
      this.openSimpleSubModal('multiplayer-modal', mpBtn);
    });

    const instrBtn = document.getElementById(
      'btn-menu-instructions'
    ) as HTMLButtonElement | null;
    instrBtn?.addEventListener('click', () => {
      this.callbacks.onPlayUiSound?.('confirm');
      this.openSimpleSubModal('instructions-modal', instrBtn);
    });

    const settingsBtn = document.getElementById(
      'btn-menu-settings'
    ) as HTMLButtonElement | null;
    settingsBtn?.addEventListener('click', () => {
      this.callbacks.onPlayUiSound?.('confirm');
      this.callbacks.onOpenSettings();
    });

    const closeInstrBtn = document.getElementById('btn-close-instructions');
    if (closeInstrBtn) {
      closeInstrBtn.dataset.boundByMainMenu = 'true';
      closeInstrBtn.addEventListener('click', () => {
        this.callbacks.onPlayUiSound?.('cancel');
        this.closeSubModals();
      });
    }

    const closeMpBtn = document.getElementById('btn-close-multiplayer');
    if (closeMpBtn) {
      closeMpBtn.dataset.boundByMainMenu = 'true';
      closeMpBtn.addEventListener('click', () => {
        this.callbacks.onPlayUiSound?.('cancel');
        this.closeSubModals();
      });
    }

    ['instructions-modal', 'multiplayer-modal', 'confirm-modal'].forEach((id) => {
      const modal = document.getElementById(id);
      modal?.addEventListener('click', (e) => {
        if (e.target === modal) {
          this.callbacks.onPlayUiSound?.('cancel');
          this.closeSubModals();
        }
      });
    });

    // Tab chuyển nội dung trong Hướng Dẫn (#instructions-modal)
    document.querySelectorAll<HTMLButtonElement>('[data-guide-tab]').forEach((tabBtn) => {
      tabBtn.addEventListener('click', () => {
        const targetTab = tabBtn.dataset.guideTab;
        if (!targetTab) return;
        this.callbacks.onPlayUiSound?.('focus');
        document.querySelectorAll<HTMLButtonElement>('[data-guide-tab]').forEach((b) => {
          const active = b.dataset.guideTab === targetTab;
          b.classList.toggle('is-active', active);
          b.setAttribute('aria-selected', String(active));
        });
        document.querySelectorAll<HTMLElement>('[data-guide-panel]').forEach((panel) => {
          const show = panel.dataset.guidePanel === targetTab;
          panel.hidden = !show;
          if (show) {
            panel.removeAttribute('hidden');
          } else {
            panel.setAttribute('hidden', '');
          }
        });
      });
    });

    window.addEventListener('keydown', this.handleKeyDown);
    document.addEventListener('visibilitychange', this.handleVisibilityChange);
  }

  private openSimpleSubModal(modalId: string, triggerEl?: HTMLElement | null): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById(modalId);
    if (!modal) return;

    this.subModalTriggerEl =
      triggerEl ?? (document.activeElement as HTMLElement | null);
    modal.classList.add('is-open');
    modal.hidden = false;
    modal.removeAttribute('hidden');

    const closeBtn = modal.querySelector<HTMLElement>(
      'button:not([disabled]), [tabindex="0"]'
    );
    closeBtn?.focus({ preventScroll: true });
  }

  private isAnySubModalOpen(): boolean {
    if (typeof document === 'undefined') return false;
    for (const id of ['confirm-modal', 'instructions-modal', 'multiplayer-modal']) {
      const el = document.getElementById(id);
      if (el && el.classList.contains('is-open') && !el.hidden) {
        return true;
      }
    }
    return false;
  }

  private isSettingsModalOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const settingsModal = document.getElementById('settings-modal');
    return Boolean(
      settingsModal && settingsModal.classList.contains('is-open') && !settingsModal.hidden
    );
  }

  private readonly handleKeyDown = (e: KeyboardEvent): void => {
    if (this.isDestroyed || !this.isOpen()) return;

    if (this.isSettingsModalOpen()) {
      if (e.key === 'Escape') {
        const closeSettingsBtn = document.getElementById(
          'btn-close-settings'
        ) as HTMLButtonElement | null;
        if (closeSettingsBtn) {
          e.preventDefault();
          e.stopPropagation();
          this.callbacks.onPlayUiSound?.('cancel');
          closeSettingsBtn.click();
        }
      }
      return;
    }

    if (this.isAnySubModalOpen()) {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        this.callbacks.onPlayUiSound?.('cancel');
        this.closeSubModals();
      }
      return;
    }

    const code = e.code;
    const key = e.key.toLowerCase();

    if (code === 'ArrowUp' || code === 'KeyW' || key === 'arrowup' || key === 'w') {
      e.preventDefault();
      this.moveFocusByDelta(-1);
      return;
    }

    if (code === 'ArrowDown' || code === 'KeyS' || key === 'arrowdown' || key === 's') {
      e.preventDefault();
      this.moveFocusByDelta(1);
      return;
    }

    if (key === 'enter' || key === ' ') {
      const buttons = this.getMenuButtons();
      const target = buttons[this.focusedBtnIndex];
      if (target && !target.disabled) {
        e.preventDefault();
        target.click();
      }
      return;
    }

    if (code === 'KeyF' || key === 'f') {
      e.preventDefault();
      this.callbacks.onToggleFullscreen?.();
    }
  };

  private readonly handleVisibilityChange = (): void => {
    if (typeof document === 'undefined') return;
    if (document.hidden) {
      this.stopLoop();
    } else if (this.isOpen()) {
      this.startOrRenderBackground();
    }
  };

  public openConfirmDialog(
    title: string,
    body: string,
    confirmLabel: string,
    onConfirm: () => void
  ): void {
    if (typeof document === 'undefined') {
      onConfirm();
      return;
    }

    const modal = document.getElementById('confirm-modal');
    const titleEl = document.getElementById('confirm-modal-title');
    const bodyEl = document.getElementById('confirm-modal-body');
    const yesBtn = document.getElementById('btn-confirm-yes') as HTMLButtonElement | null;
    const noBtn = document.getElementById('btn-confirm-no') as HTMLButtonElement | null;

    if (!modal || !yesBtn || !noBtn) {
      onConfirm();
      return;
    }

    this.subModalTriggerEl = document.activeElement as HTMLElement | null;
    this.confirmCallback = onConfirm;

    if (titleEl) titleEl.textContent = title;
    if (bodyEl) bodyEl.textContent = body;
    yesBtn.textContent = confirmLabel;

    yesBtn.onclick = () => {
      const cb = this.confirmCallback;
      this.confirmCallback = null;
      this.closeConfirmOnly();
      cb?.();
    };

    noBtn.onclick = () => {
      this.callbacks.onPlayUiSound?.('cancel');
      this.confirmCallback = null;
      this.closeConfirmOnly();
    };

    modal.classList.add('is-open');
    modal.hidden = false;
    modal.removeAttribute('hidden');
    noBtn.focus({ preventScroll: true });
  }

  private closeConfirmOnly(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('confirm-modal');
    if (modal) {
      modal.classList.remove('is-open');
      modal.hidden = true;
      modal.setAttribute('hidden', '');
    }
    this.restoreSubModalFocus();
  }

  private restoreSubModalFocus(): void {
    if (
      this.subModalTriggerEl &&
      this.subModalTriggerEl.isConnected &&
      !this.subModalTriggerEl.closest('[hidden]')
    ) {
      this.subModalTriggerEl.focus({ preventScroll: true });
    } else if (this.isOpen()) {
      const enabled = this.getEnabledMenuButtons();
      enabled[0]?.focus({ preventScroll: true });
    }
    this.subModalTriggerEl = null;
  }

  public closeSubModals(): boolean {
    if (typeof document === 'undefined') return false;
    let closedAny = false;

    for (const id of ['confirm-modal', 'instructions-modal', 'multiplayer-modal']) {
      const el = document.getElementById(id);
      if (el && (el.classList.contains('is-open') || !el.hidden)) {
        if (el.classList.contains('is-open') && !el.hidden) {
          closedAny = true;
        }
        el.classList.remove('is-open');
        el.hidden = true;
        el.setAttribute('hidden', '');
      }
    }

    this.confirmCallback = null;
    if (closedAny) {
      this.restoreSubModalFocus();
    }
    return closedAny;
  }

  public updateSaveSummary(saveSummary: SaveSummary | null): void {
    this.saveSummary = saveSummary;
    if (typeof document === 'undefined') return;

    const continueBtn = document.getElementById(
      'btn-menu-continue'
    ) as HTMLButtonElement | null;
    const summaryEl = document.getElementById('menu-save-summary');
    const hasSave = Boolean(saveSummary?.exists);

    if (continueBtn) {
      continueBtn.disabled = !hasSave;
      continueBtn.classList.toggle('is-disabled', !hasSave);
      continueBtn.setAttribute('aria-disabled', String(!hasSave));
    }

    if (summaryEl) {
      if (hasSave && saveSummary) {
        summaryEl.textContent = `${saveSummary.chapterLabel} · ${saveSummary.mapTitle} (${saveSummary.milestoneCount}/12 mốc)`;
      } else {
        summaryEl.textContent = 'Chưa có hồ sơ lưu';
      }
    }
  }

  public setReducedMotion(reduced: boolean): void {
    this.reducedMotion = reduced;
    if (!this.isOpen()) return;
    this.startOrRenderBackground();
  }

  public show(saveSummary: SaveSummary | null, reducedMotion?: boolean): void {
    if (typeof reducedMotion === 'boolean') {
      this.reducedMotion = reducedMotion;
    } else if (typeof document !== 'undefined') {
      this.reducedMotion = document.body.classList.contains('reduced-motion');
    }

    this.updateSaveSummary(saveSummary);

    if (typeof document !== 'undefined') {
      const screen = document.getElementById('main-menu-screen');
      if (screen) {
        screen.hidden = false;
        screen.removeAttribute('hidden');
        screen.classList.add('is-open');
      }

      if (!this.canvas || !this.ctx) {
        this.initCanvas();
      }

      const continueBtn = document.getElementById(
        'btn-menu-continue'
      ) as HTMLButtonElement | null;
      const startBtn = document.getElementById(
        'btn-menu-start'
      ) as HTMLButtonElement | null;
      const preferred =
        saveSummary?.exists && continueBtn && !continueBtn.disabled
          ? continueBtn
          : startBtn ?? this.getEnabledMenuButtons()[0] ?? null;

      if (preferred) {
        this.focusMenuButton(preferred, false);
      }
    }

    this.startOrRenderBackground();
  }

  public hide(): void {
    this.stopLoop();
    this.closeSubModals();

    if (typeof document === 'undefined') return;
    const screen = document.getElementById('main-menu-screen');
    if (screen) {
      screen.classList.remove('is-open');
      screen.hidden = true;
      screen.setAttribute('hidden', '');
    }
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const screen = document.getElementById('main-menu-screen');
    return Boolean(screen && !screen.hidden);
  }

  private stopLoop(): void {
    if (this.rafId !== null && typeof cancelAnimationFrame === 'function') {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
  }

  private startOrRenderBackground(): void {
    this.stopLoop();
    if (!this.ctx || !this.canvas) return;

    if (this.reducedMotion) {
      // Khung hình tĩnh thanh tịnh, không chạy vòng lặp chuyển động liên tục
      this.renderHarborScene(2.4, true);
      return;
    }

    this.startPerfMs =
      typeof performance !== 'undefined' ? performance.now() : Date.now();
    this.renderHarborScene(0, false);
    if (typeof requestAnimationFrame === 'function') {
      this.rafId = requestAnimationFrame(this.tick);
    }
  }

  private readonly tick = (nowMs: number): void => {
    if (this.isDestroyed || !this.isOpen() || this.reducedMotion) {
      this.rafId = null;
      return;
    }
    const elapsedSec = Math.max(0, (nowMs - this.startPerfMs) * 0.001);
    this.renderHarborScene(elapsedSec, false);
    this.rafId = requestAnimationFrame(this.tick);
  };

  /**
   * Vẽ cảnh Pixel-Art Đêm Mưa Bến Cảng Cửu Long (1930) bằng bảng màu 12 sắc độ Sơn Mài & Giấy Dó.
   */
  private renderHarborScene(timeSec: number, staticFrame: boolean): void {
    const ctx = this.ctx;
    if (!ctx) return;

    const panX = staticFrame
      ? 0
      : Math.sin((timeSec * Math.PI * 2) / 18) * 10;

    // 1. Nền trời đêm Sơn Mài phân dải sắc độ
    ctx.fillStyle = PALETTE.lacquerVoid;
    ctx.fillRect(0, 0, BUFFER_W, 56);
    ctx.fillStyle = PALETTE.lacquerDeep;
    ctx.fillRect(0, 56, BUFFER_W, 52);
    ctx.fillStyle = PALETTE.lacquerElevated;
    ctx.fillRect(0, 108, BUFFER_W, WATER_Y - 108);

    // Trăng lưỡi liềm cong mềm sau sương cảng (góc phải trên)
    const moonX = Math.round(338 - panX * 0.12);
    const moonY = 30;
    ctx.fillStyle = 'rgba(212, 165, 93, 0.07)';
    ctx.fillRect(moonX - 8, moonY - 3, 20, 18);
    ctx.fillRect(moonX - 5, moonY - 5, 14, 22);
    ctx.fillStyle = PALETTE.parchmentWarm;
    const crescentRows: ReadonlyArray<readonly [number, number]> = [
      [3, 4],
      [1, 4],
      [0, 3],
      [-1, 3],
      [-1, 2],
      [-1, 2],
      [-1, 3],
      [0, 3],
      [1, 4],
      [3, 5],
      [5, 4],
    ];
    crescentRows.forEach(([dx, w], rowIdx) => {
      ctx.fillRect(moonX + dx, moonY + rowIdx, w, 1);
    });

    // 2. Dãy núi Cửu Long (Kowloon Ridge) xa mờ với parallax 0.22x
    const ridgeOffset = Math.round(-panX * 0.22);
    ctx.fillStyle = PALETTE.lacquerVoid;
    const ridgePoints = [
      [-30, 118],
      [25, 96],
      [78, 105],
      [138, 82],
      [196, 98],
      [254, 76],
      [318, 92],
      [376, 84],
      [446, 106],
      [460, WATER_Y],
      [-30, WATER_Y],
    ];
    ctx.beginPath();
    ridgePoints.forEach(([px, py], i) => {
      const rx = px + ridgeOffset;
      if (i === 0) ctx.moveTo(rx, py);
      else ctx.lineTo(rx, py);
    });
    ctx.closePath();
    ctx.fill();

    // Đốm đèn sườn núi xa
    ctx.fillStyle = PALETTE.bronzeAntique;
    const hillLights = [
      [62, 108],
      [144, 96],
      [158, 101],
      [248, 89],
      [266, 94],
      [352, 97],
    ];
    for (const [lx, ly] of hillLights) {
      ctx.fillRect(lx + ridgeOffset, ly, 2, 1);
    }

    // 3. Dãy nhà phố cảng Hương Cảng 1930 (Shophouses & mái ngói cổ, parallax 0.55x)
    const midOffset = Math.round(-panX * 0.55);
    const lanternSources: Array<{ x: number; y: number; color: string; power: number }> = [];

    for (const b of this.buildings) {
      const bx = b.x + midOffset;
      const bodyH = WATER_Y - b.roofY;

      // Thân nhà phố
      ctx.fillStyle = PALETTE.lacquerDeep;
      ctx.fillRect(bx, b.roofY, b.w, bodyH);
      ctx.fillStyle = PALETTE.lacquerVoid;
      ctx.fillRect(bx, b.roofY, 3, bodyH);
      ctx.fillStyle = PALETTE.inkSecondary;
      ctx.fillRect(bx + b.w - 1, b.roofY, 1, bodyH);

      // Mái ngói giật cấp truyền thống
      ctx.fillStyle = PALETTE.lacquerElevated;
      ctx.fillRect(bx - 3, b.roofY - 4, b.w + 6, 5);
      ctx.fillStyle = PALETTE.bronzeMuted;
      ctx.fillRect(bx - 4, b.roofY, b.w + 8, 2);

      // Mái hiên tầng trệt
      const eaveY = WATER_Y - 22;
      ctx.fillStyle = PALETTE.bronzeMuted;
      ctx.fillRect(bx - 2, eaveY, b.w + 4, 3);
      ctx.fillStyle = PALETTE.lacquerVoid;
      ctx.fillRect(bx, eaveY + 3, b.w, 19);

      // Cửa sổ gỗ sáng đèn dầu vàng đồng
      const cols = Math.max(2, Math.floor((b.w - 10) / 13));
      for (let r = 0; r < b.stories - 1; r++) {
        const wy = b.roofY + 9 + r * 18;
        if (wy >= eaveY - 8) continue;
        for (let c = 0; c < cols; c++) {
          const wx = bx + 7 + c * 13;
          const litHash = (b.windowSeed * 7 + r * 5 + c * 3) % 5;
          if (litHash <= 2) {
            const flicker =
              !staticFrame && litHash === 0 && Math.sin(timeSec * 3.1 + wx) > 0.75;
            ctx.fillStyle = flicker ? PALETTE.parchmentLight : PALETTE.bronzeBright;
            ctx.fillRect(wx, wy, 6, 9);
            ctx.fillStyle = PALETTE.bronzeMuted;
            ctx.fillRect(wx + 2, wy, 1, 9);
            ctx.fillRect(wx, wy + 4, 6, 1);
            if (r === 0 && c === 0) {
              lanternSources.push({
                x: wx + 3,
                y: WATER_Y,
                color: PALETTE.bronzeBright,
                power: 0.55,
              });
            }
          } else {
            ctx.fillStyle = PALETTE.lacquerVoid;
            ctx.fillRect(wx, wy, 6, 9);
          }
        }
      }

      // Biển hiệu dọc Đỏ Son Triện
      if (b.hasSign) {
        const sx = bx + b.w - 8;
        const sy = b.roofY + 12;
        ctx.fillStyle = PALETTE.crimsonSeal;
        ctx.fillRect(sx, sy, 5, 22);
        ctx.fillStyle = PALETTE.bronzeBright;
        ctx.fillRect(sx + 2, sy + 3, 1, 3);
        ctx.fillRect(sx + 2, sy + 9, 1, 3);
        ctx.fillRect(sx + 2, sy + 15, 1, 3);
      }
    }

    // Bờ kè đá bến cảng
    ctx.fillStyle = PALETTE.lacquerElevated;
    ctx.fillRect(0, WATER_Y - 4, BUFFER_W, 4);
    ctx.fillStyle = PALETTE.bronzeAntique;
    ctx.fillRect(0, WATER_Y - 4, BUFFER_W, 1);

    // 4. Thuyền buồm gỗ (Junk Boat) neo ngoài vịnh Cửu Long
    const boatBob = staticFrame ? 0 : Math.round(Math.sin(timeSec * 1.7) * 1);
    const boatX = Math.round(298 - panX * 0.72);
    const boatY = WATER_Y - 3 + boatBob;

    // Cột buồm & cánh buồm nan quạt
    ctx.fillStyle = PALETTE.bronzeMuted;
    ctx.fillRect(boatX + 20, boatY - 44, 2, 42);
    ctx.fillRect(boatX + 38, boatY - 32, 2, 30);

    ctx.fillStyle = PALETTE.lacquerElevated;
    ctx.beginPath();
    ctx.moveTo(boatX + 22, boatY - 42);
    ctx.lineTo(boatX + 4, boatY - 12);
    ctx.lineTo(boatX + 22, boatY - 8);
    ctx.closePath();
    ctx.fill();

    ctx.beginPath();
    ctx.moveTo(boatX + 40, boatY - 30);
    ctx.lineTo(boatX + 26, boatY - 10);
    ctx.lineTo(boatX + 40, boatY - 7);
    ctx.closePath();
    ctx.fill();

    // Nan buồm ngang
    ctx.fillStyle = PALETTE.bronzeAntique;
    for (let sy = boatY - 34; sy <= boatY - 14; sy += 7) {
      ctx.fillRect(boatX + 9, sy, 13, 1);
    }

    // Thân thuyền gỗ
    ctx.fillStyle = PALETTE.lacquerVoid;
    ctx.beginPath();
    ctx.moveTo(boatX - 4, boatY - 4);
    ctx.lineTo(boatX + 54, boatY - 6);
    ctx.lineTo(boatX + 46, boatY + 6);
    ctx.lineTo(boatX + 4, boatY + 6);
    ctx.closePath();
    ctx.fill();
    ctx.fillStyle = PALETTE.bronzeMuted;
    ctx.fillRect(boatX + 2, boatY - 3, 46, 2);

    // Đèn bão mũi thuyền
    const boatLanternX = boatX + 6;
    const boatLanternY = boatY - 6;
    ctx.fillStyle = PALETTE.crimsonVermilion;
    ctx.fillRect(boatLanternX, boatLanternY, 3, 3);
    ctx.fillStyle = 'rgba(223, 107, 87, 0.22)';
    ctx.fillRect(boatLanternX - 4, boatLanternY - 4, 11, 11);
    lanternSources.push({
      x: boatLanternX + 1,
      y: WATER_Y + 4,
      color: PALETTE.crimsonVermilion,
      power: 0.75,
    });

    // 5. Mặt nước cảng đêm & phản chiếu ánh đèn lung linh
    ctx.fillStyle = PALETTE.lacquerVoid;
    ctx.fillRect(0, WATER_Y, BUFFER_W, BUFFER_H - WATER_Y);
    ctx.fillStyle = PALETTE.lacquerDeep;
    for (let wy = WATER_Y; wy < BUFFER_H; wy += 6) {
      ctx.fillRect(0, wy, BUFFER_W, 2);
    }

    // Các trụ đèn lồng cầu tàu tiền cảnh (parallax 1.0x) để tính phản chiếu trước khi vẽ cầu tàu
    const fgOffset = Math.round(-panX);
    const pierLanternPosts = [
      { x: 54 + fgOffset, postTopY: 122, color: PALETTE.crimsonVermilion },
      { x: 132 + fgOffset, postTopY: 130, color: PALETTE.bronzeBright },
      { x: 378 + fgOffset, postTopY: 120, color: PALETTE.crimsonVermilion },
    ];
    for (const lp of pierLanternPosts) {
      lanternSources.push({
        x: lp.x,
        y: WATER_Y + 2,
        color: lp.color,
        power: 1.0,
      });
    }

    // Vẽ vệt phản chiếu ánh đèn trên mặt nước
    for (const src of lanternSources) {
      const maxDepth = Math.round(48 * src.power);
      for (let dy = 2; dy < maxDepth; dy += 3) {
        const wy = src.y + dy;
        if (wy >= BUFFER_H) break;
        const waveShift = staticFrame
          ? ((dy * 3) % 3) - 1
          : Math.round(Math.sin(timeSec * 3.6 + dy * 0.35 + src.x * 0.1) * (1.2 + dy * 0.05));
        const segW = Math.max(2, Math.round((1 - dy / maxDepth) * 9 * src.power));
        ctx.fillStyle = dy < maxDepth * 0.45 ? src.color : PALETTE.bronzeMuted;
        ctx.fillRect(Math.round(src.x - segW * 0.5 + waveShift), wy, segW, 1);
      }
    }

    // Vòng gợn nước mưa lăn tăn
    ctx.fillStyle = PALETTE.bronzeAntique;
    for (const rp of this.ripples) {
      const prog = staticFrame ? 0.45 : ((timeSec * 0.85 + rp.phase) % 1);
      const halfW = Math.round(2 + prog * 6);
      if (prog < 0.78) {
        const rx = Math.round(rp.x - panX * 0.65);
        ctx.fillRect(rx - halfW, rp.y, halfW * 2, 1);
      }
    }

    // 6. Sương mù bến cảng trôi nhẹ (Harbour Fog Layers)
    const fogShiftA = staticFrame ? 0 : Math.round((timeSec * 9) % BUFFER_W);
    const fogShiftB = staticFrame ? 40 : Math.round((timeSec * 5.5 + 120) % BUFFER_W);
    ctx.fillStyle = 'rgba(230, 229, 220, 0.045)';
    for (let k = -1; k <= 1; k++) {
      ctx.fillRect(fogShiftA + k * BUFFER_W, WATER_Y - 14, 190, 9);
      ctx.fillRect(fogShiftB + k * BUFFER_W, WATER_Y - 5, 230, 8);
    }

    // 7. Cầu tàu gỗ tiền cảnh, cọc gỗ và đèn lồng đung đưa
    const drawPierSection = (startX: number, width: number, deckY: number): void => {
      // Cọc gỗ cắm xuống mặt nước
      for (let px = startX + 8; px < startX + width - 4; px += 22) {
        ctx.fillStyle = PALETTE.lacquerVoid;
        ctx.fillRect(px, deckY + 4, 6, BUFFER_H - deckY);
        ctx.fillStyle = PALETTE.bronzeMuted;
        ctx.fillRect(px + 1, deckY + 4, 2, 26);
      }
      // Mặt sàn ván gỗ
      ctx.fillStyle = PALETTE.lacquerElevated;
      ctx.fillRect(startX, deckY, width, 6);
      ctx.fillStyle = PALETTE.bronzeAntique;
      ctx.fillRect(startX, deckY, width, 1);
      ctx.fillStyle = PALETTE.bronzeMuted;
      for (let vx = startX + 4; vx < startX + width; vx += 7) {
        ctx.fillRect(vx, deckY + 1, 4, 2);
      }
    };

    drawPierSection(-24 + fgOffset, 174, 184);
    drawPierSection(346 + fgOffset, 108, 186);

    // Kiện hàng gỗ trên cầu tàu trái
    const crateX = 18 + fgOffset;
    ctx.fillStyle = PALETTE.bronzeMuted;
    ctx.fillRect(crateX, 171, 16, 13);
    ctx.fillStyle = PALETTE.lacquerVoid;
    ctx.fillRect(crateX + 1, 172, 14, 11);
    ctx.fillStyle = PALETTE.bronzeAntique;
    ctx.fillRect(crateX + 2, 173, 12, 9);
    ctx.fillStyle = PALETTE.bronzeMuted;
    ctx.fillRect(crateX + 2, 177, 12, 1);

    // Trụ treo đèn lồng & quầng sáng Chiaroscuro ấm áp
    for (let i = 0; i < pierLanternPosts.length; i++) {
      const lp = pierLanternPosts[i];
      const baseDeckY = i === 2 ? 186 : 184;
      const sway = staticFrame
        ? 0
        : Math.round(Math.sin(timeSec * 2.2 + i * 1.7) * 1.4);

      // Cột gỗ dựng đứng & tay đòn ngang
      ctx.fillStyle = PALETTE.lacquerVoid;
      ctx.fillRect(lp.x - 2, lp.postTopY, 4, baseDeckY - lp.postTopY);
      ctx.fillStyle = PALETTE.bronzeMuted;
      ctx.fillRect(lp.x - 1, lp.postTopY, 2, baseDeckY - lp.postTopY);
      const armDir = i === 2 ? -1 : 1;
      const lanternX = lp.x + armDir * 8 + sway;
      const lanternY = lp.postTopY + 6;
      ctx.fillStyle = PALETTE.bronzeAntique;
      ctx.fillRect(
        armDir > 0 ? lp.x : lp.x - 9,
        lp.postTopY + 1,
        10,
        2
      );
      ctx.fillRect(lanternX, lp.postTopY + 3, 1, 3);

      // Quầng sáng đèn dầu nhiều lớp
      ctx.fillStyle = 'rgba(212, 165, 93, 0.08)';
      ctx.fillRect(lanternX - 18, lanternY - 14, 37, 34);
      ctx.fillStyle = 'rgba(223, 107, 87, 0.16)';
      ctx.fillRect(lanternX - 9, lanternY - 7, 19, 19);

      // Bầu đèn lồng Đỏ Son & tâm sáng Giấy Dó
      ctx.fillStyle = PALETTE.crimsonSeal;
      ctx.fillRect(lanternX - 3, lanternY, 7, 8);
      ctx.fillStyle = lp.color;
      ctx.fillRect(lanternX - 2, lanternY + 1, 5, 6);
      ctx.fillStyle = PALETTE.parchmentLight;
      ctx.fillRect(lanternX - 1, lanternY + 2, 3, 4);
      ctx.fillStyle = PALETTE.bronzeAntique;
      ctx.fillRect(lanternX - 2, lanternY - 1, 5, 1);
      ctx.fillRect(lanternX - 2, lanternY + 8, 5, 1);
    }

    // 8. Hạt mưa chéo mảnh đặc trưng đêm mưa Cửu Long
    ctx.fillStyle = 'rgba(230, 229, 220, 0.22)';
    for (const drop of this.rainDrops) {
      const progressY = staticFrame
        ? drop.y
        : (drop.y + timeSec * drop.speed) % BUFFER_H;
      const driftX = staticFrame
        ? drop.x
        : ((drop.x - timeSec * drop.speed * 0.22) % (BUFFER_W + 40) + (BUFFER_W + 40)) %
            (BUFFER_W + 40) -
          20;

      const ix = Math.round(driftX);
      const iy = Math.round(progressY);
      for (let s = 0; s < drop.len; s += 2) {
        ctx.fillRect(ix - Math.floor(s * 0.3), iy + s, 1, 2);
      }
    }
  }

  public destroy(): void {
    this.isDestroyed = true;
    this.stopLoop();
    if (typeof window !== 'undefined') {
      window.removeEventListener('keydown', this.handleKeyDown);
    }
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.handleVisibilityChange);
    }
  }
}
