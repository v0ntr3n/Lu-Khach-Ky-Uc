/**
 * @file tests/playwright/m3_systems_ui.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M3 — RPG Systems & UI:
 * - M3-E2E-01: Kiểm tra bố cục toàn màn hình (#game-shell / .main-stage chiếm 100vw × 100dvh),
 *              thanh theo dõi mục tiêu trực tiếp (#hud-objective-tracker) và #codex-progress-count trong #journal-modal -> #tab-journal-codex.
 * - M3-E2E-02: Hội thoại phân nhánh đa lựa chọn với NPC tại harbor_square, hiển thị #dialogue-provenance-badge,
 *              chọn nút .dialogue-choice-btn, cập nhật cờ nhiệm vụ & mục tiêu trên HUD và chụp ảnh m3_dialogue_choices.png.
 * - M3-E2E-03: Kiểm tra các cửa sổ Nhật Ký (#journal-modal, phím J), Túi Tư Liệu (#inventory-modal, phím I),
 *              Tạm Dừng & Cài Đặt (#pause-modal -> #pause-open-settings -> #settings-modal), khóa điều khiển di chuyển thế giới,
 *              bật/tắt Reduced Motion & Large Text, lưu thủ công (#btn-manual-save) và khôi phục sau khi tải lại trang qua #btn-menu-continue.
 */

import { expect, test } from '@playwright/test';
import {
  ARTIFACT_DIR,
  continueSavedSession,
  startNewExploringSession,
} from './helpers';

test.describe('M3 RPG Systems & UI — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await startNewExploringSession(page);
  });

  test('M3-E2E-01: Fullscreen Viewport Layout & Active Quest HUD Tracker', async ({
    page,
  }) => {
    const gameShell = page.locator('#game-shell');
    const mainStage = page.locator('.main-stage');
    await expect(gameShell).toBeVisible();
    await expect(mainStage).toBeVisible();
    await expect(page.locator('#hud-objective-tracker')).toBeVisible();

    const layoutMetrics = await page.evaluate(() => {
      const shellEl = document.getElementById('game-shell');
      const stageEl = document.querySelector('.main-stage') as HTMLElement | null;
      const shellRect = shellEl?.getBoundingClientRect();
      const stageRect = stageEl?.getBoundingClientRect();
      return {
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
        shellWidth: Math.round(shellRect?.width ?? 0),
        shellHeight: Math.round(shellRect?.height ?? 0),
        stageWidth: Math.round(stageRect?.width ?? 0),
        stageHeight: Math.round(stageRect?.height ?? 0),
      };
    });

    expect(layoutMetrics.shellWidth).toBe(layoutMetrics.viewportWidth);
    expect(layoutMetrics.shellHeight).toBe(layoutMetrics.viewportHeight);
    expect(layoutMetrics.stageWidth).toBe(layoutMetrics.viewportWidth);
    expect(layoutMetrics.stageHeight).toBe(layoutMetrics.viewportHeight);

    await expect(page.locator('#hud-active-quest-title')).toContainText(
      'Hội Nghị Hợp Nhất Cửu Long'
    );
    await expect(page.locator('#hud-active-objective-text')).toContainText(
      'Bắt liên lạc tại Quảng Trường Bến Cảng'
    );

    // Mở #journal-modal -> #tab-journal-codex để xác minh #codex-progress-count hiển thị rõ ràng, sau đó đóng lại
    await page.keyboard.press('KeyJ');
    await expect(page.locator('#journal-modal')).toBeVisible();
    await page.locator('#tab-journal-codex').click();
    await expect(page.locator('#codex-progress-count')).toBeVisible();
    await expect(page.locator('#codex-progress-count')).toHaveText('0 / 12 CỘT MỐC');
    await page.locator('#btn-close-journal').click();
    await expect(page.locator('#journal-modal')).toBeHidden();
  });

  test('M3-E2E-02: Multi-Choice NPC Dialogue & Objective Progression', async ({
    page,
  }) => {
    // Tiến lại gần Giao Liên Lâm Văn Bính (256, 368) tại harbor_square và nhấn phím E
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

    const provenanceBadge = page.locator('#dialogue-provenance-badge');
    await expect(provenanceBadge).toBeVisible();
    await expect(provenanceBadge).toContainText('Giáo trình 2021');

    const choiceButtons = page.locator('.dialogue-choice-btn');
    await expect(choiceButtons.first()).toBeVisible();
    expect(await choiceButtons.count()).toBeGreaterThanOrEqual(2);

    // Chụp ảnh nghiệm thu hộp thoại đa lựa chọn & nhãn sử liệu
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m3_dialogue_choices.png`,
      fullPage: false,
    });

    // Bấm lựa chọn đầu tiên (hỏi về Sổ Vận Đơn tại Kho Hàng)
    await choiceButtons.first().click();

    await expect(provenanceBadge).toHaveAttribute('data-provenance-type', 'fictional_npc');
    await expect(page.locator('#hud-active-objective-text')).toContainText(
      'Kiểm tra Sổ Vận Đơn & Báo Thanh Niên tại Kho Hàng Số 4'
    );

    const questState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: {
              questFlags: Set<string>;
              completedObjectives: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        hasMetFlag: game.store.questFlags.has('ch1_met_liaison'),
        completedMeetObj: game.store.completedObjectives.has('obj_ch1_meet_liaison'),
      };
    });
    expect(questState.hasMetFlag).toBe(true);
    expect(questState.completedMeetObj).toBe(true);
  });

  test('M3-E2E-03: Journal ([J]), Inventory/Evidence ([I]), and Pause/Settings ([Tab]) Modals + Input Locking & Save Persistence', async ({
    page,
  }) => {
    // 1. Mở Nhật Ký Nhiệm Vụ (#journal-modal) bằng phím J -> kiểm tra khóa di chuyển (mode === 'journal')
    await page.keyboard.press('KeyJ');
    const journalModal = page.locator('#journal-modal');
    await expect(journalModal).toBeVisible();

    const beforeJournalMove = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            store: { getMode(): string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mode: game.store.getMode(),
        x: game.playerX,
        y: game.playerY,
      };
    });
    expect(beforeJournalMove.mode).toBe('journal');

    await page.keyboard.down('KeyD');
    await page.waitForTimeout(200);
    await page.keyboard.up('KeyD');

    const afterJournalMove = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { playerX: number; playerY: number };
        }
      ).__LU_KHACH_GAME__!;
      return { x: game.playerX, y: game.playerY };
    });
    expect(afterJournalMove).toEqual({
      x: beforeJournalMove.x,
      y: beforeJournalMove.y,
    });

    await expect(
      page.locator('[data-quest-id="q_ch1_main_unification"]')
    ).toBeVisible();
    await expect(
      page.locator('[data-quest-id="q_ch1_side_dock_watch"]')
    ).toBeVisible();

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m3_journal_modal.png`,
      fullPage: false,
    });

    await page.locator('#btn-close-journal').click();
    await expect(journalModal).toBeHidden();

    // 2. Thu thập tư liệu ev_ch1_shipping_ledger và mở Túi Tư Liệu (#inventory-modal) bằng phím I
    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            inventorySystem: { addItem(id: string, store: unknown): unknown };
            store: unknown;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.inventorySystem.addItem('ev_ch1_shipping_ledger', game.store);
      game.syncHudWithState();
    });

    await page.keyboard.press('KeyI');
    const inventoryModal = page.locator('#inventory-modal');
    await expect(inventoryModal).toBeVisible();

    const invMode = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { getMode(): string } };
        }
      ).__LU_KHACH_GAME__!;
      return game.store.getMode();
    });
    expect(invMode).toBe('inventory');

    const ledgerCard = page.locator('[data-evidence-id="ev_ch1_shipping_ledger"]');
    await expect(ledgerCard).toBeVisible();
    await ledgerCard.click();

    const detailBox = page.locator('#inventory-item-detail');
    await expect(detailBox).toContainText('Sổ Vận Đơn Cửu Long & Báo Thanh Niên');
    await expect(detailBox).toContainText('Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)');

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m3_inventory_modal.png`,
      fullPage: false,
    });

    await page.locator('#btn-close-inventory').click();
    await expect(inventoryModal).toBeHidden();

    // 3. Mở Tạm Dừng (#pause-modal) -> Cài đặt hệ thống (#pause-open-settings -> #settings-modal),
    //    bật Reduced Motion & Large Text, đóng Cài đặt, bấm Lưu Thủ Công rồi tải lại trang và bấm TIẾP TỤC
    await page.locator('#btn-open-pause').click();
    const pauseModal = page.locator('#pause-modal');
    await expect(pauseModal).toBeVisible();

    await page.locator('#pause-open-settings').click();
    const settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    await page.locator('#toggle-reduced-motion').click();
    await page.locator('#toggle-text-scale').click();

    await expect(page.locator('body')).toHaveClass(/reduced-motion/);
    await expect(page.locator('body')).toHaveClass(/text-scale-large/);

    await page.locator('#btn-close-settings').click();
    await expect(settingsModal).toBeHidden();

    await page.locator('#btn-open-pause').click();
    await expect(pauseModal).toBeVisible();
    await page.locator('#btn-manual-save').click();

    // Tải lại trang, bấm TIẾP TỤC (#btn-menu-continue) và kiểm tra dữ liệu đã lưu được khôi phục đầy đủ
    await page.reload();
    await continueSavedSession(page);

    await expect(page.locator('body')).toHaveClass(/reduced-motion/);
    await expect(page.locator('body')).toHaveClass(/text-scale-large/);

    const restoredData = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: {
              collectedEvidence: Set<string>;
              settings: { reducedMotion: boolean; textScale: string };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        hasLedger: game.store.collectedEvidence.has('ev_ch1_shipping_ledger'),
        reducedMotion: game.store.settings.reducedMotion,
        textScale: game.store.settings.textScale,
      };
    });

    expect(restoredData.hasLedger).toBe(true);
    expect(restoredData.reducedMotion).toBe(true);
    expect(restoredData.textScale).toBe('large');
  });
});
