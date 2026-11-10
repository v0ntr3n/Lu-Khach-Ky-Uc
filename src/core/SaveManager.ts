/**
 * @file src/core/SaveManager.ts
 * @description Quản lý lưu trữ trạng thái tiến trình được đánh phiên bản (SaveV2)
 * với khả năng tự động di trú từ định dạng cũ (SaveV1) và khôi phục vị trí spawn an toàn.
 */

import type {
  CameraZoomPreset,
  EffectsQuality,
  EraId,
  GameSettings,
  RomanMilestoneId,
  SaveDataV1,
  SaveDataV2,
  SaveSummary,
} from '../types/game';

export const SAVE_KEY_V2 = 'lu_khach_ky_uc_save_v2';
export const SAVE_KEY_V1 = 'lu_khach_ky_uc_save_v1';
export const SETTINGS_KEY_V1 = 'lu_khach_ky_uc_settings_v1';

const VALID_ERAS: readonly EraId[] = ['1930', '1945', '1954_1975', '1986_2045'];
const VALID_CAMERA_ZOOMS: readonly CameraZoomPreset[] = ['wide', 'normal', 'close'];
const VALID_EFFECTS_QUALITIES: readonly EffectsQuality[] = ['full', 'subtle'];
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

const DEFAULT_ERA_CHAPTER_LABELS: Record<EraId, string> = {
  '1930': 'CHƯƠNG I · 1930 – 1931',
  '1945': 'CHƯƠNG II · 1939 – 1945',
  '1954_1975': 'CHƯƠNG III · 1954 – 1975',
  '1986_2045': 'CHƯƠNG IV · 1986 – 2045',
};

const DEFAULT_MAP_TITLES: Record<string, string> = {
  harbor_square: 'Quảng Trường Bến Cảng Hương Cảng (1930)',
  warehouse_interior: 'Kho Hàng Số 4 — Kho Hàng Bến Cảng',
  old_street: 'Phố Cổ Cửu Long (1930)',
  small_pier: 'Bến Thuyền Nhỏ — Cảng Thơm (1930)',
  safehouse_interior: 'Nhà An Toàn Giao Liên (1930)',
  shadow_alley: 'Hẻm Tối Tuần Tra (1930)',
  archive_courtyard: 'Sân Trước Viện Lưu Trữ (1930)',
  secret_archive: 'Phòng Lưu Trữ Bí Mật Cửu Long (1930)',
  tan_trao_banyan: 'Cây Đa Tân Trào (08/1945)',
  ba_dinh_square: 'Quảng Trường Ba Đình (02/09/1945)',
  pha_din_pass: 'Đèo Pha Đin — Tuyến Lửa Điện Biên (1954)',
  spring_1975_corridor: 'Hành Lang Mùa Xuân Đại Thắng (1975)',
  doi_moi_avenue: 'Đại Lộ Đổi Mới (1986)',
  nexus_2045_plaza: 'Quảng Trường Ký Ức Tương Lai (2045)',
};

export const DEFAULT_SAFE_MAP_ID = 'harbor_square';
export const DEFAULT_SAFE_SPAWN_ID = 'default_spawn';
export const DEFAULT_SAFE_PLAYER_POS = { x: 85, y: 148 } as const;

export const DEFAULT_GAME_SETTINGS: GameSettings = {
  muted: false,
  masterVolume: 0.85,
  reducedMotion: false,
  textScale: 'normal',
  showHints: true,
  cameraZoom: 'normal',
  effectsQuality: 'full',
  presenterModeUnlocked: false,
};

export function normalizeSettings(raw?: Partial<GameSettings> | null): GameSettings {
  const volume =
    typeof raw?.masterVolume === 'number' && Number.isFinite(raw.masterVolume)
      ? Math.min(1, Math.max(0, raw.masterVolume))
      : 0.85;

  const cameraZoom: CameraZoomPreset =
    raw?.cameraZoom && VALID_CAMERA_ZOOMS.includes(raw.cameraZoom)
      ? raw.cameraZoom
      : 'normal';

  const effectsQuality: EffectsQuality =
    raw?.effectsQuality && VALID_EFFECTS_QUALITIES.includes(raw.effectsQuality)
      ? raw.effectsQuality
      : 'full';

  return {
    muted: typeof raw?.muted === 'boolean' ? raw.muted : DEFAULT_GAME_SETTINGS.muted,
    masterVolume: volume,
    reducedMotion:
      typeof raw?.reducedMotion === 'boolean'
        ? raw.reducedMotion
        : DEFAULT_GAME_SETTINGS.reducedMotion,
    textScale: raw?.textScale === 'large' ? 'large' : 'normal',
    showHints:
      typeof raw?.showHints === 'boolean' ? raw.showHints : DEFAULT_GAME_SETTINGS.showHints,
    cameraZoom,
    effectsQuality,
    presenterModeUnlocked:
      typeof raw?.presenterModeUnlocked === 'boolean' ? raw.presenterModeUnlocked : false,
  };
}

/**
 * Chuẩn hóa cài đặt nhúng trong SaveDataV2: kiểm tra/kẹp biên đầy đủ các trường tùy chọn mới
 * khi chúng hiện diện, đồng thời giữ nguyên cấu trúc 4 trường cơ bản nếu bản lưu không ghi trường tùy chọn.
 */
export function normalizeEmbeddedSaveSettings(
  raw?: Partial<GameSettings> | null
): GameSettings {
  const full = normalizeSettings(raw);
  const embedded: GameSettings = {
    muted: full.muted,
    reducedMotion: full.reducedMotion,
    textScale: full.textScale,
    showHints: full.showHints,
  };

  if (raw && raw.masterVolume !== undefined) {
    embedded.masterVolume = full.masterVolume;
  }
  if (raw && raw.cameraZoom !== undefined) {
    embedded.cameraZoom = full.cameraZoom;
  }
  if (raw && raw.effectsQuality !== undefined) {
    embedded.effectsQuality = full.effectsQuality;
  }
  if (raw && raw.presenterModeUnlocked !== undefined) {
    embedded.presenterModeUnlocked = full.presenterModeUnlocked;
  }

  return embedded;
}

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
    activeQuestIds: [],
    completedQuestIds: [],
    completedObjectiveIds: [],
    dialogueHistory: [],
    settings: { ...DEFAULT_GAME_SETTINGS },
  };
}

export class SaveManager {
  public static readonly STORAGE_KEY_V2 = SAVE_KEY_V2;
  public static readonly STORAGE_KEY_V1 = SAVE_KEY_V1;
  public static readonly STORAGE_KEY_SETTINGS_V1 = SETTINGS_KEY_V1;

  public static normalizeSettings(raw?: Partial<GameSettings> | null): GameSettings {
    return normalizeSettings(raw);
  }

  public normalizeSettings(raw?: Partial<GameSettings> | null): GameSettings {
    return normalizeSettings(raw);
  }

  public static migrateV1ToV2(v1: SaveDataV1): SaveDataV2 {
    return migrateV1ToV2(v1);
  }

  public migrateV1ToV2(v1: SaveDataV1): SaveDataV2 {
    return migrateV1ToV2(v1);
  }

  public static saveSettings(settings: GameSettings): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const normalized = normalizeSettings(settings);
      localStorage.setItem(SETTINGS_KEY_V1, JSON.stringify(normalized));
    } catch {
      // Bỏ qua lỗi quota hoặc môi trường ẩn danh không cho phép localStorage
    }
  }

  public saveSettings(settings: GameSettings): void {
    SaveManager.saveSettings(settings);
  }

  public static loadSettings(): GameSettings {
    try {
      if (typeof localStorage === 'undefined') {
        return normalizeSettings(DEFAULT_GAME_SETTINGS);
      }

      const rawSettings = localStorage.getItem(SETTINGS_KEY_V1);
      if (rawSettings) {
        const parsed = JSON.parse(rawSettings) as Partial<GameSettings> | null;
        if (parsed && typeof parsed === 'object') {
          return normalizeSettings(parsed);
        }
      }

      const rawV2 = localStorage.getItem(SAVE_KEY_V2);
      if (rawV2) {
        const parsedSave = JSON.parse(rawV2) as Partial<SaveDataV2> | null;
        if (parsedSave && parsedSave.version === 2 && parsedSave.settings) {
          return normalizeSettings(parsedSave.settings);
        }
      }

      return normalizeSettings(DEFAULT_GAME_SETTINGS);
    } catch {
      return normalizeSettings(DEFAULT_GAME_SETTINGS);
    }
  }

  public loadSettings(): GameSettings {
    return SaveManager.loadSettings();
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

          const safeSettings = normalizeEmbeddedSaveSettings(parsed.settings);

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
            activeQuestIds: Array.isArray(parsed.activeQuestIds)
              ? parsed.activeQuestIds
              : [],
            completedQuestIds: Array.isArray(parsed.completedQuestIds)
              ? parsed.completedQuestIds
              : [],
            completedObjectiveIds: Array.isArray(parsed.completedObjectiveIds)
              ? parsed.completedObjectiveIds
              : [],
            dialogueHistory: Array.isArray(parsed.dialogueHistory)
              ? parsed.dialogueHistory
              : [],
            settings: safeSettings,
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

  public static hasSave(): boolean {
    return SaveManager.load() !== null;
  }

  public hasSave(): boolean {
    return SaveManager.hasSave();
  }

  public static getSaveSummary(
    chapters?: readonly { id: EraId; chapterNum: string; years: string }[],
    mapTitles?: Record<string, string>
  ): SaveSummary | null {
    const saved = SaveManager.load();
    if (!saved) {
      return null;
    }

    const matchedChapter = chapters?.find((c) => c.id === saved.eraId);
    const chapterLabel = matchedChapter
      ? `${matchedChapter.chapterNum} · ${matchedChapter.years}`
      : DEFAULT_ERA_CHAPTER_LABELS[saved.eraId] ?? saved.eraId;

    const mapId = saved.currentMapId || DEFAULT_SAFE_MAP_ID;
    const mapTitle =
      mapTitles?.[mapId] ?? DEFAULT_MAP_TITLES[mapId] ?? mapId;

    return {
      exists: true,
      timestamp: saved.timestamp,
      eraId: saved.eraId,
      chapterLabel,
      mapId,
      mapTitle,
      milestoneCount: new Set(saved.collectedMilestones).size,
      evidenceCount: new Set(saved.collectedEvidence).size,
    };
  }

  public getSaveSummary(
    chapters?: readonly { id: EraId; chapterNum: string; years: string }[],
    mapTitles?: Record<string, string>
  ): SaveSummary | null {
    return SaveManager.getSaveSummary(chapters, mapTitles);
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

  public static clearSave(): void {
    SaveManager.clear();
  }

  public clearSave(): void {
    SaveManager.clear();
  }

  public static clearAllIncludingSettings(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.removeItem(SAVE_KEY_V2);
      localStorage.removeItem(SAVE_KEY_V1);
      localStorage.removeItem(SETTINGS_KEY_V1);
    } catch {
      // Bỏ qua lỗi khi xóa localStorage
    }
  }

  public clearAllIncludingSettings(): void {
    SaveManager.clearAllIncludingSettings();
  }
}
