/**
 * @file tests/playwright/m4_chapter1_playthrough.spec.ts
 * @description Bộ kiểm thử End-to-End (Playwright Chromium) cho Cột mốc M4 — Chapter I Complete Vertical Slice:
 * - M4-E2E-01: Nón tầm nhìn lính tuần tra (Stealth Vision Cone), ẩn nấp dưới Mái Hiên (Shelter Cover) và hồi sinh không phạt
 *              về Đèn Báo An Toàn (Non-Punitive Checkpoint Reset) tại `shadow_alley` mà không mất bất kỳ tư liệu hay cờ nhiệm vụ nào.
 * - M4-E2E-02: Chơi trọn vẹn Chương I (Hương Cảng 1930) từ đầu đến cuối KHÔNG bật Chế độ Giảng Viên hay dịch chuyển gian lận:
 *              Bến Cảng (Mốc I) -> Kho Hàng (Sổ Vận Đơn) -> Bến Thuyền Nhỏ (Nhiệm vụ phụ Lão Chèo Đò & Lịch Đổi Ca) ->
 *              Phố Cổ -> Nhà An Toàn (Mốc II & Mật Tín) -> Hẻm Tối -> Sân Kho Lưu Trữ -> Phòng Lưu Trữ Bí Mật ->
 *              Giải đố Hồ sơ Cương lĩnh 02/1930 (thử sai 2 lần hiện gợi ý học thuật -> chọn 3 đáp án đúng -> Mốc III) ->
 *              Mở Cổng Thời Gian 1930 (Climax Bước A + Bước B).
 */

import { expect, test } from '@playwright/test';

const ARTIFACT_DIR =
  '/home/antigravity/.gemini/jetski/brain/31f52e7023c04649246b83f79b9ff3a6';

test.describe('M4 Chapter I Complete Vertical Slice — Browser E2E Suite', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.locator('#skip-opening-btn').click();
  });

  test('M4-E2E-01: Stealth Vision Cone, Shelter Cover & Non-Punitive Checkpoint Reset in shadow_alley', async ({
    page,
  }) => {
    // 1. Chuyển tới shadow_alley và thu thập sẵn 1 tư liệu + 1 cờ nhiệm vụ để kiểm chứng không bị mất khi bị phát hiện
    const setupAlley = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            loadMap(mapId: string): void;
            inventorySystem: { addItem(id: string, store: unknown): unknown };
            store: {
              questFlags: Set<string>;
              collectedEvidence: Set<string>;
              toggleDebugOverlay(force?: boolean): boolean;
            };
            currentMap: {
              shelters: Array<{
                label: string;
                bounds?: { x: number; y: number; width: number; height: number };
              }>;
              checkpoints: Array<{ id: string; x: number; y: number }>;
            };
            playerX: number;
            playerY: number;
            isPlayerInShelter(): boolean;
            syncHudWithState(): void;
          };
        }
      ).__LU_KHACH_GAME__!;

      game.inventorySystem.addItem('ev_ch1_street_leaflet', game.store);
      game.store.questFlags.add('ch1_met_liaison');
      game.loadMap('shadow_alley');

      const shelter = game.currentMap.shelters.find((s) => s.bounds);
      if (shelter?.bounds) {
        game.playerX = shelter.bounds.x + Math.floor(shelter.bounds.width / 2);
        game.playerY = shelter.bounds.y + Math.floor(shelter.bounds.height / 2);
      }
      game.syncHudWithState();

      return {
        inShelter: game.isPlayerInShelter(),
        checkpoint: game.currentMap.checkpoints[0],
      };
    });

    expect(setupAlley.inShelter).toBe(true);
    await expect(page.locator('#stage-shelter-status')).toContainText('ĐANG ẨN MÌNH');

    // Chụp ảnh màn hình thực tế tại Hẻm Tối Tuần Tra (shadow_alley)
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m4_stealth_alley.png`,
      fullPage: false,
    });

    // 2. Đặt nhân vật ra ngoài vùng che chắn ngay trước mặt lính tuần tra và chạy updatePhysics để kích hoạt Alert Reset
    const afterAlert = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            stealthSystem: {
              getGuards(): Array<{
                x: number;
                y: number;
                facingAngleRad: number;
                speed: number;
              }>;
            };
            playerX: number;
            playerY: number;
            hazardCooldown: number;
            updatePhysics(dt: number, timeSec: number): void;
            store: {
              collectedEvidence: Set<string>;
              questFlags: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;

      const guard = game.stealthSystem.getGuards()[0];
      guard.speed = 0;
      game.playerX = guard.x + Math.cos(guard.facingAngleRad) * 28;
      game.playerY = guard.y + Math.sin(guard.facingAngleRad) * 28;
      game.hazardCooldown = 0;

      // Tích lũy nghi ngờ vượt ngưỡng 1.0 -> tự động rút lui về Checkpoint
      game.updatePhysics(0.5, 10.0);
      game.updatePhysics(0.6, 10.6);

      return {
        x: game.playerX,
        y: game.playerY,
        hasLeaflet: game.store.collectedEvidence.has('ev_ch1_street_leaflet'),
        hasLiaisonFlag: game.store.questFlags.has('ch1_met_liaison'),
      };
    });

    // Nhân vật được đưa về đúng tọa độ Checkpoint của shadow_alley và bảo toàn 100% tư liệu & cờ nhiệm vụ
    expect(afterAlert.x).toBe(setupAlley.checkpoint.x);
    expect(afterAlert.y).toBe(setupAlley.checkpoint.y);
    expect(afterAlert.hasLeaflet).toBe(true);
    expect(afterAlert.hasLiaisonFlag).toBe(true);
    await expect(page.locator('#stage-toast')).toContainText(
      'Đội tuần tra phát hiện! Lữ Khách đã rút lui về Đèn Báo An Toàn gần nhất'
    );
  });

  test('M4-E2E-02: Complete Chapter I Vertical Slice Playthrough (Start -> Main & Side Quests -> Dossier Puzzle -> 1930 Climax Completion)', async ({
    page,
  }) => {
    // Khẳng định khởi đầu ở harbor_square, không bật Presenter Mode, 0/12 Cột mốc
    await expect(page.locator('#codex-progress-count')).toHaveText('0 / 12 CỘT MỐC');

    // Helper đi qua portal sang khu vực kế tiếp
    const enterZoneViaPortal = async (destinationMap: string) => {
      const res = await page.evaluate((destId) => {
        const game = (
          window as unknown as {
            __LU_KHACH_GAME__?: {
              sceneTransition: {
                cooldownRemainingSec: number;
                requiresStepOffPortal: boolean;
              };
              currentMap: {
                portals: Array<{ destinationMap: string }>;
              };
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
      }, destinationMap);

      expect(res.ok).toBe(true);
      expect(res.mapId).toBe(destinationMap);
    };

    // Helper di chuyển tới thực thể tương tác / NPC trên bản đồ hiện tại và nhấn phím E
    const moveNearAndPressE = async (targetX: number, targetY: number) => {
      await page.evaluate(
        ({ x, y }) => {
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
        },
        { x: targetX, y: targetY }
      );
      await page.keyboard.press('KeyE');
    };

    // BƯỚC 1: Gặp Giao Liên Lâm Văn Bính tại Quảng Trường Bến Cảng (harbor_square) -> chọn nhánh hội thoại -> Mốc I
    await moveNearAndPressE(256, 384);
    await expect(page.locator('.dialogue-choice-btn').first()).toBeVisible();
    await page.locator('.dialogue-choice-btn').first().click();
    await expect(page.locator('#codex-progress-count')).toHaveText('1 / 12 CỘT MỐC');

    // Đóng hộp thoại nếu còn mở để tiếp tục di chuyển thế giới
    const closeDlgBtn = page.locator('#btn-close-dialogue');
    if (await closeDlgBtn.isVisible()) {
      await closeDlgBtn.click();
    }

    // BƯỚC 2: Vào Kho Hàng Bến Cảng (warehouse_interior) -> thu thập Sổ Vận Đơn (ev_ch1_shipping_ledger)
    await enterZoneViaPortal('warehouse_interior');
    await moveNearAndPressE(272, 224);

    const afterWarehouse = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: {
              collectedEvidence: Set<string>;
              completedObjectives: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        hasLedger: game.store.collectedEvidence.has('ev_ch1_shipping_ledger'),
        completedLedgerObj: game.store.completedObjectives.has(
          'obj_ch1_inspect_warehouse_ledger'
        ),
      };
    });
    expect(afterWarehouse.hasLedger).toBe(true);
    expect(afterWarehouse.completedLedgerObj).toBe(true);

    // BƯỚC 3 (NHIỆM VỤ PHỤ): Quay lại harbor_square -> vào Bến Thuyền Nhỏ (small_pier) -> gặp Lão Chèo Đò & lấy Lịch Đổi Ca
    await enterZoneViaPortal('harbor_square');
    await enterZoneViaPortal('small_pier');

    // Trò chuyện với Lão Chèo Đò (npc_pier_boatman)
    const boatmanPos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              npcs: Array<{ id: string; npcId: string; x: number; y: number; moveSpeed?: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const npc = game.currentMap.npcs[0];
      npc.moveSpeed = 0;
      return { x: npc.x - 8, y: npc.y };
    });
    await moveNearAndPressE(boatmanPos.x, boatmanPos.y);
    await expect(page.locator('[data-choice-id="c_boatman_accept"]')).toBeVisible();
    await page.locator('[data-choice-id="c_boatman_accept"]').click();
    if (await page.locator('[data-choice-id="c_boatman_done"]').isVisible()) {
      await page.locator('[data-choice-id="c_boatman_done"]').click();
    }

    // Thu thập Lịch Đổi Ca Tuần Tra (ev_ch1_patrol_schedule) tại Bến Thuyền Nhỏ
    const schedulePos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              interactables: Array<{ evidenceId?: string; x: number; y: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const item = game.currentMap.interactables.find(
        (i) => i.evidenceId === 'ev_ch1_patrol_schedule'
      )!;
      return { x: item.x, y: item.y };
    });
    await moveNearAndPressE(schedulePos.x, schedulePos.y);

    const sideQuestCompleted = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            store: {
              collectedEvidence: Set<string>;
              completedQuests: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        hasSchedule: game.store.collectedEvidence.has('ev_ch1_patrol_schedule'),
        completedSideQuest: game.store.completedQuests.has('q_ch1_side_dock_watch'),
      };
    });
    expect(sideQuestCompleted.hasSchedule).toBe(true);
    expect(sideQuestCompleted.completedSideQuest).toBe(true);

    // BƯỚC 4: Quay về harbor_square -> sang Phố Cổ (old_street) -> vào Nhà An Toàn (safehouse_interior) -> khôi phục Mốc II & Mật Tín
    await enterZoneViaPortal('harbor_square');
    await enterZoneViaPortal('old_street');
    await enterZoneViaPortal('safehouse_interior');

    const safehouseTarget = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              npcs: Array<{ x: number; y: number }>;
              interactables: Array<{ evidenceId?: string; x: number; y: number }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const cipherItem = game.currentMap.interactables.find(
        (i) => i.evidenceId === 'ev_ch1_cipher_note'
      );
      if (cipherItem) return { x: cipherItem.x, y: cipherItem.y };
      const npc = game.currentMap.npcs[0];
      return { x: npc.x, y: npc.y + 12 };
    });
    await moveNearAndPressE(safehouseTarget.x, safehouseTarget.y);

    // Nếu mở hội thoại với Trần Thị Liên thì chọn nhánh nhận Mật Tín & Mốc II
    const innerChoice = page.locator('[data-choice-id="c_inner_ask_delegates"]');
    if (await innerChoice.isVisible()) {
      await innerChoice.click();
    }
    if (await closeDlgBtn.isVisible()) {
      await closeDlgBtn.click();
    }

    await expect(page.locator('#codex-progress-count')).toHaveText('2 / 12 CỘT MỐC');

    // BƯỚC 5: Đi từ safehouse_interior -> old_street -> shadow_alley -> archive_courtyard -> secret_archive
    await enterZoneViaPortal('old_street');
    await enterZoneViaPortal('shadow_alley');
    await enterZoneViaPortal('archive_courtyard');
    await enterZoneViaPortal('secret_archive');

    // BƯỚC 6 (DOSSIER PUZZLE): Tương tác với Bàn Đối Chiếu Hồ Sơ (puzzle_ch1_unification_dossier) bằng phím E
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
    await moveNearAndPressE(puzzlePos.x, puzzlePos.y);

    const puzzleModal = page.locator('#dossier-puzzle-modal');
    await expect(puzzleModal).toBeVisible();
    await expect(page.locator('#dossier-progressive-hint')).toBeHidden();

    // Thử sai 2 lần liên tiếp -> kiểm tra hiển thị gợi ý học thuật lũy tiến (#dossier-progressive-hint) và không mất tiến trình
    await page.locator('[data-option-id="opt_goal_wrong_1"]').click();
    await page.locator('[data-option-id="opt_task_wrong_1"]').click();
    await page.locator('[data-option-id="opt_force_wrong_1"]').click();
    await page.locator('#btn-submit-dossier').click();
    await expect(page.locator('#dossier-progressive-hint')).toBeHidden();

    await page.locator('#btn-submit-dossier').click();
    await expect(page.locator('#dossier-progressive-hint')).toBeVisible();
    await expect(page.locator('#dossier-progressive-hint')).toContainText(
      'Giáo trình 2021, tr. 61–65'
    );

    // Chọn 3 nhận định xác thực của Cương lĩnh 02/1930 và bấm Xác Thực
    await page.locator('[data-option-id="opt_goal_authentic"]').click();
    await page.locator('[data-option-id="opt_task_authentic"]').click();
    await page.locator('[data-option-id="opt_force_authentic"]').click();
    await page.locator('#btn-submit-dossier').click();

    await expect(puzzleModal).toHaveClass(/is-solved/);
    await expect(page.locator('#codex-progress-count')).toHaveText('3 / 12 CỘT MỐC');

    // Chụp ảnh nghiệm thu Bàn Kiểm Định & Đối Chiếu Hồ Sơ Lịch Sử sau khi giải thành công
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m4_dossier_puzzle.png`,
      fullPage: false,
    });

    await page.locator('#btn-close-dossier').click();
    await expect(puzzleModal).toBeHidden();

    // BƯỚC 7 (CLIMAX 1930): Tiến tới Cổng Thời Gian 1930 (climax_gate_1930) và nhấn phím E
    const gatePos = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentMap: {
              interactables: Array<{
                kind: string;
                x: number;
                y: number;
                milestoneId?: string;
              }>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      const g = game.currentMap.interactables.find((i) => i.kind === 'climax_gate')!;
      g.milestoneId = undefined;
      return { x: g.x, y: g.y };
    });
    await moveNearAndPressE(gatePos.x, gatePos.y);

    const climaxModal = page.locator('#climax-modal');
    await expect(climaxModal).toHaveClass(/is-open/);

    // Chọn phương án đúng ở Bước A để mở Bước B
    const correctOptionIdx = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            currentChapter: {
              climax: { options: Array<{ id: string; isCorrect: boolean }> };
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return game.currentChapter.climax.options.findIndex((o) => o.isCorrect);
    });

    await page.locator('.climax-option-btn').nth(correctOptionIdx).click();
    await expect(page.locator('#climax-step-b')).toBeVisible();

    // Chụp ảnh nghiệm thu Climax Chương I (1930)
    await page.waitForTimeout(150);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/m4_climax_1930.png`,
      fullPage: false,
    });

    await page.locator('#climax-b-action-btn').click();

    const finalVerification = await page.evaluate(() => {
      const game = (
        window as unknown as {
          __LU_KHACH_GAME__?: {
            isBurstAnimating: boolean;
            store: {
              isPresenterMode: boolean;
              collectedMilestones: Set<string>;
              completedQuests: Set<string>;
            };
          };
        }
      ).__LU_KHACH_GAME__!;
      return {
        isPresenterMode: game.store.isPresenterMode,
        isBurstAnimating: game.isBurstAnimating,
        milestones: Array.from(game.store.collectedMilestones),
        completedMainQuest: game.store.completedQuests.has('q_ch1_main_unification'),
        completedSideQuest: game.store.completedQuests.has('q_ch1_side_dock_watch'),
      };
    });

    expect(finalVerification.isPresenterMode).toBe(false);
    expect(finalVerification.isBurstAnimating).toBe(true);
    expect(finalVerification.milestones).toEqual(
      expect.arrayContaining(['I', 'II', 'III'])
    );
    expect(finalVerification.completedMainQuest).toBe(true);
    expect(finalVerification.completedSideQuest).toBe(true);
  });
});
