/**
 * @file tests/playwright/m6_full_game_qa.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M6 — Additional Chapters (II, III, IV) & Final QA:
 * - M6-E2E-01: Chương II (1945 Tân Trào -> Ba Đình): Chuyển sang Chương II, kiểm tra spawn tại tan_trao_banyan,
 *              tương tác với npc_tan_trao_messenger (Mốc IV), giải câu đố Điều Phối Mạng Lưới Giao Liên
 *              (puzzle_ch2_dispatch_network -> Mốc V), đi qua portal tới ba_dinh_square, gặp npc_ba_dinh_witness (Mốc VI)
 *              và chụp ảnh m6_ch2_ba_dinh_1945.png.
 * - M6-E2E-02: Chương III (1954–1975 Đèo Pha Đin -> Hành Lang Mùa Xuân 1975): Chuyển sang Chương III, kiểm tra spawn tại pha_din_pass,
 *              tương tác với npc_pha_din_quartermaster (Mốc VII), giải Sa Bàn Tiếp Vận Pha Đin
 *              (puzzle_ch3_supply_logistics -> Mốc VIII), đi qua portal tới spring_1975_corridor, gặp npc_1975_signal_officer (Mốc IX)
 *              và chụp ảnh m6_ch3_spring_1975.png.
 * - M6-E2E-03: Chương IV (1986–2045 Đại Lộ Đổi Mới -> Quảng Trường Kỷ Nguyên Số 2045): Chuyển sang Chương IV, kiểm tra spawn tại doi_moi_avenue,
 *              tương tác với npc_doi_moi_economist (Mốc X), giải Ma Trận Quyết Sách Đổi Mới
 *              (puzzle_ch4_causal_policy -> Mốc XI), đi qua portal tới nexus_2045_plaza, gặp npc_archivist_2045 (Mốc XII),
 *              kiểm tra nhãn speculative_2045 trên #dialogue-provenance-badge, đo hiệu năng 30 frame và chụp ảnh m6_ch4_nexus_2045.png.
 */

import { expect, test } from '@playwright/test';
import {
  ARTIFACT_DIR,
  openPresenterDrawer,
  startNewExploringSession,
} from './helpers';

test.describe('M6 Chapters II–IV, Distinct Era Mechanics, Provenance & Performance — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await startNewExploringSession(page);
  });

  test('M6-E2E-01: Chapter II (1945 Tân Trào -> Ba Đình) Map Traversal, Dispatch Network Mechanic & Climax', async ({
    page,
  }) => {
    // 1. Mở Ngăn Giảng Viên (Ctrl+Shift+P), chuyển sang Chương II (1945) và kiểm tra xuất phát tại tan_trao_banyan
    await openPresenterDrawer(page);
    await page.locator('[data-era-tab="1945"]').click();

    const initialMapId = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { getCurrentMapId(): string };
        }
      ).__LU_KHACH_GAME__!;
      return game.getCurrentMapId();
    });
    expect(initialMapId).toBe('tan_trao_banyan');
    await expect(page.locator('#stage-location-badge')).toContainText('Tân Trào');

    // 2. Gặp Giao Liên Việt Minh Hoàng Quốc Bảo (npc_tan_trao_messenger) -> chọn nhánh hội thoại -> Mốc IV
    const messengerPos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return { x: npc.x - 8, y: npc.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, messengerPos);

    await page.keyboard.press('KeyE');
    await expect(page.locator('[data-choice-id="c_ch2_ask_tw8"]')).toBeVisible();
    await page.locator('[data-choice-id="c_ch2_ask_tw8"]').click();

    const closeDlgBtn = page.locator('#btn-close-dialogue');
    if (await closeDlgBtn.isVisible()) {
      await closeDlgBtn.click();
    }

    // 3. Mở Bàn Điều Phối Mạng Lưới Giao Liên (puzzle_ch2_dispatch_network) và chọn 3 đáp án lịch sử xác thực -> Mốc V
    const puzzlePos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              interactables: Array<{ kind: string; x: number; y: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const p = game.currentMap.interactables.find((i) => i.kind === 'puzzle')!;
      return { x: p.x, y: p.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, puzzlePos);

    await page.keyboard.press('KeyE');
    const puzzleModal = page.locator('#dossier-puzzle-modal');
    await expect(puzzleModal).toBeVisible();
    await expect(page.locator('#dossier-puzzle-title')).toContainText(
      'ĐIỀU PHỐI MẠNG LƯỚI GIAO LIÊN & MẬT ĐIỆN TỔNG KHỞI NGHĨA (08/1945)'
    );

    await page.locator('[data-option-id="opt_ch2_time_authentic"]').click();
    await page.locator('[data-option-id="opt_ch2_route_authentic"]').click();
    await page.locator('[data-option-id="opt_ch2_state_authentic"]').click();
    await page.locator('#btn-submit-dossier').click();

    await expect(puzzleModal).toHaveClass(/is-solved/);
    await page.locator('#btn-close-dossier').click();

    // 4. Đi qua portal tới Quảng Trường Ba Đình (ba_dinh_square) và gặp Đội Viên Tự Vệ (npc_ba_dinh_witness) -> Mốc VI
    const portalRes = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            sceneTransition: {
              cooldownRemainingSec: number;
              requiresStepOffPortal: boolean;
            };
            currentMap: {
              portals: Array<{ destinationMap: string }>;
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
            enterPortal(portal: unknown): boolean;
            getCurrentMapId(): string;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.sceneTransition.cooldownRemainingSec = 0;
      game.sceneTransition.requiresStepOffPortal = false;
      const portal = game.currentMap.portals.find(
        (p) => p.destinationMap === 'ba_dinh_square'
      )!;
      const ok = game.enterPortal(portal);
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return {
        ok,
        mapId: game.getCurrentMapId(),
        npcPos: { x: npc.x - 8, y: npc.y },
      };
    });
    expect(portalRes.ok).toBe(true);
    expect(portalRes.mapId).toBe('ba_dinh_square');

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, portalRes.npcPos);

    await page.keyboard.press('KeyE');
    await expect(
      page.locator('[data-choice-id="c_badinh_ask_significance"]')
    ).toBeVisible();
    await page.locator('[data-choice-id="c_badinh_ask_significance"]').click();

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m6_ch2_ba_dinh_1945.png`,
      fullPage: false,
    });

    const ch2Milestones = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { collectedMilestones: Set<string> } };
        }
      ).__LU_KHACH_GAME__!;
      return Array.from(game.store.collectedMilestones);
    });
    expect(ch2Milestones).toEqual(expect.arrayContaining(['IV', 'V', 'VI']));
  });

  test('M6-E2E-02: Chapter III (1954–1975 Đèo Pha Đin -> Hành Lang Mùa Xuân 1975) Supply Logistics Mechanic & Traversal', async ({
    page,
  }) => {
    // 1. Mở Ngăn Giảng Viên (Ctrl+Shift+P), chuyển sang Chương III (1954–1975) và kiểm tra xuất phát tại pha_din_pass
    await openPresenterDrawer(page);
    await page.locator('[data-era-tab="1954_1975"]').click();

    const initialMapId = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { getCurrentMapId(): string };
        }
      ).__LU_KHACH_GAME__!;
      return game.getCurrentMapId();
    });
    expect(initialMapId).toBe('pha_din_pass');
    await expect(page.locator('#stage-location-badge')).toContainText('Pha Đin');

    // 2. Gặp Đội Trưởng Dân Công Hậu Cần (npc_pha_din_quartermaster) -> Mốc VII
    const quartermasterPos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return { x: npc.x - 8, y: npc.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, quartermasterPos);

    await page.keyboard.press('KeyE');
    await expect(page.locator('[data-choice-id="c_phadin_ask_1946"]')).toBeVisible();
    await page.locator('[data-choice-id="c_phadin_ask_1946"]').click();

    const closeDlgBtn = page.locator('#btn-close-dialogue');
    if (await closeDlgBtn.isVisible()) {
      await closeDlgBtn.click();
    }

    // 3. Mở Sa Bàn Tiếp Vận Pha Đin (puzzle_ch3_supply_logistics) và chọn 3 đáp án đúng -> Mốc VIII
    const puzzlePos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              interactables: Array<{ kind: string; x: number; y: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const p = game.currentMap.interactables.find((i) => i.kind === 'puzzle')!;
      return { x: p.x, y: p.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, puzzlePos);

    await page.keyboard.press('KeyE');
    const puzzleModal = page.locator('#dossier-puzzle-modal');
    await expect(puzzleModal).toBeVisible();
    await expect(page.locator('#dossier-puzzle-title')).toContainText(
      'SA BÀN ĐIỀU PHỐI TUYẾN TIẾP VẬN PHA ĐIN & CHIẾN LƯỢC HAI MIỀN (1954 – 1975)'
    );

    await page.locator('[data-option-id="opt_ch3_dbp_authentic"]').click();
    await page.locator('[data-option-id="opt_ch3_two_strat_authentic"]').click();
    await page.locator('[data-option-id="opt_ch3_1975_authentic"]').click();
    await page.locator('#btn-submit-dossier').click();

    await expect(puzzleModal).toHaveClass(/is-solved/);
    await page.locator('#btn-close-dossier').click();

    // 4. Đi qua portal tới Hành Lang Mùa Xuân 1975 (spring_1975_corridor) và gặp Chiến Sĩ Thông Tin (npc_1975_signal_officer) -> Mốc IX
    const portalRes = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            sceneTransition: {
              cooldownRemainingSec: number;
              requiresStepOffPortal: boolean;
            };
            currentMap: {
              portals: Array<{ destinationMap: string }>;
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
            enterPortal(portal: unknown): boolean;
            getCurrentMapId(): string;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.sceneTransition.cooldownRemainingSec = 0;
      game.sceneTransition.requiresStepOffPortal = false;
      const portal = game.currentMap.portals.find(
        (p) => p.destinationMap === 'spring_1975_corridor'
      )!;
      const ok = game.enterPortal(portal);
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return {
        ok,
        mapId: game.getCurrentMapId(),
        npcPos: { x: npc.x - 8, y: npc.y },
      };
    });
    expect(portalRes.ok).toBe(true);
    expect(portalRes.mapId).toBe('spring_1975_corridor');

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, portalRes.npcPos);

    await page.keyboard.press('KeyE');
    await expect(page.locator('[data-choice-id="c_1975_ask_victory"]')).toBeVisible();
    await page.locator('[data-choice-id="c_1975_ask_victory"]').click();

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m6_ch3_spring_1975.png`,
      fullPage: false,
    });

    const ch3Milestones = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { store: { collectedMilestones: Set<string> } };
        }
      ).__LU_KHACH_GAME__!;
      return Array.from(game.store.collectedMilestones);
    });
    expect(ch3Milestones).toEqual(expect.arrayContaining(['VII', 'VIII', 'IX']));
  });

  test('M6-E2E-03: Chapter IV (1986–2045 Đại Lộ Đổi Mới -> Quảng Trường Kỷ Nguyên Số 2045) Causal Policy Mechanic, Speculative 2045 Provenance & Frame-Time Performance Audit', async ({
    page,
  }) => {
    // 1. Mở Ngăn Giảng Viên (Ctrl+Shift+P), chuyển sang Chương IV (1986–2045) và kiểm tra xuất phát tại doi_moi_avenue
    await openPresenterDrawer(page);
    await page.locator('[data-era-tab="1986_2045"]').click();

    const initialMapId = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: { getCurrentMapId(): string };
        }
      ).__LU_KHACH_GAME__!;
      return game.getCurrentMapId();
    });
    expect(initialMapId).toBe('doi_moi_avenue');
    await expect(page.locator('#stage-location-badge')).toContainText('Đổi Mới');

    // 2. Gặp Chuyên Gia Nghiên Cứu Đổi Mới (npc_doi_moi_economist) -> Mốc X
    const economistPos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return { x: npc.x - 8, y: npc.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, economistPos);

    await page.keyboard.press('KeyE');
    await expect(page.locator('[data-choice-id="c_ch4_ask_doimoi"]')).toBeVisible();
    await page.locator('[data-choice-id="c_ch4_ask_doimoi"]').click();

    const closeDlgBtn = page.locator('#btn-close-dialogue');
    if (await closeDlgBtn.isVisible()) {
      await closeDlgBtn.click();
    }

    // 3. Mở Ma Trận Nhân Quả Quyết Sách Đổi Mới (puzzle_ch4_causal_policy) -> Mốc XI
    const puzzlePos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              interactables: Array<{ kind: string; x: number; y: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const p = game.currentMap.interactables.find((i) => i.kind === 'puzzle')!;
      return { x: p.x, y: p.y };
    });

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, puzzlePos);

    await page.keyboard.press('KeyE');
    const puzzleModal = page.locator('#dossier-puzzle-modal');
    await expect(puzzleModal).toBeVisible();
    await expect(page.locator('#dossier-puzzle-title')).toContainText(
      'MA TRẬN NHÂN QUẢ QUYẾT SÁCH ĐỔI MỚI & TẦM NHÌN 2045 (1986 – 2045)'
    );

    await page.locator('[data-option-id="opt_ch4_1986_authentic"]').click();
    await page.locator('[data-option-id="opt_ch4_2011_authentic"]').click();
    await page.locator('[data-option-id="opt_ch4_2045_authentic"]').click();
    await page.locator('#btn-submit-dossier').click();

    await expect(puzzleModal).toHaveClass(/is-solved/);
    await page.locator('#btn-close-dossier').click();

    // 4. Đi qua portal tới Quảng Trường Kỷ Nguyên Số (nexus_2045_plaza) và gặp Giám Tuyển Lưu Trữ 2045 (npc_archivist_2045)
    const portalRes = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            sceneTransition: {
              cooldownRemainingSec: number;
              requiresStepOffPortal: boolean;
            };
            currentMap: {
              portals: Array<{ destinationMap: string }>;
              npcs: Array<{ x: number; y: number; moveSpeed?: number }>;
            };
            enterPortal(portal: unknown): boolean;
            getCurrentMapId(): string;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.sceneTransition.cooldownRemainingSec = 0;
      game.sceneTransition.requiresStepOffPortal = false;
      const portal = game.currentMap.portals.find(
        (p) => p.destinationMap === 'nexus_2045_plaza'
      )!;
      const ok = game.enterPortal(portal);
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return {
        ok,
        mapId: game.getCurrentMapId(),
        npcPos: { x: npc.x - 8, y: npc.y },
      };
    });
    expect(portalRes.ok).toBe(true);
    expect(portalRes.mapId).toBe('nexus_2045_plaza');

    await page.evaluate(({ x, y }) => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            playerX: number;
            playerY: number;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;
      game.playerX = x;
      game.playerY = y;
      game.syncHudWithState();
    }, portalRes.npcPos);

    await page.keyboard.press('KeyE');

    // Provenance metadata is still available to the game, but isn't presented as a citation badge.
    const provenanceBadge = page.locator('#dialogue-provenance-badge');
    await expect(provenanceBadge).toBeHidden();
    await expect(provenanceBadge).toHaveAttribute(
      'data-provenance-type',
      'speculative_2045'
    );
    await expect(page.locator('#dialogue-citation')).toBeHidden();

    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m6_ch4_nexus_2045.png`,
      fullPage: false,
    });

    await expect(page.locator('[data-choice-id="c_2045_ask_xii"]')).toBeVisible();
    await page.locator('[data-choice-id="c_2045_ask_xii"]').click();

    // 5. Đo hiệu năng thời gian xử lý 30 bước vật lý & kết xuất liên tiếp (Frame-Time Performance Audit)
    const perfStats = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            updatePhysics(dt: number, timeSec: number): void;
            store: { collectedMilestones: Set<string> };
          };
        }
      ).__LU_KHACH_GAME__!;

      const t0 = performance.now();
      for (let i = 0; i < 30; i++) {
        game.updatePhysics(1 / 60, i / 60);
      }
      const totalMs = performance.now() - t0;
      return {
        avgStepMs: totalMs / 30,
        milestones: Array.from(game.store.collectedMilestones),
      };
    });

    expect(perfStats.avgStepMs).toBeLessThan(8);
    expect(perfStats.milestones).toEqual(expect.arrayContaining(['X', 'XI', 'XII']));
  });
});
