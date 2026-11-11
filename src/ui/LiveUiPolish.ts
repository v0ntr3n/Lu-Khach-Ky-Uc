/**
 * @file src/ui/LiveUiPolish.ts
 * @description Runtime UI polish & viewport anti-collision rules for Lữ Khách Ký Ức.
 *
 * Fixes 5 live-audited UI issues while strictly honoring M13/M14 token & typography gates:
 * 1. Compact minimap landmark title + full header width usage (eliminates stale 112px ellipsis clip).
 * 2. Repositions #stage-toast to top-center when #dialogue-modal is open so toasts never cover NPC portraits/choices.
 * 3. Single-row compact #tutorial-coach-bar on desktop (>= 960px) to preserve southern viewport visibility.
 * 4. Flex-centered hitboxes and stacking context on .rpg-tab-btn / #instructions-modal [data-guide-tab].
 * 5. Normalizes document.title against empty or legacy worker/shell titles.
 */

export const CANONICAL_DOCUMENT_TITLE =
  'Lữ Khách Ký Ức (1930 – 2045) — Hành Trình Lịch Sử Sơn Mài & Giấy Dó';

export const LIVE_UI_POLISH_STYLE_ID = 'rpg-live-ui-polish';

const LEGACY_TITLE_PATTERN = /niên\s*biểu\s*ký\s*ức/iu;

/**
 * Formats a full map title (e.g. "Quảng Trường Bến Cảng Hương Cảng (1930)")
 * into a concise landmark label for `#minimap-zone-title` while `#stage-location-badge`
 * already displays the full location + historical year at top-center.
 */
export function formatCompactMinimapTitle(rawTitle: string): string {
  const trimmed = rawTitle.trim();
  if (!trimmed) return '';

  // 1. Strip trailing parenthetical era/interior qualifiers such as "(1930)" or "(Nội Thất)"
  let compact = trimmed.replace(/(?:\s*\([^)]*\))+$/u, '').trim();

  // 2. Keep primary landmark clause before em-dash or ampersand compound subtitles
  if (compact.includes(' — ')) {
    const primary = compact.split(' — ')[0];
    if (primary) compact = primary.trim();
  } else if (compact.includes(' & ')) {
    const primary = compact.split(' & ')[0];
    if (primary) compact = primary.trim();
  }

  // 3. Strip redundant trailing region/era nouns when the landmark prefix already has >= 3 words
  const regionMatch = /^(.*?)\s+(Hương Cảng|Cửu Long|Lịch Sử)$/u.exec(compact);
  if (regionMatch && regionMatch[1]) {
    const base = regionMatch[1].trim();
    if (base.split(/\s+/u).length >= 3) {
      compact = base;
    }
  }

  // 4. Shorten overly long compound campaign corridor titles while keeping recognizable phrasing
  if (compact.length > 26 && compact.startsWith('Hành Lang Chiến Dịch ')) {
    compact = 'Hành Lang Chiến Dịch';
  }

  return compact || trimmed;
}

/**
 * Ensures browser tab title uses the canonical Sơn Mài & Giấy Dó title rather than
 * an empty title or legacy "NIÊN BIỂU KÝ ỨC · 1930 - 2045" shell metadata.
 */
export function normalizeDocumentTitle(doc: Document = document): string {
  const current = (doc.title ?? '').trim();
  if (!current || LEGACY_TITLE_PATTERN.test(current)) {
    doc.title = CANONICAL_DOCUMENT_TITLE;
  }
  return doc.title;
}

export const LIVE_UI_POLISH_CSS = `
/* 1. Minimap header & left HUD stack alignment — eliminate artificial 112px ellipsis clip */
#minimap-zone-title {
  max-width: 100%;
  flex: 1 1 auto;
  min-width: 0;
}

@media (min-width: 960px) {
  #hud-left-stack {
    width: 208px;
  }

  #minimap-panel:not(.is-collapsed),
  #hud-objective-tracker {
    width: 208px;
  }

  #minimap-panel:not(.is-collapsed) #minimap-canvas {
    width: 100%;
  }
}

/* 2. Stage toast vs DialoguePanel anti-collision — relocate toast below top location badge */
body.is-dialogue-open #stage-toast,
body.is-tutorial-bar-open.is-dialogue-open #stage-toast,
body:has(#dialogue-modal.is-open:not([hidden])) #stage-toast {
  top: 48px;
  bottom: auto;
  left: 50%;
  right: auto;
  transform: translate(-50%, 6px);
}

body.is-dialogue-open #stage-toast.is-visible,
body.is-tutorial-bar-open.is-dialogue-open #stage-toast.is-visible,
body:has(#dialogue-modal.is-open:not([hidden])) #stage-toast.is-visible {
  transform: translate(-50%, 0);
}

/* 3. Single-row compact tutorial coach bar on desktop viewports */
@media (min-width: 960px) and (min-height: 501px) {
  #tutorial-coach-bar.is-open {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    bottom: 12px;
    width: min(920px, calc(100vw - 36px));
    padding: 6px 12px;
  }

  #tutorial-coach-bar .tutorial-coach-top {
    flex: 0 0 auto;
    flex-wrap: nowrap;
    gap: 8px;
  }

  #tutorial-coach-bar .tutorial-coach-body {
    flex: 1 1 auto;
    min-width: 0;
    flex-wrap: nowrap;
    justify-content: space-between;
    gap: 12px;
  }

  #tutorial-coach-bar #tutorial-step-title {
    font-size: 0.95rem;
    white-space: nowrap;
  }

  #tutorial-coach-bar #tutorial-step-instruction {
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
    font-size: 0.9rem;
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  #tutorial-coach-bar .tutorial-coach-actions {
    flex: 0 0 auto;
    flex-wrap: nowrap;
    gap: 6px;
  }

  #tutorial-coach-bar #btn-tutorial-next,
  #tutorial-coach-bar #btn-tutorial-skip {
    min-height: 32px;
    padding: 4px 10px;
    font-size: 0.875rem;
  }

  body.is-tutorial-bar-open:not(.is-dialogue-open):not(.is-touch-controls) #stage-toast {
    bottom: 66px;
  }
}

/* 4. Guide & Journal tab bar hitbox alignment */
#instructions-modal .rpg-tab-bar,
#journal-modal .rpg-tab-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  position: relative;
  z-index: 3;
}

.rpg-tab-btn,
#instructions-modal [data-guide-tab] {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 42px;
  padding: 8px 16px;
  line-height: 1.25;
  position: relative;
  z-index: 3;
  pointer-events: auto;
  touch-action: manipulation;
  user-select: none;
  box-sizing: border-box;
}
`;

export interface LiveUiPolishHandle {
  syncDialogueState: () => void;
  destroy: () => void;
}

export function installLiveUiPolish(doc: Document = document): LiveUiPolishHandle {
  normalizeDocumentTitle(doc);

  let styleEl = doc.getElementById(LIVE_UI_POLISH_STYLE_ID) as HTMLStyleElement | null;
  if (!styleEl && doc.head) {
    styleEl = doc.createElement('style');
    styleEl.id = LIVE_UI_POLISH_STYLE_ID;
    styleEl.textContent = LIVE_UI_POLISH_CSS;
    doc.head.appendChild(styleEl);
  } else if (styleEl && styleEl.textContent !== LIVE_UI_POLISH_CSS) {
    styleEl.textContent = LIVE_UI_POLISH_CSS;
  }

  const syncDialogueState = (): void => {
    if (!doc.body) return;
    const dialogueModal = doc.getElementById('dialogue-modal');
    const isDialogueOpen = Boolean(
      dialogueModal &&
        dialogueModal.classList.contains('is-open') &&
        !dialogueModal.hasAttribute('hidden'),
    );
    doc.body.classList.toggle('is-dialogue-open', isDialogueOpen);
  };

  syncDialogueState();

  let observer: MutationObserver | null = null;
  const dialogueModal = doc.getElementById('dialogue-modal');
  if (dialogueModal && typeof MutationObserver !== 'undefined') {
    observer = new MutationObserver(() => {
      syncDialogueState();
    });
    observer.observe(dialogueModal, {
      attributes: true,
      attributeFilter: ['class', 'hidden'],
    });
  }

  return {
    syncDialogueState,
    destroy: () => {
      observer?.disconnect();
      observer = null;
      doc.body?.classList.remove('is-dialogue-open');
    },
  };
}
