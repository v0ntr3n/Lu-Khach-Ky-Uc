/**
 * @file src/ui/GameHUD.ts
 * @description Quản lý HUD ngữ cảnh trên khung nhìn toàn màn hình (Objective Tracker,
 * Location/Shelter Badges, và thông báo Toast).
 */

import type { ChapterData, QuestDefinition, QuestObjective } from '../types/game';

export class GameHUD {
  public updateStageHeader(chapter: ChapterData, isSheltered: boolean): void {
    const numEl = document.getElementById('stage-chapter-num');
    const titleEl = document.getElementById('stage-chapter-title');
    const locEl = document.getElementById('stage-location-badge');
    const hintEl = document.getElementById('stage-mechanic-hint');
    const shelterBadge = document.getElementById('stage-shelter-status');

    if (numEl) numEl.textContent = `${chapter.chapterNum} (${chapter.years})`;
    if (titleEl) titleEl.textContent = chapter.title;
    if (locEl && !locEl.dataset.customMap) {
      locEl.textContent = `📍 ${chapter.locationLabel} · ${chapter.coordinates}`;
    }
    if (hintEl) hintEl.textContent = chapter.mechanicHint;

    if (shelterBadge) {
      shelterBadge.textContent = isSheltered
        ? '🛡️ ĐANG ẨN MÌNH TRONG BÓNG TỐI (AN TOÀN)'
        : '🚶 ĐANG DI CHUYỂN TRÊN TUYẾN ĐƯỜNG LỊCH SỬ';
      shelterBadge.classList.toggle('is-sheltered', isSheltered);
    }
  }

  public updateMapLocationBadge(
    mapTitle: string,
    coordinates?: string,
    promptText?: string | null
  ): void {
    const locEl = document.getElementById('stage-location-badge');
    if (locEl) {
      locEl.dataset.customMap = 'true';
      locEl.textContent = coordinates ? `📍 ${mapTitle} · ${coordinates}` : `📍 ${mapTitle}`;
    }

    const interactBtn = document.getElementById('ctrl-interact');
    if (interactBtn && promptText !== undefined) {
      interactBtn.textContent = promptText ? `✨ ${promptText}` : '✨ Khôi Phục / Mở Cổng (E)';
    }
  }

  public updateObjectiveTracker(
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null,
    hintText: string,
    showHints: boolean
  ): void {
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
        questTitleEl.textContent = '✓ HOÀN THÀNH NHIỆM VỤ CHƯƠNG I';
      }
      if (objTextEl) {
        objTextEl.textContent = 'Tiến tới Phòng Hội Nghị Hợp Nhất để mở Cổng Thời Gian.';
      }
    }

    if (objHintEl) {
      const displayHint = showHints && Boolean(hintText);
      objHintEl.hidden = !displayHint;
      if (displayHint) {
        objHintEl.removeAttribute('hidden');
        objHintEl.textContent = `💡 Gợi ý: ${hintText}`;
      } else {
        objHintEl.setAttribute('hidden', '');
      }
    }
  }

  public showToast(message: string): void {
    const toast = document.getElementById('stage-toast');
    if (!toast) return;
    toast.textContent = message;
    toast.classList.add('is-visible');
    window.clearTimeout((toast as unknown as { _timer?: number })._timer);
    (toast as unknown as { _timer?: number })._timer = window.setTimeout(() => {
      toast.classList.remove('is-visible');
    }, 3200);
  }
}
