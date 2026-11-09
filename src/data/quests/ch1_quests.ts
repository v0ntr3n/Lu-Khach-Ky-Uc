/**
 * @file src/data/quests/ch1_quests.ts
 * @description Định nghĩa Nhiệm vụ Chính & Nhiệm vụ Phụ cho Chương I (Hương Cảng 1930).
 */

import { QUEST_CATALOG } from '../../systems/QuestSystem';
import type { QuestDefinition } from '../../types/game';

export const CH1_QUESTS: readonly QuestDefinition[] = QUEST_CATALOG.filter(
  (q) => q.eraId === '1930'
);

export const CH1_MAIN_QUEST: QuestDefinition = CH1_QUESTS.find(
  (q) => q.id === 'q_ch1_main_unification'
)!;

export const CH1_SIDE_QUEST: QuestDefinition = CH1_QUESTS.find(
  (q) => q.id === 'q_ch1_side_dock_watch'
)!;
