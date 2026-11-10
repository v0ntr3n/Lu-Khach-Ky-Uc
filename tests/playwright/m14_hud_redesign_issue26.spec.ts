/**
 * @file tests/playwright/m14_hud_redesign_issue26.spec.ts
 * @description Kiểm thử tích hợp E2E & Trực quan cho Issue #26 (M14):
 * "UI/UX: HUD nhiệm vụ 1 dòng, tên vị trí top-center và intro ẢI"
 * - Kiểm tra HUD nhiệm vụ 1 dòng ngắn gọn bên dưới Minimap (không lặp tiêu đề nhiệm vụ/mô tả dài/gợi ý).
 * - Kiểm tra hậu tố `[E]` trên HUD nhiệm vụ đồng bộ 100% theo khoảng cách tương tác thực tế:
 *   + Khi ở xa NPC: `Gặp Giao Liên · Bến Cảng` (không có `[E]`)
 *   + Khi tiến vào vùng tương tác: `Gặp Giao Liên · Bến Cảng [E]`
 *   + Khi mở hộp thoại / rời vùng tương tác: `[E]` biến mất ngay lập tức.
 * - Kiểm tra tên vị trí top-center (`#stage-location-badge`) không khung viền/nền hộp, căn giữa phía trên màn hình, tự động cập nhật khi đổi khu vực và không đè lên Minimap hay Stage Intro.
 * - Kiểm tra Stage Intro (`ẢI 1 : Hương Cảng 1930` .. `ẢI 4 : Đổi Mới – Kỷ Nguyên Số 2045`) tự động fade-in -> giữ -> fade-out, không phát lại khi đổi Light/Dark mode hoặc chuyển khu vực phụ trong cùng Ải.
 */

import { test, expect, type Page } from '@playwright/test';

const ARTIFACT_DIR =
  '/home/antigravity/.gemini/jetski/brain/v1/default/jetski_remote_agents_3p/9794bb53-67ce-4471-a58e-ea7055d0c4ce';

async function startNewGameToPlaying(page: Page): Promise<void> {
  await page.goto('/', { waitUntil: 'networkidle' });
  await page.evaluate(() => localStorage.clear());
  await page.reload({ waitUntil: 'networkidle' });

  await page.locator('#btn-menu-start').click();
  const skipBtn = page.locator('#skip-opening-btn');
  await expect(skipBtn).toBeVisible({ timeout: 5000 });
  await skipBtn.click();

  await expect(page.locator('#hud-layer')).toBeVisible();
}

test.describe('M14 (Issue #26): 1-Line Quest HUD, Top-Center Location Label & Stage Intro', () => {
  test('M14-E2E-01: Stage Intro ẢI 1..4 lifecycle, non-overlap with top-center location label, and no replay on theme toggle or sub-zone transition', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await startNewGameToPlaying(page);

    const banner = page.locator('#region-entry-banner');
    const bannerTitle = page.locator('#region-entry-title');
    const locationBadge = page.locator('#stage-location-badge');

    // Ngay khi vừa vào Ải 1, Stage Intro hiển thị "ẢI 1 : Hương Cảng 1930"
    await expect(banner).toBeVisible();
    await expect(bannerTitle).toHaveText('ẢI 1 : Hương Cảng 1930');

    // Trong lúc Stage Intro đang hiển thị, .hud-location-plaque tạm ẩn (opacity = 0) để không đè chữ ở top-center
    const introOverlapCheck = await page.evaluate(() => {
      const hudLayer = document.getElementById('hud-layer')!;
      const plaque = document.querySelector('.hud-location-plaque') as HTMLElement;
      const bannerEl = document.getElementById('region-entry-banner')!;
      const bannerStyle = window.getComputedStyle(bannerEl);
      return {
        isIntroActiveClass: hudLayer.classList.contains('is-stage-intro-active'),
        plaqueOpacity: Number.parseFloat(window.getComputedStyle(plaque).opacity),
        bannerBg: bannerStyle.backgroundColor,
        bannerBorder: bannerStyle.borderTopWidth,
      };
    });
    expect(introOverlapCheck.isIntroActiveClass).toBe(true);
    expect(introOverlapCheck.plaqueOpacity).toBeLessThanOrEqual(0.05);
    expect(introOverlapCheck.bannerBg).toBe('rgba(0, 0, 0, 0)');
    expect(introOverlapCheck.bannerBorder).toBe('0px');

    // Chụp ảnh minh chứng Stage Intro ở Dark Mode (sau khi hoàn tất fade-in 240ms)
    await page.waitForTimeout(260);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m14_stage_intro_dark.png`,
      fullPage: false,
    });

    // Chờ Stage Intro tự động fade-out hoàn tất (~2.2s)
    await expect(banner).toBeHidden({ timeout: 4500 });

    // Sau khi Stage Intro kết thúc, tên vị trí top-center hiện rõ ràng (opacity = 1) và không có khung nền
    await expect(locationBadge).toBeVisible();
    await expect(locationBadge).toHaveText('Quảng Trường Bến Cảng Hương Cảng (1930)');

    const topCenterCheck = await page.evaluate(() => {
      const plaque = document.querySelector('.hud-location-plaque') as HTMLElement;
      const badge = document.getElementById('stage-location-badge')!;
      const minimap = document.getElementById('minimap-panel')!;
      const plaqueStyle = window.getComputedStyle(plaque);
      const badgeStyle = window.getComputedStyle(badge);
      const badgeRect = badge.getBoundingClientRect();
      const minimapRect = minimap.getBoundingClientRect();
      const viewportCenterX = window.innerWidth / 2;
      const badgeCenterX = badgeRect.left + badgeRect.width / 2;

      return {
        plaqueOpacity: Number.parseFloat(plaqueStyle.opacity),
        plaqueBg: plaqueStyle.backgroundColor,
        plaqueBorder: plaqueStyle.borderTopWidth,
        badgeBg: badgeStyle.backgroundColor,
        badgeBorder: badgeStyle.borderTopWidth,
        centerOffsetPx: Math.abs(badgeCenterX - viewportCenterX),
        topPx: badgeRect.top,
        overlapsMinimap:
          badgeRect.right > minimapRect.left && badgeRect.bottom > minimapRect.top,
      };
    });

    expect(topCenterCheck.plaqueOpacity).toBe(1);
    expect(topCenterCheck.plaqueBg).toBe('rgba(0, 0, 0, 0)');
    expect(topCenterCheck.plaqueBorder).toBe('0px');
    expect(topCenterCheck.badgeBg).toBe('rgba(0, 0, 0, 0)');
    expect(topCenterCheck.badgeBorder).toBe('0px');
    expect(topCenterCheck.centerOffsetPx).toBeLessThan(20);
    expect(topCenterCheck.topPx).toBeLessThan(60);
    expect(topCenterCheck.overlapsMinimap).toBe(false);

    // Đổi theme sang Light Mode và ngược lại -> Stage Intro KHÔNG được phát lại
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      game.updateSettings({ themeMode: 'light' });
      game.syncHudWithState();
    });
    await expect(banner).toBeHidden();

    // Chuyển qua cổng sang khu vực phụ trong cùng Ải 1 (old_street) -> cập nhật tên vị trí top-center, KHÔNG phát lại Stage Intro
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      const portalToStreet = game.currentMap.portals.find(
        (p: any) => p.destinationMap === 'old_street'
      );
      game.enterPortal(portalToStreet);
    });
    await expect(locationBadge).toContainText('Phố Cổ Cửu Long');
    await expect(banner).toBeHidden();

    // Chuyển sang Ải 2 ('1945') -> Stage Intro hiển thị "ẢI 2 : Tân Trào – Ba Đình 1945"
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      game.switchEra('1945');
    });
    await expect(banner).toBeVisible();
    await expect(bannerTitle).toHaveText('ẢI 2 : Tân Trào – Ba Đình 1945');
    await expect(locationBadge).toContainText('Đình Tân Trào & Cây Đa Lịch Sử (08/1945)');
  });

  test('M14-E2E-02: 1-Line Quest HUD & real-time [E] proximity suffix in Dark & Light Mode', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await startNewGameToPlaying(page);

    // Ẩn thanh hướng dẫn tân thủ để kiểm tra HUD thuần khiết và chờ Stage Intro kết thúc
    await page.locator('#btn-tutorial-skip').click();
    await expect(page.locator('#region-entry-banner')).toBeHidden({ timeout: 4500 });

    const objTracker = page.locator('#hud-objective-tracker');
    const objText = page.locator('#hud-active-objective-text');
    const questTitle = page.locator('#hud-active-quest-title');
    const objAction = page.locator('#hud-active-objective-action');
    const objHint = page.locator('#hud-active-objective-hint');
    const worldPrompt = page.locator('#world-interaction-prompt');

    // 1. Khi người chơi đứng ở điểm xuất phát (xa NPC Giao Liên):
    // - Chỉ hiển thị duy nhất 1 dòng nhiệm vụ ngắn gọn, KHÔNG có [E]
    await expect(objTracker).toBeVisible();
    await expect(objText).toBeVisible();
    await expect(objText).toHaveText('Gặp Giao Liên · Bến Cảng');
    await expect(questTitle).toBeHidden();
    await expect(objAction).toBeHidden();
    await expect(objHint).toBeHidden();
    await expect(worldPrompt).toBeHidden();

    // Kiểm tra #hud-objective-tracker gọn đúng 1 dòng (white-space: nowrap) và nằm ngay dưới #minimap-panel
    const trackerMetrics = await page.evaluate(() => {
      const tracker = document.getElementById('hud-objective-tracker')!;
      const textEl = document.getElementById('hud-active-objective-text')!;
      const minimap = document.getElementById('minimap-panel')!;
      const trackerRect = tracker.getBoundingClientRect();
      const minimapRect = minimap.getBoundingClientRect();
      const textStyle = window.getComputedStyle(textEl);
      return {
        whiteSpace: textStyle.whiteSpace,
        trackerHeight: Math.round(trackerRect.height),
        isBelowMinimap: trackerRect.top >= minimapRect.bottom - 2,
      };
    });
    expect(trackerMetrics.whiteSpace).toBe('nowrap');
    expect(trackerMetrics.trackerHeight).toBeLessThanOrEqual(44);
    expect(trackerMetrics.isBelowMinimap).toBe(true);

    // Chụp ảnh HUD ở Dark Mode (ngoài vùng tương tác)
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m14_hud_dark.png`,
      fullPage: false,
    });

    // 2. Di chuyển nhân vật vào đúng tầm tương tác với Giao Liên Lâm Văn Bính (256, 368):
    // - Hậu tố ' [E]' xuất hiện ngay lập tức trên dòng HUD nhiệm vụ, đồng bộ với #world-interaction-prompt
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      game.playerX = 256;
      game.playerY = 384;
      game.syncHudWithState();
    });

    await expect(worldPrompt).toBeVisible();
    await expect(objText).toHaveText('Gặp Giao Liên · Bến Cảng [E]');

    // Chuyển sang Light Mode và chụp ảnh minh chứng HUD có [E] + tên vị trí top-center ở Light Mode
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      game.updateSettings({ themeMode: 'light' });
      game.syncHudWithState();
    });
    await page.waitForTimeout(100);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m14_hud_interact_e_light.png`,
      fullPage: false,
    });

    // 3. Nhấn [E] mở hội thoại -> Mục tiêu chuyển sang bước kế tiếp, đồng thời khi modal mở tương tác thế giới bị khóa nên '[E]' trên HUD tự ẩn đi
    await page.keyboard.press('KeyE');
    await expect(page.locator('#dialogue-modal')).toBeVisible();
    await expect(worldPrompt).toBeHidden();
    await expect(objText).toHaveText('Kiểm tra sổ vận đơn · Kho 4');

    // Đóng hội thoại -> Nhân vật vẫn đứng cạnh NPC nên '[E]' hiện lại
    await page.locator('#btn-close-dialogue').click();
    await expect(page.locator('#dialogue-modal')).toBeHidden();
    await expect(worldPrompt).toBeVisible();
    await expect(objText).toHaveText('Kiểm tra sổ vận đơn · Kho 4 [E]');

    // 4. Bước ra xa NPC -> '[E]' biến mất ngay lập tức
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      game.playerX = 120;
      game.playerY = 220;
      game.syncHudWithState();
    });
    await expect(worldPrompt).toBeHidden();
    await expect(objText).toHaveText('Kiểm tra sổ vận đơn · Kho 4');

    await page.screenshot({
      path: `${ARTIFACT_DIR}/m14_hud_light.png`,
      fullPage: false,
    });

    // 5. Kiểm tra kích hoạt bằng bàn phím (Enter) trên #hud-objective-tracker để mở Sổ Tay [J]
    await objTracker.focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('#journal-modal')).toBeVisible();
  });
});
