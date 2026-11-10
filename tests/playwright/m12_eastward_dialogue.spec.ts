/**
 * @file tests/playwright/m12_eastward_dialogue.spec.ts
 * @description Kiểm thử End-to-End (Playwright Chromium) cho Giao diện Hội thoại Pixel RPG phong cách Eastward (M12):
 * - Hộp thoại Sơn Mài Đậm nhỏ gọn neo đáy (#dialogue-parchment-card cao <= 195px, rộng <= 760px).
 * - Khung chân dung Pixel Art 80×80px (#dialogue-portrait-canvas) sắc nét với dữ liệu pixel thực tế.
 * - Ngăn Tư liệu & Trích dẫn (#dialogue-lore-drawer) ẩn mặc định và mở rộng khi bấm #btn-toggle-dialogue-lore hoặc phím Tab.
 * - Phân trang lời thoại ngắn 1–2 câu (bấm #btn-dialogue-next-page hoặc phím Space) và chọn nhánh bằng phím số [1..3].
 */

import { expect, test } from '@playwright/test';
import { ARTIFACT_DIR, startNewExploringSession } from './helpers';

test.describe('M12 Eastward-Inspired Pixel RPG Dialogue UI — Browser E2E Suite', () => {
  test('M12-E2E-01: Compact Lacquer Speech Bar, Pixel Portrait, Pagination & Expandable Lore Drawer', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await startNewExploringSession(page);

    // Đứng cạnh Giao Liên Lâm Văn Bính tại harbor_square và mở hội thoại bằng phím E
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
    await expect(page.locator('#dialogue-modal')).toBeVisible();

    const metrics = await page.evaluate(() => {
      const card = document.getElementById('dialogue-parchment-card')!;
      const portraitCanvas = document.getElementById(
        'dialogue-portrait-canvas'
      ) as HTMLCanvasElement;
      const loreDrawer = document.getElementById('dialogue-lore-drawer')!;
      const choices = document.getElementById('dialogue-choices')!;
      const cardRect = card.getBoundingClientRect();
      const portraitRect = portraitCanvas.getBoundingClientRect();
      const drawerRect = loreDrawer.getBoundingClientRect();
      const choicesRect = choices.getBoundingClientRect();
      const cardStyle = window.getComputedStyle(card);
      const portraitStyle = window.getComputedStyle(portraitCanvas);

      const ctx = portraitCanvas.getContext('2d')!;
      const img = ctx.getImageData(0, 0, portraitCanvas.width, portraitCanvas.height).data;
      let nonZeroAlphaPixels = 0;
      for (let i = 3; i < img.length; i += 4) {
        if (img[i] > 0) nonZeroAlphaPixels += 1;
      }

      return {
        cardWidth: Math.round(cardRect.width),
        cardHeight: Math.round(cardRect.height),
        cardBg: cardStyle.backgroundColor,
        portraitCssWidth: Math.round(portraitRect.width),
        portraitCssHeight: Math.round(portraitRect.height),
        portraitLogicalWidth: portraitCanvas.width,
        portraitLogicalHeight: portraitCanvas.height,
        portraitImageRendering: portraitStyle.imageRendering,
        nonZeroAlphaPixels,
        drawerCollapsedByDefault: loreDrawer.classList.contains('is-collapsed'),
        drawerCollapsedHeight: Math.round(drawerRect.height),
        choicesAboveOrNearCardTop: choicesRect.bottom <= cardRect.top + 8,
      };
    });

    // 1. Hộp thoại nhỏ gọn neo đáy theo phong cách Eastward
    expect(metrics.cardWidth).toBeLessThanOrEqual(760);
    expect(metrics.cardHeight).toBeLessThanOrEqual(195);
    expect(metrics.cardBg).toBe('rgb(27, 22, 18)');

    // 2. Chân dung Pixel Art 80×80px (40×40 logical, nearest-neighbor)
    expect(metrics.portraitCssWidth).toBe(80);
    expect(metrics.portraitCssHeight).toBe(80);
    expect(metrics.portraitLogicalWidth).toBe(40);
    expect(metrics.portraitLogicalHeight).toBe(40);
    expect(['pixelated', 'crisp-edges']).toContain(metrics.portraitImageRendering);
    expect(metrics.nonZeroAlphaPixels).toBeGreaterThan(800);

    // 3. Ngăn Tư liệu & Trích dẫn ẩn mặc định, khay lựa chọn nằm gọn phía trên góc phải
    expect(metrics.drawerCollapsedByDefault).toBe(true);
    expect(metrics.drawerCollapsedHeight).toBe(0);
    expect(metrics.choicesAboveOrNearCardTop).toBe(true);

    // 4. Phân trang lời thoại ngắn 1–2 câu: Trang 1/2 -> bấm Space sang Trang 2/2
    await expect(page.locator('#dialogue-page-indicator')).toHaveText('Trang 1/2');
    await expect(page.locator('#btn-dialogue-next-page')).toBeVisible();
    await page.keyboard.press('Space');
    await expect(page.locator('#dialogue-page-indicator')).toHaveText('Trang 2/2');
    await expect(page.locator('#dialogue-text')).toContainText('Thanh Niên');

    // 5. Mở ngăn Tư liệu bằng phím Tab hoặc nút #btn-toggle-dialogue-lore
    await page.keyboard.press('Tab');
    await expect(page.locator('#dialogue-lore-drawer')).toHaveClass(/is-expanded/);
    const expandedHeight = await page.evaluate(() => {
      return Math.round(
        document.getElementById('dialogue-lore-drawer')!.getBoundingClientRect().height
      );
    });
    expect(expandedHeight).toBeGreaterThan(48);
    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_dialogue_lore_expanded.png`,
    });

    // Thu gọn lại để chụp ảnh hộp thoại Eastward gọn gàng
    await page.locator('#btn-toggle-dialogue-lore').click();
    await expect(page.locator('#dialogue-lore-drawer')).toHaveClass(/is-collapsed/);

    await page.screenshot({
      path: `${ARTIFACT_DIR}/qa_npc_dialogue_1920x1080.png`,
    });

    // 6. Chọn lựa chọn [1] bằng phím số 1
    await page.keyboard.press('Digit1');
    await expect(page.locator('#dialogue-provenance-badge')).toHaveAttribute(
      'data-provenance-type',
      'fictional_npc'
    );
  });
});
