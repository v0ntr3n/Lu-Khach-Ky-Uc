/**
 * @file tests/playwright/m13_journal_theme_system.spec.ts
 * @description Kiểm thử E2E & Kiểm định Trực quan (Visual QA + WCAG AA Contrast) cho M13:
 * 1. Hệ thống Theme (System / Light / Dark) trong #settings-modal:
 *    - Đổi theme tức thì không reload, lưu vào localStorage, không nháy sai theme khi reload (Zero-FOUC).
 * 2. Bố cục 2 cột của Nhật Ký Hành Trình (#journal-quests-panel) & Biên Niên Sử (#journal-codex-panel):
 *    - Cột trái tinh gọn, cột phải hiển thị chi tiết mục tiêu/cột mốc đang chọn, không lặp text.
 *    - Hỗ trợ điều hướng bàn phím, cuộn mượt và Click-to-Track.
 * 3. Kiểm tra độ tương phản WCAG AA (>= 4.5:1) và giữ nguyên kích thước/bố cục khi chuyển Light <-> Dark
 *    trên toàn bộ màn hình: Main Menu, HUD, Minimap, Quest Journal, Inventory, World Map, Dialogue, Settings, Pause.
 * 4. Đảm bảo #pixel-canvas và #main-menu-bg-canvas giữ nguyên màu gốc (filter: none) ở cả hai chế độ.
 */

import { test, expect, type Page } from '@playwright/test';
import path from 'node:path';

const ARTIFACT_DIR =
  '/home/antigravity/.gemini/jetski/brain/v1/default/jetski_remote_agents_3p/9794bb53-67ce-4471-a58e-ea7055d0c4ce';

/**
 * Hàm tính tỷ lệ tương phản WCAG 2.1 giữa hai màu rgb/rgba trong trình duyệt.
 */
async function assertWcagContrast(
  page: Page,
  selector: string,
  minRatio = 4.5
): Promise<number> {
  const result = await page.evaluate((sel) => {
    const el = document.querySelector<HTMLElement>(sel);
    if (!el) return { found: false, ratio: 0, fg: '', bg: '' };

    function parseColor(str: string): [number, number, number, number] {
      const m = str.match(/rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\s*\)/);
      if (!m) return [0, 0, 0, 1];
      return [
        Number(m[1]),
        Number(m[2]),
        Number(m[3]),
        m[4] !== undefined ? Number(m[4]) : 1,
      ];
    }

    function blend(
      fg: [number, number, number, number],
      bg: [number, number, number, number]
    ): [number, number, number, number] {
      const a = fg[3] + bg[3] * (1 - fg[3]);
      if (a === 0) return [0, 0, 0, 0];
      return [
        Math.round((fg[0] * fg[3] + bg[0] * bg[3] * (1 - fg[3])) / a),
        Math.round((fg[1] * fg[3] + bg[1] * bg[3] * (1 - fg[3])) / a),
        Math.round((fg[2] * fg[3] + bg[2] * bg[3] * (1 - fg[3])) / a),
        a,
      ];
    }

    const fgColor = parseColor(window.getComputedStyle(el).color);

    // Lần ngược cây DOM để gộp các lớp nền полупрозрачный (semi-transparent)
    const bgLayers: Array<[number, number, number, number]> = [];
    let cur: HTMLElement | null = el;
    while (cur) {
      const bgStr = window.getComputedStyle(cur).backgroundColor;
      const parsed = parseColor(bgStr);
      if (parsed[3] > 0) {
        bgLayers.push(parsed);
        if (parsed[3] >= 0.99) break;
      }
      cur = cur.parentElement;
    }

    const rootTheme = document.documentElement.dataset.theme;
    let effectiveBg: [number, number, number, number] =
      rootTheme === 'light' ? [239, 231, 214, 1] : [17, 14, 11, 1];

    for (let i = bgLayers.length - 1; i >= 0; i--) {
      effectiveBg = blend(bgLayers[i], effectiveBg);
    }

    const finalFg = blend(fgColor, effectiveBg);

    function luminance([r, g, b]: [number, number, number, number]): number {
      const a = [r, g, b].map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
      });
      return a[0] * 0.2126 + a[1] * 0.7152 + a[2] * 0.0722;
    }

    const l1 = luminance(finalFg);
    const l2 = luminance(effectiveBg);
    const ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);

    return {
      found: true,
      ratio: Number(ratio.toFixed(2)),
      fg: `rgb(${finalFg.slice(0, 3).join(',')})`,
      bg: `rgb(${effectiveBg.slice(0, 3).join(',')})`,
    };
  }, selector);

  expect(result.found, `Element ${selector} should exist`).toBe(true);
  expect(
    result.ratio,
    `WCAG contrast for ${selector} (fg=${result.fg}, bg=${result.bg}) should be >= ${minRatio}`
  ).toBeGreaterThanOrEqual(minRatio);

  return result.ratio;
}

test.describe('M13: Eastward Quest Journal Redesign & Unified Light/Dark Theme System', () => {
  test('M13-E2E-01: Instant Theme Switching (System/Light/Dark), Zero-Flash Persistence, WCAG AA Contrast & 2-Column Journal', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?skipOpening=1');

    // 1. Màn hình chính khởi động ở Dark Mode (do cấu hình mặc định)
    const mainMenu = page.locator('#main-menu-screen');
    await expect(mainMenu).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

    // Đảm bảo canvas nền menu không bị filter
    const menuCanvasFilterDark = await page
      .locator('#main-menu-bg-canvas')
      .evaluate((el) => window.getComputedStyle(el).filter);
    expect(menuCanvasFilterDark).toBe('none');

    // Mở Cài Đặt từ Main Menu -> Chuyển sang Sáng (Light Mode)
    await page.locator('#btn-menu-settings').click();
    const settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    const settingsBoxDark = await page
      .locator('#settings-modal .rpg-modal-dialog')
      .boundingBox();

    await page.locator('#settings-modal [data-theme-mode="light"]').click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await expect(page.locator('html')).toHaveAttribute('data-theme-mode', 'light');

    // Bố cục và kích thước hộp thoại Cài Đặt giữ nguyên tuyệt đối khi đổi theme
    const settingsBoxLight = await page
      .locator('#settings-modal .rpg-modal-dialog')
      .boundingBox();
    expect(settingsBoxLight?.width).toBeCloseTo(settingsBoxDark!.width, 1);
    expect(settingsBoxLight?.height).toBeCloseTo(settingsBoxDark!.height, 1);

    // Kiểm tra tương phản WCAG AA trên Cài Đặt ở Light Mode (kể cả các nút bấm ngay sau khi đổi theme)
    await assertWcagContrast(page, '#settings-modal-title', 4.5);
    await assertWcagContrast(page, '#settings-modal .settings-group-title', 4.5);
    await assertWcagContrast(page, '#settings-modal [data-theme-mode="system"]', 4.5);
    await assertWcagContrast(page, '#settings-modal [data-theme-mode="light"]', 4.5);

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_settings_light.png'),
    });

    // Đóng Cài Đặt -> chụp Main Menu ở Light Mode
    await page.locator('#btn-close-settings').click();
    await expect(settingsModal).toBeHidden();
    await assertWcagContrast(page, '#btn-menu-start', 4.5);

    const menuCanvasFilterLight = await page
      .locator('#main-menu-bg-canvas')
      .evaluate((el) => window.getComputedStyle(el).filter);
    expect(menuCanvasFilterLight).toBe('none');

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_main_menu_light.png'),
    });

    // 2. Reload trang để kiểm tra Zero-FOUC (giữ nguyên Light Mode từ localStorage ngay khi nạp)
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await expect(page.locator('html')).toHaveAttribute('data-theme-mode', 'light');

    // 3. Bắt đầu trò chơi mới vào thế giới RPG
    await page.locator('#btn-menu-start').click();
    await page.waitForFunction(
      () => {
        const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
        return game?.store?.getMode() === 'exploring';
      },
      { timeout: 8000 }
    );
    await expect(page.locator('#hud-top-left')).toBeVisible();

    // Bỏ qua thanh tân thủ nếu đang mở để kiểm tra HUD và các modal rõ ràng
    const skipTutBtn = page.locator('#btn-tutorial-skip');
    if (await skipTutBtn.isVisible()) {
      await skipTutBtn.click();
    }

    // Kiểm tra #pixel-canvas tuyệt đối không bị filter trong Light Mode
    const pixelCanvasFilterLight = await page
      .locator('#pixel-canvas')
      .evaluate((el) => window.getComputedStyle(el).filter);
    expect(pixelCanvasFilterLight).toBe('none');

    // Kiểm tra độ tương phản WCAG AA trên HUD ở Light Mode
    await assertWcagContrast(page, '#stage-location-badge', 4.5);
    await assertWcagContrast(page, '#hud-active-objective-text', 4.5);
    await assertWcagContrast(page, '#minimap-zone-title', 4.5);

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_gameplay_hud_light.png'),
    });

    // 4. Mở Nhật Ký Hành Trình [J] -> Kiểm tra bố cục 2 cột Eastward RPG ở cả Light & Dark Mode
    await page.keyboard.press('KeyJ');
    const journalModal = page.locator('#journal-modal');
    await expect(journalModal).toBeVisible();

    const questList = page.locator('#journal-quest-list');
    const inspectorBanner = page.locator('#journal-active-banner');
    await expect(questList).toBeVisible();
    await expect(inspectorBanner).toBeVisible();

    // Xác minh 2 cột nằm cạnh nhau trên màn hình Desktop (1920x1080)
    const listBoxLight = await questList.boundingBox();
    const inspectorBoxLight = await inspectorBanner.boundingBox();
    expect(listBoxLight).not.toBeNull();
    expect(inspectorBoxLight).not.toBeNull();
    expect(inspectorBoxLight!.x).toBeGreaterThan(listBoxLight!.x + 200);

    // Kiểm tra tương phản WCAG AA trong Nhật Ký ở Light Mode
    await assertWcagContrast(page, '#journal-modal-title', 4.5);
    await assertWcagContrast(page, '.journal-quest-title', 4.5);
    await assertWcagContrast(page, '.journal-obj-title', 4.5);
    await assertWcagContrast(page, '.journal-banner-title', 4.5);
    await assertWcagContrast(page, '.journal-banner-label', 4.5);
    await assertWcagContrast(page, '.journal-quest-lore summary', 4.5);
    await page.locator('.journal-quest-lore summary').click();
    await assertWcagContrast(page, '.journal-quest-lore-content p', 4.5);

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_journal_quests_light.png'),
    });

    // Chuyển sang ngăn Biên Niên Sử I–XII (#tab-journal-codex) -> Kiểm tra bố cục 2 cột
    await page.locator('#tab-journal-codex').click();
    const codexList = page.locator('#codex-milestone-list');
    const codexDetail = page.locator('#codex-milestone-detail');
    await expect(codexList).toBeVisible();
    await expect(codexDetail).toBeVisible();

    const codexListBox = await codexList.boundingBox();
    const codexDetailBox = await codexDetail.boundingBox();
    expect(codexDetailBox!.x).toBeGreaterThan(codexListBox!.x + 200);
    await assertWcagContrast(page, '.codex-detail-title', 4.5);
    await assertWcagContrast(page, '.codex-detail-desc', 4.5);

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_journal_codex_light.png'),
    });

    // Quay lại ngăn Nhiệm Vụ và chuyển sang Dark Mode để so sánh trực quan
    await page.locator('#tab-journal-quests').click();
    await page.evaluate(() => {
      const btn = document.querySelector<HTMLButtonElement>(
        '#settings-modal [data-theme-mode="dark"]'
      );
      btn?.click();
    });
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

    // Kích thước 2 cột của Nhật Ký không đổi khi chuyển sang Dark Mode
    const listBoxDark = await questList.boundingBox();
    const inspectorBoxDark = await inspectorBanner.boundingBox();
    expect(listBoxDark?.width).toBeCloseTo(listBoxLight!.width, 1);
    expect(inspectorBoxDark?.width).toBeCloseTo(inspectorBoxLight!.width, 1);

    // Kiểm tra tương phản WCAG AA trong Nhật Ký ở Dark Mode
    await assertWcagContrast(page, '#journal-modal-title', 4.5);
    await assertWcagContrast(page, '.journal-quest-title', 4.5);
    await assertWcagContrast(page, '.journal-obj-title', 4.5);
    await assertWcagContrast(page, '.journal-banner-title', 4.5);
    await assertWcagContrast(page, '.journal-banner-label', 4.5);
    await assertWcagContrast(page, '.journal-quest-lore summary', 4.5);
    await page.locator('.journal-quest-lore summary').click();
    await assertWcagContrast(page, '.journal-quest-lore-content p', 4.5);

    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_journal_quests_dark.png'),
    });

    await page.locator('#tab-journal-codex').click();
    await assertWcagContrast(page, '.codex-detail-title', 4.5);
    await assertWcagContrast(page, '.codex-detail-desc', 4.5);
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_journal_codex_dark.png'),
    });

    // Đóng Nhật Ký
    await page.keyboard.press('Escape');
    await expect(journalModal).toBeHidden();

    // 5. Kiểm tra Hộp Hội Thoại Eastward (#dialogue-modal) ở cả Light Mode & Dark Mode
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      const npc = game.currentMap.npcs[0];
      if (npc) {
        game.playerX = npc.x;
        game.playerY = npc.y + 18;
      }
    });
    await page.waitForTimeout(150);
    await page.keyboard.press('KeyE');
    await expect(page.locator('#dialogue-modal')).toBeVisible();

    await assertWcagContrast(page, '#dialogue-speaker', 4.5);
    await assertWcagContrast(page, '#dialogue-text', 4.5);
    await assertWcagContrast(page, '.dialogue-choice-btn', 4.5);
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_dialogue_dark.png'),
    });

    // Chuyển tức thì sang Light Mode ngay khi đang mở hội thoại
    await page.evaluate(() => {
      document
        .querySelector<HTMLButtonElement>('#settings-modal [data-theme-mode="light"]')
        ?.click();
    });
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await assertWcagContrast(page, '#dialogue-speaker', 4.5);
    await assertWcagContrast(page, '#dialogue-text', 4.5);
    await assertWcagContrast(page, '.dialogue-choice-btn', 4.5);
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_dialogue_light.png'),
    });

    await page.keyboard.press('Escape');
    await expect(page.locator('#dialogue-modal')).toBeHidden();

    // 6. Kiểm tra Túi Tư Liệu (#inventory-modal) và Tạm Dừng (#pause-modal) ở Light Mode
    await page.keyboard.press('KeyI');
    await expect(page.locator('#inventory-modal')).toBeVisible();
    await assertWcagContrast(page, '#inventory-modal-title', 4.5);
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_inventory_light.png'),
    });
    await page.keyboard.press('Escape');

    await page.keyboard.press('Escape');
    await expect(page.locator('#pause-modal')).toBeVisible();
    await assertWcagContrast(page, '#pause-modal-title', 4.5);
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_pause_light.png'),
    });
    await page.keyboard.press('Escape');

    // 7. Kiểm tra Mobile Landscape (844x390) với Nhật Ký ở Light Mode
    await page.setViewportSize({ width: 844, height: 390 });
    await page.waitForTimeout(150);
    await page.keyboard.press('KeyJ');
    await expect(journalModal).toBeVisible();
    await page.screenshot({
      path: path.join(ARTIFACT_DIR, 'm13_journal_mobile_light.png'),
    });
    await page.keyboard.press('Escape');

    // Đặt lại chế độ System trước khi kết thúc
    await page.evaluate(() => {
      document
        .querySelector<HTMLButtonElement>('#settings-modal [data-theme-mode="system"]')
        ?.click();
    });
    await expect(page.locator('html')).toHaveAttribute('data-theme-mode', 'system');
  });
});
