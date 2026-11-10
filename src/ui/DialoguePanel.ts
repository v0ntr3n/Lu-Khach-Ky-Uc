/**
 * @file src/ui/DialoguePanel.ts
 * @description Quản lý hộp thoại tương tác Giấy Dó neo dưới khung nhìn (#dialogue-modal / #dialogue-parchment-card),
 * nhãn phân định nguồn sử liệu (#dialogue-provenance-badge) và các nút lựa chọn nhánh hội thoại (#dialogue-choices).
 * Chỉ hiển thị khi đang đối thoại hoặc đối chiếu cột mốc lịch sử.
 */

import type {
  DialogueChoice,
  DialogueNode,
  MilestoneArtifact,
  ProvenanceMetadata,
} from '../types/game';

export interface DialoguePanelCallbacks {
  onSelectChoice?: (choiceId: string) => void;
  onOpen?: () => void;
  onClose?: () => void;
}

const DEFAULT_PROVENANCE_LABELS = {
  historical_fact: '◆ Sử liệu xác thực (Giáo trình 2021)',
  fictional_npc: '▸ Nhân vật lưu trữ hư cấu',
  speculative_2045: '✦ Viễn tưởng kỷ nguyên số 2045',
} as const;

function setOptionalBlockText(el: HTMLElement | null, value: string): void {
  if (!el) return;
  const trimmed = value.trim();
  el.textContent = trimmed;
  if (trimmed) {
    el.hidden = false;
    el.removeAttribute('hidden');
  } else {
    el.hidden = true;
    el.setAttribute('hidden', '');
  }
}

export class DialoguePanel {
  private callbacks: DialoguePanelCallbacks;

  constructor(callbacks: DialoguePanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.ensureClosedByDefault();
  }

  private ensureClosedByDefault(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('dialogue-modal');
    if (modal && !modal.classList.contains('is-open')) {
      modal.hidden = true;
      modal.setAttribute('hidden', '');
    }
  }

  private bindStaticEvents(): void {
    if (typeof document === 'undefined') return;
    const closeBtn = document.getElementById('btn-close-dialogue');
    closeBtn?.addEventListener('click', () => {
      this.close();
      this.callbacks.onClose?.();
    });
  }

  private showModal(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('dialogue-modal');
    const card = document.getElementById('dialogue-parchment-card');
    if (modal) {
      modal.classList.add('is-open');
      modal.hidden = false;
      modal.removeAttribute('hidden');
    }
    card?.classList.add('is-Active');
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('dialogue-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public openNpcDialogue(
    node: DialogueNode,
    choices: DialogueChoice[],
    milestone?: MilestoneArtifact
  ): void {
    this.showModal();

    const titleEl = document.getElementById('dialogue-milestone-title');
    const sigEl = document.getElementById('dialogue-significance');
    const citeEl = document.getElementById('dialogue-citation');

    if (milestone) {
      setOptionalBlockText(
        titleEl,
        `MỐC LA MÃ ${milestone.roman} (${milestone.year}) · ${milestone.title}`
      );
      setOptionalBlockText(sigEl, `Ý nghĩa lịch sử: ${milestone.historicalSignificance}`);
      setOptionalBlockText(citeEl, `◆ Nguồn đối chiếu: ${milestone.textbookCitation}`);
    } else {
      setOptionalBlockText(titleEl, '');
      setOptionalBlockText(sigEl, '');
      setOptionalBlockText(citeEl, '');
    }

    this.updateDialogueNode(node, choices);
    this.callbacks.onOpen?.();
  }

  public updateDialogueNode(node: DialogueNode, choices: DialogueChoice[]): void {
    if (typeof document === 'undefined') return;

    const speakerEl = document.getElementById('dialogue-speaker');
    const roleEl = document.getElementById('dialogue-role');
    const bodyEl = document.getElementById('dialogue-body-text');
    const textEl = document.getElementById('dialogue-text');
    const sigEl = document.getElementById('dialogue-significance');
    const citeEl = document.getElementById('dialogue-citation');

    if (speakerEl) speakerEl.textContent = node.speaker;
    if (roleEl) roleEl.textContent = node.role;

    const formattedSpeech = `“${node.text}”`;
    if (textEl) {
      textEl.textContent = formattedSpeech;
    } else if (bodyEl) {
      bodyEl.textContent = formattedSpeech;
    }

    if (node.provenance.historicalNote) {
      setOptionalBlockText(sigEl, `Ghi chú sử học: ${node.provenance.historicalNote}`);
    }
    if (node.provenance.citation) {
      setOptionalBlockText(citeEl, `◆ Nguồn đối chiếu: ${node.provenance.citation}`);
    }

    this.renderProvenanceBadge(node.provenance);
    this.renderChoices(choices);
  }

  public showArtifactDialogue(
    milestone: MilestoneArtifact,
    node?: DialogueNode | null,
    choices?: DialogueChoice[]
  ): void {
    this.showModal();

    if (typeof document !== 'undefined') {
      const speakerEl = document.getElementById('dialogue-speaker');
      const roleEl = document.getElementById('dialogue-role');
      const titleEl = document.getElementById('dialogue-milestone-title');
      const bodyEl = document.getElementById('dialogue-body-text');
      const textEl = document.getElementById('dialogue-text');
      const sigEl = document.getElementById('dialogue-significance');
      const citeEl = document.getElementById('dialogue-citation');

      if (speakerEl) speakerEl.textContent = node?.speaker ?? milestone.npcSpeaker;
      if (roleEl) roleEl.textContent = node?.role ?? milestone.npcRole;
      setOptionalBlockText(
        titleEl,
        `MỐC LA MÃ ${milestone.roman} (${milestone.year}) · ${milestone.title}`
      );

      const line = node ? `“${node.text}”` : `“${milestone.dialogue}”`;
      if (textEl) {
        textEl.textContent = line;
      } else if (bodyEl) {
        bodyEl.textContent = line;
      }

      setOptionalBlockText(
        sigEl,
        node?.provenance.historicalNote
          ? `Ghi chú sử học: ${node.provenance.historicalNote}`
          : `Ý nghĩa lịch sử: ${milestone.historicalSignificance}`
      );
      setOptionalBlockText(
        citeEl,
        `◆ Nguồn đối chiếu: ${node?.provenance.citation ?? milestone.textbookCitation}`
      );
    }

    const prov: ProvenanceMetadata = node?.provenance ??
      milestone.provenance ?? {
        type: 'historical_fact',
        label: DEFAULT_PROVENANCE_LABELS.historical_fact,
        citation: milestone.textbookCitation,
      };
    this.renderProvenanceBadge(prov);
    this.renderChoices(choices ?? []);
    this.callbacks.onOpen?.();
  }

  private renderProvenanceBadge(provenance: ProvenanceMetadata): void {
    if (typeof document === 'undefined') return;
    const badgeEl = document.getElementById('dialogue-provenance-badge');
    if (!badgeEl) return;

    badgeEl.hidden = false;
    badgeEl.removeAttribute('hidden');
    badgeEl.dataset.provenanceType = provenance.type;
    badgeEl.className = `provenance-badge is-${provenance.type}`;
    badgeEl.textContent =
      provenance.label || DEFAULT_PROVENANCE_LABELS[provenance.type];
  }

  private renderChoices(choices: DialogueChoice[]): void {
    if (typeof document === 'undefined') return;
    const container = document.getElementById('dialogue-choices');
    if (!container) return;

    container.innerHTML = '';
    if (!choices || choices.length === 0) {
      container.hidden = true;
      container.setAttribute('hidden', '');
      return;
    }

    container.hidden = false;
    container.removeAttribute('hidden');

    choices.forEach((ch, idx) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'dialogue-choice-btn ctrl-btn';
      btn.dataset.choiceId = ch.id;

      const keycap = document.createElement('kbd');
      keycap.className = 'choice-key';
      keycap.textContent = idx < 4 ? String(idx + 1) : '▸';

      const textSpan = document.createElement('span');
      textSpan.className = 'choice-label';
      textSpan.textContent = ch.text;

      btn.appendChild(keycap);
      btn.appendChild(textSpan);

      btn.addEventListener('click', () => {
        this.callbacks.onSelectChoice?.(ch.id);
      });
      container.appendChild(btn);
    });
  }

  public close(): void {
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('dialogue-modal');
    const card = document.getElementById('dialogue-parchment-card');
    const choicesContainer = document.getElementById('dialogue-choices');

    if (modal) {
      modal.classList.remove('is-open');
      modal.hidden = true;
      modal.setAttribute('hidden', '');
    }
    card?.classList.remove('is-Active');

    if (choicesContainer) {
      choicesContainer.innerHTML = '';
      choicesContainer.hidden = true;
      choicesContainer.setAttribute('hidden', '');
    }
  }
}
