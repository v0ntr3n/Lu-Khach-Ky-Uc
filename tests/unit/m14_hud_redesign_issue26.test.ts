// @vitest-environment jsdom
/**
 * @file tests/unit/m14_hud_redesign_issue26.test.ts
 * @description Bộ kiểm thử đơn vị (Unit Tests) cho Issue #26 (M14):
 * 1. HUD nhiệm vụ 1 dòng (hudText ngắn gọn, không có hậu tố [E] tĩnh).
 * 2. Định dạng Stage Intro (ẢI 1..4 : <Tên ải>).
 * 3. Đồng bộ hậu tố [E] thời gian thực trên GameHUD khi vào/ra vùng tương tác.
 * 4. Chống phát lại Stage Intro khi làm mới HUD hoặc đổi theme Light/Dark.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PALETTE_12, PALETTE_ENV, PALETTE_20 } from '../../src/data/chronicles';
import {
  ZONE_VIGNETTE_CLUSTERS,
  drawZoneVignetteClusters,
} from '../../src/engine/SpriteAtlas';
import { getAllBuiltInMaps } from '../../src/runtime/MapLoader';
import { CH1_DOSSIER_PUZZLE } from '../../src/data/chapters/ch1_1930';
import { QUEST_CATALOG } from '../../src/systems/QuestSystem';
import {
  GameHUD,
  STAGE_INTRO_LABELS,
  formatStageIntroLabel,
  stripStaticInteractKey,
} from '../../src/ui/GameHUD';
import { formatDialoguePageWithSingleHighlight } from '../../src/ui/DialoguePanel';
import { PuzzlePanel } from '../../src/ui/PuzzlePanel';

describe('M14 (Issue #26): 1-Line Quest HUD, Top-Center Location Label & Stage Intro', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="hud-layer">
        <div id="region-entry-banner" hidden>
          <span id="region-entry-era" hidden></span>
          <strong id="region-entry-title"></strong>
        </div>
        <div id="hud-top-left">
          <div class="hud-location-plaque">
            <span id="stage-chapter-num" hidden></span>
            <span class="hud-plaque-sep" hidden>◆</span>
            <span id="stage-location-badge"></span>
          </div>
          <span id="stage-shelter-status" hidden></span>
          <span id="stage-chapter-title" hidden></span>
          <span id="stage-mechanic-hint" hidden></span>
        </div>
        <div id="hud-objective-tracker" role="button" tabindex="0">
          <div id="hud-active-quest-title" hidden></div>
          <div id="hud-active-objective-text"></div>
          <div id="hud-active-objective-action" hidden></div>
          <div id="hud-active-objective-hint" hidden></div>
        </div>
      </div>
      <div id="world-interaction-prompt" hidden>
        <kbd class="prompt-key">E</kbd>
        <span id="world-interaction-label"></span>
      </div>
      <div id="stage-toast"></div>
      <span id="minimap-zone-title"></span>
      <div id="minimap-route-banner"></div>
    `;
  });

  it('M14-UT-01: Every quest objective defines a concise 1-line hudText without static [E]', () => {
    expect(QUEST_CATALOG.length).toBeGreaterThanOrEqual(5);
    for (const quest of QUEST_CATALOG) {
      expect(quest.hudTitle?.trim().length).toBeGreaterThan(0);
      for (const obj of quest.objectives) {
        expect(obj.hudText).toBeDefined();
        const text = obj.hudText!.trim();
        expect(text.length).toBeGreaterThan(0);
        expect(text.length).toBeLessThanOrEqual(42);
        expect(text).not.toMatch(/\[E\]/i);
        expect(text).not.toContain('→');
      }
    }
  });

  it('M14-UT-02: formatStageIntroLabel formats ẢI 1..4 : <Tên ải> accurately', () => {
    expect(formatStageIntroLabel('1930')).toBe('ẢI 1 : Hương Cảng 1930');
    expect(formatStageIntroLabel('1945')).toBe('ẢI 2 : Tân Trào – Ba Đình 1945');
    expect(formatStageIntroLabel('1954_1975')).toBe('ẢI 3 : Điện Biên – Mùa Xuân 1975');
    expect(formatStageIntroLabel('1986_2045')).toBe('ẢI 4 : Đổi Mới – Kỷ Nguyên Số 2045');
    expect(Object.keys(STAGE_INTRO_LABELS)).toEqual([
      '1930',
      '1945',
      '1954_1975',
      '1986_2045',
    ]);
    expect(stripStaticInteractKey('→ Gặp Giao Liên · Bến Cảng [E]')).toBe(
      'Gặp Giao Liên · Bến Cảng'
    );
  });

  it('M14-UT-03: GameHUD renders 1-line objective and dynamically appends [E] only when in interaction range', () => {
    const hud = new GameHUD();
    const firstQuest = QUEST_CATALOG[0]!;
    const firstObjective = firstQuest.objectives[0]!;

    hud.updateObjectiveTracker(
      { quest: firstQuest, objective: firstObjective },
      'Tiến tới đèn lồng cảng và nhấn [E]',
      true,
      'Gặp Giao Liên · Bến Cảng'
    );

    const objTextEl = document.getElementById('hud-active-objective-text')!;
    const questTitleEl = document.getElementById('hud-active-quest-title')!;
    const actionEl = document.getElementById('hud-active-objective-action')!;
    const hintEl = document.getElementById('hud-active-objective-hint')!;

    // Khi đứng xa: chỉ hiện đúng 1 dòng ngắn gọn, không có [E], các dòng phụ đều ẩn
    expect(objTextEl.textContent).toBe('Gặp Giao Liên · Bến Cảng');
    expect(questTitleEl.hidden).toBe(true);
    expect(actionEl.hidden).toBe(true);
    expect(hintEl.hidden).toBe(true);

    // Khi bước vào vùng tương tác thực tế: tự động nối thêm ' [E]'
    hud.updateInteractionPrompt('Trò chuyện: Giao Liên Lâm Văn Bính', 240, 160);
    expect(objTextEl.textContent).toBe('Gặp Giao Liên · Bến Cảng [E]');

    // Khi rời khỏi vùng tương tác: hậu tố ' [E]' biến mất ngay lập tức
    hud.updateInteractionPrompt(null);
    expect(objTextEl.textContent).toBe('Gặp Giao Liên · Bến Cảng');
  });

  it('M14-UT-04: Stage Intro deduplicates per eraId so HUD sync or theme toggle never replays it', () => {
    const hud = new GameHUD();

    const bannerEl = document.getElementById('region-entry-banner')!;
    const titleEl = document.getElementById('region-entry-title')!;
    const hudLayerEl = document.getElementById('hud-layer')!;

    hud.showStageIntro('1930');
    expect(bannerEl.hidden).toBe(false);
    expect(titleEl.textContent).toBe('ẢI 1 : Hương Cảng 1930');
    expect(hudLayerEl.classList.contains('is-stage-intro-active')).toBe(true);

    // Giả lập intro kết thúc rồi gọi lại cùng eraId '1930' (không force) -> không phát lại
    bannerEl.hidden = true;
    hudLayerEl.classList.remove('is-stage-intro-active');

    hud.showStageIntro('1930');
    expect(bannerEl.hidden).toBe(true);

    // Chuyển sang Ải 2 ('1945') -> phát intro mới
    hud.showStageIntro('1945');
    expect(bannerEl.hidden).toBe(false);
    expect(titleEl.textContent).toBe('ẢI 2 : Tân Trào – Ba Đình 1945');
  });

  it('M14-UT-05: 20-Shade Warm-Cool Palette & Asymmetric Vignette Clusters cover all 14 maps', () => {
    expect(Object.keys(PALETTE_12)).toHaveLength(12);
    expect(Object.keys(PALETTE_ENV)).toHaveLength(8);
    expect(Object.keys(PALETTE_20)).toHaveLength(20);
    expect(PALETTE_20.INDIGO_NIGHT_DEEP).toBe('#16202A');
    expect(PALETTE_20.MOSS_FOLIAGE).toBe('#4A5D46');
    expect(PALETTE_20.WET_STONE_DARK).toBe('#272A2C');

    const allMaps = getAllBuiltInMaps();
    for (const mapId of Object.keys(allMaps)) {
      const clusters = ZONE_VIGNETTE_CLUSTERS[mapId as keyof typeof ZONE_VIGNETTE_CLUSTERS];
      expect(clusters).toBeDefined();
      expect(clusters!.length).toBeGreaterThanOrEqual(2);
    }

    const fillCalls: string[] = [];
    const mockCtx = {
      fillStyle: '#000000',
      save: () => {},
      restore: () => {},
      fillRect: () => {
        fillCalls.push(String(mockCtx.fillStyle));
      },
    } as unknown as CanvasRenderingContext2D;

    drawZoneVignetteClusters(mockCtx, allMaps.harbor_square!, 0, 0, 1.5);
    expect(fillCalls.length).toBeGreaterThan(20);
  });

  it('M14-UT-06: Typography audit — only 2 font families (Be Vietnam Pro & Newsreader), zero uppercase rules, and >=13px (0.8125rem) font sizes', () => {
    const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8');

    // Chỉ tải 2 họ font từ Google Fonts
    expect(html).toContain('family=Be+Vietnam+Pro');
    expect(html).toContain('family=Newsreader');
    expect(html).not.toContain('Cormorant');
    expect(html).not.toContain('JetBrains');

    // Không dùng text-transform: uppercase làm đứt dấu tiếng Việt
    expect(html).not.toMatch(/text-transform:\s*uppercase/i);

    // Mọi khai báo font-size theo rem đều >= 0.8125rem (13px)
    const remMatches = [...html.matchAll(/font-size:\s*([0-9.]+)rem/g)];
    expect(remMatches.length).toBeGreaterThan(20);
    for (const m of remMatches) {
      const val = parseFloat(m[1]!);
      expect(val).toBeGreaterThanOrEqual(0.8125);
    }
  });

  it('M14-UT-07: formatDialoguePageWithSingleHighlight highlights at most 1 key phrase per page', () => {
    const sample =
      "“Đồng chí giữ kỹ mảnh khẩu hiệu 'Vô sản và dân tộc bị áp bức' cho kỳ họp 3–7/2/1930 tại Hương Cảng 1930.”";
    const formatted = formatDialoguePageWithSingleHighlight(sample);
    const matches = formatted.match(/class="dialogue-highlight"/g) ?? [];
    expect(matches.length).toBe(1);
  });

  it('M14-UT-08: PuzzlePanel renders 1 seamless historical document sheet with 3 inline gap slots and Crimson Seal stamp', () => {
    document.body.innerHTML += `
      <div id="dossier-puzzle-modal" hidden>
        <h3 id="dossier-puzzle-title"></h3>
        <p id="dossier-puzzle-subtitle"></p>
        <span id="dossier-provenance-badge" hidden></span>
        <p id="dossier-puzzle-instruction"></p>
        <div id="dossier-slots-container"></div>
        <div id="dossier-progressive-hint" hidden></div>
        <div id="dossier-feedback-banner" hidden></div>
        <button id="btn-submit-dossier" type="button">Đối chiếu</button>
        <button id="btn-close-dossier" type="button">Đóng</button>
      </div>
    `;

    const puzzleCh1 = CH1_DOSSIER_PUZZLE;
    const panel = new PuzzlePanel({
      onSubmitDossierPuzzle: (_id, selected) => ({
        solved: Object.keys(selected).length === puzzleCh1.slots.length,
        attemptCount: 1,
        feedback: 'Đã xác thực văn kiện!',
        showProgressiveHint: false,
      }),
    });

    panel.open(puzzleCh1, 0);

    const sheet = document.querySelector('.dossier-document-sheet');
    expect(sheet).not.toBeNull();

    const inlineGaps = document.querySelectorAll('.dossier-inline-gap-slot');
    expect(inlineGaps.length).toBe(puzzleCh1.slots.length);

    const sealEl = document.getElementById('dossier-crimson-seal')!;
    expect(sealEl.hidden).toBe(true);

    // Nhấp chọn mảnh tư liệu đầu tiên của từng ô khuyết
    for (const slot of puzzleCh1.slots) {
      const firstOpt = slot.options[0]!;
      document
        .querySelector<HTMLButtonElement>(
          `[data-slot-id="${slot.id}"][data-option-id="${firstOpt.id}"]`
        )!
        .click();
    }

    expect(inlineGaps[0]!.classList.contains('is-filled')).toBe(true);

    // Gửi đối chiếu -> Dấu Triện Son hiện lên
    document.getElementById('btn-submit-dossier')!.click();
    expect(sealEl.hidden).toBe(false);
  });
});

