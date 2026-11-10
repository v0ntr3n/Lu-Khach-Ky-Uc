/**
 * @file tests/playwright/m0_stabilization.spec.ts
 * @description Kiểm thử End-to-End (Playwright) trên trình duyệt thực cho Cột mốc M0 (Gameplay Stabilization):
 * 1. Bắt đầu từ Main Menu -> Bỏ qua hoạt cảnh mở màn (#skip-opening-btn / Escape) ẩn #rift-opening-overlay (giữ DOM node để tái sử dụng) và chuyển game sang trạng thái 'exploring'.
 * 2. Bấm vào cột mốc trong #codex-milestone-list (trong ngăn Biên Niên Sử của #journal-modal) ở chế độ thường KHÔNG tăng #codex-progress-count (0 / 12 CỘT MỐC) và KHÔNG dịch chuyển nhân vật.
 * 3. Mở #lookup-drawer (#btn-open-pause -> #btn-open-lookup) khóa di chuyển thế giới: giữ phím D khi ngăn kéo mở không làm thay đổi playerX.
 * 4. #climax-step-b bị ẩn (toBeHidden / display: none) khi mở #climax-modal và chỉ hiển thị sau khi chọn đúng phương án Bước A.
 * 5. Chế độ Giảng Viên (#btn-lecturer-unlock trong #presenter-drawer) bật chế độ xem cô lập 12 / 12 CỘT MỐC và khi tắt sẽ khôi phục 0 / 12 CỘT MỐC.
 */

import { expect, test } from '@playwright/test';
import {
  bootFreshMainMenu,
  openPresenterDrawer,
  startFromMainMenuIntoExploring,
} from './helpers';

test.describe('M0 Stabilization — Browser E2E Regression Suite', () => {
  test.beforeEach(async ({ page }) => {
    await bootFreshMainMenu(page);
  });

  test('1. Skipping opening (#skip-opening-btn or Escape) transitions game to exploring mode', async ({
    page,
  }) => {
    await page.locator('#btn-menu-start').click();

    const skipBtn = page.locator('#skip-opening-btn');
    await expect(skipBtn).toBeVisible();
    await skipBtn.click();

    // #rift-opening-overlay được ẩn đi (toBeHidden) thay vì xóa khỏi DOM để tái sử dụng qua nhiều phiên chơi
    const overlay = page.locator('#rift-opening-overlay');
    await expect(overlay).toBeHidden();
    await expect(overlay).toHaveCount(1);

    const mode = await page.evaluate(() => {
      const game = (
        window as unknown as { __LU_KHACH_GAME__?: { store: { getMode(): string } } }
      ).__LU_KHACH_GAME__;
      return game?.store.getMode();
    });
    expect(mode).toBe('exploring');
  });

  test('2. Clicking a milestone in #codex-milestone-list in normal mode does NOT increment #codex-progress-count and does NOT teleport player', async ({
    page,
  }) => {
    await startFromMainMenuIntoExploring(page);

    // Mở Nhật ký [J] -> chuyển sang ngăn Biên Niên Sử I–XII (#tab-journal-codex)
    await page.keyboard.press('KeyJ');
    await expect(page.locator('#journal-modal')).toBeVisible();
    await page.locator('#tab-journal-codex').click();

    const progressCount = page.locator('#codex-progress-count');
    await expect(progressCount).toBeVisible();
    await expect(progressCount).toHaveText('0 / 12 CỘT MỐC');

    const initialPlayerX = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: { playerX: number } })
        .__LU_KHACH_GAME__;
      return game?.playerX ?? 85;
    });
    expect(initialPlayerX).toBe(85);

    // Bấm vào cột mốc II trong #codex-milestone-list ở chế độ thường
    const secondMilestoneBtn = page.locator('#codex-milestone-list .codex-item').nth(1);
    await expect(secondMilestoneBtn).toBeVisible();
    await secondMilestoneBtn.click();

    await expect(progressCount).toHaveText('0 / 12 CỘT MỐC');

    const afterClickState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            store: { collectedMilestones: Set<string> };
          };
        }
      ).__LU_KHACH_GAME__;
      return {
        playerX: game?.playerX,
        collectedSize: game?.store.collectedMilestones.size,
      };
    });

    expect(afterClickState.playerX).toBe(initialPlayerX);
    expect(afterClickState.collectedSize).toBe(0);
  });

  test('3. Opening #lookup-drawer (#btn-open-lookup) locks world movement: holding KeyD does not move playerX', async ({
    page,
  }) => {
    await startFromMainMenuIntoExploring(page);

    await page.locator('#btn-open-pause').click();
    await expect(page.locator('#pause-modal')).toBeVisible();
    await page.locator('#btn-open-lookup').click();

    const lookupDrawer = page.locator('#lookup-drawer');
    await expect(lookupDrawer).toBeVisible();
    await expect(lookupDrawer).toHaveClass(/is-open/);

    const xBeforeHold = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { playerX: number; store: { getMode(): string } };
        }
      ).__LU_KHACH_GAME__;
      return { x: game?.playerX ?? 85, mode: game?.store.getMode() };
    });
    expect(xBeforeHold.mode).toBe('lookup');

    // Giữ phím D trong 300ms khi ngăn kéo tra cứu đang mở
    await page.keyboard.down('KeyD');
    await page.waitForTimeout(300);
    await page.keyboard.up('KeyD');

    const xAfterHold = await page.evaluate(() => {
      const game = (window as unknown as { __LU_KHACH_GAME__?: { playerX: number } })
        .__LU_KHACH_GAME__;
      return game?.playerX ?? 85;
    });

    expect(xAfterHold).toBe(xBeforeHold.x);
  });

  test('4. #climax-step-b is hidden (display: none) when #climax-modal opens and only becomes visible after clicking the correct Step A option', async ({
    page,
  }) => {
    await startFromMainMenuIntoExploring(page);

    // Mở khóa 3 mốc I, II, III của Chương I và dịch chuyển tới Cổng Thời Gian để kích hoạt Climax Modal hợp lệ
    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            store: { collectMilestone(id: 'I' | 'II' | 'III'): void };
            handleInteractAction(): void;
          };
        }
      ).__LU_KHACH_GAME__;
      if (!game) return;
      game.store.collectMilestone('I');
      game.store.collectMilestone('II');
      game.store.collectMilestone('III');
      game.playerX = 980;
      game.handleInteractAction();
    });

    const climaxModal = page.locator('#climax-modal');
    const stepB = page.locator('#climax-step-b');

    await expect(climaxModal).toHaveClass(/is-open/);
    await expect(stepB).toBeHidden();

    const stepBDisplayBefore = await stepB.evaluate((el) => getComputedStyle(el).display);
    expect(stepBDisplayBefore).toBe('none');

    // Bấm phương án sai trước (phương án thứ 2) -> Bước B vẫn phải ẩn
    const options = page.locator('#climax-a-options .climax-option-btn');
    await options.nth(1).click();
    await expect(stepB).toBeHidden();

    // Bấm phương án đúng (phương án thứ 1 của Chương I) -> Bước B hiển thị
    await options.nth(0).click();
    await expect(stepB).toBeVisible();

    const stepBDisplayAfter = await stepB.evaluate((el) => getComputedStyle(el).display);
    expect(stepBDisplayAfter).toBe('flex');
  });

  test('5. Presenter Mode (#btn-lecturer-unlock) toggles isolated 12 / 12 CỘT MỐC view and toggling it off restores 0 / 12 CỘT MỐC', async ({
    page,
  }) => {
    await startFromMainMenuIntoExploring(page);

    const progressCount = page.locator('#codex-progress-count');
    await expect(progressCount).toHaveText('0 / 12 CỘT MỐC');

    // Mở Ngăn Giảng Viên qua Ctrl+Shift+P và bật Chế độ Giảng Viên
    await openPresenterDrawer(page);
    const lecturerBtn = page.locator('#btn-lecturer-unlock');
    await expect(lecturerBtn).toBeVisible();
    await lecturerBtn.click();

    await expect(progressCount).toHaveText('12 / 12 CỘT MỐC');

    const stateWhileOn = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: {
              isPresenterMode: boolean;
              collectedMilestones: Set<string>;
              presenterCollectedMilestones: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__;
      return {
        isPresenterMode: game?.store.isPresenterMode,
        realSize: game?.store.collectedMilestones.size,
        presenterSize: game?.store.presenterCollectedMilestones.size,
      };
    });
    expect(stateWhileOn).toEqual({
      isPresenterMode: true,
      realSize: 0,
      presenterSize: 12,
    });

    // Đóng hộp thoại xem trước Cột mốc I (mở tự động khi bật Giảng Viên), mở lại Ngăn Giảng Viên và tắt Chế độ Giảng Viên -> khôi phục 0 / 12 CỘT MỐC
    if (await page.locator('#dialogue-modal').isVisible()) {
      await page.keyboard.press('Escape');
      await expect(page.locator('#dialogue-modal')).toBeHidden();
    }
    await openPresenterDrawer(page);
    await expect(lecturerBtn).toBeVisible();
    await lecturerBtn.click();
    await expect(progressCount).toHaveText('0 / 12 CỘT MỐC');
  });
});
