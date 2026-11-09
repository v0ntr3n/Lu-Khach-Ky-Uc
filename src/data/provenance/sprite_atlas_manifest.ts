/**
 * @file src/data/provenance/sprite_atlas_manifest.ts
 * @description Danh mục kiểm duyệt nguồn gốc Sprite Atlas (Curated Sprite Atlas Manifest)
 * tuân thủ kế hoạch tích hợp Local-First trong docs/plans/SPRITE_ATLAS.md:
 * - Chỉ chọn lọc các vật phẩm tĩnh (crates, lanterns, ledgers, scrolls, barrels) đã qua kiểm duyệt thủ công.
 * - Các nhân vật lịch sử và NPC 4 hướng sử dụng điểm ảnh gốc tự vẽ (Original Pixel Art),
 *   tuyệt đối không thay thế nhân vật lịch sử bằng hình AI đại trà.
 * - Cung cấp hàm kiểm tra hồi quy (validateSpriteAtlasManifest) phát hiện trùng ID, tràn biên hoặc chồng lấn khung chữ nhật.
 */

export interface CuratedSpriteManifestEntry {
  id: string;
  source:
    | 'Angais/pixel-art-atlas-1.4m (curated-adaptation)'
    | 'Lu-Khach-Ky-Uc Original Pixel Art';
  source_revision: string;
  row_id: string;
  caption: string;
  usage: string[];
  reviewed: boolean;
  license_note: string;
  atlas: string;
  rect: { x: number; y: number; w: number; h: number };
  pivot: { x: number; y: number };
}

export const CURATED_SPRITE_ATLAS_MANIFEST: readonly CuratedSpriteManifestEntry[] = [
  {
    id: 'prop_oil_lamp_a',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_00142_oil_lamp',
    caption: 'oil lamp',
    usage: ['chapter1:harbor_square', 'chapter1:warehouse_interior'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 0, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_sealed_scroll',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_00891_sealed_scroll',
    caption: 'sealed scroll',
    usage: ['chapter1:secret_archive', 'chapter1:safehouse_interior'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 16, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 14 },
  },
  {
    id: 'prop_shipping_crate',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_01204_wooden_crate',
    caption: 'crate',
    usage: ['chapter1:harbor_square', 'chapter1:warehouse_interior', 'chapter1:small_pier'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 32, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_dock_barrel',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_01588_dock_barrel',
    caption: 'barrel',
    usage: ['chapter1:harbor_square', 'chapter1:small_pier'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 48, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_ledger_book',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_02019_ledger_book',
    caption: 'ledger',
    usage: ['chapter1:warehouse_interior', 'chapter1:secret_archive'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 64, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 14 },
  },
  {
    id: 'prop_ink_bottle',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_02341_ink_bottle',
    caption: 'ink bottle',
    usage: ['chapter1:safehouse_interior', 'chapter1:secret_archive'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 80, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_cipher_telegram',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_02790_cipher_sheet',
    caption: 'cipher sheet',
    usage: ['chapter1:safehouse_interior'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 96, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 14 },
  },
  {
    id: 'prop_safety_lantern',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_03112_storm_lantern',
    caption: 'storm lantern checkpoint',
    usage: ['chapter1:all_zones_checkpoint'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 112, y: 0, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_tea_kettle',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_03405_tea_kettle',
    caption: 'tea kettle',
    usage: ['chapter1:old_street'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 0, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_bamboo_chest',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_03880_chest',
    caption: 'chest',
    usage: ['chapter1:safehouse_interior', 'chapter1:warehouse_interior'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 16, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_stone_bench',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_04115_stone_bench',
    caption: 'stone bench',
    usage: ['chapter1:archive_courtyard'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 32, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'prop_archive_seal',
    source: 'Angais/pixel-art-atlas-1.4m (curated-adaptation)',
    source_revision: '2024-11-curated-local-v1',
    row_id: 'row_04502_bronze_seal',
    caption: 'archive bronze seal',
    usage: ['chapter1:secret_archive'],
    reviewed: true,
    license_note:
      'Curated & palette-remapped to 12-tone Lacquer/Parchment palette; verified upstream dataset card for local prototype.',
    atlas: 'chapter1_props.png',
    rect: { x: 48, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'char_player_4dir',
    source: 'Lu-Khach-Ky-Uc Original Pixel Art',
    source_revision: 'original-authored-2026-v1',
    row_id: 'orig_char_traveler_2045',
    caption: 'Traveler 2045 4-direction turnaround',
    usage: ['player:all_chapters'],
    reviewed: true,
    license_note: 'First-party original pixel art authored specifically for Lu Khach Ky Uc.',
    atlas: 'chapter1_props.png',
    rect: { x: 64, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'char_liaison_4dir',
    source: 'Lu-Khach-Ky-Uc Original Pixel Art',
    source_revision: 'original-authored-2026-v1',
    row_id: 'orig_char_liaison_1930',
    caption: 'Kowloon Liaison 4-direction turnaround',
    usage: ['chapter1:harbor_square', 'chapter1:safehouse_interior'],
    reviewed: true,
    license_note: 'First-party original pixel art authored specifically for Lu Khach Ky Uc.',
    atlas: 'chapter1_props.png',
    rect: { x: 80, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'char_worker_4dir',
    source: 'Lu-Khach-Ky-Uc Original Pixel Art',
    source_revision: 'original-authored-2026-v1',
    row_id: 'orig_char_dock_worker_1930',
    caption: 'Harbor Worker & Boatman 4-direction turnaround',
    usage: ['chapter1:harbor_square', 'chapter1:small_pier'],
    reviewed: true,
    license_note: 'First-party original pixel art authored specifically for Lu Khach Ky Uc.',
    atlas: 'chapter1_props.png',
    rect: { x: 96, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
  {
    id: 'char_guard_4dir',
    source: 'Lu-Khach-Ky-Uc Original Pixel Art',
    source_revision: 'original-authored-2026-v1',
    row_id: 'orig_char_patrol_guard_1930',
    caption: 'Shadow Alley Patrol Guard 4-direction turnaround',
    usage: ['chapter1:shadow_alley'],
    reviewed: true,
    license_note: 'First-party original pixel art authored specifically for Lu Khach Ky Uc.',
    atlas: 'chapter1_props.png',
    rect: { x: 112, y: 16, w: 16, h: 16 },
    pivot: { x: 8, y: 15 },
  },
];

export function validateSpriteAtlasManifest(
  entries: readonly CuratedSpriteManifestEntry[] = CURATED_SPRITE_ATLAS_MANIFEST,
  atlasWidth = 128,
  atlasHeight = 128
): { valid: boolean; errors: string[] } {
  const errors: string[] = [];
  const seenIds = new Set<string>();

  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i];
    if (seenIds.has(entry.id)) {
      errors.push(`Duplicate sprite manifest ID: "${entry.id}"`);
    }
    seenIds.add(entry.id);

    if (!entry.reviewed) {
      errors.push(`Sprite "${entry.id}" has not been manually reviewed (reviewed === false)`);
    }
    if (!entry.source_revision || entry.source_revision === 'PIN_EXACT_REVISION') {
      errors.push(`Sprite "${entry.id}" is missing a pinned source_revision`);
    }
    if (!entry.license_note || entry.license_note.trim().length === 0) {
      errors.push(`Sprite "${entry.id}" is missing a license_note`);
    }

    const { x, y, w, h } = entry.rect;
    if (w <= 0 || h <= 0) {
      errors.push(`Sprite "${entry.id}" has non-positive dimensions (${w}x${h})`);
    }
    if (x < 0 || y < 0 || x + w > atlasWidth || y + h > atlasHeight) {
      errors.push(
        `Sprite "${entry.id}" rect (${x},${y},${w},${h}) exceeds atlas bounds (${atlasWidth}x${atlasHeight})`
      );
    }

    for (let j = i + 1; j < entries.length; j++) {
      const other = entries[j];
      if (other.atlas !== entry.atlas) continue;
      const r1 = entry.rect;
      const r2 = other.rect;
      const overlaps =
        r1.x < r2.x + r2.w &&
        r1.x + r1.w > r2.x &&
        r1.y < r2.y + r2.h &&
        r1.y + r1.h > r2.y;
      if (overlaps) {
        errors.push(
          `Sprite rect overlap in "${entry.atlas}" between "${entry.id}" and "${other.id}"`
        );
      }
    }
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}
