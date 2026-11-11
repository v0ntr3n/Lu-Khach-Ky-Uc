// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { THEME_KEY_V1 } from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import { JournalPanel, type JournalQuestEntry } from '../../src/ui/JournalPanel';
import {
  MODERN_WEB_PLATFORM_CSS,
  MODERN_WEB_PLATFORM_STYLE_ID,
  ThemeManager,
} from '../../src/ui/ThemeManager';
import { createDomElement, setSanitizedHTML } from '../../src/ui/safeDom';

describe('M17 — Modern Web Guidance P1/P2 Hardening', () => {
  afterEach(() => {
    localStorage.clear();
    document.head.innerHTML = '';
    document.body.className = '';
    document.body.innerHTML = '';
  });

  it('M17-UT-01: ThemeManager keeps meta[name="color-scheme"] synced, cleans up system override storage, and syncs across tabs', () => {
    const manager = new ThemeManager('system');
    const meta = document.querySelector<HTMLMetaElement>('meta[name="color-scheme"]');
    expect(meta).not.toBeNull();
    expect(meta?.content).toBe('light dark');
    expect(localStorage.getItem(THEME_KEY_V1)).toBeNull();

    manager.setThemeMode('light', true);
    expect(meta?.content).toBe('light');
    expect(localStorage.getItem(THEME_KEY_V1)).toBe('light');

    // Cross-tab storage event updates meta[name="color-scheme"] and document theme
    localStorage.setItem(THEME_KEY_V1, 'dark');
    window.dispatchEvent(
      new StorageEvent('storage', {
        key: THEME_KEY_V1,
        oldValue: 'light',
        newValue: 'dark',
        storageArea: localStorage,
      })
    );
    expect(manager.getThemeMode()).toBe('dark');
    expect(meta?.content).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');

    // Returning to system removes the pinned localStorage override and restores "light dark"
    manager.setThemeMode('system', true);
    expect(meta?.content).toBe('light dark');
    expect(localStorage.getItem(THEME_KEY_V1)).toBeNull();

    manager.destroy();
  });

  it('M17-UT-02: MODERN_WEB_PLATFORM_CSS applies color-scheme, scrollbar-color, overscroll-behavior: contain, and content-visibility: auto with zero hardcoded colors', () => {
    const manager = new ThemeManager('system');
    expect(document.getElementById(MODERN_WEB_PLATFORM_STYLE_ID)).not.toBeNull();
    expect(MODERN_WEB_PLATFORM_CSS).toContain('color-scheme: light dark;');
    expect(MODERN_WEB_PLATFORM_CSS).toContain('scrollbar-color: var(--border-default)');
    expect(MODERN_WEB_PLATFORM_CSS).toContain('overscroll-behavior: contain;');
    expect(MODERN_WEB_PLATFORM_CSS).toContain('content-visibility: auto;');
    expect(MODERN_WEB_PLATFORM_CSS).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(MODERN_WEB_PLATFORM_CSS).not.toMatch(/\brgba?\s*\(/i);
    expect(MODERN_WEB_PLATFORM_CSS).not.toMatch(/text-transform\s*:\s*uppercase/i);
    manager.destroy();
  });

  it('M17-UT-03: setSanitizedHTML strips scripts, event handlers, and disallowed elements while JournalPanel has zero .innerHTML sinks', () => {
    const box = createDomElement('div');
    setSanitizedHTML(
      box,
      '<p class="lead" onclick="evil()" data-secret="1">Xin chào <strong>Lữ Khách</strong><script>window.__xss=1</script><img src="x" onerror="evil()"></p>'
    );

    expect(box.querySelector('script')).toBeNull();
    expect(box.querySelector('img')).toBeNull();
    const p = box.querySelector('p')!;
    expect(p.getAttribute('onclick')).toBeNull();
    expect(p.getAttribute('data-secret')).toBeNull();
    expect(p.className).toBe('lead');
    expect(p.textContent).toBe('Xin chào Lữ Khách');

    const journalSource = readFileSync(
      resolve(process.cwd(), 'src/ui/JournalPanel.ts'),
      'utf8'
    );
    expect(journalSource).not.toContain('.innerHTML');

    document.body.innerHTML = `
      <div id="journal-modal" hidden>
        <p id="journal-summary"></p>
        <div id="journal-quests-panel">
          <div class="journal-two-col-grid">
            <div id="journal-quest-list"></div>
            <aside id="journal-active-banner"></aside>
          </div>
        </div>
        <div id="journal-codex-panel" hidden>
          <strong id="codex-progress-count"></strong>
          <div class="codex-two-col-grid">
            <div id="codex-milestone-list"></div>
            <aside id="codex-milestone-detail"></aside>
          </div>
        </div>
      </div>
    `;

    const panel = new JournalPanel();
    const entries: JournalQuestEntry[] = [
      {
        quest: {
          id: 'q_xss',
          eraId: '1930',
          title: '<img src=x onerror=alert(1)>Hội Nghị 1930',
          subtitle: 'Sub',
          category: 'main',
          summary: '<script>alert(2)</script>Tóm tắt an toàn',
          objectives: [
            {
              id: 'obj_xss',
              title: '<b>Mục tiêu 1</b>',
              description: 'Mô tả an toàn',
              zoneId: 'harbor_square',
            },
          ],
        },
        objectives: [
          {
            objective: {
              id: 'obj_xss',
              title: '<b>Mục tiêu 1</b>',
              description: 'Mô tả an toàn',
              zoneId: 'harbor_square',
            },
            status: 'active',
          },
        ],
      },
    ];

    panel.updateState(entries, null, '', false, 'obj_xss', true);
    expect(
      document.querySelectorAll('#journal-modal img, #journal-modal script').length
    ).toBe(0);
    expect(document.getElementById('journal-quest-list')?.textContent).toContain(
      '<img src=x onerror=alert(1)>Hội Nghị 1930'
    );
    expect(document.querySelector('.journal-quest-lore summary')?.textContent).toBe(
      'Xem chi tiết & tư liệu'
    );

    const ch1 = CHAPTERS[0]!;
    panel.updateCodex(ch1, new Set(['I']), ch1.milestones[0]!);
    expect(document.querySelector('.codex-detail-top')?.textContent).toContain(
      '✓ ĐÃ KHÔI PHỤC'
    );
  });
});
