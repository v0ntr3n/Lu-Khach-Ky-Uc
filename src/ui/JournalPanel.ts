/**
 * @file src/ui/JournalPanel.ts
 * @description Giao diện Nhật Ký Hành Trình & Nhiệm Vụ Lịch Sử (#journal-modal, Phím J).
 * Hiển thị mục tiêu hiện hành, gợi ý dẫn đường, và trạng thái các nhiệm vụ chính/phụ kèm nguồn sử liệu.
 */

import type {
  QuestDefinition,
  QuestObjective,
  QuestObjectiveStatus,
} from '../types/game';

export interface JournalQuestEntry {
  quest: QuestDefinition;
  objectives: Array<{
    objective: QuestObjective;
    status: QuestObjectiveStatus;
  }>;
}

export interface JournalPanelCallbacks {
  onClose?: () => void;
}

const STATUS_BADGE: Record<QuestObjectiveStatus, string> = {
  completed: '✓ ĐÃ HOÀN THÀNH',
  active: '◆ ĐANG THỰC HIỆN',
  locked: '🔒 CHƯA MỞ KHÓA',
};

export class JournalPanel {
  private callbacks: JournalPanelCallbacks;
  private quests: JournalQuestEntry[] = [];
  private activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null = null;
  private hintText = '';
  private isPresenterMode = false;

  constructor(callbacks: JournalPanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-journal');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('journal-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean): boolean {
    const modal = document.getElementById('journal-modal');
    const openBtn = document.getElementById('btn-open-journal');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.render();
    } else {
      modal.setAttribute('hidden', '');
    }

    openBtn?.classList.toggle('is-active', next);
    openBtn?.setAttribute('aria-expanded', String(next));
    return next;
  }

  public updateState(
    quests: JournalQuestEntry[],
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null,
    hintText: string,
    isPresenterMode = false
  ): void {
    this.quests = quests;
    this.activeObjective = activeObjective;
    this.hintText = hintText;
    this.isPresenterMode = isPresenterMode;
    this.render();
  }

  public render(): void {
    const summaryEl = document.getElementById('journal-summary');
    const bannerEl = document.getElementById('journal-active-banner');
    const listEl = document.getElementById('journal-quest-list');

    let totalObjectives = 0;
    let completedObjectives = 0;
    for (const q of this.quests) {
      for (const obj of q.objectives) {
        totalObjectives++;
        if (obj.status === 'completed' || this.isPresenterMode) {
          completedObjectives++;
        }
      }
    }

    if (summaryEl) {
      summaryEl.textContent = `Tiến độ nhiệm vụ: ${completedObjectives} / ${totalObjectives} mục tiêu hoàn thành${
        this.isPresenterMode ? ' · [CHẾ ĐỘ GIẢNG VIÊN]' : ''
      }`;
    }

    if (bannerEl) {
      if (this.activeObjective) {
        bannerEl.innerHTML = `
          <div class="journal-banner-label">◆ MỤC TIÊU ĐANG THỰC HIỆN</div>
          <strong class="journal-banner-title">${this.activeObjective.quest.title} — ${this.activeObjective.objective.title}</strong>
          <p class="journal-banner-desc">${this.activeObjective.objective.description}</p>
          ${
            this.hintText
              ? `<p class="journal-banner-hint">💡 Gợi ý dẫn đường: ${this.hintText}</p>`
              : ''
          }
        `;
      } else {
        bannerEl.innerHTML = `
          <div class="journal-banner-label">✓ ĐÃ HOÀN THÀNH TOÀN BỘ MỤC TIÊU</div>
          <strong class="journal-banner-title">Sẵn sàng mở Cổng Thời Gian Chương I</strong>
        `;
      }
    }

    if (!listEl) return;
    listEl.innerHTML = '';

    for (const entry of this.quests) {
      const { quest, objectives } = entry;
      const card = document.createElement('div');
      card.className = `journal-quest-card is-${quest.category}`;
      card.dataset.questId = quest.id;

      const categoryLabel = quest.category === 'main' ? 'NHIỆM VỤ CHÍNH' : 'NHIỆM VỤ PHỤ';
      const provBadge = quest.provenance.label || 'SỬ LIỆU GIÁO TRÌNH 2021';

      let objectivesHtml = '';
      for (const item of objectives) {
        const effStatus: QuestObjectiveStatus = this.isPresenterMode ? 'completed' : item.status;
        const badgeText = STATUS_BADGE[effStatus];
        const titleText =
          effStatus === 'locked'
            ? 'Mục tiêu tiếp theo (Hoàn thành bước trước để mở khóa)'
            : item.objective.title;
        const descText =
          effStatus === 'locked'
            ? 'Hãy hoàn thành mục tiêu hiện tại để tiếp tục dòng sự kiện lịch sử.'
            : item.objective.description;

        objectivesHtml += `
          <li class="journal-objective-row is-${effStatus}" data-objective-id="${item.objective.id}">
            <div class="journal-obj-top">
              <span class="journal-status-badge is-${effStatus}">${badgeText}</span>
              <strong>${titleText}</strong>
            </div>
            <p class="journal-obj-desc">${descText}</p>
          </li>
        `;
      }

      card.innerHTML = `
        <div class="journal-quest-header">
          <div>
            <span class="journal-category-badge">${categoryLabel}</span>
            <h4 class="journal-quest-title">${quest.title}</h4>
            <p class="journal-quest-sub">${quest.subtitle}</p>
          </div>
          <span class="provenance-badge is-${quest.provenance.type}">${provBadge}</span>
        </div>
        <p class="journal-quest-summary">${quest.summary}</p>
        <ul class="journal-objective-list">${objectivesHtml}</ul>
        ${
          quest.provenance.citation
            ? `<div class="journal-quest-cite">📖 ${quest.provenance.citation}</div>`
            : ''
        }
      `;
      listEl.appendChild(card);
    }
  }
}
