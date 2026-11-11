/**
 * @file src/ui/DialoguePanel.ts
 * @description Quản lý hộp thoại Pixel RPG phong cách Eastward neo dưới khung nhìn (#dialogue-modal / #dialogue-parchment-card),
 * khung chân dung Pixel Art 80×80 (#dialogue-portrait-canvas), phân trang lời thoại ngắn 1–2 câu,
 * ngăn tư liệu lịch sử mở rộng (#dialogue-lore-drawer) và khay lựa chọn gọn góc phải (#dialogue-choices).
 */

import {
  type DialoguePortraitArchetype,
  drawDialoguePortrait,
  resolveDialoguePortraitArchetype,
} from '../engine/SpriteAtlas';
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
  historical_fact: '◆ Tư liệu lịch sử',
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

function formatQuotedSpeech(raw: string): string {
  const trimmed = raw.trim().replace(/^[“"']+|[”"']+$/g, '').trim();
  return `“${trimmed}”`;
}

/**
 * Chia lời thoại thành các trang ngắn 1–2 câu (tối đa ~140 ký tự mỗi trang nếu có nhiều câu)
 * để hộp thoại luôn gọn gàng, nhịp điệu giàu cảm xúc theo phong cách Eastward.
 */
export function splitDialogueIntoPages(raw: string): string[] {
  const cleaned = raw.trim().replace(/^[“"']+|[”"']+$/g, '').trim();
  if (!cleaned) {
    return ['“...”'];
  }

  const sentences = cleaned
    .split(/(?<=[.!?…])\s+/u)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);

  if (sentences.length <= 1) {
    return [formatQuotedSpeech(cleaned)];
  }

  const pages: string[] = [];
  let currentBatch: string[] = [];
  let currentLen = 0;

  for (const sentence of sentences) {
    const projectedLen = currentLen ? currentLen + 1 + sentence.length : sentence.length;
    if (
      currentBatch.length > 0 &&
      (currentBatch.length >= 2 || projectedLen > 140)
    ) {
      pages.push(formatQuotedSpeech(currentBatch.join(' ')));
      currentBatch = [sentence];
      currentLen = sentence.length;
    } else {
      currentBatch.push(sentence);
      currentLen = projectedLen;
    }
  }

  if (currentBatch.length > 0) {
    pages.push(formatQuotedSpeech(currentBatch.join(' ')));
  }

  return pages.length > 0 ? pages : [formatQuotedSpeech(cleaned)];
}

export const DIALOGUE_NAMEPLATE_MAP: Readonly<
  Record<string, { name: string; role: string }>
> = {
  // Chương I · Hương Cảng 1930
  'Giao Liên Lâm Văn Bính': { name: 'Anh Bính', role: 'Giao liên' },
  'Bác Công Nhân Bến Cảng': { name: 'Bác Công Nhân', role: 'Bến cảng' },
  'Người Gác Đèn Bến Cảng': { name: 'Người Gác Đèn', role: 'Hoa tiêu' },
  'Thủ Kho Lưu Trữ Cửu Long': { name: 'Thủ Kho', role: 'Kho Số 4' },
  'Bà Hàng Nước Phố Cổ': { name: 'Bà Hàng Nước', role: 'Phố Cổ' },
  'Giao Liên Trần Thị Liên': { name: 'Chị Liên', role: 'Giao liên' },
  'Lão Chèo Đò Bến Thuyền': { name: 'Lão Chèo Đò', role: 'Bến thuyền' },
  'Ông Từ Giữ Kho Lưu Trữ': { name: 'Ông Từ', role: 'Kho lưu trữ' },
  'Giao Liên Cơ Sở Hương Cảng': { name: 'Giao Liên Cơ Sở', role: 'Hương Cảng' },
  'Ký Lục Viên Hội Nghị Hợp Nhất': { name: 'Ký Lục Viên', role: 'Hội nghị 1930' },
  'Chiến Sĩ Tự Vệ Đỏ Nghệ Tĩnh': { name: 'Tự Vệ Đỏ', role: 'Nghệ Tĩnh 1930' },

  // Chương II · Tân Trào – Ba Đình 1945
  'Giao Liên Việt Minh Hoàng Quốc Bảo': { name: 'Anh Bảo', role: 'Việt Minh' },
  'Đội Viên Tự Vệ Thành Hoàng Diệu': { name: 'Tự Vệ Hoàng Diệu', role: 'Ba Đình 1945' },
  'Cán Bộ Mặt Trận Việt Minh': { name: 'Cán Bộ Việt Minh', role: 'Pác Bó' },
  'Chiến Sĩ Giải Phóng Quân': { name: 'Giải Phóng Quân', role: 'Tân Trào 1945' },
  'Đội Viên Tự Vệ Thủ Đô': { name: 'Tự Vệ Thủ Đô', role: 'Ba Đình 1945' },

  // Chương III · Điện Biên – Mùa Xuân 1975
  'Đội Trưởng Dân Công Hậu Cần': { name: 'Đội Trưởng Dân Công', role: 'Đèo Pha Đin' },
  'Chiến Sĩ Thông Tin Chiến Dịch Hồ Chí Minh': {
    name: 'Chiến Sĩ Thông Tin',
    role: 'Mùa Xuân 1975',
  },
  'Dân Công Hỏa Tuyến Điện Biên': { name: 'Dân Công Hỏa Tuyến', role: 'Điện Biên 1954' },
  'Chiến Sĩ Đường Hồ Chí Minh': { name: 'Chiến Sĩ Trường Sơn', role: 'Đường 559' },
  'Chiến Sĩ Xe Tăng Chiến Dịch Hồ Chí Minh': {
    name: 'Chiến Sĩ Xe Tăng',
    role: 'Mùa Xuân 1975',
  },

  // Chương IV · Đổi Mới – Kỷ Nguyên Số 2045
  'Chuyên Gia Nghiên Cứu Đổi Mới': { name: 'Chuyên Gia Đổi Mới', role: 'Chính sách 1986' },
  'Giám Tuyển Lưu Trữ Quốc Gia 2045': { name: 'Giám Tuyển Ký Ức', role: 'Lưu trữ 2045' },
  'Kiến Trúc Sư Kinh Tế Đổi Mới': { name: 'Chuyên Gia Kinh Tế', role: 'Đổi Mới 1986' },
  'Chuyên Gia Hội Nhập & Công Nghiệp Hóa': {
    name: 'Chuyên Gia Hội Nhập',
    role: 'Công nghiệp hóa',
  },
  'Lữ Khách Lưu Trữ Quốc Gia 2045': { name: 'Lữ Khách Ký Ức', role: 'Kỷ nguyên 2045' },
};

/**
 * Keep complete character identities in story data, while the dialogue
 * nameplate uses short, readable labels like an RPG.
 */
export function getDialogueNameplate(
  speaker: string,
  role: string
): { name: string; role: string } {
  const readableName = speaker.trim();
  const readableRole = role.trim();
  const mapped = DIALOGUE_NAMEPLATE_MAP[readableName];
  if (mapped) {
    return mapped;
  }
  return {
    name: readableName,
    role:
      readableRole.toLocaleLowerCase('vi') === readableName.toLocaleLowerCase('vi')
        ? ''
        : readableRole,
  };
}

function escapeDialogueHtml(raw: string): string {
  return raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * Tô sáng tối đa 1 cụm từ khóa/mốc thời gian quan trọng trong mỗi trang lời thoại
 * để tránh biến hộp thoại thành văn bản gạch chân xanh đỏ (Hạng mục B.3).
 */
export function formatDialoguePageWithSingleHighlight(pageText: string): string {
  const escaped = escapeDialogueHtml(pageText);
  const pattern =
    /(&#39;[^&#]{2,42}&#39;|\b\d{1,2}(?:[–-]\d{1,2})?\/\d{1,2}\/\d{4}\b|\b\d{1,2}\/\d{4}\b|\b(?:1930|1941|1945|1954|1968|1975|1986|2021|2045)\b)/u;
  return escaped.replace(
    pattern,
    '<span class="dialogue-highlight">$1</span>'
  );
}

export class DialoguePanel {
  private callbacks: DialoguePanelCallbacks;
  private pages: string[] = [];
  private currentPageIndex = 0;
  private currentArchetype: DialoguePortraitArchetype = 'liaison';
  private portraitTimerId: ReturnType<typeof setInterval> | null = null;
  private speakingTicksRemaining = 0;

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

    const nextPageBtn = document.getElementById('btn-dialogue-next-page');
    nextPageBtn?.addEventListener('click', (ev) => {
      ev.stopPropagation();
      this.nextPage();
    });

    const bodyTextEl = document.getElementById('dialogue-body-text');
    bodyTextEl?.addEventListener('click', () => {
      if (this.pages.length > 1 && this.currentPageIndex < this.pages.length - 1) {
        this.nextPage();
      }
    });

    const loreToggleBtn = document.getElementById('btn-toggle-dialogue-lore');
    loreToggleBtn?.addEventListener('click', () => {
      this.toggleLoreDrawer();
    });

    if (typeof window !== 'undefined') {
      window.addEventListener('keydown', (e: KeyboardEvent) => {
        if (!this.isOpen()) return;
        const target = e.target as HTMLElement | null;
        if (
          target &&
          (target.tagName === 'INPUT' ||
            target.tagName === 'TEXTAREA' ||
            target.isContentEditable)
        ) {
          return;
        }

        if (e.code === 'Tab') {
          if (e.defaultPrevented) return;
          e.preventDefault();
          this.toggleLoreDrawer();
          return;
        }

        if (e.code === 'Space') {
          if (this.pages.length > 1 && this.currentPageIndex < this.pages.length - 1) {
            e.preventDefault();
            this.nextPage();
          }
        }
      });
    }
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
    this.toggleLoreDrawer(false);
    this.startPortraitLoop();
  }

  public isOpen(): boolean {
    if (typeof document === 'undefined') return false;
    const modal = document.getElementById('dialogue-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public isLoreDrawerExpanded(): boolean {
    if (typeof document === 'undefined') return false;
    const drawer = document.getElementById('dialogue-lore-drawer');
    return Boolean(drawer && !drawer.classList.contains('is-collapsed'));
  }

  public toggleLoreDrawer(force?: boolean): boolean {
    if (typeof document === 'undefined') return false;
    const drawer = document.getElementById('dialogue-lore-drawer');
    const toggleBtn = document.getElementById('btn-toggle-dialogue-lore');
    if (!drawer) return false;

    const currentlyCollapsed = drawer.classList.contains('is-collapsed');
    const shouldExpand = force !== undefined ? force : currentlyCollapsed;

    if (shouldExpand) {
      drawer.classList.remove('is-collapsed');
      drawer.classList.add('is-expanded');
      toggleBtn?.classList.add('is-active');
      toggleBtn?.setAttribute('aria-expanded', 'true');
    } else {
      drawer.classList.add('is-collapsed');
      drawer.classList.remove('is-expanded');
      toggleBtn?.classList.remove('is-active');
      toggleBtn?.setAttribute('aria-expanded', 'false');
    }

    return shouldExpand;
  }

  private startPortraitLoop(): void {
    this.stopPortraitLoop();
    this.renderPortraitFrame();
    if (typeof window === 'undefined') return;
    this.portraitTimerId = setInterval(() => {
      if (!this.isOpen()) {
        this.stopPortraitLoop();
        return;
      }
      if (this.speakingTicksRemaining > 0) {
        this.speakingTicksRemaining -= 1;
      }
      this.renderPortraitFrame();
    }, 120);
  }

  private stopPortraitLoop(): void {
    if (this.portraitTimerId !== null) {
      clearInterval(this.portraitTimerId);
      this.portraitTimerId = null;
    }
  }

  private renderPortraitFrame(): void {
    if (typeof document === 'undefined') return;
    if (
      typeof navigator !== 'undefined' &&
      /jsdom/i.test(navigator.userAgent || '')
    ) {
      return;
    }
    const canvas = document.getElementById(
      'dialogue-portrait-canvas'
    ) as HTMLCanvasElement | null;
    if (!canvas || typeof canvas.getContext !== 'function') return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const reducedMotion = document.body.classList.contains('is-reduced-motion');
    const nowSec =
      typeof performance !== 'undefined' ? performance.now() * 0.001 : 0;
    drawDialoguePortrait(
      ctx,
      this.currentArchetype,
      nowSec,
      this.speakingTicksRemaining > 0,
      reducedMotion
    );
  }

  public getCurrentArchetype(): DialoguePortraitArchetype {
    return this.currentArchetype;
  }

  public getPageCount(): number {
    return this.pages.length;
  }

  public getCurrentPageIndex(): number {
    return this.currentPageIndex;
  }

  public nextPage(): boolean {
    if (this.pages.length <= 1) return false;
    if (this.currentPageIndex < this.pages.length - 1) {
      this.currentPageIndex += 1;
    } else {
      this.currentPageIndex = 0;
    }
    this.speakingTicksRemaining = 8;
    this.renderCurrentPageText();
    this.renderPortraitFrame();
    return true;
  }

  private setSpeechPages(rawText: string): void {
    this.pages = splitDialogueIntoPages(rawText);
    this.currentPageIndex = 0;
    this.speakingTicksRemaining = 10;
    this.renderCurrentPageText();
  }

  private renderCurrentPageText(): void {
    if (typeof document === 'undefined') return;
    const bodyEl = document.getElementById('dialogue-body-text');
    const textEl = document.getElementById('dialogue-text');
    const indicatorEl = document.getElementById('dialogue-page-indicator');
    const nextBtn = document.getElementById('btn-dialogue-next-page');

    const activeLine = this.pages[this.currentPageIndex] ?? '“...”';
    const highlightedHtml = formatDialoguePageWithSingleHighlight(activeLine);
    if (textEl) {
      textEl.innerHTML = highlightedHtml;
    } else if (bodyEl) {
      bodyEl.innerHTML = highlightedHtml;
    }

    const total = this.pages.length;
    if (indicatorEl) {
      if (total > 1) {
        indicatorEl.textContent = `Trang ${this.currentPageIndex + 1}/${total}`;
        indicatorEl.hidden = false;
        indicatorEl.removeAttribute('hidden');
      } else {
        indicatorEl.textContent = 'Trang 1/1';
        indicatorEl.hidden = true;
        indicatorEl.setAttribute('hidden', '');
      }
    }

    if (nextBtn) {
      const hasMorePages = total > 1 && this.currentPageIndex < total - 1;
      if (hasMorePages) {
        nextBtn.hidden = false;
        nextBtn.removeAttribute('hidden');
      } else {
        nextBtn.hidden = true;
        nextBtn.setAttribute('hidden', '');
      }
    }
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
      setOptionalBlockText(citeEl, '');
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
    const sigEl = document.getElementById('dialogue-significance');
    const citeEl = document.getElementById('dialogue-citation');

    const displayIdentity = getDialogueNameplate(node.speaker, node.role);
    if (speakerEl) {
      speakerEl.textContent = displayIdentity.name;
      speakerEl.title = node.speaker;
    }
    if (roleEl) {
      roleEl.textContent = displayIdentity.role;
      roleEl.hidden = !displayIdentity.role;
    }

    this.currentArchetype = resolveDialoguePortraitArchetype(
      node.speaker,
      node.role,
      false
    );
    this.setSpeechPages(node.text);
    this.renderPortraitFrame();

    // Only retain narrative context in the optional drawer; never show source badges or citations.
    setOptionalBlockText(
      sigEl,
      node.provenance.historicalNote
        ? `Ghi chú: ${node.provenance.historicalNote}`
        : ''
    );
    setOptionalBlockText(citeEl, '');

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
      const sigEl = document.getElementById('dialogue-significance');
      const citeEl = document.getElementById('dialogue-citation');

      const speaker = node?.speaker ?? milestone.npcSpeaker;
      const role = node?.role ?? milestone.npcRole;
      const displayIdentity = getDialogueNameplate(speaker, role);
      if (speakerEl) {
        speakerEl.textContent = displayIdentity.name;
        speakerEl.title = speaker;
      }
      if (roleEl) {
        roleEl.textContent = displayIdentity.role;
        roleEl.hidden = !displayIdentity.role;
      }

      this.currentArchetype = resolveDialoguePortraitArchetype(
        speaker,
        role,
        !node
      );
      setOptionalBlockText(
        titleEl,
        `MỐC LA MÃ ${milestone.roman} (${milestone.year}) · ${milestone.title}`
      );

      this.setSpeechPages(node ? node.text : milestone.dialogue);
      this.renderPortraitFrame();

      setOptionalBlockText(
        sigEl,
        node?.provenance.historicalNote
          ? `Ghi chú sử học: ${node.provenance.historicalNote}`
          : `Ý nghĩa lịch sử: ${milestone.historicalSignificance}`
      );
      setOptionalBlockText(citeEl, '');
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

    // Provenance remains in structured game data, but isn't visual clutter.
    badgeEl.dataset.provenanceType = provenance.type;
    badgeEl.textContent = '';
    badgeEl.hidden = true;
    badgeEl.setAttribute('hidden', '');
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
    this.stopPortraitLoop();
    if (typeof document === 'undefined') return;
    const modal = document.getElementById('dialogue-modal');
    const card = document.getElementById('dialogue-parchment-card');
    const choicesContainer = document.getElementById('dialogue-choices');

    this.toggleLoreDrawer(false);

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

