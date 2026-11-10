/**
 * @file src/ui/PausePanel.ts
 * @description Giao diện Danh Mục Tạm Dừng (#pause-modal, Phím Esc / Tab).
 * Điều phối tiếp tục hành trình, mở nhanh các sổ tay tra cứu (Nhật ký, Tư liệu, Bản đồ, Cố vấn),
 * mở Cài đặt hệ thống, Toàn màn hình, Lưu tiến trình, Về Menu Chính, Xóa lưu trữ (có xác nhận),
 * và Bảng điều khiển Giảng viên (chỉ hiển thị khi được mở khóa tường minh).
 */

import type { GameSettings } from '../types/game';

export interface PausePanelCallbacks {
  onUpdateSettings?: (partial: Partial<GameSettings>) => void;
  onManualSave?: () => void;
  onResetSave?: () => void;
  onReturnToMainMenu?: () => void;
  onOpenSettings?: (triggerEl?: HTMLElement | null) => void;
  onToggleFullscreen?: () => Promise<boolean> | boolean;
  onOpenPresenter?: () => void;
  onRequestConfirm?: (
    title: string,
    body: string,
    confirmLabel: string,
    onConfirm: () => void
  ) => boolean;
  onClose?: () => void;
}

export class PausePanel {
  private callbacks: PausePanelCallbacks;
  private isPresenterMode = false;
  private isFullscreen = false;
  private settings: GameSettings = {
    muted: false,
    masterVolume: 0.85,
    reducedMotion: false,
    textScale: 'normal',
    showHints: true,
    cameraZoom: 'normal',
    effectsQuality: 'full',
    presenterModeUnlocked: false,
  };

  constructor(callbacks: PausePanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.syncUI();
  }

  private readonly handleFullscreenChange = (): void => {
    if (typeof document === 'undefined') return;
    this.isFullscreen = Boolean(document.fullscreenElement);
    this.syncUI();
  };

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    document.addEventListener('fullscreenchange', this.handleFullscreenChange);

    const modal = document.getElementById('pause-modal');
    modal?.addEventListener('click', (e) => {
      if (e.target === modal) {
        this.toggle(false);
        this.callbacks.onClose?.();
      }
    });

    const closeBtn = document.getElementById('btn-close-pause');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });

    const openSettingsBtn = document.getElementById('pause-open-settings');
    openSettingsBtn?.addEventListener('click', () => {
      this.callbacks.onOpenSettings?.(openSettingsBtn);
    });

    const fullscreenBtn = document.getElementById('pause-toggle-fullscreen');
    fullscreenBtn?.addEventListener('click', async () => {
      const result = await this.callbacks.onToggleFullscreen?.();
      this.isFullscreen =
        typeof result === 'boolean'
          ? result
          : Boolean(typeof document !== 'undefined' && document.fullscreenElement);
      this.syncUI();
    });

    const saveBtn = document.getElementById('btn-manual-save');
    saveBtn?.addEventListener('click', () => {
      this.callbacks.onManualSave?.();
    });

    const mainMenuBtn = document.getElementById('btn-return-main-menu');
    mainMenuBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onReturnToMainMenu?.();
    });

    const presenterBtn = document.getElementById('pause-open-presenter');
    presenterBtn?.addEventListener('click', () => {
      if (!this.settings.presenterModeUnlocked && !this.isPresenterMode) return;
      this.toggle(false);
      this.callbacks.onOpenPresenter?.();
    });

    const resetBtn = document.getElementById('btn-reset-save');
    resetBtn?.addEventListener('click', () => {
      const title = 'XÓA LƯU TRỮ & CHƠI LẠI TỪ ĐẦU?';
      const body =
        'Toàn bộ tiến trình khám phá, cột mốc La Mã và tư liệu đã thu thập trong bộ nhớ cục bộ sẽ bị xóa vĩnh viễn để khởi đầu lại từ Chương I.';
      const confirmLabel = 'Xác nhận xóa & Chơi lại';

      if (this.callbacks.onRequestConfirm) {
        const handled = this.callbacks.onRequestConfirm(title, body, confirmLabel, () => {
          this.toggle(false);
          this.callbacks.onResetSave?.();
        });
        if (handled) return;
      }

      if (this.openFallbackConfirmModal(title, body, confirmLabel)) {
        return;
      }

      this.toggle(false);
      this.callbacks.onResetSave?.();
    });

    // Tương thích ngược cho kiểm thử đơn vị khởi tạo PausePanel độc lập không qua SettingsMenu
    this.bindLegacySettingsFallback();
  }

  private openFallbackConfirmModal(
    title: string,
    body: string,
    confirmLabel: string
  ): boolean {
    if (typeof document === 'undefined') return false;
    const confirmModal = document.getElementById('confirm-modal');
    const titleEl = document.getElementById('confirm-modal-title');
    const bodyEl = document.getElementById('confirm-modal-body');
    const yesBtn = document.getElementById('btn-confirm-yes') as HTMLButtonElement | null;
    const noBtn = document.getElementById('btn-confirm-no') as HTMLButtonElement | null;

    if (!confirmModal || !yesBtn || !noBtn) return false;

    const trigger = document.activeElement as HTMLElement | null;
    if (titleEl) titleEl.textContent = title;
    if (bodyEl) bodyEl.textContent = body;
    yesBtn.textContent = confirmLabel;

    const closeConfirm = () => {
      confirmModal.classList.remove('is-open');
      confirmModal.hidden = true;
      confirmModal.setAttribute('hidden', '');
    };

    yesBtn.onclick = () => {
      closeConfirm();
      this.toggle(false);
      this.callbacks.onResetSave?.();
    };
    noBtn.onclick = () => {
      closeConfirm();
      if (trigger && trigger.isConnected && !trigger.closest('[hidden]')) {
        trigger.focus({ preventScroll: true });
      }
    };

    confirmModal.classList.add('is-open');
    confirmModal.hidden = false;
    confirmModal.removeAttribute('hidden');
    noBtn.focus({ preventScroll: true });
    return true;
  }

  private bindLegacySettingsFallback(): void {
    if (typeof document === 'undefined') return;

    const muteBtn = document.getElementById('toggle-mute-audio');
    if (muteBtn && muteBtn.dataset.boundBySettings !== 'true') {
      muteBtn.addEventListener('click', () => {
        const nextMuted = !this.settings.muted;
        this.settings.muted = nextMuted;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ muted: nextMuted });
      });
    }

    const motionBtn = document.getElementById('toggle-reduced-motion');
    if (motionBtn && motionBtn.dataset.boundBySettings !== 'true') {
      motionBtn.addEventListener('click', () => {
        const nextReduced = !this.settings.reducedMotion;
        this.settings.reducedMotion = nextReduced;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ reducedMotion: nextReduced });
      });
    }

    const scaleBtn = document.getElementById('toggle-text-scale');
    if (scaleBtn && scaleBtn.dataset.boundBySettings !== 'true') {
      scaleBtn.addEventListener('click', () => {
        const nextScale = this.settings.textScale === 'large' ? 'normal' : 'large';
        this.settings.textScale = nextScale;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ textScale: nextScale });
      });
    }

    const hintsBtn = document.getElementById('toggle-show-hints');
    if (hintsBtn && hintsBtn.dataset.boundBySettings !== 'true') {
      hintsBtn.addEventListener('click', () => {
        const nextHints = !this.settings.showHints;
        this.settings.showHints = nextHints;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ showHints: nextHints });
      });
    }
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('pause-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('pause-modal');
    const openBtn = document.getElementById('btn-open-pause');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.isFullscreen = Boolean(document.fullscreenElement) || this.isFullscreen;
      this.syncUI();
      const resumeBtn = document.getElementById('btn-close-pause');
      resumeBtn?.focus({ preventScroll: true });
    } else {
      modal.setAttribute('hidden', '');
    }

    openBtn?.classList.toggle('is-active', next);
    openBtn?.setAttribute('aria-expanded', String(next));
    return next;
  }

  public setPresenterMode(isPresenterMode: boolean): void {
    this.isPresenterMode = isPresenterMode;
    this.syncUI();
  }

  public updateSettings(settings: GameSettings, isPresenterMode?: boolean): void {
    this.settings = { ...this.settings, ...settings };
    if (typeof isPresenterMode === 'boolean') {
      this.isPresenterMode = isPresenterMode;
    }
    this.syncUI();
  }

  public syncUI(): void {
    if (typeof document === 'undefined') return;

    const isLargeText = this.settings.textScale === 'large';
    document.body.classList.toggle('reduced-motion', this.settings.reducedMotion);
    document.body.classList.toggle('text-scale-large', isLargeText);
    document.documentElement.classList.toggle('text-scale-large', isLargeText);

    const fullscreenBtn = document.getElementById('pause-toggle-fullscreen');
    if (fullscreenBtn) {
      fullscreenBtn.textContent = this.isFullscreen
        ? '◆ Thoát toàn màn hình [F]'
        : '▸ Toàn màn hình [F]';
      fullscreenBtn.setAttribute('aria-pressed', String(this.isFullscreen));
    }

    const presenterBtn = document.getElementById('pause-open-presenter');
    if (presenterBtn) {
      const canShowPresenter = Boolean(
        this.settings.presenterModeUnlocked || this.isPresenterMode
      );
      presenterBtn.hidden = !canShowPresenter;
      if (canShowPresenter) {
        presenterBtn.removeAttribute('hidden');
      } else {
        presenterBtn.setAttribute('hidden', '');
      }
    }

    // Chỉ đồng bộ nhãn trực tiếp trên #toggle-* nếu SettingsMenu chưa tiếp quản
    const muteBtn = document.getElementById('toggle-mute-audio');
    if (muteBtn && muteBtn.dataset.boundBySettings !== 'true') {
      muteBtn.textContent = this.settings.muted
        ? '○ Âm thanh Ngũ cung: Đang tắt'
        : '◆ Âm thanh Ngũ cung: Đang bật';
      muteBtn.setAttribute('aria-pressed', String(!this.settings.muted));
    }

    const motionBtn = document.getElementById('toggle-reduced-motion');
    if (motionBtn && motionBtn.dataset.boundBySettings !== 'true') {
      motionBtn.textContent = this.settings.reducedMotion
        ? '◆ Giảm chuyển động: Đang bật'
        : '○ Giảm chuyển động: Đang tắt';
      motionBtn.setAttribute('aria-pressed', String(this.settings.reducedMotion));
    }

    const scaleBtn = document.getElementById('toggle-text-scale');
    if (scaleBtn && scaleBtn.dataset.boundBySettings !== 'true') {
      scaleBtn.textContent = isLargeText
        ? '◆ Cỡ chữ giao diện: Lớn (114%)'
        : '○ Cỡ chữ giao diện: Chuẩn (100%)';
      scaleBtn.setAttribute('aria-pressed', String(isLargeText));
    }

    const hintsBtn = document.getElementById('toggle-show-hints');
    if (hintsBtn && hintsBtn.dataset.boundBySettings !== 'true') {
      hintsBtn.textContent = this.settings.showHints
        ? '◆ Gợi ý nhiệm vụ trên HUD [H]: Đang bật'
        : '○ Gợi ý nhiệm vụ trên HUD [H]: Đang tắt';
      hintsBtn.setAttribute('aria-pressed', String(this.settings.showHints));
    }
  }
}
