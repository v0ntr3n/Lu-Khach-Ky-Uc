/**
 * @file tests/unit/m6_chapters_provenance.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest) cho Cột mốc M6 — Additional Chapters (II, III, IV) & Final QA:
 * 1. 14-Map Multi-Era WorldGraph & Tiled Map Validation (8 khu vực Chương I + 6 khu vực Chương II–IV).
 * 2. Cơ chế giải đố/chiến lược riêng biệt của 4 Thời kỳ (CH1_DOSSIER_PUZZLE, CH2_DISPATCH_NETWORK_PUZZLE,
 *    CH3_SUPPLY_LOGISTICS_PUZZLE, CH4_CAUSAL_POLICY_PUZZLE).
 * 3. Kiểm toán Nguồn gốc Sử liệu (Provenance Audit) trên toàn bộ Evidence, Quest và Hội thoại Chương II–IV
 *    cùng nhãn phân định Viễn cảnh 2045 (speculative_2045).
 */

import { describe, expect, it } from 'vitest';
import { CH1_DOSSIER_PUZZLE } from '../../src/data/chapters/ch1_1930';
import {
  CH2_DISPATCH_NETWORK_PUZZLE,
  CH3_SUPPLY_LOGISTICS_PUZZLE,
  CH4_CAUSAL_POLICY_PUZZLE,
} from '../../src/data/chapters/ch2_3_4_mechanics';
import { getAllBuiltInMaps, toValidatableMap } from '../../src/runtime/MapLoader';
import { DialogueSystem } from '../../src/systems/DialogueSystem';
import { EvidenceSystem } from '../../src/systems/EvidenceSystem';
import { QUEST_CATALOG } from '../../src/systems/QuestSystem';
import { type ValidatableMap, validateMapData } from '../../src/world/MapValidator';
import {
  CHAPTER1_WORLD_GRAPH,
  getDefaultZoneForEra,
} from '../../src/world/WorldGraph';

describe('M6 Additional Chapters (II–IV), Distinct Era Mechanics & Provenance Audit Suite', () => {
  describe('1. 14-Map Multi-Era WorldGraph & Tiled Map Validation', () => {
    it('loads and validates all 14 built-in maps across Chapters I–IV and resolves default zones per era', () => {
      const allMaps = getAllBuiltInMaps();
      const mapIds = Object.keys(allMaps);

      const expectedMaps = [
        // Chapter I (8 zones)
        'harbor_square',
        'warehouse_interior',
        'old_street',
        'small_pier',
        'safehouse_interior',
        'shadow_alley',
        'archive_courtyard',
        'secret_archive',
        // Chapter II (2 zones)
        'tan_trao_banyan',
        'ba_dinh_square',
        // Chapter III (2 zones)
        'pha_din_pass',
        'spring_1975_corridor',
        // Chapter IV (2 zones)
        'doi_moi_avenue',
        'nexus_2045_plaza',
      ];

      expect(mapIds).toHaveLength(14);
      expect(mapIds).toEqual(expect.arrayContaining(expectedMaps));

      const validatableMaps: Record<string, ValidatableMap> = {};
      for (const [id, normMap] of Object.entries(allMaps)) {
        validatableMaps[id] = toValidatableMap(normMap);
      }

      for (const id of expectedMaps) {
        const res = validateMapData(validatableMaps[id], validatableMaps);
        expect(res.errors, `Validation errors in map "${id}"`).toEqual([]);
        expect(res.valid).toBe(true);
      }

      const graphCheck = CHAPTER1_WORLD_GRAPH.validateWorldGraph(allMaps);
      expect(graphCheck.errors).toEqual([]);
      expect(graphCheck.valid).toBe(true);

      expect(getDefaultZoneForEra('1930')).toBe('harbor_square');
      expect(getDefaultZoneForEra('1945')).toBe('tan_trao_banyan');
      expect(getDefaultZoneForEra('1954_1975')).toBe('pha_din_pass');
      expect(getDefaultZoneForEra('1986_2045')).toBe('doi_moi_avenue');
    });
  });

  describe('2. Distinct Era Mechanics (CH1, CH2, CH3, CH4)', () => {
    it('ensures each chapter has a unique puzzle ID, zone, historical slots, authentic options, progressive hint, and textbook citation', () => {
      const puzzles = [
        CH1_DOSSIER_PUZZLE,
        CH2_DISPATCH_NETWORK_PUZZLE,
        CH3_SUPPLY_LOGISTICS_PUZZLE,
        CH4_CAUSAL_POLICY_PUZZLE,
      ];

      const puzzleIds = new Set(puzzles.map((p) => p.id));
      const puzzleZones = new Set(puzzles.map((p) => p.zoneId));
      const rewardMilestones = new Set(puzzles.map((p) => p.rewardMilestone));

      expect(puzzleIds.size).toBe(4);
      expect(puzzleZones.size).toBe(4);
      expect(rewardMilestones).toEqual(new Set(['III', 'V', 'VIII', 'XI']));

      for (const p of puzzles) {
        expect(p.slots).toHaveLength(3);
        expect(p.progressiveHint).not.toContain('Giáo trình');
        expect(p.textbookCitation).toContain(
          'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
        );

        for (const slot of p.slots) {
          expect(slot.options).toHaveLength(3);
          expect(slot.options.filter((o) => o.isAuthentic)).toHaveLength(1);
        }
      }
    });
  });

  describe('3. Historical Provenance & 2045 Speculative Framing Audit', () => {
    it('verifies textbook citations on all historical_fact items, fictional_npc tags on composite characters, and speculative_2045 framing on 2045 nodes', () => {
      const evidenceSystem = new EvidenceSystem();
      const dialogueSystem = new DialogueSystem();

      // 1. Kiểm toán toàn bộ Evidence
      const allEvidence = evidenceSystem.getAllEvidence();
      expect(allEvidence.length).toBeGreaterThanOrEqual(8);
      for (const item of allEvidence) {
        if (item.provenance.type === 'historical_fact') {
          expect(item.provenance.citation).toContain(
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
          );
          expect(item.textbookCitation).toContain(
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
          );
        }
      }

      // 2. Kiểm toán toàn bộ Quests
      expect(QUEST_CATALOG.length).toBeGreaterThanOrEqual(5);
      for (const quest of QUEST_CATALOG) {
        if (quest.provenance.type === 'historical_fact') {
          expect(quest.provenance.citation).toContain(
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
          );
        }
      }

      // 3. Kiểm toán hội thoại các chương II, III, IV
      const ch2To4Dialogues = [
        'dlg_tan_trao_messenger',
        'dlg_ba_dinh_witness',
        'dlg_pha_din_quartermaster',
        'dlg_1975_signal_officer',
        'dlg_doi_moi_economist',
        'dlg_archivist_2045',
      ];

      for (const dlgId of ch2To4Dialogues) {
        const tree = dialogueSystem.getTree(dlgId);
        expect(tree, `Dialogue tree "${dlgId}" must exist`).toBeDefined();
        for (const node of Object.values(tree!.nodes)) {
          if (node.provenance.type === 'historical_fact') {
            expect(node.provenance.citation).toContain(
              'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
            );
          }
        }
      }

      // 4. Kiểm tra nhân vật hư cấu (fictional_npc) và khung dẫn truyện tương lai 2045 (speculative_2045)
      const boatmanRoot = dialogueSystem.startDialogue('dlg_pier_boatman');
      expect(boatmanRoot?.provenance.type).toBe('fictional_npc');

      const archivist2045Root = dialogueSystem.startDialogue('dlg_archivist_2045');
      expect(archivist2045Root).not.toBeNull();
      expect(archivist2045Root?.provenance.type).toBe('speculative_2045');
      expect(archivist2045Root?.provenance.citation).toContain(
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021)'
      );
    });
  });
});
