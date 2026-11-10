// @vitest-environment jsdom
/**
 * @file tests/unit/m12_eastward_dialogue.test.ts
 * @description Kiểm thử đơn vị cho Giao diện Hội thoại Pixel RPG phong cách Eastward (M12):
 * - Chia trang lời thoại ngắn 1–2 câu mỗi trang (splitDialogueIntoPages).
 * - Kiểm toán toàn bộ 12 cây hội thoại: nhãn vai trò ngắn gọn, nút lựa chọn súc tích (3–6 từ, không lặp nhãn "(Kết thúc)").
 * - Vẽ chân dung Pixel Art 40×40 cho đủ 12 nguyên mẫu nhân vật/cổ vật (resolveDialoguePortraitArchetype & drawDialoguePortrait).
 * - Đóng/mở ngăn Tư liệu lịch sử (#dialogue-lore-drawer) và chuyển trang trong DialoguePanel.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  type DialoguePortraitArchetype,
  drawDialoguePortrait,
  resolveDialoguePortraitArchetype,
} from '../../src/engine/SpriteAtlas';
import { DIALOGUE_TREES, DialogueSystem } from '../../src/systems/DialogueSystem';
import {
  DialoguePanel,
  splitDialogueIntoPages,
} from '../../src/ui/DialoguePanel';

describe('M12 Eastward-Inspired Pixel RPG Dialogue UI — Unit Suite', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="dialogue-modal" hidden>
        <div class="dialogue-floating-stack">
          <aside id="dialogue-lore-drawer" class="is-collapsed">
            <h3 id="dialogue-milestone-title" hidden></h3>
            <p id="dialogue-significance" hidden></p>
            <div id="dialogue-citation" hidden></div>
          </aside>
          <div id="dialogue-choices" hidden></div>
        </div>
        <article id="dialogue-parchment-card">
          <header id="dialogue-nameplate">
            <span id="dialogue-speaker"></span>
            <span id="dialogue-role"></span>
          </header>
          <div class="dialogue-meta-bar">
            <span id="dialogue-provenance-badge"></span>
            <button type="button" id="btn-toggle-dialogue-lore" aria-expanded="false"></button>
          </div>
          <div id="dialogue-portrait-frame">
            <canvas id="dialogue-portrait-canvas" width="40" height="40"></canvas>
          </div>
          <div class="dialogue-speech-column">
            <p id="dialogue-body-text"><span id="dialogue-text"></span></p>
            <div class="dialogue-bottom-bar">
              <span id="dialogue-page-indicator" hidden></span>
              <button type="button" id="btn-dialogue-next-page" hidden></button>
              <button type="button" id="btn-close-dialogue"></button>
            </div>
          </div>
        </article>
      </div>
    `;
  });

  it('splits dialogue text into 1–2 sentence beats per page and wraps each page in quotes', () => {
    const pages = splitDialogueIntoPages(
      '“Đồng chí tới đúng giờ lắm! Mưa đêm Hương Cảng đang giúp chúng ta che mắt mật thám. Từ năm 1925, báo Thanh Niên và cuốn Đường Kách Mệnh do Nguyễn Ái Quốc biên soạn đã theo tàu bí mật từ Quảng Châu qua bến này về nước.”'
    );

    expect(pages.length).toBe(2);
    expect(pages[0]).toBe(
      '“Đồng chí tới đúng giờ lắm! Mưa đêm Hương Cảng đang giúp chúng ta che mắt mật thám.”'
    );
    expect(pages[1]).toContain('Từ năm 1925, báo Thanh Niên');
  });

  it('audits all 12 dialogue trees for concise choice buttons, clean role labels, and 1–2 sentences per page', () => {
    expect(DIALOGUE_TREES.length).toBeGreaterThanOrEqual(12);

    for (const tree of DIALOGUE_TREES) {
      for (const node of Object.values(tree.nodes)) {
        // Role không chứa nhãn học thuật lặp lại như "(Nhân vật dẫn truyện hư cấu)"
        expect(node.role).not.toContain('(Nhân vật');
        expect(node.role.length).toBeLessThanOrEqual(38);

        // Mỗi trang sau khi tách không quá 2 câu
        const pages = splitDialogueIntoPages(node.text);
        expect(pages.length).toBeGreaterThanOrEqual(1);
        for (const page of pages) {
          const sentenceCount = page
            .replace(/^[“"']+|[”"']+$/g, '')
            .split(/(?<=[.!?…])\s+/u)
            .filter(Boolean).length;
          expect(sentenceCount).toBeLessThanOrEqual(2);
        }

        // Nút lựa chọn ngắn gọn, dễ quét mắt, không lặp hậu tố "(Kết thúc)"
        for (const choice of node.choices) {
          expect(choice.text).not.toContain('(Kết thúc)');
          expect(choice.text.length).toBeLessThanOrEqual(38);
        }
      }
    }
  });

  it('resolves and renders pixel-art bust portraits across all 12 NPC and artifact archetypes', () => {
    const archetypes: DialoguePortraitArchetype[] = [
      resolveDialoguePortraitArchetype('Giao Liên Lâm Văn Bính', 'Giao liên Cửu Long'),
      resolveDialoguePortraitArchetype('Bác Công Nhân Bến Cảng', 'Công nhân bốc xếp'),
      resolveDialoguePortraitArchetype('Người Gác Đèn Bến Cảng', 'Hoa tiêu đêm'),
      resolveDialoguePortraitArchetype('Thủ Kho Lưu Trữ Cửu Long', 'Thủ kho Kho Số 4'),
      resolveDialoguePortraitArchetype('Bà Hàng Nước Phố Cổ', 'Cơ sở cảnh giới'),
      resolveDialoguePortraitArchetype('Lão Chèo Đò Bến Thuyền', 'Người đưa đò'),
      resolveDialoguePortraitArchetype('Giao Liên Việt Minh Hoàng Quốc Bảo', 'Việt Bắc'),
      resolveDialoguePortraitArchetype('Đội Trưởng Dân Công Hậu Cần', 'Đèo Pha Đin'),
      resolveDialoguePortraitArchetype('Chiến Sĩ Thông Tin Chiến Dịch Hồ Chí Minh', '1975'),
      resolveDialoguePortraitArchetype('Chuyên Gia Nghiên Cứu Đổi Mới', '1986'),
      resolveDialoguePortraitArchetype('Giám Tuyển Lưu Trữ Quốc Gia 2045', '2045'),
      resolveDialoguePortraitArchetype('Bia Sử Liệu', 'Cột mốc', true),
    ];

    expect(new Set(archetypes).size).toBe(12);

    const fillRectSpy = vi.fn();
    const mockCtx = {
      save: vi.fn(),
      restore: vi.fn(),
      clearRect: vi.fn(),
      fillRect: fillRectSpy,
      strokeRect: vi.fn(),
      fillStyle: '',
      strokeStyle: '',
      lineWidth: 1,
      imageSmoothingEnabled: false,
    } as unknown as CanvasRenderingContext2D;

    for (const arch of archetypes) {
      fillRectSpy.mockClear();
      drawDialoguePortrait(mockCtx, arch, 0.5, true, false);
      expect(fillRectSpy).toHaveBeenCalled();
    }
  });

  it('keeps the historical lore drawer collapsed by default, toggles it on demand, and steps through pages', () => {
    const dialogueSystem = new DialogueSystem();
    const rootNode = dialogueSystem.startDialogue('dlg_harbor_liaison')!;
    const choices = dialogueSystem.getAvailableChoices(rootNode, new Set(), new Set());

    const panel = new DialoguePanel({});
    panel.openNpcDialogue(rootNode, choices);

    expect(panel.isOpen()).toBe(true);
    expect(panel.isLoreDrawerExpanded()).toBe(false);
    expect(panel.getCurrentArchetype()).toBe('liaison');
    expect(panel.getPageCount()).toBe(2);
    expect(panel.getCurrentPageIndex()).toBe(0);

    // Bảng tên hội thoại rút gọn theo phong cách RPG ("Anh Bính" / "Giao liên"), giữ nguyên tên đầy đủ trong tooltip
    const speakerEl = document.getElementById('dialogue-speaker')!;
    const roleEl = document.getElementById('dialogue-role')!;
    expect(speakerEl.textContent).toBe('Anh Bính');
    expect(speakerEl.title).toBe('Giao Liên Lâm Văn Bính');
    expect(roleEl.textContent).toBe('Giao liên');
    expect(roleEl.hidden).toBe(false);

    // Keep source provenance in game data, never as an on-screen badge or reference line.
    expect(document.getElementById('dialogue-provenance-badge')?.hidden).toBe(true);
    expect(document.getElementById('dialogue-citation')?.hidden).toBe(true);
    expect(document.getElementById('dialogue-citation')?.textContent).toBe('');

    // Chuyển sang trang 2
    panel.nextPage();
    expect(panel.getCurrentPageIndex()).toBe(1);
    expect(document.getElementById('dialogue-text')?.textContent).toContain(
      'Thanh Niên'
    );

    // Mở ngăn Tư liệu lịch sử
    panel.toggleLoreDrawer();
    expect(panel.isLoreDrawerExpanded()).toBe(true);
    expect(
      document.getElementById('btn-toggle-dialogue-lore')?.getAttribute('aria-expanded')
    ).toBe('true');

    panel.close();
    expect(panel.isOpen()).toBe(false);
    expect(panel.isLoreDrawerExpanded()).toBe(false);
  });
});
