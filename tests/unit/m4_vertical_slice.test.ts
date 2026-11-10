/**
 * @file tests/unit/m4_vertical_slice.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest) cho Cột mốc M4 — Chapter I Complete Vertical Slice:
 * 1. StealthSystem: kiểm tra hình học nón tầm nhìn (isPointInVisionCone), che chắn bởi ShelterZone & tường/occluder (CollisionWorld.hasLineOfSight),
 *    khẳng định đứng ngoài vùng che chắn luôn bị phát hiện (không miễn nhiễm khi chỉ cúi người),
 *    giảm bán kính & tốc độ nghi ngờ khi có Lịch Đổi Ca (hasScheduleBonus), và vòng đời trạng thái calm -> suspicious -> alert.
 * 2. CH1_DOSSIER_PUZZLE & CH1_PROVENANCE_MANIFEST: kiểm tra 3 ô ghép hồ sơ Cương lĩnh 02/1930, gợi ý học thuật lũy tiến,
 *    và phân biệt rạch ròi giữa historical_fact (có số trang Giáo trình 2021) với fictional_npc.
 */

import { describe, expect, it } from 'vitest';
import { CH1_DOSSIER_PUZZLE } from '../../src/data/chapters/ch1_1930';
import { CH1_PROVENANCE_MANIFEST } from '../../src/data/provenance/ch1_provenance';
import { getBuiltInMap } from '../../src/runtime/MapLoader';
import {
  type PatrolRuntimeState,
  StealthSystem,
} from '../../src/systems/StealthSystem';
import { CollisionWorld } from '../../src/world/CollisionWorld';

describe('M4 Chapter I Vertical Slice — StealthSystem, Dossier Puzzle & Provenance Suite', () => {
  describe('1. StealthSystem Vision Cones, Cover & Occluders', () => {
    it('accurately evaluates vision cone geometry, shelter cover, wall/occluder Line-of-Sight, schedule bonus, and calm -> suspicious -> alert transitions', () => {
      const stealth = new StealthSystem();

      // Nón nhìn hướng sang phải (angle = 0), dài 100px, góc mở 60 độ (PI / 3)
      expect(
        stealth.isPointInVisionCone(160, 200, 100, 200, 0, 100, Math.PI / 3)
      ).toBe(true);
      // Ngoài tầm xa (dist = 120 > 100)
      expect(
        stealth.isPointInVisionCone(220, 200, 100, 200, 0, 100, Math.PI / 3)
      ).toBe(false);
      // Ngoài góc mở (vuông góc 90 độ > 30 độ nửa góc mở)
      expect(
        stealth.isPointInVisionCone(100, 270, 100, 200, 0, 100, Math.PI / 3)
      ).toBe(false);

      const guard: PatrolRuntimeState = {
        id: 'guard_test',
        label: 'Lính Tuần Tra Hẻm Tối',
        warningToast: 'Bị phát hiện!',
        x: 100,
        y: 200,
        speed: 0,
        waypoints: [{ x: 100, y: 200 }],
        waypointIndex: 0,
        waitRemainingSec: 0,
        facingAngleRad: 0,
        coneLength: 100,
        coneAngleRad: Math.PI / 3,
        alertState: 'calm',
        suspicionLevel: 0,
      };

      const emptyWorld = new CollisionWorld([], [], {
        x: 0,
        y: 0,
        width: 512,
        height: 512,
      });

      // 1. Đứng giữa đường trống trong nón tầm nhìn (isSheltered = false) -> BỊ PHÁT HIỆN (cúi người ngoài mái hiên không miễn nhiễm)
      expect(
        stealth.isPlayerDetected(
          { x: 160, y: 200 },
          false,
          emptyWorld,
          [],
          guard,
          false
        )
      ).toBe(true);

      // 2. Đứng trong vùng ẩn nấp (isSheltered = true) -> AN TOÀN
      expect(
        stealth.isPlayerDetected(
          { x: 160, y: 200 },
          true,
          emptyWorld,
          [],
          guard,
          false
        )
      ).toBe(false);

      // 3. Bị chắn bởi tường / occluder giữa (100, 200) và (160, 200) -> AN TOÀN
      const wallOccluder = [{ x: 125, y: 180, width: 12, height: 40 }];
      expect(
        stealth.isPlayerDetected(
          { x: 160, y: 200 },
          false,
          emptyWorld,
          wallOccluder,
          guard,
          false
        )
      ).toBe(false);

      // 4. Khi có Lịch Đổi Ca (hasScheduleBonus = true), tầm nhìn hiệu dụng giảm còn 78px -> đứng ở khoảng cách 88px không bị phát hiện
      expect(
        stealth.isPlayerDetected(
          { x: 188, y: 200 },
          false,
          emptyWorld,
          [],
          guard,
          false
        )
      ).toBe(true);
      expect(
        stealth.isPlayerDetected(
          { x: 188, y: 200 },
          false,
          emptyWorld,
          [],
          guard,
          true
        )
      ).toBe(false);

      // 5. Kiểm tra vòng đời trạng thái calm -> suspicious -> alert qua update()
      const shadowMap = getBuiltInMap('shadow_alley');
      stealth.initForMap(shadowMap);
      const firstGuard = stealth.getGuards()[0];
      expect(firstGuard).toBeDefined();
      firstGuard.speed = 0;

      // Đặt nhân vật ngay trước mặt lính tuần tra (cách 36px theo hướng nhìn)
      const inFrontPos = {
        x: firstGuard.x + Math.cos(firstGuard.facingAngleRad) * 36,
        y: firstGuard.y + Math.sin(firstGuard.facingAngleRad) * 36,
      };

      // Bước 1 (dt = 0.25s): suspicion tăng lên >= 0.25 -> trạng thái 'suspicious'
      const stepSuspicious = stealth.update(
        0.25,
        inFrontPos,
        false,
        { ...shadowMap, occluders: [] },
        emptyWorld,
        false
      );
      expect(stepSuspicious.triggeredAlertGuard).toBeNull();
      expect(stepSuspicious.highestAlertState).toBe('suspicious');
      expect(stepSuspicious.maxSuspicion).toBeGreaterThanOrEqual(0.25);

      // Bước 2 (dt = 0.7s): suspicion đạt 1.0 -> kích hoạt 'alert' và trả về triggeredAlertGuard
      const stepAlert = stealth.update(
        0.7,
        inFrontPos,
        false,
        { ...shadowMap, occluders: [] },
        emptyWorld,
        false
      );
      expect(stepAlert.highestAlertState).toBe('alert');
      expect(stepAlert.triggeredAlertGuard).not.toBeNull();
      expect(stepAlert.triggeredAlertGuard?.id).toBe(firstGuard.id);
    });
  });

  describe('2. Dossier Puzzle (CH1_DOSSIER_PUZZLE) & Provenance Registry (CH1_PROVENANCE_MANIFEST)', () => {
    it('validates 3 authentic dossier slots with progressive hints and distinguishes historical_fact citations from fictional_npc', () => {
      expect(CH1_DOSSIER_PUZZLE.id).toBe('puzzle_ch1_unification_dossier');
      expect(CH1_DOSSIER_PUZZLE.zoneId).toBe('secret_archive');
      expect(CH1_DOSSIER_PUZZLE.rewardMilestone).toBe('III');
      expect(CH1_DOSSIER_PUZZLE.rewardEvidenceId).toBe('ev_ch1_unification_appeal');
      expect(CH1_DOSSIER_PUZZLE.progressiveHint).not.toContain('Giáo trình');
      expect(CH1_DOSSIER_PUZZLE.textbookCitation).toContain(
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
      );

      expect(CH1_DOSSIER_PUZZLE.slots.map((s) => s.id)).toEqual([
        'slot_strategic_goal',
        'slot_primary_task',
        'slot_leadership_force',
      ]);

      for (const slot of CH1_DOSSIER_PUZZLE.slots) {
        expect(slot.options).toHaveLength(3);
        const authenticOpts = slot.options.filter((o) => o.isAuthentic);
        const distractorOpts = slot.options.filter((o) => !o.isAuthentic);
        expect(authenticOpts).toHaveLength(1);
        expect(distractorOpts).toHaveLength(2);
      }

      // Kiểm tra CH1_PROVENANCE_MANIFEST phân định rõ historical_fact và fictional_npc
      expect(CH1_PROVENANCE_MANIFEST.milestone_I_thanh_nien.type).toBe(
        'historical_fact'
      );
      expect(CH1_PROVENANCE_MANIFEST.milestone_I_thanh_nien.citation).toContain(
        'tr. 44–56'
      );
      expect(CH1_PROVENANCE_MANIFEST.milestone_II_unification_conference.type).toBe(
        'historical_fact'
      );
      expect(
        CH1_PROVENANCE_MANIFEST.milestone_II_unification_conference.citation
      ).toContain('tr. 57–61');
      expect(CH1_PROVENANCE_MANIFEST.milestone_III_first_platform.type).toBe(
        'historical_fact'
      );
      expect(CH1_PROVENANCE_MANIFEST.fictional_liaison_network.type).toBe(
        'fictional_npc'
      );
      expect(CH1_PROVENANCE_MANIFEST.speculative_2045_frame.type).toBe(
        'speculative_2045'
      );
    });
  });
});
