/**
 * @file src/core/GameApp.ts
 * @description Bộ điều phối vòng đời chính của "Lữ Khách Ký Ức (1930 – 2045)".
 * Quản lý máy trạng thái BOOT → MAIN_MENU → LOADING → OPENING → PLAYING → PAUSED → MAIN_MENU,
 * kết nối GameShell (100vw × 100dvh integer pixel scaling), FullscreenManager, MainMenu,
 * LoadingScreen, OpeningFracture, GameStore, InputManager, SaveManager, CameraController,
 * SceneTransition, CollisionWorld, InteractionSystem, QuestSystem, DialogueSystem,
 * EvidenceSystem, InventorySystem, StealthSystem, PixelRenderer, SoundSynth và HudController.
 */

import { getEraPuzzleForMapOrId } from '../data/chapters/ch2_3_4_mechanics';
import { CHAPTERS } from '../data/chronicles';
import { OpeningFractureController } from '../engine/OpeningFracture';
import {
  INTERNAL_HEIGHT,
  INTERNAL_WIDTH,
  PixelRenderer,
  type RenderState,
} from '../engine/PixelRenderer';
import { SoundSynth } from '../engine/SoundSynth';
import { CameraController } from '../runtime/CameraController';
import { getAllBuiltInMaps, getBuiltInMap } from '../runtime/MapLoader';
import {
  computeEntityDepth,
  resolveObjectiveTarget,
} from '../runtime/PhaserRuntime';
import { SceneTransition } from '../runtime/SceneTransition';
import { DialogueSystem } from '../systems/DialogueSystem';
import { EvidenceSystem } from '../systems/EvidenceSystem';
import { InteractionSystem, type InteractionCandidate } from '../systems/InteractionSystem';
import { InventorySystem } from '../systems/InventorySystem';
import { QuestSystem } from '../systems/QuestSystem';
import { StealthSystem } from '../systems/StealthSystem';
import type {
  AppLifecycleState,
  ChapterData,
  DossierPuzzleDefinition,
  EraId,
  Facing4,
  GameMode,
  GameSettings,
  MilestoneArtifact,
  NormalizedMapData,
  ObjectiveTargetResolution,
  PatrolActorRenderState,
  PortalDefinition,
  Rect2D,
  RomanMilestoneId,
  SaveSummary,
  StealthAlertState,
  Vec2,
  ViewportMetrics,
} from '../types/game';
import { FullscreenManager } from '../ui/FullscreenManager';
import { GameShell } from '../ui/GameShell';
import { HudController } from '../ui/HudController';
import { LoadingScreen } from '../ui/LoadingScreen';
import { MainMenu } from '../ui/MainMenu';
import { MiniMapPanel } from '../ui/MiniMapPanel';
import { TutorialController } from '../ui/TutorialController';
import { CollisionWorld } from '../world/CollisionWorld';
import { RoutePlanner, type TrackedRouteState } from '../world/GridPathfinder';
import { GameStore } from './GameStore';
import { InputManager } from './InputManager';
import { SaveManager } from './SaveManager';

export class LuKhachKyUcGame {
  public readonly store: GameStore;
  public readonly input: InputManager;
  public readonly saveManager: SaveManager;
  public readonly renderer: PixelRenderer;
  public readonly sound: SoundSynth;
  public readonly gameShell: GameShell;
  public readonly fullscreenManager: FullscreenManager;
  public readonly loadingScreen: LoadingScreen;
  public readonly openingFracture: OpeningFractureController;
  public readonly hud: HudController;
  public readonly mainMenu: MainMenu;
  public readonly miniMap: MiniMapPanel;
  public readonly routePlanner: RoutePlanner;
  public readonly tutorialController: TutorialController;
  public readonly interactionSystem: InteractionSystem;
  public readonly questSystem: QuestSystem;
  public readonly dialogueSystem: DialogueSystem;
  public readonly evidenceSystem: EvidenceSystem;
  public readonly inventorySystem: InventorySystem;
  public readonly stealthSystem: StealthSystem;
  public readonly cameraController: CameraController;
  public readonly sceneTransition: SceneTransition;

  public allMaps: Record<string, NormalizedMapData>;
  public currentMap: NormalizedMapData;
  public collisionWorld: CollisionWorld;

  public playerX = 85;
  public playerY = 384;
  public cameraX = 0;
  public cameraY = 294;
  public facingRight = true;
  public playerFacing: Facing4 = 'right';
  public isCrouching = false;
  public useLegacyRenderer = false;

  public climaxBurstProgress = 0;
  public isBurstAnimating = false;
  public transitionFadeAlpha = 0;
  private activeBurstToken = 0;
  private postBurstDelaySec = 0;

  private lastFrameMs = performance.now();
  private smoothedFps = 60;
  private stepSoundCooldown = 0;
  private hazardCooldown = 0;
  private npcRouteStates = new Map<
    string,
    { waypointIndex: number; waitRemainingSec: number }
  >();
  private puzzleAttemptCounts = new Map<string, number>();

  constructor() {
    const canvas = document.getElementById('pixel-canvas') as HTMLCanvasElement | null;
    if (!canvas) {
      throw new Error('Không tìm thấy phần tử #pixel-canvas.');
    }

    let urlUnlockPresenter = false;
    if (typeof window !== 'undefined' && window.location?.search) {
      const params = new URLSearchParams(window.location.search);
      this.useLegacyRenderer = params.get('renderer') === 'legacy';
      urlUnlockPresenter = params.get('presenter') === '1';
    }

    this.store = new GameStore();
    this.store.setMode('boot');

    this.saveManager = new SaveManager();
    const initialSettings = this.saveManager.loadSettings();
    if (urlUnlockPresenter) {
      initialSettings.presenterModeUnlocked = true;
    }
    this.store.settings = this.saveManager.normalizeSettings(initialSettings);

    this.interactionSystem = new InteractionSystem();
    this.questSystem = new QuestSystem();
    this.dialogueSystem = new DialogueSystem();
    this.evidenceSystem = new EvidenceSystem();
    this.inventorySystem = new InventorySystem(this.evidenceSystem);
    this.stealthSystem = new StealthSystem();
    this.routePlanner = new RoutePlanner();

    this.allMaps = getAllBuiltInMaps();
    this.currentMap = getBuiltInMap('harbor_square');
    this.collisionWorld = new CollisionWorld(
      this.currentMap.obstacles,
      this.currentMap.shelters,
      {
        x: 0,
        y: 0,
        width: this.currentMap.widthPx,
        height: this.currentMap.heightPx,
      }
    );
    this.stealthSystem.initForMap(this.currentMap);

    this.renderer = new PixelRenderer(canvas);
    this.sound = new SoundSynth();
    this.sound.setMuted(this.store.settings.muted);
    this.sound.setVolume(this.store.settings.masterVolume ?? 0.85);

    this.cameraController = new CameraController({
      worldWidth: this.currentMap.widthPx,
      worldHeight: this.currentMap.heightPx,
      viewportWidth: INTERNAL_WIDTH,
      viewportHeight: INTERNAL_HEIGHT,
    });
    this.sceneTransition = new SceneTransition('harbor_square', 'default_spawn', 0.65);

    this.gameShell = new GameShell({
      shellSelector: '#game-shell',
      canvas,
      initialZoom: this.store.settings.cameraZoom ?? 'normal',
      onViewportChange: (metrics) => {
        this.handleViewportChange(metrics);
      },
    });

    const shellElement =
      typeof document !== 'undefined'
        ? document.getElementById('game-shell')
        : null;
    this.fullscreenManager = new FullscreenManager(shellElement, (fsState) => {
      const isActive = fsState.isBrowserFullscreen || fsState.isViewportFallback;
      this.hud?.updateSettings(this.store.settings, isActive);
      this.gameShell?.refresh(true);
    });

    this.loadingScreen = new LoadingScreen();
    this.openingFracture = new OpeningFractureController(
      '#rift-opening-overlay',
      '#rift-opening-canvas'
    );

    this.store.setMapState(
      this.currentMap.id,
      'default_spawn',
      { x: this.playerX, y: this.playerY },
      this.playerFacing
    );
    this.syncCameraBoundsAndSnap();

    this.miniMap = new MiniMapPanel({
      onToggleMinimap: (visible) => {
        this.updateSettings({ showMinimap: visible });
      },
      onOpenWorldMap: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        this.tutorialController?.recordAction('map');
        this.syncHudWithState();
        this.hud.toggleWorldMapModal(true);
      },
    });

    this.input = new InputManager({
      onInteract: () => {
        if (!this.isInGameplaySession()) return;
        this.handleInteractAction();
      },
      onToggleLookup: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        this.hud.toggleLookupDrawer();
      },
      onToggleMap: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        this.tutorialController?.recordAction('map');
        this.syncHudWithState();
        this.hud.toggleWorldMapModal();
      },
      onToggleJournal: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        this.tutorialController?.recordAction('journal');
        this.syncHudWithState();
        this.hud.toggleJournalModal();
      },
      onToggleInventory: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        this.syncHudWithState();
        this.hud.toggleInventoryModal();
      },
      onTogglePause: () => {
        if (!this.canOpenGameplayHotkeyOverlay()) return;
        if (this.hud.dialoguePanel.isOpen()) {
          this.hud.dialoguePanel.toggleLoreDrawer();
          return;
        }
        this.tutorialController?.recordAction('hint');
        this.syncHudWithState();
        this.hud.togglePauseModal();
      },
      onToggleDebugOverlay: () => {
        if (!this.isInGameplaySession()) return;
        if (
          !this.store.settings.presenterModeUnlocked &&
          !this.store.isPresenterMode &&
          !this.store.debugOverlayEnabled
        ) {
          return;
        }
        this.toggleDebugOverlay();
      },
      onToggleHints: () => {
        if (!this.isInGameplaySession()) return;
        this.tutorialController?.recordAction('hint');
        const nextShowHints = !this.store.settings.showHints;
        this.updateSettings({ showHints: nextShowHints });
        this.hud.showToast(
          nextShowHints
            ? '◆ Đã bật hiển thị Gợi ý Nhiệm vụ [H].'
            : '○ Đã tắt hiển thị Gợi ý Nhiệm vụ [H].'
        );
      },
      onToggleMinimap: () => {
        if (!this.isInGameplaySession()) return;
        const visible = this.miniMap.toggle();
        this.hud.showToast(
          visible
            ? '◆ Đã mở Bản đồ thu nhỏ [N].'
            : '○ Đã thu gọn Bản đồ thu nhỏ [N].'
        );
      },
      onToggleFullscreen: () => {
        void this.toggleFullscreen();
      },
      onTogglePresenterShortcut: () => {
        this.handlePresenterShortcut();
      },
      onNumberKey: (num: 1 | 2 | 3 | 4) => {
        if (!this.isInGameplaySession()) return;
        this.handleNumberKeyShortcut(num);
      },
      onEscape: () => {
        if (!this.isInGameplaySession()) {
          return;
        }
        if (this.hud.isOverlayOpen()) {
          this.dialogueSystem.closeDialogue();
          this.hud.closeAllOverlays();
          return;
        }
        this.tutorialController?.recordAction('hint');
        this.syncHudWithState();
        this.hud.togglePauseModal(true);
      },
    });

    this.tutorialController = new TutorialController({
      onTutorialCompletedChange: (completed: boolean) => {
        this.updateSettings({ tutorialCompleted: completed });
        if (completed) {
          this.hud?.showToast(
            '✓ Đã hoàn tất hướng dẫn tân thủ! Bạn có thể xem lại bất cứ lúc nào trong Cài đặt.'
          );
        }
      },
      onPlayUiSound: (kind) => {
        if (kind === 'confirm') {
          this.sound.playMenuConfirm();
        } else {
          this.sound.playMenuCancel();
        }
      },
    });

    this.hud = new HudController({
      onSelectEra: (eraId) => this.switchEra(eraId),
      onTeleportToMilestone: (m) => this.teleportToMilestone(m),
      onTriggerClimaxStepB: () => this.triggerClimaxStepBSetPiece(),
      onAdvanceNextChapter: () => this.advanceToNextChapter(),
      onToggleLecturerUnlockAll: () => this.togglePresenterMode(),
      onToggleMute: () => {
        const muted = this.sound.toggleMute();
        this.updateSettings({ muted });
        return muted;
      },
      onOverlayStateChange: (isOpen: boolean, overlayMode: GameMode) => {
        if (overlayMode === 'journal' && isOpen) {
          this.tutorialController.recordAction('journal');
        } else if (overlayMode === 'map' && isOpen) {
          this.tutorialController.recordAction('map');
        } else if (overlayMode === 'paused' && isOpen) {
          this.tutorialController.recordAction('hint');
        }
        this.tutorialController.setOverlaySuppressed(isOpen);

        if (
          this.store.getLifecycle() === 'MAIN_MENU' ||
          this.store.getLifecycle() === 'LOADING'
        ) {
          this.input.clearKeys();
          return;
        }
        this.store.setMode(isOpen ? overlayMode : 'exploring');
        this.input.clearKeys();
        if (isOpen) {
          this.hud.updateInteractionPrompt(null);
        }
      },
      onFastTravelToZone: (zoneId: string, spawnId: string) => {
        this.fastTravelToZone(zoneId, spawnId);
      },
      onToggleDebugOverlay: () => this.toggleDebugOverlay(),
      onSelectDialogueChoice: (choiceId: string) => {
        this.selectDialogueChoice(choiceId);
      },
      onManualSave: () => {
        this.manualSave();
      },
      onResetSave: () => {
        this.resetSave();
      },
      onReturnToMainMenu: () => {
        this.returnToMainMenu();
      },
      onToggleFullscreen: () => this.toggleFullscreen(),
      onUpdateSettings: (partial: Partial<GameSettings>) => {
        this.updateSettings(partial);
      },
      onReplayTutorial: () => {
        this.hud.closeAllOverlays();
        if (this.isInGameplaySession()) {
          this.store.setMode('exploring');
          this.tutorialController.setGameplayVisible(true);
          this.tutorialController.setOverlaySuppressed(false);
        }
        this.tutorialController.replay();
        this.hud.showToast('◆ Đã mở lại Hướng dẫn Tân thủ từ Bước 1/5.');
      },
      onTrackObjective: (objectiveId: string | null) => {
        this.trackObjective(objectiveId);
      },
      onSubmitDossierPuzzle: (
        puzzleId: string,
        selectedOptionIds: Record<string, string>
      ) => this.submitDossierPuzzle(puzzleId, selectedOptionIds),
    });

    this.hud.updateSettings(
      this.store.settings,
      this.fullscreenManager.isFullscreenActive()
    );
    this.hud.setPresenterModeUI(this.store.isPresenterMode);
    this.syncHudWithState();

    this.mainMenu = new MainMenu({
      onStartNewGame: (confirmedOverwrite: boolean) => {
        void this.startNewGame(confirmedOverwrite);
      },
      onContinueGame: () => {
        void this.continueGame();
      },
      onOpenSettings: () => {
        this.hud.updateSettings(
          this.store.settings,
          this.fullscreenManager.isFullscreenActive()
        );
        const triggerBtn =
          typeof document !== 'undefined'
            ? document.getElementById('btn-menu-settings')
            : null;
        this.hud.toggleSettingsModal(true, triggerBtn);
      },
      onPlayUiSound: (kind) => {
        if (kind === 'focus') {
          this.sound.playMenuFocus();
        } else if (kind === 'confirm') {
          this.sound.playMenuConfirm();
        } else {
          this.sound.playMenuCancel();
        }
      },
    });

    // Khởi động vào MAIN_MENU: không chạy thế giới hay OpeningFracture trước khi người chơi chọn
    this.enterMainMenu();

    requestAnimationFrame(this.gameLoop);
  }

  public getLifecycleState(): AppLifecycleState {
    return this.store.getLifecycle();
  }

  public getSaveSummary(): SaveSummary | null {
    const mapTitles: Record<string, string> = {};
    for (const [id, map] of Object.entries(this.allMaps)) {
      mapTitles[id] = map.title;
    }
    return this.saveManager.getSaveSummary(CHAPTERS, mapTitles);
  }

  private isInGameplaySession(): boolean {
    const lifecycle = this.store.getLifecycle();
    return lifecycle === 'PLAYING' || lifecycle === 'PAUSED';
  }

  private canOpenGameplayHotkeyOverlay(): boolean {
    if (!this.isInGameplaySession()) {
      return false;
    }
    const mode = this.store.getMode();
    return mode !== 'opening' && mode !== 'climax' && mode !== 'loading';
  }

  private getLogicalViewportWidth(): number {
    if (this.useLegacyRenderer) {
      return INTERNAL_WIDTH;
    }
    return this.gameShell?.getMetrics().logicalWidth ?? INTERNAL_WIDTH;
  }

  private getLogicalViewportHeight(): number {
    if (this.useLegacyRenderer) {
      return INTERNAL_HEIGHT;
    }
    return this.gameShell?.getMetrics().logicalHeight ?? INTERNAL_HEIGHT;
  }

  private syncCameraBoundsAndSnap(): void {
    const vw = this.getLogicalViewportWidth();
    const vh = this.getLogicalViewportHeight();
    this.cameraController.setViewport(vw, vh);
    this.cameraController.setBounds(
      this.currentMap.widthPx,
      this.currentMap.heightPx,
      vw,
      vh
    );
    const snapped = this.cameraController.snapTo(this.playerX, this.playerY);
    this.cameraX = snapped.x;
    this.cameraY = snapped.y;
  }

  private handleViewportChange(metrics: ViewportMetrics): void {
    if (!this.renderer || !this.cameraController || !this.currentMap) {
      return;
    }

    if (this.useLegacyRenderer) {
      this.renderer.setLogicalSize(INTERNAL_WIDTH, INTERNAL_HEIGHT);
      if (typeof document !== 'undefined') {
        const canvas = document.getElementById(
          'pixel-canvas'
        ) as HTMLCanvasElement | null;
        if (canvas) {
          const scale = Math.max(
            1,
            Math.floor(
              Math.min(
                metrics.cssWidth / INTERNAL_WIDTH,
                metrics.cssHeight / INTERNAL_HEIGHT
              )
            )
          );
          canvas.style.width = `${INTERNAL_WIDTH * scale}px`;
          canvas.style.height = `${INTERNAL_HEIGHT * scale}px`;
        }
      }
      return;
    }

    this.renderer.setLogicalSize(metrics.logicalWidth, metrics.logicalHeight);
    this.cameraController.setViewport(metrics.logicalWidth, metrics.logicalHeight);
    this.cameraController.setBounds(
      this.currentMap.widthPx,
      this.currentMap.heightPx,
      metrics.logicalWidth,
      metrics.logicalHeight
    );
    const snapped = this.cameraController.snapTo(this.playerX, this.playerY);
    this.cameraX = snapped.x;
    this.cameraY = snapped.y;
  }

  public async toggleFullscreen(): Promise<boolean> {
    const active = await this.fullscreenManager.toggle();
    this.hud.updateSettings(this.store.settings, active);
    this.gameShell.refresh(true);
    return active;
  }

  private handlePresenterShortcut(): void {
    if (!this.store.settings.presenterModeUnlocked) {
      this.updateSettings({ presenterModeUnlocked: true });
      this.hud.showToast(
        '◆ Đã mở khóa Công cụ Giảng viên / Kiểm thử (Ctrl+Shift+P).'
      );
    }
    if (this.isInGameplaySession()) {
      this.syncHudWithState();
      this.hud.togglePresenterDrawer();
    }
  }

  private enterMainMenu(): void {
    this.openingFracture.cancel();
    this.loadingScreen.hide();
    this.dialogueSystem.closeDialogue();
    this.hud.closeAllOverlays();
    this.hud.resetStageIntroTracker();
    this.hud.setHudVisible(false);
    this.hud.updateInteractionPrompt(null);
    this.tutorialController.setGameplayVisible(false);
    this.tutorialController.setOverlaySuppressed(false);
    this.input.clearKeys();
    this.store.setMode('main_menu');
    this.mainMenu.show(this.getSaveSummary(), this.store.settings.reducedMotion);
  }

  public async startNewGame(confirmedOverwrite = false): Promise<boolean> {
    if (this.store.getLifecycle() === 'LOADING') {
      return false;
    }

    if (this.saveManager.hasSave() && !confirmedOverwrite) {
      const summary = this.getSaveSummary();
      const detail = summary?.chapterLabel
        ? `${summary.chapterLabel} — ${summary.mapTitle} (${summary.milestoneCount}/12 cột mốc)`
        : 'tiến trình đã lưu hiện tại';
      this.mainMenu.openConfirmDialog(
        'KHỞI ĐẦU HÀNH TRÌNH MỚI?',
        `Hệ thống phát hiện hồ sơ lưu trữ: ${detail}. Khởi đầu mới sẽ ghi đè dữ liệu này. Bạn có muốn tiếp tục?`,
        'Ghi đè & Bắt đầu',
        () => {
          void this.startNewGame(true);
        }
      );
      return false;
    }

    this.sound.unlockAudio();
    const preservedSettings = this.saveManager.normalizeSettings({
      ...this.store.settings,
      tutorialCompleted: false,
    });

    this.openingFracture.cancel();
    this.dialogueSystem.closeDialogue();
    this.hud.closeAllOverlays();
    this.mainMenu.hide();
    this.hud.resetStageIntroTracker();
    this.hud.setHudVisible(false);
    this.hud.updateInteractionPrompt(null);
    this.tutorialController.setGameplayVisible(false);
    this.input.clearKeys();

    this.saveManager.clearSave();
    this.store.resetToDefault();
    this.store.settings = preservedSettings;
    this.saveManager.saveSettings(preservedSettings);

    this.puzzleAttemptCounts.clear();
    this.npcRouteStates.clear();
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.postBurstDelaySec = 0;
    this.hazardCooldown = 0;
    this.transitionFadeAlpha = 0;

    if (this.store.debugOverlayEnabled) {
      this.store.toggleDebugOverlay(false);
      this.hud.setDebugOverlayUI(false);
    }
    this.hud.setPresenterModeUI(false);

    this.store.setMode('loading');
    this.loadMap('harbor_square', 'default_spawn', { x: 85, y: 384 }, 'right');
    const activeToken = this.store.getTransitionToken();

    await this.loadingScreen.playTransition({
      mode: 'new_game',
      reducedMotion: this.store.settings.reducedMotion,
    });

    if (this.store.getTransitionToken() !== activeToken) {
      return false;
    }

    this.persistProgress();
    this.syncHudWithState();
    this.hud.setHudVisible(true);
    this.renderCurrentWorldFrame(performance.now() * 0.001);

    this.store.setMode('opening');
    this.openingFracture.start({
      mode: 'full',
      reducedMotion: this.store.settings.reducedMotion,
      onComplete: () => {
        if (this.store.getTransitionToken() !== activeToken) {
          return;
        }
        this.store.setMode('exploring');
        this.input.clearKeys();
        this.tutorialController.setGameplayVisible(true);
        this.tutorialController.setOverlaySuppressed(false);
        this.tutorialController.startIfFirstPlay(false);
        this.syncHudWithState();
        this.hud.showStageIntro(this.currentChapter.id);
      },
    });

    return true;
  }

  public async continueGame(): Promise<boolean> {
    if (this.store.getLifecycle() === 'LOADING') {
      return false;
    }

    const saved = this.saveManager.load();
    if (!saved) {
      this.mainMenu.updateSaveSummary(null);
      return false;
    }

    this.sound.unlockAudio();
    const standaloneSettings = this.saveManager.loadSettings();

    this.openingFracture.cancel();
    this.dialogueSystem.closeDialogue();
    this.hud.closeAllOverlays();
    this.mainMenu.hide();
    this.hud.resetStageIntroTracker();
    this.hud.setHudVisible(false);
    this.hud.updateInteractionPrompt(null);
    this.tutorialController.setGameplayVisible(false);
    this.input.clearKeys();

    this.puzzleAttemptCounts.clear();
    this.npcRouteStates.clear();
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.postBurstDelaySec = 0;
    this.hazardCooldown = 0;
    this.transitionFadeAlpha = 0;

    this.store.restoreFromSave(saved, CHAPTERS);
    this.store.settings = this.saveManager.normalizeSettings({
      ...saved.settings,
      ...standaloneSettings,
    });
    this.saveManager.saveSettings(this.store.settings);
    this.sound.setMuted(this.store.settings.muted);
    this.sound.setVolume(this.store.settings.masterVolume ?? 0.85);
    this.gameShell.setZoomPreset(this.store.settings.cameraZoom ?? 'normal');
    this.hud.setPresenterModeUI(this.store.isPresenterMode);

    const savedMapId =
      saved.currentMapId && this.allMaps[saved.currentMapId]
        ? saved.currentMapId
        : 'harbor_square';
    const safeY =
      saved.playerPos.y && saved.playerPos.y !== 148 ? saved.playerPos.y : 384;

    this.store.setMode('loading');
    this.loadMap(
      savedMapId,
      saved.spawnId || 'default_spawn',
      { x: saved.playerPos.x, y: safeY },
      saved.playerFacing ?? 'right'
    );
    const activeToken = this.store.getTransitionToken();

    await this.loadingScreen.playTransition({
      mode: 'continue',
      reducedMotion: this.store.settings.reducedMotion,
    });

    if (this.store.getTransitionToken() !== activeToken) {
      return false;
    }

    this.syncHudWithState();
    this.hud.setHudVisible(true);
    this.renderCurrentWorldFrame(performance.now() * 0.001);

    this.store.setMode('opening');
    this.openingFracture.start({
      mode: 'continue',
      reducedMotion: this.store.settings.reducedMotion,
      onComplete: () => {
        if (this.store.getTransitionToken() !== activeToken) {
          return;
        }
        this.store.setMode('exploring');
        this.input.clearKeys();
        this.tutorialController.setGameplayVisible(true);
        this.tutorialController.setOverlaySuppressed(false);
        this.tutorialController.startIfFirstPlay(
          Boolean(this.store.settings.tutorialCompleted ?? true)
        );
        this.syncHudWithState();
        this.hud.showStageIntro(this.currentChapter.id);
      },
    });

    return true;
  }

  public returnToMainMenu(): void {
    if (this.isInGameplaySession() && !this.store.isPresenterMode) {
      this.persistProgress();
    }

    this.store.bumpTransitionToken();
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.postBurstDelaySec = 0;
    this.transitionFadeAlpha = 0;

    this.enterMainMenu();
  }

  public get currentChapterIndex(): number {
    return this.store.currentChapterIndex;
  }

  public set currentChapterIndex(idx: number) {
    this.store.currentChapterIndex = idx;
  }

  public get currentChapter(): ChapterData {
    return CHAPTERS[this.store.currentChapterIndex];
  }

  public get collectedMilestones(): Set<RomanMilestoneId> {
    return this.store.collectedMilestones;
  }

  public getPlayerPosition(): Vec2 {
    return { x: this.playerX, y: this.playerY };
  }

  public getCurrentMapId(): string {
    return this.store.currentMapId;
  }

  public getEntityDepth(footY: number): number {
    return computeEntityDepth(footY);
  }

  public getPlayerFootRect(x = this.playerX, y = this.playerY): Rect2D {
    return {
      x: x - 6,
      y: y - 6,
      width: 12,
      height: 8,
    };
  }

  public toggleDebugOverlay(force?: boolean): boolean {
    const enabled = this.store.toggleDebugOverlay(force);
    this.hud.setDebugOverlayUI(enabled, {
      mapId: this.currentMap.id,
      x: this.playerX,
      y: this.playerY,
      zoom: this.gameShell.getZoomPreset(),
      scale: this.gameShell.getMetrics().pixelScale,
      fps: this.smoothedFps,
    });
    this.hud.showToast(
      enabled
        ? '◆ Đã bật Lớp Phủ Kiểm Định (F3): Hiển thị vật cản, vùng ẩn nấp, cổng & lộ trình NPC.'
        : '○ Đã tắt Lớp Phủ Kiểm Định (F3).'
    );
    return enabled;
  }

  public updateSettings(partial: Partial<GameSettings>): GameSettings {
    const next = this.saveManager.normalizeSettings({
      ...this.store.settings,
      ...partial,
    });
    this.store.settings = next;
    this.saveManager.saveSettings(next);

    this.sound.setMuted(next.muted);
    this.sound.setVolume(next.masterVolume ?? 0.85);

    if (partial.cameraZoom !== undefined) {
      this.gameShell.setZoomPreset(next.cameraZoom ?? 'normal');
    }

    if (partial.reducedMotion !== undefined) {
      this.mainMenu.setReducedMotion(next.reducedMotion);
    }

    if (partial.presenterModeUnlocked === false) {
      if (this.store.isPresenterMode) {
        this.store.togglePresenterMode();
        this.hud.setPresenterModeUI(false);
      }
      if (this.store.debugOverlayEnabled) {
        this.store.toggleDebugOverlay(false);
        this.hud.setDebugOverlayUI(false);
      }
      if (this.hud.isPresenterDrawerOpen()) {
        this.hud.togglePresenterDrawer(false);
      }
    }

    if (
      this.isInGameplaySession() &&
      !this.store.isPresenterMode &&
      this.saveManager.hasSave()
    ) {
      this.persistProgress();
    }

    this.hud.updateSettings(next, this.fullscreenManager.isFullscreenActive());
    if (this.isInGameplaySession()) {
      this.syncHudWithState();
    }
    return { ...next };
  }

  public handleNumberKeyShortcut(num: 1 | 2 | 3 | 4): void {
    const idx = num - 1;
    const mode = this.store.getMode();

    if (mode === 'climax') {
      const btns = document.querySelectorAll<HTMLButtonElement>(
        '#climax-a-options .climax-option-btn'
      );
      btns[idx]?.click();
      return;
    }

    const choiceBtns = document.querySelectorAll<HTMLButtonElement>(
      '#dialogue-choices .dialogue-choice-btn'
    );
    const targetBtn = choiceBtns[idx];
    if (targetBtn && !targetBtn.closest('[hidden]')) {
      targetBtn.click();
    }
  }

  public manualSave(): boolean {
    if (this.store.isPresenterMode) {
      this.hud.showToast(
        '⚠ Đang ở Chế độ Giảng Viên: Không ghi đè dữ liệu lưu tiến trình thường.'
      );
      return false;
    }
    this.persistProgress();
    this.mainMenu.updateSaveSummary(this.getSaveSummary());
    this.hud.showToast('◆ Đã lưu tiến trình Lữ Khách vào bộ nhớ cục bộ (SaveV2)!');
    return true;
  }

  public resetSave(): void {
    const preservedSettings = this.saveManager.normalizeSettings(this.store.settings);
    this.saveManager.clearSave();
    this.store.resetToDefault();
    this.store.settings = preservedSettings;
    this.saveManager.saveSettings(preservedSettings);
    this.puzzleAttemptCounts.clear();
    this.npcRouteStates.clear();
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.postBurstDelaySec = 0;
    this.hud.setPresenterModeUI(false);
    this.loadMap('harbor_square', 'default_spawn', { x: 85, y: 384 }, 'right');
    this.hud.closeAllOverlays();
    this.hud.setHudVisible(true);
    this.store.setMode('exploring');
    this.mainMenu.updateSaveSummary(null);
    this.syncHudWithState();
    this.hud.showToast('◆ Đã xóa dữ liệu lưu và đặt lại hành trình từ đầu Chương I!');
  }

  public openDossierPuzzle(puzzle?: DossierPuzzleDefinition): void {
    const resolved =
      puzzle ?? getEraPuzzleForMapOrId(this.currentMap.id, this.currentChapter.id);
    const attempts = this.puzzleAttemptCounts.get(resolved.id) ?? 0;
    this.hud.openDossierPuzzleModal(resolved, attempts);
  }

  public submitDossierPuzzle(
    puzzleId: string,
    selectedOptionIds: Record<string, string>
  ): {
    solved: boolean;
    attemptCount: number;
    feedback: string;
    showProgressiveHint: boolean;
  } {
    const puzzle = getEraPuzzleForMapOrId(
      puzzleId || this.currentMap.id,
      this.currentChapter.id
    );
    const prevAttempts = this.puzzleAttemptCounts.get(puzzle.id) ?? 0;
    const attemptCount = prevAttempts + 1;
    this.puzzleAttemptCounts.set(puzzle.id, attemptCount);

    let allAuthentic = true;
    for (const slot of puzzle.slots) {
      const selectedId = selectedOptionIds[slot.id];
      const matchedOpt = slot.options.find((opt) => opt.id === selectedId);
      if (!matchedOpt || !matchedOpt.isAuthentic) {
        allAuthentic = false;
        break;
      }
    }

    if (!allAuthentic) {
      this.sound.playAlertPushback();
      return {
        solved: false,
        attemptCount,
        feedback:
          'Chưa chính xác! Hãy đối chiếu kỹ nội dung văn kiện lịch sử và thử lại.',
        showProgressiveHint: attemptCount >= 2,
      };
    }

    if (!this.store.isPresenterMode) {
      this.store.questFlags.add(puzzle.rewardFlag);
      if (puzzle.rewardEvidenceId) {
        this.inventorySystem.addItem(puzzle.rewardEvidenceId, this.store);
      }
      if (puzzle.rewardMilestone) {
        this.store.collectMilestone(puzzle.rewardMilestone);
      }
    } else if (puzzle.rewardMilestone) {
      this.store.presenterCollectedMilestones.add(puzzle.rewardMilestone);
    }

    this.questSystem.evaluateProgress(this.store, this.currentChapter.id);
    this.persistProgress();
    this.sound.playSealStamp();
    this.syncHudWithState();
    this.hud.showToast(
      `✓ Đã xác thực ${puzzle.title} & khôi phục Cột mốc ${puzzle.rewardMilestone ?? 'III'}!`
    );

    return {
      solved: true,
      attemptCount,
      feedback: `✓ Xác thực hoàn tất! Đã khôi phục Cột mốc ${puzzle.rewardMilestone ?? 'III'} và mở khóa tiến trình tiếp theo.`,
      showProgressiveHint: false,
    };
  }

  public resetPlayerToMapCheckpoint(): void {
    const cp =
      this.currentMap.checkpoints[0] ??
      this.currentMap.spawns.find((s) => s.id === this.currentMap.defaultSpawnId) ??
      this.currentMap.spawns[0];

    this.playerX = cp?.x ?? 64;
    this.playerY = cp?.y ?? 256;
    this.playerFacing = cp?.facing ?? 'right';
    this.facingRight = this.playerFacing !== 'left';
    this.store.playerPos = { x: this.playerX, y: this.playerY };
    this.store.playerFacing = this.playerFacing;
    this.stealthSystem.resetSuspicion();
  }

  public openDialogueForNpc(
    npcIdOrDialogueId: string,
    milestone?: MilestoneArtifact
  ): boolean {
    const node = this.dialogueSystem.startDialogue(npcIdOrDialogueId);
    if (!node) return false;
    const choices = this.dialogueSystem.getAvailableChoices(
      node,
      this.store.questFlags,
      this.store.collectedEvidence
    );
    this.hud.openNpcDialogue(node, choices, milestone);
    return true;
  }

  public selectDialogueChoice(choiceId: string): void {
    const { nextNode, choice, ended } = this.dialogueSystem.selectChoice(
      choiceId,
      this.store.questFlags,
      this.store.collectedEvidence
    );

    if (choice) {
      this.store.dialogueHistory.add(choice.id);

      if (!this.store.isPresenterMode) {
        if (choice.grantsFlag) {
          this.store.questFlags.add(choice.grantsFlag);
        }
        if (choice.grantsEvidence) {
          this.inventorySystem.addItem(choice.grantsEvidence, this.store);
        }
        if (choice.grantsMilestone) {
          this.store.collectMilestone(choice.grantsMilestone);
        }
      } else if (choice.grantsMilestone) {
        this.store.presenterCollectedMilestones.add(choice.grantsMilestone);
      }

      if (choice.responseToast) {
        this.hud.showToast(choice.responseToast);
      }
    }

    this.questSystem.evaluateProgress(this.store, this.currentChapter.id);
    this.persistProgress();
    this.syncHudWithState();

    if (ended || !nextNode) {
      this.hud.closeDialogueModal();
      return;
    }

    const nextChoices = this.dialogueSystem.getAvailableChoices(
      nextNode,
      this.store.questFlags,
      this.store.collectedEvidence
    );
    this.hud.updateDialogueNode(nextNode, nextChoices);
  }

  public loadMap(
    mapId: string,
    spawnId?: string,
    posOverride?: Vec2,
    facingOverride?: Facing4
  ): NormalizedMapData {
    this.allMaps = getAllBuiltInMaps();
    const map = this.allMaps[mapId] ?? getBuiltInMap(mapId);
    this.currentMap = map;
    this.collisionWorld = new CollisionWorld(map.obstacles, map.shelters, {
      x: 0,
      y: 0,
      width: map.widthPx,
      height: map.heightPx,
    });
    this.stealthSystem.initForMap(map);

    const spawn =
      (spawnId ? map.spawns.find((s) => s.id === spawnId) : undefined) ??
      map.spawns.find((s) => s.id === map.defaultSpawnId) ??
      map.spawns[0];

    this.playerX = posOverride?.x ?? spawn?.x ?? 160;
    this.playerY = posOverride?.y ?? spawn?.y ?? 384;
    this.playerFacing = facingOverride ?? spawn?.facing ?? 'down';
    this.facingRight = this.playerFacing !== 'left';

    this.store.setMapState(
      map.id,
      spawn?.id ?? 'default_spawn',
      { x: this.playerX, y: this.playerY },
      this.playerFacing
    );
    this.sceneTransition.currentMapId = map.id;
    this.sceneTransition.currentSpawnId = spawn?.id ?? 'default_spawn';

    this.syncCameraBoundsAndSnap();

    return map;
  }

  public enterPortal(portal: PortalDefinition): boolean {
    if (this.sceneTransition.getCooldownRemaining() > 0) {
      return false;
    }

    this.allMaps = getAllBuiltInMaps();
    const res = this.sceneTransition.transitionTo(
      portal.destinationMap,
      portal.destinationSpawn,
      this.allMaps,
      portal.requiresFlag,
      this.store.questFlags
    );

    if (!res.transitioned || !res.targetMapId || !res.spawnPos) {
      if (res.reason === 'missing_flag') {
        this.hud.showToast('◆ Lối đi này cần hoàn thành nhiệm vụ liên lạc trước.');
      }
      return false;
    }

    this.store.portalCooldownSec = this.sceneTransition.getCooldownRemaining();
    this.transitionFadeAlpha = this.store.settings.reducedMotion ? 0 : 0.85;
    this.loadMap(
      res.targetMapId,
      res.targetSpawnId,
      res.spawnPos,
      res.spawnFacing ?? 'down'
    );
    this.persistProgress();
    this.syncHudWithState();
    this.hud.showToast(`◆ Đã chuyển tới: ${this.currentMap.title}`);
    return true;
  }

  public fastTravelToZone(zoneId: string, spawnId?: string): boolean {
    this.allMaps = getAllBuiltInMaps();
    const targetMap = this.allMaps[zoneId];
    if (!targetMap) {
      return false;
    }

    const checkpointInteractable = targetMap.interactables.find(
      (i) => i.kind === 'checkpoint'
    );
    const checkpointSpawnId =
      spawnId ?? targetMap.checkpoints[0]?.id ?? targetMap.defaultSpawnId;

    if (
      !this.store.canFastTravelToZone(
        zoneId,
        checkpointInteractable?.id,
        checkpointSpawnId
      )
    ) {
      this.hud.showToast(
        '◆ Khu vực này chưa được khám phá hoặc chưa kích hoạt Đèn Báo An Toàn!'
      );
      return false;
    }

    this.loadMap(zoneId, checkpointSpawnId);
    this.hud.toggleWorldMapModal(false);
    if (this.isInGameplaySession()) {
      this.store.setMode('exploring');
    }
    this.input.clearKeys();
    this.persistProgress();
    this.syncHudWithState();
    this.hud.showToast(`◆ Đã dịch chuyển nhanh tới: ${this.currentMap.title}`);
    return true;
  }

  public interactWithDoorOrNearest(): boolean {
    return this.handleInteractAction();
  }

  private persistProgress(): void {
    if (this.store.isPresenterMode) return;
    const data = this.store.toSaveDataV2(
      this.currentChapter.id,
      this.playerX,
      this.playerFacing,
      this.playerY
    );
    this.saveManager.save(data);
  }

  private getDefaultZoneForEra(eraId: EraId): string {
    this.allMaps = getAllBuiltInMaps();
    if (eraId === '1945' && this.allMaps['tan_trao_banyan']) {
      return 'tan_trao_banyan';
    }
    if (eraId === '1954_1975' && this.allMaps['pha_din_pass']) {
      return 'pha_din_pass';
    }
    if (eraId === '1986_2045' && this.allMaps['doi_moi_avenue']) {
      return 'doi_moi_avenue';
    }
    return 'harbor_square';
  }

  public switchEra(eraId: EraId): void {
    const idx = CHAPTERS.findIndex((c) => c.id === eraId);
    if (idx === -1) return;

    this.store.bumpTransitionToken();
    this.store.currentChapterIndex = idx;
    const defaultZoneId = this.getDefaultZoneForEra(eraId);
    this.store.unlockCheckpoint(defaultZoneId, defaultZoneId);
    this.loadMap(
      defaultZoneId,
      'default_spawn',
      defaultZoneId === 'harbor_square' ? { x: 85, y: 384 } : undefined,
      'right'
    );
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.postBurstDelaySec = 0;
    this.hazardCooldown = 0;

    this.hud.closeAllOverlays();
    if (this.isInGameplaySession()) {
      this.store.setMode('exploring');
    }
    this.input.clearKeys();

    if (this.isInGameplaySession()) {
      this.persistProgress();
    }
    this.syncHudWithState();
    this.hud.showStageIntro(this.currentChapter.id);
    this.hud.showToast(
      `Đã bước qua vết nứt thời gian tới ${this.currentChapter.chapterNum} (${this.currentChapter.years})`
    );
  }

  public teleportToMilestone(m: MilestoneArtifact): void {
    if (!this.store.isPresenterMode) {
      if (this.store.getActiveMilestones().has(m.roman)) {
        this.hud.showArtifactDialogue(m);
      } else {
        this.hud.showToast(
          '◆ Cột mốc chưa khôi phục! Hãy khám phá trong thế giới hoặc bật Chế độ Giảng Viên.'
        );
      }
      return;
    }

    const eraIdx = CHAPTERS.findIndex((c) => c.id === m.eraId);
    if (eraIdx !== -1 && eraIdx !== this.store.currentChapterIndex) {
      this.store.bumpTransitionToken();
      this.store.currentChapterIndex = eraIdx;
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.postBurstDelaySec = 0;
    }

    this.allMaps = getAllBuiltInMaps();
    if (m.mapId && this.allMaps[m.mapId] && this.currentMap.id !== m.mapId) {
      this.loadMap(m.mapId, undefined, { x: m.worldX, y: m.worldY ?? 384 });
    } else {
      this.playerX = m.worldX;
      if (m.worldY !== undefined) {
        this.playerY = m.worldY;
      }
      this.store.playerPos = { x: this.playerX, y: this.playerY };
      this.syncCameraBoundsAndSnap();
    }

    this.store.presenterCollectedMilestones.add(m.roman);
    this.sound.playSealStamp();
    this.hud.showArtifactDialogue(m);
    this.syncHudWithState();
  }

  public togglePresenterMode(): void {
    const isNowPresenter = this.store.togglePresenterMode();
    if (isNowPresenter && !this.store.settings.presenterModeUnlocked) {
      this.store.settings = this.saveManager.normalizeSettings({
        ...this.store.settings,
        presenterModeUnlocked: true,
      });
    }
    this.hud.setPresenterModeUI(isNowPresenter);
    this.hud.updateSettings(
      this.store.settings,
      this.fullscreenManager.isFullscreenActive()
    );

    if (isNowPresenter) {
      this.sound.playClimaxFanfare();
      const firstOfChapter = this.currentChapter.milestones[0];
      if (firstOfChapter) {
        this.hud.showArtifactDialogue(firstOfChapter);
      }
      this.hud.showToast(
        '◆ Chế độ Giảng Viên: Đã mở khóa cô lập toàn bộ 12/12 Cột Mốc La Mã (I – XII)!'
      );
    } else {
      this.hud.showToast(
        'Đã tắt Chế độ Giảng Viên — Khôi phục tiến trình khám phá thực tế.'
      );
    }

    this.syncHudWithState();
  }

  public unlockAllMilestonesForLecturer(): void {
    this.togglePresenterMode();
  }

  public trackObjective(objectiveId: string | null): boolean {
    if (objectiveId) {
      const lookup = this.questSystem.getObjectiveById(objectiveId);
      if (!lookup) {
        return false;
      }
      const status = this.questSystem.getObjectiveStatus(lookup.objective, this.store);
      if (status !== 'active') {
        this.hud.showToast(
          status === 'completed'
            ? '✓ Mục tiêu này đã hoàn thành.'
            : '◆ Mục tiêu này đang khóa, hãy hoàn tất điều kiện tiên quyết trước.'
        );
        return false;
      }
      this.store.trackedObjectiveId = objectiveId;
      this.routePlanner.invalidate();
      this.persistProgress();
      this.syncHudWithState();
      this.hud.showToast(`▼ Đang dẫn đường: ${lookup.objective.title}`);
      return true;
    }

    this.store.trackedObjectiveId = null;
    this.routePlanner.invalidate();
    this.persistProgress();
    this.syncHudWithState();
    return true;
  }

  public getActiveObjectiveResolution(): ObjectiveTargetResolution {
    const activeObjective = this.questSystem.getCurrentActiveObjective(
      this.currentChapter.id,
      this.store
    );
    if (!activeObjective) {
      const nextLocked = this.questSystem.getNextLockedObjective(
        this.currentChapter.id,
        this.store
      );
      if (nextLocked) {
        return resolveObjectiveTarget(this.currentMap, nextLocked.objective, {
          isLocked: true,
          prerequisiteText: this.questSystem.getPrerequisiteSummary(
            nextLocked.objective,
            this.store
          ),
          questFlags: this.store.questFlags,
        });
      }
      return resolveObjectiveTarget(this.currentMap, null, {
        questFlags: this.store.questFlags,
      });
    }

    return resolveObjectiveTarget(this.currentMap, activeObjective.objective, {
      questFlags: this.store.questFlags,
    });
  }

  public getTrackedRouteState(): TrackedRouteState {
    const activeObjective = this.questSystem.getCurrentActiveObjective(
      this.currentChapter.id,
      this.store
    );
    const resolution = this.getActiveObjectiveResolution();
    return this.routePlanner.computeTrackedRoute(
      this.currentMap,
      this.playerX,
      this.playerY,
      activeObjective?.objective ?? null,
      resolution,
      this.store.questFlags
    );
  }

  public handleInteractAction(): boolean {
    if (!this.store.isWorldInputAllowed()) {
      return false;
    }

    const chapter = this.currentChapter;
    const candidate = this.findNearestInteractionCandidate();
    if (candidate) {
      this.tutorialController.recordAction('interact');
    }

    // 1. Tương tác với Portal / Cửa ra vào
    if (candidate?.kind === 'portal' && candidate.portal) {
      return this.enterPortal(candidate.portal);
    }

    if (
      candidate?.kind === 'door' &&
      candidate.interactable?.destinationMap &&
      candidate.interactable?.destinationSpawn
    ) {
      return this.enterPortal({
        id: candidate.interactable.id,
        x: candidate.interactable.x,
        y: candidate.interactable.y,
        width: candidate.interactable.width ?? 32,
        height: candidate.interactable.height ?? 16,
        destinationMap: candidate.interactable.destinationMap,
        destinationSpawn: candidate.interactable.destinationSpawn,
        requiresFlag: candidate.interactable.requiresFlag,
        label: candidate.interactable.label,
      });
    }

    // 2. Kích hoạt Điểm Lưu / Đèn Báo An Toàn (Checkpoint)
    if (candidate?.kind === 'checkpoint' && candidate.interactable) {
      this.store.unlockCheckpoint(candidate.interactable.id, this.currentMap.id);
      this.store.currentSpawnId = candidate.interactable.id;
      this.persistProgress();
      this.sound.playSealStamp();
      this.syncHudWithState();
      this.hud.showToast(
        `◆ Đã kích hoạt Điểm Lưu & Dịch Chuyển: ${candidate.interactable.label}`
      );
      return true;
    }

    // 3. Tương tác với Bàn Đối Chiếu Hồ Sơ / Sa Bàn Chiến Lược (Dossier Puzzle)
    if (candidate?.kind === 'puzzle') {
      const puzzle = getEraPuzzleForMapOrId(
        candidate.interactable?.id ?? this.currentMap.id,
        chapter.id
      );
      this.openDossierPuzzle(puzzle);
      return true;
    }

    // 4. Tương tác trực tiếp với Cổng Thời Gian cuối màn (Climax Gate)
    if (candidate?.kind === 'climax_gate') {
      const check = this.store.canTriggerChapterClimax(chapter);
      if (!check.allowed) {
        this.hud.showToast(
          `◆ Cần khôi phục đủ Cột mốc ${check.missing.join(', ')} trước khi mở Cổng Thời Gian!`
        );
        return false;
      }
      this.hud.openClimaxModal(chapter);
      return true;
    }

    // 5. Tương tác với Cột mốc lịch sử hoặc NPC / Tư liệu gắn với cột mốc
    const nearbyMilestone = candidate?.milestone ?? this.findNearbyMilestone();
    if (nearbyMilestone) {
      this.tutorialController.recordAction('interact');
      this.store.collectMilestone(nearbyMilestone.roman);
      if (nearbyMilestone.roman === 'I' && !this.store.isPresenterMode) {
        this.store.questFlags.add('ch1_met_liaison');
      }
      if (candidate?.interactable?.evidenceId) {
        this.inventorySystem.addItem(candidate.interactable.evidenceId, this.store);
      }
      if (candidate?.interactable?.grantsFlag && !this.store.isPresenterMode) {
        this.store.questFlags.add(candidate.interactable.grantsFlag);
      }

      const dialogueKey =
        candidate?.npc?.dialogueId ??
        candidate?.npc?.npcId ??
        candidate?.interactable?.dialogueId;
      const dlgNode = dialogueKey ? this.dialogueSystem.startDialogue(dialogueKey) : null;
      const dlgChoices = dlgNode
        ? this.dialogueSystem.getAvailableChoices(
            dlgNode,
            this.store.questFlags,
            this.store.collectedEvidence
          )
        : undefined;

      this.questSystem.evaluateProgress(this.store, chapter.id);
      this.persistProgress();
      this.sound.playSealStamp();
      this.hud.showArtifactDialogue(nearbyMilestone, dlgNode, dlgChoices);
      this.syncHudWithState();
      this.hud.showToast(
        `✓ Đã đóng dấu khôi phục Cột mốc ${nearbyMilestone.roman} (${nearbyMilestone.year})`
      );
      return true;
    }

    // 6. Tương tác với NPC không gắn cột mốc trực tiếp (hội thoại phân nhánh)
    if (candidate?.kind === 'npc') {
      const npcDialogueKey =
        candidate.npc?.dialogueId ||
        candidate.npc?.npcId ||
        candidate.interactable?.dialogueId ||
        candidate.interactable?.id;
      const node = npcDialogueKey
        ? this.dialogueSystem.startDialogue(npcDialogueKey)
        : null;
      if (node) {
        const choices = this.dialogueSystem.getAvailableChoices(
          node,
          this.store.questFlags,
          this.store.collectedEvidence
        );
        this.sound.playSealStamp();
        this.hud.openNpcDialogue(node, choices);
        this.syncHudWithState();
        return true;
      }
    }

    // 7. Tương tác với vật thể / chứng cứ / bảng tin trong bản đồ 2D
    if (candidate?.interactable) {
      if (candidate.interactable.evidenceId) {
        this.inventorySystem.addItem(candidate.interactable.evidenceId, this.store);
      }
      if (candidate.interactable.grantsFlag && !this.store.isPresenterMode) {
        this.store.questFlags.add(candidate.interactable.grantsFlag);
      }
      this.questSystem.evaluateProgress(this.store, chapter.id);
      this.persistProgress();
      this.sound.playSealStamp();
      this.hud.showToast(`◆ Đã thu thập tư liệu: ${candidate.interactable.label}`);
      this.syncHudWithState();
      return true;
    }

    // 8. Kiểm tra nếu đứng gần Cổng Thời Gian cuối màn (1D fallback)
    const nearClimaxGate =
      Math.abs(this.playerX - chapter.climax.gateWorldX) <= 38;

    if (nearClimaxGate) {
      this.tutorialController.recordAction('interact');
      const check = this.store.canTriggerChapterClimax(chapter);
      if (!check.allowed) {
        this.hud.showToast(
          `◆ Cần khôi phục đủ Cột mốc ${check.missing.join(', ')} trước khi mở Cổng Thời Gian!`
        );
        return false;
      }
      this.hud.openClimaxModal(chapter);
      return true;
    }

    this.hud.showToast(
      'Hãy di chuyển lại gần Nhân vật, Cửa Kho hoặc Cột Mốc Lịch Sử rồi bấm [E].'
    );
    return false;
  }

  public triggerClimaxStepBSetPiece(): void {
    const chapter = this.currentChapter;
    const token = this.store.bumpTransitionToken();
    this.activeBurstToken = token;
    this.postBurstDelaySec = 0;

    if (!this.store.isPresenterMode) {
      for (const m of chapter.milestones) {
        this.store.collectMilestone(m.roman);
      }
      this.questSystem.evaluateProgress(this.store, chapter.id);
      this.persistProgress();
    } else {
      for (const m of chapter.milestones) {
        this.store.presenterCollectedMilestones.add(m.roman);
      }
    }

    this.sound.playClimaxFanfare();
    this.isBurstAnimating = true;
    this.climaxBurstProgress = 0.05;
    this.syncHudWithState();
    this.hud.showToast(`◆ ${chapter.climax.stepBTitle}`);
  }

  public advanceToNextChapter(): void {
    this.store.bumpTransitionToken();
    this.postBurstDelaySec = 0;

    if (this.store.currentChapterIndex < CHAPTERS.length - 1) {
      this.store.currentChapterIndex++;
      const defaultZoneId = this.getDefaultZoneForEra(this.currentChapter.id);
      this.store.unlockCheckpoint(defaultZoneId, defaultZoneId);
      this.loadMap(
        defaultZoneId,
        'default_spawn',
        defaultZoneId === 'harbor_square' ? { x: 85, y: 384 } : undefined,
        'right'
      );
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.hazardCooldown = 0;
      this.persistProgress();
      this.syncHudWithState();
      this.hud.showStageIntro(this.currentChapter.id);
      this.hud.showToast(
        `Đã mở Cổng Thời Gian sang ${this.currentChapter.chapterNum}: ${this.currentChapter.title}`
      );
    } else {
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.hud.showToast(
        '◆ Chúc mừng Lữ Khách! Bạn đã khôi phục trọn vẹn Biên Niên Sử 1930 – 2045 (12/12 Cột mốc)!'
      );
    }
  }

  public findNearestInteractionCandidate(): InteractionCandidate | null {
    return this.interactionSystem.findNearestCandidate(
      { x: this.playerX, y: this.playerY },
      this.currentMap,
      this.currentChapter
    );
  }

  public findNearbyMilestone(): MilestoneArtifact | null {
    const candidate = this.findNearestInteractionCandidate();
    if (candidate?.milestone) {
      return candidate.milestone;
    }

    for (const m of this.currentChapter.milestones) {
      if (Math.abs(this.playerX - m.worldX) <= 28) {
        return m;
      }
    }
    return null;
  }

  public isPlayerInShelter(): boolean {
    const inMapShelter = this.collisionWorld.isPointInShelter(
      this.playerX,
      this.playerY
    );
    if (inMapShelter) return true;
    return this.store.isPlayerInShelter(
      this.playerX,
      this.currentChapter.shelters,
      this.isCrouching,
      this.playerY
    );
  }

  private hasPatrolScheduleBonus(): boolean {
    return (
      this.store.isPresenterMode ||
      this.store.collectedEvidence.has('ev_ch1_patrol_schedule') ||
      this.store.questFlags.has('ch1_pier_schedule')
    );
  }

  private getCurrentStealthAlertState(
    patrolActors?: PatrolActorRenderState[]
  ): StealthAlertState {
    const states =
      patrolActors ??
      this.stealthSystem.getRenderStates(this.hasPatrolScheduleBonus());
    let highest: StealthAlertState = 'calm';
    for (const actor of states) {
      if (actor.alertState === 'alert') {
        return 'alert';
      }
      if (actor.alertState === 'suspicious') {
        highest = 'suspicious';
      }
    }
    return highest;
  }

  private updateScreenInteractionPrompt(
    candidate: InteractionCandidate | null,
    allowWorldInput: boolean
  ): void {
    if (!allowWorldInput || !candidate?.promptText) {
      this.hud.updateInteractionPrompt(null);
      return;
    }

    const metrics = this.gameShell.getMetrics();
    const screenPt = this.gameShell.worldToScreen(
      candidate.x,
      candidate.y - 24,
      this.cameraX,
      this.cameraY
    );
    this.hud.updateInteractionPrompt(
      candidate.promptText,
      screenPt.x,
      screenPt.y,
      metrics.cssWidth,
      metrics.cssHeight
    );
  }

  public syncHudWithState(): void {
    const chapter = this.currentChapter;
    this.questSystem.evaluateProgress(this.store, chapter.id);

    if (
      this.store.trackedObjectiveId &&
      this.store.completedObjectives.has(this.store.trackedObjectiveId)
    ) {
      this.store.trackedObjectiveId = null;
      this.routePlanner.invalidate();
    }

    const candidate = this.findNearestInteractionCandidate();
    const nearby = candidate?.milestone ?? this.findNearbyMilestone();
    const patrolActors = this.stealthSystem.getRenderStates(
      this.hasPatrolScheduleBonus()
    );
    const stealthAlertState = this.getCurrentStealthAlertState(patrolActors);
    const showStealthStatus =
      this.currentMap.id === 'shadow_alley' || stealthAlertState !== 'calm';

    this.hud.renderSidebarCodex(
      chapter,
      this.store.getActiveMilestones(),
      nearby
    );
    this.hud.updateStageHeader(
      chapter,
      showStealthStatus && this.isPlayerInShelter(),
      showStealthStatus ? stealthAlertState : 'calm'
    );
    this.hud.updateMapLocationBadge(
      this.currentMap.title || chapter.locationLabel,
      chapter.coordinates,
      candidate?.promptText ?? null
    );
    this.updateScreenInteractionPrompt(candidate, this.store.isWorldInputAllowed());
    this.hud.updateWorldMapState(
      this.currentMap.id,
      this.store.discoveredMaps,
      this.store.unlockedCheckpoints,
      this.store.isPresenterMode
    );

    const eraQuests = this.questSystem.getQuestsForEra(chapter.id);
    const questsWithStatus = eraQuests.map((quest) => ({
      quest,
      objectives: quest.objectives.map((objective) => {
        const status = this.questSystem.getObjectiveStatus(objective, this.store);
        return {
          objective,
          status,
          prerequisiteText:
            status === 'locked'
              ? this.questSystem.getPrerequisiteSummary(objective, this.store)
              : undefined,
        };
      }),
    }));
    const activeObjective = this.questSystem.getCurrentActiveObjective(
      chapter.id,
      this.store
    );
    const resolution = this.getActiveObjectiveResolution();
    const nextActionText =
      resolution.nextActionText ||
      this.questSystem.getNextActionForActiveObjective(chapter.id, this.store);
    const hintText = this.questSystem.getHintForActiveObjective(
      chapter.id,
      this.store
    );

    this.hud.updateRpgSystemsState({
      quests: questsWithStatus,
      activeObjective,
      trackedObjectiveId: activeObjective?.objective.id ?? null,
      nextActionText,
      hintText,
      allEvidence: [...this.evidenceSystem.getAllEvidence()],
      collectedEvidenceIds: this.store.collectedEvidence,
      settings: this.store.settings,
      isPresenterMode: this.store.isPresenterMode,
    });

    this.miniMap.setVisible(this.store.settings.showMinimap !== false, false);
  }

  private updateNpcRoutes(dt: number): void {
    if (dt <= 0) return;

    for (const npc of this.currentMap.npcs) {
      if (!npc.route || npc.route.length === 0) continue;

      const stateKey = `${this.currentMap.id}:${npc.id}`;
      let state = this.npcRouteStates.get(stateKey);
      if (!state) {
        state = { waypointIndex: 0, waitRemainingSec: 0 };
        this.npcRouteStates.set(stateKey, state);
      }

      if (state.waitRemainingSec > 0) {
        state.waitRemainingSec = Math.max(0, state.waitRemainingSec - dt);
        continue;
      }

      const target = npc.route[state.waypointIndex % npc.route.length];
      if (!target) continue;

      const dx = target.x - npc.x;
      const dy = target.y - npc.y;
      const dist = Math.hypot(dx, dy);
      const speed = npc.moveSpeed ?? 36;
      const step = speed * dt;

      if (dist <= step || dist < 0.5) {
        npc.x = target.x;
        npc.y = target.y;
        if (target.facing) {
          npc.facing = target.facing;
        }
        state.waitRemainingSec = target.waitSec ?? 1.2;
        state.waypointIndex = (state.waypointIndex + 1) % npc.route.length;
      } else {
        const nx = dx / dist;
        const ny = dy / dist;
        npc.x += nx * step;
        npc.y += ny * step;
        if (Math.abs(dx) >= Math.abs(dy)) {
          npc.facing = dx < 0 ? 'left' : 'right';
        } else {
          npc.facing = dy < 0 ? 'up' : 'down';
        }
      }
    }
  }

  private renderCurrentWorldFrame(timeSec: number): void {
    const candidate = this.findNearestInteractionCandidate();
    const nearbyMilestone = candidate?.milestone ?? this.findNearbyMilestone();
    const nearClimaxGate =
      candidate?.kind === 'climax_gate' ||
      Math.abs(this.playerX - this.currentChapter.climax.gateWorldX) <= 38;
    const allowWorldInput = this.store.isWorldInputAllowed();
    const patrolActors = this.stealthSystem.getRenderStates(
      this.hasPatrolScheduleBonus()
    );
    const routeState = this.getTrackedRouteState();
    const activeQuestTargetIds = new Set<string>();
    const optionalTargetIds = new Set<string>();
    for (const q of this.questSystem.getQuestsForEra(this.currentChapter.id)) {
      for (const obj of q.objectives) {
        const status = this.questSystem.getObjectiveStatus(obj, this.store);
        if (status === 'active' && obj.targetRef) {
          const refId =
            obj.targetRef.npcId ??
            obj.targetRef.interactableId ??
            obj.targetRef.portalId;
          if (refId) {
            if (q.category === 'main' && !obj.isOptional) {
              activeQuestTargetIds.add(refId);
            } else {
              optionalTargetIds.add(refId);
            }
          }
          if (obj.targetRef.milestoneId) {
            activeQuestTargetIds.add(obj.targetRef.milestoneId);
          }
        }
      }
    }

    const renderState: RenderState = {
      chapter: this.currentChapter,
      playerX: this.playerX,
      playerY: this.playerY,
      cameraX: this.cameraX,
      cameraY: this.cameraY,
      facingRight: this.facingRight,
      playerFacing: this.playerFacing,
      isMoving: this.input.isMoving(allowWorldInput),
      isSheltered: this.isPlayerInShelter(),
      collectedMilestones: this.store.getActiveMilestones(),
      nearbyMilestone,
      nearClimaxGate,
      climaxBurstProgress: this.climaxBurstProgress,
      timeSec,
      mapData: this.useLegacyRenderer ? undefined : this.currentMap,
      nearbyPrompt: candidate?.promptText ?? null,
      nearbyTargetPos: candidate ? { x: candidate.x, y: candidate.y } : null,
      trackedTargetPos: routeState.resolution.position
        ? { x: routeState.resolution.position.x, y: routeState.resolution.position.y }
        : null,
      trackedTargetId:
        routeState.resolution.targetEntityId ??
        routeState.resolution.position?.targetId ??
        null,
      activeQuestTargetIds,
      optionalTargetIds,
      routePoints: routeState.path?.points,
      showHints: this.store.settings.showHints,
      debugOverlay: this.store.debugOverlayEnabled,
      patrolActors,
      reducedMotion: this.store.settings.reducedMotion,
      effectsQuality: this.store.settings.effectsQuality ?? 'full',
      transitionFadeAlpha: this.transitionFadeAlpha,
    };

    this.renderer.render(renderState);

    this.miniMap.render({
      map: this.currentMap,
      playerX: this.playerX,
      playerY: this.playerY,
      playerFacing: this.playerFacing,
      routeState,
      unlockedCheckpoints: this.store.unlockedCheckpoints,
      collectedMilestones: this.store.getActiveMilestones(),
      discoveredMaps: this.store.discoveredMaps,
      timeSec,
      reducedMotion: this.store.settings.reducedMotion,
    });
  }

  private readonly gameLoop = (nowMs: number): void => {
    const rawDt = Math.max(0.0001, (nowMs - this.lastFrameMs) * 0.001);
    const dt = Math.min(0.05, rawDt);
    this.lastFrameMs = nowMs;
    const instantFps = Math.min(240, 1 / rawDt);
    this.smoothedFps = this.smoothedFps * 0.9 + instantFps * 0.1;

    const lifecycle = this.store.getLifecycle();
    if (
      lifecycle === 'BOOT' ||
      lifecycle === 'MAIN_MENU' ||
      lifecycle === 'LOADING'
    ) {
      requestAnimationFrame(this.gameLoop);
      return;
    }

    const timeSec = nowMs * 0.001;
    if (lifecycle === 'PLAYING') {
      this.updatePhysics(dt, timeSec);
    }

    this.renderCurrentWorldFrame(timeSec);

    if (this.store.debugOverlayEnabled) {
      const metrics = this.gameShell.getMetrics();
      this.hud.setDebugOverlayUI(true, {
        mapId: this.currentMap.id,
        x: this.playerX,
        y: this.playerY,
        zoom: metrics.zoomPreset,
        scale: metrics.pixelScale,
        fps: this.smoothedFps,
      });
    }

    requestAnimationFrame(this.gameLoop);
  };

  public updatePhysics(dt: number, timeSec: number): void {
    if (this.store.getLifecycle() === 'PAUSED' || this.store.getMode() === 'paused') {
      return;
    }

    const chapter = this.currentChapter;
    const allowWorldInput = this.store.isWorldInputAllowed();
    const moveVec = this.input.getMoveVector(allowWorldInput);
    this.isCrouching = this.input.isCrouching(allowWorldInput);

    if (this.hazardCooldown > 0) {
      this.hazardCooldown = Math.max(0, this.hazardCooldown - dt);
    }

    if (this.transitionFadeAlpha > 0) {
      this.transitionFadeAlpha = Math.max(0, this.transitionFadeAlpha - dt * 2.6);
    }

    this.sceneTransition.update(dt, this.getPlayerFootRect(), this.currentMap.portals);
    this.store.portalCooldownSec = this.sceneTransition.getCooldownRemaining();

    if (allowWorldInput) {
      if (!this.useLegacyRenderer) {
        this.updateNpcRoutes(dt);

        const stealthRes = this.stealthSystem.update(
          dt,
          { x: this.playerX, y: this.playerY },
          this.isPlayerInShelter(),
          this.currentMap,
          this.collisionWorld,
          this.hasPatrolScheduleBonus()
        );

        if (stealthRes.triggeredAlertGuard && this.hazardCooldown <= 0) {
          this.resetPlayerToMapCheckpoint();
          this.hazardCooldown = 1.0;
          this.sound.playAlertPushback();
          this.hud.showToast(
            '◆ Đội tuần tra phát hiện! Lữ Khách đã rút lui về Đèn Báo An Toàn gần nhất (giữ nguyên mọi tư liệu).'
          );
          return;
        }
      }

      const speed = this.useLegacyRenderer ? (this.isCrouching ? 42 : 95) : 96;
      const dx = moveVec.x * speed * dt;
      const dy = this.useLegacyRenderer ? 0 : moveVec.y * speed * dt;

      if (dx !== 0 || dy !== 0) {
        const prevX = this.playerX;
        const prevY = this.playerY;
        if (this.useLegacyRenderer) {
          this.playerX = Math.max(24, Math.min(chapter.worldWidth - 28, this.playerX + dx));
        } else {
          const footRect = this.getPlayerFootRect();
          const resolved = this.collisionWorld.resolveMovement(footRect, dx, dy);
          this.playerX = resolved.x + 6;
          this.playerY = resolved.y + 6;
        }
        this.tutorialController.recordMovementDelta(
          this.playerX - prevX,
          this.playerY - prevY
        );

        if (Math.abs(moveVec.x) >= Math.abs(moveVec.y) && moveVec.x !== 0) {
          this.playerFacing = moveVec.x < 0 ? 'left' : 'right';
          this.facingRight = moveVec.x >= 0;
        } else if (moveVec.y !== 0) {
          this.playerFacing = moveVec.y < 0 ? 'up' : 'down';
        }

        this.store.playerPos = { x: this.playerX, y: this.playerY };
        this.store.playerFacing = this.playerFacing;

        this.stepSoundCooldown -= dt;
        if (this.stepSoundCooldown <= 0) {
          this.sound.playStep();
          this.stepSoundCooldown = 0.28;
        }

        // Kiểm tra tự động bước qua cổng (Portal) khi đi vào ô cửa
        if (!this.useLegacyRenderer && this.sceneTransition.getCooldownRemaining() <= 0) {
          this.allMaps = getAllBuiltInMaps();
          const portalRes = this.sceneTransition.tryPortalTransition(
            this.getPlayerFootRect(),
            this.currentMap.portals,
            this.allMaps,
            this.store.questFlags
          );
          if (portalRes.transitioned && portalRes.targetMapId && portalRes.spawnPos) {
            this.store.portalCooldownSec = this.sceneTransition.getCooldownRemaining();
            this.loadMap(
              portalRes.targetMapId,
              portalRes.targetSpawnId,
              portalRes.spawnPos,
              portalRes.spawnFacing ?? 'down'
            );
            this.persistProgress();
            this.syncHudWithState();
            this.hud.showToast(`◆ Đã chuyển tới: ${this.currentMap.title}`);
            return;
          }
        }
      }

      // Kiểm tra va chạm Mức 1 với Đèn tuần tra / Pháo sáng (chỉ đẩy lùi nhẹ về mái hiên)
      if (
        this.currentMap.id === 'harbor_square' &&
        !this.isPlayerInShelter() &&
        this.climaxBurstProgress <= 0 &&
        this.hazardCooldown <= 0
      ) {
        const detectionRadius = this.isCrouching ? 12 : 15;
        for (const h of chapter.hazards) {
          const sweepX = h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
          if (Math.abs(this.playerX - sweepX) < detectionRadius) {
            const pushedX = Math.max(36, this.playerX - 48);
            if (!this.collisionWorld.isRectColliding(this.getPlayerFootRect(pushedX, this.playerY))) {
              this.playerX = pushedX;
              this.store.playerPos = { x: this.playerX, y: this.playerY };
            }
            this.hazardCooldown = 1.0;
            this.sound.playAlertPushback();
            this.hud.showToast(`⚠ ${h.warningToast}`);
            break;
          }
        }
      }
    }

    // Cập nhật tiến trình Đại Cảnh Bùng Nổ (Bước B)
    if (this.isBurstAnimating) {
      this.climaxBurstProgress = Math.min(1, this.climaxBurstProgress + dt * 0.35);
      if (this.climaxBurstProgress >= 1) {
        this.isBurstAnimating = false;
        this.postBurstDelaySec = 1.6;
      }
    } else if (this.postBurstDelaySec > 0) {
      if (this.store.getTransitionToken() !== this.activeBurstToken) {
        this.postBurstDelaySec = 0;
      } else {
        this.postBurstDelaySec = Math.max(0, this.postBurstDelaySec - dt);
        if (this.postBurstDelaySec <= 0) {
          this.advanceToNextChapter();
        }
      }
    }

    // Camera mượt bám theo Lữ Khách trong biên bản đồ 2D
    if (this.useLegacyRenderer) {
      const targetCamX = Math.max(
        0,
        Math.min(
          chapter.worldWidth - INTERNAL_WIDTH,
          this.playerX - INTERNAL_WIDTH * 0.38
        )
      );
      this.cameraX += (targetCamX - this.cameraX) * Math.min(1, dt * 8);
      this.cameraY = 0;
    } else {
      const camPos = this.cameraController.update(this.playerX, this.playerY, dt);
      this.cameraX = camPos.x;
      this.cameraY = camPos.y;
    }

    const candidate = this.findNearestInteractionCandidate();
    const patrolActors = this.stealthSystem.getRenderStates(
      this.hasPatrolScheduleBonus()
    );
    const stealthAlertState = this.getCurrentStealthAlertState(patrolActors);
    const showStealthStatus =
      this.currentMap.id === 'shadow_alley' || stealthAlertState !== 'calm';

    this.hud.updateStageHeader(
      chapter,
      showStealthStatus && this.isPlayerInShelter(),
      showStealthStatus ? stealthAlertState : 'calm'
    );
    this.hud.updateMapLocationBadge(
      this.currentMap.title || chapter.locationLabel,
      chapter.coordinates,
      candidate?.promptText ?? null
    );
    this.updateScreenInteractionPrompt(candidate, allowWorldInput);
  }
}
