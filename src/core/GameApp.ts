/**
 * @file src/core/GameApp.ts
 * @description Bộ điều phối vòng đời chính của "Lữ Khách Ký Ức (1930 – 2045)".
 * Kết nối GameStore, InputManager, SaveManager, OpeningFracture, PixelRenderer (320x180),
 * SoundSynth và HudController.
 */

import { CHAPTERS } from '../data/chronicles';
import { OpeningFractureController } from '../engine/OpeningFracture';
import { INTERNAL_WIDTH, PixelRenderer } from '../engine/PixelRenderer';
import { SoundSynth } from '../engine/SoundSynth';
import type {
  ChapterData,
  EraId,
  GameMode,
  MilestoneArtifact,
  RomanMilestoneId,
} from '../types/game';
import { HudController } from '../ui/HudController';
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

  public playerX = 85;
  public cameraX = 0;
  public facingRight = true;
  public isCrouching = false;

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

    this.store = new GameStore();
    this.saveManager = new SaveManager();

    const saved = this.saveManager.load();
    if (saved) {
      this.store.restoreFromSave(saved, CHAPTERS);
      this.playerX = saved.playerPos.x;
      this.facingRight = saved.playerFacing !== 'left';
    }

    this.renderer = new PixelRenderer(canvas);
    this.sound = new SoundSynth();

    this.input = new InputManager({
      onInteract: () => this.handleInteractAction(),
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

  private persistProgress(): void {
    if (this.store.isPresenterMode) return;
    const data = this.store.toSaveDataV2(
      this.currentChapter.id,
      this.playerX,
      this.facingRight ? 'right' : 'left'
    );
    this.saveManager.save(data);
  }

  public switchEra(eraId: EraId): void {
    const idx = CHAPTERS.findIndex((c) => c.id === eraId);
    if (idx === -1) return;

    this.store.bumpTransitionToken();
    this.store.currentChapterIndex = idx;
    this.playerX = 85;
    this.cameraX = 0;
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
    this.playerX = m.worldX;
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

  public handleInteractAction(): void {
    if (!this.store.isWorldInputAllowed()) {
      return;
    }

    const chapter = this.currentChapter;

    // Kiểm tra nếu đứng gần Cột mốc lịch sử
    const nearbyMilestone = this.findNearbyMilestone();
    if (nearbyMilestone) {
      this.store.collectMilestone(nearbyMilestone.roman);
      this.persistProgress();
      this.sound.playSealStamp();
      this.hud.showArtifactDialogue(nearbyMilestone);
      this.syncHudWithState();
      this.hud.showToast(
        `✓ Đã đóng dấu khôi phục Cột mốc ${nearbyMilestone.roman} (${nearbyMilestone.year})`
      );
      return;
    }

    // Kiểm tra nếu đứng gần Cổng Thời Gian cuối màn
    if (Math.abs(this.playerX - chapter.climax.gateWorldX) <= 38) {
      const check = this.store.canTriggerChapterClimax(chapter);
      if (!check.allowed) {
        this.hud.showToast(
          `🔒 Cần khôi phục đủ Cột mốc ${check.missing.join(', ')} trước khi mở Cổng Thời Gian!`
        );
        return;
      }
      this.hud.openClimaxModal(chapter);
      return;
    }

    this.hud.showToast('Hãy di chuyển lại gần Cột Mốc La Mã hoặc Cổng Thời Gian rồi bấm [E].');
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
      this.playerX = 85;
      this.cameraX = 0;
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

  public findNearbyMilestone(): MilestoneArtifact | null {
    for (const m of this.currentChapter.milestones) {
      if (Math.abs(this.playerX - m.worldX) <= 28) {
        return m;
      }
    }
    return null;
  }

  public isPlayerInShelter(): boolean {
    return this.store.isPlayerInShelter(
      this.playerX,
      this.currentChapter.shelters,
      this.isCrouching
    );
  }

  public syncHudWithState(): void {
    const nearby = this.findNearbyMilestone();
    this.hud.renderSidebarCodex(
      this.currentChapter,
      this.store.getActiveMilestones(),
      nearby
    );
    this.hud.updateStageHeader(this.currentChapter, this.isPlayerInShelter());
  }

  private readonly gameLoop = (nowMs: number): void => {
    const dt = Math.min(0.05, (nowMs - this.lastFrameMs) * 0.001);
    this.lastFrameMs = nowMs;
    const timeSec = nowMs * 0.001;

    this.updatePhysics(dt, timeSec);

    const nearbyMilestone = this.findNearbyMilestone();
    const nearClimaxGate =
      Math.abs(this.playerX - this.currentChapter.climax.gateWorldX) <= 38;

    const allowWorldInput = this.store.isWorldInputAllowed();

    this.renderer.render({
      chapter: this.currentChapter,
      playerX: this.playerX,
      cameraX: this.cameraX,
      facingRight: this.facingRight,
      isMoving: this.input.isMoving(allowWorldInput),
      isSheltered: this.isPlayerInShelter(),
      collectedMilestones: this.store.getActiveMilestones(),
      nearbyMilestone,
      nearClimaxGate,
      climaxBurstProgress: this.climaxBurstProgress,
      timeSec,
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

    if (allowWorldInput) {
      const speed = this.isCrouching ? 42 : 95;

      if (moveVec.x < 0) {
        this.playerX = Math.max(24, this.playerX + moveVec.x * speed * dt);
        this.facingRight = false;
      } else if (moveVec.x > 0) {
        this.playerX = Math.min(chapter.worldWidth - 28, this.playerX + moveVec.x * speed * dt);
        this.facingRight = true;
      }

      if (moveVec.x !== 0 && !this.isCrouching) {
        this.stepSoundCooldown -= dt;
        if (this.stepSoundCooldown <= 0) {
          this.sound.playStep();
          this.stepSoundCooldown = 0.28;
        }
      }

      // Kiểm tra va chạm Mức 1 với Đèn tuần tra / Pháo sáng (chỉ đẩy lùi nhẹ về mái hiên)
      if (!this.isPlayerInShelter() && this.climaxBurstProgress <= 0 && this.hazardCooldown <= 0) {
        const detectionRadius = this.isCrouching ? 12 : 15;
        for (const h of chapter.hazards) {
          const sweepX = h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
          if (Math.abs(this.playerX - sweepX) < detectionRadius) {
            this.playerX = Math.max(36, this.playerX - 48);
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

    // Camera mượt bám theo Lữ Khách
    const targetCamX = Math.max(
      0,
      Math.min(chapter.worldWidth - INTERNAL_WIDTH, this.playerX - INTERNAL_WIDTH * 0.38)
    );
    this.cameraX += (targetCamX - this.cameraX) * Math.min(1, dt * 8);

    this.hud.updateStageHeader(chapter, this.isPlayerInShelter());
  }
}
