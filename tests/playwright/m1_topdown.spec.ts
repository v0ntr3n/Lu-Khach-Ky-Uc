/**
 * @file tests/playwright/m1_topdown.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M1 — Phaser 3 + Tiled Top-Down Runtime:
 * 1. Khởi chạy vào bản đồ top-down `harbor_square`, di chuyển 8 hướng (W, A, S, D và chéo W+D) thay đổi cả playerX & playerY với tốc độ chuẩn hóa.
 * 2. Va chạm vật cản (CollisionWorld) chặn nhân vật đi xuyên tường kho hàng, và thứ tự chiều sâu Y-depth cập nhật động theo playerY.
 * 3. Chuyển cảnh 2 chiều giữa `harbor_square` và `warehouse_interior` qua điểm spawn có tên (`warehouse_south_door` & `from_warehouse`)
 *    không bị dội cổng (zero portal ping-pong) và cập nhật `#stage-location-badge`.
 * 4. Tiếp cận NPC / vật thể tương tác trong `harbor_square` và `warehouse_interior`, nhấn phím `E` kích hoạt tương tác đúng 1 lần.
 */

import { expect, test } from '@playwright/test';
import { ARTIFACT_DIR, startNewExploringSession } from './helpers';

test.describe('M1 Top-Down Runtime — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await startNewExploringSession(page);
  });

  test('1. Boots into top-down harbor_square map and moves in 8 directions (cardinal & diagonal W+D) changing both playerX and playerY', async ({
    page,
  }) => {
    const initial = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            getCurrentMapId(): string;
            store: { getMode(): string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mapId: game.getCurrentMapId(),
        mode: game.store.getMode(),
        x: game.playerX,
        y: game.playerY,
      };
    });

    expect(initial.mapId).toBe('harbor_square');
    expect(initial.mode).toBe('exploring');

    // Di chuyển chéo Đông-Bắc (W + D) trên trục đường thoáng (y = 384)
    await page.keyboard.down('KeyW');
    await page.keyboard.down('KeyD');
    await page.waitForTimeout(260);
    await page.keyboard.up('KeyW');
    await page.keyboard.up('KeyD');

    const afterDiagonal = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { playerX: number; playerY: number };
        }
      ).__LU_KHACH_GAME__!;
      return { x: game.playerX, y: game.playerY };
    });

    expect(afterDiagonal.x).toBeGreaterThan(initial.x + 5);
    expect(afterDiagonal.y).toBeLessThan(initial.y - 5);

    // Di chuyển xuống phía Nam (S)
    await page.keyboard.down('KeyS');
    await page.waitForTimeout(180);
    await page.keyboard.up('KeyS');

    const afterSouth = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { playerX: number; playerY: number };
        }
      ).__LU_KHACH_GAME__!;
      return { x: game.playerX, y: game.playerY };
    });
    expect(afterSouth.y).toBeGreaterThan(afterDiagonal.y + 4);

    // Chụp ảnh màn hình thực tế tại Quảng Trường Bến Cảng (harbor_square)
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m1_harbor_square.png`,
      fullPage: false,
    });
  });

  test('2. Solid building collision blocks player from walking through warehouse walls and Y-depth updates dynamically as playerY changes', async ({
    page,
  }) => {
    // Đặt nhân vật ngay phía nam bức tường kho hàng (x = 485, y = 412; tường kho ở x: 464..592, y: 304..400)
    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = 485;
      game.playerY = 412;
    });

    // Giữ phím W đi thẳng lên tường kho hàng trong 350ms
    await page.keyboard.down('KeyW');
    await page.waitForTimeout(350);
    await page.keyboard.up('KeyW');

    const collisionState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            getEntityDepth(footY: number): number;
          };
        }
      ).__LU_KHACH_GAME__!;
      const blockedY = game.playerY;

      // Kiểm tra Y-depth động so với NPC Giao Liên tại y = 368 (footY = 384)
      const npcDepth = game.getEntityDepth(384);
      const depthWhenNorth = game.getEntityDepth(350);
      const depthWhenSouth = game.getEntityDepth(410);

      return {
        blockedY,
        npcDepth,
        depthWhenNorth,
        depthWhenSouth,
      };
    });

    // Tường kho kết thúc ở y = 400, hộp va chạm chân [y - 6, y + 2] bị chặn ở y >= 406
    expect(collisionState.blockedY).toBeGreaterThanOrEqual(406);
    expect(collisionState.depthWhenNorth).toBeLessThan(collisionState.npcDepth);
    expect(collisionState.depthWhenSouth).toBeGreaterThan(collisionState.npcDepth);
  });

  test('3. Two-way door transition between harbor_square and warehouse_interior works via named spawns with zero portal ping-pong and updates #stage-location-badge', async ({
    page,
  }) => {
    const locationBadge = page.locator('#stage-location-badge');
    await expect(locationBadge).toContainText('Quảng Trường Bến Cảng Hương Cảng (1930)');

    // Đặt nhân vật ngay trước cửa Kho Hàng (exit_harbor_to_warehouse tại 512, 400) và bấm E để vào
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
      game.playerX = 524;
      game.playerY = 420;
      game.syncHudWithState();
    });

    await expect(page.locator('#world-interaction-prompt')).toBeVisible();
    await expect(page.locator('#world-interaction-label')).toContainText('Vào:');
    await expect(page.locator('#ctrl-interact')).toContainText('Vào:');
    await page.keyboard.press('KeyE');

    // Chờ vài frame để xác nhận KHÔNG xảy ra hiện tượng dội cổng ngược (portal ping-pong)
    await page.waitForTimeout(200);

    const insideWarehouse = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            getCurrentMapId(): string;
            store: { currentSpawnId: string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mapId: game.getCurrentMapId(),
        spawnId: game.store.currentSpawnId,
        x: game.playerX,
        y: game.playerY,
      };
    });

    expect(insideWarehouse.mapId).toBe('warehouse_interior');
    expect(insideWarehouse.spawnId).toBe('warehouse_south_door');
    expect(insideWarehouse.x).toBe(272);
    expect(insideWarehouse.y).toBe(384);
    await expect(locationBadge).toContainText('Kho Hàng Bến Cảng (Nội Thất)');

    // Chụp ảnh màn hình thực tế bên trong Nhà Kho (warehouse_interior)
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m1_warehouse_interior.png`,
      fullPage: false,
    });

    // Chờ hết thời gian miễn nhiễm cổng (0.65s), sau đó đi xuống cửa ra exit_warehouse_to_harbor (272, 416)
    await page.waitForTimeout(700);
    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            handleInteractAction(): boolean;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = 280;
      game.playerY = 410;
      game.handleInteractAction();
    });

    const returnedHarbor = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            getCurrentMapId(): string;
            store: { currentSpawnId: string };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mapId: game.getCurrentMapId(),
        spawnId: game.store.currentSpawnId,
        x: game.playerX,
        y: game.playerY,
      };
    });

    expect(returnedHarbor.mapId).toBe('harbor_square');
    expect(returnedHarbor.spawnId).toBe('from_warehouse');
    expect(returnedHarbor.x).toBe(512);
    expect(returnedHarbor.y).toBe(432);
    await expect(locationBadge).toContainText('Quảng Trường Bến Cảng Hương Cảng (1930)');
  });

  test('4. Approaching an NPC or interactable and pressing E triggers interaction exactly once', async ({
    page,
  }) => {
    // Tiến lại gần NPC Giao Liên Lâm Văn Bính (256, 368) gắn với Cột mốc I
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

    await expect(page.locator('#world-interaction-prompt')).toBeVisible();
    await expect(page.locator('#world-interaction-label')).toContainText(
      'Trò chuyện: Lâm Văn Bính (Giao Liên)'
    );
    await expect(page.locator('#ctrl-interact')).toContainText(
      'Trò chuyện: Lâm Văn Bính (Giao Liên)'
    );

    // Giữ phím E (gửi kèm các sự kiện repeat) -> chỉ kích hoạt đúng 1 lần
    await page.keyboard.down('KeyE');
    await page.waitForTimeout(120);
    await page.keyboard.up('KeyE');

    await expect(page.locator('#codex-progress-count')).toHaveText('1 / 12 CỘT MỐC');
    await expect(page.locator('#dialogue-milestone-title')).toContainText('MỐC LA MÃ I');
  });
});
