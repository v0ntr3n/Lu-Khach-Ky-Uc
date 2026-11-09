/**
 * @file src/core/SaveManager.ts
 * @description Quản lý lưu trữ trạng thái tiến trình được đánh phiên bản (SaveV2)
 * với khả năng tự động di trú từ định dạng cũ (SaveV1) và khôi phục vị trí spawn an toàn.
 */

import type { EraId, RomanMilestoneId, SaveDataV1, SaveDataV2 } from '../types/game';

export const SAVE_KEY_V2 = 'lu_khach_ky_uc_save_v2';
export const SAVE_KEY_V1 = 'lu_khach_ky_uc_save_v1';

const VALID_ERAS: readonly EraId[] = ['1930', '1945', '1954_1975', '1986_2045'];
const VALID_MILESTONES: readonly RomanMilestoneId[] = [
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

export const DEFAULT_SAFE_MAP_ID = 'harbor_square';
export const DEFAULT_SAFE_SPAWN_ID = 'default_spawn';
export const DEFAULT_SAFE_PLAYER_POS = { x: 85, y: 148 } as const;

export function migrateV1ToV2(v1: SaveDataV1): SaveDataV2 {
  const eraId: EraId = VALID_ERAS.includes(v1.chapterId) ? v1.chapterId : '1930';
  const rawMilestones = Array.isArray(v1.collectedMilestones) ? v1.collectedMilestones : [];
  const collectedMilestones = Array.from(
    new Set(rawMilestones.filter((m): m is RomanMilestoneId => VALID_MILESTONES.includes(m)))
  );

  return {
    version: 2,
    timestamp: Date.now(),
    eraId,
    currentMapId: DEFAULT_SAFE_MAP_ID,
    spawnId: DEFAULT_SAFE_SPAWN_ID,
    playerPos: { x: DEFAULT_SAFE_PLAYER_POS.x, y: DEFAULT_SAFE_PLAYER_POS.y },
    playerFacing: 'right',
    collectedMilestones,
    collectedEvidence: [],
    questFlags: [],
    discoveredMaps: [DEFAULT_SAFE_MAP_ID],
    unlockedCheckpoints: [DEFAULT_SAFE_SPAWN_ID],
    completedChapters: [],
  };
}

export class SaveManager {
  public static readonly STORAGE_KEY_V2 = SAVE_KEY_V2;
  public static readonly STORAGE_KEY_V1 = SAVE_KEY_V1;

  public static migrateV1ToV2(v1: SaveDataV1): SaveDataV2 {
    return migrateV1ToV2(v1);
  }

  public migrateV1ToV2(v1: SaveDataV1): SaveDataV2 {
    return migrateV1ToV2(v1);
  }

  public static save(data: SaveDataV2): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const normalized: SaveDataV2 = {
        ...data,
        version: 2,
        timestamp: data.timestamp ?? Date.now(),
      };
      localStorage.setItem(SAVE_KEY_V2, JSON.stringify(normalized));
    } catch {
      // Bỏ qua lỗi quota hoặc môi trường ẩn danh không cho phép localStorage
    }
  }

  public save(data: SaveDataV2): void {
    SaveManager.save(data);
  }

  public static load(): SaveDataV2 | null {
    try {
      if (typeof localStorage === 'undefined') return null;

      const rawV2 = localStorage.getItem(SAVE_KEY_V2);
      if (rawV2) {
        const parsed = JSON.parse(rawV2) as Partial<
          Omit<SaveDataV2, 'version'> & Omit<SaveDataV1, 'version'> & { version?: number }
        >;
        if (parsed && parsed.version === 2) {
          const safeEra: EraId =
            parsed.eraId && VALID_ERAS.includes(parsed.eraId) ? parsed.eraId : '1930';
          const safePos =
            parsed.playerPos &&
            typeof parsed.playerPos.x === 'number' &&
            typeof parsed.playerPos.y === 'number' &&
            Number.isFinite(parsed.playerPos.x) &&
            Number.isFinite(parsed.playerPos.y) &&
            parsed.playerPos.x >= 24
              ? { x: parsed.playerPos.x, y: parsed.playerPos.y }
              : { x: DEFAULT_SAFE_PLAYER_POS.x, y: DEFAULT_SAFE_PLAYER_POS.y };

          return {
            version: 2,
            timestamp: typeof parsed.timestamp === 'number' ? parsed.timestamp : Date.now(),
            eraId: safeEra,
            currentMapId: parsed.currentMapId || DEFAULT_SAFE_MAP_ID,
            spawnId: parsed.spawnId || DEFAULT_SAFE_SPAWN_ID,
            playerPos: safePos,
            playerFacing: parsed.playerFacing ?? 'right',
            collectedMilestones: Array.isArray(parsed.collectedMilestones)
              ? parsed.collectedMilestones.filter((m): m is RomanMilestoneId =>
                  VALID_MILESTONES.includes(m)
                )
              : [],
            collectedEvidence: Array.isArray(parsed.collectedEvidence)
              ? parsed.collectedEvidence
              : [],
            questFlags: Array.isArray(parsed.questFlags) ? parsed.questFlags : [],
            discoveredMaps: Array.isArray(parsed.discoveredMaps)
              ? parsed.discoveredMaps
              : [DEFAULT_SAFE_MAP_ID],
            unlockedCheckpoints: Array.isArray(parsed.unlockedCheckpoints)
              ? parsed.unlockedCheckpoints
              : [DEFAULT_SAFE_SPAWN_ID],
            completedChapters: Array.isArray(parsed.completedChapters)
              ? parsed.completedChapters
              : [],
          };
        }

        if (parsed && typeof parsed.chapterId === 'string') {
          const migrated = migrateV1ToV2(parsed as SaveDataV1);
          SaveManager.save(migrated);
          return migrated;
        }
      }

      const rawV1 = localStorage.getItem(SAVE_KEY_V1);
      if (rawV1) {
        const parsedV1 = JSON.parse(rawV1) as SaveDataV1;
        if (parsedV1 && typeof parsedV1 === 'object') {
          const migrated = migrateV1ToV2(parsedV1);
          SaveManager.save(migrated);
          localStorage.removeItem(SAVE_KEY_V1);
          return migrated;
        }
      }

      return null;
    } catch {
      return null;
    }
  }

  public load(): SaveDataV2 | null {
    return SaveManager.load();
  }

  public static clear(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.removeItem(SAVE_KEY_V2);
      localStorage.removeItem(SAVE_KEY_V1);
    } catch {
      // Bỏ qua lỗi khi xóa localStorage
    }
  }

  public clear(): void {
    SaveManager.clear();
  }
}
