/**
 * @file src/ui/HudController.ts
 * @description Điều phối toàn bộ giao diện (Chronicle Ribbon I-XII, GameHUD, DialoguePanel,
 * JournalPanel, InventoryPanel, WorldMapPanel, PausePanel, Climax A+B, và Cố vấn Sử học AI).
 */

import { AI_LOOKUP_DATABASE, CHAPTERS } from '../data/chronicles';
import type {
  ChapterData,
  DialogueChoice,
  DialogueNode,
  DossierPuzzleDefinition,
  EraId,
  EvidenceItem,
  GameMode,
  GameSettings,
  HudCallbacks,
  MilestoneArtifact,
  QuestDefinition,
  QuestObjective,
  QuestObjectiveStatus,
  RomanMilestoneId,
} from '../types/game';
import { DialoguePanel } from './DialoguePanel';
import { GameHUD } from './GameHUD';
import { InventoryPanel } from './InventoryPanel';
import { JournalPanel } from './JournalPanel';
import { PausePanel } from './PausePanel';
import { PuzzlePanel } from './PuzzlePanel';
import { WorldMapPanel } from './WorldMapPanel';

export type { GameMode, HudCallbacks };

export class HudController {
  private callbacks: HudCallbacks;
  public readonly gameHud: GameHUD;
  public readonly dialoguePanel: DialoguePanel;
  public readonly journalPanel: JournalPanel;
  public readonly inventoryPanel: InventoryPanel;
  public readonly pausePanel: PausePanel;
  public readonly puzzlePanel: PuzzlePanel;
  public readonly worldMapPanel: WorldMapPanel;

  constructor(callbacks: HudCallbacks) {
    this.callbacks = callbacks;
    this.gameHud = new GameHUD();
    this.dialoguePanel = new DialoguePanel({
      onSelectChoice: (choiceId) => {
        this.callbacks.onSelectDialogueChoice?.(choiceId);
      },
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'dialogue');
      },
    });
    this.journalPanel = new JournalPanel({
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'journal');
      },
    });
    this.inventoryPanel = new InventoryPanel({
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'inventory');
      },
    });
    this.pausePanel = new PausePanel({
      onUpdateSettings: (partial) => {
        this.callbacks.onUpdateSettings?.(partial);
      },
      onManualSave: () => {
        this.callbacks.onManualSave?.();
      },
      onResetSave: () => {
        this.callbacks.onResetSave?.();
      },
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'paused');
      },
    });
    this.puzzlePanel = new PuzzlePanel({
      onSubmitDossierPuzzle: (puzzleId, selectedOptions) =>
        this.callbacks.onSubmitDossierPuzzle?.(puzzleId, selectedOptions),
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'puzzle');
      },
    });
    this.worldMapPanel = new WorldMapPanel({
      onFastTravel: (zoneId, spawnId) => {
        this.callbacks.onOverlayStateChange?.(false, 'map');
        this.callbacks.onFastTravelToZone?.(zoneId, spawnId);
      },
      onClose: () => {
        this.callbacks.onOverlayStateChange?.(false, 'map');
      },
    });
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    // Chuyển chương trên Topbar
    document.querySelectorAll<HTMLButtonElement>('[data-era-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const eraId = btn.dataset.eraTab as EraId;
        if (eraId) this.callbacks.onSelectEra(eraId);
      });
    });

    // Nút Chế độ Giảng viên (Mở khóa toàn bộ 12 mốc La Mã I-XII)
    const lecturerBtn = document.getElementById('btn-lecturer-unlock');
    lecturerBtn?.addEventListener('click', () => {
      this.callbacks.onToggleLecturerUnlockAll();
    });

    // Nút Âm thanh 8-bit
    const muteBtn =
      document.getElementById('btn-audio-toggle') ?? document.getElementById('btn-mute-audio');
    muteBtn?.addEventListener('click', () => {
      const muted = this.callbacks.onToggleMute();
      if (muteBtn) {
        muteBtn.textContent = muted ? '🔇 Tắt tiếng' : '🔊 Ngũ Cung 8-Bit';
      }
    });

    // Các nút mở Overlay trên Topbar
    const lookupBtn = document.getElementById('btn-open-lookup');
    const closeLookupBtn = document.getElementById('btn-close-lookup');
    const closeClimaxBtn = document.getElementById('btn-close-climax');
    const openMapBtn = document.getElementById('btn-open-map');
    const openJournalBtn = document.getElementById('btn-open-journal');
    const openInventoryBtn = document.getElementById('btn-open-inventory');
    const openPauseBtn = document.getElementById('btn-open-pause');
    const debugBtn = document.getElementById('btn-debug-overlay');

    lookupBtn?.addEventListener('click', () => this.toggleLookupDrawer(true));
    closeLookupBtn?.addEventListener('click', () => this.toggleLookupDrawer(false));
    closeClimaxBtn?.addEventListener('click', () => this.closeClimaxModal());
    openMapBtn?.addEventListener('click', () => this.toggleWorldMapModal());
    openJournalBtn?.addEventListener('click', () => this.toggleJournalModal());
    openInventoryBtn?.addEventListener('click', () => this.toggleInventoryModal());
    openPauseBtn?.addEventListener('click', () => this.togglePauseModal());
    debugBtn?.addEventListener('click', () => {
      const active = this.callbacks.onToggleDebugOverlay?.() ?? false;
      debugBtn.classList.toggle('is-active', active);
      debugBtn.setAttribute('aria-pressed', String(active));
    });

    // Xử lý tìm kiếm trong Bàn Cố Vấn Sử Học
    const lookupInput = document.getElementById('lookup-search-input') as HTMLInputElement | null;
    lookupInput?.addEventListener('input', () => {
      this.renderLookupResults(lookupInput.value);
    });

    // Các thẻ gợi ý hỏi nhanh
    document.querySelectorAll<HTMLButtonElement>('[data-lookup-chip]').forEach((chip) => {
      chip.addEventListener('click', () => {
        const q = chip.dataset.lookupChip ?? '';
        if (lookupInput) {
          lookupInput.value = q;
          this.renderLookupResults(q);
        }
      });
    });

    this.renderLookupResults('');
  }

  public renderSidebarCodex(
    activeChapter: ChapterData,
    collected: Set<RomanMilestoneId>,
    activeMilestone: MilestoneArtifact | null
  ): void {
    const container = document.getElementById('codex-milestone-list');
    const progressEl = document.getElementById('codex-progress-count');
    if (progressEl) {
      progressEl.textContent = `${collected.size} / 12 CỘT MỐC`;
    }
    if (!container) return;

    container.innerHTML = '';

    for (const ch of CHAPTERS) {
      const groupEl = document.createElement('div');
      groupEl.className = 'codex-chapter-group';

      const headerEl = document.createElement('div');
      headerEl.className = `codex-chapter-header ${ch.id === activeChapter.id ? 'is-active' : ''}`;
      headerEl.textContent = `${ch.chapterNum} · ${ch.years}`;
      groupEl.appendChild(headerEl);

      for (const m of ch.milestones) {
        const isDone = collected.has(m.roman);
        const isCurrent = activeMilestone?.roman === m.roman;

        const itemBtn = document.createElement('button');
        itemBtn.type = 'button';
        itemBtn.className = `codex-item ${isDone ? 'is-collected' : ''} ${isCurrent ? 'is-current' : ''}`;
        itemBtn.innerHTML = `
          <span class="codex-roman">${isDone ? '✓' : ''} ${m.roman}</span>
          <span class="codex-dots" aria-hidden="true"></span>
          <span class="codex-title">${m.year} · ${m.title}</span>
        `;
        itemBtn.addEventListener('click', () => {
          this.callbacks.onTeleportToMilestone(m);
        });
        groupEl.appendChild(itemBtn);
      }

      container.appendChild(groupEl);
    }

    // Cập nhật trạng thái active trên thanh Topbar
    document.querySelectorAll<HTMLButtonElement>('[data-era-tab]').forEach((btn) => {
      btn.classList.toggle('is-active', btn.dataset.eraTab === activeChapter.id);
    });
  }

  public updateStageHeader(chapter: ChapterData, isSheltered: boolean): void {
    this.gameHud.updateStageHeader(chapter, isSheltered);
  }

  public updateMapLocationBadge(
    mapTitle: string,
    coordinates?: string,
    promptText?: string | null
  ): void {
    this.gameHud.updateMapLocationBadge(mapTitle, coordinates, promptText);
  }

  public showArtifactDialogue(
    milestone: MilestoneArtifact,
    node?: DialogueNode | null,
    choices?: DialogueChoice[]
  ): void {
    this.dialoguePanel.showArtifactDialogue(milestone, node, choices);
  }

  public openNpcDialogue(
    node: DialogueNode,
    choices: DialogueChoice[],
    milestone?: MilestoneArtifact
  ): void {
    this.dialoguePanel.openNpcDialogue(node, choices, milestone);
    this.callbacks.onOverlayStateChange?.(true, 'dialogue');
  }

  public updateDialogueNode(node: DialogueNode, choices: DialogueChoice[]): void {
    this.dialoguePanel.updateDialogueNode(node, choices);
  }

  public closeDialogueModal(): void {
    const wasOpen = this.dialoguePanel.isOpen();
    this.dialoguePanel.close();
    if (wasOpen) {
      this.callbacks.onOverlayStateChange?.(false, 'dialogue');
    }
  }

  public showToast(message: string): void {
    this.gameHud.showToast(message);
  }

  public openClimaxModal(chapter: ChapterData): void {
    const modal = document.getElementById('climax-modal');
    const stepABox = document.getElementById('climax-step-a');
    const stepBBox = document.getElementById('climax-step-b');
    const titleA = document.getElementById('climax-a-title');
    const promptA = document.getElementById('climax-a-prompt');
    const citeA = document.getElementById('climax-a-cite');
    const optionsContainer = document.getElementById('climax-a-options');
    const feedbackEl = document.getElementById('climax-a-feedback');

    if (!modal || !stepABox || !stepBBox || !optionsContainer || !feedbackEl) return;

    modal.classList.add('is-open');
    stepABox.hidden = false;
    stepABox.removeAttribute('hidden');
    stepBBox.hidden = true;
    stepBBox.setAttribute('hidden', '');
    feedbackEl.textContent = '';
    feedbackEl.className = 'climax-feedback';

    if (titleA) titleA.textContent = chapter.climax.stepATitle;
    if (promptA) promptA.textContent = chapter.climax.stepAPrompt;
    if (citeA) citeA.textContent = `📖 ${chapter.climax.stepACitation}`;

    optionsContainer.innerHTML = '';
    for (const opt of chapter.climax.options) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'climax-option-btn';
      btn.textContent = opt.text;
      btn.addEventListener('click', () => {
        feedbackEl.textContent = opt.feedback;
        if (opt.isCorrect) {
          feedbackEl.className = 'climax-feedback is-correct';
          this.revealClimaxStepB(chapter);
        } else {
          feedbackEl.className = 'climax-feedback is-retry';
        }
      });
      optionsContainer.appendChild(btn);
    }

    this.callbacks.onOverlayStateChange?.(true, 'climax');
  }

  private revealClimaxStepB(chapter: ChapterData): void {
    const stepBBox = document.getElementById('climax-step-b');
    const titleB = document.getElementById('climax-b-title');
    const subB = document.getElementById('climax-b-subtitle');
    const citeB = document.getElementById('climax-b-cite');
    const actionBtn = document.getElementById('climax-b-action-btn');

    if (!stepBBox || !actionBtn) return;
    stepBBox.hidden = false;
    stepBBox.removeAttribute('hidden');

    if (titleB) titleB.textContent = chapter.climax.stepBTitle;
    if (subB) subB.textContent = chapter.climax.stepBSubtitle;
    if (citeB) citeB.textContent = `📖 ${chapter.climax.stepBCitation}`;

    actionBtn.textContent = chapter.climax.stepBActionLabel;
    actionBtn.onclick = () => {
      this.closeClimaxModal();
      this.callbacks.onTriggerClimaxStepB();
    };
  }

  public closeClimaxModal(): void {
    const modal = document.getElementById('climax-modal');
    if (!modal) return;
    const wasOpen = modal.classList.contains('is-open');
    modal.classList.remove('is-open');
    if (wasOpen) {
      this.callbacks.onOverlayStateChange?.(false, 'exploring');
    }
  }

  public toggleLookupDrawer(forceOpen?: boolean): void {
    const drawer = document.getElementById('lookup-drawer');
    if (!drawer) return;
    const next = forceOpen ?? !drawer.classList.contains('is-open');
    drawer.classList.toggle('is-open', next);
    this.callbacks.onOverlayStateChange?.(next, next ? 'lookup' : 'exploring');
    if (next) {
      const input = document.getElementById('lookup-search-input') as HTMLInputElement | null;
      input?.focus();
    }
  }

  public toggleWorldMapModal(force?: boolean): void {
    const isOpen = this.worldMapPanel.toggle(force);
    this.callbacks.onOverlayStateChange?.(isOpen, 'map');
  }

  public toggleJournalModal(force?: boolean): void {
    const isOpen = this.journalPanel.toggle(force);
    this.callbacks.onOverlayStateChange?.(isOpen, 'journal');
  }

  public toggleInventoryModal(force?: boolean): void {
    const isOpen = this.inventoryPanel.toggle(force);
    this.callbacks.onOverlayStateChange?.(isOpen, 'inventory');
  }

  public togglePauseModal(force?: boolean): void {
    const isOpen = this.pausePanel.toggle(force);
    this.callbacks.onOverlayStateChange?.(isOpen, 'paused');
  }

  public openDossierPuzzleModal(
    puzzle: DossierPuzzleDefinition,
    attemptCount = 0
  ): void {
    this.puzzlePanel.open(puzzle, attemptCount);
    this.callbacks.onOverlayStateChange?.(true, 'puzzle');
  }

  public closeDossierPuzzleModal(): void {
    const wasOpen = this.puzzlePanel.isOpen();
    this.puzzlePanel.close();
    if (wasOpen) {
      this.callbacks.onOverlayStateChange?.(false, 'puzzle');
    }
  }

  public closeAllOverlays(): void {
    this.closeClimaxModal();
    this.closeDialogueModal();
    this.closeDossierPuzzleModal();
    if (document.getElementById('lookup-drawer')?.classList.contains('is-open')) {
      this.toggleLookupDrawer(false);
    }
    if (this.worldMapPanel.isOpen()) {
      this.worldMapPanel.toggle(false);
    }
    if (this.journalPanel.isOpen()) {
      this.journalPanel.toggle(false);
    }
    if (this.inventoryPanel.isOpen()) {
      this.inventoryPanel.toggle(false);
    }
    if (this.pausePanel.isOpen()) {
      this.pausePanel.toggle(false);
    }
    this.callbacks.onOverlayStateChange?.(false, 'exploring');
  }

  public setPresenterModeUI(isPresenterMode: boolean): void {
    const lecturerBtn = document.getElementById('btn-lecturer-unlock');
    if (lecturerBtn) {
      lecturerBtn.textContent = isPresenterMode
        ? '⚡ Chế Độ Giảng Viên: BẬT'
        : '⚡ Chế Độ Giảng Viên';
      lecturerBtn.classList.toggle('is-presenter-active', isPresenterMode);
      lecturerBtn.setAttribute('aria-pressed', String(isPresenterMode));
    }

    const codexSubSpan = document.querySelector<HTMLElement>('.codex-sub span:first-child');
    if (codexSubSpan) {
      codexSubSpan.textContent = isPresenterMode
        ? 'Chế độ Giảng Viên: Bấm để dịch chuyển'
        : 'Bấm mốc đã mở để xem tư liệu';
    }
  }

  public updateWorldMapState(
    currentMapId: string,
    discoveredMaps: ReadonlySet<string>,
    unlockedCheckpoints: ReadonlySet<string>,
    isPresenterMode = false
  ): void {
    this.worldMapPanel.updateState(
      currentMapId,
      discoveredMaps,
      unlockedCheckpoints,
      isPresenterMode
    );
  }

  public updateRpgSystemsState(data: {
    quests: Array<{
      quest: QuestDefinition;
      objectives: Array<{ objective: QuestObjective; status: QuestObjectiveStatus }>;
    }>;
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null;
    hintText: string;
    allEvidence: EvidenceItem[];
    collectedEvidenceIds: ReadonlySet<string>;
    settings: GameSettings;
    isPresenterMode: boolean;
  }): void {
    this.gameHud.updateObjectiveTracker(
      data.activeObjective,
      data.hintText,
      data.settings.showHints
    );
    this.journalPanel.updateState(
      data.quests,
      data.activeObjective,
      data.hintText,
      data.isPresenterMode
    );
    this.inventoryPanel.updateState(
      data.allEvidence,
      data.collectedEvidenceIds,
      data.isPresenterMode
    );
    this.pausePanel.updateSettings(data.settings);
  }

  public isOverlayOpen(): boolean {
    const climaxOpen =
      document.getElementById('climax-modal')?.classList.contains('is-open') ?? false;
    const lookupOpen =
      document.getElementById('lookup-drawer')?.classList.contains('is-open') ?? false;
    return (
      climaxOpen ||
      lookupOpen ||
      this.dialoguePanel.isOpen() ||
      this.worldMapPanel.isOpen() ||
      this.journalPanel.isOpen() ||
      this.inventoryPanel.isOpen() ||
      this.pausePanel.isOpen() ||
      this.puzzlePanel.isOpen()
    );
  }

  private renderLookupResults(query: string): void {
    const container = document.getElementById('lookup-results-list');
    if (!container) return;

    const normalized = query.trim().toLowerCase();
    const matched = normalized
      ? AI_LOOKUP_DATABASE.filter(
          (item) =>
            String(item.question).toLowerCase().includes(normalized) ||
            item.answer.toLowerCase().includes(normalized) ||
            item.keywords.some((k) => k.toLowerCase().includes(normalized))
        )
      : AI_LOOKUP_DATABASE;

    container.innerHTML = '';
    if (matched.length === 0) {
      container.innerHTML = `<p class="lookup-empty">Không tìm thấy từ khóa khớp. Hãy thử: "Cương lĩnh 1930", "Trung ương 8", "Điện Biên Phủ", "Đổi mới 1986", "2045".</p>`;
      return;
    }

    for (const entry of matched) {
      const card = document.createElement('div');
      card.className = 'lookup-entry-card';
      card.innerHTML = `
        <div class="lookup-entry-header">
          <span class="lookup-milestone-badge">MỐC ${entry.relatedMilestone}</span>
          <strong>${entry.question}</strong>
        </div>
        <p class="lookup-entry-answer">${entry.answer}</p>
        <div class="lookup-entry-cite">📖 ${entry.citation}</div>
      `;
      container.appendChild(card);
    }
  }
}
