/**
 * @file src/ui/JournalPanel.ts
 * @description Cửa sổ Nhật Ký Hành Trình & Biên Niên Sử La Mã I–XII (#journal-modal, phím J)
 * thiết kế theo phong cách Pixel RPG Eastward với bố cục 2 cột (Master List + Detail Inspector):
 * - Ngăn 1 (#journal-quests-panel): Cột trái (#journal-quest-list) liệt kê nhóm nhiệm vụ Chính/Phụ
 *   và các bước mục tiêu ngắn gọn, dễ quét; Cột phải (#journal-active-banner) hiển thị chi tiết mục tiêu
 *   đang chọn/dẫn đường, gợi ý hành động và ghi chú bối cảnh (loại bỏ hoàn toàn văn bản trùng lặp và huy hiệu rườm rà).
 * - Ngăn 2 (#journal-codex-panel): Cột trái (#codex-milestone-list) liệt kê 12 Cột mốc La Mã I–XII theo chương;
 *   Cột phải (#codex-milestone-detail) hiển thị chi tiết cột mốc, ý nghĩa lịch sử và nguồn tham chiếu.
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

export interface ActiveObjectiveBannerInfo {
  questTitle: string;
  objectiveId?: string;
  objectiveTitle: string;
  description: string;
  hint?: string;
}

export interface JournalPanelCallbacks {
  onSelectObjective?: (objectiveId: string) => void;
  onSelectMilestone?: (milestone: MilestoneArtifact) => void;
  onClose?: () => void;
}

/** Escape text interpolated into Journal's small static HTML templates. */
function escapeJournalHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

export class JournalPanel {
  private callbacks: JournalPanelCallbacks;
  private activeTab: JournalTabId = 'quests';

  private quests: JournalQuestEntry[] = [];
  private activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null = null;
  private trackedObjectiveId: string | null = null;
  private hintText = '';
  private isPresenterMode = false;
  private showHints = true;

  private inspectedQuestId: string | null = null;
  private inspectedObjectiveId: string | null = null;
  private lastInspectorObjectiveId: string | null = null;
  private loreExpanded = false;

  private codexActiveChapter: ChapterData | null = CHAPTERS[0] ?? null;
  private codexCollected: Set<RomanMilestoneId> = new Set();
  private codexActiveMilestone: MilestoneArtifact | null = null;
  private inspectedMilestoneRoman: RomanMilestoneId | null = null;
  private onSelectMilestone?: (milestone: MilestoneArtifact) => void;

  constructor(callbacks: JournalPanelCallbacks = {}) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    const closeBtn = document.getElementById('btn-close-journal');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });

    const modal = document.getElementById('journal-modal');
    modal?.addEventListener('click', (e) => {
      if (e.target === modal) {
        this.toggle(false);
        this.callbacks.onClose?.();
      }
    });

    const tabQuests = document.getElementById('tab-journal-quests');
    const tabCodex = document.getElementById('tab-journal-codex');

    tabQuests?.addEventListener('click', () => this.setActiveTab('quests'));
    tabCodex?.addEventListener('click', () => this.setActiveTab('codex'));
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('journal-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public getActiveTab(): JournalTabId {
    return this.activeTab;
  }

  public switchTab(tab: JournalTabId): void {
    this.setActiveTab(tab);
  }

  public setActiveTab(tab: JournalTabId): void {
    this.activeTab = tab;
    if (typeof document === 'undefined') return;

    const tabQuests = document.getElementById('tab-journal-quests');
    const tabCodex = document.getElementById('tab-journal-codex');
    const panelQuests = document.getElementById('journal-quests-panel');
    const panelCodex = document.getElementById('journal-codex-panel');

    const isQuests = tab === 'quests';

    if (tabQuests) {
      tabQuests.classList.toggle('is-active', isQuests);
      tabQuests.setAttribute('aria-selected', String(isQuests));
    }
    if (tabCodex) {
      tabCodex.classList.toggle('is-active', !isQuests);
      tabCodex.setAttribute('aria-selected', String(!isQuests));
    }

    if (panelQuests) {
      panelQuests.hidden = !isQuests;
      if (!isQuests) {
        panelQuests.setAttribute('hidden', '');
      } else {
        panelQuests.removeAttribute('hidden');
      }
    }

    if (panelCodex) {
      panelCodex.hidden = isQuests;
      if (isQuests) {
        panelCodex.setAttribute('hidden', '');
      } else {
        panelCodex.removeAttribute('hidden');
      }
    }
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

  /**
   * Cập nhật trạng thái nhiệm vụ từ HudController / GameApp và kết xuất bố cục 2 cột.
   */
  public updateState(
    quests: JournalQuestEntry[],
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null,
    hintText: string,
    isPresenterMode = false,
    trackedObjectiveId: string | null = null,
    showHints = true
  ): void {
    this.quests = quests;
    this.activeObjective = activeObjective;
    this.hintText = hintText;
    this.isPresenterMode = isPresenterMode;
    this.showHints = showHints;
    this.trackedObjectiveId =
      trackedObjectiveId ?? activeObjective?.objective.id ?? null;
    this.render();
  }

  /**
   * Kết xuất ngăn Nhiệm Vụ (#journal-quests-panel) theo bố cục 2 cột Eastward RPG:
   * - Cột trái (#journal-quest-list): Danh sách nhiệm vụ Chính/Phụ và các dòng mục tiêu tinh gọn.
   * - Cột phải (#journal-active-banner): Khung chi tiết nhiệm vụ & bước mục tiêu đang chọn/theo dõi.
   */
  public render(): void {
    if (typeof document === 'undefined') return;

    const summaryEl = document.getElementById('journal-summary');
    const bannerEl = document.getElementById('journal-active-banner');
    const listEl = document.getElementById('journal-quest-list');

    // Đảm bảo khung 2 cột luôn được bao bọc quanh #journal-quest-list và #journal-active-banner
    const questsPanel = document.getElementById('journal-quests-panel');
    if (questsPanel && listEl && bannerEl) {
      let twoColWrap = questsPanel.querySelector<HTMLElement>('.journal-two-col-grid');
      if (!twoColWrap) {
        twoColWrap = document.createElement('div');
        twoColWrap.className = 'journal-two-col-grid';
        listEl.parentNode?.insertBefore(twoColWrap, listEl);
        twoColWrap.appendChild(listEl);
        twoColWrap.appendChild(bannerEl);
      }
    }

    let totalObjectives = 0;
    let completedObjectives = 0;
    let completedQuests = 0;

    for (const qEntry of this.quests) {
      let allDone = qEntry.objectives.length > 0;
      for (const objItem of qEntry.objectives) {
        totalObjectives += 1;
        const effStatus: QuestObjectiveStatus = this.isPresenterMode
          ? 'completed'
          : objItem.status;
        if (effStatus === 'completed') {
          completedObjectives += 1;
        } else {
          allDone = false;
        }
      }
      if (allDone) {
        completedQuests += 1;
      }
    }

    if (summaryEl) {
      summaryEl.textContent = `${completedObjectives}/${totalObjectives} mục tiêu · ${completedQuests}/${this.quests.length} nhiệm vụ${
        this.isPresenterMode ? ' · Giảng viên' : ''
      }`;
    }

    const effectiveTrackedId =
      this.trackedObjectiveId ?? this.activeObjective?.objective.id ?? null;

    // Xác định nhiệm vụ và mục tiêu đang được chọn để hiển thị chi tiết ở cột phải
    let selectedEntry: JournalQuestEntry | undefined;
    let selectedObjItem:
      | {
          objective: QuestObjective;
          status: QuestObjectiveStatus;
          prerequisiteText?: string;
        }
      | undefined;

    if (this.inspectedObjectiveId) {
      for (const entry of this.quests) {
        const found = entry.objectives.find(
          (o) => o.objective.id === this.inspectedObjectiveId
        );
        if (found) {
          selectedEntry = entry;
          selectedObjItem = found;
          break;
        }
      }
    }

    if (!selectedEntry && effectiveTrackedId) {
      for (const entry of this.quests) {
        const found = entry.objectives.find(
          (o) => o.objective.id === effectiveTrackedId
        );
        if (found) {
          selectedEntry = entry;
          selectedObjItem = found;
          break;
        }
      }
    }

    if (!selectedEntry && this.inspectedQuestId) {
      selectedEntry = this.quests.find(
        (e) => e.quest.id === this.inspectedQuestId
      );
    }

    if (!selectedEntry && this.quests.length > 0) {
      selectedEntry =
        this.quests.find((e) =>
          e.objectives.some(
            (o) => (this.isPresenterMode ? 'completed' : o.status) !== 'completed'
          )
        ) ?? this.quests[0];
    }

    if (selectedEntry && !selectedObjItem) {
      selectedObjItem =
        selectedEntry.objectives.find(
          (o) => o.objective.id === effectiveTrackedId
        ) ??
        selectedEntry.objectives.find(
          (o) => (this.isPresenterMode ? 'completed' : o.status) === 'active'
        ) ??
        selectedEntry.objectives[selectedEntry.objectives.length - 1];
    }

    this.renderQuestInspector(
      bannerEl,
      selectedEntry ?? null,
      selectedObjItem ?? null,
      effectiveTrackedId
    );

    if (!listEl) return;
    listEl.innerHTML = '';

    for (const entry of this.quests) {
      const { quest, objectives } = entry;
      let doneSteps = 0;
      let hasActive = false;

      for (const item of objectives) {
        const effStatus: QuestObjectiveStatus = this.isPresenterMode
          ? 'completed'
          : item.status;
        if (effStatus === 'completed') doneSteps += 1;
        if (effStatus === 'active') hasActive = true;
      }

      const isQuestCompleted =
        objectives.length > 0 && doneSteps === objectives.length;
      const isQuestSelected = selectedEntry?.quest.id === quest.id;

      const card = document.createElement('article');
      card.className = `journal-quest-card is-${quest.category} ${
        isQuestCompleted ? 'is-completed' : hasActive ? 'is-active' : 'is-locked'
      }${isQuestSelected ? ' is-selected' : ''}`;
      card.dataset.questId = quest.id;

      const header = document.createElement('div');
      header.className = 'journal-quest-header';
      header.tabIndex = 0;
      header.setAttribute('role', 'button');
      header.setAttribute('aria-label', `Chọn nhiệm vụ ${quest.title}`);
      header.title = quest.title;
      header.innerHTML = `
        <div class="journal-quest-title-wrap">
          <span class="journal-category-glyph" aria-hidden="true">${quest.category === 'main' ? '◆' : '◇'}</span>
          <h4 class="journal-quest-title">${escapeJournalHtml(quest.hudTitle || quest.title)}</h4>
        </div>
        <span class="journal-step-count">${doneSteps}/${objectives.length}</span>
      `;
      const selectQuest = () => {
        this.inspectedQuestId = quest.id;
        const firstActive = objectives.find(
          (o) => (this.isPresenterMode ? 'completed' : o.status) === 'active'
        );
        this.inspectedObjectiveId =
          firstActive?.objective.id ?? objectives[0]?.objective.id ?? null;
        this.render();
      };
      header.addEventListener('click', selectQuest);
      header.addEventListener('keydown', (ev) => {
        if (ev.key === 'Enter' || ev.key === ' ') {
          ev.preventDefault();
          selectQuest();
        }
      });
      card.appendChild(header);

      const objList = document.createElement('ul');
      objList.className = 'journal-objective-list';

      for (const item of objectives) {
        const effStatus: QuestObjectiveStatus = this.isPresenterMode
          ? 'completed'
          : item.status;
        const isTracked =
          effStatus === 'active' && item.objective.id === effectiveTrackedId;
        const isInspected =
          selectedObjItem?.objective.id === item.objective.id;

        const li = document.createElement('li');
        li.className = `journal-objective-row is-${effStatus}${
          isTracked ? ' is-tracked' : ''
        }${isInspected ? ' is-selected' : ''}`;
        li.dataset.objectiveId = item.objective.id;

        const statusGlyph =
          effStatus === 'completed'
            ? '✓'
            : isTracked
              ? '▼'
              : effStatus === 'active'
                ? '◆'
                : '○';

        li.innerHTML = `
          <div class="journal-obj-top">
            <span class="journal-status-badge is-${effStatus}${isTracked ? ' is-tracked' : ''}" aria-hidden="true">${statusGlyph}</span>
            <strong class="journal-obj-title">${escapeJournalHtml(item.objective.hudText || item.objective.title)}</strong>
          </div>
        `;
        li.title = item.objective.title;

        if (effStatus === 'active') {
          li.tabIndex = 0;
          li.setAttribute('role', 'button');
          li.setAttribute(
            'aria-label',
            `Dẫn đường mục tiêu: ${item.objective.title}`
          );
          const triggerSelect = () => {
            this.inspectedQuestId = quest.id;
            this.inspectedObjectiveId = item.objective.id;
            this.callbacks.onSelectObjective?.(item.objective.id);
          };
          li.addEventListener('click', triggerSelect);
          li.addEventListener('keydown', (ev) => {
            if (ev.key === 'Enter' || ev.key === ' ') {
              ev.preventDefault();
              triggerSelect();
            }
          });
        } else {
          li.addEventListener('click', () => {
            this.inspectedQuestId = quest.id;
            this.inspectedObjectiveId = item.objective.id;
            this.render();
          });
        }

        objList.appendChild(li);
      }

      card.appendChild(objList);
      listEl.appendChild(card);
    }
  }

  private renderQuestInspector(
    bannerEl: HTMLElement | null,
    entry: JournalQuestEntry | null,
    objItem: {
      objective: QuestObjective;
      status: QuestObjectiveStatus;
      prerequisiteText?: string;
    } | null,
    effectiveTrackedId: string | null
  ): void {
    if (!bannerEl) return;

    if (!entry && !this.activeObjective) {
      bannerEl.innerHTML = `
        <div class="journal-inspector-empty">
          <div class="journal-banner-label">✓ Đã hoàn thành</div>
          <div class="journal-banner-title">Đến Cổng Thời Gian</div>
        </div>
      `;
      return;
    }

    const quest = entry?.quest ?? this.activeObjective?.quest;
    const objective = objItem?.objective ?? this.activeObjective?.objective;
    const effStatus: QuestObjectiveStatus = this.isPresenterMode
      ? 'completed'
      : (objItem?.status ?? 'active');
    const isTracked = Boolean(
      objective && effStatus === 'active' && objective.id === effectiveTrackedId
    );

    if (this.lastInspectorObjectiveId !== (objective?.id ?? null)) {
      this.lastInspectorObjectiveId = objective?.id ?? null;
      this.loreExpanded = false;
    }

    const stepIndex =
      quest && objective
        ? Math.max(1, quest.objectives.findIndex((o) => o.id === objective.id) + 1)
        : 1;
    const totalSteps = quest ? Math.max(1, quest.objectives.length) : 1;
    const statusText =
      effStatus === 'completed'
        ? '✓ Hoàn thành'
        : effStatus === 'locked'
          ? '○ Chưa mở'
          : isTracked
            ? '◆ Đang dẫn đường'
            : '◆ Có thể thực hiện';
    const shortObjective = escapeJournalHtml(
      objective?.hudText || objective?.title || 'Đã hoàn thành mục tiêu'
    );
    const shortQuest = escapeJournalHtml(quest?.hudTitle || quest?.title || '');
    const unlockMessage =
      effStatus === 'locked'
        ? '<p class="journal-compact-hint">Hoàn thành mục tiêu trước để mở khóa.</p>'
        : '';

    const trackActionHtml =
      objective && effStatus === 'active'
        ? `<button type="button" class="ctrl-btn journal-inspector-track-btn${isTracked ? ' is-primary' : ''}" data-inspect-track-id="${escapeJournalHtml(objective.id)}">${isTracked ? '✓ Đang dẫn đường' : '⌖ Dẫn đường'}</button>`
        : '';

    // The full historical text, specific requirements and citations are preserved
    // in a deliberate disclosure, rather than competing with gameplay actions.
    const citation =
      quest?.provenance?.citation ||
      quest?.provenance?.historicalNote ||
      quest?.provenance?.label || '';
    const hint = objective?.hintText || (isTracked ? this.hintText : '');
    const loreHtml = `
      <details class="journal-quest-lore" ${this.loreExpanded ? 'open' : ''}>
        <summary>Xem chi tiết &amp; tư liệu</summary>
        <div class="journal-quest-lore-content">
          ${quest?.title ? `<h5>${escapeJournalHtml(quest.title)}</h5>` : ''}
          ${quest?.summary ? `<p>${escapeJournalHtml(quest.summary)}</p>` : ''}
          ${objective?.title ? `<p><strong>Mục tiêu:</strong> ${escapeJournalHtml(objective.title)}</p>` : ''}
          ${objective?.description ? `<p>${escapeJournalHtml(objective.description)}</p>` : ''}
          ${objItem?.prerequisiteText ? `<p><strong>Điều kiện:</strong> ${escapeJournalHtml(objItem.prerequisiteText)}</p>` : ''}
          ${this.showHints && hint && effStatus !== 'completed' ? `<p><strong>Gợi ý:</strong> ${escapeJournalHtml(hint)}</p>` : ''}
          ${citation ? `<p class="journal-quest-cite"><strong>Nguồn:</strong> ${escapeJournalHtml(citation)}</p>` : ''}
        </div>
      </details>
    `;

    const sealHtml =
      effStatus === 'completed'
        ? '<span class="crimson-seal-stamp" aria-label="Đã kiểm định">ĐÃ KIỂM ĐỊNH</span>'
        : '';

    bannerEl.innerHTML = `
      <div class="journal-inspector-top">
        <span class="journal-banner-label">${statusText}</span>
        ${sealHtml}
        <span class="journal-inspector-step-pill">${stepIndex}/${totalSteps}</span>
      </div>
      <div class="journal-inspector-quest-head">
        <span class="journal-inspector-quest-title">${shortQuest}</span>
      </div>
      <div class="journal-inspector-objective-box is-${effStatus}">
        <div class="journal-banner-title"><strong>${shortObjective}</strong></div>
        ${unlockMessage}
        ${trackActionHtml}
      </div>
      ${loreHtml}
    `;

    const trackBtn = bannerEl.querySelector<HTMLButtonElement>(
      '.journal-inspector-track-btn'
    );
    trackBtn?.addEventListener('click', () => {
      const objId = trackBtn.dataset.inspectTrackId;
      if (objId) this.callbacks.onSelectObjective?.(objId);
    });
    const lore = bannerEl.querySelector<HTMLDetailsElement>('.journal-quest-lore');
    lore?.addEventListener('toggle', () => {
      this.loreExpanded = lore.open;
    });
  }

  /**
   * Cập nhật ngăn Biên Niên Sử La Mã I–XII (#journal-codex-panel) theo bố cục 2 cột:
   * - Cột trái (#codex-milestone-list): Danh sách 12 cột mốc tinh gọn chia theo 4 Chương.
   * - Cột phải (#codex-milestone-detail): Khung tra cứu chi tiết cột mốc đang chọn.
   */
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

    const countEl = document.getElementById('codex-progress-count');
    if (countEl) {
      countEl.textContent = `${this.codexCollected.size} / 12 CỘT MỐC`;
    }

    const codexList = document.getElementById('codex-milestone-list');
    if (!codexList) return;

    // Đảm bảo khung 2 cột cho Biên Niên Sử có vùng chi tiết #codex-milestone-detail
    const codexPanel = document.getElementById('journal-codex-panel');
    let detailEl = document.getElementById('codex-milestone-detail');
    if (codexPanel && !detailEl) {
      let splitWrap = codexPanel.querySelector<HTMLElement>('.codex-two-col-grid');
      if (!splitWrap) {
        splitWrap = document.createElement('div');
        splitWrap.className = 'codex-two-col-grid';
        codexList.parentNode?.insertBefore(splitWrap, codexList);
        splitWrap.appendChild(codexList);
      }
      detailEl = document.createElement('aside');
      detailEl.id = 'codex-milestone-detail';
      detailEl.setAttribute('aria-live', 'polite');
      splitWrap.appendChild(detailEl);
    }

    const activeChapter = this.codexActiveChapter ?? CHAPTERS[0];
    const allMilestones: Array<{ chapter: ChapterData; milestone: MilestoneArtifact }> = [];
    for (const ch of CHAPTERS) {
      for (const m of ch.milestones) {
        allMilestones.push({ chapter: ch, milestone: m });
      }
    }

    const collectedArr = Array.from(this.codexCollected);
    const targetRoman =
      this.inspectedMilestoneRoman ??
      this.codexActiveMilestone?.roman ??
      collectedArr[collectedArr.length - 1] ??
      activeChapter?.milestones[0]?.roman ??
      'I';

    const selectedEntry =
      allMilestones.find((e) => e.milestone.roman === targetRoman) ??
      allMilestones[0];

    codexList.innerHTML = '';

    for (const ch of CHAPTERS) {
      const groupEl = document.createElement('div');
      groupEl.className = 'codex-chapter-group';

      const headerEl = document.createElement('div');
      headerEl.className = `codex-chapter-header ${
        ch.id === activeChapter?.id ? 'is-active' : ''
      }`;
      headerEl.textContent = `${ch.chapterNum} · ${ch.years} — ${ch.title}`;
      groupEl.appendChild(headerEl);

      const itemsBox = document.createElement('div');
      itemsBox.className = 'codex-chapter-items';

      for (const m of ch.milestones) {
        const isDone = this.codexCollected.has(m.roman);
        const isCurrent = selectedEntry?.milestone.roman === m.roman;

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

        itemBtn.innerHTML = `
          <span class="codex-roman">${isDone ? '✓ ' : ''}${m.roman}</span>
          <div class="codex-item-body">
            <strong class="codex-title">${m.year} · ${
              isDone || this.isPresenterMode ? m.title : 'Cột mốc chưa khôi phục'
            }</strong>
          </div>
        `;

        itemBtn.addEventListener('click', () => {
          this.inspectedMilestoneRoman = m.roman;
          this.renderCodexDetail(
            detailEl,
            ch,
            m,
            this.codexCollected.has(m.roman)
          );
          codexList
            .querySelectorAll('.codex-item.is-current')
            .forEach((el) => el.classList.remove('is-current'));
          itemBtn.classList.add('is-current');
          if (this.onSelectMilestone) {
            this.onSelectMilestone(m);
          } else {
            this.callbacks.onSelectMilestone?.(m);
          }
        });
        itemsBox.appendChild(itemBtn);
      }

      groupEl.appendChild(itemsBox);
      codexList.appendChild(groupEl);
    }

    if (selectedEntry) {
      this.renderCodexDetail(
        detailEl,
        selectedEntry.chapter,
        selectedEntry.milestone,
        this.codexCollected.has(selectedEntry.milestone.roman)
      );
    }
  }

  private renderCodexDetail(
    detailEl: HTMLElement | null,
    chapter: ChapterData,
    milestone: MilestoneArtifact,
    isCollected: boolean
  ): void {
    if (!detailEl) return;

    if (!isCollected && !this.isPresenterMode) {
      detailEl.innerHTML = `
        <div class="codex-detail-top">
          <span class="codex-detail-seal is-locked">CỘT MỐC ${milestone.roman} · ${milestone.year}</span>
          <span class="codex-detail-status is-locked">○ Chưa khôi phục</span>
        </div>
        <h4 class="codex-detail-title">Dấu Ấn Lịch Sử Chưa Mở Khóa</h4>
        <div class="codex-detail-meta">${chapter.chapterNum} · ${chapter.locationLabel}</div>
        <p class="codex-detail-desc">
          Khám phá không gian <strong>${chapter.locationLabel}</strong> và hoàn thành nhiệm vụ liên quan để khôi phục sự kiện lịch sử này vào Biên Niên Sử.
        </p>
        <div class="codex-detail-cite">◆ Tham chiếu: ${milestone.textbookCitation}</div>
      `;
      return;
    }

    const quoteText = milestone.dialogue[0] ?? '';

    detailEl.innerHTML = `
      <div class="codex-detail-top">
        <span class="codex-detail-seal is-collected">✓ CỘT MỐC ${milestone.roman} · ${milestone.year}</span>
        <span class="crimson-seal-stamp" aria-label="Đã khôi phục">ẤN CHƯƠNG LỊCH SỬ</span>
        <span class="codex-detail-status is-collected">Đã khôi phục</span>
      </div>
      <h4 class="codex-detail-title">${milestone.title}</h4>
      <div class="codex-detail-meta">${chapter.chapterNum} · ${milestone.npcSpeaker} (${milestone.npcRole})</div>
      ${
        quoteText
          ? `<blockquote class="codex-detail-quote">${quoteText}</blockquote>`
          : ''
      }
      <p class="codex-detail-desc">${milestone.historicalSignificance}</p>
      <div class="codex-detail-cite">◆ Tư liệu: ${milestone.textbookCitation}</div>
    `;
  }
}
