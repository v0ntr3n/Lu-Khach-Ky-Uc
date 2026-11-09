/**
 * @file src/main.ts
 * @description Bộ điều phối vòng đời chính của "Lữ Khách Ký Ức (1930 – 2045)".
 * Kết nối Mở màn 16 mảnh vỡ (OpeningFracture), PixelRenderer (320x180), SoundSynth và HudController.
 */

import { CHAPTERS } from './data/chronicles';
import { OpeningFractureController } from './engine/OpeningFracture';
import { INTERNAL_WIDTH, PixelRenderer } from './engine/PixelRenderer';
import { SoundSynth } from './engine/SoundSynth';
import type { ChapterData, EraId, MilestoneArtifact, RomanMilestoneId } from './types/game';
import { HudController } from './ui/HudController';

class LuKhachKyUcGame {
  private renderer: PixelRenderer;
  private sound: SoundSynth;
  private hud: HudController;

  private currentChapterIndex = 0;
  private playerX = 85;
  private cameraX = 0;
  private facingRight = true;
  private isCrouching = false;

  private keysPressed = new Set<string>();
  private collectedMilestones = new Set<RomanMilestoneId>();
  private climaxBurstProgress = 0;
  private isBurstAnimating = false;

  private lastFrameMs = performance.now();
  private stepSoundCooldown = 0;

  constructor() {
    const canvas = document.getElementById('pixel-canvas') as HTMLCanvasElement | null;
    if (!canvas) {
      throw new Error('Không tìm thấy phần tử #pixel-canvas.');
    }

    this.renderer = new PixelRenderer(canvas);
    this.sound = new SoundSynth();
    this.hud = new HudController({
      onSelectEra: (eraId) => this.switchEra(eraId),
      onTeleportToMilestone: (m) => this.teleportToMilestone(m),
      onTriggerClimaxStepB: () => this.triggerClimaxStepBSetPiece(),
      onAdvanceNextChapter: () => this.advanceToNextChapter(),
      onToggleLecturerUnlockAll: () => this.unlockAllMilestonesForLecturer(),
      onToggleMute: () => this.sound.toggleMute(),
    });

    this.bindInputListeners();
    this.syncHudWithState();

    // Khởi chạy Mở Màn Nứt Vỡ Màng Đen 16 Mảnh (0.0s -> 4.8s)
    const opening = new OpeningFractureController(
      '#rift-opening-overlay',
      '#rift-opening-canvas'
    );
    opening.start();

    requestAnimationFrame(this.gameLoop);
  }

  private get currentChapter(): ChapterData {
    return CHAPTERS[this.currentChapterIndex];
  }

  private bindInputListeners(): void {
    window.addEventListener('keydown', (e) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;

      const key = e.key.toLowerCase();
      this.keysPressed.add(key);

      if (key === 'e' || e.key === ' ') {
        e.preventDefault();
        this.handleInteractAction();
      } else if (key === 'q') {
        e.preventDefault();
        this.hud.toggleLookupDrawer();
      } else if (e.key === 'Escape') {
        this.hud.closeClimaxModal();
        this.hud.toggleLookupDrawer(false);
      }
    });

    window.addEventListener('keyup', (e) => {
      this.keysPressed.delete(e.key.toLowerCase());
    });

    // Nút điều khiển cảm ứng / chuột dưới sân khấu
    const bindHoldBtn = (id: string, virtualKey: string) => {
      const el = document.getElementById(id);
      if (!el) return;
      const press = () => this.keysPressed.add(virtualKey);
      const release = () => this.keysPressed.delete(virtualKey);
      el.addEventListener('pointerdown', press);
      el.addEventListener('pointerup', release);
      el.addEventListener('pointerleave', release);
    };

    bindHoldBtn('ctrl-left', 'arrowleft');
    bindHoldBtn('ctrl-right', 'arrowright');
    bindHoldBtn('ctrl-shelter', 'arrowdown');

    document.getElementById('ctrl-interact')?.addEventListener('click', () => {
      this.handleInteractAction();
    });
  }

  private switchEra(eraId: EraId): void {
    const idx = CHAPTERS.findIndex((c) => c.id === eraId);
    if (idx === -1) return;
    this.currentChapterIndex = idx;
    this.playerX = 85;
    this.cameraX = 0;
    this.climaxBurstProgress = 0;
    this.isBurstAnimating = false;
    this.hud.closeClimaxModal();
    this.syncHudWithState();
    this.hud.showToast(`Đã bước qua vết nứt thời gian tới ${this.currentChapter.chapterNum} (${this.currentChapter.years})`);
  }

  private teleportToMilestone(m: MilestoneArtifact): void {
    const eraIdx = CHAPTERS.findIndex((c) => c.id === m.eraId);
    if (eraIdx !== -1 && eraIdx !== this.currentChapterIndex) {
      this.currentChapterIndex = eraIdx;
      this.climaxBurstProgress = 0;
    }
    this.playerX = m.worldX;
    this.collectedMilestones.add(m.roman);
    this.sound.playSealStamp();
    this.hud.showArtifactDialogue(m);
    this.syncHudWithState();
  }

  private unlockAllMilestonesForLecturer(): void {
    for (const ch of CHAPTERS) {
      for (const m of ch.milestones) {
        this.collectedMilestones.add(m.roman);
      }
    }
    this.sound.playClimaxFanfare();
    const firstOfChapter = this.currentChapter.milestones[0];
    if (firstOfChapter) {
      this.hud.showArtifactDialogue(firstOfChapter);
    }
    this.syncHudWithState();
    this.hud.showToast('⚡ Chế độ Giảng Viên: Đã mở khóa toàn bộ 12/12 Cột Mốc La Mã (I – XII)!');
  }

  private handleInteractAction(): void {
    const chapter = this.currentChapter;

    // Kiểm tra nếu đứng gần Cột mốc lịch sử
    const nearbyMilestone = this.findNearbyMilestone();
    if (nearbyMilestone) {
      this.collectedMilestones.add(nearbyMilestone.roman);
      this.sound.playSealStamp();
      this.hud.showArtifactDialogue(nearbyMilestone);
      this.syncHudWithState();
      this.hud.showToast(`✓ Đã đóng dấu khôi phục Cột mốc ${nearbyMilestone.roman} (${nearbyMilestone.year})`);
      return;
    }

    // Kiểm tra nếu đứng gần Cổng Thời Gian cuối màn
    if (Math.abs(this.playerX - chapter.climax.gateWorldX) <= 38) {
      this.hud.openClimaxModal(chapter);
      return;
    }

    this.hud.showToast('Hãy di chuyển lại gần Cột Mốc La Mã hoặc Cổng Thời Gian rồi bấm [E].');
  }

  private triggerClimaxStepBSetPiece(): void {
    const chapter = this.currentChapter;
    for (const m of chapter.milestones) {
      this.collectedMilestones.add(m.roman);
    }
    this.sound.playClimaxFanfare();
    this.isBurstAnimating = true;
    this.climaxBurstProgress = 0.05;
    this.syncHudWithState();
    this.hud.showToast(`✨ ${chapter.climax.stepBTitle}`);
  }

  private advanceToNextChapter(): void {
    if (this.currentChapterIndex < CHAPTERS.length - 1) {
      this.currentChapterIndex++;
      this.playerX = 85;
      this.cameraX = 0;
      this.climaxBurstProgress = 0;
      this.isBurstAnimating = false;
      this.syncHudWithState();
      this.hud.showToast(`Đã mở Cổng Thời Gian sang ${this.currentChapter.chapterNum}: ${this.currentChapter.title}`);
    } else {
      this.hud.showToast('🎉 Chúc mừng Lữ Khách! Bạn đã khôi phục trọn vẹn Biên Niên Sử 1930 – 2045 (12/12 Cột mốc)!');
    }
  }

  private findNearbyMilestone(): MilestoneArtifact | null {
    for (const m of this.currentChapter.milestones) {
      if (Math.abs(this.playerX - m.worldX) <= 28) {
        return m;
      }
    }
    return null;
  }

  private isPlayerInShelter(): boolean {
    if (this.isCrouching) return true;
    for (const s of this.currentChapter.shelters) {
      if (this.playerX >= s.xStart && this.playerX <= s.xEnd) {
        return true;
      }
    }
    return false;
  }

  private syncHudWithState(): void {
    const nearby = this.findNearbyMilestone();
    this.hud.renderSidebarCodex(
      this.currentChapter,
      this.collectedMilestones,
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

    this.renderer.render({
      chapter: this.currentChapter,
      playerX: this.playerX,
      cameraX: this.cameraX,
      facingRight: this.facingRight,
      isMoving: this.isMoving(),
      isSheltered: this.isPlayerInShelter(),
      collectedMilestones: this.collectedMilestones,
      nearbyMilestone,
      nearClimaxGate,
      climaxBurstProgress: this.climaxBurstProgress,
      timeSec,
    });

    requestAnimationFrame(this.gameLoop);
  };

  private isMoving(): boolean {
    return (
      this.keysPressed.has('a') ||
      this.keysPressed.has('arrowleft') ||
      this.keysPressed.has('d') ||
      this.keysPressed.has('arrowright')
    );
  }

  private updatePhysics(dt: number, timeSec: number): void {
    const chapter = this.currentChapter;
    const moveLeft = this.keysPressed.has('a') || this.keysPressed.has('arrowleft');
    const moveRight = this.keysPressed.has('d') || this.keysPressed.has('arrowright');
    this.isCrouching = this.keysPressed.has('s') || this.keysPressed.has('arrowdown');

    const speed = this.isCrouching ? 42 : 95;

    if (moveLeft && !moveRight) {
      this.playerX = Math.max(24, this.playerX - speed * dt);
      this.facingRight = false;
    } else if (moveRight && !moveLeft) {
      this.playerX = Math.min(chapter.worldWidth - 28, this.playerX + speed * dt);
      this.facingRight = true;
    }

    if ((moveLeft || moveRight) && !this.isCrouching) {
      this.stepSoundCooldown -= dt;
      if (this.stepSoundCooldown <= 0) {
        this.sound.playStep();
        this.stepSoundCooldown = 0.28;
      }
    }

    // Kiểm tra va chạm Mức 1 với Đèn tuần tra / Pháo sáng (chỉ đẩy lùi nhẹ về mái hiên)
    if (!this.isPlayerInShelter() && this.climaxBurstProgress <= 0) {
      for (const h of chapter.hazards) {
        const sweepX = h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
        if (Math.abs(this.playerX - sweepX) < 15) {
          this.playerX = Math.max(36, this.playerX - 48);
          this.sound.playAlertPushback();
          this.hud.showToast(`⚠️ ${h.warningToast}`);
          break;
        }
      }
    }

    // Cập nhật tiến trình Đại Cảnh Bùng Nổ (Bước B)
    if (this.isBurstAnimating) {
      this.climaxBurstProgress = Math.min(1, this.climaxBurstProgress + dt * 0.35);
      if (this.climaxBurstProgress >= 1) {
        this.isBurstAnimating = false;
        window.setTimeout(() => {
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

window.addEventListener('DOMContentLoaded', () => {
  new LuKhachKyUcGame();
});
