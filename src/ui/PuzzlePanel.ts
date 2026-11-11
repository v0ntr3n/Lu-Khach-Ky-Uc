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
    if (subtitleEl) subtitleEl.textContent = puzzle.subtitle;
    if (provEl) {
      provEl.textContent = '';
      provEl.hidden = true;
    }
    if (instructionEl) instructionEl.textContent = puzzle.instruction;

    if (hintEl) {
      const showHint = attemptCount >= 2;
      hintEl.textContent = `💡 Gợi ý: ${puzzle.progressiveHint}`;
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

    const docSheet = document.createElement('div');
    docSheet.className = 'dossier-document-sheet';

    const docHeader = document.createElement('div');
    docHeader.className = 'dossier-doc-header';
    docHeader.innerHTML = `
      <span class="dossier-doc-kicker">VĂN KIỆN LƯU TRỮ · BẢN ĐỐI CHIẾU GỐC</span>
      <span id="dossier-crimson-seal" class="crimson-seal-stamp" hidden>ẤN CHƯƠNG XÁC THỰC</span>
    `;
    docSheet.appendChild(docHeader);

    const inlineGapsBar = document.createElement('div');
    inlineGapsBar.className = 'dossier-inline-gaps-bar';
    inlineGapsBar.setAttribute('aria-label', 'Các chỗ khuyết trên văn kiện');

    this.activePuzzle.slots.forEach((slot, idx) => {
      const gapEl = document.createElement('div');
      gapEl.className = 'dossier-inline-gap-slot';
      gapEl.dataset.inlineSlotId = slot.id;

      const numSpan = document.createElement('span');
      numSpan.className = 'dossier-inline-gap-num';
      numSpan.textContent = `Mảnh [${idx + 1}] · ${slot.label}`;

      const valSpan = document.createElement('span');
      valSpan.className = 'dossier-inline-gap-value';
      valSpan.textContent = '[ Chọn mảnh tư liệu bên dưới ]';

      gapEl.appendChild(numSpan);
      gapEl.appendChild(valSpan);
      inlineGapsBar.appendChild(gapEl);
    });

    docSheet.appendChild(inlineGapsBar);

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

          const inlineSlot = inlineGapsBar.querySelector<HTMLElement>(
            `.dossier-inline-gap-slot[data-inline-slot-id="${slot.id}"]`
          );
          if (inlineSlot) {
            inlineSlot.classList.add('is-filled');
            const valEl = inlineSlot.querySelector<HTMLElement>('.dossier-inline-gap-value');
            if (valEl) {
              valEl.textContent = opt.text;
            }
          }
        });

        optionsBox.appendChild(btn);
      }

      slotCard.appendChild(optionsBox);
      docSheet.appendChild(slotCard);
    }

    container.appendChild(docSheet);
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
    const sealEl = document.getElementById('dossier-crimson-seal');

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
      hintEl.textContent = `💡 Gợi ý: ${this.activePuzzle.progressiveHint}`;
    }

    if (res.solved && modal) {
      modal.classList.add('is-solved');
      if (sealEl) {
        sealEl.hidden = false;
        sealEl.removeAttribute('hidden');
      }
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
