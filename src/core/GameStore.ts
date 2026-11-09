/**
 * @file src/core/GameStore.ts
 * @description Kho trạng thái trung tâm có thẩm quyền duy nhất (Authoritative Serializable State)
 * cho "Lữ Khách Ký Ức (1930 – 2045)". Quản lý máy trạng thái GameMode, tọa độ 2D Top-Down,
 * bản đồ hiện tại (currentMapId / currentSpawnId), các điểm lưu (unlockedCheckpoints),
 * nhiệm vụ (completedQuests / completedObjectives), lịch sử hội thoại (dialogueHistory),
 * cài đặt trợ năng (settings), tiến trình Cột mốc La Mã và sự cô lập hoàn toàn của Presenter Mode.
 */

import type {
  ChapterData,
  EraId,
  Facing4,
  GameMode,
  GameSettings,
  RomanMilestoneId,
  SaveDataV2,
  ShelterZone,
  Vec2,
} from '../types/game';
import {
  DEFAULT_GAME_SETTINGS,
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

  // Trạng thái 2D Top-Down (M1+)
  public currentMapId: string = DEFAULT_SAFE_MAP_ID;
  public currentSpawnId: string = DEFAULT_SAFE_SPAWN_ID;
  public playerPos: Vec2 = { x: 160, y: 384 };
  public playerFacing: Facing4 = 'right';
  public readonly questFlags = new Set<string>();
  public readonly collectedEvidence = new Set<string>();
  public readonly discoveredMaps = new Set<string>([DEFAULT_SAFE_MAP_ID]);
  public portalCooldownSec = 0;

  // Trạng thái M2 World Exploration (Checkpoints & Debug Overlay)
  public readonly unlockedCheckpoints = new Set<string>([
    'harbor_square',
    'default_spawn',
    'cp_harbor_lantern',
  ]);
  public debugOverlayEnabled = false;

  // Trạng thái M3 RPG Systems (Quests, Objectives, Dialogue History & Settings)
  public readonly completedObjectives = new Set<string>();
  public readonly completedQuests = new Set<string>();
  public readonly dialogueHistory = new Set<string>();
  public settings: GameSettings = { ...DEFAULT_GAME_SETTINGS };

  public setMode(mode: GameMode): void {
    this.mode = mode;
  }

  public getMode(): GameMode {
    return this.mode;
  }

  public isWorldInputAllowed(): boolean {
    return this.mode === 'exploring';
  }

  public updateSettings(partial: Partial<GameSettings>): GameSettings {
    this.settings = {
      ...this.settings,
      ...partial,
    };
    return { ...this.settings };
  }

  public setMapState(
    mapId: string,
    spawnId: string,
    pos: Vec2,
    facing: Facing4 = 'down'
  ): void {
    this.currentMapId = mapId;
    this.currentSpawnId = spawnId;
    this.playerPos = { x: pos.x, y: pos.y };
    this.playerFacing = facing;
    this.discoveredMaps.add(mapId);
    this.bumpTransitionToken();
  }

  public unlockCheckpoint(checkpointOrZoneId: string, mapId?: string): boolean {
    const hadMain = this.unlockedCheckpoints.has(checkpointOrZoneId);
    const hadMap = mapId ? this.unlockedCheckpoints.has(mapId) : true;
    this.unlockedCheckpoints.add(checkpointOrZoneId);
    if (mapId) {
      this.unlockedCheckpoints.add(mapId);
      this.discoveredMaps.add(mapId);
    }
    return !hadMain || !hadMap;
  }

  public hasCheckpointForZone(
    zoneId: string,
    checkpointId?: string,
    checkpointSpawnId?: string
  ): boolean {
    if (this.isPresenterMode) {
      return true;
    }
    if (this.unlockedCheckpoints.has(zoneId)) {
      return true;
    }
    if (checkpointId && this.unlockedCheckpoints.has(checkpointId)) {
      return true;
    }
    if (checkpointSpawnId && this.unlockedCheckpoints.has(checkpointSpawnId)) {
      return true;
    }
    return false;
  }

  public canFastTravelToZone(
    toZoneId: string,
    checkpointId?: string,
    checkpointSpawnId?: string
  ): boolean {
    if (this.isPresenterMode) {
      return true;
    }
    if (!this.discoveredMaps.has(toZoneId)) {
      return false;
    }
    return this.hasCheckpointForZone(toZoneId, checkpointId, checkpointSpawnId);
  }

  public toggleDebugOverlay(force?: boolean): boolean {
    this.debugOverlayEnabled = force ?? !this.debugOverlayEnabled;
    return this.debugOverlayEnabled;
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
    _isCrouching = false,
    playerY?: number
  ): boolean {
    return shelters.some((s) => {
      if (s.bounds && playerY !== undefined) {
        const inX = playerX >= s.bounds.x && playerX <= s.bounds.x + s.bounds.width;
        const inY = playerY >= s.bounds.y && playerY <= s.bounds.y + s.bounds.height;
        if (inX && inY) {
          return true;
        }
      }
      return playerX >= s.xStart && playerX <= s.xEnd;
    });
  }

  public bumpTransitionToken(): number {
    this.transitionToken += 1;
    return this.transitionToken;
  }

  public getTransitionToken(): number {
    return this.transitionToken;
  }

  public resetToDefault(): void {
    this.currentChapterIndex = 0;
    this.isPresenterMode = false;
    this.collectedMilestones.clear();
    this.presenterCollectedMilestones.clear();
    this.questFlags.clear();
    this.collectedEvidence.clear();
    this.discoveredMaps.clear();
    this.discoveredMaps.add(DEFAULT_SAFE_MAP_ID);
    this.unlockedCheckpoints.clear();
    this.unlockedCheckpoints.add(DEFAULT_SAFE_MAP_ID);
    this.unlockedCheckpoints.add(DEFAULT_SAFE_SPAWN_ID);
    this.unlockedCheckpoints.add('cp_harbor_lantern');
    this.completedObjectives.clear();
    this.completedQuests.clear();
    this.dialogueHistory.clear();
    this.currentMapId = DEFAULT_SAFE_MAP_ID;
    this.currentSpawnId = DEFAULT_SAFE_SPAWN_ID;
    this.playerPos = { x: 85, y: 384 };
    this.playerFacing = 'right';
    this.bumpTransitionToken();
  }

  public toSaveDataV2(
    eraId: EraId,
    playerX: number,
    facing: Facing4 = 'right',
    playerY?: number
  ): SaveDataV2 {
    const safeX =
      Number.isFinite(playerX) && playerX >= 24
        ? Math.round(playerX)
        : DEFAULT_SAFE_PLAYER_POS.x;
    const safeY =
      playerY !== undefined && Number.isFinite(playerY) && playerY >= 16
        ? Math.round(playerY)
        : DEFAULT_SAFE_PLAYER_POS.y;

    return {
      version: 2,
      timestamp: Date.now(),
      eraId,
      currentMapId: this.currentMapId || DEFAULT_SAFE_MAP_ID,
      spawnId: this.currentSpawnId || DEFAULT_SAFE_SPAWN_ID,
      playerPos: {
        x: safeX,
        y: safeY,
      },
      playerFacing: facing,
      collectedMilestones: Array.from(this.collectedMilestones),
      collectedEvidence: Array.from(this.collectedEvidence),
      questFlags: Array.from(this.questFlags),
      discoveredMaps: Array.from(this.discoveredMaps),
      unlockedCheckpoints: Array.from(this.unlockedCheckpoints),
      completedChapters: [],
      activeQuestIds: [],
      completedQuestIds: Array.from(this.completedQuests),
      completedObjectiveIds: Array.from(this.completedObjectives),
      dialogueHistory: Array.from(this.dialogueHistory),
      settings: { ...this.settings },
    };
  }

  public restoreFromSave(save: SaveDataV2, chapters: readonly ChapterData[]): void {
    this.collectedMilestones.clear();
    for (const roman of save.collectedMilestones) {
      this.collectedMilestones.add(roman);
    }

    this.collectedEvidence.clear();
    for (const ev of save.collectedEvidence ?? []) {
      this.collectedEvidence.add(ev);
    }

    this.questFlags.clear();
    for (const flag of save.questFlags ?? []) {
      this.questFlags.add(flag);
    }

    this.discoveredMaps.clear();
    this.discoveredMaps.add(DEFAULT_SAFE_MAP_ID);
    for (const m of save.discoveredMaps ?? []) {
      this.discoveredMaps.add(m);
    }

    this.unlockedCheckpoints.clear();
    this.unlockedCheckpoints.add(DEFAULT_SAFE_MAP_ID);
    this.unlockedCheckpoints.add(DEFAULT_SAFE_SPAWN_ID);
    this.unlockedCheckpoints.add('cp_harbor_lantern');
    for (const cp of save.unlockedCheckpoints ?? []) {
      this.unlockedCheckpoints.add(cp);
    }

    this.completedObjectives.clear();
    for (const objId of save.completedObjectiveIds ?? []) {
      this.completedObjectives.add(objId);
    }

    this.completedQuests.clear();
    for (const qId of save.completedQuestIds ?? []) {
      this.completedQuests.add(qId);
    }

    this.dialogueHistory.clear();
    for (const dId of save.dialogueHistory ?? []) {
      this.dialogueHistory.add(dId);
    }

    if (save.settings) {
      this.settings = {
        ...DEFAULT_GAME_SETTINGS,
        ...save.settings,
      };
    }

    this.currentMapId = save.currentMapId || DEFAULT_SAFE_MAP_ID;
    this.currentSpawnId = save.spawnId || DEFAULT_SAFE_SPAWN_ID;
    this.playerPos = {
      x: save.playerPos?.x ?? DEFAULT_SAFE_PLAYER_POS.x,
      y: save.playerPos?.y ?? 384,
    };
    this.playerFacing = save.playerFacing ?? 'right';

    const chapterIdx = chapters.findIndex((c) => c.id === save.eraId);
    if (chapterIdx !== -1) {
      this.currentChapterIndex = chapterIdx;
    }
  }
}
