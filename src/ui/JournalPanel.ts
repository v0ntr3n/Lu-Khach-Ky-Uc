/**
 * @file src/ui/JournalPanel.ts
 * @description Cửa sổ Nhật Ký Hành Trình & Biên Niên Sử La Mã I–XII (#journal-modal, phím J)
 * thiết kế theo phong cách Pixel RPG Eastward với bố cục 2 cột (Master List + Detail Inspector):
 * - Ngăn 1 (#journal-quests-panel): Cột trái (#journal-quest-list) liệt kê nhóm nhiệm vụ Chính/Phụ
 *   tinh gọn; Cột phải (#journal-active-banner) hiển thị mục tiêu đang chọn, nút dẫn đường, dấu Triện Son
 *   và ngăn mở rộng `<details class="journal-quest-lore">` chứa chi tiết & tư liệu.
 * - Ngăn 2 (#journal-codex-panel): Cột trái (#codex-milestone-list) liệt kê 12 Cột mốc La Mã I–XII;
 *   Cột phải (#codex-milestone-detail) hiển thị chi tiết cột mốc, dấu Triện Son và nguồn tham chiếu.
 * - Dựng cây DOM trực tiếp qua `createDomElement()` / `replaceChildren()` (tuyệt đối không nối chuỗi HTML thô)
 *   theo chuẩn Modern Web Guidance (`sanitize-untrusted-html`).
 */

import { CHAPTERS } from '../data/chronicles';
import type {
  ChapterData,
  JournalTabId,
  MilestoneArtifact,
  QuestDefinition,
  QuestObjective,
  QuestObjectiveStatus,
  RomanMilestoneId,
} from '../types/game';
import { createDomElement } from './safeDom';

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
    const triggerBtn = document.getElementById('btn-open-journal');
    if (!modal) return false;

    const next = force ?? modal.hidden;
    if (next && initialTab) {
      this.setActiveTab(initialTab);
    } else if (next) {
      this.setActiveTab(this.activeTab);
    }

    modal.hidden = !next;
    modal.classList.toggle('is-open', next);
    if (next) {
      modal.removeAttribute('hidden');
      const closeBtn = document.getElementById('btn-close-journal');
      closeBtn?.focus({ preventScroll: true });
    } else {
      modal.setAttribute('hidden', '');
    }

    triggerBtn?.classList.toggle('is-active', next);
    triggerBtn?.setAttribute('aria-expanded', String(next));
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
   * Kết xuất ngăn Nhiệm Vụ (#journal-quests-panel) theo bố cục 2 cột Eastward RPG.
   */
  public render(): void {
    if (typeof document === 'undefined') return;

    const summaryEl = document.getElementById('journal-summary');
    const bannerEl = document.getElementById('journal-active-banner');
    const listEl = document.getElementById('journal-quest-list');

    const questsPanel = document.getElementById('journal-quests-panel');
    if (questsPanel && listEl && bannerEl) {
      let twoColWrap = questsPanel.querySelector<HTMLElement>('.journal-two-col-grid');
      if (!twoColWrap) {
        twoColWrap = createDomElement('div', { className: 'journal-two-col-grid' });
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
    const questCards: HTMLElement[] = [];

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

      const card = createDomElement('article', {
        className: `journal-quest-card is-${quest.category} ${
          isQuestCompleted ? 'is-completed' : hasActive ? 'is-active' : 'is-locked'
        }${isQuestSelected ? ' is-selected' : ''}`,
      });
      card.dataset.questId = quest.id;

      const header = createDomElement(
        'div',
        {
          className: 'journal-quest-header',
          attrs: {
            role: 'button',
            'aria-label': `Chọn nhiệm vụ ${quest.title}`,
          },
        },
        [
          createDomElement('div', { className: 'journal-quest-title-wrap' }, [
            createDomElement('span', {
              className: 'journal-category-glyph',
              text: quest.category === 'main' ? '◆' : '◇',
              attrs: { 'aria-hidden': 'true' },
            }),
            createDomElement('h4', {
              className: 'journal-quest-title',
              text: quest.hudTitle || quest.title,
            }),
          ]),
          createDomElement('span', {
            className: 'journal-step-count',
            text: `${doneSteps}/${objectives.length}`,
          }),
        ]
      );
      header.tabIndex = 0;
      header.title = quest.title;

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

      const objList = createDomElement('ul', {
        className: 'journal-objective-list',
      });

      for (const item of objectives) {
        const effStatus: QuestObjectiveStatus = this.isPresenterMode
          ? 'completed'
          : item.status;
        const isTracked =
          effStatus === 'active' && item.objective.id === effectiveTrackedId;
        const isInspected =
          selectedObjItem?.objective.id === item.objective.id;

        const statusGlyph =
          effStatus === 'completed'
            ? '✓'
            : isTracked
              ? '▼'
              : effStatus === 'active'
                ? '◆'
                : '○';

        const li = createDomElement(
          'li',
          {
            className: `journal-objective-row is-${effStatus}${
              isTracked ? ' is-tracked' : ''
            }${isInspected ? ' is-selected' : ''}`,
          },
          [
            createDomElement('div', { className: 'journal-obj-top' }, [
              createDomElement('span', {
                className: `journal-status-badge is-${effStatus}${
                  isTracked ? ' is-tracked' : ''
                }`,
                text: statusGlyph,
                attrs: { 'aria-hidden': 'true' },
              }),
              createDomElement('strong', {
                className: 'journal-obj-title',
                text: item.objective.hudText || item.objective.title,
              }),
            ]),
          ]
        );
        li.dataset.objectiveId = item.objective.id;
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
      questCards.push(card);
    }

    listEl.replaceChildren(...questCards);
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
      const emptyBox = createDomElement(
        'div',
        { className: 'journal-inspector-empty' },
        [
          createDomElement('div', {
            className: 'journal-banner-label',
            text: '✓ Đã hoàn thành',
          }),
          createDomElement('div', {
            className: 'journal-banner-title',
            text: 'Đến Cổng Thời Gian',
          }),
        ]
      );
      bannerEl.replaceChildren(emptyBox);
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
    const shortObjective =
      objective?.hudText || objective?.title || 'Đã hoàn thành mục tiêu';
    const shortQuest = quest?.hudTitle || quest?.title || '';

    const unlockHintEl =
      effStatus === 'locked'
        ? createDomElement('p', {
            className: 'journal-compact-hint',
            text: 'Hoàn thành mục tiêu trước để mở khóa.',
          })
        : null;

    let trackBtnEl: HTMLButtonElement | null = null;
    if (objective && effStatus === 'active') {
      trackBtnEl = createDomElement('button', {
        className: `ctrl-btn journal-inspector-track-btn${
          isTracked ? ' is-primary' : ''
        }`,
        text: isTracked ? '✓ Đang dẫn đường' : '⌖ Dẫn đường',
        attrs: { type: 'button' },
      });
      trackBtnEl.dataset.inspectTrackId = objective.id;
      const targetObjId = objective.id;
      trackBtnEl.addEventListener('click', () => {
        this.callbacks.onSelectObjective?.(targetObjId);
      });
    }

    const citation =
      quest?.provenance?.citation ||
      quest?.provenance?.historicalNote ||
      quest?.provenance?.label ||
      '';
    const hint = objective?.hintText || (isTracked ? this.hintText : '');

    const loreContentChildren: Array<Node | null> = [
      quest?.title ? createDomElement('h5', { text: quest.title }) : null,
      quest?.summary ? createDomElement('p', { text: quest.summary }) : null,
      objective?.title
        ? createDomElement('p', undefined, [
            createDomElement('strong', { text: 'Mục tiêu:' }),
            document.createTextNode(` ${objective.title}`),
          ])
        : null,
      objective?.description
        ? createDomElement('p', { text: objective.description })
        : null,
      objItem?.prerequisiteText
        ? createDomElement('p', undefined, [
            createDomElement('strong', { text: 'Điều kiện:' }),
            document.createTextNode(` ${objItem.prerequisiteText}`),
          ])
        : null,
      this.showHints && hint && effStatus !== 'completed'
        ? createDomElement('p', undefined, [
            createDomElement('strong', { text: 'Gợi ý:' }),
            document.createTextNode(` ${hint}`),
          ])
        : null,
      citation
        ? createDomElement('p', { className: 'journal-quest-cite' }, [
            createDomElement('strong', { text: 'Nguồn:' }),
            document.createTextNode(` ${citation}`),
          ])
        : null,
    ];

    const loreDetails = createDomElement(
      'details',
      { className: 'journal-quest-lore' },
      [
        createDomElement('summary', { text: 'Xem chi tiết & tư liệu' }),
        createDomElement(
          'div',
          { className: 'journal-quest-lore-content' },
          loreContentChildren
        ),
      ]
    );
    if (this.loreExpanded) {
      loreDetails.open = true;
    }
    loreDetails.addEventListener('toggle', () => {
      this.loreExpanded = loreDetails.open;
    });

    const statusHeaderEl =
      effStatus === 'completed'
        ? createDomElement('span', {
            className: 'crimson-seal-stamp',
            text: '✓ ĐÃ KIỂM ĐỊNH',
            attrs: { 'aria-label': 'Đã kiểm định' },
          })
        : createDomElement('span', {
            className: 'journal-banner-label',
            text: statusText,
          });

    const topRow = createDomElement('div', { className: 'journal-inspector-top' }, [
      statusHeaderEl,
      createDomElement('span', {
        className: 'journal-inspector-step-pill',
        text: `${stepIndex}/${totalSteps}`,
      }),
    ]);

    const questHead = createDomElement(
      'div',
      { className: 'journal-inspector-quest-head' },
      [
        createDomElement('span', {
          className: 'journal-inspector-quest-title',
          text: shortQuest,
        }),
      ]
    );

    const objBox = createDomElement(
      'div',
      { className: `journal-inspector-objective-box is-${effStatus}` },
      [
        createDomElement('div', { className: 'journal-banner-title' }, [
          createDomElement('strong', { text: shortObjective }),
        ]),
        unlockHintEl,
        trackBtnEl,
      ]
    );

    bannerEl.replaceChildren(topRow, questHead, objBox, loreDetails);
  }

  /**
   * Cập nhật ngăn Biên Niên Sử La Mã I–XII (#journal-codex-panel) theo bố cục 2 cột.
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
    this.onSelectMilestone = onSelectMilestone;
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

    const codexPanel = document.getElementById('journal-codex-panel');
    let detailEl = document.getElementById('codex-milestone-detail');
    if (codexPanel && !detailEl) {
      let splitWrap = codexPanel.querySelector<HTMLElement>('.codex-two-col-grid');
      if (!splitWrap) {
        splitWrap = createDomElement('div', { className: 'codex-two-col-grid' });
        codexList.parentNode?.insertBefore(splitWrap, codexList);
        splitWrap.appendChild(codexList);
      }
      detailEl = createDomElement('aside', {
        attrs: { id: 'codex-milestone-detail', 'aria-live': 'polite' },
      });
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

    const groups: HTMLElement[] = [];

    for (const ch of CHAPTERS) {
      const groupEl = createDomElement('div', { className: 'codex-chapter-group' });

      const headerEl = createDomElement('div', {
        className: `codex-chapter-header ${
          ch.id === activeChapter?.id ? 'is-active' : ''
        }`,
        text: `${ch.chapterNum} · ${ch.years} — ${ch.title}`,
      });
      groupEl.appendChild(headerEl);

      const itemsBox = createDomElement('div', { className: 'codex-chapter-items' });

      for (const m of ch.milestones) {
        const isDone = this.codexCollected.has(m.roman);
        const isCurrent = selectedEntry?.milestone.roman === m.roman;

        const itemBtn = createDomElement(
          'button',
          {
            className: [
              'codex-item',
              isDone ? 'is-collected' : 'is-locked',
              isCurrent ? 'is-current' : '',
            ]
              .filter(Boolean)
              .join(' '),
            attrs: { type: 'button' },
          },
          [
            createDomElement('span', {
              className: 'codex-roman',
              text: `${isDone ? '✓ ' : ''}${m.roman}`,
            }),
            createDomElement('div', { className: 'codex-item-body' }, [
              createDomElement('strong', {
                className: 'codex-title',
                text: `${m.year} · ${
                  isDone || this.isPresenterMode ? m.title : 'Cột mốc chưa khôi phục'
                }`,
              }),
            ]),
          ]
        );
        itemBtn.dataset.milestoneRoman = m.roman;

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
      groups.push(groupEl);
    }

    codexList.replaceChildren(...groups);

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
      const lockedDesc = createDomElement('p', { className: 'codex-detail-desc' }, [
        document.createTextNode('Khám phá không gian '),
        createDomElement('strong', { text: chapter.locationLabel }),
        document.createTextNode(
          ' và hoàn thành nhiệm vụ liên quan để khôi phục sự kiện lịch sử này vào Biên Niên Sử.'
        ),
      ]);

      detailEl.replaceChildren(
        createDomElement('div', { className: 'codex-detail-top' }, [
          createDomElement('span', {
            className: 'codex-detail-seal is-locked',
            text: `CỘT MỐC ${milestone.roman} · ${milestone.year}`,
          }),
          createDomElement('span', {
            className: 'codex-detail-status is-locked',
            text: '○ Chưa khôi phục',
          }),
        ]),
        createDomElement('h4', {
          className: 'codex-detail-title',
          text: 'Dấu Ấn Lịch Sử Chưa Mở Khóa',
        }),
        createDomElement('div', {
          className: 'codex-detail-meta',
          text: `${chapter.chapterNum} · ${chapter.locationLabel}`,
        }),
        lockedDesc,
        createDomElement('div', {
          className: 'codex-detail-cite',
          text: `◆ Tham chiếu: ${milestone.textbookCitation}`,
        })
      );
      return;
    }

    const quoteText = milestone.dialogue[0] ?? '';
    const nodes: Node[] = [
      createDomElement('div', { className: 'codex-detail-top' }, [
        createDomElement('span', {
          className: 'codex-detail-seal is-collected',
          text: `CỘT MỐC ${milestone.roman} · ${milestone.year}`,
        }),
        createDomElement('span', {
          className: 'crimson-seal-stamp',
          text: '✓ ĐÃ KHÔI PHỤC',
          attrs: { 'aria-label': 'Đã khôi phục' },
        }),
      ]),
      createDomElement('h4', {
        className: 'codex-detail-title',
        text: milestone.title,
      }),
      createDomElement('div', {
        className: 'codex-detail-meta',
        text: `${chapter.chapterNum} · ${milestone.npcSpeaker} (${milestone.npcRole})`,
      }),
    ];

    if (quoteText) {
      nodes.push(
        createDomElement('blockquote', {
          className: 'codex-detail-quote',
          text: quoteText,
        })
      );
    }

    nodes.push(
      createDomElement('p', {
        className: 'codex-detail-desc',
        text: milestone.historicalSignificance,
      }),
      createDomElement('div', {
        className: 'codex-detail-cite',
        text: `◆ Tư liệu: ${milestone.textbookCitation}`,
      })
    );

    detailEl.replaceChildren(...nodes);
  }
}
