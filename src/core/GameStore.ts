/**
 * @file src/core/GameStore.ts
 * @description Kho trạng thái trung tâm có thẩm quyền duy nhất (Authoritative Serializable State)
 * cho "Lữ Khách Ký Ức (1930 – 2045)". Quản lý máy trạng thái GameMode, tiến trình Cột mốc La Mã,
 * sự cô lập hoàn toàn của Chế độ Giảng viên (Presenter Mode), điều kiện mở Cổng Thời Gian Climax,
 * kiểm tra vùng ẩn nấp (Shelter) và bộ đếm Transition Token chống timer cũ.
 */

import type {
  ChapterData,
  EraId,
  Facing4,
  GameMode,
  RomanMilestoneId,
  SaveDataV2,
  ShelterZone,
} from '../types/game';
import {
  DEFAULT_SAFE_MAP_ID,
  DEFAULT_SAFE_PLAYER_POS,
  DEFAULT_SAFE_SPAWN_ID,
} from './SaveManager';

export const ALL_ROMAN_MILESTONES: readonly RomanMilestoneId[] = [
  'I',
  'II',
  'III',
  'IV',
  'V',
  'VI',
  'VII',
  'VIII',
  'IX',
  'X',
  'XI',
  'XII',
];

export class GameStore {
  public mode: GameMode = 'opening';
  public currentChapterIndex = 0;
  public isPresenterMode = false;
  public readonly collectedMilestones = new Set<RomanMilestoneId>();
  public readonly presenterCollectedMilestones = new Set<RomanMilestoneId>();
  public transitionToken = 0;

  public setMode(mode: GameMode): void {
    this.mode = mode;
  }

  public getMode(): GameMode {
    return this.mode;
  }

  public isWorldInputAllowed(): boolean {
    return this.mode === 'exploring';
  }

  public togglePresenterMode(): boolean {
    this.isPresenterMode = !this.isPresenterMode;
    if (this.isPresenterMode) {
      this.presenterCollectedMilestones.clear();
      for (const roman of ALL_ROMAN_MILESTONES) {
        this.presenterCollectedMilestones.add(roman);
      }
    } else {
      this.presenterCollectedMilestones.clear();
    }
    return this.isPresenterMode;
  }

  public getActiveMilestones(): Set<RomanMilestoneId> {
    return this.isPresenterMode ? this.presenterCollectedMilestones : this.collectedMilestones;
  }

  public collectMilestone(roman: RomanMilestoneId): boolean {
    const isNew = !this.collectedMilestones.has(roman);
    this.collectedMilestones.add(roman);
    if (this.isPresenterMode) {
      this.presenterCollectedMilestones.add(roman);
    }
    return isNew;
  }

  public canTriggerChapterClimax(chapter: ChapterData): {
    allowed: boolean;
    missing: RomanMilestoneId[];
  } {
    const active = this.getActiveMilestones();
    const missing: RomanMilestoneId[] = [];
    for (const m of chapter.milestones) {
      if (!active.has(m.roman)) {
        missing.push(m.roman);
      }
    }
    return {
      allowed: missing.length === 0,
      missing,
    };
  }

  public isPlayerInShelter(
    playerX: number,
    shelters: ShelterZone[],
    _isCrouching = false
  ): boolean {
    return shelters.some((s) => playerX >= s.xStart && playerX <= s.xEnd);
  }

  public bumpTransitionToken(): number {
    this.transitionToken += 1;
    return this.transitionToken;
  }

  public getTransitionToken(): number {
    return this.transitionToken;
  }

  public toSaveDataV2(eraId: EraId, playerX: number, facing: Facing4 = 'right'): SaveDataV2 {
    return {
      version: 2,
      timestamp: Date.now(),
      eraId,
      currentMapId: DEFAULT_SAFE_MAP_ID,
      spawnId: DEFAULT_SAFE_SPAWN_ID,
      playerPos: {
        x: Number.isFinite(playerX) && playerX >= 24 ? Math.round(playerX) : DEFAULT_SAFE_PLAYER_POS.x,
        y: DEFAULT_SAFE_PLAYER_POS.y,
      },
      playerFacing: facing,
      collectedMilestones: Array.from(this.collectedMilestones),
      collectedEvidence: [],
      questFlags: [],
      discoveredMaps: [DEFAULT_SAFE_MAP_ID],
      unlockedCheckpoints: [DEFAULT_SAFE_SPAWN_ID],
      completedChapters: [],
    };
  }

  public restoreFromSave(save: SaveDataV2, chapters: readonly ChapterData[]): void {
    this.collectedMilestones.clear();
    for (const roman of save.collectedMilestones) {
      this.collectedMilestones.add(roman);
    }
    const chapterIdx = chapters.findIndex((c) => c.id === save.eraId);
    if (chapterIdx !== -1) {
      this.currentChapterIndex = chapterIdx;
    }
  }
}
