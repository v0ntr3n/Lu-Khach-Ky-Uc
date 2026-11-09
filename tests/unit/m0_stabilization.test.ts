/**
 * @file tests/unit/m0_stabilization.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest + jsdom) cho Cột mốc M0 (Gameplay Stabilization).
 * Kiểm tra đầy đủ 9 nhóm lỗi P0/P1:
 * 1. Bảo vệ tiến trình khỏi dịch chuyển / bấm sidebar ở chế độ thường & cô lập Presenter Mode
 * 2. Kiểm tra điều kiện tiên quyết mở Cổng Thời Gian Climax (I, II, III)
 * 3. Sửa lỗi bất tử khi cúi người (crouch) ngoài vùng ẩn nấp + CollisionWorld AABB
 * 4. Khóa đầu vào di chuyển thế giới khi mở Modal/Drawer & chuẩn hóa vector 8 hướng
 * 5. Tự động xóa phím giữ khi mất tiêu điểm (window blur, visibilitychange, pointercancel)
 * 6. Chống race condition khi chuyển cảnh bất đồng bộ bằng transitionToken
 * 7. Chặn lặp phím hành động (key repeat suppression) cho phím E / Space / Q
 * 8. Di trú dữ liệu lưu trữ SaveDataV1 -> SaveDataV2 về điểm spawn an toàn có tên
 * 9. Kiểm tra tính toàn vẹn bản đồ với MapValidator (spawn, portal, collision, reachability)
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ALL_ROMAN_MILESTONES, GameStore } from '../../src/core/GameStore';
import { InputManager } from '../../src/core/InputManager';
import {
  DEFAULT_SAFE_MAP_ID,
  DEFAULT_SAFE_PLAYER_POS,
  DEFAULT_SAFE_SPAWN_ID,
  SAVE_KEY_V1,
  SAVE_KEY_V2,
  SaveManager,
} from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import type { MilestoneArtifact, SaveDataV1 } from '../../src/types/game';
import { HudController } from '../../src/ui/HudController';
import { CollisionWorld } from '../../src/world/CollisionWorld';
import { type ValidatableMap, validateMapData } from '../../src/world/MapValidator';

describe('M0 Stabilization — Unit & Integration Regression Suite', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    localStorage.clear();
  });

  describe('1. Sidebar / Teleport progression protection & Presenter Mode isolation', () => {
    it('does NOT add uncollected milestone to collectedMilestones when clicked in normal mode (isPresenterMode === false)', () => {
      document.body.innerHTML = `
        <div class="codex-sub">
          <span>Bấm để dịch chuyển</span>
          <strong id="codex-progress-count">0 / 12 CỘT MỐC</strong>
        </div>
        <div id="codex-milestone-list"></div>
        <div id="stage-toast"></div>
      `;

      const store = new GameStore();
      store.setMode('exploring');
      let playerX = 85;

      const hud = new HudController({
        onSelectEra: () => {},
        onTeleportToMilestone: (milestone: MilestoneArtifact) => {
          // Logic chuẩn M0: ở chế độ thường (isPresenterMode === false), không được tự mở khóa hay dịch chuyển tới mốc chưa thu thập
          if (store.isPresenterMode) {
            playerX = milestone.worldX;
            hud.showArtifactDialogue(milestone);
            return;
          }
          if (store.collectedMilestones.has(milestone.roman)) {
            hud.showArtifactDialogue(milestone);
          } else {
            hud.showToast(
              `Cột mốc ${milestone.roman} chưa được khôi phục. Hãy điều khiển Lữ Khách đến tận nơi và bấm [E].`
            );
          }
        },
        onTriggerClimaxStepB: () => {},
        onAdvanceNextChapter: () => {},
        onToggleLecturerUnlockAll: () => {},
        onToggleMute: () => false,
      });

      hud.renderSidebarCodex(CHAPTERS[0], store.getActiveMilestones(), null);

      const firstMilestoneBtn = document.querySelector<HTMLButtonElement>(
        '#codex-milestone-list .codex-item'
      );
      expect(firstMilestoneBtn).not.toBeNull();
      firstMilestoneBtn?.click();

      expect(store.isPresenterMode).toBe(false);
      expect(store.collectedMilestones.size).toBe(0);
      expect(store.getActiveMilestones().size).toBe(0);
      expect(playerX).toBe(85);
      expect(document.getElementById('codex-progress-count')?.textContent).toBe(
        '0 / 12 CỘT MỐC'
      );
    });

    it('isolates Presenter Mode unlocks into presenterCollectedMilestones and restores collectedMilestones when toggled off', () => {
      const store = new GameStore();
      store.collectMilestone('I');
      expect(store.collectedMilestones.has('I')).toBe(true);
      expect(store.collectedMilestones.size).toBe(1);

      // Bật Presenter Mode -> hiển thị đủ 12 mốc trong presenterCollectedMilestones
      const enabled = store.togglePresenterMode();
      expect(enabled).toBe(true);
      expect(store.isPresenterMode).toBe(true);
      expect(store.presenterCollectedMilestones.size).toBe(12);
      expect(store.getActiveMilestones().size).toBe(12);
      for (const roman of ALL_ROMAN_MILESTONES) {
        expect(store.getActiveMilestones().has(roman)).toBe(true);
      }
      // Tiến trình thực của người chơi vẫn giữ nguyên 1 mốc
      expect(store.collectedMilestones.size).toBe(1);

      // Tắt Presenter Mode -> khôi phục đúng tiến trình thực (chỉ có mốc 'I')
      const disabled = store.togglePresenterMode();
      expect(disabled).toBe(false);
      expect(store.isPresenterMode).toBe(false);
      expect(store.presenterCollectedMilestones.size).toBe(0);
      expect(store.getActiveMilestones().size).toBe(1);
      expect(store.getActiveMilestones().has('I')).toBe(true);
      expect(store.getActiveMilestones().has('II')).toBe(false);
    });
  });

  describe('2. Climax prerequisite check', () => {
    it('blocks Chapter I climax until milestones I, II, and III are all collected', () => {
      const store = new GameStore();
      const chapter1 = CHAPTERS[0];

      // Khi chưa thu thập mốc nào
      const res0 = store.canTriggerChapterClimax(chapter1);
      expect(res0).toEqual({
        allowed: false,
        missing: ['I', 'II', 'III'],
      });

      // Khi mới thu thập mốc 'I'
      store.collectMilestone('I');
      const res1 = store.canTriggerChapterClimax(chapter1);
      expect(res1).toEqual({
        allowed: false,
        missing: ['II', 'III'],
      });

      // Khi thu thập thêm mốc 'II'
      store.collectMilestone('II');
      const res2 = store.canTriggerChapterClimax(chapter1);
      expect(res2).toEqual({
        allowed: false,
        missing: ['III'],
      });

      // Khi thu thập đủ cả 'I', 'II', 'III'
      store.collectMilestone('III');
      const res3 = store.canTriggerChapterClimax(chapter1);
      expect(res3).toEqual({
        allowed: true,
        missing: [],
      });
    });
  });

  describe('3. Stealth crouch invulnerability fix & CollisionWorld geometry', () => {
    it('returns false when crouching in open hazard sweep area (playerX = 380) and true inside shelter [210, 295] (playerX = 250)', () => {
      const store = new GameStore();
      const shelters = CHAPTERS[0].shelters;

      // Cúi người giữa vùng đèn tuần tra quét qua (x = 380) KHÔNG được miễn nhiễm
      expect(store.isPlayerInShelter(380, shelters, true)).toBe(false);
      expect(store.isPlayerInShelter(380, shelters, false)).toBe(false);

      // Đứng trong Mái hiên phố Cửu Long [210, 295] (x = 250) được bảo vệ
      expect(store.isPlayerInShelter(250, shelters, false)).toBe(true);
      expect(store.isPlayerInShelter(250, shelters, true)).toBe(true);
    });

    it('validates CollisionWorld.isPointInShelter and axis-separated resolveMovement', () => {
      const obstacles = [{ x: 100, y: 100, width: 32, height: 32 }];
      const shelters = [
        ...CHAPTERS[0].shelters,
        {
          xStart: 400,
          xEnd: 460,
          label: 'Hiên nhà 2D',
          bounds: { x: 400, y: 200, width: 60, height: 40 },
        },
      ];
      const world = new CollisionWorld(obstacles, shelters, {
        x: 0,
        y: 0,
        width: 1024,
        height: 768,
      });

      expect(world.isPointInShelter(380)).toBe(false);
      expect(world.isPointInShelter(250)).toBe(true);
      expect(world.isPointInShelter(420, 220)).toBe(true);
      expect(world.isPointInShelter(420, 300)).toBe(false);

      // Kiểm tra resolveMovement trượt dọc theo vật cản (sliding along wall)
      // Nhân vật có footRect tại (84, 108, 16, 16) di chuyển chéo (+4, +6) đụng tường ở x = 100
      const startFoot = { x: 84, y: 108, width: 16, height: 16 };
      const resolved = world.resolveMovement(startFoot, 4, 6);
      // Trục X bị chặn tại 84 (vì 84 + 4 + 16 = 104 > 100), nhưng trục Y vẫn trượt tự do tới 114
      expect(resolved.x).toBe(84);
      expect(resolved.y).toBe(114);
    });
  });

  describe('4. Modal input locking & 8-way diagonal vector normalization', () => {
    it('returns { x: 0, y: 0 } when allowWorldInput is false (mode !== exploring) even while D or ArrowRight is held', () => {
      const store = new GameStore();
      const input = new InputManager();

      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));

      store.setMode('lookup');
      expect(store.isWorldInputAllowed()).toBe(false);
      expect(input.getMoveVector(store.isWorldInputAllowed())).toEqual({ x: 0, y: 0 });
      expect(input.isMoving(false)).toBe(false);

      store.setMode('climax');
      expect(input.getMoveVector(store.isWorldInputAllowed())).toEqual({ x: 0, y: 0 });

      store.setMode('exploring');
      expect(store.isWorldInputAllowed()).toBe(true);
      expect(input.getMoveVector(store.isWorldInputAllowed())).toEqual({ x: 1, y: 0 });

      input.destroy();
    });

    it('normalizes diagonal movement (W + D) to unit magnitude (~0.7071, -0.7071)', () => {
      const input = new InputManager();

      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));

      const vec = input.getMoveVector(true);
      expect(vec.x).toBeCloseTo(Math.SQRT1_2, 4);
      expect(vec.y).toBeCloseTo(-Math.SQRT1_2, 4);
      expect(Math.hypot(vec.x, vec.y)).toBeCloseTo(1.0, 5);

      input.destroy();
    });
  });

  describe('5. Focus loss (blur) and pointercancel cleanup', () => {
    it('clears held keys immediately on window blur or pointercancel on #ctrl-right', () => {
      document.body.innerHTML = `
        <button id="ctrl-left" type="button"></button>
        <button id="ctrl-right" type="button"></button>
        <button id="ctrl-shelter" type="button"></button>
      `;

      const input = new InputManager();

      // Giữ phím 'd' rồi mất tiêu điểm cửa sổ (window blur)
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
      expect(input.getMoveVector(true)).toEqual({ x: 1, y: 0 });

      window.dispatchEvent(new Event('blur'));
      expect(input.getMoveVector(true)).toEqual({ x: 0, y: 0 });

      // Giữ nút cảm ứng #ctrl-right bằng pointerdown rồi xảy ra pointercancel
      const ctrlRight = document.getElementById('ctrl-right')!;
      ctrlRight.dispatchEvent(new Event('pointerdown'));
      expect(input.getMoveVector(true)).toEqual({ x: 1, y: 0 });

      ctrlRight.dispatchEvent(new Event('pointercancel'));
      expect(input.getMoveVector(true)).toEqual({ x: 0, y: 0 });

      input.destroy();
    });
  });

  describe('6. Scene transition race condition (transitionToken)', () => {
    it('invalidates previous transition tokens when bumpTransitionToken() is called so stale timers abort', () => {
      vi.useFakeTimers();
      const store = new GameStore();
      store.currentChapterIndex = 0;

      // Bắt đầu hiệu ứng chuyển cảnh ở Chương I
      const tokenAtStart = store.bumpTransitionToken();
      let chapterAdvancedByStaleTimer = false;

      window.setTimeout(() => {
        if (store.getTransitionToken() !== tokenAtStart) {
          return;
        }
        chapterAdvancedByStaleTimer = true;
        store.currentChapterIndex += 1;
      }, 1600);

      // Người chơi chuyển chương thủ công trước khi timer 1600ms kịp nổ
      store.bumpTransitionToken();
      store.currentChapterIndex = 2;

      vi.advanceTimersByTime(2000);
      expect(chapterAdvancedByStaleTimer).toBe(false);
      expect(store.currentChapterIndex).toBe(2);

      vi.useRealTimers();
    });
  });

  describe('7. Key repeat suppression on action keys', () => {
    it('fires onInteract callback only ONCE when key E is held down with repeat: true or repeated keydowns without keyup', () => {
      const onInteract = vi.fn();
      const input = new InputManager({ onInteract });

      // Lần nhấn đầu tiên
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', repeat: false }));
      // Các sự kiện lặp phím từ hệ điều hành hoặc gửi liên tiếp khi chưa nhả phím
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', repeat: true }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', repeat: true }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'E', repeat: false }));

      expect(onInteract).toHaveBeenCalledTimes(1);

      // Sau khi nhả phím (keyup), nhấn lại mới kích hoạt lần thứ 2
      window.dispatchEvent(new KeyboardEvent('keyup', { key: 'e' }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', repeat: false }));
      expect(onInteract).toHaveBeenCalledTimes(2);

      input.destroy();
    });
  });

  describe('8. SaveManager v1 -> v2 migration & Presenter Mode exclusion', () => {
    it('safely converts legacy SaveDataV1 scalar playerX into SaveDataV2 at a named safe spawn without presenter state', () => {
      const legacyV1: SaveDataV1 = {
        version: 1,
        chapterId: '1945',
        playerX: 99999, // Tọa độ scalar cũ có nguy cơ kẹt trong vật cản ở bản đồ 2D mới
        collectedMilestones: ['I', 'II', 'III', 'II'],
      };

      const migrated = SaveManager.migrateV1ToV2(legacyV1);

      expect(migrated.version).toBe(2);
      expect(migrated.eraId).toBe('1945');
      expect(migrated.currentMapId).toBe(DEFAULT_SAFE_MAP_ID);
      expect(migrated.spawnId).toBe(DEFAULT_SAFE_SPAWN_ID);
      expect(migrated.playerPos).toEqual(DEFAULT_SAFE_PLAYER_POS);
      expect(migrated.collectedMilestones).toEqual(['I', 'II', 'III']);

      // Kiểm tra tích hợp với GameStore: bật Presenter Mode không làm bẩn dữ liệu SaveDataV2
      const store = new GameStore();
      store.collectMilestone('I');
      store.togglePresenterMode();
      expect(store.getActiveMilestones().size).toBe(12);

      const savedV2 = store.toSaveDataV2('1930', 120);
      expect(savedV2.collectedMilestones).toEqual(['I']);

      // Kiểm tra tự động đọc và nâng cấp từ localStorage SAVE_KEY_V1
      localStorage.setItem(SAVE_KEY_V1, JSON.stringify(legacyV1));
      const loadedFromStorage = SaveManager.load();
      expect(loadedFromStorage).not.toBeNull();
      expect(loadedFromStorage?.version).toBe(2);
      expect(loadedFromStorage?.spawnId).toBe(DEFAULT_SAFE_SPAWN_ID);
      expect(localStorage.getItem(SAVE_KEY_V1)).toBeNull();
      expect(localStorage.getItem(SAVE_KEY_V2)).not.toBeNull();
    });
  });

  describe('9. MapValidator spawn, portal, and collision overlap detection', () => {
    it('passes a valid connected map pair and detects spawn-in-obstacle, broken portal target, and unreachable portal', () => {
      const harborMap: ValidatableMap = {
        id: 'harbor_square',
        width: 1024,
        height: 768,
        tileSize: 16,
        defaultSpawnId: 'default_spawn',
        spawns: [
          { id: 'default_spawn', x: 64, y: 64, facing: 'right' },
          { id: 'from_warehouse', x: 480, y: 400, facing: 'down' },
        ],
        portals: [
          {
            id: 'exit_to_warehouse',
            x: 512,
            y: 400,
            width: 32,
            height: 16,
            destinationMap: 'ch1_warehouse',
            destinationSpawn: 'warehouse_south_door',
          },
        ],
        interactables: [
          {
            id: 'npc_courier',
            x: 128,
            y: 128,
            width: 16,
            height: 16,
            kind: 'npc',
            label: 'Giao Liên Hương Cảng',
            actionId: 'talk_courier',
          },
        ],
        obstacles: [{ x: 256, y: 256, width: 64, height: 64 }],
      };

      const warehouseMap: ValidatableMap = {
        id: 'ch1_warehouse',
        width: 576,
        height: 448,
        tileSize: 16,
        defaultSpawnId: 'warehouse_south_door',
        spawns: [{ id: 'warehouse_south_door', x: 256, y: 384, facing: 'up' }],
        portals: [
          {
            id: 'exit_to_harbor',
            x: 256,
            y: 416,
            width: 32,
            height: 16,
            destinationMap: 'harbor_square',
            destinationSpawn: 'from_warehouse',
          },
        ],
        interactables: [],
        obstacles: [],
      };

      const allMaps = {
        harbor_square: harborMap,
        ch1_warehouse: warehouseMap,
      };

      const validResult = validateMapData(harborMap, allMaps);
      expect(validResult.valid).toBe(true);
      expect(validResult.errors).toHaveLength(0);

      // Trường hợp lỗi: spawn nằm đè lên vật cản & portal trỏ tới spawn không tồn tại
      const brokenMap: ValidatableMap = {
        ...harborMap,
        id: 'broken_map',
        spawns: [{ id: 'trapped_spawn', x: 260, y: 260 }],
        portals: [
          {
            id: 'broken_portal',
            x: 512,
            y: 400,
            width: 32,
            height: 16,
            destinationMap: 'ch1_warehouse',
            destinationSpawn: 'non_existent_spawn',
          },
        ],
      };

      const brokenResult = validateMapData(brokenMap, allMaps);
      expect(brokenResult.valid).toBe(false);
      expect(
        brokenResult.errors.some((e) => e.includes('overlaps a collision obstacle'))
      ).toBe(true);
      expect(
        brokenResult.errors.some((e) => e.includes('missing destinationSpawn'))
      ).toBe(true);
    });
  });
});
