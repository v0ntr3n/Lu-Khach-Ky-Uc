/**
 * @file src/ui/TutorialController.ts
 * @description Bộ điều khiển Hướng Dẫn Tân Thủ theo ngữ cảnh (#tutorial-coach-bar) cho M7 (#12).
 * Hiển thị thanh chỉ dẫn nổi không chặn điều khiển (non-blocking coach-bar), hướng dẫn từng thao tác
 * một lần duy nhất: Di chuyển (WASD/Arrows) -> Tương tác [E] -> Nhật ký [J] -> Bản đồ [M] -> Gợi ý [H]/Tạm dừng [Tab].
 * Hỗ trợ bỏ qua tức thì ("Bỏ qua [X]") và xem lại bất kỳ lúc nào từ Cài đặt Hệ thống (#btn-replay-tutorial).
 */

import type { TutorialStepId } from '../types/game';

export interface TutorialStepDefinition {
  id: Exclude<TutorialStepId, 'completed'>;
  index: number;
  total: number;
  badgeText: string;
  title: string;
  instruction: string;
  keycap: string;
}

export const TUTORIAL_STEPS: readonly TutorialStepDefinition[] = [
  {
    id: 'move',
    index: 1,
    total: 5,
    badgeText: 'HƯỚNG DẪN 1/5',
    title: 'Bước 1 · Di chuyển Lữ Khách',
    instruction: 'Nhấn [WASD] hoặc phím Mũi tên để bước đi trên bến cảng.',
    keycap: 'WASD / ↑↓←→',
  },
  {
    id: 'interact',
    index: 2,
    total: 5,
    badgeText: 'HƯỚNG DẪN 2/5',
    title: 'Bước 2 · Trò chuyện & Tương tác',
    instruction:
      'Tiến lại gần Giao Liên Lâm Văn Bính và nhấn [E] để nhận mật hiệu.',
    keycap: 'E',
  },
  {
    id: 'journal',
    index: 3,
    total: 5,
    badgeText: 'HƯỚNG DẪN 3/5',
    title: 'Bước 3 · Nhật ký & Biên niên sử',
    instruction:
      'Nhấn [J] để mở Sổ tay Nhiệm vụ và tra cứu Biên niên sử I–XII.',
    keycap: 'J',
  },
  {
    id: 'map',
    index: 4,
    total: 5,
    badgeText: 'HƯỚNG DẪN 4/5',
    title: 'Bước 4 · Sơ đồ Khu vực & Điểm lưu',
    instruction:
      'Nhấn [M] để xem Sơ đồ liên thông và các Đèn Báo An Toàn đã mở.',
    keycap: 'M',
  },
  {
    id: 'hint',
    index: 5,
    total: 5,
    badgeText: 'HƯỚNG DẪN 5/5',
    title: 'Bước 5 · Gợi ý & Tạm dừng',
    instruction:
      'Nhấn [H] để bật/tắt gợi ý nhiệm vụ hoặc [Tab]/[Esc] để mở Menu Tạm Dừng.',
    keycap: 'H / Tab',
  },
];

export interface TutorialControllerCallbacks {
  onComplete?: () => void;
  onTutorialCompletedChange?: (completed: boolean) => void;
  onStepChange?: (step: TutorialStepId) => void;
  onPlayUiSound?: (kind: 'confirm' | 'cancel') => void;
}

export class TutorialController {
  private callbacks: TutorialControllerCallbacks;
  private currentStep: TutorialStepId = 'completed';
  private isGameplayVisible = false;
  private isOverlaySuppressed = false;
  private cumulativeMoveDistancePx = 0;

  constructor(callbacks: TutorialControllerCallbacks = {}) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.syncDOM();
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;

    const skipBtn = document.getElementById('btn-tutorial-skip');
    skipBtn?.addEventListener('click', () => {
      this.callbacks.onPlayUiSound?.('cancel');
      this.skip();
    });

    const nextBtn = document.getElementById('btn-tutorial-next');
    nextBtn?.addEventListener('click', () => {
      this.callbacks.onPlayUiSound?.('confirm');
      this.advanceStep();
    });
  }

  public getCurrentStep(): TutorialStepId {
    return this.currentStep;
  }

  public isActive(): boolean {
    return this.currentStep !== 'completed';
  }

  public isCompleted(): boolean {
    return this.currentStep === 'completed';
  }

  public getStepDefinition(): TutorialStepDefinition | null {
    if (this.currentStep === 'completed') return null;
    return TUTORIAL_STEPS.find((s) => s.id === this.currentStep) ?? null;
  }

  public startIfFirstPlay(alreadyCompleted = false): void {
    this.cumulativeMoveDistancePx = 0;
    if (alreadyCompleted) {
      this.currentStep = 'completed';
      this.syncDOM();
      return;
    }
    this.currentStep = 'move';
    this.callbacks.onStepChange?.(this.currentStep);
    this.syncDOM();
  }

  public replay(): void {
    this.cumulativeMoveDistancePx = 0;
    this.currentStep = 'move';
    this.isOverlaySuppressed = false;
    this.callbacks.onTutorialCompletedChange?.(false);
    this.callbacks.onStepChange?.(this.currentStep);
    this.syncDOM();
  }

  public skip(): void {
    if (this.currentStep === 'completed') return;
    this.currentStep = 'completed';
    this.callbacks.onStepChange?.('completed');
    this.callbacks.onTutorialCompletedChange?.(true);
    this.callbacks.onComplete?.();
    this.syncDOM();
  }

  public advanceStep(): void {
    if (this.currentStep === 'completed') return;
    const idx = TUTORIAL_STEPS.findIndex((s) => s.id === this.currentStep);
    if (idx === -1 || idx >= TUTORIAL_STEPS.length - 1) {
      this.skip();
      return;
    }
    this.currentStep = TUTORIAL_STEPS[idx + 1].id;
    this.callbacks.onStepChange?.(this.currentStep);
    this.syncDOM();
  }

  public recordMovementDelta(dx: number, dy: number): void {
    if (this.currentStep !== 'move') return;
    const dist = Math.hypot(dx, dy);
    if (!Number.isFinite(dist) || dist <= 0) return;
    this.cumulativeMoveDistancePx += dist;
    if (this.cumulativeMoveDistancePx >= 24) {
      this.advanceStep();
    }
  }

  public recordAction(
    action: Exclude<TutorialStepId, 'completed'>
  ): void {
    if (this.currentStep === 'completed') return;

    if (this.currentStep === action) {
      this.advanceStep();
      return;
    }

    // Nếu người chơi tương tác [E] ngay ở Bước 1 ('move'), tự động chuyển sang Bước 3 ('journal')
    if (this.currentStep === 'move' && action === 'interact') {
      this.currentStep = 'journal';
      this.callbacks.onStepChange?.(this.currentStep);
      this.syncDOM();
    }
  }

  public setGameplayVisible(visible: boolean): void {
    this.isGameplayVisible = visible;
    this.syncDOM();
  }

  public setOverlaySuppressed(suppressed: boolean): void {
    this.isOverlaySuppressed = suppressed;
    this.syncDOM();
  }

  public syncDOM(): void {
    if (typeof document === 'undefined') return;
    const barEl = document.getElementById('tutorial-coach-bar');
    if (!barEl) return;

    const stepDef = this.getStepDefinition();
    const shouldShow =
      Boolean(stepDef) && this.isGameplayVisible && !this.isOverlaySuppressed;

    barEl.hidden = !shouldShow;
    barEl.classList.toggle('is-open', shouldShow);
    barEl.classList.toggle('is-visible', shouldShow);
    if (!shouldShow || !stepDef) {
      barEl.setAttribute('hidden', '');
      return;
    }

    barEl.removeAttribute('hidden');
    barEl.dataset.tutorialStep = stepDef.id;

    const badgeEl = document.getElementById('tutorial-step-badge');
    const titleEl = document.getElementById('tutorial-step-title');
    const textEl = document.getElementById('tutorial-step-instruction');
    const keycapEl = document.getElementById('tutorial-step-keycap');
    const nextBtn = document.getElementById('btn-tutorial-next');

    if (badgeEl) badgeEl.textContent = stepDef.badgeText;
    if (titleEl) titleEl.textContent = stepDef.title;
    if (textEl) textEl.textContent = stepDef.instruction;
    if (keycapEl) keycapEl.textContent = stepDef.keycap;
    if (nextBtn) {
      nextBtn.textContent =
        stepDef.index >= stepDef.total ? 'Hoàn tất ✓' : 'Bước tiếp ▸';
    }
  }
}
