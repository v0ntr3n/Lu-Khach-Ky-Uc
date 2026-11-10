/**
 * @file tests/playwright/helpers.ts
 * @description Các hàm tiện ích dùng chung cho bộ kiểm thử Playwright E2E (M0 – M7)
 * của "Lữ Khách Ký Ức (1930 – 2045)" trên kiến trúc Fullscreen RPG Shell.
 */

import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, type Page } from '@playwright/test';

// Portable screenshot directory for local runs and GitHub Actions.
// Do not put this inside Playwright's test-results outputDir, which is reset on start.
export const ARTIFACT_DIR = join(process.cwd(), 'artifacts', 'playwright');
mkdirSync(ARTIFACT_DIR, { recursive: true });

/**
 * Xóa sạch localStorage và nạp lại ứng dụng vào trạng thái khởi động sạch tại #main-menu-screen.
 */
export async function bootFreshMainMenu(page: Page, url = '/'): Promise<void> {
  await page.goto(url);
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator('#main-menu-screen')).toBeVisible();
}

/**
 * Từ #main-menu-screen đang mở, bấm BẮT ĐẦU (#btn-menu-start), chờ qua màn nạp dữ liệu,
 * bấm Bỏ qua Mở màn (#skip-opening-btn nếu hiển thị) và xác nhận game vào chế độ 'exploring'.
 */
export async function startFromMainMenuIntoExploring(page: Page): Promise<void> {
  await expect(page.locator('#main-menu-screen')).toBeVisible();
  await page.locator('#btn-menu-start').click();

  await expect(page.locator('#loading-screen')).toBeHidden({ timeout: 10000 });

  const skipBtn = page.locator('#skip-opening-btn');
  const overlay = page.locator('#rift-opening-overlay');

  // Nếu không bật Reduced Motion, #rift-opening-overlay sẽ mở kèm nút #skip-opening-btn
  const isOverlayOpen = await page.evaluate(() => {
    const el = document.getElementById('rift-opening-overlay');
    const reduced = document.body.classList.contains('reduced-motion');
    return !reduced && Boolean(el && !el.hidden);
  });

  if (isOverlayOpen) {
    await expect(skipBtn).toBeVisible();
    await skipBtn.click();
  } else if (await skipBtn.isVisible().catch(() => false)) {
    await skipBtn.click();
  }

  await expect(overlay).toBeHidden();
  await expect(page.locator('#hud-layer')).toBeVisible();
}

/**
 * Khởi động sạch từ đầu (xóa localStorage -> Main Menu -> BẮT ĐẦU -> Bỏ qua Mở màn -> exploring).
 */
export async function startNewExploringSession(page: Page, url = '/'): Promise<void> {
  await bootFreshMainMenu(page, url);
  await startFromMainMenuIntoExploring(page);
}

/**
 * Tiếp tục hồ sơ đã lưu từ #main-menu-screen (#btn-menu-continue) và bỏ qua hoạt cảnh mở màn nếu hiển thị.
 */
export async function continueSavedSession(page: Page): Promise<void> {
  await expect(page.locator('#main-menu-screen')).toBeVisible();
  const continueBtn = page.locator('#btn-menu-continue');
  await expect(continueBtn).toBeEnabled();
  await continueBtn.click();

  await expect(page.locator('#loading-screen')).toBeHidden({ timeout: 10000 });

  const skipBtn = page.locator('#skip-opening-btn');
  if (await skipBtn.isVisible().catch(() => false)) {
    await skipBtn.click();
  }

  await expect(page.locator('#rift-opening-overlay')).toBeHidden();
  await expect(page.locator('#hud-layer')).toBeVisible();
}

/**
 * Mở khóa và bật Ngăn điều khiển Giảng viên / Kiểm thử (#presenter-drawer) bằng tổ hợp phím Ctrl+Shift+P.
 */
export async function openPresenterDrawer(page: Page): Promise<void> {
  const drawer = page.locator('#presenter-drawer');
  if (!(await drawer.isVisible().catch(() => false))) {
    await page.keyboard.press('Control+Shift+KeyP');
  }
  await expect(drawer).toBeVisible();
}
