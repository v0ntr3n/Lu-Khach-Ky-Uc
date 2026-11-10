// @vitest-environment jsdom
/**
 * @file tests/unit/m13_journal_theme_system.test.ts
 * @description Bộ kiểm thử đơn vị (Unit Tests) cho M13:
 * 1. Hệ thống Theme (System / Light / Dark), lưu trữ localStorage và tự động đồng bộ matchMedia.
 * 2. Kiểm tra nghiêm ngặt Design Tokens trong index.html: Không có màu hardcode (#hex, rgb, rgba) ngoài khối định nghĩa biến CSS.
 * 3. Giao diện Nhật Ký Nhiệm Vụ & Biên Niên Sử 2 cột (JournalPanel) phong cách Eastward:
 *    - Cột trái tinh gọn, cột phải hiển thị chi tiết mục tiêu/cột mốc đang chọn.
 *    - Không lặp lại câu mô tả dài ở cả hai cột.
 *    - Tương tác chọn mục tiêu dẫn đường (Click-to-Track) và xem chi tiết mục tiêu khóa/hoàn thành.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { ThemeManager, resolveEffectiveTheme } from '../../src/ui/ThemeManager';
import { JournalPanel, type JournalQuestEntry } from '../../src/ui/JournalPanel';
import { SaveManager, THEME_KEY_V1 } from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import type { QuestDefinition } from '../../src/types/game';

describe('M13: Unified Eastward Theme System & 2-Column Quest Journal', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.removeAttribute('data-theme-mode');
    document.body.removeAttribute('data-theme');
    document.body.removeAttribute('data-theme-mode');
  });

  it('M13-UT-01: ThemeManager resolves light, dark, and system modes and persists to localStorage', () => {
    const saveManager = new SaveManager();
    const themeChanges: Array<{ mode: string; effective: string }> = [];

    const manager = new ThemeManager('dark');
    const unsubscribe = manager.onChange((effective, mode) => {
      themeChanges.push({ mode, effective });
    });

    expect(manager.getThemeMode()).toBe('dark');
    expect(manager.getEffectiveTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.dataset.themeMode).toBe('dark');
    expect(document.body.dataset.theme).toBe('dark');

    // Chuyển sang Light Mode tức thì không reload
    manager.setThemeMode('light');
    expect(manager.getThemeMode()).toBe('light');
    expect(manager.getEffectiveTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.dataset.themeMode).toBe('light');
    expect(document.body.dataset.theme).toBe('light');
    expect(localStorage.getItem(THEME_KEY_V1)).toBe('light');

    // Lưu qua SaveManager và nạp lại
    saveManager.saveSettings({
      reducedMotion: false,
      textScale: 1,
      showHints: true,
      muted: false,
      themeMode: 'light',
    });
    const loadedSettings = saveManager.loadSettings();
    expect(loadedSettings.themeMode).toBe('light');

    // Kiểm tra chế độ System với giả lập matchMedia
    const origMatchMedia = window.matchMedia;
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query.includes('prefers-color-scheme: light'),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));

    expect(resolveEffectiveTheme('system')).toBe('light');
    manager.setThemeMode('system');
    expect(document.documentElement.dataset.themeMode).toBe('system');
    expect(document.documentElement.dataset.theme).toBe('light');

    window.matchMedia = origMatchMedia;
    unsubscribe();
    expect(themeChanges.length).toBeGreaterThanOrEqual(2);
  });

  it('M13-UT-02: index.html defines centralized Light/Dark CSS tokens and zero hardcoded colors in component CSS rules', () => {
    const htmlPath = path.resolve(__dirname, '../../index.html');
    const html = fs.readFileSync(htmlPath, 'utf-8');

    // Kiểm tra script chống nháy theme (Zero-FOUC) trên <head>
    expect(html).toContain("localStorage.getItem('lu_khach_ky_uc_theme')");
    expect(html).toContain("[data-theme='light']");
    expect(html).toContain('--bg-canvas:');
    expect(html).toContain('--surface-primary:');
    expect(html).toContain('--text-primary:');
    expect(html).toContain('--border-default:');
    expect(html).toContain('--accent-primary:');
    expect(html).toContain('--status-tracked-bg:');

    // Trích xuất nội dung <style> và tách phần sau khối định nghĩa biến [data-theme='light']
    const styleMatch = html.match(/<style>([\s\S]*?)<\/style>/);
    expect(styleMatch).not.toBeNull();
    const styleContent = styleMatch![1];

    const lightBlockIndex = styleContent.indexOf("[data-theme='light']");
    expect(lightBlockIndex).toBeGreaterThan(0);

    // Tìm dấu đóng '}' của khối [data-theme='light']
    const afterLightStart = styleContent.slice(lightBlockIndex);
    const closingBraceOffset = afterLightStart.indexOf('}');
    expect(closingBraceOffset).toBeGreaterThan(0);

    const componentCss = afterLightStart
      .slice(closingBraceOffset + 1)
      .replace(/\/\*[\s\S]*?\*\//g, ''); // Xóa comment CSS

    const hardcodedHex = componentCss.match(/#[0-9a-fA-F]{3,8}\b/g) ?? [];
    const hardcodedRgb = componentCss.match(/\brgba?\s*\(/g) ?? [];

    expect(hardcodedHex).toEqual([]);
    expect(hardcodedRgb).toEqual([]);
  });

  it('M13-UT-03: JournalPanel renders 2-column layout for Quests and Codex without duplicate text and supports Click-to-Track', () => {
    document.body.innerHTML = `
      <button id="btn-open-journal" type="button"></button>
      <div id="journal-modal" hidden>
        <button id="btn-close-journal" type="button"></button>
        <p id="journal-summary"></p>
        <button id="tab-journal-quests" class="is-active" aria-selected="true"></button>
        <button id="tab-journal-codex" aria-selected="false"></button>
        <div id="journal-quests-panel">
          <div class="journal-two-col-grid">
            <div id="journal-quest-list"></div>
            <aside id="journal-active-banner"></aside>
          </div>
        </div>
        <div id="journal-codex-panel" hidden>
          <strong id="codex-progress-count">0 / 12 CỘT MỐC</strong>
          <div class="codex-two-col-grid">
            <div id="codex-milestone-list"></div>
            <aside id="codex-milestone-detail"></aside>
          </div>
        </div>
      </div>
    `;

    const trackedIds: string[] = [];
    const panel = new JournalPanel({
      onSelectObjective: (objId) => trackedIds.push(objId),
    });

    const sampleQuest: QuestDefinition = {
      id: 'q_main_1930',
      eraId: '1930',
      title: 'Hội Nghị Hợp Nhất Hương Cảng',
      subtitle: 'Mùa xuân năm 1930',
      category: 'main',
      summary: 'Kết nối các đầu mối liên lạc bí mật tại Cửu Long.',
      provenance: {
        type: 'historical_fact',
        label: 'Lịch sử Đảng CSVN',
        citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương I',
      },
      objectives: [
        {
          id: 'obj_1',
          title: 'Gặp đồng chí Lâm Đức Thụ',
          description: 'Tìm gặp người liên lạc tại Quảng trường Bến Cảng để nhận mật hiệu.',
          zoneId: 'harbor_square',
          hintText: 'Tiến lại gần đèn lồng đỏ phía đông bến cảng.',
        },
        {
          id: 'obj_2',
          title: 'Kiểm tra kho hàng Cửu Long',
          description: 'Tìm mảnh ghép Cương lĩnh đầu tiên trong nhà kho.',
          zoneId: 'warehouse_interior',
        },
      ],
    };

    const entries: JournalQuestEntry[] = [
      {
        quest: sampleQuest,
        objectives: [
          { objective: sampleQuest.objectives[0], status: 'active' },
          {
            objective: sampleQuest.objectives[1],
            status: 'locked',
            prerequisiteText: 'Hoàn thành cuộc gặp tại Bến Cảng trước.',
          },
        ],
      },
    ];

    panel.updateState(
      entries,
      { quest: sampleQuest, objective: sampleQuest.objectives[0] },
      'Tiến lại gần đèn lồng đỏ phía đông bến cảng.',
      false,
      'obj_1',
      true
    );

    const questListEl = document.getElementById('journal-quest-list')!;
    const inspectorEl = document.getElementById('journal-active-banner')!;

    // Cột trái chỉ hiển thị tiêu đề bước mục tiêu ngắn gọn, không lặp lại đoạn mô tả dài
    expect(questListEl.textContent).toContain('Gặp đồng chí Lâm Đức Thụ');
    expect(questListEl.textContent).not.toContain(
      'Tìm gặp người liên lạc tại Quảng trường Bến Cảng để nhận mật hiệu.'
    );

    // Cột phải (Inspector) hiển thị đầy đủ mô tả, gợi ý và nguồn tư liệu
    expect(inspectorEl.textContent).toContain(
      'Tìm gặp người liên lạc tại Quảng trường Bến Cảng để nhận mật hiệu.'
    );
    expect(inspectorEl.textContent).toContain('Giáo trình Lịch sử Đảng CSVN (2021)');

    // Nhấp vào mục tiêu đang khóa ở cột trái -> cập nhật cột phải sang mục tiêu khóa mà không đóng modal
    const lockedRow = questListEl.querySelector<HTMLElement>('[data-objective-id="obj_2"]')!;
    lockedRow.click();
    expect(inspectorEl.textContent).toContain('Hoàn thành cuộc gặp tại Bến Cảng trước.');
    expect(trackedIds).toEqual([]);

    // Nhấp vào mục tiêu active ở cột trái -> kích hoạt onSelectObjective
    const activeRow = questListEl.querySelector<HTMLElement>('[data-objective-id="obj_1"]')!;
    activeRow.click();
    expect(trackedIds).toEqual(['obj_1']);

    // Kiểm tra ngăn Biên Niên Sử 2 cột
    const ch1 = CHAPTERS[0];
    panel.updateCodex(ch1, new Set(['I']), ch1.milestones[0]);
    const codexDetail = document.getElementById('codex-milestone-detail')!;
    expect(document.getElementById('codex-progress-count')?.textContent).toBe(
      '1 / 12 CỘT MỐC'
    );
    expect(codexDetail.textContent).toContain(ch1.milestones[0].title);
  });
});
