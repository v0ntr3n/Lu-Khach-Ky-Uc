/**
 * @file src/ui/InventoryPanel.ts
 * @description Giao diện Túi Tư Liệu & Bằng Chứng Lịch Sử (#inventory-modal, Phím I).
 * Chỉ mở khóa chi tiết cho các tư liệu đã thu thập (hoặc khi bật Chế Độ Giảng Viên).
 */

import type { EvidenceItem } from '../types/game';

export interface InventoryPanelCallbacks {
  onClose?: () => void;
}

const CATEGORY_LABELS: Record<EvidenceItem['category'], string> = {
  document: '📜 VĂN KIỆN',
  cipher: '🔐 MẬT ĐÀM',
  artifact: '🏺 DI VẬT',
  dispatch: '✉️ CHỈ THỊ',
};

const PROVENANCE_LABELS = {
  historical_fact: 'TƯ LIỆU LỊCH SỬ XÁC THỰC',
  fictional_npc: 'NHÂN VẬT / TÌNH TIẾT HƯ CẤU',
  speculative_2045: 'DỮ LIỆU VIỄN TƯỞNG 2045',
} as const;

export class InventoryPanel {
  private callbacks: InventoryPanelCallbacks;
  private allEvidence: EvidenceItem[] = [];
  private collectedIds: Set<string> = new Set();
  private selectedId: string | null = null;
  private isPresenterMode = false;

  constructor(callbacks: InventoryPanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-inventory');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('inventory-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean): boolean {
    const modal = document.getElementById('inventory-modal');
    const openBtn = document.getElementById('btn-open-inventory');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.render();
    } else {
      modal.setAttribute('hidden', '');
    }

    openBtn?.classList.toggle('is-active', next);
    openBtn?.setAttribute('aria-expanded', String(next));
    return next;
  }

  public updateState(
    allEvidence: EvidenceItem[],
    collectedIds: ReadonlySet<string>,
    isPresenterMode = false
  ): void {
    this.allEvidence = allEvidence;
    this.collectedIds = new Set(collectedIds);
    this.isPresenterMode = isPresenterMode;

    if (
      this.selectedId &&
      !this.isEvidenceUnlocked(this.selectedId)
    ) {
      this.selectedId = null;
    }
    if (!this.selectedId) {
      const firstUnlocked = this.allEvidence.find((item) => this.isEvidenceUnlocked(item.id));
      this.selectedId = firstUnlocked?.id ?? null;
    }

    this.render();
  }

  private isEvidenceUnlocked(id: string): boolean {
    return this.isPresenterMode || this.collectedIds.has(id);
  }

  public render(): void {
    const summaryEl = document.getElementById('inventory-summary');
    const listEl = document.getElementById('inventory-item-list');
    const detailEl = document.getElementById('inventory-item-detail');

    const total = this.allEvidence.length;
    const unlockedCount = this.allEvidence.filter((e) => this.isEvidenceUnlocked(e.id)).length;

    if (summaryEl) {
      summaryEl.textContent = `Đã thu thập ${unlockedCount} / ${total} tư liệu${
        this.isPresenterMode ? ' · [CHẾ ĐỘ GIẢNG VIÊN: MỞ TOÀN BỘ]' : ''
      }`;
    }

    if (listEl) {
      listEl.innerHTML = '';
      for (const item of this.allEvidence) {
        const unlocked = this.isEvidenceUnlocked(item.id);
        const isSelected = item.id === this.selectedId;

        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = [
          'inventory-item-card',
          unlocked ? 'is-collected' : 'is-locked',
          isSelected ? 'is-selected' : '',
        ]
          .filter(Boolean)
          .join(' ');
        btn.dataset.evidenceId = item.id;

        if (unlocked) {
          btn.innerHTML = `
            <span class="inv-card-cat">${CATEGORY_LABELS[item.category]}</span>
            <strong class="inv-card-title">${item.title}</strong>
            <span class="inv-card-summary">${item.shortSummary}</span>
          `;
          btn.addEventListener('click', () => {
            this.selectedId = item.id;
            this.render();
          });
        } else {
          btn.innerHTML = `
            <span class="inv-card-cat">🔒 CHƯA THU THẬP</span>
            <strong class="inv-card-title">Tư liệu chưa thu thập</strong>
            <span class="inv-card-summary">Khám phá hiện trường hoặc đối thoại NPC để thu thập</span>
          `;
        }

        listEl.appendChild(btn);
      }
    }

    if (detailEl) {
      const selectedItem = this.allEvidence.find(
        (e) => e.id === this.selectedId && this.isEvidenceUnlocked(e.id)
      );
      if (!selectedItem) {
        detailEl.innerHTML = `
          <div class="inv-detail-empty">
            <h4>Chưa chọn tư liệu</h4>
            <p>Hãy thu thập các văn kiện, mật đàm và di vật lịch sử trên bản đồ rồi bấm vào thẻ tư liệu bên trái để đối chiếu toàn văn Giáo trình 2021.</p>
          </div>
        `;
      } else {
        const provLabel =
          selectedItem.provenance.label || PROVENANCE_LABELS[selectedItem.provenance.type];
        detailEl.innerHTML = `
          <div class="inv-detail-header">
            <span class="inv-card-cat">${CATEGORY_LABELS[selectedItem.category]}</span>
            <span class="provenance-badge is-${selectedItem.provenance.type}">${provLabel}</span>
          </div>
          <h4 class="inv-detail-title">${selectedItem.title}</h4>
          <blockquote class="inv-detail-quote">"${selectedItem.historicalQuote}"</blockquote>
          <p class="inv-detail-sig"><strong>Ý nghĩa lịch sử:</strong> ${selectedItem.historicalSignificance}</p>
          <div class="inv-detail-cite">📖 ${selectedItem.textbookCitation}</div>
        `;
      }
    }
  }
}
