/**
 * @file src/ui/PausePanel.ts
 * @description Giao diện Tạm Dừng & Thiết Lập Hệ Thống (#pause-modal, Phím Tab).
 * Quản lý các nút cấu hình trợ năng (Âm thanh, Giảm chuyển động, Cỡ chữ lớn, Gợi ý HUD)
 * và thao tác Lưu / Xóa Lưu Trữ.
 */

import type { GameSettings } from '../types/game';

export interface PausePanelCallbacks {
  onUpdateSettings?: (partial: Partial<GameSettings>) => void;
  onManualSave?: () => void;
  onResetSave?: () => void;
  onClose?: () => void;
}

export class PausePanel {
  private callbacks: PausePanelCallbacks;
  private settings: GameSettings = {
    muted: false,
    reducedMotion: false,
    textScale: 'normal',
    showHints: true,
  };

  constructor(callbacks: PausePanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.syncUI();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-pause');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });

    const muteBtn = document.getElementById('toggle-mute-audio');
    muteBtn?.addEventListener('click', () => {
      const nextMuted = !this.settings.muted;
      this.settings.muted = nextMuted;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ muted: nextMuted });
    });

    const motionBtn = document.getElementById('toggle-reduced-motion');
    motionBtn?.addEventListener('click', () => {
      const nextReduced = !this.settings.reducedMotion;
      this.settings.reducedMotion = nextReduced;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ reducedMotion: nextReduced });
    });

    const scaleBtn = document.getElementById('toggle-text-scale');
    scaleBtn?.addEventListener('click', () => {
      const nextScale = this.settings.textScale === 'large' ? 'normal' : 'large';
      this.settings.textScale = nextScale;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ textScale: nextScale });
    });

    const hintsBtn = document.getElementById('toggle-show-hints');
    hintsBtn?.addEventListener('click', () => {
      const nextHints = !this.settings.showHints;
      this.settings.showHints = nextHints;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ showHints: nextHints });
    });

    const saveBtn = document.getElementById('btn-manual-save');
    saveBtn?.addEventListener('click', () => {
      this.callbacks.onManualSave?.();
    });

    const resetBtn = document.getElementById('btn-reset-save');
    resetBtn?.addEventListener('click', () => {
      this.callbacks.onResetSave?.();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('pause-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean): boolean {
    const modal = document.getElementById('pause-modal');
    const openBtn = document.getElementById('btn-open-pause');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.syncUI();
    } else {
      modal.setAttribute('hidden', '');
    }

    openBtn?.classList.toggle('is-active', next);
    openBtn?.setAttribute('aria-expanded', String(next));
    return next;
  }

  public updateSettings(settings: GameSettings): void {
    this.settings = { ...settings };
    this.syncUI();
  }

  public syncUI(): void {
    if (typeof document === 'undefined') return;

    document.body.classList.toggle('reduced-motion', this.settings.reducedMotion);
    document.body.classList.toggle('text-scale-large', this.settings.textScale === 'large');

    const muteBtn = document.getElementById('toggle-mute-audio');
    if (muteBtn) {
      muteBtn.textContent = this.settings.muted
        ? '🔇 Âm thanh Ngũ Cung 8-Bit: TẮT'
        : '🔊 Âm thanh Ngũ Cung 8-Bit: BẬT';
      muteBtn.setAttribute('aria-pressed', String(!this.settings.muted));
    }

    const motionBtn = document.getElementById('toggle-reduced-motion');
    if (motionBtn) {
      motionBtn.textContent = this.settings.reducedMotion
        ? '✓ Giảm chuyển động / Hiệu ứng mưa & rung: BẬT'
        : '○ Giảm chuyển động / Hiệu ứng mưa & rung: TẮT';
      motionBtn.setAttribute('aria-pressed', String(this.settings.reducedMotion));
    }

    const scaleBtn = document.getElementById('toggle-text-scale');
    if (scaleBtn) {
      scaleBtn.textContent =
        this.settings.textScale === 'large'
          ? '✓ Cỡ chữ lớn dễ đọc: BẬT'
          : '○ Cỡ chữ lớn dễ đọc: MẶC ĐỊNH';
      scaleBtn.setAttribute('aria-pressed', String(this.settings.textScale === 'large'));
    }

    const hintsBtn = document.getElementById('toggle-show-hints');
    if (hintsBtn) {
      hintsBtn.textContent = this.settings.showHints
        ? '✓ Hiển thị gợi ý nhiệm vụ trên HUD: BẬT'
        : '○ Hiển thị gợi ý nhiệm vụ trên HUD: TẮT';
      hintsBtn.setAttribute('aria-pressed', String(this.settings.showHints));
    }
  }
}
