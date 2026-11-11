import { expect, test } from '@playwright/test';

const BASE_URL = process.env.PLAYWRIGHT_BASE_URL || '/';

test.describe('M7 (#12) — Gameplay Clarity: First-Play Tutorial, Concise HUD & Exact Quest Targets', () => {
  test('starts non-blocking 5-step tutorial on new game, advances via player actions, supports skip/replay, and displays concise next action on HUD', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto(BASE_URL, { waitUntil: 'networkidle' });

    await page.evaluate(() => {
      window.localStorage.clear();
    });
    await page.reload({ waitUntil: 'networkidle' });

    // Start New Game from Main Menu
    await page.locator('#btn-menu-start').click();
    const skipOpeningBtn = page.locator('#skip-opening-btn');
    await expect(skipOpeningBtn).toBeVisible({ timeout: 5000 });
    await skipOpeningBtn.click();

    // Verify PLAYING state and non-blocking tutorial coach bar is visible
    await expect(page.locator('#tutorial-coach-bar')).toBeVisible();
    await expect(page.locator('#tutorial-step-badge')).toHaveText('1/5');
    await expect(page.locator('#tutorial-step-title')).toHaveText(
      'Di chuyển Lữ Khách'
    );

    // Verify concise 1-line HUD objective tracker shows exact objective line
    const objectiveLine = page.locator('#hud-active-objective-text');
    await expect(objectiveLine).toBeVisible();
    await expect(objectiveLine).toContainText(
      'Gặp Giao Liên · Bến Cảng'
    );

    // Verify exact target resolution for initial objective points to npc_courier
    const targetRes = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__: any }).__LU_KHACH_GAME__;
      return game.getActiveObjectiveResolution();
    });
    expect(targetRes.status).toBe('resolved');
    expect(targetRes.targetEntityId).toBe('npc_courier');
    expect(targetRes.position?.targetId).toBe('npc_courier');
    expect(targetRes.position?.isPortal).toBe(false);

    // Step 1 -> Step 2: Move player at least 24px (world input is NOT blocked by tutorial!)
    await page.keyboard.down('d');
    await page.waitForTimeout(400);
    await page.keyboard.up('d');
    await expect(page.locator('#tutorial-step-badge')).toHaveText('2/5');
    await expect(page.locator('#tutorial-step-title')).toHaveText(
      'Trò chuyện & Tương tác'
    );

    // Step 2 -> Step 3: Interact near npc_courier with [E]
    await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__: any }).__LU_KHACH_GAME__;
      game.playerX = 246;
      game.playerY = 368;
      game.handleInteractAction();
    });
    // Close dialogue modal so tutorial bar unsuppresses
    await page.locator('#btn-close-dialogue').click();
    await expect(page.locator('#tutorial-step-badge')).toHaveText('3/5');
    await expect(page.locator('#tutorial-step-title')).toHaveText(
      'Nhật ký & Biên niên sử'
    );

    // Step 3 -> Step 4: Open & close Journal [J]
    await page.keyboard.press('j');
    await expect(page.locator('#journal-modal')).toBeVisible();
    await page.locator('#btn-close-journal').click();
    await expect(page.locator('#tutorial-step-badge')).toHaveText('4/5');

    // Step 4 -> Step 5: Open & close World Map [M]
    await page.keyboard.press('m');
    await expect(page.locator('#world-map-modal')).toBeVisible();
    await page.locator('#btn-close-world-map').click();
    await expect(page.locator('#tutorial-step-badge')).toHaveText('5/5');

    // Step 5 -> Completed: Toggle Hints [H]
    await page.keyboard.press('h');
    await expect(page.locator('#tutorial-coach-bar')).toBeHidden();

    // Verify tutorial can be replayed from Settings modal and skipped with keyboard [X]
    await page.locator('#btn-open-pause').click();
    await page.locator('#pause-open-settings').click();
    await expect(page.locator('#settings-modal')).toBeVisible();
    await page.locator('#btn-replay-tutorial').click();

    await expect(page.locator('#tutorial-coach-bar')).toBeVisible();
    await expect(page.locator('#tutorial-step-badge')).toHaveText('1/5');

    await page.keyboard.press('x');
    await expect(page.locator('#tutorial-coach-bar')).toBeHidden();
  });
});
