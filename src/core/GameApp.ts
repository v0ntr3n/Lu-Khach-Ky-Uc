/**
 * @file src/core/GameApp.ts
 * @description Bộ điều phối vòng đời chính của "Lữ Khách Ký Ức (1930 – 2045)".
 * Kết nối GameStore, InputManager, SaveManager, MapLoader, CameraController,
 * SceneTransition, CollisionWorld, InteractionSystem, QuestSystem, DialogueSystem,
 * EvidenceSystem, InventorySystem, StealthSystem, DossierPuzzle, OpeningFracture,
 * PixelRenderer, SoundSynth và HudController.
 */

import { CH1_DOSSIER_PUZZLE } from '../data/chapters/ch1_1930';
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
import { computeEntityDepth } from '../runtime/PhaserRuntime';
import { SceneTransition } from '../runtime/SceneTransition';
import { DialogueSystem } from '../systems/DialogueSystem';
import { EvidenceSystem } from '../systems/EvidenceSystem';
import { InteractionSystem, type InteractionCandidate } from '../systems/InteractionSystem';
import { InventorySystem } from '../systems/InventorySystem';
import { QuestSystem } from '../systems/QuestSystem';
import { StealthSystem } from '../systems/StealthSystem';
import type {
  ChapterData,
  DossierPuzzleDefinition,
  EraId,
  Facing4,
  GameMode,
  GameSettings,
  MilestoneArtifact,
  NormalizedMapData,
  PatrolActorRenderState,
  PortalDefinition,
  Rect2D,
  RomanMilestoneId,
  Vec2,
} from '../types/game';
import { HudController } from '../ui/HudController';
import { CollisionWorld } from '../world/CollisionWorld';
import { GameStore } from './GameStore';
import { InputManager } from './InputManager';
import { SaveManager } from './SaveManager';

interface HudExtendedMethods {
  toggleWorldMapModal?: (forceOpen?: boolean) => void;
  toggleJournalModal?: (forceOpen?: boolean) => void;
  toggleInventoryModal?: (forceOpen?: boolean) => void;
  togglePauseModal?: (forceOpen?: boolean) => void;
  openDossierPuzzleModal?: (
    puzzle: DossierPuzzleDefinition,
    attemptCount: number
  ) => void;
  closeDossierPuzzleModal?: () => void;
  closeAllOverlays?: () => void;
  updateWorldMapState?: (
    currentMapId: string,
    discoveredMaps: ReadonlySet<string>,
    unlockedCheckpoints: ReadonlySet<string>,
    isPresenterMode: boolean
  ) => void;
  setDebugOverlayUI?: (enabled: boolean) => void;
}

export class LuKhachKyUcGame {
  public readonly store: GameStore;
  public readonly input: InputManager;
  public readonly saveManager: SaveManager;
  public readonly renderer: PixelRenderer;
  public readonly sound: SoundSynth;
  public readonly hud: HudController;
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
  private activeBurstToken = 0;

  private lastFrameMs = performance.now();
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

    if (typeof window !== 'undefined' && window.location?.search) {
      const params = new URLSearchParams(window.location.search);
      this.useLegacyRenderer = params.get('renderer') === 'legacy';
    }

    this.store = new GameStore();
    this.saveManager = new SaveManager();
    this.interactionSystem = new InteractionSystem();
    this.questSystem = new QuestSystem();
    this.dialogueSystem = new DialogueSystem();
    this.evidenceSystem = new EvidenceSystem();
    this.inventorySystem = new InventorySystem(this.evidenceSystem);
    this.stealthSystem = new StealthSystem();

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
    this.cameraController = new CameraController({
      worldWidth: this.currentMap.widthPx,
      worldHeight: this.currentMap.heightPx,
      viewportWidth: INTERNAL_WIDTH,
      viewportHeight: INTERNAL_HEIGHT,
    });
    this.sceneTransition = new SceneTransition('harbor_square', 'default_spawn', 0.65);

    const saved = this.saveManager.load();
    if (saved) {
      this.store.restoreFromSave(saved, CHAPTERS);
      const savedMapId =
        saved.currentMapId && this.allMaps[saved.currentMapId]
          ? saved.currentMapId
          : 'harbor_square';
      this.currentMap = getBuiltInMap(savedMapId);
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
      this.cameraController.setBounds(
        this.currentMap.widthPx,
        this.currentMap.heightPx,
        INTERNAL_WIDTH,
        INTERNAL_HEIGHT
      );
      this.playerX = saved.playerPos.x;
      this.playerY =
        saved.playerPos.y && saved.playerPos.y !== 148 ? saved.playerPos.y : 384;
      this.playerFacing = saved.playerFacing ?? 'right';
      this.facingRight = this.playerFacing !== 'left';
    }

    this.store.setMapState(
      this.currentMap.id,
      this.store.currentSpawnId || 'default_spawn',
      { x: this.playerX, y: this.playerY },
      this.playerFacing
    );
    const initialCam = this.cameraController.snapTo(this.playerX, this.playerY);
    this.cameraX = initialCam.x;
    this.cameraY = initialCam.y;

    this.renderer = new PixelRenderer(canvas);
    this.sound = new SoundSynth();

    this.input = new InputManager({
      onInteract: () => {
        this.handleInteractAction();
      },
      onToggleLookup: () => {
        if (this.store.getMode() === 'opening' || this.store.getMode() === 'climax') {
          return;
        }
        this.hud.toggleLookupDrawer();
      },
      onToggleMap: () => {
        if (this.store.getMode() === 'opening' || this.store.getMode() === 'climax') {
          return;
        }
        this.syncHudWithState();
        this.hud.toggleWorldMapModal();
      },
      onToggleJournal: () => {
        if (this.store.getMode() === 'opening' || this.store.getMode() === 'climax') {
          return;
        }
        this.syncHudWithState();
        this.hud.toggleJournalModal();
      },
      onToggleInventory: () => {
        if (this.store.getMode() === 'opening' || this.store.getMode() === 'climax') {
          return;
        }
        this.syncHudWithState();
        this.hud.toggleInventoryModal();
      },
      onTogglePause: () => {
        if (this.store.getMode() === 'opening' || this.store.getMode() === 'climax') {
          return;
        }
        this.syncHudWithState();
        this.hud.togglePauseModal();
      },
      onToggleDebugOverlay: () => {
        this.toggleDebugOverlay();
      },
      onEscape: () => {
        if (this.store.getMode() === 'opening') {
          return;
        }
        if (this.hud.isOverlayOpen()) {
          this.dialogueSystem.closeDialogue();
          this.hud.closeAllOverlays();
          return;
        }
        this.syncHudWithState();
        this.hud.togglePauseModal(true);
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
        this.store.updateSettings({ muted });
        this.persistProgress();
        return muted;
      },
      onOverlayStateChange: (isOpen: boolean, overlayMode: GameMode) => {
        this.store.setMode(isOpen ? overlayMode : 'exploring');
        this.input.clearKeys();
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
      onUpdateSettings: (partial: Partial<GameSettings>) => {
        this.updateSettings(partial);
      },
      onSubmitDossierPuzzle: (
        puzzleId: string,
        selectedOptionIds: Record<string, string>
      ) => this.submitDossierPuzzle(puzzleId, selectedOptionIds),
    });

    this.hud.setPresenterModeUI(this.store.isPresenterMode);
    this.syncHudWithState();

    // Khởi chạy Mở Màn Nứt Vỡ Màng Đen 16 Mảnh (0.0s -> 4.8s)
    this.store.setMode('opening');
    const opening = new OpeningFractureController(
      '#rift-opening-overlay',
      '#rift-opening-canvas',
      () => {
        this.store.setMode('exploring');
        this.input.clearKeys();
      }
    );
    opening.start();

    requestAnimationFrame(this.gameLoop);
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
    (this.hud as unknown as HudExtendedMethods).setDebugOverlayUI?.(enabled);
    this.hud.showToast(
      enabled
        ? '🛠️ Đã bật Lớp Phủ Gỡ Lỗi (F3): Hiển thị vật cản, vùng ẩn nấp, cổng & lộ trình NPC.'
        : '🛠️ Đã tắt Lớp Phủ Gỡ Lỗi (F3).'
    );
    return enabled;
  }

  public updateSettings(partial: Partial<GameSettings>): GameSettings {
    const next = this.store.updateSettings(partial);
    this.persistProgress();
    this.syncHudWithState();
    return next;
  }

  public manualSave(): boolean {
    if (this.store.isPresenterMode) {
      this.hud.showToast(
        '⚠️ Đang ở Chế độ Giảng Viên: Không ghi đè dữ liệu lưu tiến trình thường.'
      );
      return false;
    }
    this.persistProgress();
    this.hud.showToast('💾 Đã lưu tiến trình Lữ Khách vào bộ nhớ cục bộ (SaveV2)!');
    return true;
  }

  public resetSave(): void {
    this.saveManager.clearSave();
    this.store.resetToDefault();
    this.puzzleAttemptCounts.clear();
    this.hud.setPresenterModeUI(false);
    this.loadMap('harbor_square', 'default_spawn', { x: 85, y: 384 }, 'right');
    this.hud.closeAllOverlays();
    this.store.setMode('exploring');
    this.syncHudWithState();
    this.hud.showToast('🔄 Đã xóa dữ liệu lưu và đặt lại hành trình từ đầu Chương I!');
  }

  public openDossierPuzzle(puzzle: DossierPuzzleDefinition = CH1_DOSSIER_PUZZLE): void {
    const attempts = this.puzzleAttemptCounts.get(puzzle.id) ?? 0;
    (this.hud as unknown as HudExtendedMethods).openDossierPuzzleModal?.(puzzle, attempts);
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
    const puzzle = CH1_DOSSIER_PUZZLE;
    const prevAttempts = this.puzzleAttemptCounts.get(puzzleId || puzzle.id) ?? 0;
    const attemptCount = prevAttempts + 1;
    this.puzzleAttemptCounts.set(puzzleId || puzzle.id, attemptCount);

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
          'Chưa chính xác! Hãy đối chiếu kỹ nội dung Chánh cương vắn tắt (02/1930) và thử lại.',
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
      '✓ Đã xác thực Cương lĩnh chính trị đầu tiên (02/1930) & khôi phục Cột mốc III!'
    );

    return {
      solved: true,
      attemptCount,
      feedback:
        '✓ Xác thực hoàn tất! Đã khôi phục Cột mốc III (1930 – 1931) và mở khóa Cổng Thời Gian 1930.',
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

  public openDialogueForNpc(npcIdOrDialogueId: string, milestone?: MilestoneArtifact): boolean {
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

    this.cameraController.setBounds(
      map.widthPx,
      map.heightPx,
      INTERNAL_WIDTH,
      INTERNAL_HEIGHT
    );
    const cam = this.cameraController.snapTo(this.playerX, this.playerY);
    this.cameraX = cam.x;
    this.cameraY = cam.y;

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
        this.hud.showToast('🔒 Lối đi này cần hoàn thành nhiệm vụ liên lạc trước.');
      }
      return false;
    }

    this.store.portalCooldownSec = this.sceneTransition.getCooldownRemaining();
    this.loadMap(
      res.targetMapId,
      res.targetSpawnId,
      res.spawnPos,
      res.spawnFacing ?? 'down'
    );
    this.persistProgress();
    this.syncHudWithState();
    this.hud.showToast(`🚪 Đã chuyển tới: ${this.currentMap.title}`);
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
        '🔒 Khu vực này chưa được khám phá hoặc chưa kích hoạt Đèn Báo An Toàn!'
      );
      return false;
    }

    this.loadMap(zoneId, checkpointSpawnId);
    this.hud.toggleWorldMapModal(false);
    if (this.store.getMode() !== 'opening') {
      this.store.setMode('exploring');
    }
    this.input.clearKeys();
    this.persistProgress();
    this.syncHudWithState();
    this.hud.showToast(`🧭 Đã dịch chuyển nhanh tới: ${this.currentMap.title}`);
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

  public switchEra(eraId: EraId): void {
    const idx = CHAPTERS.findIndex((c) => c.id === eraId);
    if (idx === -1) return;

    this.store.bumpTransitionToken();
    this.store.currentChapterIndex = idx;
    this.loadMap('harbor_square', 'default_spawn', { x: 85, y: 384 }, 'right');
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.hazardCooldown = 0;

    this.hud.closeAllOverlays();
    if (this.store.getMode() !== 'opening') {
      this.store.setMode('exploring');
    }
    this.input.clearKeys();

    this.persistProgress();
    this.syncHudWithState();
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
          '🔒 Cột mốc chưa khôi phục! Hãy khám phá trong thế giới hoặc bật Chế độ Giảng Viên.'
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
    }

    this.store.presenterCollectedMilestones.add(m.roman);
    this.sound.playSealStamp();
    this.hud.showArtifactDialogue(m);
    this.syncHudWithState();
  }

  public togglePresenterMode(): void {
    const isNowPresenter = this.store.togglePresenterMode();
    this.hud.setPresenterModeUI(isNowPresenter);

    if (isNowPresenter) {
      this.sound.playClimaxFanfare();
      const firstOfChapter = this.currentChapter.milestones[0];
      if (firstOfChapter) {
        this.hud.showArtifactDialogue(firstOfChapter);
      }
      this.hud.showToast(
        '⚡ Chế độ Giảng Viên: Đã mở khóa toàn bộ 12/12 Cột Mốc La Mã (I – XII)!'
      );
    } else {
      this.hud.showToast('Đã tắt Chế độ Giảng Viên — Khôi phục tiến trình khám phá thực tế.');
    }

    this.syncHudWithState();
  }

  public unlockAllMilestonesForLecturer(): void {
    this.togglePresenterMode();
  }

  public handleInteractAction(): boolean {
    if (!this.store.isWorldInputAllowed()) {
      return false;
    }

    const chapter = this.currentChapter;
    const candidate = this.findNearestInteractionCandidate();

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
        `🏮 Đã kích hoạt Điểm Lưu & Dịch Chuyển: ${candidate.interactable.label}`
      );
      return true;
    }

    // 3. Tương tác với Bàn Đối Chiếu Hồ Sơ Lịch Sử (Dossier Puzzle)
    if (candidate?.kind === 'puzzle') {
      this.openDossierPuzzle(CH1_DOSSIER_PUZZLE);
      return true;
    }

    // 4. Tương tác trực tiếp với Cổng Thời Gian cuối màn (Climax Gate)
    if (candidate?.kind === 'climax_gate') {
      const check = this.store.canTriggerChapterClimax(chapter);
      if (!check.allowed) {
        this.hud.showToast(
          `🔒 Cần khôi phục đủ Cột mốc ${check.missing.join(', ')} trước khi mở Cổng Thời Gian!`
        );
        return false;
      }
      this.hud.openClimaxModal(chapter);
      return true;
    }

    // 5. Tương tác với Cột mốc lịch sử hoặc NPC / Tư liệu gắn với cột mốc
    const nearbyMilestone = candidate?.milestone ?? this.findNearbyMilestone();
    if (nearbyMilestone) {
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
      this.hud.showToast(`📜 Đã thu thập tư liệu: ${candidate.interactable.label}`);
      this.syncHudWithState();
      return true;
    }

    // 8. Kiểm tra nếu đứng gần Cổng Thời Gian cuối màn (1D fallback)
    const nearClimaxGate =
      Math.abs(this.playerX - chapter.climax.gateWorldX) <= 38;

    if (nearClimaxGate) {
      const check = this.store.canTriggerChapterClimax(chapter);
      if (!check.allowed) {
        this.hud.showToast(
          `🔒 Cần khôi phục đủ Cột mốc ${check.missing.join(', ')} trước khi mở Cổng Thời Gian!`
        );
        return false;
      }
      this.hud.openClimaxModal(chapter);
      return true;
    }

    this.hud.showToast('Hãy di chuyển lại gần Cột Mốc La Mã, Cửa Kho hoặc Cổng Thời Gian rồi bấm [E].');
    return false;
  }

  public triggerClimaxStepBSetPiece(): void {
    const chapter = this.currentChapter;
    const token = this.store.bumpTransitionToken();
    this.activeBurstToken = token;

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
    this.hud.showToast(`✨ ${chapter.climax.stepBTitle}`);
  }

  public advanceToNextChapter(): void {
    this.store.bumpTransitionToken();

    if (this.store.currentChapterIndex < CHAPTERS.length - 1) {
      this.store.currentChapterIndex++;
      this.loadMap('harbor_square', 'default_spawn', { x: 85, y: 384 }, 'right');
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.hazardCooldown = 0;
      this.persistProgress();
      this.syncHudWithState();
      this.hud.showToast(
        `Đã mở Cổng Thời Gian sang ${this.currentChapter.chapterNum}: ${this.currentChapter.title}`
      );
    } else {
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.hud.showToast(
        '🎉 Chúc mừng Lữ Khách! Bạn đã khôi phục trọn vẹn Biên Niên Sử 1930 – 2045 (12/12 Cột mốc)!'
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
    const inMapShelter = this.collisionWorld.isPointInShelter(this.playerX, this.playerY);
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

  public syncHudWithState(): void {
    const chapter = this.currentChapter;
    this.questSystem.evaluateProgress(this.store, chapter.id);

    const candidate = this.findNearestInteractionCandidate();
    const nearby = candidate?.milestone ?? this.findNearbyMilestone();
    this.hud.renderSidebarCodex(
      chapter,
      this.store.getActiveMilestones(),
      nearby
    );
    this.hud.updateStageHeader(chapter, this.isPlayerInShelter());
    this.hud.updateMapLocationBadge(
      this.currentMap.title || chapter.locationLabel,
      chapter.coordinates,
      candidate?.promptText ?? null
    );
    this.hud.updateWorldMapState(
      this.currentMap.id,
      this.store.discoveredMaps,
      this.store.unlockedCheckpoints,
      this.store.isPresenterMode
    );

    const eraQuests = this.questSystem.getQuestsForEra(chapter.id);
    const questsWithStatus = eraQuests.map((quest) => ({
      quest,
      objectives: quest.objectives.map((objective) => ({
        objective,
        status: this.questSystem.getObjectiveStatus(objective, this.store),
      })),
    }));
    const activeObjective = this.questSystem.getCurrentActiveObjective(
      chapter.id,
      this.store
    );
    const hintText = this.questSystem.getHintForActiveObjective(
      chapter.id,
      this.store
    );

    this.hud.updateRpgSystemsState({
      quests: questsWithStatus,
      activeObjective,
      hintText,
      allEvidence: [...this.evidenceSystem.getAllEvidence()],
      collectedEvidenceIds: this.store.collectedEvidence,
      settings: this.store.settings,
      isPresenterMode: this.store.isPresenterMode,
    });
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

  private readonly gameLoop = (nowMs: number): void => {
    const dt = Math.min(0.05, (nowMs - this.lastFrameMs) * 0.001);
    this.lastFrameMs = nowMs;
    const timeSec = nowMs * 0.001;

    this.updatePhysics(dt, timeSec);

    const candidate = this.findNearestInteractionCandidate();
    const nearbyMilestone = candidate?.milestone ?? this.findNearbyMilestone();
    const nearClimaxGate =
      candidate?.kind === 'climax_gate' ||
      Math.abs(this.playerX - this.currentChapter.climax.gateWorldX) <= 38;

    const allowWorldInput = this.store.isWorldInputAllowed();

    const renderState: RenderState & {
      debugOverlay?: boolean;
      patrolActors?: PatrolActorRenderState[];
    } = {
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
      debugOverlay: this.store.debugOverlayEnabled,
      patrolActors: this.stealthSystem.getRenderStates(this.hasPatrolScheduleBonus()),
    };

    this.renderer.render(renderState);

    requestAnimationFrame(this.gameLoop);
  };

  public updatePhysics(dt: number, timeSec: number): void {
    const chapter = this.currentChapter;
    const allowWorldInput = this.store.isWorldInputAllowed();
    const moveVec = this.input.getMoveVector(allowWorldInput);
    this.isCrouching = this.input.isCrouching(allowWorldInput);

    if (this.hazardCooldown > 0) {
      this.hazardCooldown = Math.max(0, this.hazardCooldown - dt);
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
            '🚨 Đội tuần tra phát hiện! Lữ Khách đã rút lui về Đèn Báo An Toàn gần nhất (giữ nguyên mọi tư liệu).'
          );
          return;
        }
      }

      const speed = this.useLegacyRenderer ? (this.isCrouching ? 42 : 95) : 96;
      const dx = moveVec.x * speed * dt;
      const dy = this.useLegacyRenderer ? 0 : moveVec.y * speed * dt;

      if (dx !== 0 || dy !== 0) {
        if (this.useLegacyRenderer) {
          this.playerX = Math.max(24, Math.min(chapter.worldWidth - 28, this.playerX + dx));
        } else {
          const footRect = this.getPlayerFootRect();
          const resolved = this.collisionWorld.resolveMovement(footRect, dx, dy);
          this.playerX = resolved.x + 6;
          this.playerY = resolved.y + 6;
        }

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
            this.hud.showToast(`🚪 Đã chuyển tới: ${this.currentMap.title}`);
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
            this.hud.showToast(`⚠️ ${h.warningToast}`);
            break;
          }
        }
      }
    }

    // Cập nhật tiến trình Đại Cảnh Bùng Nổ (Bước B)
    if (this.isBurstAnimating) {
      const token = this.activeBurstToken;
      this.climaxBurstProgress = Math.min(1, this.climaxBurstProgress + dt * 0.35);
      if (this.climaxBurstProgress >= 1) {
        this.isBurstAnimating = false;
        window.setTimeout(() => {
          if (this.store.getTransitionToken() !== token) return;
          this.advanceToNextChapter();
        }, 1600);
      }
    }

    // Camera mượt bám theo Lữ Khách trong biên bản đồ 2D
    if (this.useLegacyRenderer) {
      const targetCamX = Math.max(
        0,
        Math.min(chapter.worldWidth - INTERNAL_WIDTH, this.playerX - INTERNAL_WIDTH * 0.38)
      );
      this.cameraX += (targetCamX - this.cameraX) * Math.min(1, dt * 8);
      this.cameraY = 0;
    } else {
      const camPos = this.cameraController.update(this.playerX, this.playerY, dt);
      this.cameraX = camPos.x;
      this.cameraY = camPos.y;
    }

    const candidate = this.findNearestInteractionCandidate();
    this.hud.updateStageHeader(chapter, this.isPlayerInShelter());
    this.hud.updateMapLocationBadge(
      this.currentMap.title || chapter.locationLabel,
      chapter.coordinates,
      candidate?.promptText ?? null
    );
  }
}
