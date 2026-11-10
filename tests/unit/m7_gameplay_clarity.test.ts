import { describe, expect, it } from 'vitest';
import { GameStore } from '../../src/core/GameStore';
import { SaveManager } from '../../src/core/SaveManager';
import { getAllBuiltInMaps, getBuiltInMap } from '../../src/runtime/MapLoader';
import {
  getObjectiveTargetPosition,
  resolveObjectiveTarget,
} from '../../src/runtime/PhaserRuntime';
import { QUEST_CATALOG, QuestSystem } from '../../src/systems/QuestSystem';
import { TutorialController } from '../../src/ui/TutorialController';

describe('M7 (#12) — Gameplay Clarity: First-Play Tutorial, Concise HUD & Exact Quest Targets', () => {
  it('validates all 15 objectives across Chapters I–IV have canonical QuestTargetRef links in built-in maps', () => {
    const questSystem = new QuestSystem();
    const allMaps = getAllBuiltInMaps();
    const validation = questSystem.validateAllObjectiveTargets(allMaps);

    expect(validation.valid).toBe(true);
    expect(validation.errors).toEqual([]);
    expect(QUEST_CATALOG.flatMap((q) => q.objectives).length).toBe(15);
  });

  it('resolves exact target for obj_ch1_meet_liaison to npc_courier and never picks climax_gate_1930', () => {
    const questSystem = new QuestSystem();
    const harborMap = getBuiltInMap('harbor_square');
    const found = questSystem.getObjectiveById('obj_ch1_meet_liaison');

    expect(found).not.toBeNull();
    const resolution = resolveObjectiveTarget(harborMap, found!.objective);
    expect(resolution.status).toBe('resolved');
    expect(resolution.targetEntityId).toBe('npc_courier');
    expect(resolution.position).not.toBeNull();
    expect(resolution.position?.targetId).toBe('npc_courier');
    expect(resolution.position?.isPortal).toBe(false);
    expect(resolution.nextActionText).toContain('Lâm Văn Bính');

    // Verify getObjectiveTargetPosition also returns exact npc_courier position
    const pos = getObjectiveTargetPosition(harborMap, found!.objective);
    expect(pos?.targetId).toBe('npc_courier');
    expect(pos?.isPortal).toBe(false);
  });

  it('distinguishes resolved, locked, and unavailable target resolution states deterministically', () => {
    const questSystem = new QuestSystem();
    const store = new GameStore();
    const harborMap = getBuiltInMap('harbor_square');
    const warehouseObj = questSystem.getObjectiveById(
      'obj_ch1_inspect_warehouse_ledger'
    )!.objective;

    // 1. Locked objective when prerequisite milestone I is missing
    const lockedStatus = questSystem.getObjectiveStatus(warehouseObj, store);
    expect(lockedStatus).toBe('locked');
    const lockedRes = resolveObjectiveTarget(harborMap, warehouseObj, {
      isLocked: true,
      prerequisiteText: questSystem.getPrerequisiteSummary(warehouseObj, store),
      questFlags: store.questFlags,
    });
    expect(lockedRes.status).toBe('locked');
    expect(lockedRes.position).toBeNull();
    expect(lockedRes.nextActionText).toContain('Cột mốc I');

    // 2. Active cross-zone objective on harbor_square resolves via portal to warehouse_interior
    store.collectMilestone('I');
    store.questFlags.add('ch1_met_liaison');
    const crossZoneRes = resolveObjectiveTarget(harborMap, warehouseObj, {
      questFlags: store.questFlags,
    });
    expect(crossZoneRes.status).toBe('resolved');
    expect(crossZoneRes.position?.isPortal).toBe(true);
    expect(crossZoneRes.position).not.toBeNull();

    // 3. Unavailable target when entity ID does not exist on map and no portal matches
    const unavailableRes = resolveObjectiveTarget(harborMap, {
      zoneId: 'harbor_square',
      interactableId: 'non_existent_entity_id_999',
    });
    expect(unavailableRes.status).toBe('unavailable');
    expect(unavailableRes.position).toBeNull();
  });

  it('never returns a locked objective as the current active objective', () => {
    const questSystem = new QuestSystem();
    const store = new GameStore();

    // Remove initial active objectives by marking them completed WITHOUT collecting Milestone I
    store.completedObjectives.add('obj_ch1_meet_liaison');
    store.completedObjectives.add('obj_ch1_talk_pier_boatman');
    store.completedObjectives.add('obj_ch1_check_patrol_schedule');

    const active = questSystem.getCurrentActiveObjective('1930', store);
    expect(active).toBeNull();

    const nextLocked = questSystem.getNextLockedObjective('1930', store);
    expect(nextLocked?.objective.id).toBe('obj_ch1_inspect_warehouse_ledger');
  });

  it('progresses TutorialController through 5 steps, supports skip and replay, and persists tutorialCompleted', () => {
    let persistedCompleted: boolean | null = null;
    const tutorial = new TutorialController({
      onTutorialCompletedChange: (completed) => {
        persistedCompleted = completed;
      },
    });

    tutorial.setGameplayVisible(true);
    tutorial.startIfFirstPlay(false);

    expect(tutorial.isActive()).toBe(true);
    expect(tutorial.getCurrentStep()).toBe('move');

    // Step 1: move >= 24px
    tutorial.recordMovementDelta(15, 0);
    expect(tutorial.getCurrentStep()).toBe('move');
    tutorial.recordMovementDelta(12, 0);
    expect(tutorial.getCurrentStep()).toBe('interact');

    // Step 2: interact [E]
    tutorial.recordAction('interact');
    expect(tutorial.getCurrentStep()).toBe('journal');

    // Step 3: journal [J]
    tutorial.recordAction('journal');
    expect(tutorial.getCurrentStep()).toBe('map');

    // Step 4: map [M]
    tutorial.recordAction('map');
    expect(tutorial.getCurrentStep()).toBe('hint');

    // Step 5: hint [H] -> completed
    tutorial.recordAction('hint');
    expect(tutorial.isCompleted()).toBe(true);
    expect(tutorial.isActive()).toBe(false);
    expect(persistedCompleted).toBe(true);

    // Replay resets to Step 1
    tutorial.replay();
    expect(tutorial.isCompleted()).toBe(false);
    expect(tutorial.isActive()).toBe(true);
    expect(tutorial.getCurrentStep()).toBe('move');
    expect(persistedCompleted).toBe(false);

    // Skip immediately completes
    tutorial.skip();
    expect(tutorial.isCompleted()).toBe(true);
    expect(tutorial.isActive()).toBe(false);
    expect(persistedCompleted).toBe(true);

    // SaveManager round-trips tutorialCompleted setting
    const saveManager = new SaveManager();
    const normalized = saveManager.normalizeSettings({ tutorialCompleted: true });
    expect(normalized.tutorialCompleted).toBe(true);
  });
});
