/**
 * @file src/ui/GameHUD.ts
 * @description Quản lý lớp HUD ngữ cảnh tối giản trên khung nhìn toàn màn hình (#hud-layer):
 * Thẻ chương/địa điểm góc trái, trạng thái ẩn nấp theo ngữ cảnh, thẻ nhiệm vụ hiện hành góc phải,
 * nhãn tương tác nổi trong không gian màn hình (#world-interaction-prompt), thanh gỡ lỗi F3 và Toast.
 */

import type {
  ChapterData,
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

function cleanPromptLabel(raw: string): string {
  return raw
    .replace(/^\s*\[?\s*E\s*\]?\s*[·:–\-]?\s*/i, '')
    .replace(/\s*[\[(]\s*(?:phím\s*)?E\s*[\])]\s*$/i, '')
    .replace(/^[\p{Extended_Pictographic}\uFE0F\s]+/u, '')
    .trim();
}

export class GameHUD {
  private hasExplicitPromptCoords = false;

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
    isSheltered: boolean,
    stealthAlertState?: StealthAlertState
  ): void {
    if (typeof document === 'undefined') return;

    const numEl = document.getElementById('stage-chapter-num');
    const titleEl = document.getElementById('stage-chapter-title');
    const locEl = document.getElementById('stage-location-badge');
    const hintEl = document.getElementById('stage-mechanic-hint');
    const shelterBadge = document.getElementById('stage-shelter-status');

    if (numEl) numEl.textContent = `${chapter.chapterNum} · ${chapter.years}`;
    if (titleEl) titleEl.textContent = chapter.title;
    if (locEl && !locEl.dataset.customMap) {
      locEl.textContent = chapter.locationLabel;
    }
    if (hintEl) hintEl.textContent = chapter.mechanicHint;

    if (shelterBadge) {
      shelterBadge.classList.remove('is-sheltered', 'is-suspicious', 'is-alert');

      if (stealthAlertState === 'alert') {
        shelterBadge.hidden = false;
        shelterBadge.removeAttribute('hidden');
        shelterBadge.classList.add('is-alert');
        shelterBadge.textContent = '◆ Báo động tuần tra';
      } else if (stealthAlertState === 'suspicious') {
        shelterBadge.hidden = false;
        shelterBadge.removeAttribute('hidden');
        shelterBadge.classList.add('is-suspicious');
        shelterBadge.textContent = '◆ Tuần tra đang nghi vấn';
      } else if (isSheltered) {
        shelterBadge.hidden = false;
        shelterBadge.removeAttribute('hidden');
        shelterBadge.classList.add('is-sheltered');
        shelterBadge.textContent = '◆ Ẩn mình trong bóng tối';
      } else {
        shelterBadge.hidden = true;
        shelterBadge.setAttribute('hidden', '');
        shelterBadge.textContent = '';
      }
    }
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

  private hideInteractionPromptEl(): void {
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
    if (typeof document === 'undefined') return;
    const promptEl = document.getElementById('world-interaction-prompt');
    if (!promptEl) return;

    if (!promptText || !promptText.trim()) {
      this.hideInteractionPromptEl();
      return;
    }

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
    const minX = Math.min(vw * 0.5, 16 + halfBadgeW);
    const maxX = Math.max(vw * 0.5, vw - 16 - halfBadgeW);

    const clampedX = Math.max(16, Math.min(vw - 16, Math.max(minX, Math.min(maxX, rawX))));
    const clampedY = Math.max(64, Math.min(Math.max(64, vh - 32), rawY));

    promptEl.style.left = `${Math.round(clampedX)}px`;
    promptEl.style.top = `${Math.round(clampedY)}px`;
  }

  public updateObjectiveTracker(
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null,
    hintText: string,
    showHints: boolean
  ): void {
    if (typeof document === 'undefined') return;

    const questTitleEl = document.getElementById('hud-active-quest-title');
    const objTextEl = document.getElementById('hud-active-objective-text');
    const objHintEl = document.getElementById('hud-active-objective-hint');

    if (activeObjective) {
      if (questTitleEl) {
        questTitleEl.textContent = `◆ ${activeObjective.quest.title}`;
      }
      if (objTextEl) {
        objTextEl.textContent = `${activeObjective.objective.title}: ${activeObjective.objective.description}`;
      }
    } else {
      if (questTitleEl) {
        questTitleEl.textContent = '✓ HOÀN THÀNH MỤC TIÊU CHƯƠNG';
      }
      if (objTextEl) {
        objTextEl.textContent = 'Tiến tới không gian hội tụ để kích hoạt Cổng Thời Gian.';
      }
    }

    if (objHintEl) {
      const displayHint = showHints && Boolean(hintText);
      objHintEl.hidden = !displayHint;
      if (displayHint) {
        objHintEl.removeAttribute('hidden');
        objHintEl.textContent = `▸ Gợi ý: ${hintText}`;
      } else {
        objHintEl.setAttribute('hidden', '');
      }
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
}
