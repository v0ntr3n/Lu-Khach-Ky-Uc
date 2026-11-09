/**
 * @file tests/unit/m3_rpg_systems.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest) cho Cột mốc M3 — RPG Systems & UI:
 * 1. QuestSystem & điều kiện tiên quyết của mục tiêu (Objective Dependencies & Presenter Mode isolation).
 * 2. DialogueSystem & hội thoại phân nhánh đa lựa chọn (Branching Choices, grantsFlag / grantsEvidence / grantsMilestone).
 * 3. EvidenceSystem & InventorySystem (kiểm tra ProvenanceMetadata, trích dẫn Giáo trình 2021 và cô lập Presenter Mode).
 * 4. SaveDataV2 Persistence & Migration (lưu và khôi phục completedQuestIds, completedObjectiveIds, dialogueHistory, collectedEvidence, questFlags, settings).
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { GameStore } from '../../src/core/GameStore';
import { SaveManager } from '../../src/core/SaveManager';
import { CHAPTERS } from '../../src/data/chronicles';
import { DialogueSystem } from '../../src/systems/DialogueSystem';
import { EvidenceSystem } from '../../src/systems/EvidenceSystem';
import { InventorySystem } from '../../src/systems/InventorySystem';
import { QuestSystem } from '../../src/systems/QuestSystem';

describe('M3 RPG Systems — Quest, Dialogue, Evidence, Inventory & SaveV2 Suite', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  describe('1. QuestSystem & Objective Dependencies', () => {
    it('tracks main and side quests for 1930, enforces objective prerequisites, and isolates Presenter Mode', () => {
      const store = new GameStore();
      const questSystem = new QuestSystem();

      const ch1Quests = questSystem.getQuestsForEra('1930');
      expect(ch1Quests.map((q) => q.id)).toEqual(
        expect.arrayContaining(['q_ch1_main_unification', 'q_ch1_side_dock_watch'])
      );

      const mainQuest = ch1Quests.find((q) => q.id === 'q_ch1_main_unification')!;
      expect(mainQuest.category).toBe('main');

      const [objMeet, objLedger, objSafehouse, objArchive] = mainQuest.objectives;
      expect(objMeet.id).toBe('obj_ch1_meet_liaison');
      expect(objLedger.id).toBe('obj_ch1_inspect_warehouse_ledger');
      expect(objSafehouse.id).toBe('obj_ch1_decipher_safehouse');
      expect(objArchive.id).toBe('obj_ch1_reach_secret_archive');

      // Trạng thái khởi đầu: chỉ có obj_ch1_meet_liaison là 'active', các bước sau đều 'locked'
      expect(questSystem.getObjectiveStatus(objMeet, store)).toBe('active');
      expect(questSystem.getObjectiveStatus(objLedger, store)).toBe('locked');
      expect(questSystem.getObjectiveStatus(objSafehouse, store)).toBe('locked');
      expect(questSystem.getObjectiveStatus(objArchive, store)).toBe('locked');

      // Khôi phục Cột mốc I -> hoàn thành obj_ch1_meet_liaison và mở khóa obj_ch1_inspect_warehouse_ledger
      store.collectMilestone('I');
      questSystem.evaluateProgress(store, '1930');
      expect(questSystem.getObjectiveStatus(objMeet, store)).toBe('completed');
      expect(questSystem.getObjectiveStatus(objLedger, store)).toBe('active');
      expect(questSystem.getObjectiveStatus(objSafehouse, store)).toBe('locked');

      // Thu thập Sổ Vận Đơn (ev_ch1_shipping_ledger) -> hoàn thành obj_ch1_inspect_warehouse_ledger và mở khóa obj_ch1_decipher_safehouse
      store.collectedEvidence.add('ev_ch1_shipping_ledger');
      questSystem.evaluateProgress(store, '1930');
      expect(questSystem.getObjectiveStatus(objLedger, store)).toBe('completed');
      expect(questSystem.getObjectiveStatus(objSafehouse, store)).toBe('active');

      // Kiểm tra sự cô lập của Presenter Mode: bật lên hiển thị completed toàn bộ, tắt đi giữ nguyên trạng thái thực
      const savedCompletedCount = store.completedObjectives.size;
      const savedEvidenceCount = store.collectedEvidence.size;

      store.togglePresenterMode();
      expect(store.isPresenterMode).toBe(true);
      expect(questSystem.getObjectiveStatus(objSafehouse, store)).toBe('completed');
      expect(questSystem.getObjectiveStatus(objArchive, store)).toBe('completed');
      questSystem.evaluateProgress(store, '1930');

      store.togglePresenterMode();
      expect(store.isPresenterMode).toBe(false);
      expect(store.completedObjectives.size).toBe(savedCompletedCount);
      expect(store.collectedEvidence.size).toBe(savedEvidenceCount);
      expect(questSystem.getObjectiveStatus(objSafehouse, store)).toBe('active');
      expect(questSystem.getObjectiveStatus(objArchive, store)).toBe('locked');
    });
  });

  describe('2. DialogueSystem & Branching Choices', () => {
    it('starts dialogue trees with provenance metadata, branches across choices, grants flags/evidence, and ends cleanly', () => {
      const dialogueSystem = new DialogueSystem();
      const flags = new Set<string>();
      const evidence = new Set<string>();

      const rootNode = dialogueSystem.startDialogue('dlg_harbor_liaison');
      expect(rootNode).not.toBeNull();
      expect(rootNode?.speaker).toContain('Lâm Văn Bính');
      expect(rootNode?.provenance.type).toBe('historical_fact');
      expect(rootNode?.provenance.citation).toContain('Giáo trình Lịch sử Đảng');
      expect(rootNode?.choices.length).toBeGreaterThanOrEqual(2);

      // Chọn nhánh hỏi về Kho Hàng ('c_liaison_ask_warehouse') -> chuyển sang node 'warehouse_guide'
      const step1 = dialogueSystem.selectChoice(
        'c_liaison_ask_warehouse',
        flags,
        evidence
      );
      expect(step1.ended).toBe(false);
      expect(step1.choice?.grantsFlag).toBe('ch1_met_liaison');
      expect(step1.choice?.grantsMilestone).toBe('I');
      expect(step1.nextNode?.id).toBe('warehouse_guide');
      expect(step1.nextNode?.provenance.type).toBe('fictional_npc');

      // Chọn nhánh kết thúc ('c_liaison_done_1') -> nextNodeId === null, kết thúc hội thoại sạch sẽ
      const step2 = dialogueSystem.selectChoice('c_liaison_done_1', flags, evidence);
      expect(step2.ended).toBe(true);
      expect(step2.nextNode).toBeNull();
      expect(dialogueSystem.getCurrentNode()).toBeNull();

      // Kiểm tra cây hội thoại Nhà An Toàn cấp cả grantsEvidence ('ev_ch1_cipher_note') và grantsMilestone ('II')
      const innerRoot = dialogueSystem.startDialogue('npc_liaison_inner');
      expect(innerRoot).not.toBeNull();
      const innerStep = dialogueSystem.selectChoice(
        'c_inner_ask_delegates',
        flags,
        evidence
      );
      expect(innerStep.choice?.grantsEvidence).toBe('ev_ch1_cipher_note');
      expect(innerStep.choice?.grantsMilestone).toBe('II');
      expect(innerStep.nextNode?.id).toBe('delegates_node');
    });
  });

  describe('3. EvidenceSystem & InventorySystem', () => {
    it('provides Chapter I evidence items with valid ProvenanceMetadata/citations and isolates Presenter Mode in InventorySystem', () => {
      const store = new GameStore();
      const evidenceSystem = new EvidenceSystem();
      const inventorySystem = new InventorySystem(evidenceSystem);

      const ch1Evidence = evidenceSystem.getEvidenceForEra('1930');
      expect(ch1Evidence.length).toBeGreaterThanOrEqual(5);

      for (const item of ch1Evidence) {
        expect(item.provenance).toBeDefined();
        expect(item.provenance.label.length).toBeGreaterThan(3);
        if (item.provenance.type === 'historical_fact') {
          expect(item.textbookCitation).toContain('Giáo trình Lịch sử Đảng');
          expect(item.provenance.citation).toContain('Giáo trình Lịch sử Đảng');
        }
      }

      // Thêm vật phẩm trong chế độ thường
      expect(inventorySystem.getAllCollectedItems(store)).toHaveLength(0);
      const added = inventorySystem.addItem('ev_ch1_shipping_ledger', store);
      expect(added?.id).toBe('ev_ch1_shipping_ledger');
      expect(store.collectedEvidence.has('ev_ch1_shipping_ledger')).toBe(true);
      expect(store.questFlags.has('ch1_inspected_ledger')).toBe(true);
      expect(inventorySystem.getAllCollectedItems(store)).toHaveLength(1);

      // Bật Presenter Mode: hiển thị toàn bộ danh mục nhưng không ghi đè dữ liệu thật khi thêm mới
      store.togglePresenterMode();
      expect(inventorySystem.getAllCollectedItems(store).length).toBe(
        evidenceSystem.getAllEvidence().length
      );
      inventorySystem.addItem('ev_ch1_cipher_note', store);

      store.togglePresenterMode();
      expect(store.collectedEvidence.has('ev_ch1_cipher_note')).toBe(false);
      expect(inventorySystem.getAllCollectedItems(store)).toHaveLength(1);
    });
  });

  describe('4. SaveDataV2 Persistence & Migration', () => {
    it('round-trips completedQuestIds, completedObjectiveIds, dialogueHistory, collectedEvidence, questFlags, and settings via SaveManager', () => {
      const store = new GameStore();
      store.collectMilestone('I');
      store.collectedEvidence.add('ev_ch1_shipping_ledger');
      store.questFlags.add('ch1_met_liaison');
      store.questFlags.add('ch1_inspected_ledger');
      store.completedObjectives.add('obj_ch1_meet_liaison');
      store.completedObjectives.add('obj_ch1_inspect_warehouse_ledger');
      store.completedQuests.add('q_ch1_side_dock_watch');
      store.dialogueHistory.add('dlg_harbor_liaison:root');
      store.updateSettings({
        muted: true,
        reducedMotion: true,
        textScale: 'large',
        showHints: false,
      });

      const savePayload = store.toSaveDataV2('1930', 256, 'down', 368);
      SaveManager.save(savePayload);

      const loaded = SaveManager.load();
      expect(loaded).not.toBeNull();
      expect(loaded?.version).toBe(2);
      expect(loaded?.completedQuestIds).toEqual(['q_ch1_side_dock_watch']);
      expect(loaded?.completedObjectiveIds).toEqual([
        'obj_ch1_meet_liaison',
        'obj_ch1_inspect_warehouse_ledger',
      ]);
      expect(loaded?.dialogueHistory).toEqual(['dlg_harbor_liaison:root']);
      expect(loaded?.collectedEvidence).toEqual(['ev_ch1_shipping_ledger']);
      expect(loaded?.questFlags).toEqual(['ch1_met_liaison', 'ch1_inspected_ledger']);
      expect(loaded?.settings).toEqual({
        muted: true,
        reducedMotion: true,
        textScale: 'large',
        showHints: false,
      });

      const restoredStore = new GameStore();
      restoredStore.restoreFromSave(loaded!, CHAPTERS);
      expect(restoredStore.completedQuests.has('q_ch1_side_dock_watch')).toBe(true);
      expect(restoredStore.completedObjectives.has('obj_ch1_meet_liaison')).toBe(true);
      expect(restoredStore.dialogueHistory.has('dlg_harbor_liaison:root')).toBe(true);
      expect(restoredStore.collectedEvidence.has('ev_ch1_shipping_ledger')).toBe(true);
      expect(restoredStore.settings).toEqual({
        muted: true,
        reducedMotion: true,
        textScale: 'large',
        showHints: false,
      });
    });
  });
});
