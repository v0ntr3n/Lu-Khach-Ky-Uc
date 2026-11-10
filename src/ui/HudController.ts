/**
 * @file src/ui/HudController.ts
 * @description Điều phối toàn bộ hệ thống giao diện trong màn chơi (GameHUD, DialoguePanel,
 * JournalPanel + Biên Niên Sử I–XII, InventoryPanel, WorldMapPanel, PausePanel, SettingsMenu,
 * PuzzlePanel, Climax A+B, Cố vấn Sử học, và Ngăn điều khiển Giảng viên độc lập).
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
  StealthAlertState,
} from '../types/game';
import { DialoguePanel } from './DialoguePanel';
import { type DebugReadoutInfo, GameHUD } from './GameHUD';
import { InventoryPanel } from './InventoryPanel';
import { type JournalTabId, JournalPanel } from './JournalPanel';
import { PausePanel } from './PausePanel';
import { PuzzlePanel } from './PuzzlePanel';
import { SettingsMenu } from './SettingsMenu';
import { WorldMapPanel } from './WorldMapPanel';

export type { GameMode, HudCallbacks };

function isModalElementOpen(id: string): boolean {
  if (typeof document === 'undefined') return false;
  const el = document.getElementById(id);
  return Boolean(el && el.classList.contains('is-open') && !el.hidden);
}

function closeModalElement(id: string): boolean {
  if (typeof document === 'undefined') return false;
  const el = document.getElementById(id);
  if (!el) return false;
  const wasOpen = el.classList.contains('is-open') && !el.hidden;
  el.classList.remove('is-open');
  el.hidden = true;
  el.setAttribute('hidden', '');
  return wasOpen;
}

export class HudController {
  private callbacks: HudCallbacks;
  private isPresenterMode = false;
  private currentSettings: GameSettings = {
    muted: false,
    masterVolume: 0.85,
    reducedMotion: false,
    textScale: 'normal',
    showHints: true,
    cameraZoom: 'normal',
    effectsQuality: 'full',
    presenterModeUnlocked: false,
  };
  private lastFocusTrigger: HTMLElement | null = null;

  public readonly gameHud: GameHUD;
  public readonly dialoguePanel: DialoguePanel;
  public readonly journalPanel: JournalPanel;
  public readonly inventoryPanel: InventoryPanel;
  public readonly settingsMenu: SettingsMenu;
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
      onOpen: () => {
        if (this.journalPanel.isOpen()) {
          this.journalPanel.toggle(false);
        }
        if (this.isPresenterDrawerOpen()) {
          this.setPresenterDrawerState(false, false);
        }
        this.callbacks.onOverlayStateChange?.(true, 'dialogue');
      },
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'dialogue');
      },
    });

    this.journalPanel = new JournalPanel({
      onSelectObjective: (objectiveId) => {
        this.journalPanel.toggle(false);
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'journal');
        this.callbacks.onTrackObjective?.(objectiveId);
      },
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'journal');
      },
    });

    this.inventoryPanel = new InventoryPanel({
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'inventory');
      },
    });

    // Khởi tạo SettingsMenu trước PausePanel để gắn cờ sở hữu #toggle-* trong #settings-modal
    this.settingsMenu = new SettingsMenu({
      onUpdateSettings: (partial) => {
        this.handleSettingsPartialUpdate(partial);
      },
      onToggleFullscreen: () => this.handleToggleFullscreen(),
      onReplayTutorial: () => {
        if (!this.isMainMenuVisible()) {
          this.settingsMenu.close();
          this.restoreFocusAfterClose();
          this.callbacks.onOverlayStateChange?.(false, 'settings');
        }
        this.callbacks.onReplayTutorial?.();
      },
      onClose: () => {
        const mainMenuOpen = this.isMainMenuVisible();
        if (!mainMenuOpen) {
          this.restoreFocusAfterClose();
          this.callbacks.onOverlayStateChange?.(false, 'settings');
        }
      },
    });

    this.pausePanel = new PausePanel({
      onUpdateSettings: (partial) => {
        this.handleSettingsPartialUpdate(partial);
      },
      onManualSave: () => {
        this.callbacks.onManualSave?.();
      },
      onResetSave: () => {
        this.callbacks.onResetSave?.();
      },
      onReturnToMainMenu: () => {
        this.closeAllOverlays();
        this.callbacks.onReturnToMainMenu?.();
      },
      onOpenSettings: (triggerEl) => {
        if (this.pausePanel.isOpen()) {
          this.pausePanel.toggle(false);
        }
        this.toggleSettingsModal(true, triggerEl);
        this.callbacks.onOpenSettings?.();
      },
      onToggleFullscreen: () => this.handleToggleFullscreen(),
      onOpenPresenter: () => {
        this.togglePresenterDrawer(true);
      },
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'paused');
      },
    });

    this.puzzlePanel = new PuzzlePanel({
      onSubmitDossierPuzzle: (puzzleId, selectedOptions) =>
        this.callbacks.onSubmitDossierPuzzle?.(puzzleId, selectedOptions),
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'puzzle');
      },
    });

    this.worldMapPanel = new WorldMapPanel({
      onFastTravel: (zoneId, spawnId) => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'map');
        this.callbacks.onFastTravelToZone?.(zoneId, spawnId);
      },
      onClose: () => {
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'map');
      },
    });

    this.bindStaticEvents();
  }

  private isMainMenuVisible(): boolean {
    if (typeof document === 'undefined') return false;
    const menuEl = document.getElementById('main-menu-screen');
    return Boolean(menuEl && !menuEl.hidden);
  }

  private rememberTriggerFocus(): void {
    if (typeof document === 'undefined') return;
    const active = document.activeElement as HTMLElement | null;
    if (active && active !== document.body && !active.closest('[role="dialog"]')) {
      this.lastFocusTrigger = active;
    }
  }

  private restoreFocusAfterClose(): void {
    if (typeof document === 'undefined') return;
    if (
      this.lastFocusTrigger &&
      this.lastFocusTrigger.isConnected &&
      !this.lastFocusTrigger.closest('[hidden]')
    ) {
      this.lastFocusTrigger.focus({ preventScroll: true });
      this.lastFocusTrigger = null;
      return;
    }
    this.lastFocusTrigger = null;
    const active = document.activeElement as HTMLElement | null;
    if (active && active !== document.body && typeof active.blur === 'function') {
      active.blur();
    }
  }

  private handleSettingsPartialUpdate(partial: Partial<GameSettings>): void {
    this.currentSettings = { ...this.currentSettings, ...partial };
    this.settingsMenu.updateSettings(this.currentSettings);
    this.pausePanel.updateSettings(this.currentSettings, this.isPresenterMode);
    this.callbacks.onUpdateSettings?.(partial);
  }

  private async handleToggleFullscreen(): Promise<boolean> {
    if (this.callbacks.onToggleFullscreen) {
      const res = await this.callbacks.onToggleFullscreen();
      return Boolean(res);
    }
    if (typeof document === 'undefined') return false;
    try {
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen?.();
        return true;
      }
      await document.exitFullscreen?.();
      return false;
    } catch {
      return Boolean(document.fullscreenElement);
    }
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    // Chuyển chương trong Ngăn Giảng Viên (#presenter-drawer)
    document.querySelectorAll<HTMLButtonElement>('[data-era-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const eraId = btn.dataset.eraTab as EraId;
        if (eraId) this.callbacks.onSelectEra(eraId);
      });
    });

    // Nút Chế độ Giảng viên (Mở khóa toàn bộ 12 mốc La Mã I-XII độc lập)
    const lecturerBtn = document.getElementById('btn-lecturer-unlock');
    lecturerBtn?.addEventListener('click', () => {
      this.callbacks.onToggleLecturerUnlockAll();
    });

    // Nút Âm thanh dự phòng nếu tồn tại
    const muteBtn =
      document.getElementById('btn-audio-toggle') ?? document.getElementById('btn-mute-audio');
    muteBtn?.addEventListener('click', () => {
      const muted = this.callbacks.onToggleMute();
      this.currentSettings.muted = muted;
      this.settingsMenu.updateSettings(this.currentSettings);
      this.pausePanel.updateSettings(this.currentSettings, this.isPresenterMode);
      if (muteBtn) {
        muteBtn.textContent = muted ? '○ Đang tắt tiếng' : '◆ Ngũ cung 8-Bit';
      }
    });

    const lookupBtn = document.getElementById('btn-open-lookup');
    const closeLookupBtn = document.getElementById('btn-close-lookup');
    const closeClimaxBtn = document.getElementById('btn-close-climax');
    const openMapBtn = document.getElementById('btn-open-map');
    const openJournalBtn = document.getElementById('btn-open-journal');
    const openInventoryBtn = document.getElementById('btn-open-inventory');
    const openPauseBtn = document.getElementById('btn-open-pause');
    const closePresenterBtn = document.getElementById('btn-close-presenter');
    const debugBtn = document.getElementById('btn-debug-overlay');

    lookupBtn?.addEventListener('click', () => {
      if (this.pausePanel.isOpen()) this.pausePanel.toggle(false);
      this.toggleLookupDrawer(true);
    });
    closeLookupBtn?.addEventListener('click', () => this.toggleLookupDrawer(false));
    closeClimaxBtn?.addEventListener('click', () => this.closeClimaxModal());

    openMapBtn?.addEventListener('click', () => {
      if (this.pausePanel.isOpen()) this.pausePanel.toggle(false);
      this.toggleWorldMapModal(true);
    });
    openJournalBtn?.addEventListener('click', () => {
      if (this.pausePanel.isOpen()) this.pausePanel.toggle(false);
      this.toggleJournalModal(true);
    });
    openInventoryBtn?.addEventListener('click', () => {
      if (this.pausePanel.isOpen()) this.pausePanel.toggle(false);
      this.toggleInventoryModal(true);
    });
    openPauseBtn?.addEventListener('click', () => this.togglePauseModal());
    closePresenterBtn?.addEventListener('click', () => this.togglePresenterDrawer(false));

    const objectiveTrackerEl = document.getElementById('hud-objective-tracker');
    objectiveTrackerEl?.addEventListener('click', () => {
      if (this.pausePanel.isOpen()) this.pausePanel.toggle(false);
      this.toggleJournalModal(true, 'quests');
    });

    debugBtn?.addEventListener('click', () => {
      const active = this.callbacks.onToggleDebugOverlay?.() ?? false;
      this.setDebugOverlayUI(active);
    });

    // Đóng dự phòng cho #instructions-modal / #multiplayer-modal nếu MainMenu không khởi tạo cùng
    const closeInstrBtn = document.getElementById('btn-close-instructions');
    if (closeInstrBtn && closeInstrBtn.dataset.boundByMainMenu !== 'true') {
      closeInstrBtn.addEventListener('click', () => {
        closeModalElement('instructions-modal');
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'exploring');
      });
    }

    const closeMpBtn = document.getElementById('btn-close-multiplayer');
    if (closeMpBtn && closeMpBtn.dataset.boundByMainMenu !== 'true') {
      closeMpBtn.addEventListener('click', () => {
        closeModalElement('multiplayer-modal');
        this.restoreFocusAfterClose();
        this.callbacks.onOverlayStateChange?.(false, 'exploring');
      });
    }

    // Xử lý tìm kiếm trong Bàn Cố Vấn Sử Học
    const lookupInput = document.getElementById(
      'lookup-search-input'
    ) as HTMLInputElement | null;
    lookupInput?.addEventListener('input', () => {
      this.renderLookupResults(lookupInput.value);
    });

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

  public setHudVisible(visible: boolean): void {
    this.gameHud.setVisible(visible);
  }

  public renderSidebarCodex(
    activeChapter: ChapterData,
    collected: Set<RomanMilestoneId>,
    activeMilestone: MilestoneArtifact | null
  ): void {
    this.journalPanel.updateCodex(activeChapter, collected, activeMilestone, (m) => {
      this.callbacks.onTeleportToMilestone(m);
    });

    if (typeof document === 'undefined') return;

    // Đồng bộ danh sách mốc lịch sử trong Ngăn điều khiển Giảng viên (#presenter-codex-list)
    const presenterList = document.getElementById('presenter-codex-list');
    if (presenterList) {
      presenterList.innerHTML = '';
      for (const ch of CHAPTERS) {
        const groupEl = document.createElement('div');
        groupEl.className = 'codex-chapter-group';

        const headerEl = document.createElement('div');
        headerEl.className = `codex-chapter-header ${
          ch.id === activeChapter.id ? 'is-active' : ''
        }`;
        headerEl.textContent = `${ch.chapterNum} · ${ch.years}`;
        groupEl.appendChild(headerEl);

        const itemsBox = document.createElement('div');
        itemsBox.className = 'codex-chapter-items';

        for (const m of ch.milestones) {
          const isDone = collected.has(m.roman);
          const isCurrent = activeMilestone?.roman === m.roman;

          const itemBtn = document.createElement('button');
          itemBtn.type = 'button';
          itemBtn.className = `codex-item ${isDone ? 'is-collected' : 'is-locked'} ${
            isCurrent ? 'is-current' : ''
          }`;
          itemBtn.innerHTML = `
            <span class="codex-roman">${isDone ? '✓ ' : ''}${m.roman}</span>
            <span class="codex-title">${m.year} · ${m.title}</span>
          `;
          itemBtn.addEventListener('click', () => {
            this.callbacks.onTeleportToMilestone(m);
          });
          itemsBox.appendChild(itemBtn);
        }

        groupEl.appendChild(itemsBox);
        presenterList.appendChild(groupEl);
      }
    }

    document.querySelectorAll<HTMLButtonElement>('[data-era-tab]').forEach((btn) => {
      btn.classList.toggle('is-active', btn.dataset.eraTab === activeChapter.id);
    });
  }

  public updateStageHeader(
    chapter: ChapterData,
    isSheltered: boolean,
    stealthAlertState?: StealthAlertState
  ): void {
    this.gameHud.updateStageHeader(chapter, isSheltered, stealthAlertState);
  }

  public updateMapLocationBadge(
    mapTitle: string,
    coordinates?: string,
    promptText?: string | null
  ): void {
    this.gameHud.updateMapLocationBadge(mapTitle, coordinates, promptText);
  }

  public updateInteractionPrompt(
    promptText: string | null,
    screenX?: number,
    screenY?: number,
    shellW?: number,
    shellH?: number
  ): void {
    this.gameHud.updateInteractionPrompt(promptText, screenX, screenY, shellW, shellH);
  }

  public setDebugOverlayUI(enabled: boolean, info?: DebugReadoutInfo): void {
    if (typeof document !== 'undefined') {
      const debugBtn = document.getElementById('btn-debug-overlay');
      debugBtn?.classList.toggle('is-active', enabled);
      debugBtn?.setAttribute('aria-pressed', String(enabled));
    }
    this.gameHud.updateDebugReadout(enabled, info);
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
  }

  public updateDialogueNode(node: DialogueNode, choices: DialogueChoice[]): void {
    this.dialoguePanel.updateDialogueNode(node, choices);
  }

  public closeDialogueModal(): void {
    const wasOpen = this.dialoguePanel.isOpen();
    this.dialoguePanel.close();
    if (wasOpen) {
      this.restoreFocusAfterClose();
      this.callbacks.onOverlayStateChange?.(false, 'dialogue');
    }
  }

  public showToast(message: string): void {
    this.gameHud.showToast(message);
  }

  public showRegionEntryBanner(zoneTitle: string, eraLabel: string): void {
    this.gameHud.showRegionEntryBanner(zoneTitle, eraLabel);
  }

  public openClimaxModal(chapter: ChapterData): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('climax-modal');
    const stepABox = document.getElementById('climax-step-a');
    const stepBBox = document.getElementById('climax-step-b');
    const titleA = document.getElementById('climax-a-title');
    const promptA = document.getElementById('climax-a-prompt');
    const citeA = document.getElementById('climax-a-cite');
    const optionsContainer = document.getElementById('climax-a-options');
    const feedbackEl = document.getElementById('climax-a-feedback');

    if (!modal || !stepABox || !stepBBox || !optionsContainer || !feedbackEl) return;

    this.rememberTriggerFocus();
    modal.classList.add('is-open');
    modal.hidden = false;
    modal.removeAttribute('hidden');

    stepABox.hidden = false;
    stepABox.removeAttribute('hidden');
    stepBBox.hidden = true;
    stepBBox.setAttribute('hidden', '');
    feedbackEl.textContent = '';
    feedbackEl.className = 'climax-feedback';

    if (titleA) titleA.textContent = chapter.climax.stepATitle;
    if (promptA) promptA.textContent = chapter.climax.stepAPrompt;
    if (citeA) { citeA.textContent = ''; citeA.hidden = true; }

    optionsContainer.innerHTML = '';
    chapter.climax.options.forEach((opt, idx) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'climax-option-btn';
      btn.textContent = `[${idx + 1}] ${opt.text}`;
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
    });

    this.callbacks.onOverlayStateChange?.(true, 'climax');
  }

  private revealClimaxStepB(chapter: ChapterData): void {
    if (typeof document === 'undefined') return;
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
    if (citeB) { citeB.textContent = ''; citeB.hidden = true; }

    actionBtn.textContent = chapter.climax.stepBActionLabel;
    actionBtn.onclick = () => {
      this.closeClimaxModal();
      this.callbacks.onTriggerClimaxStepB();
    };
  }

  public closeClimaxModal(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('climax-modal');
    if (!modal) return;
    const wasOpen = modal.classList.contains('is-open') && !modal.hidden;
    modal.classList.remove('is-open');
    modal.hidden = true;
    modal.setAttribute('hidden', '');
    if (wasOpen) {
      this.restoreFocusAfterClose();
      this.callbacks.onOverlayStateChange?.(false, 'exploring');
    }
  }

  public toggleLookupDrawer(forceOpen?: boolean): void {
    if (typeof document === 'undefined') return;
    const drawer = document.getElementById('lookup-drawer');
    if (!drawer) return;
    const currentlyOpen = drawer.classList.contains('is-open') && !drawer.hidden;
    const next = forceOpen ?? !currentlyOpen;

    if (next) {
      this.rememberTriggerFocus();
      drawer.hidden = false;
      drawer.removeAttribute('hidden');
      drawer.classList.add('is-open');
    } else {
      drawer.classList.remove('is-open');
      drawer.hidden = true;
      drawer.setAttribute('hidden', '');
      this.restoreFocusAfterClose();
    }

    this.callbacks.onOverlayStateChange?.(next, next ? 'lookup' : 'exploring');
    if (next) {
      const input = document.getElementById(
        'lookup-search-input'
      ) as HTMLInputElement | null;
      input?.focus({ preventScroll: true });
    }
  }

  public isPresenterDrawerOpen(): boolean {
    return isModalElementOpen('presenter-drawer');
  }

  private setPresenterDrawerState(next: boolean, notifyCallback: boolean): boolean {
    if (typeof document === 'undefined') return false;
    const drawer = document.getElementById('presenter-drawer');
    if (!drawer) return false;

    if (next && !this.currentSettings.presenterModeUnlocked && !this.isPresenterMode) {
      return false;
    }

    if (next) {
      this.rememberTriggerFocus();
      drawer.hidden = false;
      drawer.removeAttribute('hidden');
      drawer.classList.add('is-open');
      const closeBtn = document.getElementById('btn-close-presenter');
      closeBtn?.focus({ preventScroll: true });
    } else {
      drawer.classList.remove('is-open');
      drawer.hidden = true;
      drawer.setAttribute('hidden', '');
      this.restoreFocusAfterClose();
    }

    if (notifyCallback) {
      this.callbacks.onOverlayStateChange?.(next, next ? 'presenter' : 'exploring');
    }
    return next;
  }

  public togglePresenterDrawer(force?: boolean): boolean {
    const next = force ?? !this.isPresenterDrawerOpen();
    return this.setPresenterDrawerState(next, true);
  }

  public toggleSettingsModal(force?: boolean, triggerEl?: HTMLElement | null): boolean {
    const next = force ?? !this.settingsMenu.isOpen();
    if (next) {
      this.settingsMenu.open(triggerEl);
    } else {
      this.settingsMenu.close();
    }
    const isOpen = this.settingsMenu.isOpen();
    if (!this.isMainMenuVisible()) {
      this.callbacks.onOverlayStateChange?.(isOpen, isOpen ? 'settings' : 'exploring');
    }
    return isOpen;
  }

  public toggleWorldMapModal(force?: boolean): void {
    if (force !== false && !this.worldMapPanel.isOpen()) {
      this.rememberTriggerFocus();
    }
    const isOpen = this.worldMapPanel.toggle(force);
    if (!isOpen) this.restoreFocusAfterClose();
    this.callbacks.onOverlayStateChange?.(isOpen, 'map');
  }

  public toggleJournalModal(force?: boolean, tab?: JournalTabId): void {
    if (force !== false && !this.journalPanel.isOpen()) {
      this.rememberTriggerFocus();
    }
    const isOpen = this.journalPanel.toggle(force, tab);
    if (!isOpen) this.restoreFocusAfterClose();
    this.callbacks.onOverlayStateChange?.(isOpen, 'journal');
  }

  public toggleInventoryModal(force?: boolean): void {
    if (force !== false && !this.inventoryPanel.isOpen()) {
      this.rememberTriggerFocus();
    }
    const isOpen = this.inventoryPanel.toggle(force);
    if (!isOpen) this.restoreFocusAfterClose();
    this.callbacks.onOverlayStateChange?.(isOpen, 'inventory');
  }

  public togglePauseModal(force?: boolean): void {
    if (force !== false && !this.pausePanel.isOpen()) {
      this.rememberTriggerFocus();
    }
    const isOpen = this.pausePanel.toggle(force);
    if (!isOpen) this.restoreFocusAfterClose();
    this.callbacks.onOverlayStateChange?.(isOpen, 'paused');
  }

  public openDossierPuzzleModal(
    puzzle: DossierPuzzleDefinition,
    attemptCount = 0
  ): void {
    this.rememberTriggerFocus();
    this.puzzlePanel.open(puzzle, attemptCount);
    this.callbacks.onOverlayStateChange?.(true, 'puzzle');
  }

  public closeDossierPuzzleModal(): void {
    const wasOpen = this.puzzlePanel.isOpen();
    this.puzzlePanel.close();
    if (wasOpen) {
      this.restoreFocusAfterClose();
      this.callbacks.onOverlayStateChange?.(false, 'puzzle');
    }
  }

  public closeAllOverlays(): void {
    this.closeClimaxModal();
    this.dialoguePanel.close();
    this.puzzlePanel.close();

    if (isModalElementOpen('lookup-drawer')) {
      const drawer = document.getElementById('lookup-drawer');
      if (drawer) {
        drawer.classList.remove('is-open');
        drawer.hidden = true;
        drawer.setAttribute('hidden', '');
      }
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
    if (this.settingsMenu.isOpen()) {
      this.settingsMenu.close();
    }
    if (this.isPresenterDrawerOpen()) {
      this.setPresenterDrawerState(false, false);
    }

    closeModalElement('instructions-modal');
    closeModalElement('multiplayer-modal');
    closeModalElement('confirm-modal');

    this.restoreFocusAfterClose();
    if (!this.isMainMenuVisible()) {
      this.callbacks.onOverlayStateChange?.(false, 'exploring');
    }
  }

  public setPresenterModeUI(isPresenterMode: boolean): void {
    this.isPresenterMode = isPresenterMode;
    this.pausePanel.setPresenterMode(isPresenterMode);

    if (typeof document === 'undefined') return;
    const lecturerBtn = document.getElementById('btn-lecturer-unlock');
    if (lecturerBtn) {
      lecturerBtn.textContent = isPresenterMode
        ? '◆ Mở khóa 12 mốc (Giảng viên): Đang bật'
        : '○ Mở khóa 12 mốc (Giảng viên): Đang tắt';
      lecturerBtn.classList.toggle('is-presenter-active', isPresenterMode);
      lecturerBtn.setAttribute('aria-pressed', String(isPresenterMode));
    }

    const codexSubSpan = document.querySelector<HTMLElement>('.codex-sub span:first-child');
    if (codexSubSpan) {
      codexSubSpan.textContent = isPresenterMode
        ? 'Chế độ Giảng viên: Bấm cột mốc bất kỳ để dịch chuyển'
        : 'Bấm cột mốc đã khôi phục để đối chiếu sử liệu';
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

  public updateSettings(settings: GameSettings, isFullscreen?: boolean): void {
    this.currentSettings = { ...this.currentSettings, ...settings };
    this.settingsMenu.updateSettings(this.currentSettings, isFullscreen);
    this.pausePanel.updateSettings(this.currentSettings, this.isPresenterMode);
  }

  public updateRpgSystemsState(data: {
    quests: Array<{
      quest: QuestDefinition;
      objectives: Array<{
        objective: QuestObjective;
        status: QuestObjectiveStatus;
        prerequisiteText?: string;
      }>;
    }>;
    activeObjective: { quest: QuestDefinition; objective: QuestObjective } | null;
    trackedObjectiveId?: string | null;
    hintText: string;
    nextActionText?: string;
    allEvidence: EvidenceItem[];
    collectedEvidenceIds: ReadonlySet<string>;
    settings: GameSettings;
    isPresenterMode: boolean;
  }): void {
    this.isPresenterMode = data.isPresenterMode;
    this.currentSettings = { ...this.currentSettings, ...data.settings };

    this.gameHud.updateObjectiveTracker(
      data.activeObjective,
      data.hintText,
      data.settings.showHints,
      data.nextActionText
    );
    this.journalPanel.updateState(
      data.quests,
      data.activeObjective,
      data.hintText,
      data.isPresenterMode,
      data.trackedObjectiveId ?? null,
      data.settings.showHints
    );
    this.inventoryPanel.updateState(
      data.allEvidence,
      data.collectedEvidenceIds,
      data.isPresenterMode
    );
    this.settingsMenu.updateSettings(this.currentSettings);
    this.pausePanel.updateSettings(this.currentSettings, data.isPresenterMode);
  }

  public isOverlayOpen(): boolean {
    return (
      isModalElementOpen('climax-modal') ||
      isModalElementOpen('lookup-drawer') ||
      isModalElementOpen('instructions-modal') ||
      isModalElementOpen('multiplayer-modal') ||
      isModalElementOpen('confirm-modal') ||
      this.dialoguePanel.isOpen() ||
      this.worldMapPanel.isOpen() ||
      this.journalPanel.isOpen() ||
      this.inventoryPanel.isOpen() ||
      this.pausePanel.isOpen() ||
      this.puzzlePanel.isOpen() ||
      this.settingsMenu.isOpen() ||
      this.isPresenterDrawerOpen()
    );
  }

  private renderLookupResults(query: string): void {
    if (typeof document === 'undefined') return;
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
        
      `;
      container.appendChild(card);
    }
  }
}
