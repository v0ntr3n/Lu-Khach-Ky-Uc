/**
 * @file src/ui/LoadingScreen.ts
 * @description Màn hình chuyển cảnh nạp dữ liệu theo từng giai đoạn (#loading-screen)
 * mang thẩm mỹ Sơn Mài & Triện Đồng, tôn trọng chế độ Giảm chuyển động (Reduced Motion).
 */

export interface LoadingTransitionOptions {
  reducedMotion?: boolean;
  mode?: 'new_game' | 'continue';
  onStage?: (pct: number, label: string) => void;
}

interface LoadingStageStep {
  pct: number;
  label: string;
  holdMs: number;
}

export class LoadingScreen {
  private runToken = 0;

  private getElements(): {
    screen: HTMLElement | null;
    stageLabel: HTMLElement | null;
    progressBar: HTMLElement | null;
    progressFill: HTMLElement | null;
    progressPct: HTMLElement | null;
  } {
    if (typeof document === 'undefined') {
      return {
        screen: null,
        stageLabel: null,
        progressBar: null,
        progressFill: null,
        progressPct: null,
      };
    }

    return {
      screen: document.getElementById('loading-screen'),
      stageLabel: document.getElementById('loading-stage-label'),
      progressBar: document.getElementById('loading-progress-bar'),
      progressFill: document.getElementById('loading-progress-fill'),
      progressPct: document.getElementById('loading-progress-pct'),
    };
  }

  private applyProgress(pct: number, label: string): void {
    const { stageLabel, progressBar, progressFill, progressPct } = this.getElements();
    const clamped = Math.max(0, Math.min(100, Math.round(pct)));

    if (stageLabel) {
      stageLabel.textContent = label;
    }
    if (progressFill) {
      progressFill.style.width = `${clamped}%`;
    }
    if (progressBar) {
      progressBar.setAttribute('aria-valuenow', String(clamped));
    }
    if (progressPct) {
      progressPct.textContent = `${clamped}%`;
    }
  }

  private waitMs(ms: number): Promise<void> {
    if (ms <= 0 || typeof window === 'undefined') {
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      window.setTimeout(resolve, ms);
    });
  }

  public async playTransition(options?: LoadingTransitionOptions): Promise<void> {
    const token = ++this.runToken;
    const { screen } = this.getElements();

    const reduced =
      options?.reducedMotion ??
      (typeof document !== 'undefined' &&
        document.body.classList.contains('reduced-motion'));
    const mode = options?.mode ?? 'new_game';

    const finalLabel =
      mode === 'continue'
        ? 'Đang khôi phục dấu ấn hành trình Lữ Khách...'
        : 'Đang chuẩn bị Vết Nứt Thời Gian...';

    const stages: LoadingStageStep[] = reduced
      ? [
          {
            pct: 35,
            label: 'Đang nạp bản đồ Tiled & không gian Cửu Long...',
            holdMs: 40,
          },
          {
            pct: 75,
            label: 'Đang đồng bộ hệ thống sử liệu & nhiệm vụ...',
            holdMs: 40,
          },
          {
            pct: 100,
            label: finalLabel,
            holdMs: 40,
          },
        ]
      : [
          {
            pct: 32,
            label: 'Đang nạp bản đồ Tiled & không gian Cửu Long...',
            holdMs: 145,
          },
          {
            pct: 72,
            label: 'Đang đồng bộ hệ thống sử liệu & nhiệm vụ...',
            holdMs: 145,
          },
          {
            pct: 100,
            label: finalLabel,
            holdMs: 120,
          },
        ];

    if (screen) {
      screen.hidden = false;
      screen.removeAttribute('hidden');
      screen.classList.remove('is-fading-out');
      screen.classList.add('is-visible');
    }

    this.applyProgress(8, stages[0].label);

    for (const step of stages) {
      if (this.runToken !== token) return;
      this.applyProgress(step.pct, step.label);
      options?.onStage?.(step.pct, step.label);
      await this.waitMs(step.holdMs);
    }

    if (this.runToken !== token) return;

    if (screen && !reduced) {
      screen.classList.add('is-fading-out');
      await this.waitMs(65);
    }

    if (this.runToken === token) {
      this.hide();
    }
  }

  public hide(): void {
    this.runToken++;
    const { screen } = this.getElements();
    if (!screen) return;

    screen.classList.remove('is-visible', 'is-fading-out');
    screen.hidden = true;
    screen.setAttribute('hidden', '');
  }

  public isVisible(): boolean {
    const { screen } = this.getElements();
    return Boolean(screen && !screen.hidden && screen.classList.contains('is-visible'));
  }
}
