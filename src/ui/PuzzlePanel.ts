/**
 * @file src/ui/PuzzlePanel.ts
 * @description Giao diện Bàn Kiểm Định & Đối Chiếu Hồ Sơ Lịch Sử (#dossier-puzzle-modal).
 * Hỗ trợ ghép mảnh Cương lĩnh chính trị đầu tiên (2/1930), hiển thị gợi ý học thuật lũy tiến
 * khi thử sai >= 2 lần, và xác thực trích dẫn Giáo trình 2021.
 */

import type { DossierPuzzleDefinition } from '../types/game';

export interface PuzzleSubmitResult {
  solved: boolean;
  attemptCount: number;
  feedback: string;
  showProgressiveHint: boolean;
}

export interface PuzzlePanelCallbacks {
  onSubmitDossierPuzzle?: (
    puzzleId: string,
    selectedOptionIds: Record<string, string>
  ) => PuzzleSubmitResult | undefined;
  onClose?: () => void;
}

export class PuzzlePanel {
  private callbacks: PuzzlePanelCallbacks;
  private activePuzzle: DossierPuzzleDefinition | null = null;
  private selectedOptions: Record<string, string> = {};

  constructor(callbacks: PuzzlePanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-dossier');
    closeBtn?.addEventListener('click', () => {
      this.close();
      this.callbacks.onClose?.();
    });

    const submitBtn = document.getElementById('btn-submit-dossier');
    submitBtn?.addEventListener('click', () => {
      this.handleSubmit();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('dossier-puzzle-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public open(puzzle: DossierPuzzleDefinition, attemptCount = 0): void {
    this.activePuzzle = puzzle;
    this.selectedOptions = {};

    const modal = document.getElementById('dossier-puzzle-modal');
    const titleEl = document.getElementById('dossier-puzzle-title');
    const subtitleEl = document.getElementById('dossier-puzzle-subtitle');
    const provEl = document.getElementById('dossier-provenance-badge');
    const instructionEl = document.getElementById('dossier-puzzle-instruction');
    const hintEl = document.getElementById('dossier-progressive-hint');
    const feedbackEl = document.getElementById('dossier-feedback-banner');

    if (!modal) return;

    modal.classList.add('is-open');
    modal.classList.remove('is-solved');
    modal.hidden = false;
    modal.removeAttribute('hidden');

    if (titleEl) titleEl.textContent = puzzle.title;
    if (subtitleEl) subtitleEl.textContent = `${puzzle.subtitle} · 📖 ${puzzle.textbookCitation}`;
    if (provEl) {
      provEl.className = `provenance-badge is-${puzzle.provenance.type}`;
      provEl.textContent = puzzle.provenance.label;
    }
    if (instructionEl) instructionEl.textContent = puzzle.instruction;

    if (hintEl) {
      const showHint = attemptCount >= 2;
      hintEl.textContent = `💡 Gợi ý học thuật: ${puzzle.progressiveHint} (${puzzle.textbookCitation})`;
      hintEl.hidden = !showHint;
      if (showHint) {
        hintEl.removeAttribute('hidden');
      } else {
        hintEl.setAttribute('hidden', '');
      }
    }

    if (feedbackEl) {
      feedbackEl.textContent = '';
      feedbackEl.className = 'dossier-feedback-banner';
      feedbackEl.hidden = true;
      feedbackEl.setAttribute('hidden', '');
    }

    this.renderSlots();
  }

  private renderSlots(): void {
    const container = document.getElementById('dossier-slots-container');
    if (!container || !this.activePuzzle) return;

    container.innerHTML = '';

    for (const slot of this.activePuzzle.slots) {
      const slotCard = document.createElement('div');
      slotCard.className = 'dossier-slot-card';
      slotCard.dataset.slotCardId = slot.id;

      const header = document.createElement('div');
      header.className = 'dossier-slot-header';
      header.innerHTML = `
        <strong class="dossier-slot-label">${slot.label}</strong>
        <p class="dossier-slot-prompt">${slot.prompt}</p>
      `;
      slotCard.appendChild(header);

      const optionsBox = document.createElement('div');
      optionsBox.className = 'dossier-options-group';

      for (const opt of slot.options) {
        const isSelected = this.selectedOptions[slot.id] === opt.id;
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = `dossier-option-btn ${isSelected ? 'is-selected' : ''}`;
        btn.dataset.slotId = slot.id;
        btn.dataset.optionId = opt.id;
        btn.setAttribute('aria-pressed', String(isSelected));
        btn.textContent = opt.text;

        btn.addEventListener('click', () => {
          this.selectedOptions[slot.id] = opt.id;
          optionsBox.querySelectorAll<HTMLButtonElement>('.dossier-option-btn').forEach((b) => {
            const active = b.dataset.optionId === opt.id;
            b.classList.toggle('is-selected', active);
            b.setAttribute('aria-pressed', String(active));
          });
        });

        optionsBox.appendChild(btn);
      }

      slotCard.appendChild(optionsBox);
      container.appendChild(slotCard);
    }
  }

  private handleSubmit(): void {
    if (!this.activePuzzle) return;

    const res = this.callbacks.onSubmitDossierPuzzle?.(
      this.activePuzzle.id,
      { ...this.selectedOptions }
    );
    if (!res) return;

    const modal = document.getElementById('dossier-puzzle-modal');
    const feedbackEl = document.getElementById('dossier-feedback-banner');
    const hintEl = document.getElementById('dossier-progressive-hint');

    if (feedbackEl) {
      feedbackEl.hidden = false;
      feedbackEl.removeAttribute('hidden');
      feedbackEl.textContent = res.feedback;
      feedbackEl.className = `dossier-feedback-banner ${
        res.solved ? 'is-success' : 'is-error'
      }`;
    }

    if (hintEl && res.showProgressiveHint) {
      hintEl.hidden = false;
      hintEl.removeAttribute('hidden');
      hintEl.textContent = `💡 Gợi ý học thuật: ${this.activePuzzle.progressiveHint} (${this.activePuzzle.textbookCitation})`;
    }

    if (res.solved && modal) {
      modal.classList.add('is-solved');
    }
  }

  public close(): void {
    const modal = document.getElementById('dossier-puzzle-modal');
    if (!modal) return;
    modal.classList.remove('is-open');
    modal.hidden = true;
    modal.setAttribute('hidden', '');
  }
}
