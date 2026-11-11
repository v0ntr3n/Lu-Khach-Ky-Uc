// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
  CANONICAL_DOCUMENT_TITLE,
  LIVE_UI_POLISH_CSS,
  LIVE_UI_POLISH_STYLE_ID,
  formatCompactMinimapTitle,
  installLiveUiPolish,
  normalizeDocumentTitle,
} from '../../src/ui/LiveUiPolish';

describe('M16 — Live UI Polish & RPG UI Design Skill', () => {
  afterEach(() => {
    document.head.innerHTML = '';
    document.body.className = '';
    document.body.innerHTML = '';
    document.title = '';
  });

  it('M16-UT-01: formatCompactMinimapTitle removes redundant region/year suffixes while preserving landmark names', () => {
    expect(formatCompactMinimapTitle('Quảng Trường Bến Cảng Hương Cảng (1930)')).toBe(
      'Quảng Trường Bến Cảng',
    );
    expect(formatCompactMinimapTitle('Kho Hàng Số 4 — Kho Hàng Bến Cảng (Nội Thất)')).toBe(
      'Kho Hàng Số 4',
    );
    expect(formatCompactMinimapTitle('Đình Tân Trào & Cây Đa Lịch Sử (08/1945)')).toBe(
      'Đình Tân Trào',
    );
    expect(formatCompactMinimapTitle('Quảng Trường Ba Đình Lịch Sử (02/09/1945)')).toBe(
      'Quảng Trường Ba Đình',
    );
    expect(formatCompactMinimapTitle('Đèo Pha Đin & Tuyến Tiếp Vận Điện Biên Phủ (1954)')).toBe(
      'Đèo Pha Đin',
    );
    expect(formatCompactMinimapTitle('Hành Lang Chiến Dịch Hồ Chí Minh (30/04/1975)')).toBe(
      'Hành Lang Chiến Dịch',
    );
    expect(formatCompactMinimapTitle('Đại Lộ Đổi Mới & Hội Nhập (1986 – 2011)')).toBe(
      'Đại Lộ Đổi Mới',
    );
    expect(formatCompactMinimapTitle('Quảng Trường Kỷ Nguyên Số (2021 – Viễn Cảnh 2045)')).toBe(
      'Quảng Trường Kỷ Nguyên Số',
    );
  });

  it('M16-UT-02: normalizeDocumentTitle replaces empty or legacy NIÊN BIỂU KÝ ỨC shell titles', () => {
    document.title = 'NIÊN BIỂU KÝ ỨC · 1930 - 2045';
    expect(normalizeDocumentTitle(document)).toBe(CANONICAL_DOCUMENT_TITLE);
    expect(document.title).toBe(CANONICAL_DOCUMENT_TITLE);

    document.title = '';
    expect(normalizeDocumentTitle(document)).toBe(CANONICAL_DOCUMENT_TITLE);

    document.title = 'Lữ Khách Ký Ức — Tuỳ biến';
    expect(normalizeDocumentTitle(document)).toBe('Lữ Khách Ký Ức — Tuỳ biến');
  });

  it('M16-UT-03: installLiveUiPolish syncs body.is-dialogue-open via MutationObserver and cleans up on destroy', async () => {
    document.title = 'NIÊN BIỂU KÝ ỨC · 1930 - 2045';
    document.body.innerHTML = `
      <section id="dialogue-modal" hidden></section>
      <div id="stage-toast"></div>
    `;

    const handle = installLiveUiPolish(document);
    expect(document.title).toBe(CANONICAL_DOCUMENT_TITLE);
    expect(document.getElementById(LIVE_UI_POLISH_STYLE_ID)).not.toBeNull();
    expect(document.body.classList.contains('is-dialogue-open')).toBe(false);

    const modal = document.getElementById('dialogue-modal')!;
    modal.removeAttribute('hidden');
    modal.classList.add('is-open');
    await Promise.resolve();
    expect(document.body.classList.contains('is-dialogue-open')).toBe(true);

    modal.classList.remove('is-open');
    modal.setAttribute('hidden', '');
    await Promise.resolve();
    expect(document.body.classList.contains('is-dialogue-open')).toBe(false);

    handle.destroy();
    expect(document.body.classList.contains('is-dialogue-open')).toBe(false);
  });

  it('M16-UT-04: LIVE_UI_POLISH_CSS obeys zero hardcoded color tokens, >= 0.8125rem font floor, and zero uppercase', () => {
    expect(LIVE_UI_POLISH_CSS).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(LIVE_UI_POLISH_CSS).not.toMatch(/\brgba?\s*\(/i);
    expect(LIVE_UI_POLISH_CSS).not.toMatch(/text-transform\s*:\s*uppercase/i);

    const remMatches = [...LIVE_UI_POLISH_CSS.matchAll(/font-size\s*:\s*([0-9.]+)rem/g)];
    expect(remMatches.length).toBeGreaterThan(0);
    for (const match of remMatches) {
      expect(Number(match[1])).toBeGreaterThanOrEqual(0.8125);
    }

    expect(LIVE_UI_POLISH_CSS).toContain('body.is-dialogue-open #stage-toast');
    expect(LIVE_UI_POLISH_CSS).toContain('#tutorial-coach-bar.is-open');
    expect(LIVE_UI_POLISH_CSS).toContain('#instructions-modal [data-guide-tab]');
  });

  it('M16-UT-05: .claude/skills/rpg-ui-design/SKILL.md is registered and documents token & overlay anti-collision rules', () => {
    const rootDir = resolve(__dirname, '../..');
    const skillContent = readFileSync(
      resolve(rootDir, '.claude/skills/rpg-ui-design/SKILL.md'),
      'utf8',
    );
    expect(skillContent).toContain('name: rpg-ui-design');
    expect(skillContent).toContain('Sơn Mài & Giấy Dó');
    expect(skillContent).toContain('Viewport Overlay Anti-Collision Matrix');
    expect(skillContent).toContain('0.8125rem');

    const skillsDoc = readFileSync(resolve(rootDir, 'docs/plans/SKILLS.md'), 'utf8');
    expect(skillsDoc).toContain('rpg-ui-design/SKILL.md');
  });
});
