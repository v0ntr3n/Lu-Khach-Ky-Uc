/**
 * @file src/ui/GameHUD.ts
 * @description Quản lý lớp HUD ngữ cảnh tối giản trên khung nhìn toàn màn hình (#hud-layer):
 * Nhãn khu vực top-center không khung, intro ải (ẢI N : <Tên ải>), trạng thái ẩn nấp góc trái,
 * thanh nhiệm vụ 1 dòng góc phải (chỉ hiện [E] khi vào tầm tương tác thật),
 * nhãn tương tác nổi trong không gian màn hình (#world-interaction-prompt), thanh gỡ lỗi F3 và Toast.
 */

import type {
  ChapterData,
  EraId,
  QuestDefinition,
  QuestObjective,
  StealthAlertState,
} from '../types/game';

export interface DebugReadoutInfo {
  mapId: string;
  x: number;
  y: number;
  zoom: string;
  scale: number;
  fps?: number;
}

export const STAGE_INTRO_LABELS: Record<EraId, string> = {
  '1930': 'ẢI 1 : Hương Cảng 1930',
  '1945': 'ẢI 2 : Tân Trào – Ba Đình 1945',
  '1954_1975': 'ẢI 3 : Điện Biên – Mùa Xuân 1975',
  '1986_2045': 'ẢI 4 : Đổi Mới – Kỷ Nguyên Số 2045',
};

export function formatStageIntroLabel(eraId: EraId | string): string {
  return STAGE_INTRO_LABELS[eraId as EraId] ?? `ẢI 1 : ${String(eraId)}`;
}

function cleanPromptLabel(raw: string): string {
  return raw
    .replace(/^\s*\[?\s*E\s*\]?\s*[·:–\-]?\s*/i, '')
    .replace(/\s*[\[(]\s*(?:phím\s*)?E\s*[\])]\s*$/i, '')
    .replace(/^[\p{Extended_Pictographic}\uFE0F\s]+/u, '')
    .trim();
}

export function stripStaticInteractKey(raw: string): string {
  return raw
    .replace(/^\s*[→◆▸✓]\s*/u, '')
    .replace(/\s*(?:và\s+nhấn\s+)?\[\s*(?:phím\s*)?E\s*\]\.?\s*$/i, '')
    .replace(/\s*\[\s*E\s*\]/gi, '')
    .trim();
}

export class GameHUD {
  private hasExplicitPromptCoords = false;
  private canInteractNow = false;
  private lastActiveObjective: {
    quest: QuestDefinition;
    objective: QuestObjective;
  } | null = null;
  private lastNextActionText: string | undefined = undefined;
  private lastHintText = '';
  private lastShowHints = true;
  private lastStageIntroEraId: string | null = null;

  public setVisible(visible: boolean): void {
    if (typeof document === 'undefined') return;
    const hudLayer = document.getElementById('hud-layer');
    if (!hudLayer) return;

    hudLayer.hidden = !visible;
    hudLayer.classList.toggle('is-hidden', !visible);
    if (visible) {
      hudLayer.removeAttribute('hidden');
    } else {
      hudLayer.setAttribute('hidden', '');
      this.hideInteractionPromptEl();
    }
  }

  public isVisible(): boolean {
    if (typeof document === 'undefined') return false;
    const hudLayer = document.getElementById('hud-layer');
    return Boolean(hudLayer && !hudLayer.hidden);
  }

  public updateStageHeader(
    chapter: ChapterData,
    _isSheltered: boolean,
    _stealthAlertState?: StealthAlertState
  ): void {
    if (typeof document === 'undefined') return;

    const numEl = document.getElementById('stage-chapter-num');
    const titleEl = document.getElementById('stage-chapter-title');
    const locEl = document.getElementById('stage-location-badge');
    const hintEl = document.getElementById('stage-mechanic-hint');

    if (numEl) numEl.textContent = `${chapter.chapterNum} · ${chapter.years}`;
    if (titleEl) titleEl.textContent = chapter.title;
    if (locEl && !locEl.dataset.customMap) {
      locEl.textContent = chapter.locationLabel;
    }
    if (hintEl) hintEl.textContent = chapter.mechanicHint;

  }

  public updateMapLocationBadge(
    mapTitle: string,
    _coordinates?: string,
    promptText?: string | null
  ): void {
    if (typeof document === 'undefined') return;

    const locEl = document.getElementById('stage-location-badge');
    if (locEl) {
      locEl.dataset.customMap = 'true';
      locEl.textContent = mapTitle;
    }

    const interactBtn = document.getElementById('ctrl-interact');
    if (interactBtn && promptText !== undefined) {
      const cleaned = promptText ? cleanPromptLabel(promptText) : '';
      interactBtn.textContent = cleaned ? `${cleaned} [E]` : 'Tương tác [E]';
    }

    if (promptText !== undefined && !this.hasExplicitPromptCoords) {
      this.renderInteractionPromptInternal(promptText);
    }
  }

  public updateInteractionPrompt(
    promptText: string | null,
    screenX?: number,
    screenY?: number,
    shellW?: number,
    shellH?: number
  ): void {
    this.hasExplicitPromptCoords = true;
    this.renderInteractionPromptInternal(promptText, screenX, screenY, shellW, shellH);
  }

  private setCanInteractNow(nextCanInteract: boolean): void {
    if (this.canInteractNow !== nextCanInteract) {
      this.canInteractNow = nextCanInteract;
      this.renderObjectiveSingleLine();
    }
  }

  private hideInteractionPromptEl(): void {
    this.setCanInteractNow(false);
    if (typeof document === 'undefined') return;
    const promptEl = document.getElementById('world-interaction-prompt');
    if (!promptEl) return;
    promptEl.classList.remove('is-visible');
    promptEl.hidden = true;
    promptEl.setAttribute('hidden', '');
  }

  private renderInteractionPromptInternal(
    promptText: string | null,
    screenX?: number,
    screenY?: number,
    shellW?: number,
    shellH?: number
  ): void {
    if (!promptText || !promptText.trim()) {
      this.hideInteractionPromptEl();
      return;
    }

    this.setCanInteractNow(true);
    if (typeof document === 'undefined') return;
    const promptEl = document.getElementById('world-interaction-prompt');
    if (!promptEl) return;

    const labelEl = document.getElementById('world-interaction-label');
    const cleaned = cleanPromptLabel(promptText) || promptText.trim();
    if (labelEl) {
      labelEl.textContent = cleaned;
    } else {
      promptEl.textContent = `E · ${cleaned}`;
    }

    promptEl.hidden = false;
    promptEl.removeAttribute('hidden');
    promptEl.classList.add('is-visible');

    const vw =
      shellW ??
      (typeof window !== 'undefined' && window.innerWidth > 0 ? window.innerWidth : 1280);
    const vh =
      shellH ??
      (typeof window !== 'undefined' && window.innerHeight > 0 ? window.innerHeight : 720);

    const rawX = typeof screenX === 'number' && Number.isFinite(screenX) ? screenX : vw * 0.5;
    const rawY =
      typeof screenY === 'number' && Number.isFinite(screenY) ? screenY : vh - 88;

    const halfBadgeW = Math.max(0, Math.ceil((promptEl.offsetWidth || 0) * 0.5));
    const minX = Math.min(vw * 0.5, 32 + halfBadgeW);
    const maxX = Math.max(vw * 0.5, vw - 32 - halfBadgeW);

    const clampedX = Math.max(32, Math.min(vw - 32, Math.max(minX, Math.min(maxX, rawX))));
    const clampedY = Math.max(56, Math.min(Math.max(56, vh - 48), rawY));

    promptEl.style.left = `${Math.round(clampedX)}px`;
    promptEl.style.top = `${Math.round(clampedY)}px`;
  }

  public updateObjectiveTracker(
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null,
    hintText: string,
    showHints: boolean,
    nextActionText?: string,
    canInteract?: boolean
  ): void {
    this.lastActiveObjective = activeObjective;
    this.lastNextActionText = nextActionText;
    this.lastHintText = hintText;
    this.lastShowHints = showHints;
    if (typeof canInteract === 'boolean') {
      this.canInteractNow = canInteract;
    }
    this.renderObjectiveSingleLine();
  }

  private renderObjectiveSingleLine(): void {
    if (typeof document === 'undefined') return;

    const trackerEl = document.getElementById('hud-objective-tracker');
    const questTitleEl = document.getElementById('hud-active-quest-title');
    const objTextEl = document.getElementById('hud-active-objective-text');
    const objActionEl = document.getElementById('hud-active-objective-action');
    const objHintEl = document.getElementById('hud-active-objective-hint');

    let baseLine = '';
    if (this.lastActiveObjective) {
      const raw =
        this.lastActiveObjective.objective.hudText ||
        this.lastActiveObjective.objective.title;
      baseLine = stripStaticInteractKey(raw);
    } else {
      const rawFallback =
        this.lastNextActionText &&
        !/Cổng Thời Gian cuối chương/i.test(this.lastNextActionText)
          ? this.lastNextActionText
          : 'Đến Cổng Thời Gian';
      baseLine = stripStaticInteractKey(rawFallback) || 'Đến Cổng Thời Gian';
    }

    if (objTextEl) {
      objTextEl.textContent = this.canInteractNow ? `${baseLine} [E]` : baseLine;
    }

    if (trackerEl) {
      trackerEl.classList.toggle('is-interact-ready', this.canInteractNow);
    }

    if (questTitleEl) {
      questTitleEl.textContent = this.lastActiveObjective
        ? `◆ ${this.lastActiveObjective.quest.title}`
        : '◆ Đã hoàn tất nhiệm vụ chương';
      questTitleEl.hidden = true;
      questTitleEl.setAttribute('hidden', '');
    }

    if (objActionEl) {
      objActionEl.textContent = baseLine;
      objActionEl.hidden = true;
      objActionEl.setAttribute('hidden', '');
    }

    if (objHintEl) {
      const displayHint = this.lastShowHints && Boolean(this.lastHintText);
      objHintEl.textContent = displayHint ? `▸ Gợi ý: ${this.lastHintText}` : '';
      objHintEl.hidden = true;
      objHintEl.setAttribute('hidden', '');
    }
  }

  public updateDebugReadout(enabled: boolean, info?: DebugReadoutInfo): void {
    if (typeof document === 'undefined') return;
    const readoutEl = document.getElementById('dev-debug-readout');
    if (!readoutEl) return;

    readoutEl.hidden = !enabled;
    if (!enabled) {
      readoutEl.setAttribute('hidden', '');
      return;
    }

    readoutEl.removeAttribute('hidden');
    if (info) {
      const fpsSegment =
        typeof info.fps === 'number' ? ` · FPS ${Math.round(info.fps)}` : '';
      readoutEl.textContent = `[F3 DEBUG] MAP: ${info.mapId} · XY: (${Math.round(
        info.x
      )}, ${Math.round(info.y)}) · ZOOM: ${info.zoom} (${info.scale}×)${fpsSegment}`;
    } else if (!readoutEl.textContent) {
      readoutEl.textContent = '[F3 DEBUG] Đang hiển thị lớp kiểm định va chạm & lộ trình';
    }
  }

  public showToast(message: string): void {
    if (typeof document === 'undefined') return;
    const toast = document.getElementById('stage-toast');
    if (!toast) return;
    toast.textContent = message;
    toast.classList.add('is-visible');
    if (typeof window !== 'undefined') {
      window.clearTimeout((toast as unknown as { _timer?: number })._timer);
      (toast as unknown as { _timer?: number })._timer = window.setTimeout(() => {
        toast.classList.remove('is-visible');
      }, 3200);
    }
  }

  public resetStageIntroTracker(): void {
    this.lastStageIntroEraId = null;
  }

  public showStageIntro(eraId: EraId | string, force = false): boolean {
    const key = String(eraId);
    if (!force && this.lastStageIntroEraId === key) {
      return false;
    }
    this.lastStageIntroEraId = key;
    const introLabel = formatStageIntroLabel(eraId);
    this.showRegionEntryBanner(introLabel, '');
    return true;
  }

  public showRegionEntryBanner(zoneTitle: string, eraLabel = ''): void {
    if (typeof document === 'undefined') return;
    const banner = document.getElementById('region-entry-banner');
    const eraEl = document.getElementById('region-entry-era');
    const titleEl = document.getElementById('region-entry-title');
    const hudLayer = document.getElementById('hud-layer');
    if (!banner) return;

    if (eraEl) {
      eraEl.textContent = eraLabel;
      const hasEra = Boolean(eraLabel && eraLabel.trim());
      eraEl.hidden = !hasEra;
      if (hasEra) {
        eraEl.removeAttribute('hidden');
      } else {
        eraEl.setAttribute('hidden', '');
      }
    }
    if (titleEl) {
      titleEl.textContent = zoneTitle;
    }

    banner.hidden = false;
    banner.removeAttribute('hidden');
    banner.classList.remove('is-fading-out');
    banner.classList.add('is-visible');
    hudLayer?.classList.add('is-stage-intro-active');

    if (typeof window !== 'undefined') {
      const bannerTimers = banner as unknown as {
        _timer?: number;
        _fadeTimer?: number;
      };
      window.clearTimeout(bannerTimers._timer);
      window.clearTimeout(bannerTimers._fadeTimer);

      const isReducedMotion =
        document.body.classList.contains('reduced-motion') ||
        document.body.classList.contains('is-reduced-motion');
      const holdMs = 1750;
      const fadeOutMs = isReducedMotion ? 0 : 260;

      bannerTimers._timer = window.setTimeout(() => {
        banner.classList.remove('is-visible');
        banner.classList.add('is-fading-out');
        hudLayer?.classList.remove('is-stage-intro-active');

        if (fadeOutMs <= 0) {
          banner.classList.remove('is-fading-out');
          banner.hidden = true;
          banner.setAttribute('hidden', '');
        } else {
          bannerTimers._fadeTimer = window.setTimeout(() => {
            banner.classList.remove('is-fading-out');
            banner.hidden = true;
            banner.setAttribute('hidden', '');
          }, fadeOutMs);
        }
      }, holdMs);
    }
  }
}
