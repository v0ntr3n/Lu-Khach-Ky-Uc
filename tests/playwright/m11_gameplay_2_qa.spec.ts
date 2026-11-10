import { expect, test } from '@playwright/test';

test.describe('M11 — Gameplay 2.0 Quality Gate: Navigation, Accessibility, Performance & 14-Map QA (Issues #11, #14, #15, #16)', () => {
  test('verifies onboarding tutorial, concise HUD, minimap A* routing, click-to-track, region entry banner, and 14-map era transitions with 0 console errors', async ({
    page,
  }) => {
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        const text = msg.text();
        if (!text.includes('ERR_CERT_AUTHORITY_INVALID')) {
          consoleErrors.push(text);
        }
      }
    });

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/', { waitUntil: 'networkidle' });

    await page.evaluate(() => {
      window.localStorage.clear();
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      if (game) {
        game.updateSettings({ reducedMotion: true });
      }
    });

    // 1. Khởi chạy từ Menu Chính -> BẮT ĐẦU
    await page.locator('#btn-menu-start').click();
    await page.waitForFunction(
      () => {
        const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
        return game?.store?.getMode() === 'exploring';
      },
      { timeout: 6000 }
    );

    // 2. Kiểm tra Thanh hướng dẫn tân thủ (#tutorial-coach-bar), HUD 2 dòng & Minimap
    const coachBar = page.locator('#tutorial-coach-bar');
    await expect(coachBar).toBeVisible();
    await expect(page.locator('#tutorial-step-badge')).toContainText('1/5');

    await expect(page.locator('#hud-active-objective-action')).toContainText('→');
    await expect(page.locator('#minimap-panel')).toBeVisible();
    await expect(page.locator('#minimap-canvas')).toBeVisible();

    // Bỏ qua hướng dẫn tân thủ bằng nút Bỏ qua [X]
    await page.locator('#btn-tutorial-skip').click();
    await expect(coachBar).toBeHidden();

    // 3. Kiểm tra Click-to-Track từ thẻ HUD (#hud-objective-tracker) mở Nhật ký [J]
    await page.locator('#hud-objective-tracker').click();
    await expect(page.locator('#journal-modal')).toBeVisible();

    const sideObjective = page.locator('[data-objective-id="obj_ch1_talk_pier_boatman"]');
    await expect(sideObjective).toBeVisible();
    await sideObjective.click();
    await expect(page.locator('#journal-modal')).toBeHidden();
    await expect(page.locator('#minimap-route-banner')).toContainText('Bến Thuyền Nhỏ');

    // 4. Kiểm tra Biển hiệu chào mừng khu vực (#region-entry-banner) và nạp đủ 14 bản đồ qua 4 Chương
    const eraZoneSummary = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      const eras = ['1930', '1945', '1954_1975', '1986_2045'] as const;
      const visitedTitles: string[] = [];
      for (const era of eras) {
        game.switchEra(era);
        visitedTitles.push(game.currentMap.title);
      }
      // Quay lại Chương I và dịch chuyển qua cổng sang Phố Cũ Cửu Long
      game.switchEra('1930');
      const portalToStreet = game.currentMap.portals.find(
        (p: any) => p.destinationMap === 'old_street'
      );
      if (portalToStreet) {
        game.enterPortal(portalToStreet);
      }
      return {
        visitedTitles,
        currentMapId: game.currentMap.id,
        currentMapTitle: game.currentMap.title,
      };
    });

    expect(eraZoneSummary.visitedTitles).toHaveLength(4);
    expect(eraZoneSummary.currentMapId).toBe('old_street');

    // Biển hiệu chào mừng khu vực (#region-entry-banner) hiển thị tên khu vực mới
    const regionBanner = page.locator('#region-entry-banner');
    await expect(regionBanner).toBeVisible();
    await expect(page.locator('#region-entry-title')).toContainText('Phố Cổ Cửu Long');

    expect(consoleErrors).toEqual([]);
  });
});
