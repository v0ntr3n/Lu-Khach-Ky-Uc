/**
 * @file src/engine/DecorAtlas.ts
 * @description Kho ma trận điểm ảnh 20-màu Eastward cho:
 * 1. 7 nguyên mẫu NPC theo thời kỳ lịch sử (Bà Năm, Lão Chèo Đò, Việt Minh 1945, Dân công 1954,
 *    Chiến sĩ Thông tin 1975, Chuyên gia Đổi Mới 1986, Giám tuyển 2045) khớp chân dung hội thoại.
 * 2. 36 ma trận tĩnh vật trang trí (Decor Sprites) có bộ đệm Offscreen Canvas O(1) phủ khắp 14 bản đồ
 *    ở 3 lớp: mặt đất ('ground'), sắp xếp chiều sâu ('ysort') và vòm cao ('canopy'), giữ nguyên 100%
 *    va chạm vật lý và 39 tuyến dẫn đường A*.
 */

import type { NormalizedMapData } from '../types/game';
import { COLOR_MAP, type DirectionalCharacterSet, drawMatrixSprite } from './SpriteAtlas';

// ============================================================================
// PHẦN 1: BỘ DỰNG NGUYÊN MẪU NHÂN VẬT 4 HƯỚNG THEO THỜI KỲ (16×22)
// ============================================================================

interface ArchetypeAppearanceSpec {
  /** 4 dòng đỉnh đầu / mũ / nón (mặt trước) */
  hatDown: [string, string, string, string];
  /** 4 dòng đỉnh đầu / mũ / nón (nhìn từ sau lưng) */
  hatUp: [string, string, string, string];
  /** 4 dòng đỉnh đầu / mũ / nón (nhìn nghiêng phải) */
  hatSide: [string, string, string, string];
  /** Dòng cằm / râu / cổ áo (row 6, 16 ký tự) */
  chinDown?: string;
  /** Dòng mắt mặt trước (row 5, 16 ký tự) */
  eyesDown?: string;
  /** 7 dòng thân áo mặt trước (rows 7..13) */
  torsoDown: [string, string, string, string, string, string, string];
  /** 7 dòng thân áo sau lưng (rows 7..13) */
  torsoUp: [string, string, string, string, string, string, string];
  /** 7 dòng thân áo nhìn nghiêng (rows 7..13) */
  torsoSide: [string, string, string, string, string, string, string];
  /** Màu quần chính & màu giày (1 ký tự trong COLOR_MAP) */
  pantsChar: string;
  bootChar: string;
}

function padRow16(row: string): string {
  if (row.length === 16) return row;
  if (row.length > 16) return row.slice(0, 16);
  return row.padEnd(16, '.');
}

function buildArchetypeSet(spec: ArchetypeAppearanceSpec): DirectionalCharacterSet {
  const p = spec.pantsChar;
  const b = spec.bootChar;
  const eyes = spec.eyesDown ?? '..KPWKPPPPKWPK..';
  const chin = spec.chinDown ?? '...KPPPPPPPPK...';

  const downBase = [
    ...spec.hatDown,
    '..KPPPPPPPPPPK..',
    eyes,
    chin,
    ...spec.torsoDown,
    `..K${p}${p}${p}${p}KK${p}${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `...K${p}${p}K..K${p}${p}K...`,
    `...K${p}${p}K..K${p}${p}K...`,
    `...K${b}${b}K..K${b}${b}K...`,
    `..K${b}${b}${b}K..K${b}${b}${b}K..`,
    '..KKKKK..KKKKK..',
  ].map(padRow16);

  const downBreathe = [
    '................',
    ...downBase.slice(0, 14),
    ...downBase.slice(15),
  ].map(padRow16);

  const downWalk1 = [
    ...downBase.slice(0, 14),
    `..K${p}${p}${p}${p}KK${p}${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `..K${b}${b}${b}K..K${p}${p}${p}K..`,
    `..KKKKK...K${p}${p}K..`,
    `..........K${p}${p}K..`,
    `..........K${b}${b}K..`,
    `.........K${b}${b}${b}K..`,
    '.........KKKKK..',
  ].map(padRow16);

  const downWalk2 = [
    ...downBase.slice(0, 14),
    `..K${p}${p}${p}${p}KK${p}${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${b}${b}${b}K..`,
    `..K${p}${p}K...KKKKK..`,
    `..K${p}${p}K..........`,
    `..K${b}${b}K..........`,
    `..K${b}${b}${b}K.........`,
    '..KKKKK.........',
  ].map(padRow16);

  const upBase = [
    ...spec.hatUp,
    '..KIIIIIIIIIIK..',
    '..KIIIIIIIIIIK..',
    '...KIIIIIIIIK...',
    ...spec.torsoUp,
    `..K${p}${p}${p}${p}KK${p}${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `...K${p}${p}K..K${p}${p}K...`,
    `...K${p}${p}K..K${p}${p}K...`,
    `...K${b}${b}K..K${b}${b}K...`,
    `..K${b}${b}${b}K..K${b}${b}${b}K..`,
    '..KKKKK..KKKKK..',
  ].map(padRow16);

  const upBreathe = [
    '................',
    ...upBase.slice(0, 14),
    ...upBase.slice(15),
  ].map(padRow16);

  const upWalk1 = [
    ...upBase.slice(0, 14),
    ...downWalk1.slice(14),
  ].map(padRow16);

  const upWalk2 = [
    ...upBase.slice(0, 14),
    ...downWalk2.slice(14),
  ].map(padRow16);

  const sideBase = [
    ...spec.hatSide,
    '...KIIPPPPPPPK..',
    '...KIIPPPPPKWK..',
    '....KPPPPPPPPK..',
    ...spec.torsoSide,
    `....K${p}${p}${p}${p}${p}${p}K....`,
    `....K${p}${p}${p}${p}${p}K.....`,
    `....K${p}${p}${p}${p}K......`,
    `.....K${p}${p}${p}K......`,
    `.....K${p}${p}${p}K......`,
    `.....K${b}${b}${b}K......`,
    `....K${b}${b}${b}${b}${b}K.....`,
    '....KKKKKKK.....',
  ].map(padRow16);

  const sideBreathe = [
    '................',
    ...sideBase.slice(0, 14),
    ...sideBase.slice(15),
  ].map(padRow16);

  const sideWalk1 = [
    ...sideBase.slice(0, 14),
    `....K${p}${p}${p}${p}${p}${p}K....`,
    `...K${p}${p}${p}KK${p}${p}${p}K...`,
    `..K${p}${p}${p}K..K${p}${p}${p}K..`,
    `..K${b}${b}K....K${p}${p}K..`,
    `.K${b}${b}${b}K....K${b}${b}K..`,
    `.KKKKK....K${b}${b}${b}K.`,
    '..........KKKKK.',
    '................',
  ].map(padRow16);

  const sideWalk2 = [
    ...sideBase.slice(0, 14),
    `....K${p}${p}${p}${p}${p}${p}K....`,
    `....K${p}${p}${p}${p}${p}K.....`,
    `...K${p}${p}${p}KK${p}${p}K....`,
    `...K${p}${p}K..K${b}${b}K...`,
    `...K${b}${b}K..K${b}${b}${b}K..`,
    `..K${b}${b}${b}K..KKKKK..`,
    '..KKKKK.........',
    '................',
  ].map(padRow16);

  return {
    downIdle: downBase,
    downBreathe,
    downWalk1,
    downWalk2,
    upIdle: upBase,
    upBreathe,
    upWalk1,
    upWalk2,
    sideIdle: sideBase,
    sideBreathe,
    sideWalk1,
    sideWalk2,
  };
}

/** Bà Năm Trà Đá (1930) — Khăn vấn tóc truyền thống, áo bà ba nâu đồng, tạp dề giấy dó, quần lĩnh đen */
export const VENDOR_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIBBBBBBBBIK..',
    '..KIIIIIIIIIIK..',
  ],
  hatUp: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIBBBBBBBBIK..',
    '..KIIIKKKKIIIK..',
  ],
  hatSide: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIBBBBBBBIK...',
    '..KIIIIIIIIIK...',
  ],
  torsoDown: [
    '..KBBBPWWPBBBK..',
    '.KBBBBPPPPBBBBK.',
    '.KBBMBPPPPBMBBK.',
    '.KBBBBPPPPBBBBK.',
    '.KPBBWWWWWWBBPK.',
    '..KBBWWWWWWBBK..',
    '..KBBPPPPPPBBK..',
  ],
  torsoUp: [
    '..KBBBBBBBBBBK..',
    '.KBBBBMMMMBBBBK.',
    '.KBBMBBBBBMBBBK.',
    '.KBBBBBBBBBBBBK.',
    '.KBBBBMMMMBBBBK.',
    '..KBBBBBBBBBBK..',
    '..KBBBBMMBBBBK..',
  ],
  torsoSide: [
    '...KBBBBPPBBK...',
    '...KBBBBPPPBBK..',
    '...KBBMBPPPBBK..',
    '...KBBBBPPPBBK..',
    '...KPBBBWWWWBK..',
    '....KBBBWWWWBK..',
    '....KBBBPPPPBK..',
  ],
  pantsChar: 'L',
  bootChar: 'M',
});

/** Lão Chèo Đò (1930) — Nón lá rộng vành vàng đồng, râu bạc, áo nâu sồng & khăn rằn vắt vai */
export const BOATMAN_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '......KKKK......',
    '....KGGGGGGK....',
    '..KGGGGGGGGGGK..',
    '.KBBBBGGGGGBBBK.',
  ],
  hatUp: [
    '......KKKK......',
    '....KGGGGGGK....',
    '..KGGGGGGGGGGK..',
    '.KBBBBGGGGGBBBK.',
  ],
  hatSide: [
    '......KKKK......',
    '....KGGGGGGK....',
    '..KGGGGGGGGGGK..',
    '.KBBBBGGGGGBBBK.',
  ],
  chinDown: '...KPWWWWWWPK...',
  torsoDown: [
    '..KMMWPWWPWMMK..',
    '.KMMMWPWWPWMMMK.',
    '.KMMMBMMMMBMMMK.',
    '.KMMMMMMMMMMMMK.',
    '.KPMMSSSSSSMMK..',
    '..KMMMMMMMMMMK..',
    '..KMMMMKKMMMMK..',
  ],
  torsoUp: [
    '..KMMWPPPPWMMK..',
    '.KMMMMMMMMMMMMK.',
    '.KMMMBMMMMBMMMK.',
    '.KMMMMMMMMMMMMK.',
    '.KMMMSSSSSSMMMK.',
    '..KMMMMMMMMMMK..',
    '..KMMMMKKMMMMK..',
  ],
  torsoSide: [
    '...KMMWPPWMMK...',
    '...KMMWPPWMMMK..',
    '...KMMBMMBMMMK..',
    '...KMMMMMMMMMK..',
    '...KPMSSSSSMMK..',
    '....KMMMMMMMMK..',
    '....KMMMMMMMMK..',
  ],
  pantsChar: 'S',
  bootChar: 'D',
});

/** Giao Liên Việt Minh / Nhân Chứng Ba Đình (1945) — Mũ ca-lô gắn sao vàng, áo kaki, khăn quàng đỏ */
export const CADRE_1945_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK....',
    '...KMMMRGMMMK...',
    '..KBBBBWGWBBBBK.',
    '..KMMMMMMMMMMK..',
  ],
  hatUp: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KBBBBBBBBBBK..',
    '..KMMMMMMMMMMK..',
  ],
  hatSide: [
    '....KKKKKKKK....',
    '...KMMMMRGMK....',
    '..KBBBBBBWGK....',
    '..KMMMMMMMMK....',
  ],
  torsoDown: [
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRBBBBBK.',
    '.KBBMBBBBBMBBBK.',
    '.KBBBBMBBMBBBBK.',
    '.KPBBMGGGGMBBPK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
  ],
  torsoUp: [
    '..KRRRRRRRRRRK..',
    '.KBBBBBBBBBBBBK.',
    '.KBBMBBBBBMBBBK.',
    '.KBBBBMMMMBBBBK.',
    '.KBBBBGGGGGBBBK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
  ],
  torsoSide: [
    '...KRRRRVVRRK...',
    '...KBBBBRRBBBK..',
    '...KBBMBBBMBBK..',
    '...KBBBBMBBBBK..',
    '...KPBBGGGGMBK..',
    '....KBBBBBBBBK..',
    '....KBBBBBBBBK..',
  ],
  pantsChar: 'M',
  bootChar: 'D',
});

/** Dân Công / Tiếp Vận Đèo Pha Đin (1954) — Mũ cối nan phủ lưới lá rêu ngụy trang, áo trấn thủ xanh rêu */
export const SOLDIER_1954_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK....',
    '...KEHEEEHEEK...',
    '..KEEEEEHEEEEEK.',
    '.KBBBBBBBBBBBBK.',
  ],
  hatUp: [
    '....KKKKKKKK....',
    '...KEHEEEHEEK...',
    '..KEEEEEEEEEEEK.',
    '.KBBBBBBBBBBBBK.',
  ],
  hatSide: [
    '....KKKKKKKK....',
    '...KEHEEEHEK....',
    '..KEEEEEEEEK....',
    '.KBBBBBBBBBBK...',
  ],
  torsoDown: [
    '..KFFEEPPPEFFK..',
    '.KEFFEPPPPEFFEK.',
    '.KEFEEPPPEEFEK..',
    '.KEFFEEEEEEFFEK.',
    '.KPEEMGGGGMEEPK.',
    '..KEEEEFFEEEEK..',
    '..KEEEEKKEEEEK..',
  ],
  torsoUp: [
    '..KFFBBBBBBFFK..',
    '.KEFFBBBBBBFFEK.',
    '.KEFEBBBBBBFEK..',
    '.KEFFBBBBBBFFEK.',
    '.KEEEGGGGGGEEEK.',
    '..KEEEEFFEEEEK..',
    '..KEEEEKKEEEEK..',
  ],
  torsoSide: [
    '...KFFEEPPFFK...',
    '...KEFFEPPPFEK..',
    '...KEFEEPPFFEK..',
    '...KEFFEEEEFEK..',
    '...KPEEGGGMEEK..',
    '....KEEEEEEEFK..',
    '....KEEEEEEEFK..',
  ],
  pantsChar: 'F',
  bootChar: 'D',
});

/** Chiến Sĩ Thông Tin Mùa Xuân (1975) — Mũ cối sao vàng, quân phục xanh lá rừng, đài vô tuyến & ăng-ten */
export const COMMANDER_1975_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK..G.',
    '...KEEEEREEEEKG.',
    '..KEEEERWGEEEEK.',
    '.KFFFFEEEEEEFFK.',
  ],
  hatUp: [
    '....KKKKKKKK..G.',
    '...KEEEEEEEEKG..',
    '..KEEEEEEEEEEK..',
    '.KFFFFEEEEEEFFK.',
  ],
  hatSide: [
    '.G..KKKKKKKK....',
    '.GK.KEEEERGK....',
    '..K.KEEEEWGK....',
    '..KFFFFEEEEFK...',
  ],
  torsoDown: [
    '..KVRREEEERRVK..',
    '.KEEEEEEEEEEEEK.',
    '.KEFEEGGGGEEFEK.',
    '.KEEEEEEEEEEEEK.',
    '.KPEEMGGGGMEEPK.',
    '..KEEEEFFEEEEK..',
    '..KEEEEKKEEEEK..',
  ],
  torsoUp: [
    '..KEESSSSSSEEK..',
    '.KEESAAAAAASFEK.',
    '.KEESAGGGGASFEK.',
    '.KEESAAAAAASFEK.',
    '.KEEEGGGGGGEEEK.',
    '..KEEEEFFEEEEK..',
    '..KEEEEKKEEEEK..',
  ],
  torsoSide: [
    '..KAAEERRVEEEK..',
    '..KAAEEEEEEEEFK.',
    '..KAGEEGGGEEEFK.',
    '..KAAEEEEEEEEFK.',
    '...KPEEGGGMEEFK.',
    '....KEEEEEEEEFK.',
    '....KEEEEEEEEFK.',
  ],
  pantsChar: 'F',
  bootChar: 'K',
});

/** Chuyên Gia Kinh Tế Đổi Mới (1986) — Tóc rẽ ngôi kính đồng, sơ mi trắng ngà, ghi-lê chàm & cà vạt đỏ */
export const REFORM_1986_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIIIIIIIIIIK..',
    '..KIIPPPPPPIIK..',
  ],
  hatUp: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIIIIIIIIIIK..',
    '..KIIIIIIIIIIK..',
  ],
  hatSide: [
    '....KKKKKKKK....',
    '...KIIIIIIIIK...',
    '..KIIIIIIIIIK...',
    '..KIIIIPPPPPK...',
  ],
  eyesDown: '..KGGKGGGGKGGK..',
  torsoDown: [
    '..KUUWWRRWWUUK..',
    '.KUUUWWRRWWUUUK.',
    '.KUUWWWRRWWWUUK.',
    '.KUUWWWRRWWWUUK.',
    '.KPUUUGGGGUUUPK.',
    '..KUUUUWWUUUUK..',
    '..KUUUUKKUUUUK..',
  ],
  torsoUp: [
    '..KUUUUUUUUUUK..',
    '.KUUUUUUUUUUUUK.',
    '.KUUNUUUUUUNUUK.',
    '.KUUUUUUUUUUUUK.',
    '.KUUUUGGGGUUUUK.',
    '..KUUUUUUUUUUK..',
    '..KUUUUKKUUUUK..',
  ],
  torsoSide: [
    '...KUUWWRWWUK...',
    '...KUUWWRWWUUK..',
    '...KUUWWRWWUUK..',
    '...KUUWWWWMMMK..',
    '...KPUUGGGMMMK..',
    '....KUUUUUMMMK..',
    '....KUUUUUUUUK..',
  ],
  pantsChar: 'T',
  bootChar: 'K',
});

/** Giám Tuyển Lưu Trữ Kỷ Nguyên Số (2045) — Kính quang học chu sa, áo khoác cổ cao chàm đêm viền mạch vàng đồng */
export const ARCHIVIST_2045_SET: DirectionalCharacterSet = buildArchetypeSet({
  hatDown: [
    '....KKKKKKKK....',
    '...KIIGGGIIK....',
    '..KIIIIIIIIIIK..',
    '..KIIPPPPPPIIK..',
  ],
  hatUp: [
    '....KKKKKKKK....',
    '...KIIGGGIIIK...',
    '..KIIIIIIIIIIK..',
    '..KIIIIIIIIIIK..',
  ],
  hatSide: [
    '....KKKKKKKK....',
    '...KIIGGGIIIK...',
    '..KIIIIIIIIIK...',
    '..KIIIIPPPPPK...',
  ],
  eyesDown: '..KVVVVWWVVVVK..',
  torsoDown: [
    '..KNNNGGGGNNNK..',
    '.KNGNUUUUUUNGK..',
    '.KNGNUVVVVUNGK..',
    '.KNGNUUUUUUNGK..',
    '.KPNNWWWWWWNNPK.',
    '..KNNNUGGUNNNK..',
    '..KNNNNKKNNNNK..',
  ],
  torsoUp: [
    '..KNNNGGGGNNNK..',
    '.KNGNUUUUUUNGNK.',
    '.KNGNUGGGGUNGNK.',
    '.KNGNUUUUUUNGNK.',
    '.KNNNWWWWWWNNNK.',
    '..KNNNUGGUNNNK..',
    '..KNNNNKKNNNNK..',
  ],
  torsoSide: [
    '...KNNGGGNNNK...',
    '...KNGUUUUUNGK..',
    '...KNGUVVVUNGK..',
    '...KNGUUUUUNGK..',
    '...KPNWWWWWNNK..',
    '....KNNUGGUNNK..',
    '....KNNNNNNNNK..',
  ],
  pantsChar: 'N',
  bootChar: 'G',
});

// ============================================================================
// PHẦN 2: KHO 32 MA TRẬN PIXEL-ART DECOR 20-MÀU CÓ CACHE OFFSCREEN CANVAS
// ============================================================================

export type DecorRenderLayer = 'ground' | 'ysort' | 'canopy';

export type DecorSpriteId =
  | 'decor_reed_mat'
  | 'decor_rain_puddle_lantern'
  | 'decor_fallen_banyan_leaves'
  | 'decor_cobble_drain_grate'
  | 'decor_crimson_runner_rug'
  | 'decor_dock_rope_coil'
  | 'decor_wood_barrels'
  | 'decor_iron_anchor'
  | 'decor_fish_baskets'
  | 'decor_dock_handcart'
  | 'decor_tea_stall_set'
  | 'decor_vintage_bicycle'
  | 'decor_ceramic_planter'
  | 'decor_street_signpost'
  | 'decor_bamboo_cargo_cages'
  | 'decor_scroll_rack'
  | 'decor_printing_table'
  | 'decor_lacquer_cabinet'
  | 'decor_brazier_stool'
  | 'decor_alley_rain_vats'
  | 'decor_stone_incense_stele'
  | 'decor_bonsai_pedestal'
  | 'decor_bamboo_grove'
  | 'decor_camp_brazier'
  | 'decor_flag_stanchion'
  | 'decor_loudspeaker_pole'
  | 'decor_pack_bicycle'
  | 'decor_sandbag_emplacement'
  | 'decor_highway_milestone'
  | 'decor_field_radio_desk'
  | 'decor_newsstand_1986'
  | 'decor_classic_scooter'
  | 'decor_solar_bench_2045'
  | 'decor_holo_lotus_stele'
  | 'decor_banyan_canopy_crown'
  | 'decor_festive_lantern_string';

export interface ZoneDecorPlacement {
  id: string;
  spriteId: DecorSpriteId;
  /** Tâm neo X trên bản đồ (world px) */
  x: number;
  /** Tâm chân Y trên bản đồ (world px, dùng cho Y-sort khi layer === 'ysort') */
  y: number;
  layer: DecorRenderLayer;
  flipX?: boolean;
}

export const DECOR_SPRITE_MATRICES: Record<DecorSpriteId, string[]> = {
  // 1. Chiếu cói / Thảm cói đan tay (22×14)
  decor_reed_mat: [
    '.KKKKKKKKKKKKKKKKKKKK.',
    'KRRRRRRRRRRRRRRRRRRRRK',
    'KRGGGGGGGGGGGGGGGGGGRK',
    'KRGPBGPBGPBGPBGPBGPGRK',
    'KRGBPBGPBGPBGPBGPBGPRK',
    'KRGPBGPBGPBGPBGPBGPGRK',
    'KRGBPBGPBGPBGPBGPBGPRK',
    'KRGPBGPBGPBGPBGPBGPGRK',
    'KRGBPBGPBGPBGPBGPBGPRK',
    'KRGPBGPBGPBGPBGPBGPGRK',
    'KRGGGGGGGGGGGGGGGGGGRK',
    'KRRRRRRRRRRRRRRRRRRRRK',
    '.KKKKKKKKKKKKKKKKKKKK.',
    '..OOOOOOOOOOOOOOOOOO..',
  ],

  // 2. Vũng nước mưa đêm phản chiếu ánh đèn vàng (20×10)
  decor_rain_puddle_lantern: [
    '....NNNNNNNNNNNN....',
    '..NNUUUUUUUUUUUUNN..',
    '.NUUUUCCCCCCCCUUUUN.',
    'NUUUCCGWWGGGCCUUUUUN',
    'NUUUCCCGGGGCCCUUUUUN',
    '.NUUUUUCCCCUUUUUUUN.',
    '..NNUUUUUUUUUUUUNN..',
    '....NNNNNNNNNNNN....',
    '....................',
    '....................',
  ],

  // 3. Cụm lá đa vàng & cánh hoa rụng trên sân đá (18×10)
  decor_fallen_banyan_leaves: [
    '....GG........EE..',
    '..GGB...........EH',
    '..........RR......',
    '.EH.....GGB.......',
    '..F..........GG...',
    '......EE....GWB...',
    '.GG....F..........',
    '..B.........EH....',
    '......GG.....F....',
    '..................',
  ],

  // 4. Nắp cống gang thoát nước mưa phố cổ (16×12)
  decor_cobble_drain_grate: [
    '.KKKKKKKKKKKKKK.',
    'KTTTTTTTTTTTTTTK',
    'KTAAKAAKAAKAAKTK',
    'KTAAKAAKAAKAAKTK',
    'KTTTTTTTTTTTTTTK',
    'KTAAKAAKAAKAAKTK',
    'KTAAKAAKAAKAAKTK',
    'KTTTTTTTTTTTTTTK',
    'KFFTTTTTTTTTTFFK',
    '.KKKKKKKKKKKKKK.',
    '................',
    '................',
  ],

  // 5. Thảm dệt đỏ son viền kim nhũ kho lưu trữ (24×14)
  decor_crimson_runner_rug: [
    'GGGGGGGGGGGGGGGGGGGGGGGG',
    'KRRRRRRRRRRRRRRRRRRRRRRK',
    'KRGGGGGGGGGGGGGGGGGGGGRK',
    'KRGRRRRRRRRRRRRRRRRRRGRK',
    'KRGRRGGGGWWGGGGWWGGRRGRK',
    'KRGRRGVVRRRRRRRRVVGRRGRK',
    'KRGRRGVVRRRRRRRRVVGRRGRK',
    'KRGRRGGGGWWGGGGWWGGRRGRK',
    'KRGRRRRRRRRRRRRRRRRRRGRK',
    'KRGGGGGGGGGGGGGGGGGGGGRK',
    'KRRRRRRRRRRRRRRRRRRRRRRK',
    'GGGGGGGGGGGGGGGGGGGGGGGG',
    '........................',
    '........................',
  ],

  // 6. Cuộn dây thừng gai lớn trên cầu cảng (16×12)
  decor_dock_rope_coil: [
    '....KKKKKKKK....',
    '..KKBBBBBBBBKK..',
    '.KBBPPPPPPPPBBK.',
    'KBBPPKKKKKKPPBBK',
    'KBBPKBBBBBBKPBBK',
    'KBBPKBPPPPBKPBBK',
    'KBBPKBBBBBBKPBBK',
    '.KBBPPPPPPPPBBK.',
    '..KKBBBBBBBBKKB.',
    '...OOOOOOOOOOOBK',
    '.............OO.',
    '................',
  ],

  // 7. Cặp thùng gỗ sồi đai sắt bến cảng (20×18)
  decor_wood_barrels: [
    '...KKKKKK..KKKKKK...',
    '..KBBBBBBKKBBBBBBK..',
    '.KBMGGGGMBBMGGGGMBK.',
    '.KKKKKKKKKKKKKKKKKK.',
    '.KBMBBBBBMBMBBBBBMK.',
    '.KBMGGGGMBBMGGGGMBK.',
    '.KBMBBBBBMBMBBBBBMK.',
    '.KTTTTTTTTTTTTTTTTK.',
    '.KBMBBBBBMBMBBBBBMK.',
    '.KBMGGGGMBBMGGGGMBK.',
    '.KBBBBBBBBBBBBBBBBK.',
    '.KTTTTTTTTTTTTTTTTK.',
    '..KMMMMMMKKMMMMMMK..',
    '..KKKKKKKKKKKKKKKK..',
    '..OOOOOOOOOOOOOOOO..',
    '...OOOOOOOOOOOOOO...',
    '....................',
    '....................',
  ],

  // 8. Mỏ neo sắt đúc & xích neo tàu biển (18×20)
  decor_iron_anchor: [
    '.......KKKK.......',
    '......KACCAK......',
    '......KA..AK......',
    '......KAAAAK......',
    '....KKKKKKKKKK....',
    '...KCCCCCCCCCCK...',
    '....KKKKKKKKKK....',
    '.......KAAK.......',
    '.......KCAK.......',
    '.......KAAK.......',
    '..KK...KCAK...KK..',
    '.KCAK..KAAK..KACK.',
    '.KAAKK.KCAK.KKAAK.',
    '..KAAAKKAAKKAAAK..',
    '...KAAAAAAAAAAK...',
    '....KKAAAAAAKK....',
    '......KKKKKK......',
    '....OOOOOOOOOO....',
    '..................',
    '..................',
  ],

  // 9. Đôi thúng cá đan tre & đòn gánh bến thuyền (22×16)
  decor_fish_baskets: [
    '..GGGGGGGGGGGGGGGGGG..',
    '.KKKKKKKK....KKKKKKKK.',
    'KGGGGGGGGBKKBGGGGGGGGK',
    'KGBBBBBBBG..GBBBBBBBGK',
    'KGPWWWWWPG..GPWWWWWPGK',
    'KGCCCCCCCG..GCCCCCCCGK',
    'KGBGBGBGBK..KBGBGBGBGK',
    'KBBBBBBBBK..KBBBBBBBBK',
    '.KMMMMMMK....KMMMMMMK.',
    '..KKKKKK......KKKKKK..',
    '..OOOOOO......OOOOOO..',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
  ],

  // 10. Xe kéo tay hai bánh chở kiện trà Cửu Long (24×18)
  decor_dock_handcart: [
    '....KKKKKKKKKKKKKKKK....',
    '...KBBBBBBBBBBBBBBBBK...',
    '...KBPPPBBBBBBBBGGGBK...',
    '...KBPRPBBBBBBBBGGGBK...',
    '...KBBBBBBBBBBBBBBBBK...',
    '.KKKKKKKKKKKKKKKKKKKKKK.',
    'KMMMMMMMMMMMMMMMMMMMMGGK',
    'KKKKKKKKKKKKKKKKKKKKKKKK',
    '...KKK..KKKKKK..KKK.....',
    '..KAAAKK..KK..KKAAAK....',
    '.KAAKAAK......KAAKAAK...',
    '.KAAKAAK......KAAKAAK...',
    '..KAAAK........KAAAK....',
    '...KKK..........KKK.....',
    '..OOOOOOOOOOOOOOOOOO....',
    '........................',
    '........................',
    '........................',
  ],

  // 11. Bộ bàn trà đá Bà Năm, ấm sứ lam & đôn gỗ (26×18)
  decor_tea_stall_set: [
    '.........KK...............',
    '........KWWK...KK.........',
    '.......KUUUUUKKWWK........',
    '......KKKKKKKKKKKKK.......',
    '.....KGGGGGGGGGGGGGK......',
    '....KBBBBBBBBBBBBBBBK.....',
    '....KMMMMMMMMMMMMMMMK.....',
    '.KKKKK..KMMK..KMMK..KKKKK.',
    'KGGGGGK.KMMK..KMMK.KGGGGGK',
    'KBBBBBK.KMMK..KMMK.KBBBBBK',
    '.KMMMK..KKKK..KKKK..KMMMK.',
    '.KMMMK..............KMMMK.',
    '.KKKKK..............KKKKK.',
    '.OOOOOOOOOOOOOOOOOOOOOOOO.',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 12. Xe đạp cổ khung thép dựa tường phố (24×18)
  decor_vintage_bicycle: [
    '...............KKKK.....',
    '......KKKK....KGGGGK....',
    '.....KBBBBK....KKAK.....',
    '......KKKK....KAAK......',
    '.......KAK...KAAK.......',
    '...KKKKKAAKKKAAAKKK.....',
    '..KAAAAKAAAKAAKAAAAK....',
    '.KAAKKKAKAAKAKAKKKAAK...',
    'KAAK..KAAKAAKAAK..KAAK..',
    'KAAK..KAAKKKKAAK..KAAK..',
    'KAAK..KAAK..KAAK..KAAK..',
    '.KAAKKKAAK..KAAKKKAAK...',
    '..KAAAAAK....KAAAAAK....',
    '...KKKKK......KKKKK.....',
    '..OOOOOOOOOOOOOOOOOO....',
    '........................',
    '........................',
    '........................',
  ],

  // 13. Chậu gốm men lam trồng cây cảnh xanh rêu (16×20)
  decor_ceramic_planter: [
    '.....HHHH.......',
    '...HHEEEEHH.....',
    '..HEEEEEEEEH....',
    '.HEEEFFFFEEEH...',
    '.HEEFFGGFFEEH...',
    '..FFEEBBEEFF....',
    '....FFMMFF......',
    '......MM........',
    '...KKKKKKKK.....',
    '..KWWWWWWWWK....',
    '..KUUUUUUUUK....',
    '..KWWUUUUWWK....',
    '...KUUUUUUK.....',
    '...KWWWWWWK.....',
    '....KUUUUK......',
    '....KKKKKK......',
    '...OOOOOOOO.....',
    '................',
    '................',
    '................',
  ],

  // 14. Trụ biển hiệu gỗ sơn mài & đèn lồng đỏ (16×24)
  decor_street_signpost: [
    '....KKKKKKKK....',
    '...KGGGGGGGGK...',
    '...KBBBBBBBBK...',
    '...KKKKKKKKKK...',
    '...KMMK..KRRK...',
    '...KMMK.KRVVRK..',
    '...KMMK.KRGGRK..',
    '...KMMK.KRVVRK..',
    '...KMMK..KGGK...',
    '...KMMK...KK....',
    '...KMMK.........',
    '...KMMK.........',
    '...KMMK.........',
    '...KMMK.........',
    '...KMMK.........',
    '...KMMK.........',
    '..KBBBBK........',
    '.KTTTTTTK.......',
    '.KKKKKKKK.......',
    '.OOOOOOOO.......',
    '................',
    '................',
    '................',
    '................',
  ],

  // 15. Lồng tre chở hàng & đèn bão đồng bến nhỏ (22×18)
  decor_bamboo_cargo_cages: [
    '..............KKKK....',
    '...KKKKKKKK..KGGGGK...',
    '..KGGGGGGGGK.KGWWGK...',
    '..KGBGBGBGBK.KBBBBK...',
    '.KKKKKKKKKKKK.KKKK....',
    'KGGGGGGGGGGGGK........',
    'KGBGBGBGBGBGBKKKKKKKK.',
    'KBBBBBBBBBBBBKGGGGGGGK',
    'KGBGBGBGBGBGBKGBGBGBGK',
    'KMMMMMMMMMMMMKBBBBBBBK',
    'KKKKKKKKKKKKKKKKKKKKKK',
    '.OOOOOOOOOOOOOOOOOOOO.',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
  ],

  // 16. Giá gỗ đựng cuộn sắc lệnh & văn kiện đóng dấu son (22×22)
  decor_scroll_rack: [
    '.KKKKKKKKKKKKKKKKKKKK.',
    'KBBBBBBBBBBBBBBBBBBBBK',
    'KBMGGGGGGGGGGGGGGGGMBK',
    'KBMPPWPPWPPWPPWPPWPMBK',
    'KBMPPRPPVPPRPPVPPRPMBK',
    'KBMPPWPPWPPWPPWPPWPMBK',
    'KBBBBBBBBBBBBBBBBBBBBK',
    'KMMMMMMMMMMMMMMMMMMMMK',
    'KBMPPWPPWPPWPPWPPWPMBK',
    'KBMPPVPPRPPRPPVPPRPMBK',
    'KBMPPWPPWPPWPPWPPWPMBK',
    'KBBBBBBBBBBBBBBBBBBBBK',
    'KMMMMMMMMMMMMMMMMMMMMK',
    'KBMUUUUUUBBBBRRRRRRMBK',
    'KBMUUUUUUBBBBRRRRRRMBK',
    'KBBBBBBBBBBBBBBBBBBBBK',
    'KKKKKKKKKKKKKKKKKKKKKK',
    '.KMMK............KMMK.',
    '.KKKK............KKKK.',
    '.OOOOOOOOOOOOOOOOOOOO.',
    '......................',
    '......................',
  ],

  // 17. Bàn in thạch bản truyền đơn bí mật & chồng giấy dó (26×20)
  decor_printing_table: [
    '......KKKKKKKK............',
    '.....KWWWWWWWWK..KKKK.....',
    '.....KWRRRRRWWK.KGGGGK....',
    '..KKKKKKKKKKKKKKKKKKKKKK..',
    '.KBBBBBBBBBBBBBBBBBBBBBBK.',
    'KBBTTTTTTTTTTBBPPPPPPPPBBK',
    'KBBTAAAAAAAATBBPRRRRRRPBBK',
    'KBBTAAAAAAAATBBPPPPPPPPBBK',
    'KBBTTTTTTTTTTBBBBBBBBBBBBK',
    'KBBBBBBBBBBBBBBBBBBBBBBBBK',
    'KKKKKKKKKKKKKKKKKKKKKKKKKK',
    '..KMMK..............KMMK..',
    '..KMMK..............KMMK..',
    '..KMMK..............KMMK..',
    '..KKKK..............KKKK..',
    '.OOOOOOOOOOOOOOOOOOOOOOOO.',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 18. Tủ hồ sơ sơn mài khóa đồng & bình sứ cổ (22×24)
  decor_lacquer_cabinet: [
    '........KKKK..........',
    '.......KUUUUUK........',
    '.......KWWUUWK........',
    '........KUUUK.........',
    '.KKKKKKKKKKKKKKKKKKKK.',
    'KGGGGGGGGGGGGGGGGGGGGK',
    'KDDDDDDDDDDDDDDDDDDDDK',
    'KDRRRRRRRRDDRRRRRRRRDK',
    'KDRBBBBBBBDDRBBBBBBBDK',
    'KDRBBBBGGGDDRGGGBBBBDK',
    'KDRBBBBBBBDDRBBBBBBBDK',
    'KDDDDDDDDDDDDDDDDDDDDK',
    'KBBBBBBBBBBBBBBBBBBBBK',
    'KDRRRRRRRRDDRRRRRRRRDK',
    'KDRBBBBGGGDDRGGGBBBBDK',
    'KDRBBBBBBBDDRBBBBBBBDK',
    'KDDDDDDDDDDDDDDDDDDDDK',
    'KKKKKKKKKKKKKKKKKKKKKK',
    '.KMMK............KMMK.',
    '.KKKK............KKKK.',
    '.OOOOOOOOOOOOOOOOOOOO.',
    '......................',
    '......................',
    '......................',
  ],

  // 19. Lò than đồng sưởi ấm & đôn gỗ đọc sách (20×16)
  decor_brazier_stool: [
    '....KKKKKKKK........',
    '..KKGGGGGGGGKK......',
    '.KGGRVVWWVVRGGK.....',
    '.KGGRRRRRRRRGGK.....',
    '.KBBBBBBBBBBBBK.KKKK',
    '..KMMMMMMMMMMK.KBBBK',
    '...KGGK..KGGK..KMMMK',
    '...KKKK..KKKK..KMMK.',
    '..OOOOOOOOOOOO.KKKK.',
    '...............OOOO.',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
  ],

  // 20. Lu sành hứng nước mưa đêm hẻm nhỏ (22×20)
  decor_alley_rain_vats: [
    '...KKKKKKKK...........',
    '..KBBBBBBBBK..KKKKKK..',
    '.KBMGGGGGGMBKKBBBBBBK.',
    '.KBBBBBBBBBBKUUUUUUUUK',
    'KMMMMMMMMMMMMKCCCCCCUK',
    'KMBBBBBBBBBBMMUUUUUUUK',
    'KMMMMMMMMMMMMKUUUUUUUK',
    'KMBBBBBBBBBBMMNNNNNNUK',
    '.KMMMMMMMMMMKUUUUUUUK.',
    '.KLLLLLLLLLLK.KNNNNK..',
    '..KKKKKKKKKK...KKKK...',
    '..OOOOOOOOOOOOOOOOOO..',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
  ],

  // 21. Bia đá rêu phong & lư hương đồng sân lưu trữ (20×24)
  decor_stone_incense_stele: [
    '.....KKKKKKKK.......',
    '....KAAAAAAAAK......',
    '...KAACCCCCAAAK.....',
    '...KAACGGGGCAK......',
    '...KAACCCCCAAK......',
    '...KAAAAAAAAAK......',
    '...KFFAAAAAFFK......',
    '...KEEAAAAFEEK..KK..',
    '...KFFAAAAAFFK.KGGK.',
    '..KTTTTTTTTTTKKBBBBK',
    '.KAAAAAAAAAAAAKGGGGK',
    '.KFFFFFFFFFFFFKKKKKK',
    '.KKKKKKKKKKKKKKKKKK.',
    '.OOOOOOOOOOOOOOOOOO.',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
    '....................',
  ],

  // 22. Chậu bonsai cổ thụ đặt trên bệ đá chạm (18×22)
  decor_bonsai_pedestal: [
    '....HHHHHH........',
    '..HHEEEEEEHH......',
    '.HEEEEFFEEEEH.....',
    '.HEEFF..FFEEHH....',
    '..FFF....HEEEEH...',
    '....MM...EEFFEF...',
    '....MM..MM........',
    '....MMMMM.........',
    '..KKKKKKKKKK......',
    '.KRRRRRRRRRRK.....',
    '.KRGGGGGGGGRK.....',
    '..KRRRRRRRRK......',
    '..KTTTTTTTTK......',
    '..KAAAAAAAAK......',
    '.KTTTTTTTTTTK.....',
    '.KKKKKKKKKKKK.....',
    '.OOOOOOOOOOOO.....',
    '..................',
    '..................',
    '..................',
    '..................',
    '..................',
  ],

  // 23. Khóm tre xanh Việt Bắc & tảng đá rêu Tân Trào (22×26)
  decor_bamboo_grove: [
    '....HH....HH..........',
    '..HEEEH..HEEEH..HH....',
    '.HEEEEF..FEEEEEHEEH...',
    '..FFGG....GGFFEEEEEH..',
    '....GH....HG..FFGG....',
    '....EH....HE....GH....',
    '..HHEH....HEHH..EH....',
    '.HEEEH....HEEEH.EH....',
    '..FFEH....HEFF..EH....',
    '....GH....HG....GH....',
    '....EH....HE....EH....',
    '....EH....HE....EH....',
    '....GH....HG....GH....',
    '....EH....HE..TTTTTT..',
    '....EH....HE.TAAAAAAT.',
    '...FEH...FHE.TAAFFAAT.',
    '..FFEEFFFFEEFTFFFFFFT.',
    '..FFFFFFFFFFFFKKKKKK..',
    '..OOOOOOOOOOOOOOOOOO..',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
    '......................',
  ],

  // 24. Chảo lửa trại kháng chiến Tân Trào rực hồng (16×18)
  decor_camp_brazier: [
    '.......WW.......',
    '......WGGW......',
    '.....WGVVGW.....',
    '....WVVRRVVW....',
    '...KVVRRRRVVK...',
    '..KTTTTTTTTTTK..',
    '..KAAAAAAAAAAK..',
    '...KTTTTTTTTK...',
    '....KKTTTTKK....',
    '....KAK..KAK....',
    '...KAAK..KAAK...',
    '...KKKK..KKKK...',
    '..OOOOOOOOOOOO..',
    '................',
    '................',
    '................',
    '................',
    '................',
  ],

  // 25. Hàng trụ đồng giăng dây nhung đỏ lễ đài Ba Đình (24×18)
  decor_flag_stanchion: [
    '..KKKK............KKKK..',
    '.KGGGGK..........KGGGGK.',
    '..KGGK............KGGK..',
    '..KBBKRRRRRRRRRRRRKBBK..',
    '..KBBK.RRRRRRRRRR.KBBK..',
    '..KBBK...RRRRRR...KBBK..',
    '..KBBK............KBBK..',
    '..KBBK...HHHHHH...KBBK..',
    '..KBBK..HEEEEEEH..KBBK..',
    '.KGGGGK.KRRRRRRK.KGGGGK.',
    'KBBBBBBKKGGGGGGKKBBBBBBK',
    'KKKKKKKKKKKKKKKKKKKKKKKK',
    '.OOOOOOOOOOOOOOOOOOOOOO.',
    '........................',
    '........................',
    '........................',
    '........................',
    '........................',
  ],

  // 26. Cột loa phóng thanh đồng ngày Tết Độc Lập 02/09/1945 (16×26)
  decor_loudspeaker_pole: [
    '......KKKK......',
    '..KKKKGGGGKKKK..',
    '.KGGGGBBBBGGGGK.',
    '.KGWGGGGGGGGWGK.',
    '..KKKKGGGGKKKK..',
    '......KAAK......',
    '......KAAKRRRRR.',
    '......KAAKRVGVR.',
    '......KAAKRRRRR.',
    '......KAAK......',
    '......KAAK......',
    '......KAAK......',
    '......KAAK......',
    '......KAAK......',
    '......KAAK......',
    '......KAAK......',
    '.....KTTTTK.....',
    '....KAAAAAAK....',
    '....KKKKKKKK....',
    '....OOOOOOOO....',
    '................',
    '................',
    '................',
    '................',
    '................',
    '................',
  ],

  // 27. Xe đạp thồ Điện Biên Phủ gia cố cọc tre & bao gạo (26×20)
  decor_pack_bicycle: [
    '..GGGGGGGGGGGGGGGGGG......',
    '....KKKKKKKKKKKK..........',
    '...KPPPPPPPPPPPPK..GGGG...',
    '...KPPPPBBBBPPPPK.KGGGGK..',
    '..KKKKKKKKKKKKKKKK..KK....',
    '.KBBBBBBBBBBBBBBBBKKAK....',
    '..KKKKKKKKKKKKKKKKKAAK....',
    '...KKKKK..KAAK..KKAAAK....',
    '..KAAAAAK.KAAK.KAAAAAK....',
    '.KAAKKKAAKKAAKKAAKKKAAK...',
    '.KAAK.KAAK.KK.KAAK.KAAK...',
    '.KAAKKKAAK....KAAKKKAAK...',
    '..KAAAAAK......KAAAAAK....',
    '...KKKKK........KKKKK.....',
    '..OOOOOOOOOOOOOOOOOOOO....',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 28. Công sự bao cát & hòm đạn ngụy trang lá rừng (26×16)
  decor_sandbag_emplacement: [
    '....HHHH....KKKKKKKK......',
    '..HHEEEEHH.KEEEEEEEEK.....',
    '.KKKKKKKKKKKEGGGGGGEEK....',
    'KPPPPPPKKKKKEEEEEEEEK.....',
    'KBBBBBBKPPPPPPKKKKKKKK....',
    '.KKKKKKKKBBBBBKPPPPPPPK...',
    'KPPPPPPPKPPPPPKBBBBBBBK...',
    'KBBBBBBBKBBBBBKPPPPPPPK...',
    'KKKKKKKKKKKKKKKKBBBBBBK...',
    '.OOOOOOOOOOOOOOOOOOOOOO...',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 29. Cột cây số quốc lộ sơn đỏ trắng Mùa Xuân 1975 (16×22)
  decor_highway_milestone: [
    '.....KKKKKK.....',
    '....KRRRRRRK....',
    '...KRRRRRRRRK...',
    '...KRRRRRRRRK...',
    '...KKKKKKKKKK...',
    '...KWWWWWWWWK...',
    '...KWKKKKKKWK...',
    '...KWWWWWWWWK...',
    '...KWKKKKKKWK...',
    '...KWWWWWWWWK...',
    '...KPPPPPPPPK...',
    '..FKKKKKKKKKKF..',
    '.FFEEAAAAAAAAEEF',
    '.FFFFFFFFFFFFFF.',
    '..OOOOOOOOOOOO..',
    '................',
    '................',
    '................',
    '................',
    '................',
    '................',
    '................',
  ],

  // 30. Bàn thông tin tác chiến & đài vô tuyến dã chiến 1975 (26×20)
  decor_field_radio_desk: [
    '.................G........',
    '.................G........',
    '....KKKK.......KKKKK......',
    '...KGGGGK.....KAAAAAK.....',
    '...KGWWGK.....KAGGVAK.....',
    '.KKKKKKKKKKKKKKKKKKKKKKK..',
    'KEEEEEEEEEEEEEEEEEEEEEEK..',
    'KEEWWWWWWWWEEAAAAAAAAEEK..',
    'KEEWRRRRRWWEEGGGGGGGGEEK..',
    'KEEEEEEEEEEEEEEEEEEEEEEK..',
    'KKKKKKKKKKKKKKKKKKKKKKKK..',
    '..KMMK..............KMMK..',
    '..KMMK..............KMMK..',
    '..KKKK..............KKKK..',
    '.OOOOOOOOOOOOOOOOOOOOOOOO.',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 31. Sạp báo & tạp chí phố thương mại Đổi Mới 1986 (26×22)
  decor_newsstand_1986: [
    '.KKKKKKKKKKKKKKKKKKKKKKKK.',
    'KRRRRWWWRRRRWWWRRRRWWWRRRK',
    'KRRRRWWWRRRRWWWRRRRWWWRRRK',
    'KGGGGGGGGGGGGGGGGGGGGGGGGK',
    '.KMMK................KMMK.',
    '.KMMK.WWWW.GGGG.UUUU.KMMK.',
    '.KMMK.WRRW.GBBG.UWWU.KMMK.',
    '.KMMK.WWWW.GGGG.UUUU.KMMK.',
    '.KKKKKKKKKKKKKKKKKKKKKKKK.',
    '.KBBBBBBBBBBBBBBBBBBBBBBK.',
    '.KBGGGGGGGGGGGGGGGGGGGGBK.',
    '.KBBBBBBBBBBBBBBBBBBBBBBK.',
    '.KMMMMMMMMMMMMMMMMMMMMMMK.',
    '.KKKKKKKKKKKKKKKKKKKKKKKK.',
    '.OOOOOOOOOOOOOOOOOOOOOOOO.',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 32. Xe máy Cub cổ điển thập niên Đổi Mới dựng chân chống (24×18)
  decor_classic_scooter: [
    '..............KKKK......',
    '.............KGGGGK.....',
    '....KKKKKK....KUUUK.....',
    '...KBBBBBBK...KUUUK.....',
    '...KKKKKKKK..KUUUUUK....',
    '..KUUUUUUUUUKUUUUUUK....',
    '.KUUUUUWWWWUUUUUUUUK....',
    '.KUUUUWWWWWWUUUUUUK.....',
    '..KKKKKUUUUUUKKKKK......',
    '.KAAAAAKKKKKKAAAAAK.....',
    'KAAKKKAAK..KAAKKKAAK....',
    'KAAK.KAAK..KAAK.KAAK....',
    '.KAAAAAK....KAAAAAK.....',
    '..KKKKK......KKKKK......',
    '.OOOOOOOOOOOOOOOOOO.....',
    '........................',
    '........................',
    '........................',
  ],

  // 33. Ghế nghỉ năng lượng mặt trời & bồn cây Kỷ Nguyên Số 2045 (26×18)
  decor_solar_bench_2045: [
    '...................HHHH...',
    '.KKKKKKKKKKKKKKKK.HEEEEH..',
    'KGGGGGGGGGGGGGGGGKHEEFEH..',
    'KNNUUUUUUUUUUUUNNK.FFFF...',
    'KNNUCCCCCCCCCCUNNKKKKKKKKK',
    'KGGGGGGGGGGGGGGGGKGGGGGGGK',
    '.KKKKKKKKKKKKKKKKKNNNNNNNK',
    '..KGGK......KGGK.KNNUGUNNK',
    '..KNNK......KNNK.KKKKKKKKK',
    '..KKKK......KKKK..OOOOOOO.',
    '.OOOOOOOOOOOOOOOO.........',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
    '..........................',
  ],

  // 34. Bia ký ức quang học hoa sen vàng đồng 2045 (18×24)
  decor_holo_lotus_stele: [
    '........WW........',
    '.......WGGW.......',
    '......WGVVGW......',
    '.......WGGW.......',
    '........WW........',
    '.....KKKKKKKK.....',
    '....KGGGGGGGGK....',
    '...KGCCCCCCCCGK...',
    '...KGCWWWWWWCGK...',
    '...KGCCCCCCCCGK...',
    '...KGGGGGGGGGGK...',
    '..KNNNNNNNNNNNNK..',
    '..KNGGGGGGGGGGNK..',
    '.KNNNUUUUUUUUNNNK.',
    '.KGGGGGGGGGGGGGGK.',
    '.KKKKKKKKKKKKKKKK.',
    '.OOOOOOOOOOOOOOOO.',
    '..................',
    '..................',
    '..................',
    '..................',
    '..................',
    '..................',
    '..................',
  ],

  // 35. Tán đa cổ thụ nhiều tầng hữu cơ có rễ phụ rủ (32×22)
  decor_banyan_canopy_crown: [
    '........HHHHHHHHHHHHHHHH........',
    '.....HHHEEEEEEEEEEEEEEEEHHH.....',
    '...HHEEEEEEEHHHHHHHHHEEEEEEHH...',
    '..HEEEEEEHHHEEEEEEEEHHHEEEEEEH..',
    '.HEEEEEHHEEEEEEEEEEEEEEHHEEEEEH.',
    'HEEEEEHEEEEEEEFFFFEEEEEEEHEEEEEH',
    'HEEEEHEEEEFFFFFFFFFFFFEEEEHEEEEH',
    'FEEEEHEEEFFFFGGGGGGFFFFEEEHEEEEF',
    'FFEEEFFFFEEEEEEEEEEEEEEFFFFEEEFF',
    '.FFEEFFFEEEEHHHHHHHHHEEEEFFFEEFF',
    '.FFFFFEEEEHHEEEEEEEEHHEEEEFFFFF.',
    '..FFFFEEEEEEFFFFFFFFEEEEEEFFFF..',
    '...FFFFFEEEEEEEEEEEEEEEEFFFFF...',
    '.....FFFFFFFEEEEEEEEFFFFFFF.....',
    '.......FFFFFFFFFFFFFFFFFF.......',
    '.........MM..........MM.........',
    '.........M............M.........',
    '................................',
    '................................',
    '................................',
    '................................',
    '................................',
  ],

  // 36. Dây đèn lồng đỏ vàng giăng ngang phố cổ / sân hội (32×12)
  decor_festive_lantern_string: [
    'KKK..........................KKK',
    '...KKKK..................KKKK...',
    '......KRRRRK........KRRRRK......',
    '.....KRVGGVRKKKKKKKKRVGGVRK.....',
    '.....KRVWWVRK......KRVWWVRK.....',
    '......KGGGGK..KKKK..KGGGGK......',
    '.......KGGK..KRRRRK..KGGK.......',
    '.............KRVVRK.............',
    '.............KRGGRK.............',
    '..............KGGK..............',
    '................................',
    '................................',
  ],
};

// Bộ nhớ đệm Offscreen Canvas cho từng (spriteId + flipX) -> O(1) drawImage mỗi khung hình
const cachedDecorCanvasMap = new Map<string, HTMLCanvasElement | null>();

function getCachedDecorCanvas(
  spriteId: DecorSpriteId,
  flipX: boolean
): HTMLCanvasElement | null {
  if (typeof document === 'undefined') return null;
  if (
    typeof navigator !== 'undefined' &&
    typeof navigator.userAgent === 'string' &&
    navigator.userAgent.toLowerCase().includes('jsdom')
  ) {
    return null;
  }
  const key = `${spriteId}:${flipX ? '1' : '0'}`;
  if (cachedDecorCanvasMap.has(key)) {
    return cachedDecorCanvasMap.get(key) ?? null;
  }

  const matrix = DECOR_SPRITE_MATRICES[spriteId];
  if (!matrix || matrix.length === 0) return null;

  const height = matrix.length;
  const width = matrix[0]?.length ?? 16;
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = typeof canvas.getContext === 'function' ? canvas.getContext('2d') : null;
  if (!ctx) {
    cachedDecorCanvasMap.set(key, null);
    return null;
  }

  ctx.imageSmoothingEnabled = false;
  drawMatrixSprite(ctx, matrix, 0, 0, 1, flipX);
  cachedDecorCanvasMap.set(key, canvas);
  return canvas;
}

/**
 * Vẽ một tĩnh vật trang trí (Decor Sprite) với tâm neo ngang `screenX` và chân đế `screenY`.
 * Tự động dùng bộ đệm Offscreen Canvas trên trình duyệt và lui về `drawMatrixSprite` trong môi trường kiểm thử.
 */
export function drawDecorSprite(
  ctx: CanvasRenderingContext2D,
  spriteId: DecorSpriteId,
  screenX: number,
  screenY: number,
  flipX = false
): void {
  const matrix = DECOR_SPRITE_MATRICES[spriteId];
  if (!matrix || matrix.length === 0) return;

  const h = matrix.length;
  const w = matrix[0]?.length ?? 16;
  const drawX = Math.round(screenX - w * 0.5);
  const drawY = Math.round(screenY - h + 2);

  const cached = getCachedDecorCanvas(spriteId, flipX);
  if (cached && typeof ctx.drawImage === 'function') {
    try {
      ctx.drawImage(cached, drawX, drawY);
      return;
    } catch {
      // Fallback dưới môi trường canvas giả lập
    }
  }
  drawMatrixSprite(ctx, matrix, drawX, drawY, 1, flipX);
}

/**
 * Vẽ vật thể tương tác `kind === 'prop'` từng bị ẩn trên bản đồ (`prop_harbor_notice`, `prop_cargo_manifest_crate`).
 */
export function drawPropInteractableSprite(
  ctx: CanvasRenderingContext2D,
  propId: string,
  screenX: number,
  screenY: number
): void {
  const noticeMatrix = [
    '..KKKKKKKKKKKKKKKKKKKK..',
    '.KRRRRRRRRRRRRRRRRRRRRK.',
    'KGGGGGGGGGGGGGGGGGGGGGGK',
    '.KBBBBBBBBBBBBBBBBBBBBK.',
    '.KBMWWWWWWWBPPPPPPPPBMK.',
    '.KBMWRRRRRWBPRRRRRRPBMK.',
    '.KBMWWWWWWWBPPPPPPPPBMK.',
    '.KBMWWWWWWWBPPPPVVPPBMK.',
    '.KBBBBBBBBBBBBBBBBBBBBK.',
    '.KKKKKKKKKKKKKKKKKKKKKK.',
    '...KMMK..........KMMK...',
    '...KMMK..........KMMK...',
    '...KMMK..........KMMK...',
    '..KTTTTK........KTTTTK..',
    '..KKKKKK........KKKKKK..',
    '..OOOOOOOOOOOOOOOOOOOO..',
  ];

  const manifestCrateMatrix = [
    '.............KKKK.......',
    '............KGGGGK......',
    '....KKKKKKKKKGWWGK......',
    '...KWWWWWWWWKKKKKK......',
    '..KKKKKKKKKKKKKKKKKKKK..',
    '.KBBBBBBBBBBBBBBBBBBBBK.',
    '.KBMGGGGGGGGGGGGGGGGMBK.',
    '.KBMBBBBBBBBBBBBBBBBMBK.',
    '.KBMGGGGPPPPPGGGGGGGMBK.',
    '.KBMBBBBPRRRPBBBBBBBMBK.',
    '.KBMBBBBPPPPPBBBBBBBMBK.',
    '.KMMMMMMMMMMMMMMMMMMMMK.',
    '.KKKKKKKKKKKKKKKKKKKKKK.',
    '.OOOOOOOOOOOOOOOOOOOOOO.',
  ];

  const matrix = propId.includes('notice') ? noticeMatrix : manifestCrateMatrix;
  const w = matrix[0]?.length ?? 24;
  const h = matrix.length;
  drawMatrixSprite(
    ctx,
    matrix,
    Math.round(screenX - w * 0.5),
    Math.round(screenY - h + 4),
    1,
    false
  );
}

// ============================================================================
// PHẦN 3: BỐ CỤC 112 TĨNH VẬT DECOR THỦ CÔNG TRÊN TOÀN BỘ 14 BẢN ĐỒ (CHƯƠNG I–IV)
// Mọi tọa độ đều được đối chiếu ngoài vùng Spawn, Portal và hành lang A* chính.
// ============================================================================

export const ZONE_DECOR_PROPS: Record<string, ZoneDecorPlacement[]> = {
  // 1. Bến Cảng Cửu Long (1024×768)
  harbor_square: [
    { id: 'hd_puddle_1', spriteId: 'decor_rain_puddle_lantern', x: 212, y: 416, layer: 'ground' },
    { id: 'hd_puddle_2', spriteId: 'decor_rain_puddle_lantern', x: 640, y: 428, layer: 'ground', flipX: true },
    { id: 'hd_rope_1', spriteId: 'decor_dock_rope_coil', x: 184, y: 638, layer: 'ground' },
    { id: 'hd_rope_2', spriteId: 'decor_dock_rope_coil', x: 616, y: 642, layer: 'ground', flipX: true },
    { id: 'hd_grate_1', spriteId: 'decor_cobble_drain_grate', x: 416, y: 244, layer: 'ground' },
    { id: 'hd_barrels_west', spriteId: 'decor_wood_barrels', x: 104, y: 174, layer: 'ysort' },
    { id: 'hd_handcart_customs', spriteId: 'decor_dock_handcart', x: 304, y: 174, layer: 'ysort' },
    { id: 'hd_signpost_plaza', spriteId: 'decor_street_signpost', x: 564, y: 174, layer: 'ysort' },
    { id: 'hd_barrels_mid', spriteId: 'decor_wood_barrels', x: 604, y: 396, layer: 'ysort', flipX: true },
    { id: 'hd_anchor_quay', spriteId: 'decor_iron_anchor', x: 432, y: 650, layer: 'ysort' },
    { id: 'hd_baskets_pier', spriteId: 'decor_fish_baskets', x: 760, y: 646, layer: 'ysort' },
    { id: 'hd_bicycle_east', spriteId: 'decor_vintage_bicycle', x: 792, y: 174, layer: 'ysort', flipX: true },
    { id: 'hd_canopy_west', spriteId: 'decor_banyan_canopy_crown', x: 344, y: 298, layer: 'canopy' },
    { id: 'hd_canopy_east', spriteId: 'decor_banyan_canopy_crown', x: 664, y: 298, layer: 'canopy', flipX: true },
  ],

  // 2. Kho Hàng Số 4 Cửu Long (576×448)
  warehouse_interior: [
    { id: 'wd_rug_center', spriteId: 'decor_crimson_runner_rug', x: 272, y: 236, layer: 'ground' },
    { id: 'wd_mat_south', spriteId: 'decor_reed_mat', x: 272, y: 364, layer: 'ground' },
    { id: 'wd_barrels_nw', spriteId: 'decor_wood_barrels', x: 64, y: 112, layer: 'ysort' },
    { id: 'wd_cages_ne', spriteId: 'decor_bamboo_cargo_cages', x: 504, y: 116, layer: 'ysort', flipX: true },
    { id: 'wd_scrolls_west', spriteId: 'decor_scroll_rack', x: 60, y: 288, layer: 'ysort' },
    { id: 'wd_handcart_se', spriteId: 'decor_dock_handcart', x: 496, y: 356, layer: 'ysort', flipX: true },
    { id: 'wd_cabinet_north', spriteId: 'decor_lacquer_cabinet', x: 272, y: 76, layer: 'ysort' },
  ],

  // 3. Phố Cổ Cửu Long (768×640)
  old_street: [
    { id: 'osd_puddle_1', spriteId: 'decor_rain_puddle_lantern', x: 268, y: 264, layer: 'ground' },
    { id: 'osd_grate_1', spriteId: 'decor_cobble_drain_grate', x: 488, y: 356, layer: 'ground' },
    { id: 'osd_leaves_1', spriteId: 'decor_fallen_banyan_leaves', x: 608, y: 264, layer: 'ground' },
    { id: 'osd_tea_stall', spriteId: 'decor_tea_stall_set', x: 260, y: 428, layer: 'ysort' },
    { id: 'osd_planter_west', spriteId: 'decor_ceramic_planter', x: 148, y: 172, layer: 'ysort' },
    { id: 'osd_bicycle_shop', spriteId: 'decor_vintage_bicycle', x: 444, y: 172, layer: 'ysort' },
    { id: 'osd_signpost_east', spriteId: 'decor_street_signpost', x: 664, y: 172, layer: 'ysort', flipX: true },
    { id: 'osd_vats_south', spriteId: 'decor_alley_rain_vats', x: 416, y: 454, layer: 'ysort' },
    { id: 'osd_lanterns_1', spriteId: 'decor_festive_lantern_string', x: 240, y: 178, layer: 'canopy' },
    { id: 'osd_lanterns_2', spriteId: 'decor_festive_lantern_string', x: 560, y: 178, layer: 'canopy', flipX: true },
  ],

  // 4. Bến Thuyền Nhỏ (640×480)
  small_pier: [
    { id: 'spd_rope_1', spriteId: 'decor_dock_rope_coil', x: 208, y: 356, layer: 'ground' },
    { id: 'spd_puddle_1', spriteId: 'decor_rain_puddle_lantern', x: 184, y: 268, layer: 'ground' },
    { id: 'spd_baskets_1', spriteId: 'decor_fish_baskets', x: 296, y: 364, layer: 'ysort' },
    { id: 'spd_anchor_1', spriteId: 'decor_iron_anchor', x: 92, y: 356, layer: 'ysort' },
    { id: 'spd_cages_1', spriteId: 'decor_bamboo_cargo_cages', x: 304, y: 152, layer: 'ysort' },
    { id: 'spd_barrels_east', spriteId: 'decor_wood_barrels', x: 548, y: 348, layer: 'ysort', flipX: true },
  ],

  // 5. Nhà An Toàn Bí Mật (512×384)
  safehouse_interior: [
    { id: 'shd_rug_center', spriteId: 'decor_crimson_runner_rug', x: 240, y: 212, layer: 'ground' },
    { id: 'shd_mat_bed', spriteId: 'decor_reed_mat', x: 404, y: 268, layer: 'ground' },
    { id: 'shd_printing_press', spriteId: 'decor_printing_table', x: 96, y: 124, layer: 'ysort' },
    { id: 'shd_cabinet_ne', spriteId: 'decor_lacquer_cabinet', x: 416, y: 92, layer: 'ysort', flipX: true },
    { id: 'shd_scrolls_sw', spriteId: 'decor_scroll_rack', x: 76, y: 296, layer: 'ysort' },
    { id: 'shd_brazier_se', spriteId: 'decor_brazier_stool', x: 412, y: 312, layer: 'ysort' },
  ],

  // 6. Hẻm Đêm Cửu Long (704×512)
  shadow_alley: [
    { id: 'sad_puddle_west', spriteId: 'decor_rain_puddle_lantern', x: 196, y: 296, layer: 'ground' },
    { id: 'sad_grate_mid', spriteId: 'decor_cobble_drain_grate', x: 352, y: 288, layer: 'ground' },
    { id: 'sad_puddle_east', spriteId: 'decor_rain_puddle_lantern', x: 524, y: 312, layer: 'ground', flipX: true },
    { id: 'sad_vats_nw', spriteId: 'decor_alley_rain_vats', x: 96, y: 126, layer: 'ysort' },
    { id: 'sad_barrels_mid', spriteId: 'decor_wood_barrels', x: 336, y: 126, layer: 'ysort' },
    { id: 'sad_cages_sw', spriteId: 'decor_bamboo_cargo_cages', x: 152, y: 448, layer: 'ysort' },
    { id: 'sad_handcart_se', spriteId: 'decor_dock_handcart', x: 556, y: 448, layer: 'ysort', flipX: true },
  ],

  // 7. Sân Lưu Trữ Cổ (704×544)
  archive_courtyard: [
    { id: 'acd_leaves_w', spriteId: 'decor_fallen_banyan_leaves', x: 224, y: 268, layer: 'ground' },
    { id: 'acd_leaves_e', spriteId: 'decor_fallen_banyan_leaves', x: 472, y: 276, layer: 'ground', flipX: true },
    { id: 'acd_rug_gate', spriteId: 'decor_crimson_runner_rug', x: 352, y: 204, layer: 'ground' },
    { id: 'acd_bonsai_w', spriteId: 'decor_bonsai_pedestal', x: 184, y: 164, layer: 'ysort' },
    { id: 'acd_bonsai_e', spriteId: 'decor_bonsai_pedestal', x: 520, y: 164, layer: 'ysort', flipX: true },
    { id: 'acd_stele_sw', spriteId: 'decor_stone_incense_stele', x: 148, y: 432, layer: 'ysort' },
    { id: 'acd_stele_se', spriteId: 'decor_stone_incense_stele', x: 556, y: 432, layer: 'ysort', flipX: true },
    { id: 'acd_canopy_w', spriteId: 'decor_banyan_canopy_crown', x: 200, y: 312, layer: 'canopy' },
    { id: 'acd_canopy_e', spriteId: 'decor_banyan_canopy_crown', x: 504, y: 312, layer: 'canopy', flipX: true },
    { id: 'acd_lanterns_top', spriteId: 'decor_festive_lantern_string', x: 352, y: 156, layer: 'canopy' },
  ],

  // 8. Kho Mật Hội Nghị Hợp Nhất 02/1930 (512×384)
  secret_archive: [
    { id: 'scad_rug_main', spriteId: 'decor_crimson_runner_rug', x: 240, y: 208, layer: 'ground' },
    { id: 'scad_mat_south', spriteId: 'decor_reed_mat', x: 240, y: 328, layer: 'ground' },
    { id: 'scad_scrolls_nw', spriteId: 'decor_scroll_rack', x: 68, y: 128, layer: 'ysort' },
    { id: 'scad_cabinet_ne', spriteId: 'decor_lacquer_cabinet', x: 436, y: 112, layer: 'ysort', flipX: true },
    { id: 'scad_scrolls_sw', spriteId: 'decor_scroll_rack', x: 68, y: 288, layer: 'ysort' },
    { id: 'scad_brazier_se', spriteId: 'decor_brazier_stool', x: 428, y: 296, layer: 'ysort' },
    { id: 'scad_lantern_hall', spriteId: 'decor_festive_lantern_string', x: 256, y: 92, layer: 'canopy' },
  ],

  // 9. Cây Đa & Mái Đình Tân Trào 08/1945 (768×576)
  tan_trao_banyan: [
    { id: 'ttd_mat_dinh_1', spriteId: 'decor_reed_mat', x: 320, y: 192, layer: 'ground' },
    { id: 'ttd_mat_dinh_2', spriteId: 'decor_reed_mat', x: 448, y: 192, layer: 'ground', flipX: true },
    { id: 'ttd_leaves_banyan', spriteId: 'decor_fallen_banyan_leaves', x: 184, y: 256, layer: 'ground' },
    { id: 'ttd_bamboo_nw', spriteId: 'decor_bamboo_grove', x: 88, y: 156, layer: 'ysort' },
    { id: 'ttd_bamboo_ne', spriteId: 'decor_bamboo_grove', x: 668, y: 156, layer: 'ysort', flipX: true },
    { id: 'ttd_brazier_dinh', spriteId: 'decor_camp_brazier', x: 580, y: 180, layer: 'ysort' },
    { id: 'ttd_stele_south', spriteId: 'decor_stone_incense_stele', x: 288, y: 468, layer: 'ysort' },
    { id: 'ttd_bamboo_sw', spriteId: 'decor_bamboo_grove', x: 112, y: 472, layer: 'ysort' },
    { id: 'ttd_canopy_banyan', spriteId: 'decor_banyan_canopy_crown', x: 168, y: 186, layer: 'canopy' },
  ],

  // 10. Quảng Trường Ba Đình 02/09/1945 (832×576)
  ba_dinh_square: [
    { id: 'bdd_rug_tribune', spriteId: 'decor_crimson_runner_rug', x: 416, y: 188, layer: 'ground' },
    { id: 'bdd_leaves_plaza', spriteId: 'decor_fallen_banyan_leaves', x: 248, y: 384, layer: 'ground' },
    { id: 'bdd_stanchion_w', spriteId: 'decor_flag_stanchion', x: 248, y: 168, layer: 'ysort' },
    { id: 'bdd_stanchion_e', spriteId: 'decor_flag_stanchion', x: 584, y: 168, layer: 'ysort', flipX: true },
    { id: 'bdd_speaker_w', spriteId: 'decor_loudspeaker_pole', x: 184, y: 168, layer: 'ysort' },
    { id: 'bdd_speaker_e', spriteId: 'decor_loudspeaker_pole', x: 648, y: 168, layer: 'ysort', flipX: true },
    { id: 'bdd_planter_sw', spriteId: 'decor_ceramic_planter', x: 196, y: 468, layer: 'ysort' },
    { id: 'bdd_planter_se', spriteId: 'decor_ceramic_planter', x: 636, y: 468, layer: 'ysort', flipX: true },
  ],

  // 11. Đèo Pha Đin — Tuyến Lửa Điện Biên 1954 (832×576)
  pha_din_pass: [
    { id: 'pdd_leaves_1', spriteId: 'decor_fallen_banyan_leaves', x: 312, y: 336, layer: 'ground' },
    { id: 'pdd_pack_bike_1', spriteId: 'decor_pack_bicycle', x: 140, y: 172, layer: 'ysort' },
    { id: 'pdd_pack_bike_2', spriteId: 'decor_pack_bicycle', x: 636, y: 172, layer: 'ysort', flipX: true },
    { id: 'pdd_sandbag_w', spriteId: 'decor_sandbag_emplacement', x: 212, y: 456, layer: 'ysort' },
    { id: 'pdd_sandbag_e', spriteId: 'decor_sandbag_emplacement', x: 584, y: 456, layer: 'ysort', flipX: true },
    { id: 'pdd_brazier_camp', spriteId: 'decor_camp_brazier', x: 400, y: 460, layer: 'ysort' },
    { id: 'pdd_bamboo_edge', spriteId: 'decor_bamboo_grove', x: 736, y: 164, layer: 'ysort', flipX: true },
  ],

  // 12. Hành Lang Mùa Xuân 1975 (768×544)
  spring_1975_corridor: [
    { id: 's75_leaves_1', spriteId: 'decor_fallen_banyan_leaves', x: 288, y: 328, layer: 'ground' },
    { id: 's75_radio_desk', spriteId: 'decor_field_radio_desk', x: 176, y: 164, layer: 'ysort' },
    { id: 's75_milestone_km', spriteId: 'decor_highway_milestone', x: 156, y: 428, layer: 'ysort' },
    { id: 's75_sandbag_hq', spriteId: 'decor_sandbag_emplacement', x: 588, y: 164, layer: 'ysort', flipX: true },
    { id: 's75_pack_supply', spriteId: 'decor_sandbag_emplacement', x: 496, y: 444, layer: 'ysort' },
    { id: 's75_bamboo_se', spriteId: 'decor_bamboo_grove', x: 668, y: 444, layer: 'ysort', flipX: true },
  ],

  // 13. Đại Lộ Đổi Mới 1986 (768×544)
  doi_moi_avenue: [
    { id: 'dmd_grate_1', spriteId: 'decor_cobble_drain_grate', x: 304, y: 344, layer: 'ground' },
    { id: 'dmd_newsstand', spriteId: 'decor_newsstand_1986', x: 144, y: 168, layer: 'ysort' },
    { id: 'dmd_scooter_1', spriteId: 'decor_classic_scooter', x: 616, y: 168, layer: 'ysort', flipX: true },
    { id: 'dmd_bicycle_park', spriteId: 'decor_vintage_bicycle', x: 224, y: 444, layer: 'ysort' },
    { id: 'dmd_tea_corner', spriteId: 'decor_tea_stall_set', x: 524, y: 448, layer: 'ysort', flipX: true },
    { id: 'dmd_planter_mid', spriteId: 'decor_ceramic_planter', x: 384, y: 452, layer: 'ysort' },
    { id: 'dmd_lanterns_st', spriteId: 'decor_festive_lantern_string', x: 384, y: 162, layer: 'canopy' },
  ],

  // 14. Quảng Trường Ký Ức Kỷ Nguyên Số 2045 (768×544)
  nexus_2045_plaza: [
    { id: 'nx45_rug_plaza', spriteId: 'decor_crimson_runner_rug', x: 384, y: 296, layer: 'ground' },
    { id: 'nx45_bench_nw', spriteId: 'decor_solar_bench_2045', x: 168, y: 168, layer: 'ysort' },
    { id: 'nx45_bench_ne', spriteId: 'decor_solar_bench_2045', x: 600, y: 168, layer: 'ysort', flipX: true },
    { id: 'nx45_stele_sw', spriteId: 'decor_holo_lotus_stele', x: 204, y: 444, layer: 'ysort' },
    { id: 'nx45_stele_se', spriteId: 'decor_holo_lotus_stele', x: 564, y: 444, layer: 'ysort', flipX: true },
    { id: 'nx45_bonsai_mid', spriteId: 'decor_bonsai_pedestal', x: 384, y: 452, layer: 'ysort' },
  ],
};

export function getZoneDecorProps(
  mapId: string,
  layer?: DecorRenderLayer
): ZoneDecorPlacement[] {
  const list = ZONE_DECOR_PROPS[mapId] ?? [];
  if (!layer) return list;
  return list.filter((item) => item.layer === layer);
}

/**
 * Vẽ toàn bộ tĩnh vật của một lớp ('ground' hoặc 'canopy') cho bản đồ hiện hành.
 * Với lớp `'ysort'`, `PixelRenderer` đưa từng phần tử vào `depthQueue` theo `footY = item.y`.
 */
export function drawZoneDecorLayer(
  ctx: CanvasRenderingContext2D,
  mapData: NormalizedMapData,
  layer: Exclude<DecorRenderLayer, 'ysort'>,
  cameraX: number,
  cameraY: number,
  viewW: number,
  viewH: number
): void {
  const items = getZoneDecorProps(mapData.id, layer);
  if (items.length === 0) return;

  for (const item of items) {
    const sx = Math.round(item.x - cameraX);
    const sy = Math.round(item.y - cameraY);
    if (sx < -40 || sx > viewW + 40 || sy < -40 || sy > viewH + 40) continue;
    drawDecorSprite(ctx, item.spriteId, sx, sy, Boolean(item.flipX));
  }
}
