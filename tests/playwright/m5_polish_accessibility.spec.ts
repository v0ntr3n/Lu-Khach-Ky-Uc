/**
 * @file tests/playwright/m5_polish_accessibility.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M5 — Art, Gameplay Polish & Accessibility:
 * - M5-E2E-01: Trợ năng bàn phím 100% (phím [H] bật/tắt gợi ý nhiệm vụ trên HUD, phím số [1..3] chọn nhanh nhánh hội thoại
 *              không cần chuột, kiểm tra image-rendering: pixelated trên #pixel-canvas).
 * - M5-E2E-02: Chế độ Giảm chuyển động (Reduced Motion) & Cỡ chữ lớn (Large Text): kiểm tra class body.reduced-motion,
 *              body.text-scale-large và xác nhận game.transitionFadeAlpha === 0 khi qua cổng lúc bật Reduced Motion.
 * - M5-E2E-03: Kiểm định hiển thị đa khung hình Desktop (1280x800) & Mobile Landscape (844x390), không tràn ngang
 *              (scrollWidth <= viewportWidth), và chụp ảnh thực tế m5_desktop_polished.png & m5_mobile_landscape.png.
 */

import { expect, test } from '@playwright/test';
import { ARTIFACT_DIR, startNewExploringSession } from './helpers';

test.describe('M5 Art, Gameplay Polish & Accessibility — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await startNewExploringSession(page);
  });

  test('M5-E2E-01: Keyboard-Only Accessibility ([H] Hint Toggle, [1..3] Dialogue Choice Selection, Pixel-Perfect Canvas)', async ({
    page,
  }) => {
    // 1. Xác nhận #pixel-canvas cưỡng chế image-rendering: pixelated / crisp-edges
    const imageRendering = await page.evaluate(() => {
      const canvas = document.getElementById('pixel-canvas');
      return canvas ? window.getComputedStyle(canvas).imageRendering : '';
    });
    expect(['pixelated', 'crisp-edges']).toContain(imageRendering);

    // 2. Nhấn phím [H] để tắt/bật gợi ý nhiệm vụ (game.store.settings.showHints; trên HUD 1 dòng #hud-active-objective-hint luôn ẩn)
    const hintEl = page.locator('#hud-active-objective-hint');
    await expect(hintEl).toBeHidden();

    await page.keyboard.press('KeyH');
    await expect(hintEl).toBeHidden();
    const showHintsAfterOff = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { settings: { showHints: boolean } } };
        }
      ).__LU_KHACH_GAME__!;
      return game.store.settings.showHints;
    });
    expect(showHintsAfterOff).toBe(false);

    await page.keyboard.press('KeyH');
    await expect(hintEl).toBeHidden();
    const showHintsAfterOn = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { settings: { showHints: boolean } } };
        }
      ).__LU_KHACH_GAME__!;
      return game.store.settings.showHints;
    });
    expect(showHintsAfterOn).toBe(true);

    // 3. Tiếp cận Giao Liên Lâm Văn Bính, nhấn [E] mở hội thoại và dùng phím số [1] (Digit1) để chọn nhánh đầu tiên
    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = 256;
      game.playerY = 384;
      game.syncHudWithState();
    });

    await page.keyboard.press('KeyE');
    await expect(page.locator('.dialogue-choice-btn').first()).toBeVisible();

    // Bấm phím số 1 (Digit1) trên bàn phím thay vì click chuột
    await page.keyboard.press('Digit1');

    await expect(page.locator('#codex-progress-count')).toHaveText('1 / 12 CỘT MỐC');
    await expect(page.locator('#hud-active-objective-text')).toContainText(
      'Kiểm tra sổ vận đơn · Kho 4'
    );
  });

  test('M5-E2E-02: Reduced Motion & Large Text Accessibility Modes', async ({
    page,
  }) => {
    // Mở bảng Tạm Dừng bằng phím Tab, sau đó mở Cài đặt hệ thống (#settings-modal)
    await page.keyboard.press('Tab');
    await expect(page.locator('#pause-modal')).toBeVisible();

    await page.locator('#pause-open-settings').click();
    await expect(page.locator('#settings-modal')).toBeVisible();

    await page.locator('#toggle-reduced-motion').click();
    await page.locator('#toggle-text-scale').click();

    await expect(page.locator('body')).toHaveClass(/reduced-motion/);
    await expect(page.locator('body')).toHaveClass(/text-scale-large/);

    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-modal')).toBeHidden();
    await expect(page.locator('#pause-modal')).toBeHidden();

    // Khi bật Reduced Motion, bước qua portal sang warehouse_interior giữ transitionFadeAlpha === 0
    const transitionCheck = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            transitionFadeAlpha: number;
            sceneTransition: {
              cooldownRemainingSec: number;
              requiresStepOffPortal: boolean;
            };
            currentMap: { portals: Array<{ destinationMap: string }> };
            enterPortal(portal: unknown): boolean;
            getCurrentMapId(): string;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.sceneTransition.cooldownRemainingSec = 0;
      game.sceneTransition.requiresStepOffPortal = false;
      const portal = game.currentMap.portals.find(
        (p) => p.destinationMap === 'warehouse_interior'
      )!;
      const ok = game.enterPortal(portal);
      return {
        ok,
        mapId: game.getCurrentMapId(),
        fadeAlpha: game.transitionFadeAlpha,
      };
    });

    expect(transitionCheck.ok).toBe(true);
    expect(transitionCheck.mapId).toBe('warehouse_interior');
    expect(transitionCheck.fadeAlpha).toBe(0);
  });

  test('M5-E2E-03: Responsive Desktop (1280x800) & Mobile Landscape (844x390) Audit + Screenshots', async ({
    page,
  }) => {
    // 1. Kiểm tra ở độ phân giải Desktop tiêu chuẩn (1280x800)
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.waitForTimeout(150);

    await expect(page.locator('#pixel-canvas')).toBeVisible();
    const desktopMetrics = await page.evaluate(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    }));
    expect(desktopMetrics.scrollWidth).toBeLessThanOrEqual(desktopMetrics.innerWidth);

    await page.screenshot({
      path: `${ARTIFACT_DIR}/m5_desktop_polished.png`,
      fullPage: false,
    });

    // 2. Chuyển sang khung hình Mobile Landscape (844x390)
    await page.setViewportSize({ width: 844, height: 390 });
    await page.waitForTimeout(150);

    await expect(page.locator('#pixel-canvas')).toBeVisible();
    await expect(page.locator('.controls-bar')).toBeVisible();

    const mobileMetrics = await page.evaluate(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    }));
    expect(mobileMetrics.scrollWidth).toBeLessThanOrEqual(mobileMetrics.innerWidth);

    await page.screenshot({
      path: `${ARTIFACT_DIR}/m5_mobile_landscape.png`,
      fullPage: false,
    });
  });
});
