import { expect, test } from '@playwright/test';

test.describe('M8 — Interactive Minimap, Click-to-Track Quests & Obstacle-Aware Routing (Issue #13)', () => {
  test('renders real-time minimap, toggles with [N], opens World Map on click, and switches tracked quest from Journal', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/', { waitUntil: 'networkidle' });

    await page.evaluate(() => {
      window.localStorage.clear();
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      if (game) {
        game.updateSettings({ reducedMotion: true });
      }
    });

    await page.locator('#btn-menu-start').click();
    await page.waitForFunction(
      () => {
        const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
        return game?.store?.getMode() === 'exploring';
      },
      { timeout: 6000 }
    );
    await expect(page.locator('#hud-top-right')).toBeVisible({ timeout: 5000 });

    // 1. Minimap hiển thị ở góc trên phải ngay trên thẻ nhiệm vụ
    const minimapPanel = page.locator('#minimap-panel');
    const minimapCanvas = page.locator('#minimap-canvas');
    await expect(minimapPanel).toBeVisible();
    await expect(minimapCanvas).toBeVisible();
    await expect(page.locator('#minimap-zone-title')).toContainText('Quảng Trường Bến Cảng');

    // Kiểm tra dữ liệu tuyến đường A* ban đầu (mục tiêu chính Cột mốc I tại harbor_square)
    const initialRoute = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      const route = game.getTrackedRouteState();
      return {
        objectiveId: route.objectiveId,
        reachable: route.path.reachable,
        pointCount: route.path.points.length,
        isCrossZone: route.isCrossZone,
      };
    });
    expect(initialRoute.objectiveId).toBe('obj_ch1_meet_liaison');
    expect(initialRoute.reachable).toBe(true);
    expect(initialRoute.pointCount).toBeGreaterThanOrEqual(2);
    expect(initialRoute.isCrossZone).toBe(false);

    // 2. Bấm phím [N] để thu gọn / mở lại Bản đồ thu nhỏ
    await page.keyboard.press('n');
    await expect(minimapPanel).toHaveClass(/is-collapsed/);
    await page.keyboard.press('n');
    await expect(minimapPanel).not.toHaveClass(/is-collapsed/);

    // 3. Nhấp vào khung Minimap mở Bản đồ Thế giới (#world-map-modal), sau đó đóng bằng Escape
    await page.locator('#btn-minimap-open-map').click();
    await expect(page.locator('#world-map-modal')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('#world-map-modal')).toBeHidden();

    // 4. Mở Sổ Tay [J] -> Nhấp vào mục tiêu phụ "Hỏi thăm Lão Chèo Đò tại Bến Thuyền Nhỏ" -> Tự đóng Sổ Tay & dẫn đường liên khu vực
    await page.keyboard.press('j');
    await expect(page.locator('#journal-modal')).toBeVisible();

    const sideObjectiveRow = page.locator(
      '[data-objective-id="obj_ch1_talk_pier_boatman"]'
    );
    await expect(sideObjectiveRow).toBeVisible();
    await sideObjectiveRow.click();

    // Sổ tay tự đóng ngay sau khi chọn mục tiêu dẫn đường
    await expect(page.locator('#journal-modal')).toBeHidden();

    // HUD & Minimap cập nhật sang mục tiêu phụ liên khu vực (Bến Thuyền Nhỏ)
    await expect(page.locator('#hud-active-objective-text')).toContainText(
      'Hỏi thăm Lão Chèo Đò tại Bến Thuyền Nhỏ'
    );
    await expect(page.locator('#minimap-route-banner')).toContainText('Bến Thuyền Nhỏ');

    const crossZoneRoute = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: any }).__LU_KHACH_GAME__;
      const route = game.getTrackedRouteState();
      return {
        trackedId: game.store.trackedObjectiveId,
        objectiveId: route.objectiveId,
        reachable: route.path.reachable,
        pointCount: route.path.points.length,
        isCrossZone: route.isCrossZone,
        nextPortalZoneId: route.nextPortalZoneId,
      };
    });
    expect(crossZoneRoute.trackedId).toBe('obj_ch1_talk_pier_boatman');
    expect(crossZoneRoute.objectiveId).toBe('obj_ch1_talk_pier_boatman');
    expect(crossZoneRoute.reachable).toBe(true);
    expect(crossZoneRoute.pointCount).toBeGreaterThanOrEqual(2);
    expect(crossZoneRoute.isCrossZone).toBe(true);
    expect(crossZoneRoute.nextPortalZoneId).toBe('small_pier');
  });
});
