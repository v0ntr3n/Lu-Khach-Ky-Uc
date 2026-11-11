// @vitest-environment jsdom
/**
 * @file tests/unit/m15_tutorial_decor_overhaul.test.ts
 * @description Kiểm thử đơn vị cho M15:
 * 1. Rút gọn nhãn bước Hướng dẫn (#tutorial-coach-bar: [1/5..5/5] + tiêu đề sạch không lặp "Bước N", nối phím [X]).
 * 2. Loại bỏ nhãn lặp ở PuzzlePanel (không còn "Mảnh [1] · 1.") & JournalPanel.
 * 3. Kho 36 Sprite Decor 20-màu Eastward (112 điểm đặt trên đủ 14 bản đồ, không chắn Spawn/Portal).
 * 4. 7 nguyên mẫu NPC thế giới theo thời kỳ (Bà Năm, Lão Chèo Đò, 1945–2045) & cố định biến thể gạch theo (c, r).
 */

import { describe, expect, it } from 'vitest';
import { InputManager } from '../../src/core/InputManager';
import { CH1_DOSSIER_PUZZLE } from '../../src/data/chapters/ch1_1930';
import { CHAPTERS } from '../../src/data/chronicles';
import {
  type CharacterAtlasRole,
  COLOR_MAP,
  DECOR_SPRITE_MATRICES,
  ZONE_DECOR_PROPS,
  drawCharacter4Dir,
  drawDecorSprite,
  drawPropInteractableSprite,
  drawTopDownTile,
} from '../../src/engine/SpriteAtlas';
import { getAllBuiltInMaps } from '../../src/runtime/MapLoader';
import { cleanPromptLabel } from '../../src/ui/GameHUD';
import { JournalPanel } from '../../src/ui/JournalPanel';
import { PuzzlePanel } from '../../src/ui/PuzzlePanel';
import { TUTORIAL_STEPS, TutorialController } from '../../src/ui/TutorialController';

function createMockCtx() {
  const rects: Array<{ x: number; y: number; w: number; h: number; fill: string }> = [];
  const ctx = {
    fillStyle: '#000000',
    globalAlpha: 1,
    imageSmoothingEnabled: false,
    save: () => {},
    restore: () => {},
    translate: () => {},
    scale: () => {},
    fillRect: (x: number, y: number, w: number, h: number) => {
      rects.push({ x, y, w, h, fill: String(ctx.fillStyle) });
    },
  } as unknown as CanvasRenderingContext2D;
  return { ctx, rects };
}

describe('M15: Streamlined Tutorial UI, 20-Color Decor Atlas & Era NPC Sprites', () => {
  it('M15-UT-01: TUTORIAL_STEPS uses compact N/5 badges without redundant "HƯỚNG DẪN" or "Bước N"', () => {
    expect(TUTORIAL_STEPS).toHaveLength(5);
    TUTORIAL_STEPS.forEach((step, idx) => {
      expect(step.badgeText).toBe(`${idx + 1}/5`);
      expect(step.badgeText).not.toMatch(/HƯỚNG DẪN/i);
      expect(step.title).not.toMatch(/^Bước\s*\d+/i);
      expect(step.instruction).not.toMatch(/\[(WASD|E|J|M|H|Tab|Esc)\]/i);
    });

    document.body.innerHTML = `
      <aside id="tutorial-coach-bar" hidden>
        <span id="tutorial-step-badge"></span>
        <strong id="tutorial-step-title"></strong>
        <kbd id="tutorial-step-keycap"></kbd>
        <p id="tutorial-step-instruction"></p>
        <button id="btn-tutorial-next" type="button"></button>
        <button id="btn-tutorial-skip" type="button"></button>
      </aside>
    `;

    const ctrl = new TutorialController();
    ctrl.setGameplayVisible(true);
    ctrl.startIfFirstPlay(false);
    expect(document.body.classList.contains('is-tutorial-bar-open')).toBe(true);
    expect(document.getElementById('tutorial-step-badge')?.textContent).toBe('1/5');
    expect(document.getElementById('tutorial-step-title')?.textContent).toBe(
      'Di chuyển Lữ Khách'
    );

    ctrl.skip();
    expect(document.body.classList.contains('is-tutorial-bar-open')).toBe(false);
  });

  it('M15-UT-02: InputManager binds KeyX to onSkipTutorial while ignoring Ctrl+X', () => {
    let skipCalls = 0;
    const input = new InputManager({
      onSkipTutorial: () => {
        skipCalls += 1;
      },
    });

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', ctrlKey: true }));
    expect(skipCalls).toBe(0);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'x' }));
    window.dispatchEvent(new KeyboardEvent('keyup', { key: 'x' }));
    expect(skipCalls).toBe(1);
    input.clearKeys();
  });

  it('M15-UT-03: PuzzlePanel and JournalPanel avoid duplicate numbering and duplicate completion text', () => {
    document.body.innerHTML = `
      <div id="dossier-puzzle-modal" hidden>
        <h3 id="dossier-puzzle-title"></h3>
        <p id="dossier-puzzle-subtitle"></p>
        <span id="dossier-provenance-badge" hidden></span>
        <p id="dossier-puzzle-instruction"></p>
        <div id="dossier-slots-container"></div>
        <div id="dossier-progressive-hint" hidden></div>
        <div id="dossier-feedback-banner" hidden></div>
        <button id="btn-submit-dossier" type="button"></button>
        <button id="btn-close-dossier" type="button"></button>
      </div>
      <div id="journal-modal" hidden>
        <div id="journal-codex-panel">
          <strong id="codex-progress-count"></strong>
          <div class="codex-two-col-grid">
            <div id="codex-milestone-list"></div>
            <aside id="codex-milestone-detail"></aside>
          </div>
        </div>
      </div>
    `;

    const puzzlePanel = new PuzzlePanel();
    puzzlePanel.open(CH1_DOSSIER_PUZZLE, 0);
    const firstGapNum =
      document.querySelector('.dossier-inline-gap-num')?.textContent ?? '';
    expect(firstGapNum).not.toContain('Mảnh [1]');
    expect(firstGapNum).toMatch(/^1\./);

    const journal = new JournalPanel();
    const ch1 = CHAPTERS[0]!;
    journal.updateCodex(ch1, new Set(['I']), ch1.milestones[0]!);
    const codexTop =
      document.querySelector('.codex-detail-top')?.textContent ?? '';
    expect(codexTop).toContain('CỘT MỐC I');
    expect(codexTop).toContain('ĐÃ KHÔI PHỤC');
    expect(codexTop).not.toContain('ẤN CHƯƠNG LỊCH SỬ');

    expect(
      cleanPromptLabel('Nhấn [E] — Kích hoạt Đèn An Toàn: Bàn Lưu Trữ Mật')
    ).toBe('Kích hoạt Đèn An Toàn: Bàn Lưu Trữ Mật');
    expect(cleanPromptLabel('[E] Xem xét: Cáo Thị Bến Cảng')).toBe(
      'Xem xét: Cáo Thị Bến Cảng'
    );
  });

  it('M15-UT-04: All 36 20-color Decor Sprites validate cleanly and cover all 14 maps without blocking spawns/portals', () => {
    const spriteEntries = Object.entries(DECOR_SPRITE_MATRICES);
    expect(spriteEntries.length).toBeGreaterThanOrEqual(36);

    for (const [spriteId, matrix] of spriteEntries) {
      expect(matrix.length).toBeGreaterThanOrEqual(10);
      const width = matrix[0]!.length;
      for (const row of matrix) {
        expect(row.length, `${spriteId} row width mismatch`).toBe(width);
        for (const ch of row) {
          expect(
            ch in COLOR_MAP,
            `Unknown COLOR_MAP token "${ch}" in ${spriteId}`
          ).toBe(true);
        }
      }
      const { ctx, rects } = createMockCtx();
      drawDecorSprite(ctx, spriteId as keyof typeof DECOR_SPRITE_MATRICES, 100, 100);
      expect(rects.length, `${spriteId} drew 0 pixels`).toBeGreaterThan(10);
    }

    const allMaps = getAllBuiltInMaps();
    let totalPlacements = 0;
    for (const [mapId, map] of Object.entries(allMaps)) {
      const placements = ZONE_DECOR_PROPS[mapId];
      expect(placements, `Missing ZONE_DECOR_PROPS for ${mapId}`).toBeDefined();
      expect(placements!.length).toBeGreaterThanOrEqual(5);
      totalPlacements += placements!.length;

      for (const item of placements!) {
        expect(item.x).toBeGreaterThan(16);
        expect(item.x).toBeLessThan(map.widthPx - 16);
        expect(item.y).toBeGreaterThan(16);
        expect(item.y).toBeLessThan(map.heightPx - 8);

        if (item.layer === 'ysort') {
          for (const sp of map.spawns) {
            expect(
              Math.hypot(item.x - sp.x, item.y - sp.y),
              `${item.id} too close to spawn ${sp.id} in ${mapId}`
            ).toBeGreaterThan(22);
          }
          for (const portal of map.portals) {
            const px = portal.x + portal.width * 0.5;
            const py = portal.y + portal.height * 0.5;
            expect(
              Math.hypot(item.x - px, item.y - py),
              `${item.id} too close to portal ${portal.id} in ${mapId}`
            ).toBeGreaterThan(22);
          }
        }
      }
    }
    expect(totalPlacements).toBeGreaterThanOrEqual(100);
  });

  it('M15-UT-05: Era NPC archetypes, prop interactables, and world-coord tile hashing render deterministically', () => {
    const newRoles: CharacterAtlasRole[] = [
      'vendor',
      'boatman',
      'cadre_1945',
      'soldier_1954',
      'commander_1975',
      'reform_1986',
      'archivist_2045',
    ];

    for (const role of newRoles) {
      for (const facing of ['down', 'up', 'left', 'right'] as const) {
        const { ctx, rects } = createMockCtx();
        drawCharacter4Dir(ctx, role, 80, 80, facing, true, false, 0.4, false);
        expect(rects.length, `${role}:${facing}`).toBeGreaterThan(40);
      }
    }

    // Prop interactables draw visibly
    const propMock = createMockCtx();
    drawPropInteractableSprite(propMock.ctx, 'prop_harbor_notice', 64, 64);
    expect(propMock.rects.length).toBeGreaterThan(30);

    // Tile variant stays identical for the same world (col, row) even when screen (x, y) shifts with camera!
    const panA = createMockCtx();
    const panB = createMockCtx();
    drawTopDownTile(panA.ctx, 2, 32, 48, false, '1930', 0, true, 12, 19);
    drawTopDownTile(panB.ctx, 2, 48, 64, false, '1930', 0, true, 12, 19);
    expect(panA.rects.map((r) => r.fill)).toEqual(panB.rects.map((r) => r.fill));
  });
});
