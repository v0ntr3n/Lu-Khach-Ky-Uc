/**
 * @file src/runtime/AssetLoader.ts
 * @description Bộ quản lý và lưu đệm (Singleton Cache) cho Sprite Atlas Pixel Art Đã Kiểm Duyệt (M5):
 * - Lưu đệm một lần (`Map`) cho cả descriptor JSON và phần tử ảnh/texture (`HTMLImageElement` / `ImageBitmap`),
 *   tuyệt đối không khởi tạo lại `new Image()` ở mỗi khung hình.
 * - Cưỡng chế bộ lọc điểm ảnh sắc nét `NEAREST` (`ctx.imageSmoothingEnabled = false`).
 * - Cung cấp hàm kiểm định hình học `validateAtlasRects` (kiểm tra trùng lặp ID, vượt biên độ rộng/cao,
 *   và chồng lấn hình chữ nhật giữa các frame).
 */

import type { Rect2D, Vec2 } from '../types/game';
import { CollisionWorld } from '../world/CollisionWorld';

export interface AtlasFrameRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface AtlasFrameEntry {
  id?: string;
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  frame?: AtlasFrameRect;
  rect?: AtlasFrameRect;
  pivot: Vec2;
  reviewed?: boolean;
  provenance?: string;
}

export interface AtlasMeta {
  app: string;
  image: string;
  size: { w: number; h: number };
  scale: number;
  filter: 'NEAREST' | string;
  palette?: string[];
}

export interface AtlasDescriptor {
  meta: AtlasMeta;
  frames: Record<string, AtlasFrameEntry> | AtlasFrameEntry[];
}

export interface NormalizedAtlasFrame {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  pivot: Vec2;
}

export interface AtlasValidationResult {
  valid: boolean;
  errors: string[];
  frameCount: number;
}

export const CHAPTER1_PROPS_ATLAS: AtlasDescriptor = {
  meta: {
    app: 'Lu-Khach-Ky-Uc Atlas Curator',
    image: 'chapter1_props.png',
    size: { w: 128, h: 128 },
    scale: 1,
    filter: 'NEAREST',
    palette: [
      '#0D0A08',
      '#1A1410',
      '#2E2218',
      '#4A3525',
      '#6E4E32',
      '#8C2D19',
      '#B83A2A',
      '#C89B3C',
      '#D4A017',
      '#E6C699',
      '#F4E8D1',
      '#2B4C5E',
    ],
  },
  frames: {
    prop_oil_lamp_a: {
      id: 'prop_oil_lamp_a',
      x: 0,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 0, y: 0, w: 16, h: 16 },
      rect: { x: 0, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_sealed_scroll: {
      id: 'prop_sealed_scroll',
      x: 16,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 16, y: 0, w: 16, h: 16 },
      rect: { x: 16, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 14 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_shipping_crate: {
      id: 'prop_shipping_crate',
      x: 32,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 32, y: 0, w: 16, h: 16 },
      rect: { x: 32, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_dock_barrel: {
      id: 'prop_dock_barrel',
      x: 48,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 48, y: 0, w: 16, h: 16 },
      rect: { x: 48, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_ledger_book: {
      id: 'prop_ledger_book',
      x: 64,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 64, y: 0, w: 16, h: 16 },
      rect: { x: 64, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 14 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_ink_bottle: {
      id: 'prop_ink_bottle',
      x: 80,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 80, y: 0, w: 16, h: 16 },
      rect: { x: 80, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_cipher_telegram: {
      id: 'prop_cipher_telegram',
      x: 96,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 96, y: 0, w: 16, h: 16 },
      rect: { x: 96, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 14 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_safety_lantern: {
      id: 'prop_safety_lantern',
      x: 112,
      y: 0,
      w: 16,
      h: 16,
      frame: { x: 112, y: 0, w: 16, h: 16 },
      rect: { x: 112, y: 0, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_tea_kettle: {
      id: 'prop_tea_kettle',
      x: 0,
      y: 16,
      w: 16,
      h: 16,
      frame: { x: 0, y: 16, w: 16, h: 16 },
      rect: { x: 0, y: 16, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_bamboo_chest: {
      id: 'prop_bamboo_chest',
      x: 16,
      y: 16,
      w: 16,
      h: 16,
      frame: { x: 16, y: 16, w: 16, h: 16 },
      rect: { x: 16, y: 16, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_stone_bench: {
      id: 'prop_stone_bench',
      x: 32,
      y: 16,
      w: 16,
      h: 16,
      frame: { x: 32, y: 16, w: 16, h: 16 },
      rect: { x: 32, y: 16, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    prop_archive_seal: {
      id: 'prop_archive_seal',
      x: 48,
      y: 16,
      w: 16,
      h: 16,
      frame: { x: 48, y: 16, w: 16, h: 16 },
      rect: { x: 48, y: 16, w: 16, h: 16 },
      pivot: { x: 8, y: 15 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    char_player_4dir: {
      id: 'char_player_4dir',
      x: 0,
      y: 32,
      w: 64,
      h: 24,
      frame: { x: 0, y: 32, w: 64, h: 24 },
      rect: { x: 0, y: 32, w: 64, h: 24 },
      pivot: { x: 8, y: 22 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    char_liaison_4dir: {
      id: 'char_liaison_4dir',
      x: 64,
      y: 32,
      w: 64,
      h: 24,
      frame: { x: 64, y: 32, w: 64, h: 24 },
      rect: { x: 64, y: 32, w: 64, h: 24 },
      pivot: { x: 8, y: 22 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    char_worker_4dir: {
      id: 'char_worker_4dir',
      x: 0,
      y: 56,
      w: 64,
      h: 24,
      frame: { x: 0, y: 56, w: 64, h: 24 },
      rect: { x: 0, y: 56, w: 64, h: 24 },
      pivot: { x: 8, y: 22 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
    char_guard_4dir: {
      id: 'char_guard_4dir',
      x: 64,
      y: 56,
      w: 64,
      h: 24,
      frame: { x: 64, y: 56, w: 64, h: 24 },
      rect: { x: 64, y: 56, w: 64, h: 24 },
      pivot: { x: 8, y: 22 },
      reviewed: true,
      provenance: 'curated_son_mai_12color_local',
    },
  },
};

function normalizeFrameEntry(
  fallbackId: string,
  entry: AtlasFrameEntry
): NormalizedAtlasFrame {
  const x = entry.x ?? entry.frame?.x ?? entry.rect?.x ?? 0;
  const y = entry.y ?? entry.frame?.y ?? entry.rect?.y ?? 0;
  const w = entry.w ?? entry.frame?.w ?? entry.rect?.w ?? 0;
  const h = entry.h ?? entry.frame?.h ?? entry.rect?.h ?? 0;
  return {
    id: entry.id ?? fallbackId,
    x,
    y,
    w,
    h,
    pivot: entry.pivot ?? { x: Math.round(w * 0.5), y: h },
  };
}

/**
 * Kiểm định hình học của một `AtlasDescriptor`:
 * - Kiểm tra kích thước `meta.size` dương và `meta.filter === 'NEAREST'`.
 * - Không có frame ID trùng lặp.
 * - Mọi frame đều nằm gọn trong biên `[0, meta.size.w] x [0, meta.size.h]`.
 * - Không có bất kỳ cặp hình chữ nhật frame nào đè lên nhau (zero rectangle overlap).
 */
export function validateAtlasRects(
  descriptor: AtlasDescriptor
): AtlasValidationResult {
  const errors: string[] = [];

  const atlasW = descriptor?.meta?.size?.w ?? 0;
  const atlasH = descriptor?.meta?.size?.h ?? 0;

  if (atlasW <= 0 || atlasH <= 0) {
    errors.push(
      `Invalid atlas dimensions: ${atlasW}x${atlasH} (must be positive).`
    );
  }

  if (descriptor?.meta?.filter !== 'NEAREST') {
    errors.push(
      `Atlas filter must be "NEAREST" (received "${descriptor?.meta?.filter}").`
    );
  }

  const normalizedList: NormalizedAtlasFrame[] = [];
  const seenIds = new Set<string>();

  if (Array.isArray(descriptor?.frames)) {
    for (let i = 0; i < descriptor.frames.length; i++) {
      const item = descriptor.frames[i];
      const norm = normalizeFrameEntry(`frame_${i}`, item);
      if (seenIds.has(norm.id)) {
        errors.push(`Duplicate frame ID "${norm.id}".`);
      } else {
        seenIds.add(norm.id);
      }
      normalizedList.push(norm);
    }
  } else if (descriptor?.frames && typeof descriptor.frames === 'object') {
    for (const [key, item] of Object.entries(descriptor.frames)) {
      const norm = normalizeFrameEntry(key, item);
      if (seenIds.has(norm.id)) {
        errors.push(`Duplicate frame ID "${norm.id}".`);
      } else {
        seenIds.add(norm.id);
      }
      normalizedList.push(norm);
    }
  } else {
    errors.push('Atlas descriptor contains no frames.');
  }

  for (const f of normalizedList) {
    if (f.w <= 0 || f.h <= 0) {
      errors.push(
        `Frame "${f.id}" has non-positive dimensions (${f.w}x${f.h}).`
      );
      continue;
    }
    if (f.x < 0 || f.y < 0 || f.x + f.w > atlasW || f.y + f.h > atlasH) {
      errors.push(
        `Frame "${f.id}" (${f.x}, ${f.y}, ${f.w}x${f.h}) exceeds atlas bounds (0..${atlasW}, 0..${atlasH}).`
      );
    }
  }

  for (let i = 0; i < normalizedList.length; i++) {
    const a = normalizedList[i];
    const rectA: Rect2D = { x: a.x, y: a.y, width: a.w, height: a.h };
    for (let j = i + 1; j < normalizedList.length; j++) {
      const b = normalizedList[j];
      const rectB: Rect2D = { x: b.x, y: b.y, width: b.w, height: b.h };
      if (CollisionWorld.rectsOverlap(rectA, rectB)) {
        errors.push(
          `Frame "${a.id}" (${a.x},${a.y},${a.w}x${a.h}) overlaps with frame "${b.id}" (${b.x},${b.y},${b.w}x${b.h}).`
        );
      }
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    frameCount: normalizedList.length,
  };
}

export class AssetLoader {
  private static instance: AssetLoader | null = null;

  public static getInstance(): AssetLoader {
    if (!AssetLoader.instance) {
      AssetLoader.instance = new AssetLoader();
    }
    return AssetLoader.instance;
  }

  private readonly descriptorCache = new Map<string, AtlasDescriptor>();
  private readonly textureCache = new Map<
    string,
    HTMLImageElement | ImageBitmap | HTMLCanvasElement | { id: string; width: number; height: number }
  >();
  public imageInstantiationCount = 0;

  constructor() {
    this.descriptorCache.set('chapter1_props', CHAPTER1_PROPS_ATLAS);
  }

  /**
   * Cưỡng chế chế độ nội suy điểm ảnh sắc nét NEAREST (`imageSmoothingEnabled = false`) trên ngữ cảnh vẽ 2D.
   */
  public applyNearestNeighbor(ctx: CanvasRenderingContext2D): void {
    ctx.imageSmoothingEnabled = false;
  }

  /**
   * Trả về descriptor của một Sprite Atlas từ bộ đệm.
   */
  public getAtlasDescriptor(atlasId = 'chapter1_props'): AtlasDescriptor {
    const cached = this.descriptorCache.get(atlasId);
    if (cached) {
      return cached;
    }
    return CHAPTER1_PROPS_ATLAS;
  }

  /**
   * Đăng ký hoặc cập nhật descriptor vào bộ đệm.
   */
  public registerAtlasDescriptor(
    atlasId: string,
    descriptor: AtlasDescriptor
  ): void {
    this.descriptorCache.set(atlasId, descriptor);
  }

  /**
   * Truy xuất thông tin một frame cụ thể trong Atlas.
   */
  public getFrame(
    atlasId: string,
    frameId: string
  ): NormalizedAtlasFrame | null {
    const desc = this.getAtlasDescriptor(atlasId);
    if (Array.isArray(desc.frames)) {
      const found = desc.frames.find((f) => f.id === frameId);
      return found ? normalizeFrameEntry(frameId, found) : null;
    }
    const entry = desc.frames[frameId];
    return entry ? normalizeFrameEntry(frameId, entry) : null;
  }

  /**
   * Trả về phần tử ảnh/texture đã lưu đệm cho `atlasId` (chỉ khởi tạo đúng 1 lần duy nhất).
   */
  public getOrCreateTexture(
    atlasId = 'chapter1_props'
  ): HTMLImageElement | ImageBitmap | HTMLCanvasElement | { id: string; width: number; height: number } {
    const existing = this.textureCache.get(atlasId);
    if (existing) {
      return existing;
    }

    this.imageInstantiationCount += 1;
    const desc = this.getAtlasDescriptor(atlasId);

    if (typeof Image !== 'undefined') {
      const img = new Image(desc.meta.size.w, desc.meta.size.h);
      img.src = `/assets/atlases/${desc.meta.image}`;
      this.textureCache.set(atlasId, img);
      return img;
    }

    const handle = {
      id: atlasId,
      width: desc.meta.size.w,
      height: desc.meta.size.h,
    };
    this.textureCache.set(atlasId, handle);
    return handle;
  }

  public validateAtlasRects(
    descriptor: AtlasDescriptor = CHAPTER1_PROPS_ATLAS
  ): AtlasValidationResult {
    return validateAtlasRects(descriptor);
  }

  public getCachedTextureCount(): number {
    return this.textureCache.size;
  }
}

export const ASSET_LOADER = AssetLoader.getInstance();

export function getAtlasDescriptor(atlasId = 'chapter1_props'): AtlasDescriptor {
  return ASSET_LOADER.getAtlasDescriptor(atlasId);
}
