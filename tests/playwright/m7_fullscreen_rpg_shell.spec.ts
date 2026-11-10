/**
 * @file tests/playwright/m7_fullscreen_rpg_shell.spec.ts
 * @description Bộ kiểm thử End-to-End toàn diện (Playwright Chromium) cho Cột mốc M7 — Fullscreen RPG Shell & Main Menu:
 * 1. Xác minh loại bỏ 100% khung vỏ trang web cũ (.w26-topbar, #codex-sidebar, .stage-banner, thẻ hội thoại cố định dưới canvas)
 *    và đảm bảo #game-shell phủ kín 100vw × 100dvh, không thanh cuộn (overflow: hidden).
 * 2. Vòng đời Menu Chính (MAIN_MENU), các nút BẮT ĐẦU / TIẾP TỤC / MULTIPLAYER / HƯỚNG DẪN / CÀI ĐẶT, điều hướng bàn phím,
 *    tách biệt SETTINGS_KEY_V1 với SAVE_KEY_V2, đóng băng mô phỏng khi Tạm Dừng (PAUSED), cảnh báo ghi đè save và khôi phục TIẾP TỤC.
 * 3. Kiểm định đa độ phân giải (1366×768, 1920×1080, 2560×1440, Mobile Landscape 844×390), tỷ lệ điểm ảnh số nguyên (pixelScale === 4,
 *    nhân vật 88 CSS px ở 1920×1080), không méo khung hình, 3 mức Camera Zoom (wide 3×, normal 4×, close 5×) giữ nguyên va chạm & bán kính tương tác,
 *    và chuyển đổi Toàn màn hình [F] / #btn-toggle-fullscreen.
 * 4. Kiểm định phông chữ tiếng Việt, độ dễ đọc HUD ngữ cảnh (#world-interaction-prompt >= 15px, #dialogue-body-text 18–24px,
 *    #hud-active-objective-text >= 15px, nút điều khiển >= 16px) và đo hiệu năng kết xuất thực tế trên trình duyệt.
 */

import { expect, test } from '@playwright/test';
import {
  ARTIFACT_DIR,
  bootFreshMainMenu,
  continueSavedSession,
  startFromMainMenuIntoExploring,
  startNewExploringSession,
} from './helpers';

test.describe('M7 Fullscreen RPG Shell, Main Menu, Multi-Resolution & Typography QA', () => {
  test('M7-E2E-01: Zero Website Chrome & True Fullscreen 100vw × 100dvh Shell', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await bootFreshMainMenu(page);

    // 1. Không còn các thành phần giao diện website cũ (.w26-topbar, #codex-sidebar, .stage-banner, thẻ giấy dó cố định dưới stage)
    await expect(page.locator('.w26-topbar')).toHaveCount(0);
    await expect(page.locator('#codex-sidebar')).toHaveCount(0);
    await expect(page.locator('.stage-banner')).toHaveCount(0);
    await expect(page.locator('#stage-viewport #dialogue-parchment-card')).toHaveCount(0);

    // Khi chưa hội thoại, #dialogue-modal hoàn toàn ẩn khỏi màn hình
    await expect(page.locator('#dialogue-modal')).toBeHidden();

    // 2. #game-shell khớp tuyệt đối kích thước khung nhìn 1920×1080, không có thanh cuộn dọc/ngang
    const shellMetrics = await page.evaluate(() => {
      const shell = document.getElementById('game-shell')!;
      const rect = shell.getBoundingClientRect();
      return {
        rectX: Math.round(rect.x),
        rectY: Math.round(rect.y),
        rectWidth: Math.round(rect.width),
        rectHeight: Math.round(rect.height),
        innerWidth: window.innerWidth,
        innerHeight: window.innerHeight,
        scrollWidth: document.documentElement.scrollWidth,
        scrollHeight: document.documentElement.scrollHeight,
        bodyOverflow: window.getComputedStyle(document.body).overflow,
      };
    });

    expect(shellMetrics.rectX).toBe(0);
    expect(shellMetrics.rectY).toBe(0);
    expect(shellMetrics.rectWidth).toBe(1920);
    expect(shellMetrics.rectHeight).toBe(1080);
    expect(shellMetrics.scrollWidth).toBe(shellMetrics.innerWidth);
    expect(shellMetrics.scrollHeight).toBe(shellMetrics.innerHeight);
    expect(shellMetrics.bodyOverflow).toBe('hidden');
  });

  test('M7-E2E-02: Main Menu Startup, Sub-Modals, Keyboard Navigation, Pause Freeze & Save Lifecycle', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await bootFreshMainMenu(page);

    // 1. Khởi động vào MAIN_MENU: #main-menu-screen hiển thị, #hud-layer ẩn, mô phỏng thế giới KHÔNG chạy
    await expect(page.locator('#main-menu-screen')).toBeVisible();
    await expect(page.locator('#hud-layer')).toBeHidden();

    const menuBootState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            store: { getLifecycle(): string; getMode(): string };
            stealthSystem: { getGuards(): Array<{ x: number; y: number }> };
          };
        }
      ).__LU_KHACH_GAME__!;
      const g0 = game.stealthSystem.getGuards()[0];
      return {
        lifecycle: game.store.getLifecycle(),
        mode: game.store.getMode(),
        x: game.playerX,
        y: game.playerY,
        guardX: g0?.x ?? 0,
        guardY: g0?.y ?? 0,
      };
    });

    expect(menuBootState.lifecycle).toBe('MAIN_MENU');
    expect(menuBootState.mode).toBe('main_menu');

    // Giữ phím D trong Main Menu không làm dịch chuyển Lữ Khách hay lính tuần tra
    await page.keyboard.down('KeyD');
    await page.waitForTimeout(250);
    await page.keyboard.up('KeyD');

    const menuAfterWait = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            stealthSystem: { getGuards(): Array<{ x: number; y: number }> };
          };
        }
      ).__LU_KHACH_GAME__!;
      const g0 = game.stealthSystem.getGuards()[0];
      return {
        x: game.playerX,
        y: game.playerY,
        guardX: g0?.x ?? 0,
        guardY: g0?.y ?? 0,
      };
    });
    expect(menuAfterWait).toEqual({
      x: menuBootState.x,
      y: menuBootState.y,
      guardX: menuBootState.guardX,
      guardY: menuBootState.guardY,
    });

    // Chụp ảnh Menu Chính 1920×1080
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_main_menu_1920x1080.png`,
      fullPage: false,
    });

    // 2. Nút TIẾP TỤC bị vô hiệu hóa khi chưa có bản lưu; đổi CÀI ĐẶT chỉ lưu SETTINGS_KEY_V1 mà KHÔNG bật TIẾP TỤC
    const continueBtn = page.locator('#btn-menu-continue');
    await expect(continueBtn).toBeDisabled();
    await expect(page.locator('#menu-save-summary')).toContainText('Chưa có hồ sơ lưu');

    await page.locator('#btn-menu-settings').click();
    await expect(page.locator('#settings-modal')).toBeVisible();
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_menu_settings.png`,
      fullPage: false,
    });

    // Thay đổi cài đặt gợi ý trong Menu Chính rồi bật lại -> lưu SETTINGS_KEY_V1 nhưng không sinh SAVE_KEY_V2
    await page.locator('#toggle-show-hints').click();
    await page.locator('#toggle-show-hints').click();

    const storageCheck = await page.evaluate(() => ({
      settingsRaw: localStorage.getItem('lu_khach_ky_uc_settings_v1'),
      saveRaw: localStorage.getItem('lu_khach_ky_uc_save_v2'),
    }));
    expect(storageCheck.settingsRaw).not.toBeNull();
    expect(storageCheck.saveRaw).toBeNull();

    // Nhấn Escape đóng #settings-modal và khôi phục tiêu điểm về #btn-menu-settings
    // (đưa con trỏ chuột ra góc trung tính trước khi đóng modal để tránh sự kiện mouseenter tổng hợp khi modal ẩn khỏi layout)
    await page.mouse.move(12, 12);
    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-modal')).toBeHidden();
    await expect(continueBtn).toBeDisabled();
    await expect(page.locator('#btn-menu-settings')).toBeFocused();
    await expect(page.locator('#btn-menu-settings')).toHaveClass(/is-focused/);

    // 3. Kiểm tra MULTIPLAYER ("Sắp ra mắt" trung thực) & HƯỚNG DẪN
    await page.locator('#btn-menu-multiplayer').click();
    const mpModal = page.locator('#multiplayer-modal');
    await expect(mpModal).toBeVisible();
    await expect(mpModal).toContainText(/sắp ra mắt/i);
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_menu_multiplayer.png`,
      fullPage: false,
    });
    await page.mouse.move(12, 12);
    await page.keyboard.press('Escape');
    await expect(mpModal).toBeHidden();
    await expect(page.locator('#btn-menu-multiplayer')).toBeFocused();
    await expect(page.locator('#btn-menu-multiplayer')).toHaveClass(/is-focused/);

    await page.locator('#btn-menu-instructions').click();
    const instrModal = page.locator('#instructions-modal');
    await expect(instrModal).toBeVisible();
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_menu_instructions.png`,
      fullPage: false,
    });
    await page.mouse.move(12, 12);
    await page.keyboard.press('Escape');
    await expect(instrModal).toBeHidden();
    await expect(page.locator('#btn-menu-instructions')).toBeFocused();
    await expect(page.locator('#btn-menu-instructions')).toHaveClass(/is-focused/);

    // 4. Điều hướng bàn phím ArrowUp / ArrowDown (vòng lặp qua cuối danh sách & bỏ qua nút TIẾP TỤC đang disabled)
    await page.keyboard.press('ArrowDown');
    await expect(page.locator('#btn-menu-settings')).toHaveClass(/is-focused/);

    await page.keyboard.press('ArrowDown');
    await expect(page.locator('#btn-menu-start')).toHaveClass(/is-focused/);

    await page.keyboard.press('ArrowDown');
    await expect(page.locator('#btn-menu-multiplayer')).toHaveClass(/is-focused/);

    await page.keyboard.press('ArrowDown');
    await expect(page.locator('#btn-menu-instructions')).toHaveClass(/is-focused/);

    await page.keyboard.press('ArrowUp');
    await expect(page.locator('#btn-menu-multiplayer')).toHaveClass(/is-focused/);

    await page.keyboard.press('ArrowUp');
    await expect(page.locator('#btn-menu-start')).toHaveClass(/is-focused/);

    // 5. Bấm BẮT ĐẦU -> LOADING -> OPENING -> Bỏ qua Mở màn -> PLAYING ('exploring')
    await startFromMainMenuIntoExploring(page);

    const playingState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: { getLifecycle(): string; getMode(): string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        lifecycle: game.store.getLifecycle(),
        mode: game.store.getMode(),
      };
    });
    expect(playingState).toEqual({ lifecycle: 'PLAYING', mode: 'exploring' });

    // Di chuyển nhân vật sang phải một đoạn ngắn để tạo tọa độ lưu khác mặc định
    await page.keyboard.down('KeyD');
    await page.waitForTimeout(260);
    await page.keyboard.up('KeyD');

    // 6. Tạm dừng (Escape) -> Đóng băng toàn bộ chuyển động & mô phỏng tuần tra -> Chụp ảnh -> Về Menu Chính
    await page.keyboard.press('Escape');
    await expect(page.locator('#pause-modal')).toBeVisible();

    const pausedBefore = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            store: { getLifecycle(): string; getMode(): string };
            stealthSystem: { getGuards(): Array<{ x: number; y: number }> };
            currentMap: { npcs: Array<{ x: number; y: number }> };
          };
        }
      ).__LU_KHACH_GAME__!;
      const guard = game.stealthSystem.getGuards()[0];
      const npcWithRoute = game.currentMap.npcs[1] ?? game.currentMap.npcs[0];
      return {
        lifecycle: game.store.getLifecycle(),
        mode: game.store.getMode(),
        playerX: game.playerX,
        playerY: game.playerY,
        guardX: guard?.x ?? 0,
        guardY: guard?.y ?? 0,
        npcX: npcWithRoute?.x ?? 0,
        npcY: npcWithRoute?.y ?? 0,
      };
    });

    expect(pausedBefore.lifecycle).toBe('PAUSED');
    expect(pausedBefore.mode).toBe('paused');
    expect(pausedBefore.playerX).toBeGreaterThan(95);

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_pause_menu.png`,
      fullPage: false,
    });

    await page.keyboard.down('KeyD');
    await page.waitForTimeout(300);
    await page.keyboard.up('KeyD');

    const pausedAfter = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            stealthSystem: { getGuards(): Array<{ x: number; y: number }> };
            currentMap: { npcs: Array<{ x: number; y: number }> };
          };
        }
      ).__LU_KHACH_GAME__!;
      const guard = game.stealthSystem.getGuards()[0];
      const npcWithRoute = game.currentMap.npcs[1] ?? game.currentMap.npcs[0];
      return {
        playerX: game.playerX,
        playerY: game.playerY,
        guardX: guard?.x ?? 0,
        guardY: guard?.y ?? 0,
        npcX: npcWithRoute?.x ?? 0,
        npcY: npcWithRoute?.y ?? 0,
      };
    });

    expect(pausedAfter).toEqual({
      playerX: pausedBefore.playerX,
      playerY: pausedBefore.playerY,
      guardX: pausedBefore.guardX,
      guardY: pausedBefore.guardY,
      npcX: pausedBefore.npcX,
      npcY: pausedBefore.npcY,
    });

    // Bấm Về Menu Chính (#btn-return-main-menu) -> bảo toàn dữ liệu lưu, bật nút TIẾP TỤC với thông tin Chương I
    await page.locator('#btn-return-main-menu').click();
    await expect(page.locator('#main-menu-screen')).toBeVisible();
    await expect(page.locator('#hud-layer')).toBeHidden();
    await expect(continueBtn).toBeEnabled();
    await expect(page.locator('#menu-save-summary')).toContainText(/CHƯƠNG I/i);

    // 7. Bấm BẮT ĐẦU khi đã có hồ sơ lưu -> cảnh báo #confirm-modal -> bấm Hủy bỏ (#btn-confirm-no) giữ nguyên save
    await page.locator('#btn-menu-start').click();
    const confirmModal = page.locator('#confirm-modal');
    await expect(confirmModal).toBeVisible();
    await expect(page.locator('#confirm-modal-body')).toContainText(/CHƯƠNG I/i);

    await page.locator('#btn-confirm-no').click();
    await expect(confirmModal).toBeHidden();
    await expect(page.locator('#main-menu-screen')).toBeVisible();
    await expect(continueBtn).toBeEnabled();

    // 8. Bấm TIẾP TỤC (#btn-menu-continue) -> nạp lại đúng vị trí đã lưu
    await continueSavedSession(page);

    const resumedState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            store: { getLifecycle(): string; getMode(): string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        lifecycle: game.store.getLifecycle(),
        mode: game.store.getMode(),
        playerX: game.playerX,
      };
    });

    expect(resumedState.lifecycle).toBe('PLAYING');
    expect(resumedState.mode).toBe('exploring');
    expect(Math.abs(resumedState.playerX - Math.round(pausedBefore.playerX))).toBeLessThanOrEqual(2);
  });

  test('M7-E2E-03: Multi-Resolution Matrix, Character Scale, Zero Aspect Distortion, Camera Zoom Presets & Fullscreen Toggle', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await startNewExploringSession(page);

    // 1. Độ phân giải 1366×768
    await page.setViewportSize({ width: 1366, height: 768 });
    await page.waitForTimeout(150);
    const m1366 = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            gameShell: {
              getMetrics(): {
                pixelScale: number;
                logicalWidth: number;
                logicalHeight: number;
                isMobileLandscape: boolean;
              };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return game.gameShell.getMetrics();
    });
    expect(m1366.pixelScale).toBe(3);
    expect(m1366.isMobileLandscape).toBe(false);
    await expect(page.locator('.controls-bar')).toBeHidden();
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_gameplay_1366x768.png`,
      fullPage: false,
    });

    // 2. Độ phân giải 1920×1080 (Full HD): pixelScale === 4, nhân vật 22*4 === 88 CSS px, tỷ lệ X/Y đồng nhất
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.waitForTimeout(150);

    const fhdAudit = await page.evaluate(() => {
      const canvas = document.getElementById('pixel-canvas') as HTMLCanvasElement;
      const rect = canvas.getBoundingClientRect();
      const style = window.getComputedStyle(canvas);
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            gameShell: {
              getMetrics(): {
                pixelScale: number;
                logicalWidth: number;
                logicalHeight: number;
              };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const metrics = game.gameShell.getMetrics();
      return {
        pixelScale: metrics.pixelScale,
        charCssHeight: 22 * metrics.pixelScale,
        imageRendering: style.imageRendering,
        canvasBufferW: canvas.width,
        canvasBufferH: canvas.height,
        canvasCssW: rect.width,
        canvasCssH: rect.height,
        scaleX: rect.width / canvas.width,
        scaleY: rect.height / canvas.height,
      };
    });

    expect(fhdAudit.pixelScale).toBe(4);
    expect(fhdAudit.charCssHeight).toBe(88);
    expect(fhdAudit.charCssHeight).toBeGreaterThanOrEqual(64);
    expect(fhdAudit.charCssHeight).toBeLessThanOrEqual(96);
    expect(['pixelated', 'crisp-edges']).toContain(fhdAudit.imageRendering);
    expect(fhdAudit.scaleX).toBe(4);
    expect(fhdAudit.scaleY).toBe(4);
    expect(fhdAudit.scaleX).toBe(fhdAudit.scaleY);
    await expect(page.locator('.controls-bar')).toBeHidden();

    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_gameplay_1920x1080.png`,
      fullPage: false,
    });

    // 3. Độ phân giải 2560×1440 (QHD)
    await page.setViewportSize({ width: 2560, height: 1440 });
    await page.waitForTimeout(150);
    const m2560 = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            gameShell: { getMetrics(): { pixelScale: number } };
          };
        }
      ).__LU_KHACH_GAME__!;
      return game.gameShell.getMetrics();
    });
    expect(m2560.pixelScale).toBe(6);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_gameplay_2560x1440.png`,
      fullPage: false,
    });

    // 4. Mobile Landscape 844×390: .controls-bar hiển thị trên 844×390 và ẩn trên 1920×1080 desktop
    await page.setViewportSize({ width: 844, height: 390 });
    await page.waitForTimeout(150);
    await expect(page.locator('.controls-bar')).toBeVisible();
    const mMobile = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            gameShell: {
              getMetrics(): { pixelScale: number; isMobileLandscape: boolean };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return game.gameShell.getMetrics();
    });
    expect(mMobile.pixelScale).toBeGreaterThanOrEqual(2);
    expect(mMobile.isMobileLandscape).toBe(true);

    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_gameplay_mobile_844x390.png`,
      fullPage: false,
    });

    // Quay lại 1920×1080 để kiểm tra 3 mức Camera Zoom ('wide', 'normal', 'close') và Fullscreen Toggle
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.waitForTimeout(150);
    await expect(page.locator('.controls-bar')).toBeHidden();

    // Helper kiểm tra va chạm tường kho và bán kính tương tác thế giới ở bất kỳ mức zoom nào
    const sampleWorldPhysicsAndInteraction = async () =>
      page.evaluate(() => {
        const game = (
          window as unknown as {
            __LU_KHACH_GAME__?: {
              gameShell: { getMetrics(): { pixelScale: number; zoomPreset: string } };
              collisionWorld: {
                resolveMovement(
                  rect: { x: number; y: number; width: number; height: number },
                  dx: number,
                  dy: number
                ): { x: number; y: number };
              };
              getPlayerFootRect(
                x: number,
                y: number
              ): { x: number; y: number; width: number; height: number };
              interactionSystem: {
                findNearestCandidate(
                  pos: { x: number; y: number },
                  map: unknown,
                  chapter: unknown
                ): { kind: string; id: string } | null;
              };
              currentMap: unknown;
              currentChapter: unknown;
            };
          }
        ).__LU_KHACH_GAME__!;

        const metrics = game.gameShell.getMetrics();
        const resolvedWall = game.collisionWorld.resolveMovement(
          game.getPlayerFootRect(485, 412),
          0,
          -24
        );
        const nearLiaison = game.interactionSystem.findNearestCandidate(
          { x: 256, y: 384 },
          game.currentMap,
          game.currentChapter
        );
        const farFromLiaison = game.interactionSystem.findNearestCandidate(
          { x: 180, y: 384 },
          game.currentMap,
          game.currentChapter
        );
        return {
          pixelScale: metrics.pixelScale,
          zoomPreset: metrics.zoomPreset,
          wallStopFootY: resolvedWall.y,
          nearId: nearLiaison?.id ?? null,
          farId: farFromLiaison?.id ?? null,
        };
      });

    const baselineNormal = await sampleWorldPhysicsAndInteraction();
    expect(baselineNormal.pixelScale).toBe(4);
    expect(baselineNormal.zoomPreset).toBe('normal');

    // Chuyển sang Camera Zoom 'wide' (3×) qua #settings-modal
    await page.locator('#btn-open-pause').click();
    await page.locator('#pause-open-settings').click();
    await expect(page.locator('#settings-modal')).toBeVisible();
    await page.locator('[data-zoom-preset="wide"]').click();
    await page.locator('#btn-close-settings').click();
    await expect(page.locator('#settings-modal')).toBeHidden();

    const wideSample = await sampleWorldPhysicsAndInteraction();
    expect(wideSample.pixelScale).toBe(3);
    expect(wideSample.zoomPreset).toBe('wide');
    expect(wideSample.wallStopFootY).toBe(baselineNormal.wallStopFootY);
    expect(wideSample.nearId).toBe(baselineNormal.nearId);
    expect(wideSample.farId).toBe(baselineNormal.farId);

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_zoom_wide.png`,
      fullPage: false,
    });

    // Chuyển sang Camera Zoom 'close' (5×) qua #settings-modal
    await page.locator('#btn-open-pause').click();
    await page.locator('#pause-open-settings').click();
    await page.locator('[data-zoom-preset="close"]').click();
    await page.locator('#btn-close-settings').click();

    const closeSample = await sampleWorldPhysicsAndInteraction();
    expect(closeSample.pixelScale).toBe(5);
    expect(closeSample.zoomPreset).toBe('close');
    expect(closeSample.wallStopFootY).toBe(baselineNormal.wallStopFootY);
    expect(closeSample.nearId).toBe(baselineNormal.nearId);
    expect(closeSample.farId).toBe(baselineNormal.farId);

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_zoom_close.png`,
      fullPage: false,
    });

    // Khôi phục zoom 'normal' (4×) và kiểm tra Fullscreen toggle bằng cả nút #btn-toggle-fullscreen lẫn phím [F]
    await page.locator('#btn-open-pause').click();
    await page.locator('#pause-open-settings').click();
    await page.locator('[data-zoom-preset="normal"]').click();

    await page.locator('#btn-toggle-fullscreen').click();
    const fsFromButton = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            fullscreenManager: {
              isFullscreenActive(): boolean;
              getState(): { isBrowserFullscreen: boolean; isViewportFallback: boolean };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        active: game.fullscreenManager.isFullscreenActive(),
        state: game.fullscreenManager.getState(),
        hasFallbackClass: document.body.classList.contains('is-viewport-fullscreen'),
        hasBrowserClass: document.body.classList.contains('is-browser-fullscreen'),
      };
    });
    expect(fsFromButton.active).toBe(true);
    expect(fsFromButton.hasFallbackClass || fsFromButton.hasBrowserClass).toBe(true);

    await page.locator('#btn-toggle-fullscreen').click();
    const fsExitedFromButton = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { fullscreenManager: { isFullscreenActive(): boolean } };
        }
      ).__LU_KHACH_GAME__!;
      return game.fullscreenManager.isFullscreenActive();
    });
    expect(fsExitedFromButton).toBe(false);

    await page.locator('#btn-close-settings').click();
    await expect(page.locator('#settings-modal')).toBeHidden();

    // Bấm phím [F] trong gameplay để bật/tắt Fullscreen
    await page.keyboard.press('KeyF');
    const fsFromKeyF = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { fullscreenManager: { isFullscreenActive(): boolean } };
        }
      ).__LU_KHACH_GAME__!;
      return game.fullscreenManager.isFullscreenActive();
    });
    expect(fsFromKeyF).toBe(true);

    await page.keyboard.press('KeyF');
    const fsExitFromKeyF = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { fullscreenManager: { isFullscreenActive(): boolean } };
        }
      ).__LU_KHACH_GAME__!;
      return game.fullscreenManager.isFullscreenActive();
    });
    expect(fsExitFromKeyF).toBe(false);
  });

  test('M7-E2E-04: Vietnamese Typography, Contextual HUD Readability, Journal Codex & Frame-Time Audit', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await bootFreshMainMenu(page);

    // Kiểm tra cỡ chữ nút trên Menu Chính >= 16px
    const menuBtnFontPx = await page.evaluate(() => {
      const label =
        document.querySelector('#btn-menu-start .menu-cmd-label') ??
        document.getElementById('btn-menu-start')!;
      return parseFloat(window.getComputedStyle(label).fontSize);
    });
    expect(menuBtnFontPx).toBeGreaterThanOrEqual(16);

    await startFromMainMenuIntoExploring(page);

    // 1. Di chuyển tới Giao Liên Lâm Văn Bính (256, 384) -> kiểm tra nhãn nổi #world-interaction-prompt
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

    const promptEl = page.locator('#world-interaction-prompt');
    await expect(promptEl).toBeVisible();
    await expect(promptEl.locator('.prompt-key')).toHaveText('E');
    await expect(page.locator('#world-interaction-label')).toContainText('Trò chuyện');

    const promptFontPx = await promptEl.evaluate((el) =>
      parseFloat(window.getComputedStyle(el).fontSize)
    );
    expect(promptFontPx).toBeGreaterThanOrEqual(15);

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_interaction_prompt.png`,
      fullPage: false,
    });

    // 2. Nhấn [E] mở #dialogue-modal -> kiểm tra cỡ chữ #dialogue-body-text (18–24px) và #hud-active-objective-text (>= 15px)
    await page.keyboard.press('KeyE');
    await expect(page.locator('#dialogue-modal')).toBeVisible();

    const typographyMetrics = await page.evaluate(() => {
      const dlgBody = document.getElementById('dialogue-body-text')!;
      const hudObj = document.getElementById('hud-active-objective-text')!;
      const closeBtn = document.getElementById('btn-close-dialogue')!;
      const closeRect = closeBtn.getBoundingClientRect();
      return {
        dialogueText: dlgBody.textContent ?? '',
        dialogueBodyFontPx: parseFloat(window.getComputedStyle(dlgBody).fontSize),
        hudObjectiveFontPx: parseFloat(window.getComputedStyle(hudObj).fontSize),
        ctrlBtnFontPx: parseFloat(window.getComputedStyle(closeBtn).fontSize),
        closeBtnBottom: closeRect.bottom,
        viewportHeight: window.innerHeight,
      };
    });

    expect(typographyMetrics.dialogueText).not.toContain('““');
    expect(typographyMetrics.dialogueText).not.toContain('””');
    expect(typographyMetrics.closeBtnBottom).toBeLessThanOrEqual(
      typographyMetrics.viewportHeight
    );
    expect(typographyMetrics.dialogueBodyFontPx).toBeGreaterThanOrEqual(18);
    expect(typographyMetrics.dialogueBodyFontPx).toBeLessThanOrEqual(24);
    expect(typographyMetrics.hudObjectiveFontPx).toBeGreaterThanOrEqual(15);
    expect(typographyMetrics.ctrlBtnFontPx).toBeGreaterThanOrEqual(16);

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_npc_dialogue_1920x1080.png`,
      fullPage: false,
    });

    // Chọn nhánh đầu tiên để mở khóa Mốc I rồi đóng hội thoại
    await page.locator('.dialogue-choice-btn').first().click();
    await page.keyboard.press('Escape');
    await expect(page.locator('#dialogue-modal')).toBeHidden();

    // 3. Mở Nhật Ký [J] -> chuyển sang ngăn Biên Niên Sử I–XII (#tab-journal-codex) và chụp ảnh
    await page.keyboard.press('KeyJ');
    await expect(page.locator('#journal-modal')).toBeVisible();
    await page.locator('#tab-journal-codex').click();
    await expect(page.locator('#journal-codex-panel')).toBeVisible();
    await expect(page.locator('#codex-progress-count')).toBeVisible();
    await expect(page.locator('#codex-progress-count')).toHaveText('1 / 12 CỘT MỐC');

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_journal_codex_1920x1080.png`,
      fullPage: false,
    });

    await page.keyboard.press('Escape');
    await expect(page.locator('#journal-modal')).toBeHidden();

    // 4. Kiểm định hiệu năng 60 khung hình (Physics + Fullscreen 480×270 Pixel Render tại 1920×1080)
    const perf1080p = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            updatePhysics(dt: number, timeSec: number): void;
            renderer: { render(state: unknown): void };
            currentChapter: unknown;
            playerX: number;
            playerY: number;
            cameraX: number;
            cameraY: number;
            facingRight: boolean;
            playerFacing: string;
            store: {
              getActiveMilestones(): Set<string>;
              settings: { reducedMotion: boolean; effectsQuality?: string };
            };
            currentMap: unknown;
            stealthSystem: { getRenderStates(bonus: boolean): unknown[] };
          };
        }
      ).__LU_KHACH_GAME__!;

      const frames = 60;
      const t0 = performance.now();
      for (let i = 0; i < frames; i++) {
        const t = i / 60;
        game.updatePhysics(1 / 60, t);
        game.renderer.render({
          chapter: game.currentChapter,
          playerX: game.playerX,
          playerY: game.playerY,
          cameraX: game.cameraX,
          cameraY: game.cameraY,
          facingRight: game.facingRight,
          playerFacing: game.playerFacing,
          isMoving: false,
          isSheltered: false,
          collectedMilestones: game.store.getActiveMilestones(),
          nearbyMilestone: null,
          nearClimaxGate: false,
          climaxBurstProgress: 0,
          timeSec: t,
          mapData: game.currentMap,
          nearbyPrompt: null,
          nearbyTargetPos: null,
          debugOverlay: false,
          patrolActors: game.stealthSystem.getRenderStates(false),
          reducedMotion: game.store.settings.reducedMotion,
          effectsQuality: game.store.settings.effectsQuality ?? 'full',
          transitionFadeAlpha: 0,
        });
      }
      const elapsedMs = performance.now() - t0;
      return {
        totalMs: Number(elapsedMs.toFixed(2)),
        avgFrameMs: Number((elapsedMs / frames).toFixed(2)),
      };
    });

    console.info(
      `[M7-PERF] 1920x1080 60 frames total=${perf1080p.totalMs}ms, avg=${perf1080p.avgFrameMs}ms/frame`
    );

    // Ngân sách 60 FPS là 16.67ms/frame; trung bình phải thấp hơn ngưỡng an toàn
    expect(perf1080p.avgFrameMs).toBeLessThan(16.67);
  });
});
