/**
 * @file src/ui/SettingsMenu.ts
 * @description Giao diện Cài Đặt Hệ Thống chuyên biệt (#settings-modal) dùng chung cho cả
 * Menu Chính lẫn Menu Tạm Dừng: Âm thanh Ngũ cung, Âm lượng tổng, Tỷ lệ Camera, Cỡ chữ,
 * Toàn màn hình, Giảm chuyển động, Chất lượng hiệu ứng thời tiết, Gợi ý HUD và Chế độ Giảng viên.
 */

import type {
  CameraZoomPreset,
  EffectiveTheme,
  EffectsQuality,
  GameSettings,
  ThemeMode,
} from '../types/game';
import { ThemeManager } from './ThemeManager';

export interface SettingsMenuCallbacks {
  onUpdateSettings?: (partial: Partial<GameSettings>) => void;
  onToggleFullscreen?: () => Promise<boolean> | boolean;
  onReplayTutorial?: () => void;
  onThemeResolved?: (effectiveTheme: EffectiveTheme, mode: ThemeMode) => void;
  onClose?: () => void;
}

const VALID_ZOOM_PRESETS = new Set<CameraZoomPreset>(['wide', 'normal', 'close']);
const VALID_THEME_MODES = new Set<ThemeMode>(['system', 'light', 'dark']);

export class SettingsMenu {
  private callbacks: SettingsMenuCallbacks;
  public readonly themeManager: ThemeManager;
  private settings: GameSettings = {
    muted: false,
    masterVolume: 0.85,
    reducedMotion: false,
    textScale: 'normal',
    showHints: true,
    cameraZoom: 'normal',
    effectsQuality: 'full',
    presenterModeUnlocked: false,
    themeMode: 'system',
  };
  private isFullscreen = false;
  private lastTriggerEl: HTMLElement | null = null;

  constructor(callbacks: SettingsMenuCallbacks = {}) {
    this.callbacks = callbacks;
    this.themeManager = new ThemeManager(this.settings.themeMode ?? 'system');
    this.themeManager.onChange((effective, mode) => {
      this.callbacks.onThemeResolved?.(effective, mode);
    });
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

    const modal = document.getElementById('settings-modal');
    modal?.addEventListener('click', (e) => {
      if (e.target === modal) {
        this.close();
        this.callbacks.onClose?.();
      }
    });

    const closeBtn = document.getElementById('btn-close-settings');
    closeBtn?.addEventListener('click', () => {
      this.close();
      this.callbacks.onClose?.();
    });

    const muteBtn = document.getElementById('toggle-mute-audio');
    if (muteBtn) {
      muteBtn.dataset.boundBySettings = 'true';
      muteBtn.addEventListener('click', () => {
        const nextMuted = !this.settings.muted;
        this.settings.muted = nextMuted;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ muted: nextMuted });
      });
    }

    const volumeSlider = document.getElementById(
      'settings-volume-slider'
    ) as HTMLInputElement | null;
    if (volumeSlider) {
      const onVolumeInput = () => {
        const pct = Math.max(0, Math.min(100, Number(volumeSlider.value) || 0));
        const nextVol = Math.round(pct) / 100;
        this.settings.masterVolume = nextVol;
        const partial: Partial<GameSettings> = { masterVolume: nextVol };
        if (nextVol > 0 && this.settings.muted) {
          this.settings.muted = false;
          partial.muted = false;
        }
        this.syncUI();
        this.callbacks.onUpdateSettings?.(partial);
      };
      volumeSlider.addEventListener('input', onVolumeInput);
      volumeSlider.addEventListener('change', onVolumeInput);
    }

    document.querySelectorAll<HTMLButtonElement>('[data-zoom-preset]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const raw = btn.dataset.zoomPreset as CameraZoomPreset | undefined;
        if (!raw || !VALID_ZOOM_PRESETS.has(raw)) return;
        this.settings.cameraZoom = raw;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ cameraZoom: raw });
      });
    });

    document.querySelectorAll<HTMLButtonElement>('[data-theme-mode]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const raw = btn.dataset.themeMode as ThemeMode | undefined;
        if (!raw || !VALID_THEME_MODES.has(raw)) return;
        this.settings.themeMode = raw;
        this.themeManager.setThemeMode(raw, true);
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ themeMode: raw });
      });
    });

    const scaleBtn = document.getElementById('toggle-text-scale');
    if (scaleBtn) {
      scaleBtn.dataset.boundBySettings = 'true';
      scaleBtn.addEventListener('click', () => {
        const nextScale = this.settings.textScale === 'large' ? 'normal' : 'large';
        this.settings.textScale = nextScale;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ textScale: nextScale });
      });
    }

    const fullscreenBtn = document.getElementById('btn-toggle-fullscreen');
    fullscreenBtn?.addEventListener('click', async () => {
      const result = await this.callbacks.onToggleFullscreen?.();
      this.isFullscreen =
        typeof result === 'boolean'
          ? result
          : Boolean(typeof document !== 'undefined' && document.fullscreenElement);
      this.syncUI();
    });

    const motionBtn = document.getElementById('toggle-reduced-motion');
    if (motionBtn) {
      motionBtn.dataset.boundBySettings = 'true';
      motionBtn.addEventListener('click', () => {
        const nextReduced = !this.settings.reducedMotion;
        this.settings.reducedMotion = nextReduced;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ reducedMotion: nextReduced });
      });
    }

    const effectsBtn = document.getElementById('toggle-effects-quality');
    effectsBtn?.addEventListener('click', () => {
      const current: EffectsQuality = this.settings.effectsQuality ?? 'full';
      const nextQuality: EffectsQuality = current === 'full' ? 'subtle' : 'full';
      this.settings.effectsQuality = nextQuality;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ effectsQuality: nextQuality });
    });

    const hintsBtn = document.getElementById('toggle-show-hints');
    if (hintsBtn) {
      hintsBtn.dataset.boundBySettings = 'true';
      hintsBtn.addEventListener('click', () => {
        const nextHints = !this.settings.showHints;
        this.settings.showHints = nextHints;
        this.syncUI();
        this.callbacks.onUpdateSettings?.({ showHints: nextHints });
      });
    }

    const presenterUnlockBtn = document.getElementById('toggle-presenter-unlock');
    presenterUnlockBtn?.addEventListener('click', () => {
      const nextUnlocked = !Boolean(this.settings.presenterModeUnlocked);
      this.settings.presenterModeUnlocked = nextUnlocked;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ presenterModeUnlocked: nextUnlocked });
    });

    const replayTutorialBtn = document.getElementById('btn-replay-tutorial');
    replayTutorialBtn?.addEventListener('click', () => {
      this.settings.tutorialCompleted = false;
      this.syncUI();
      this.callbacks.onUpdateSettings?.({ tutorialCompleted: false });
      this.callbacks.onReplayTutorial?.();
    });
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('settings-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public open(triggerEl?: HTMLElement | null): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('settings-modal');
    if (!modal) return;

    this.lastTriggerEl =
      triggerEl ?? (document.activeElement as HTMLElement | null);
    this.isFullscreen = Boolean(document.fullscreenElement) || this.isFullscreen;

    modal.classList.add('is-open');
    modal.hidden = false;
    modal.removeAttribute('hidden');
    this.syncUI();

    const firstControl =
      modal.querySelector<HTMLElement>(
        '#toggle-mute-audio, #btn-close-settings, button:not([disabled])'
      ) ?? null;
    firstControl?.focus();
  }

  public close(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('settings-modal');
    if (!modal) return;

    const wasOpen = this.isOpen();
    modal.classList.remove('is-open');
    modal.hidden = true;
    modal.setAttribute('hidden', '');

    if (
      wasOpen &&
      this.lastTriggerEl &&
      this.lastTriggerEl.isConnected &&
      !this.lastTriggerEl.closest('[hidden]')
    ) {
      this.lastTriggerEl.focus();
    }
    this.lastTriggerEl = null;
  }

  public toggle(force?: boolean): boolean {
    const next = force ?? !this.isOpen();
    if (next) {
      this.open();
    } else {
      this.close();
    }
    return this.isOpen();
  }

  public updateSettings(settings: GameSettings, isFullscreen?: boolean): void {
    this.settings = {
      muted: settings.muted,
      masterVolume: settings.masterVolume ?? 0.85,
      reducedMotion: settings.reducedMotion,
      textScale: settings.textScale,
      showHints: settings.showHints,
      cameraZoom: settings.cameraZoom ?? 'normal',
      effectsQuality: settings.effectsQuality ?? 'full',
      presenterModeUnlocked: settings.presenterModeUnlocked ?? false,
      themeMode: settings.themeMode ?? 'system',
    };
    if (typeof isFullscreen === 'boolean') {
      this.isFullscreen = isFullscreen;
    } else if (typeof document !== 'undefined') {
      this.isFullscreen = Boolean(document.fullscreenElement);
    }
    this.themeManager.setThemeMode(this.settings.themeMode ?? 'system', false);
    this.syncUI();
  }

  public syncUI(): void {
    if (typeof document === 'undefined') return;

    const isLargeText = this.settings.textScale === 'large';
    document.body.classList.toggle('reduced-motion', this.settings.reducedMotion);
    document.body.classList.toggle('text-scale-large', isLargeText);
    document.documentElement.classList.toggle('text-scale-large', isLargeText);

    const activeThemeMode: ThemeMode = this.settings.themeMode ?? 'system';
    document.querySelectorAll<HTMLButtonElement>('[data-theme-mode]').forEach((btn) => {
      const isSelected = btn.dataset.themeMode === activeThemeMode;
      btn.classList.toggle('is-active', isSelected);
      btn.setAttribute('aria-pressed', String(isSelected));
    });

    const muteBtn = document.getElementById('toggle-mute-audio');
    if (muteBtn) {
      muteBtn.textContent = this.settings.muted
        ? '○ Âm thanh Ngũ cung: Đang tắt'
        : '◆ Âm thanh Ngũ cung: Đang bật';
      muteBtn.classList.toggle('is-active', !this.settings.muted);
      muteBtn.setAttribute('aria-pressed', String(!this.settings.muted));
    }

    const volumePct = Math.round(
      Math.max(0, Math.min(1, this.settings.masterVolume ?? 0.85)) * 100
    );
    const volumeSlider = document.getElementById(
      'settings-volume-slider'
    ) as HTMLInputElement | null;
    if (volumeSlider && document.activeElement !== volumeSlider) {
      volumeSlider.value = String(volumePct);
    }
    const volumeValue = document.getElementById('settings-volume-value');
    if (volumeValue) {
      volumeValue.textContent = this.settings.muted ? `0% (Tắt)` : `${volumePct}%`;
    }

    const activeZoom: CameraZoomPreset = this.settings.cameraZoom ?? 'normal';
    document.querySelectorAll<HTMLButtonElement>('[data-zoom-preset]').forEach((btn) => {
      const isSelected = btn.dataset.zoomPreset === activeZoom;
      btn.classList.toggle('is-active', isSelected);
      btn.setAttribute('aria-pressed', String(isSelected));
    });

    const scaleBtn = document.getElementById('toggle-text-scale');
    if (scaleBtn) {
      scaleBtn.textContent = isLargeText
        ? '◆ Cỡ chữ giao diện: Lớn (114%)'
        : '○ Cỡ chữ giao diện: Chuẩn (100%)';
      scaleBtn.classList.toggle('is-active', isLargeText);
      scaleBtn.setAttribute('aria-pressed', String(isLargeText));
    }

    const fullscreenBtn = document.getElementById('btn-toggle-fullscreen');
    if (fullscreenBtn) {
      fullscreenBtn.textContent = this.isFullscreen
        ? '◆ Toàn màn hình: Đang bật [F]'
        : '○ Toàn màn hình: Cửa sổ [F]';
      fullscreenBtn.classList.toggle('is-active', this.isFullscreen);
      fullscreenBtn.setAttribute('aria-pressed', String(this.isFullscreen));
    }

    const motionBtn = document.getElementById('toggle-reduced-motion');
    if (motionBtn) {
      motionBtn.textContent = this.settings.reducedMotion
        ? '◆ Giảm chuyển động: Đang bật'
        : '○ Giảm chuyển động: Đang tắt';
      motionBtn.classList.toggle('is-active', this.settings.reducedMotion);
      motionBtn.setAttribute('aria-pressed', String(this.settings.reducedMotion));
    }

    const effectsQuality: EffectsQuality = this.settings.effectsQuality ?? 'full';
    const effectsBtn = document.getElementById('toggle-effects-quality');
    if (effectsBtn) {
      effectsBtn.textContent =
        effectsQuality === 'full'
          ? '◆ Hiệu ứng thời tiết: Đầy đủ'
          : '○ Hiệu ứng thời tiết: Tối giản';
      effectsBtn.classList.toggle('is-active', effectsQuality === 'full');
      effectsBtn.setAttribute('aria-pressed', String(effectsQuality === 'full'));
    }

    const hintsBtn = document.getElementById('toggle-show-hints');
    if (hintsBtn) {
      hintsBtn.textContent = this.settings.showHints
        ? '◆ Gợi ý nhiệm vụ trên HUD [H]: Đang bật'
        : '○ Gợi ý nhiệm vụ trên HUD [H]: Đang tắt';
      hintsBtn.classList.toggle('is-active', this.settings.showHints);
      hintsBtn.setAttribute('aria-pressed', String(this.settings.showHints));
    }

    const presenterUnlocked = Boolean(this.settings.presenterModeUnlocked);
    const presenterBtn = document.getElementById('toggle-presenter-unlock');
    if (presenterBtn) {
      presenterBtn.textContent = presenterUnlocked
        ? '◆ Chế độ Giảng viên / Kiểm thử: Đã mở khóa'
        : '○ Chế độ Giảng viên / Kiểm thử: Đang khóa';
      presenterBtn.classList.toggle('is-active', presenterUnlocked);
      presenterBtn.setAttribute('aria-pressed', String(presenterUnlocked));
    }
  }
}
