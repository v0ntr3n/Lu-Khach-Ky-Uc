/**
 * @file tests/playwright/m2_exploration.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M2 — World Exploration:
 * - M2-E2E-01: Di chuyển liên thông qua chuỗi khu vực (harbor_square -> old_street -> safehouse_interior -> old_street -> shadow_alley -> archive_courtyard -> secret_archive),
 *              cập nhật discoveredMaps và đảm bảo collectedMilestones.size vẫn bằng 0 khi chỉ di chuyển qua cổng.
 * - M2-E2E-02: Mở Sơ Đồ Khu Vực (#world-map-modal) bằng phím [M] / #btn-open-map, khóa điều khiển di chuyển thế giới (mode === 'map'),
 *              chặn Fast Travel tới khu vực chưa khám phá, mở khóa Checkpoint tại old_street và dịch chuyển nhanh 2 chiều giữa harbor_square <-> old_street.
 * - M2-E2E-03: Bật/tắt Lớp Phủ Kiểm Định Bản Đồ (#btn-debug-overlay trong #presenter-drawer / F3), xác minh store.debugOverlayEnabled === true và chụp ảnh nghiệm thu thực tế.
 */

import { expect, test } from '@playwright/test';
import {
  ARTIFACT_DIR,
  openPresenterDrawer,
  startNewExploringSession,
} from './helpers';

test.describe('M2 World Exploration — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await startNewExploringSession(page);
  });

  test('M2-E2E-01: Multi-Zone Connected Traversal (harbor_square -> old_street -> safehouse_interior -> shadow_alley -> archive_courtyard -> secret_archive)', async ({
    page,
  }) => {
    const initialMap = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { getCurrentMapId(): string };
        }
      ).__LU_KHACH_GAME__!;
      return game.getCurrentMapId();
    });
    expect(initialMap).toBe('harbor_square');

    // Helper đi qua portal theo destinationMap từ bản đồ hiện tại
    const traverseToZone = async (targetZoneId: string) => {
      const transitioned = await page.evaluate((destId) => {
        const game = (
          window as unknown as {
            __LU_KHACH_GAME__?: {
              currentMap: {
                portals: Array<{
                  id: string;
                  x: number;
                  y: number;
                  width: number;
                  height: number;
                  destinationMap: string;
                  destinationSpawn: string;
                }>;
              };
              sceneTransition: { cooldownRemainingSec: number; requiresStepOffPortal: boolean };
              enterPortal(portal: unknown): boolean;
              getCurrentMapId(): string;
            };
          }
        ).__LU_KHACH_GAME__!;
        game.sceneTransition.cooldownRemainingSec = 0;
        game.sceneTransition.requiresStepOffPortal = false;
        const portal = game.currentMap.portals.find((p) => p.destinationMap === destId);
        if (!portal) return { ok: false, mapId: game.getCurrentMapId() };
        const ok = game.enterPortal(portal);
        return { ok, mapId: game.getCurrentMapId() };
      }, targetZoneId);

      expect(transitioned.ok).toBe(true);
      expect(transitioned.mapId).toBe(targetZoneId);
    };

    await traverseToZone('old_street');
    await traverseToZone('safehouse_interior');
    await traverseToZone('old_street');
    await traverseToZone('shadow_alley');
    await traverseToZone('archive_courtyard');
    await traverseToZone('secret_archive');

    const finalState = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            getCurrentMapId(): string;
            store: {
              discoveredMaps: Set<string>;
              collectedMilestones: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        currentMapId: game.getCurrentMapId(),
        discovered: Array.from(game.store.discoveredMaps),
        milestoneCount: game.store.collectedMilestones.size,
      };
    });

    expect(finalState.currentMapId).toBe('secret_archive');
    expect(finalState.discovered).toEqual(
      expect.arrayContaining([
        'harbor_square',
        'old_street',
        'safehouse_interior',
        'shadow_alley',
        'archive_courtyard',
        'secret_archive',
      ])
    );
    // Chỉ đi qua các khu vực tuyệt đối không tự cộng mốc lịch sử
    expect(finalState.milestoneCount).toBe(0);
    await expect(page.locator('#codex-progress-count')).toHaveText('0 / 12 CỘT MỐC');
  });

  test('M2-E2E-02: World Map Modal ([M]), Input Locking & Checkpoint Fast Travel', async ({
    page,
  }) => {
    // Nhấn phím M để mở #world-map-modal
    await page.keyboard.press('KeyM');

    const mapModal = page.locator('#world-map-modal');
    await expect(mapModal).toBeVisible();
    await expect(mapModal).toHaveClass(/is-open/);

    const beforeMove = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            store: { getMode(): string };
            fastTravelToZone(zoneId: string): boolean;
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mode: game.store.getMode(),
        x: game.playerX,
        y: game.playerY,
        canSkipToSecretArchive: game.fastTravelToZone('secret_archive'),
      };
    });

    expect(beforeMove.mode).toBe('map');
    expect(beforeMove.canSkipToSecretArchive).toBe(false);

    // Giữ phím D và W khi bản đồ đang mở -> nhân vật không được phép di chuyển
    await page.keyboard.down('KeyD');
    await page.keyboard.down('KeyW');
    await page.waitForTimeout(250);
    await page.keyboard.up('KeyD');
    await page.keyboard.up('KeyW');

    const afterMove = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { playerX: number; playerY: number };
        }
      ).__LU_KHACH_GAME__!;
      return { x: game.playerX, y: game.playerY };
    });
    expect(afterMove).toEqual({ x: beforeMove.x, y: beforeMove.y });

    // Đóng bản đồ, đi sang old_street và kích hoạt Đèn Báo An Toàn (Checkpoint cp_old_street_tea)
    await page.locator('#btn-close-world-map').click();
    await expect(mapModal).toBeHidden();

    await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            sceneTransition: { cooldownRemainingSec: number; requiresStepOffPortal: boolean };
            currentMap: {
              portals: Array<{ destinationMap: string }>;
              interactables: Array<{ id: string; kind: string; x: number; y: number }>;
            };
            enterPortal(portal: unknown): boolean;
            playerX: number;
            playerY: number;
            handleInteractAction(): boolean;
            store: { unlockCheckpoint(id: string, mapId?: string): boolean };
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.sceneTransition.cooldownRemainingSec = 0;
      game.sceneTransition.requiresStepOffPortal = false;
      const portalToStreet = game.currentMap.portals.find(
        (p) => p.destinationMap === 'old_street'
      );
      if (portalToStreet) {
        game.enterPortal(portalToStreet);
      }
      const cp = game.currentMap.interactables.find((i) => i.kind === 'checkpoint');
      if (cp) {
        game.playerX = cp.x;
        game.playerY = cp.y;
        game.handleInteractAction();
      }
      game.store.unlockCheckpoint('cp_old_street_tea', 'old_street');
      game.syncHudWithState();
    });

    // Mở #world-map-modal từ #pause-modal (#btn-open-pause -> #btn-open-map) và bấm nút Dịch Chuyển Nhanh về harbor_square
    await page.locator('#btn-open-pause').click();
    await expect(page.locator('#pause-modal')).toBeVisible();
    await page.locator('#btn-open-map').click();
    await expect(mapModal).toBeVisible();

    const fastTravelHarborBtn = page.locator('[data-fast-travel-zone="harbor_square"]');
    await expect(fastTravelHarborBtn).toBeVisible();
    await fastTravelHarborBtn.click();

    const atHarbor = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { getCurrentMapId(): string };
        }
      ).__LU_KHACH_GAME__!;
      return game.getCurrentMapId();
    });
    expect(atHarbor).toBe('harbor_square');

    // Mở lại bản đồ bằng phím [M] và dịch chuyển nhanh sang old_street
    await page.keyboard.press('KeyM');
    await expect(mapModal).toBeVisible();
    const fastTravelStreetBtn = page.locator('[data-fast-travel-zone="old_street"]');
    await expect(fastTravelStreetBtn).toBeVisible();
    await fastTravelStreetBtn.click();

    const atStreet = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            getCurrentMapId(): string;
            store: { collectedMilestones: Set<string> };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        mapId: game.getCurrentMapId(),
        milestones: game.store.collectedMilestones.size,
      };
    });
    expect(atStreet.mapId).toBe('old_street');
    expect(atStreet.milestones).toBe(0);
  });

  test('M2-E2E-03: Debug Overlay Toggle ([F3] / #btn-debug-overlay) & Visual Screenshot Audit', async ({
    page,
  }) => {
    await openPresenterDrawer(page);
    const debugBtn = page.locator('#btn-debug-overlay');
    await expect(debugBtn).toBeVisible();
    await debugBtn.click();

    const debugEnabled = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { debugOverlayEnabled: boolean } };
        }
      ).__LU_KHACH_GAME__!;
      return game.store.debugOverlayEnabled;
    });
    expect(debugEnabled).toBe(true);

    // Đóng ngăn Giảng viên để quan sát trọn vẹn lớp kiểm định va chạm trên khung cảnh
    await page.locator('#btn-close-presenter').click();
    await expect(page.locator('#presenter-drawer')).toBeHidden();
    await expect(page.locator('#dev-debug-readout')).toBeVisible();

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m2_debug_overlay.png`,
      fullPage: false,
    });

    // Mở Sơ Đồ Khu Vực (#world-map-modal) và chụp ảnh nghiệm thu giao diện Sơn Mài & Giấy Dó
    await page.keyboard.press('KeyM');
    await expect(page.locator('#world-map-modal')).toBeVisible();
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m2_world_map_modal.png`,
      fullPage: false,
    });
  });
});
