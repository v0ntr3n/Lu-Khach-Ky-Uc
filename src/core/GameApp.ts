/**
 * @file src/core/GameApp.ts
 * @description Bộ điều phối vòng đời chính của "Lữ Khách Ký Ức (1930 – 2045)".
 * Kết nối GameStore, InputManager, SaveManager, MapLoader, CameraController,
 * SceneTransition, CollisionWorld, InteractionSystem, OpeningFracture,
 * PixelRenderer (Top-Down 2D & 1D Fallback), SoundSynth và HudController.
 */

import { CHAPTERS } from '../data/chronicles';
import { OpeningFractureController } from '../engine/OpeningFracture';
import { INTERNAL_HEIGHT, INTERNAL_WIDTH, PixelRenderer } from '../engine/PixelRenderer';
import { SoundSynth } from '../engine/SoundSynth';
import { CameraController } from '../runtime/CameraController';
import { getAllBuiltInMaps, getBuiltInMap } from '../runtime/MapLoader';
import { computeEntityDepth } from '../runtime/PhaserRuntime';
import { SceneTransition } from '../runtime/SceneTransition';
import { InteractionSystem, type InteractionCandidate } from '../systems/InteractionSystem';
import type {
  ChapterData,
  EraId,
  Facing4,
  GameMode,
  MilestoneArtifact,
  NormalizedMapData,
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

export class LuKhachKyUcGame {
  public readonly store: GameStore;
  public readonly input: InputManager;
  public readonly saveManager: SaveManager;
  public readonly renderer: PixelRenderer;
  public readonly sound: SoundSynth;
  public readonly hud: HudController;
  public readonly interactionSystem: InteractionSystem;
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
      onEscape: () => {
        if (this.store.getMode() === 'opening') {
          return;
        }
        this.hud.closeClimaxModal();
        if (document.getElementById('lookup-drawer')?.classList.contains('is-open')) {
          this.hud.toggleLookupDrawer(false);
        }
      },
    });

    this.hud = new HudController({
      onSelectEra: (eraId) => this.switchEra(eraId),
      onTeleportToMilestone: (m) => this.teleportToMilestone(m),
      onTriggerClimaxStepB: () => this.triggerClimaxStepBSetPiece(),
      onAdvanceNextChapter: () => this.advanceToNextChapter(),
      onToggleLecturerUnlockAll: () => this.togglePresenterMode(),
      onToggleMute: () => this.sound.toggleMute(),
      onOverlayStateChange: (isOpen: boolean, overlayMode: GameMode) => {
        this.store.setMode(isOpen ? overlayMode : 'exploring');
        this.input.clearKeys();
      },
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

  public loadMap(
    mapId: string,
    spawnId?: string,
    posOverride?: Vec2,
    facingOverride?: Facing4
  ): NormalizedMapData {
    const map = this.allMaps[mapId] ?? getBuiltInMap(mapId);
    this.currentMap = map;
    this.collisionWorld = new CollisionWorld(map.obstacles, map.shelters, {
      x: 0,
      y: 0,
      width: map.widthPx,
      height: map.heightPx,
    });

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

    this.hud.closeClimaxModal();
    if (document.getElementById('lookup-drawer')?.classList.contains('is-open')) {
      this.hud.toggleLookupDrawer(false);
    }
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

    // 2. Tương tác với Cột mốc lịch sử hoặc NPC / Tư liệu gắn với cột mốc
    const nearbyMilestone = candidate?.milestone ?? this.findNearbyMilestone();
    if (nearbyMilestone) {
      this.store.collectMilestone(nearbyMilestone.roman);
      if (candidate?.interactable?.evidenceId && !this.store.isPresenterMode) {
        this.store.collectedEvidence.add(candidate.interactable.evidenceId);
      }
      if (candidate?.interactable?.grantsFlag && !this.store.isPresenterMode) {
        this.store.questFlags.add(candidate.interactable.grantsFlag);
      }
      this.persistProgress();
      this.sound.playSealStamp();
      this.hud.showArtifactDialogue(nearbyMilestone);
      this.syncHudWithState();
      this.hud.showToast(
        `✓ Đã đóng dấu khôi phục Cột mốc ${nearbyMilestone.roman} (${nearbyMilestone.year})`
      );
      return true;
    }

    // 3. Tương tác với vật thể / bảng tin / hòm hàng trong bản đồ 2D
    if (candidate?.interactable && candidate.kind !== 'climax_gate') {
      if (candidate.interactable.grantsFlag && !this.store.isPresenterMode) {
        this.store.questFlags.add(candidate.interactable.grantsFlag);
      }
      if (candidate.interactable.evidenceId && !this.store.isPresenterMode) {
        this.store.collectedEvidence.add(candidate.interactable.evidenceId);
      }
      this.sound.playSealStamp();
      this.hud.showToast(`📜 Đã kiểm tra: ${candidate.interactable.label}`);
      this.syncHudWithState();
      return true;
    }

    // 4. Kiểm tra nếu đứng gần Cổng Thời Gian cuối màn
    const nearClimaxGate =
      candidate?.kind === 'climax_gate' ||
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

  public syncHudWithState(): void {
    const candidate = this.findNearestInteractionCandidate();
    const nearby = candidate?.milestone ?? this.findNearbyMilestone();
    this.hud.renderSidebarCodex(
      this.currentChapter,
      this.store.getActiveMilestones(),
      nearby
    );
    this.hud.updateStageHeader(this.currentChapter, this.isPlayerInShelter());
    this.hud.updateMapLocationBadge(
      this.currentMap.title || this.currentChapter.locationLabel,
      this.currentChapter.coordinates,
      candidate?.promptText ?? null
    );
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

    this.renderer.render({
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
    });

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
