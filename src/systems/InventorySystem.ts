/**
 * @file src/systems/InventorySystem.ts
 * @description Hệ thống Hành trang & Túi đựng Tư liệu (Inventory System) kết nối với EvidenceSystem
 * và GameStore cho "Lữ Khách Ký Ức (1930 – 2045)".
 */

import type { GameStore } from '../core/GameStore';
import type { EraId, EvidenceItem } from '../types/game';
import { EvidenceSystem } from './EvidenceSystem';

export class InventorySystem {
  private readonly evidenceSystem: EvidenceSystem;

  constructor(evidenceSystem = new EvidenceSystem()) {
    this.evidenceSystem = evidenceSystem;
  }

  public hasItem(id: string, store: GameStore): boolean {
    if (store.isPresenterMode) {
      return this.evidenceSystem.getEvidenceById(id) !== undefined;
    }
    return store.collectedEvidence.has(id);
  }

  public addItem(id: string, store: GameStore): EvidenceItem | undefined {
    const item = this.evidenceSystem.getEvidenceById(id);
    if (!item) return undefined;

    if (!store.isPresenterMode) {
      store.collectedEvidence.add(item.id);
      if (item.grantsFlag) {
        store.questFlags.add(item.grantsFlag);
      }
      if (item.relatedMilestone) {
        store.collectMilestone(item.relatedMilestone);
      }
    } else if (item.relatedMilestone) {
      store.presenterCollectedMilestones.add(item.relatedMilestone);
    }

    return item;
  }

  public getItemsForEra(eraId: EraId, store: GameStore): EvidenceItem[] {
    const allForEra = this.evidenceSystem.getEvidenceForEra(eraId);
    if (store.isPresenterMode) {
      return allForEra;
    }
    return allForEra.filter((item) => store.collectedEvidence.has(item.id));
  }

  public getAllCollectedItems(store: GameStore): EvidenceItem[] {
    return this.evidenceSystem.getCollectedEvidence(
      store.collectedEvidence,
      store.isPresenterMode
    );
  }

  public getCategoryCounts(
    store: GameStore
  ): Record<EvidenceItem['category'], number> {
    const collected = this.getAllCollectedItems(store);
    const counts: Record<EvidenceItem['category'], number> = {
      document: 0,
      cipher: 0,
      artifact: 0,
      dispatch: 0,
    };
    for (const item of collected) {
      counts[item.category] = (counts[item.category] ?? 0) + 1;
    }
    return counts;
  }
}
