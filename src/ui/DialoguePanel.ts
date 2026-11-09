/**
 * @file src/ui/DialoguePanel.ts
 * @description Quản lý hộp thoại tương tác NPC đa lựa chọn (#dialogue-modal / #dialogue-parchment-card),
 * nhãn phân định nguồn sử liệu (#dialogue-provenance-badge) và các nút lựa chọn nhánh hội thoại (#dialogue-choices).
 */

import type {
  DialogueChoice,
  DialogueNode,
  MilestoneArtifact,
  ProvenanceMetadata,
} from '../types/game';

export interface DialoguePanelCallbacks {
  onSelectChoice?: (choiceId: string) => void;
  onClose?: () => void;
}

const DEFAULT_PROVENANCE_LABELS = {
  historical_fact: '📜 SỬ LIỆU XÁC THỰC (GIÁO TRÌNH 2021)',
  fictional_npc: '🎭 NHÂN VẬT LƯU TRỮ HƯ CẤU',
  speculative_2045: '✨ VIỄN TƯỞNG KỶ NGUYÊN SỐ 2045',
} as const;

export class DialoguePanel {
  private callbacks: DialoguePanelCallbacks;

  constructor(callbacks: DialoguePanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-dialogue');
    closeBtn?.addEventListener('click', () => {
      this.close();
      this.callbacks.onClose?.();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('dialogue-modal');
    return Boolean(modal && modal.classList.contains('is-open'));
  }

  public openNpcDialogue(
    node: DialogueNode,
    choices: DialogueChoice[],
    milestone?: MilestoneArtifact
  ): void {
    const modal = document.getElementById('dialogue-modal');
    const card = document.getElementById('dialogue-parchment-card');
    if (modal) {
      modal.classList.add('is-open');
      modal.hidden = false;
      modal.removeAttribute('hidden');
    }
    card?.classList.add('is-Active');

    if (milestone) {
      const titleEl = document.getElementById('dialogue-milestone-title');
      const sigEl = document.getElementById('dialogue-significance');
      const citeEl = document.getElementById('dialogue-citation');
      if (titleEl) {
        titleEl.textContent = `MỐC LA MÃ ${milestone.roman} (${milestone.year}): ${milestone.title}`;
      }
      if (sigEl) {
        sigEl.textContent = `Ý nghĩa lịch sử: ${milestone.historicalSignificance}`;
      }
      if (citeEl) {
        citeEl.textContent = `📖 ${milestone.textbookCitation}`;
      }
    }

    this.updateDialogueNode(node, choices);
  }

  public updateDialogueNode(node: DialogueNode, choices: DialogueChoice[]): void {
    const speakerEl = document.getElementById('dialogue-speaker');
    const roleEl = document.getElementById('dialogue-role');
    const bodyEl = document.getElementById('dialogue-body-text');
    const textEl = document.getElementById('dialogue-text');
    const sigEl = document.getElementById('dialogue-significance');
    const citeEl = document.getElementById('dialogue-citation');

    if (speakerEl) speakerEl.textContent = node.speaker;
    if (roleEl) roleEl.textContent = node.role;
    if (textEl) {
      textEl.textContent = `"${node.text}"`;
    } else if (bodyEl) {
      bodyEl.textContent = `"${node.text}"`;
    }

    if (node.provenance.historicalNote && sigEl) {
      sigEl.textContent = `Ghi chú sử học: ${node.provenance.historicalNote}`;
    }
    if (node.provenance.citation && citeEl) {
      citeEl.textContent = `📖 ${node.provenance.citation}`;
    }

    this.renderProvenanceBadge(node.provenance);
    this.renderChoices(choices);
  }

  public showArtifactDialogue(
    milestone: MilestoneArtifact,
    node?: DialogueNode | null,
    choices?: DialogueChoice[]
  ): void {
    const card = document.getElementById('dialogue-parchment-card');
    const speakerEl = document.getElementById('dialogue-speaker');
    const roleEl = document.getElementById('dialogue-role');
    const titleEl = document.getElementById('dialogue-milestone-title');
    const bodyEl = document.getElementById('dialogue-body-text');
    const textEl = document.getElementById('dialogue-text');
    const sigEl = document.getElementById('dialogue-significance');
    const citeEl = document.getElementById('dialogue-citation');

    card?.classList.add('is-Active');

    if (speakerEl) speakerEl.textContent = node?.speaker ?? milestone.npcSpeaker;
    if (roleEl) roleEl.textContent = node?.role ?? milestone.npcRole;
    if (titleEl) {
      titleEl.textContent = `MỐC LA MÃ ${milestone.roman} (${milestone.year}): ${milestone.title}`;
    }
    const line = node ? `"${node.text}"` : `"${milestone.dialogue}"`;
    if (textEl) {
      textEl.textContent = line;
    } else if (bodyEl) {
      bodyEl.textContent = line;
    }
    if (sigEl) sigEl.textContent = `Ý nghĩa lịch sử: ${milestone.historicalSignificance}`;
    if (citeEl) citeEl.textContent = `📖 ${milestone.textbookCitation}`;

    const prov: ProvenanceMetadata = node?.provenance ??
      milestone.provenance ?? {
        type: 'historical_fact',
        label: DEFAULT_PROVENANCE_LABELS.historical_fact,
        citation: milestone.textbookCitation,
      };
    this.renderProvenanceBadge(prov);
    this.renderChoices(choices ?? []);
  }

  private renderProvenanceBadge(provenance: ProvenanceMetadata): void {
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

    for (const ch of choices) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'dialogue-choice-btn ctrl-btn';
      btn.dataset.choiceId = ch.id;
      btn.textContent = `▸ ${ch.text}`;
      btn.addEventListener('click', () => {
        this.callbacks.onSelectChoice?.(ch.id);
      });
      container.appendChild(btn);
    }
  }

  public close(): void {
    const modal = document.getElementById('dialogue-modal');
    const choicesContainer = document.getElementById('dialogue-choices');
    modal?.classList.remove('is-open');
    if (choicesContainer) {
      choicesContainer.innerHTML = '';
      choicesContainer.hidden = true;
      choicesContainer.setAttribute('hidden', '');
    }
  }
}
