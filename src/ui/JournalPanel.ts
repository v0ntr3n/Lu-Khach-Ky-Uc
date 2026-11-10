/**
 * @file src/ui/JournalPanel.ts
 * @description Giao diện Nhật Ký Hành Trình & Biên Niên Sử La Mã (#journal-modal, Phím J).
 * Tích hợp hai ngăn thẻ: "Nhiệm Vụ" (quests) và "Biên Niên Sử I–XII" (codex) để người chơi
 * theo dõi mục tiêu hiện hành và tra cứu các cột mốc lịch sử đã khôi phục.
 */

import { CHAPTERS } from '../data/chronicles';
import type {
  ChapterData,
  MilestoneArtifact,
  QuestDefinition,
  QuestObjective,
  QuestObjectiveStatus,
  RomanMilestoneId,
} from '../types/game';

export type JournalTabId = 'quests' | 'codex';

export interface JournalQuestEntry {
  quest: QuestDefinition;
  objectives: Array<{
    objective: QuestObjective;
    status: QuestObjectiveStatus;
    prerequisiteText?: string;
  }>;
}

export interface JournalPanelCallbacks {
  onClose?: () => void;
}

const STATUS_BADGE: Record<QuestObjectiveStatus, string> = {
  completed: '✓ ĐÃ HOÀN THÀNH',
  active: '◆ ĐANG THỰC HIỆN',
  locked: '○ CHƯA MỞ KHÓA',
};

export class JournalPanel {
  private callbacks: JournalPanelCallbacks;
  private activeTab: JournalTabId = 'quests';
  private quests: JournalQuestEntry[] = [];
  private activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null = null;
  private hintText = '';
  private isPresenterMode = false;

  private codexActiveChapter: ChapterData | null = CHAPTERS[0] ?? null;
  private codexCollected: Set<RomanMilestoneId> = new Set();
  private codexActiveMilestone: MilestoneArtifact | null = null;
  private onSelectMilestone?: (milestone: MilestoneArtifact) => void;

  constructor(callbacks: JournalPanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    const modal = document.getElementById('journal-modal');
    modal?.addEventListener('click', (e) => {
      if (e.target === modal) {
        this.toggle(false);
        this.callbacks.onClose?.();
      }
    });

    const closeBtn = document.getElementById('btn-close-journal');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });

    const questsTabBtn = document.getElementById('tab-journal-quests');
    questsTabBtn?.addEventListener('click', () => {
      this.setActiveTab('quests');
    });

    const codexTabBtn = document.getElementById('tab-journal-codex');
    codexTabBtn?.addEventListener('click', () => {
      this.setActiveTab('codex');
    });
  }

  public getActiveTab(): JournalTabId {
    return this.activeTab;
  }

  public setActiveTab(tab: JournalTabId): void {
    this.activeTab = tab;
    if (typeof document === 'undefined') return;

    const questsTabBtn = document.getElementById('tab-journal-quests');
    const codexTabBtn = document.getElementById('tab-journal-codex');
    const questsPanel = document.getElementById('journal-quests-panel');
    const codexPanel = document.getElementById('journal-codex-panel');

    const isQuests = tab === 'quests';

    if (questsTabBtn) {
      questsTabBtn.classList.toggle('is-active', isQuests);
      questsTabBtn.setAttribute('aria-selected', String(isQuests));
    }
    if (codexTabBtn) {
      codexTabBtn.classList.toggle('is-active', !isQuests);
      codexTabBtn.setAttribute('aria-selected', String(!isQuests));
    }

    if (questsPanel) {
      questsPanel.hidden = !isQuests;
      if (isQuests) {
        questsPanel.removeAttribute('hidden');
      } else {
        questsPanel.setAttribute('hidden', '');
      }
    }

    if (codexPanel) {
      codexPanel.hidden = isQuests;
      if (!isQuests) {
        codexPanel.removeAttribute('hidden');
      } else {
        codexPanel.setAttribute('hidden', '');
      }
    }
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('journal-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean, initialTab?: JournalTabId): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('journal-modal');
    const openBtn = document.getElementById('btn-open-journal');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    if (initialTab) {
      this.activeTab = initialTab;
    }

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.setActiveTab(this.activeTab);
      this.render();
      this.renderCodex();
      const closeBtn = document.getElementById('btn-close-journal');
      closeBtn?.focus({ preventScroll: true });
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

  public updateCodex(
    activeChapter: ChapterData,
    collectedMilestones: ReadonlySet<RomanMilestoneId>,
    activeMilestone: MilestoneArtifact | null,
    onSelectMilestone?: (milestone: MilestoneArtifact) => void
  ): void {
    this.codexActiveChapter = activeChapter;
    this.codexCollected = new Set(collectedMilestones);
    this.codexActiveMilestone = activeMilestone;
    if (onSelectMilestone) {
      this.onSelectMilestone = onSelectMilestone;
    }
    this.renderCodex();
  }

  public renderCodex(): void {
    if (typeof document === 'undefined') return;

    const progressEl = document.getElementById('codex-progress-count');
    if (progressEl) {
      progressEl.textContent = `${this.codexCollected.size} / 12 CỘT MỐC`;
    }

    const container = document.getElementById('codex-milestone-list');
    if (!container) return;

    container.innerHTML = '';
    const activeChapterId = this.codexActiveChapter?.id ?? CHAPTERS[0]?.id;

    for (const ch of CHAPTERS) {
      const groupEl = document.createElement('div');
      groupEl.className = 'codex-chapter-group';

      const headerEl = document.createElement('div');
      headerEl.className = `codex-chapter-header ${
        ch.id === activeChapterId ? 'is-active' : ''
      }`;
      headerEl.textContent = `${ch.chapterNum} · ${ch.years} — ${ch.title}`;
      groupEl.appendChild(headerEl);

      const gridEl = document.createElement('div');
      gridEl.className = 'codex-chapter-items';

      for (const m of ch.milestones) {
        const isDone = this.codexCollected.has(m.roman);
        const isCurrent = this.codexActiveMilestone?.roman === m.roman;

        const itemBtn = document.createElement('button');
        itemBtn.type = 'button';
        itemBtn.dataset.milestoneRoman = m.roman;
        itemBtn.className = [
          'codex-item',
          isDone ? 'is-collected' : 'is-locked',
          isCurrent ? 'is-current' : '',
        ]
          .filter(Boolean)
          .join(' ');

        if (isDone) {
          itemBtn.innerHTML = `
            <span class="codex-roman">✓ ${m.roman}</span>
            <span class="codex-dots" aria-hidden="true"></span>
            <span class="codex-item-body">
              <strong class="codex-title">${m.year} · ${m.title}</strong>
              <span class="codex-cite">${m.textbookCitation}</span>
            </span>
          `;
        } else {
          itemBtn.innerHTML = `
            <span class="codex-roman">${m.roman}</span>
            <span class="codex-dots" aria-hidden="true"></span>
            <span class="codex-item-body">
              <strong class="codex-title">${m.year} · Cột mốc chưa khôi phục</strong>
              <span class="codex-cite">Khám phá tại hiện trường lịch sử để mở khóa sử liệu</span>
            </span>
          `;
        }

        itemBtn.addEventListener('click', () => {
          this.onSelectMilestone?.(m);
        });
        gridEl.appendChild(itemBtn);
      }

      groupEl.appendChild(gridEl);
      container.appendChild(groupEl);
    }
  }

  public render(): void {
    if (typeof document === 'undefined') return;

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
      summaryEl.textContent = `Tiến độ nhiệm vụ: ${completedObjectives} / ${totalObjectives} mục tiêu hoàn thành · ${
        this.codexCollected.size
      } / 12 cột mốc${this.isPresenterMode ? ' · [CHẾ ĐỘ GIẢNG VIÊN]' : ''}`;
    }

    if (bannerEl) {
      if (this.activeObjective) {
        bannerEl.innerHTML = `
          <div class="journal-banner-label">◆ MỤC TIÊU ĐANG THỰC HIỆN</div>
          <strong class="journal-banner-title">${this.activeObjective.quest.title} — ${this.activeObjective.objective.title}</strong>
          <p class="journal-banner-desc">${this.activeObjective.objective.description}</p>
          ${
            this.hintText
              ? `<p class="journal-banner-hint">▸ Gợi ý dẫn đường: ${this.hintText}</p>`
              : ''
          }
        `;
      } else {
        bannerEl.innerHTML = `
          <div class="journal-banner-label">✓ ĐÃ HOÀN THÀNH TOÀN BỘ MỤC TIÊU</div>
          <strong class="journal-banner-title">Sẵn sàng kích hoạt Cổng Thời Gian cuối chương</strong>
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

      const categoryLabel =
        quest.category === 'main' ? '◆ NHIỆM VỤ CHÍNH' : '▸ NHIỆM VỤ PHỤ';
      const provBadge = quest.provenance.label || '◆ Sử liệu Giáo trình 2021';

      let objectivesHtml = '';
      for (const item of objectives) {
        const effStatus: QuestObjectiveStatus = this.isPresenterMode
          ? 'completed'
          : item.status;
        const badgeText = STATUS_BADGE[effStatus];
        const titleText =
          effStatus === 'locked'
            ? `${item.objective.title} (Chưa mở khóa)`
            : item.objective.title;
        const descText =
          effStatus === 'locked'
            ? item.prerequisiteText ||
              'Hãy hoàn thành mục tiêu hiện tại để tiếp tục dòng sự kiện lịch sử.'
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
            ? `<div class="journal-quest-cite">◆ Nguồn đối chiếu: ${quest.provenance.citation}</div>`
            : ''
        }
      `;
      listEl.appendChild(card);
    }
  }
}
