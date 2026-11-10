/**
 * @file src/engine/SpriteAtlas.ts
 * @description Bộ dựng ma trận điểm ảnh 16-bit (Procedural Pixel Art Sprites & Top-Down Tiles)
 * tuân thủ tuyệt đối Bảng Màu 12 Sắc Độ Sơn Mài & Giấy Dó (DESIGN.md & SPRITE_ATLAS.md).
 * Nhân vật chính & NPC được thiết kế ở kích thước chuẩn 16×22 (hoặc 16×21) điểm ảnh với
 * phom dáng (silhouette) tách biệt rõ nét theo từng vai trò lịch sử.
 */

import { PALETTE_12 } from '../data/chronicles';
import type { EraId, Facing4, MilestoneArtifact, NormalizedMapData } from '../types/game';

export const CHARACTER_SPRITE_WIDTH = 16;
export const CHARACTER_SPRITE_HEIGHT = 22;

export type CharacterAtlasRole =
  | 'player'
  | 'liaison'
  | 'worker'
  | 'guard'
  | 'scholar'
  | 'traveler'
  | 'courier'
  | 'archivist'
  | 'observer'
  | 'historian';

const COLOR_MAP: Record<string, string> = {
  '.': '',
  'K': PALETTE_12.LACQUER_VOID,      // #110E0B - Viền tối Sơn Mài
  'L': PALETTE_12.LACQUER_DEEP,      // #1B1612 - Nền Sơn Mài đậm
  'D': PALETTE_12.LACQUER_ELEVATED,  // #241D17 - Áo tối
  'I': PALETTE_12.INK_PRIMARY,       // #232116 - Mực tàu đậm
  'S': PALETTE_12.INK_SECONDARY,     // #3E3A2C - Đá xám / Mực nhạt
  'M': PALETTE_12.BRONZE_MUTED,      // #6E4720 - Gỗ / Nâu trầm
  'B': PALETTE_12.BRONZE_ANTIQUE,    // #8A5A2B - Áo choàng đồng cổ
  'G': PALETTE_12.BRONZE_BRIGHT,     // #D4A55D - Vàng kim / Đèn lồng
  'R': PALETTE_12.CRIMSON_SEAL,      // #9E3321 - Khăn / Cờ đỏ son
  'V': PALETTE_12.CRIMSON_VERMILION, // #DF6B57 - Điểm sáng Chu Sa
  'P': PALETTE_12.PARCHMENT_WARM,    // #E6E5DC - Giấy Dó / Da
  'W': PALETTE_12.PARCHMENT_LIGHT,   // #F4EFE4 - Điểm sáng trắng ngà
};

interface DirectionalCharacterSet {
  downIdle: string[];
  downBreathe: string[];
  downWalk1: string[];
  downWalk2: string[];
  upIdle: string[];
  upBreathe: string[];
  upWalk1: string[];
  upWalk2: string[];
  sideIdle: string[];
  sideBreathe: string[];
  sideWalk1: string[];
  sideWalk2: string[];
  crouch?: string[];
}

// ============================================================================
// 1. LỮ KHÁCH KÝ ỨC 2045 (TRAVELER / PLAYER) — 16×22
// Mũ beret lưu trữ viên viền vàng kim (G/W), khăn quàng đỏ son tung bay (R/V),
// áo khoác măng-tô đồng cổ thắt đai (B/M/G), ủng tối (D/K).
// ============================================================================
const TRAVELER_SET: DirectionalCharacterSet = {
  downIdle: [
    '....KKKKKKKK....',
    '...KBBBBBBMMK...',
    '..KBBBBBBBBMMK..',
    '..KGGGGGGGGWGK..',
    '..KPPPPPPPPPPK..',
    '..KPWKPPPPKWPK..',
    '...KPPPPPPPPK...',
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRRBBBBK.',
    '.KBBMBBRRBBMBBK.',
    '.KBBBBBRVBBBBBK.',
    '.KPBBMGGGGMBBPK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
    '..KBBBK..KBBBK..',
    '..KMMMK..KMMMK..',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downBreathe: [
    '....KKKKKKKK....',
    '...KBBBBBBMMK...',
    '..KBBBBBBBBMMK..',
    '..KGGGGGGGGWGK..',
    '..KPPPPPPPPPPK..',
    '..KPWKPPPPKWPK..',
    '...KPPPPPPPPK...',
    '..KRRRRVVRVRRK..',
    '.KBBBBBRRRRBBBK.',
    '.KBBMBBRRRBMMPK.',
    '.KPBBBBBRVBBBBK.',
    '..KBBMGGGGMBBPK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
    '..KBBBK..KBBBK..',
    '..KMMMK..KMMMK..',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downWalk1: [
    '....KKKKKKKK....',
    '...KBBBBBBMMK...',
    '..KBBBBBBBBMMK..',
    '..KGGGGGGGGWGK..',
    '..KPPPPPPPPPPK..',
    '..KPWKPPPPKWPK..',
    '...KPPPPPPPPK...',
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRRRBBBK.',
    'KBBMBBBRRRBBBBBK',
    'KPBBBBBRRVBBBBK.',
    '.KBBBMGGGGMBBPK.',
    '..KBBBBMMBBBBK..',
    '.KBBBBBKKBBBBBK.',
    '.KBBBBK..KBBBBK.',
    '.KMMMK....KBBBK.',
    '..KDDK....KMMMK.',
    '..KDDK.....KDDK.',
    '..KMMK.....KMMK.',
    '.KDDDK.....KKKK.',
    '.KKKKK..........',
    '................',
  ],
  downWalk2: [
    '....KKKKKKKK....',
    '...KBBBBBBMMK...',
    '..KBBBBBBBBMMK..',
    '..KGGGGGGGGWGK..',
    '..KPPPPPPPPPPK..',
    '..KPWKPPPPKWPK..',
    '...KPPPPPPPPK...',
    '..KRRRRVVRRRRK..',
    '.KBBBRRRRBBBBBK.',
    'KBBBBBRRRBBBMBBK',
    '.KBBBBVRRBBBBBPK',
    '.KPBBMGGGGMBBBK.',
    '..KBBBBMMBBBBK..',
    '.KBBBBBKKBBBBBK.',
    '.KBBBBK..KBBBBK.',
    '.KBBBK....KMMMK.',
    '.KMMMK....KDDK..',
    '.KDDK.....KDDK..',
    '.KMMK.....KMMK..',
    '.KKKK.....KDDDK.',
    '..........KKKKK.',
    '................',
  ],
  upIdle: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGGK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRRBBBBK.',
    '.KBBBBBRVRBBBBK.',
    '.KBBMBBRRBBMBBK.',
    '.KBBBBBRKBBBBBK.',
    '.KPBBMMMMMMBBPK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
    '..KBBBK..KBBBK..',
    '..KMMMK..KMMMK..',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upBreathe: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGGK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRRRBBBK.',
    '.KBBBBBRVRRBBBK.',
    '.KBBMBBBRRBMMPK.',
    '.KPBBBBBRKBBBBK.',
    '..KBBMMMMMMBBPK.',
    '..KBBBBMMBBBBK..',
    '..KBBBBMMBBBBK..',
    '..KBBBBKKBBBBK..',
    '..KBBBK..KBBBK..',
    '..KMMMK..KMMMK..',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upWalk1: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGGK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '..KRRRRVVRRRRK..',
    '.KBBBBBRRRRBBBK.',
    'KBBBBBBRVRRBBBBK',
    'KPBBBBBRRRBBBBK.',
    '.KBBMMMMMMMBBPK.',
    '..KBBBBMMBBBBK..',
    '.KBBBBBKKBBBBBK.',
    '.KBBBBK..KBBBBK.',
    '.KMMMK....KBBBK.',
    '..KDDK....KMMMK.',
    '..KDDK.....KDDK.',
    '..KMMK.....KMMK.',
    '.KDDDK.....KKKK.',
    '.KKKKK..........',
    '................',
    '................',
  ],
  upWalk2: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGGK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '..KRRRRVVRRRRK..',
    '.KBBBRRRRBBBBBK.',
    'KBBBBRRVRBBBBBBK',
    '.KBBBBRRRBBBBBPK',
    '.KPBBMMMMMMMBBK.',
    '..KBBBBMMBBBBK..',
    '.KBBBBBKKBBBBBK.',
    '.KBBBBK..KBBBBK.',
    '.KBBBK....KMMMK.',
    '.KMMMK....KDDK..',
    '.KDDK.....KDDK..',
    '.KMMK.....KMMK..',
    '.KKKK.....KDDDK.',
    '..........KKKKK.',
    '................',
    '................',
  ],
  sideIdle: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGWK..',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWKK..',
    '....KPPPPPPPK...',
    '.KKKRRRRVVRRK...',
    'KRRVBBBBBBBBK...',
    '.KKKBBMBBBBBK...',
    '...KBBBBBBMBK...',
    '...KBBMGGGGBK...',
    '...KBBBBPPBBK...',
    '...KBBBBMMBBK...',
    '..KBBBBBBBBBK...',
    '..KBBBBKKBBBK...',
    '..KMMMK.KMMMK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideBreathe: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGWK..',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWKK..',
    '....KPPPPPPPK...',
    'KKKKRRRRVVRRK...',
    'KRVKBBBBBBBBK...',
    '.KK.KBBMBBBBK...',
    '...KBBBBBBMBK...',
    '...KBBMGGGGBK...',
    '...KBBBBPPBBK...',
    '...KBBBBMMBBK...',
    '..KBBBBBBBBBK...',
    '..KBBBBKKBBBK...',
    '..KMMMK.KMMMK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideWalk1: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGWK..',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWKK..',
    '....KPPPPPPPK...',
    'KKKKRRRRVVRRK...',
    'KRRVBBBBBBBBK...',
    '.KKKBBBBBBMBBK..',
    '...KBBMBBBBPPK..',
    '...KBBMGGGGBKK..',
    '..KBBBBBBBBBK...',
    '..KBBBBKKBBBBK..',
    '.KBBBBK..KBBBBK.',
    '.KMMMK....KMMMK.',
    '.KDDK......KDDK.',
    '.KMMK......KDDK.',
    'KDDDK......KMMK.',
    'KKKK.......KDDDK',
    '...........KKKKK',
    '................',
  ],
  sideWalk2: [
    '....KKKKKKKK....',
    '...KMMBBBBBBK...',
    '..KMMBBBBBBBBK..',
    '..KGGGGGGGGGWK..',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWKK..',
    '....KPPPPPPPK...',
    '.KKKRRRRVVRRK...',
    'KRVKBBBBBBBBK...',
    '.KKKPPBBBBMBK...',
    '..KKBBBBBBBBK...',
    '...KBBMGGGGBK...',
    '...KBBBBBBBBK...',
    '..KBBBBKKBBBBK..',
    '..KBBBK..KBBBK..',
    '..KMMMK..KMMMK..',
    '..KDDK....KDDK..',
    '..KDDK....KMMK..',
    '..KMMK....KDDDK.',
    '.KDDDK....KKKKK.',
    '.KKKKK..........',
    '................',
  ],
  crouch: [
    '....KKKKKKKK....',
    '...KBBBBBBMMK...',
    '..KBBBBBBBBMMK..',
    '..KGGGGGGGGWGK..',
    '..KPWKPPPPKWPK..',
    '.KRRRRVVRRRRRRK.',
    'KBBBBBRRRRBBBBBK',
    'KBBMBBBRVBBBMBBK',
    'KPBBBBGGGGMBBBPK',
    '.KBBBBMMMMBBBBK.',
    '.KBBBBBBBBBBBBK.',
    '..KBBBBKKBBBBK..',
    '..KMMMK..KMMMK..',
    '..KDDDK..KDDDK..',
    '..KMMMK..KMMMK..',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
};

// ============================================================================
// 2. GIAO LIÊN / LIÊN LẠC (COURIER / LIAISON) — 16×21
// Mũ lưỡi trai phẳng (M/B + vành K/D), áo khoác chàm mực (D/S),
// dây đeo chéo vàng đồng & túi da công văn bên hông (G/P/M).
// ============================================================================
const COURIER_SET: DirectionalCharacterSet = {
  downIdle: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KBBBBBBBBMMK..',
    '..KDDDDDDDDDDKK.',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '...KSSPPPPSSK...',
    '..KDDGDDDDDDDK..',
    '..KDDDGGDDDDDK..',
    '.KPDDDDGGDDDSPK.',
    '..KDDDDDGGDDDK..',
    '..KDDDDDMGGGGK..',
    '..KSSSSSMGPPGK..',
    '...KDDDDMMMMK...',
    '...KDDDK.KDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downBreathe: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KBBBBBBBBMMK..',
    '..KDDDDDDDDDDKK.',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '...KSSPPPPSSK...',
    '..KDDGDDDDDDDK..',
    '.KPDDDGGDDDDDPK.',
    '..KDDDDGGDDDDK..',
    '..KDDDDDGGDDDK..',
    '..KDDDDDMGGGGK..',
    '..KSSSSSMGPPGK..',
    '...KDDDDMMMMK...',
    '...KDDDK.KDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downWalk1: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KBBBBBBBBMMK..',
    '..KDDDDDDDDDDKK.',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '...KSSPPPPSSK...',
    '..KDDGDDDDDDDK..',
    '.KPDDDGGDDDDDK..',
    '.KDDDDDGGDDDSPK.',
    '..KDDDDDGGDDDK..',
    '..KSSSSSMGGGGK..',
    '...KDDDDMGPPGK..',
    '..KDDDK..MMMMK..',
    '..KDDK....KDDK..',
    '..KMMK....KDDK..',
    '.KDDDK....KMMK..',
    '.KKKKK....KKKK..',
    '................',
    '................',
  ],
  downWalk2: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KBBBBBBBBMMK..',
    '..KDDDDDDDDDDKK.',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '...KSSPPPPSSK...',
    '..KDDGDDDDDDDK..',
    '..KDDDGGDDDDDPK.',
    '.KPDDDDGGDDDDSK.',
    '..KDDDDDGGDDDK..',
    '..KSSSSSMGGGGK..',
    '...KDDDDMGPPGK..',
    '..KDDDK..MMMMK..',
    '..KDDK....KDDK..',
    '..KDDK....KMMK..',
    '..KMMK....KDDDK.',
    '..KKKK....KKKKK.',
    '................',
    '................',
  ],
  upIdle: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KSSSSSSSSK...',
    '..KDDDDDDGGDDK..',
    '..KDDDDDGGDDDK..',
    '.KPDDDDGGDDDDPK.',
    '..KDDDGGDDDDDK..',
    '..KDDGGDDDDDDK..',
    '..KGGGGMDDDDDK..',
    '..KGPPGMSSSSSK..',
    '...KMMMMDDDDK...',
    '...KDDK.KDDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upBreathe: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KSSSSSSSSK...',
    '..KDDDDDDGGDDK..',
    '.KPDDDDDGGDDDPK.',
    '..KDDDDGGDDDDK..',
    '..KDDDGGDDDDDK..',
    '..KDDGGDDDDDDK..',
    '..KGGGGMDDDDDK..',
    '..KGPPGMSSSSSK..',
    '...KMMMMDDDDK...',
    '...KDDK.KDDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upWalk1: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KSSSSSSSSK...',
    '..KDDDDDDGGDDK..',
    '.KPDDDDDGGDDDK..',
    '..KDDDDGGDDDSPK.',
    '..KDDDGGDDDDDK..',
    '..KGGGGMSSSSSK..',
    '..KGPPGMDDDDK...',
    '...KMMMM.KDDDK..',
    '..KDDK....KDDK..',
    '..KMMK....KDDK..',
    '.KDDDK....KMMK..',
    '.KKKKK....KKKK..',
    '................',
    '................',
    '................',
  ],
  upWalk2: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KSSSSSSSSK...',
    '..KDDDDDDGGDDK..',
    '..KDDDDDGGDDDPK.',
    '.KPDDDDGGDDDDSK.',
    '..KDDDGGDDDDDK..',
    '..KGGGGMSSSSSK..',
    '..KGPPGMDDDDK...',
    '...KMMMM.KDDDK..',
    '..KDDK....KDDK..',
    '..KDDK....KMMK..',
    '..KMMK....KDDDK.',
    '..KKKK....KKKKK.',
    '................',
    '................',
    '................',
  ],
  sideIdle: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBKK.',
    '..KLLDDDDDDDDKKK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KSSDDDDSSK...',
    '...KDDGGDDDDK...',
    '...KDDDGGDDDK...',
    '...KDDDDGGDDK...',
    '...KDDDDPPDDK...',
    '..KGGGGMDDDDK...',
    '..KGPPGMSSSSK...',
    '..KMMMMKDDDDK...',
    '...KDDDK.KDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideBreathe: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBKK.',
    '..KLLDDDDDDDDKKK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KSSDDDDSSK...',
    '...KDDGGDDDDK...',
    '...KDDDGGDDDK...',
    '...KDDDDGGPPK...',
    '..KGGGGMDDDDK...',
    '..KGPPGMDDDDK...',
    '..KMMMMKSSSSK...',
    '...KDDDDDDDDK...',
    '...KDDDK.KDDK...',
    '...KDDK..KDDK...',
    '...KDDK..KDDK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideWalk1: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBKK.',
    '..KLLDDDDDDDDKKK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KSSDDDDSSK...',
    '...KDDGGDDDDK...',
    '...KDDDGGDDPPK..',
    '..KGGGGMDDDDKK..',
    '..KGPPGMDDDDK...',
    '..KMMMMKSSSSK...',
    '..KDDDDK.KDDDK..',
    '.KDDDK....KDDK..',
    '.KDDK......KDDK.',
    '.KMMK......KMMK.',
    'KDDDK......KDDDK',
    'KKKK.......KKKKK',
    '................',
    '................',
  ],
  sideWalk2: [
    '....KKKKKKKK....',
    '...KMMMMMMMMK...',
    '..KMMBBBBBBBBKK.',
    '..KLLDDDDDDDDKKK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KSSDDDDSSK...',
    '...KDDGGDDDDK...',
    '..KPPDDGGDDDK...',
    '..KKDDDDGGDDK...',
    '..KGGGGMDDDDK...',
    '..KGPPGMSSSSK...',
    '..KMMMMKDDDDK...',
    '..KDDDK..KDDDK..',
    '..KDDK....KDDK..',
    '..KDDK....KMMK..',
    '..KMMK....KDDDK.',
    '.KDDDK....KKKKK.',
    '.KKKKK..........',
    '................',
  ],
};

// ============================================================================
// 3. HỌC GIẢ / LƯU TRỮ VIÊN / SỬ GIA (ARCHIVIST / SCHOLAR / HISTORIAN) — 16×22
// Khăn đóng xếp lớp truyền thống (K/L/D), kính trắng ngà lấp lánh (W),
// áo dài thâm (L/I/D) tà dài chấm gót, tay nâng cuộn thư / sổ sử liệu đỏ son (P/W/R).
// ============================================================================
const SCHOLAR_SET: DirectionalCharacterSet = {
  downIdle: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLDDDDDDLLK..',
    '...KPPPPPPPPK...',
    '..KPWWKPPKWWPK..',
    '...KPPPPPPPPK...',
    '...KLLWWWWLLK...',
    '..KLLLIIIILLLK..',
    '.KLLLLILLLPPPPK.',
    '.KPLLLILLLPWRPK.',
    '..KLLLILLLPPPPK.',
    '..KLLLIIIILLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLK..KLLLK..',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downBreathe: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLDDDDDDLLK..',
    '...KPPPPPPPPK...',
    '..KPWWKPPKWWPK..',
    '...KPPPPPPPPK...',
    '...KLLWWWWLLK...',
    '..KLLLIIIILLLK..',
    '.KLLLLILLPPPPK..',
    '.KPLLLILLPWRPPK.',
    '..KLLLILLPPPPK..',
    '..KLLLIIIILLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLK..KLLLK..',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  downWalk1: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLDDDDDDLLK..',
    '...KPPPPPPPPK...',
    '..KPWWKPPKWWPK..',
    '...KPPPPPPPPK...',
    '...KLLWWWWLLK...',
    '..KLLLIIIILLLK..',
    '.KPLLLILLLPPPPK.',
    '..KLLLILLLPWRPK.',
    '..KLLLIIIIPPPPK.',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '.KLLLLKPPKLLLLK.',
    '.KLLLK.PP.KLLLK.',
    '.KLLLK....KLLLK.',
    '..KMMK.....KMMK.',
    '.KDDDK.....KKKK.',
    '.KKKKK..........',
    '................',
    '................',
  ],
  downWalk2: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLDDDDDDLLK..',
    '...KPPPPPPPPK...',
    '..KPWWKPPKWWPK..',
    '...KPPPPPPPPK...',
    '...KLLWWWWLLK...',
    '..KLLLIIIILLLK..',
    '.KLLLLILLLPPPPK.',
    '.KPLLLILLLPWRPK.',
    '..KLLLIIIIPPPPK.',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '.KLLLLKPPKLLLLK.',
    '.KLLLK.PP.KLLLK.',
    '.KLLLK....KLLLK.',
    '.KMMK.....KMMK..',
    '.KKKK.....KDDDK.',
    '..........KKKKK.',
    '................',
    '................',
  ],
  upIdle: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KLLWWWWLLK...',
    '..KLLLLIILLLLK..',
    '.KLLLLLIILLLLLK.',
    '.KPLLLLIILLLLPK.',
    '..KLLLLIILLLLK..',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLK..KLLLK..',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upBreathe: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KLLWWWWLLK...',
    '..KLLLLIILLLLK..',
    '.KPLLLLIILLLLPK.',
    '..KLLLLIILLLLK..',
    '..KLLLLIILLLLK..',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLLDDLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLK..KLLLK..',
    '...KMMK..KMMK...',
    '..KDDDK..KDDDK..',
    '..KKKKK..KKKKK..',
  ],
  upWalk1: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KLLWWWWLLK...',
    '..KLLLLIILLLLK..',
    '.KPLLLLIILLLLPK.',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '.KLLLLKPPKLLLLK.',
    '.KLLLK.PP.KLLLK.',
    '.KLLLK....KLLLK.',
    '..KMMK.....KMMK.',
    '.KDDDK.....KKKK.',
    '.KKKKK..........',
    '................',
    '................',
    '................',
  ],
  upWalk2: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '...KLLWWWWLLK...',
    '..KLLLLIILLLLK..',
    '.KPLLLLIILLLLPK.',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLDDLLLLK..',
    '.KLLLLKPPKLLLLK.',
    '.KLLLK.PP.KLLLK.',
    '.KLLLK....KLLLK.',
    '.KMMK.....KMMK..',
    '.KKKK.....KDDDK.',
    '..........KKKKK.',
    '................',
    '................',
    '................',
  ],
  sideIdle: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLDDDDLLK..',
    '...KLLPPPPPPK...',
    '...KLPPPWWKWWK..',
    '....KPPPPPPPK...',
    '...KLLWWWWLLK...',
    '...KLLLLIILLK...',
    '...KLLLLIIPPPPK.',
    '...KLLLLIIPWRPK.',
    '...KLLLLPPPPP.K.',
    '...KLLLLIILLK...',
    '...KLLLLLLLLK...',
    '...KLLLDDLLLK...',
    '..KLLLLPPLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '...KLLK..KLLK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideBreathe: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLDDDDLLK..',
    '...KLLPPPPPPK...',
    '...KLPPPWWKWWK..',
    '....KPPPPPPPK...',
    '...KLLWWWWLLK...',
    '...KLLLLIILLK...',
    '...KLLLLIIPPPPK.',
    '...KLLLLPPPWPPK.',
    '...KLLLLIIPRPPK.',
    '...KLLLLIILLK...',
    '...KLLLLLLLLK...',
    '...KLLLDDLLLK...',
    '..KLLLLPPLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '...KLLK..KLLK...',
    '...KMMK..KMMK...',
    '...KDDDK.KDDDK..',
    '...KKKKK.KKKKK..',
  ],
  sideWalk1: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLDDDDLLK..',
    '...KLLPPPPPPK...',
    '...KLPPPWWKWWK..',
    '....KPPPPPPPK...',
    '...KLLWWWWLLK...',
    '...KLLLLIILLK...',
    '...KLLLLIIPPPPK.',
    '...KLLLLPPPWRPK.',
    '...KLLLLIIPPPPK.',
    '...KLLLLLLLLK...',
    '..KLLLLDDLLLLK..',
    '.KLLLLKPPKLLLLK.',
    '.KLLLK.PP.KLLLK.',
    '.KLLK......KLLK.',
    '.KMMK......KMMK.',
    'KDDDK......KDDDK',
    'KKKK.......KKKKK',
    '................',
    '................',
  ],
  sideWalk2: [
    '....KKKKKKKK....',
    '...KLLLLLLLLK...',
    '..KLDDDDDDDDLK..',
    '..KLLLLDDDDLLK..',
    '...KLLPPPPPPK...',
    '...KLPPPWWKWWK..',
    '....KPPPPPPPK...',
    '...KLLWWWWLLK...',
    '...KLLLLIILLK...',
    '...KLLLLIIPPPPK.',
    '...KLLLLPPPWRPK.',
    '...KLLLLIIPPPPK.',
    '...KLLLLLLLLK...',
    '..KLLLLDDLLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLLKPPKLLLK..',
    '..KLLK....KLLK..',
    '..KMMK....KMMK..',
    '.KDDDK....KDDDK.',
    '.KKKKK....KKKK..',
    '................',
    '................',
  ],
};

// ============================================================================
// 4. PHU BẾN CẢNG / NGƯỜI QUAN SÁT (WORKER / OBSERVER) — 16×21
// Nón lá / nón rộng vành chóp nhọn 16px (G/B/M), áo cánh Giấy Dó xắn tay (P/W)
// khoác ghi-lê nâu gỗ (M/B), đai lưng đỏ son (R), quần vải thô bền chắc (D/S).
// ============================================================================
const WORKER_SET: DirectionalCharacterSet = {
  downIdle: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '..KPPPMMMPPPPK..',
    '.KPPPMMBBMMPPPK.',
    '.KPP.MMBBMM.PPK.',
    '.....MMBBMM.....',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '..KDDDDKKDDDDK..',
    '..KDDDK..KDDDK..',
    '..KSSSK..KSSSK..',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '..KMMMK..KMMMK..',
    '..KKKKK..KKKKK..',
  ],
  downBreathe: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '..KPPPMMMPPPPK..',
    '.KPPPMMBBMMPPPK.',
    '..KPPMMBBMMPPK..',
    '...K.MMBBMM.K...',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '..KDDDDKKDDDDK..',
    '..KDDDK..KDDDK..',
    '..KSSSK..KSSSK..',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '..KMMMK..KMMMK..',
    '..KKKKK..KKKKK..',
  ],
  downWalk1: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '.KPPPPMMMPPPPK..',
    '.KPP.MMBBMMPPPK.',
    '.....MMBBMM.PPK.',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '.KDDDDK..KDDDDK.',
    '.KDDDK....KSSSK.',
    '.KSSSK.....KPPK.',
    '..KPPK.....KMMK.',
    '.KMMMK.....KKKK.',
    '.KKKKK..........',
    '................',
    '................',
  ],
  downWalk2: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KPPPPPPPPK...',
    '...KPWKPPKWPK...',
    '....KPPPPPPK....',
    '..KPPPPMMMPPPPK.',
    '.KPPPMMBBMM.PPK.',
    '.KPP.MMBBMM.....',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '.KDDDDK..KDDDDK.',
    '.KSSSK....KDDDK.',
    '.KPPK.....KSSSK.',
    '.KMMK.....KPPK..',
    '.KKKK.....KMMMK.',
    '..........KKKKK.',
    '................',
    '................',
  ],
  upIdle: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '..KPPPMMMMPPPK..',
    '.KPPPMMBBMMPPPK.',
    '.KPP.MMBBMM.PPK.',
    '.....MMBBMM.....',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '..KDDDDKKDDDDK..',
    '..KDDDK..KDDDK..',
    '..KSSSK..KSSSK..',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '..KMMMK..KMMMK..',
    '..KKKKK..KKKKK..',
    '................',
  ],
  upBreathe: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '..KPPPMMMMPPPK..',
    '.KPPPMMBBMMPPPK.',
    '..KPPMMBBMMPPK..',
    '...K.MMBBMM.K...',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '..KDDDDKKDDDDK..',
    '..KDDDK..KDDDK..',
    '..KSSSK..KSSSK..',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '..KMMMK..KMMMK..',
    '..KKKKK..KKKKK..',
    '................',
  ],
  upWalk1: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '.KPPPPMMMMPPPK..',
    '.KPP.MMBBMMPPPK.',
    '.....MMBBMM.PPK.',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '.KDDDDK..KDDDDK.',
    '.KDDDK....KSSSK.',
    '.KSSSK.....KPPK.',
    '..KPPK.....KMMK.',
    '.KMMMK.....KKKK.',
    '.KKKKK..........',
    '................',
    '................',
    '................',
  ],
  upWalk2: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLLLLLLLK...',
    '....KPPPPPPK....',
    '..KPPPPMMMMPPPK.',
    '.KPPPMMBBMM.PPK.',
    '.KPP.MMBBMM.....',
    '...KRRRRRRRRK...',
    '..KMMMMMMMMMMK..',
    '.KDDDDK..KDDDDK.',
    '.KSSSK....KDDDK.',
    '.KPPK.....KSSSK.',
    '.KMMK.....KPPK..',
    '.KKKK.....KMMMK.',
    '..........KKKKK.',
    '................',
    '................',
    '................',
  ],
  sideIdle: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KMMBBBBMMK...',
    '...KMMBPPBMMK...',
    '...KMMBPPBMMK...',
    '...KMMBBBBMMK...',
    '...KRRRRRRRRK...',
    '...KMMMMMMMMK...',
    '...KDDDDDDDDK...',
    '...KDDDK.KDDK...',
    '...KSSK..KSSK...',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '...KMMMK.KMMMK..',
    '...KKKKK.KKKKK..',
  ],
  sideBreathe: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KMMBBBBMMK...',
    '...KMMBPPBMMK...',
    '...KMMBBBBPPK...',
    '...KMMBBBBMMK...',
    '...KRRRRRRRRK...',
    '...KMMMMMMMMK...',
    '...KDDDDDDDDK...',
    '...KDDDK.KDDK...',
    '...KSSK..KSSK...',
    '...KPPK..KPPK...',
    '...KMMK..KMMK...',
    '...KMMMK.KMMMK..',
    '...KKKKK.KKKKK..',
  ],
  sideWalk1: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KMMBBBBMMK...',
    '...KMMBBBBPPPK..',
    '...KMMBBBBMMKK..',
    '...KRRRRRRRRK...',
    '...KMMMMMMMMK...',
    '..KDDDDKKDDDDK..',
    '.KDDDK....KDDDK.',
    '.KSSK......KSSK.',
    '.KPPK......KPPK.',
    'KMMMK......KMMMK',
    'KKKK.......KKKKK',
    '................',
    '................',
  ],
  sideWalk2: [
    '......KKKK......',
    '....KKGGGGKK....',
    '..KKGGGGGGGGKK..',
    '.KGGGGGGGGGGGGK.',
    'KBBBBBBBBBBBBBBK',
    '...KLLPPPPPPK...',
    '...KLPPPPPWWK...',
    '....KPPPPPPK....',
    '...KMMBBBBMMK...',
    '..KPPPMBBBMMK...',
    '..KKMMBBBBMMK...',
    '...KRRRRRRRRK...',
    '...KMMMMMMMMK...',
    '..KDDDDKKDDDDK..',
    '..KDDDK..KDDDK..',
    '..KSSK....KSSK..',
    '..KPPK....KPPK..',
    '.KMMMK....KMMMK.',
    '.KKKKK....KKKK..',
    '................',
    '................',
  ],
};

// ============================================================================
// 5. LÍNH TUẦN TRA THUỘC ĐỊA (GUARD) — 16×22
// Mũ kepi / mũ cối tuần tra chóp cao gắn huy hiệu đồng (S/I/K + G),
// cầu vai rộng đính lon đỏ son (R/V), quân phục tối thắt dây chéo & thắt lưng đỏ (L/I/R/G).
// ============================================================================
const GUARD_SET: DirectionalCharacterSet = {
  downIdle: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSGGSSSSK...',
    '..KIIIIIIIIIIK..',
    '..KKKKKKKKKKKK..',
    '...KPPPPPPPPK...',
    '...KPKKPPKKPK...',
    '....KPPPPPPK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLRLLLLLLLLK',
    'KLLLLLLRLLLLLLLK',
    'KPLLLLLLRLLLLLPK',
    '.KRRRRGGGGRRRRK.',
    '..KLLLLIILLLLK..',
    '..KLLLLKKLLLLK..',
    '..KLLLK..KLLLK..',
    '..KIIIK..KIIIK..',
    '...KLLK..KLLK...',
    '...KLLK..KLLK...',
    '...KDDK..KDDK...',
    '..KKKKK..KKKKK..',
    '..KKKKK..KKKKK..',
  ],
  downBreathe: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSWGSSSSK...',
    '..KIIIIIIIIIIK..',
    '..KKKKKKKKKKKK..',
    '...KPPPPPPPPK...',
    '...KPKKPPKKPK...',
    '....KPPPPPPK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLRLLLLLLLLK',
    'KPLLLLLRLLLLLLPK',
    '.KLLLLLLRLLLLLK.',
    '.KRRRRGGGGRRRRK.',
    '..KLLLLIILLLLK..',
    '..KLLLLKKLLLLK..',
    '..KLLLK..KLLLK..',
    '..KIIIK..KIIIK..',
    '...KLLK..KLLK...',
    '...KLLK..KLLK...',
    '...KDDK..KDDK...',
    '..KKKKK..KKKKK..',
    '..KKKKK..KKKKK..',
  ],
  downWalk1: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSGGSSSSK...',
    '..KIIIIIIIIIIK..',
    '..KKKKKKKKKKKK..',
    '...KPPPPPPPPK...',
    '...KPKKPPKKPK...',
    '....KPPPPPPK....',
    '.KRRVLLLLLLVRRK.',
    'KPLLLLRLLLLLLLLK',
    '.KLLLLLRLLLLLLPK',
    '.KRRRRGGGGRRRRK.',
    '..KLLLLIILLLLK..',
    '.KLLLLK..KLLLLK.',
    '.KLLLK....KIIIK.',
    '.KIIIK.....KLLK.',
    '..KLLK.....KDDK.',
    '..KDDK.....KKKK.',
    '.KKKKK..........',
    '.KKKKK..........',
    '................',
    '................',
  ],
  downWalk2: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSGGSSSSK...',
    '..KIIIIIIIIIIK..',
    '..KKKKKKKKKKKK..',
    '...KPPPPPPPPK...',
    '...KPKKPPKKPK...',
    '....KPPPPPPK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLRLLLLLLLPK',
    'KPLLLLLRLLLLLLK.',
    '.KRRRRGGGGRRRRK.',
    '..KLLLLIILLLLK..',
    '.KLLLLK..KLLLLK.',
    '.KIIIK....KLLLK.',
    '.KLLK.....KIIIK.',
    '.KDDK.....KLLK..',
    '.KKKK.....KDDK..',
    '..........KKKKK.',
    '..........KKKKK.',
    '................',
    '................',
  ],
  upIdle: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSSSK...',
    '..KIIIIIIIIIIK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KLLLLLLK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLLLLRLLLLLK',
    'KLLLLLLLRLLLLLLK',
    'KPLLLLLRLLLLLLPK',
    '.KRRRRRRRRRRRRK.',
    '..KLLLLIILLLLK..',
    '..KLLLLKKLLLLK..',
    '..KLLLK..KLLLK..',
    '..KIIIK..KIIIK..',
    '...KLLK..KLLK...',
    '...KLLK..KLLK...',
    '...KDDK..KDDK...',
    '..KKKKK..KKKKK..',
    '..KKKKK..KKKKK..',
    '................',
  ],
  upBreathe: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSSSK...',
    '..KIIIIIIIIIIK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KLLLLLLK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLLLLRLLLLLK',
    'KPLLLLLLRLLLLLPK',
    '.KLLLLLRLLLLLLK.',
    '.KRRRRRRRRRRRRK.',
    '..KLLLLIILLLLK..',
    '..KLLLLKKLLLLK..',
    '..KLLLK..KLLLK..',
    '..KIIIK..KIIIK..',
    '...KLLK..KLLK...',
    '...KLLK..KLLK...',
    '...KDDK..KDDK...',
    '..KKKKK..KKKKK..',
    '..KKKKK..KKKKK..',
    '................',
  ],
  upWalk1: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSSSK...',
    '..KIIIIIIIIIIK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KLLLLLLK....',
    '.KRRVLLLLLLVRRK.',
    'KPLLLLLLLRLLLLLK',
    '.KLLLLLLRLLLLLPK',
    '.KRRRRRRRRRRRRK.',
    '..KLLLLIILLLLK..',
    '.KLLLLK..KLLLLK.',
    '.KLLLK....KIIIK.',
    '.KIIIK.....KLLK.',
    '..KLLK.....KDDK.',
    '..KDDK.....KKKK.',
    '.KKKKK..........',
    '.KKKKK..........',
    '................',
    '................',
    '................',
  ],
  upWalk2: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSSSK...',
    '..KIIIIIIIIIIK..',
    '...KLLLLLLLLK...',
    '...KLLLLLLLLK...',
    '....KLLLLLLK....',
    '.KRRVLLLLLLVRRK.',
    'KLLLLLLLLRLLLLPK',
    'KPLLLLLLRLLLLLK.',
    '.KRRRRRRRRRRRRK.',
    '..KLLLLIILLLLK..',
    '.KLLLLK..KLLLLK.',
    '.KIIIK....KLLLK.',
    '.KLLK.....KIIIK.',
    '.KDDK.....KLLK..',
    '.KKKK.....KDDK..',
    '..........KKKKK.',
    '..........KKKKK.',
    '................',
    '................',
    '................',
  ],
  sideIdle: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSGGK...',
    '..KIIIIIIIIIIKK.',
    '...KLLKKKKKKK...',
    '...KLLPPPPPPK...',
    '...KLPPPPKKPK...',
    '....KPPPPPPK....',
    '..KKLLLLRRVLLK..',
    '..KLLLLLRRLLLK..',
    '..KLLLLLLLLLLK..',
    '..KLLLLPPPLLLK..',
    '..KRRRRGGGRRRK..',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLK.KLLLK..',
    '...KIIIK.KIIIK..',
    '...KLLK...KLLK..',
    '...KLLK...KLLK..',
    '...KDDK...KDDK..',
    '...KKKKK..KKKKK.',
    '...KKKKK..KKKKK.',
  ],
  sideBreathe: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSWGK...',
    '..KIIIIIIIIIIKK.',
    '...KLLKKKKKKK...',
    '...KLLPPPPPPK...',
    '...KLPPPPKKPK...',
    '....KPPPPPPK....',
    '..KKLLLLRRVLLK..',
    '..KLLLLLRRLLLK..',
    '..KLLLLLLLPPPK..',
    '..KLLLLLLLLLLK..',
    '..KRRRRGGGRRRK..',
    '..KLLLLIILLLLK..',
    '..KLLLLLLLLLLK..',
    '...KLLLK.KLLLK..',
    '...KIIIK.KIIIK..',
    '...KLLK...KLLK..',
    '...KLLK...KLLK..',
    '...KDDK...KDDK..',
    '...KKKKK..KKKKK.',
    '...KKKKK..KKKKK.',
  ],
  sideWalk1: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSGGK...',
    '..KIIIIIIIIIIKK.',
    '...KLLKKKKKKK...',
    '...KLLPPPPPPK...',
    '...KLPPPPKKPK...',
    '....KPPPPPPK....',
    '..KKLLLLRRVLLK..',
    '..KLLLLLRRLLPPK.',
    '..KLLLLLLLLLLKK.',
    '..KRRRRGGGRRRK..',
    '..KLLLLIILLLLK..',
    '.KLLLLK..KLLLLK.',
    '.KLLLK....KLLLK.',
    '.KIIIK.....KIIIK',
    '.KLLK......KLLK.',
    'KDDDK......KDDK.',
    'KKKK.......KKKKK',
    '...........KKKKK',
    '................',
    '................',
  ],
  sideWalk2: [
    '.....KKKKKK.....',
    '....KSSSSSSK....',
    '...KSSSSSSGGK...',
    '..KIIIIIIIIIIKK.',
    '...KLLKKKKKKK...',
    '...KLLPPPPPPK...',
    '...KLPPPPKKPK...',
    '....KPPPPPPK....',
    '..KKLLLLRRVLLK..',
    '.KPPPLLLRRLLLK..',
    '.KKLLLLLLLLLLK..',
    '..KRRRRGGGRRRK..',
    '..KLLLLIILLLLK..',
    '..KLLLK..KLLLK..',
    '..KIIIK..KIIIK..',
    '..KLLK....KLLK..',
    '..KDDK....KDDK..',
    '.KKKKK....KKKKK.',
    '.KKKKK....KKKK..',
    '................',
    '................',
    '................',
  ],
};

function getCharacterSetForRole(role: CharacterAtlasRole): DirectionalCharacterSet {
  switch (role) {
    case 'player':
    case 'traveler':
      return TRAVELER_SET;
    case 'courier':
    case 'liaison':
      return COURIER_SET;
    case 'archivist':
    case 'scholar':
    case 'historian':
      return SCHOLAR_SET;
    case 'observer':
    case 'worker':
      return WORKER_SET;
    case 'guard':
      return GUARD_SET;
    default:
      return SCHOLAR_SET;
  }
}

function drawGroundedContactShadow(
  ctx: CanvasRenderingContext2D,
  footX: number,
  footY: number
): void {
  const rx = Math.round(footX);
  const ry = Math.round(footY);
  ctx.save();
  ctx.fillStyle = 'rgba(17, 14, 11, 0.52)';
  ctx.fillRect(rx - 5, ry - 3, 10, 1);
  ctx.fillRect(rx - 6, ry - 2, 12, 3);
  ctx.fillRect(rx - 5, ry + 1, 10, 1);
  ctx.restore();
}

export function drawMatrixSprite(
  ctx: CanvasRenderingContext2D,
  matrix: string[],
  x: number,
  y: number,
  scale = 1,
  flipX = false
): void {
  const rows = matrix.length;
  const cols = matrix[0]?.length ?? 0;

  ctx.save();
  if (flipX) {
    ctx.translate(Math.round(x + cols * scale), Math.round(y));
    ctx.scale(-1, 1);
  } else {
    ctx.translate(Math.round(x), Math.round(y));
  }

  for (let r = 0; r < rows; r++) {
    const rowStr = matrix[r];
    for (let c = 0; c < cols; c++) {
      const ch = rowStr[c];
      const color = COLOR_MAP[ch];
      if (color) {
        ctx.fillStyle = color;
        ctx.fillRect(c * scale, r * scale, scale, scale);
      }
    }
  }
  ctx.restore();
}

export function drawCharacter4Dir(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  role: CharacterAtlasRole,
  facing: Facing4,
  walkFrame: number,
  isSheltered?: boolean,
  reducedMotion?: boolean
): void;
export function drawCharacter4Dir(
  ctx: CanvasRenderingContext2D,
  role: CharacterAtlasRole,
  x: number,
  y: number,
  facing: Facing4,
  isMoving: boolean,
  isSheltered: boolean,
  timeSec: number,
  reducedMotion?: boolean
): void;
export function drawCharacter4Dir(
  ctx: CanvasRenderingContext2D,
  arg1: number | CharacterAtlasRole,
  arg2: number,
  arg3: CharacterAtlasRole | number,
  facing: Facing4,
  arg5: number | boolean,
  arg6 = false,
  arg7: number | boolean = 0,
  arg8?: boolean
): void {
  let role: CharacterAtlasRole;
  let x: number;
  let y: number;
  let stepPhase = 0;
  let isMoving = false;
  let timeSec = 0;
  const isSheltered = Boolean(arg6);

  const domReducedMotion =
    typeof document !== 'undefined' &&
    Boolean(document.body?.classList.contains('reduced-motion'));

  let reducedMotion = domReducedMotion;

  if (typeof arg1 === 'number') {
    x = arg1;
    y = arg2;
    role = arg3 as CharacterAtlasRole;
    if (typeof arg7 === 'boolean') {
      reducedMotion = arg7;
    }
    const wf = Math.abs(Math.floor(Number(arg5)));
    isMoving = !reducedMotion && wf > 0;
    stepPhase = reducedMotion ? 0 : wf % 4;
  } else {
    role = arg1;
    x = arg2;
    y = Number(arg3);
    timeSec = typeof arg7 === 'number' && Number.isFinite(arg7) ? Math.max(0, arg7) : 0;
    if (typeof arg8 === 'boolean') {
      reducedMotion = arg8;
    }
    isMoving = !reducedMotion && Boolean(arg5);
    stepPhase = isMoving ? Math.floor(timeSec * 8) % 4 : 0;
  }

  const charSet = getCharacterSetForRole(role);
  const idleBreatheActive =
    !isMoving && !reducedMotion && Math.floor(timeSec / 0.9) % 2 === 1;

  let finalMatrix: string[];
  let flipX = false;
  let bobOffsetPx = 0;

  if (isSheltered && charSet.crouch && (role === 'traveler' || role === 'player')) {
    finalMatrix = charSet.crouch;
    flipX = facing === 'left';
  } else if (facing === 'up') {
    if (!isMoving) {
      finalMatrix = idleBreatheActive ? charSet.upBreathe : charSet.upIdle;
    } else if (stepPhase === 1) {
      finalMatrix = charSet.upWalk1;
      bobOffsetPx = -1;
    } else if (stepPhase === 3) {
      finalMatrix = charSet.upWalk2;
      bobOffsetPx = -1;
    } else {
      finalMatrix = charSet.upIdle;
    }
  } else if (facing === 'left' || facing === 'right') {
    flipX = facing === 'left';
    if (!isMoving) {
      finalMatrix = idleBreatheActive ? charSet.sideBreathe : charSet.sideIdle;
    } else if (stepPhase === 1) {
      finalMatrix = charSet.sideWalk1;
      bobOffsetPx = -1;
    } else if (stepPhase === 3) {
      finalMatrix = charSet.sideWalk2;
      bobOffsetPx = -1;
    } else {
      finalMatrix = charSet.sideIdle;
    }
  } else {
    if (!isMoving) {
      finalMatrix = idleBreatheActive ? charSet.downBreathe : charSet.downIdle;
    } else if (stepPhase === 1) {
      finalMatrix = charSet.downWalk1;
      bobOffsetPx = -1;
    } else if (stepPhase === 3) {
      finalMatrix = charSet.downWalk2;
      bobOffsetPx = -1;
    } else {
      finalMatrix = charSet.downIdle;
    }
  }

  // Bóng đổ tiếp đất hình elip chuẩn xác tại tâm chân (footX, footY)
  drawGroundedContactShadow(ctx, x, y);

  const cols = finalMatrix[0]?.length ?? CHARACTER_SPRITE_WIDTH;
  const drawX = Math.round(x - cols * 0.5);
  const drawY = Math.round(y - finalMatrix.length + bobOffsetPx);

  drawMatrixSprite(ctx, finalMatrix, drawX, drawY, 1, flipX);
}

export function drawTravelerSprite(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  facingRight: boolean,
  isMoving: boolean,
  isSheltered: boolean,
  timeSec: number
): void {
  drawCharacter4Dir(
    ctx,
    'traveler',
    x,
    y,
    facingRight ? 'right' : 'left',
    isMoving,
    isSheltered,
    timeSec
  );
}

export function drawNpcSprite(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  facingRight: boolean
): void {
  drawCharacter4Dir(
    ctx,
    'historian',
    x,
    y,
    facingRight ? 'right' : 'left',
    false,
    false,
    0,
    true
  );
}

export interface ZoneLandmarkSpec {
  id: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export const ZONE_LANDMARKS: Record<string, ZoneLandmarkSpec[]> = {
  harbor_square: [
    { id: 'lm_customs_clocktower', label: 'Tháp Đồng Hồ Hải Quan Cửu Long', x: 208, y: 56, width: 48, height: 80 },
    { id: 'lm_harbor_crane', label: 'Cần Cẩu Bốc Xếp Bến Cảng', x: 688, y: 512, width: 48, height: 64 },
    { id: 'lm_moored_sampans', label: 'Thuyền Tam Bản Neo Đậu Đêm', x: 240, y: 608, width: 96, height: 48 },
  ],
  warehouse_interior: [
    { id: 'lm_manifest_scales', label: 'Cân Hàng & Giá Kiểm Kê Xuất Nhập Khẩu', x: 112, y: 96, width: 64, height: 48 },
    { id: 'lm_tea_spice_bales', label: 'Kiện Trà & Hương Liệu Viễn Dương', x: 384, y: 96, width: 64, height: 48 },
  ],
  old_street: [
    { id: 'lm_canton_archway', label: 'Cổng Vòm Đá Phố Cổ Cửu Long', x: 64, y: 240, width: 48, height: 64 },
    { id: 'lm_shophouse_banners', label: 'Biển Hiệu Sơn Mài & Đèn Lồng Phố Cổ', x: 176, y: 64, width: 128, height: 64 },
    { id: 'lm_herbal_tea_pavilion', label: 'Quán Trà Đèn Lồng Đỏ Bà Năm', x: 168, y: 392, width: 96, height: 48 },
  ],
  small_pier: [
    { id: 'lm_pier_signal_mast', label: 'Cột Đèn Báo Hiệu Cảng Thơm', x: 128, y: 144, width: 32, height: 64 },
    { id: 'lm_night_ferry_sampan', label: 'Thuyền Đò Đêm Cửu Long', x: 352, y: 344, width: 88, height: 44 },
  ],
  safehouse_interior: [
    { id: 'lm_cipher_telegraph_wall', label: 'Bản Đồ Liên Lạc Bí Mật & Bàn Điện Báo', x: 176, y: 64, width: 144, height: 56 },
    { id: 'lm_printing_stencil_rack', label: 'Khung In Truyền Đơn Đá', x: 80, y: 144, width: 48, height: 64 },
  ],
  shadow_alley: [
    { id: 'lm_stone_overpass_arch', label: 'Vòm Đá Cổ Hẻm Tối', x: 112, y: 144, width: 64, height: 64 },
    { id: 'lm_colonial_watch_post', label: 'Trạm Gác Đèn Dầu Thực Dân', x: 512, y: 128, width: 48, height: 56 },
  ],
  archive_courtyard: [
    { id: 'lm_tam_quan_gate', label: 'Cổng Tam Quan Sân Lưu Trữ', x: 304, y: 56, width: 96, height: 88 },
    { id: 'lm_bronze_incense_urn', label: 'Đỉnh Đồng & Bia Đá Cổ', x: 328, y: 208, width: 48, height: 40 },
  ],
  secret_archive: [
    { id: 'lm_unification_conference_table', label: 'Bàn Hội Nghị Hợp Nhất 02/1930 & Cờ Đỏ Búa Liềm', x: 192, y: 72, width: 128, height: 72 },
    { id: 'lm_secret_dossier_vault', label: 'Tủ Hồ Sơ Sơn Mài Khóa Đồng', x: 64, y: 72, width: 80, height: 56 },
  ],
  tan_trao_banyan: [
    { id: 'lm_twin_banyan_tree', label: 'Cây Đa Tân Trào Cổ Thụ Đôi', x: 112, y: 136, width: 96, height: 88 },
    { id: 'lm_dinh_tan_trao', label: 'Mái Đình Tân Trào Lịch Sử 08/1945', x: 240, y: 56, width: 288, height: 96 },
  ],
  ba_dinh_square: [
    { id: 'lm_independence_tribune', label: 'Lễ Đài Độc Lập Ba Đình 02/09/1945', x: 304, y: 52, width: 224, height: 104 },
    { id: 'lm_ba_dinh_flag_avenue', label: 'Hàng Cờ Đỏ Sao Vàng Quảng Trường Ba Đình', x: 144, y: 184, width: 512, height: 48 },
  ],
  pha_din_pass: [
    { id: 'lm_pack_bicycle_convoy', label: 'Đoàn Xe Đạp Thồ Điện Biên Phủ', x: 160, y: 184, width: 128, height: 48 },
    { id: 'lm_pha_din_command_bunker', label: 'Hầm Chỉ Huy & Lưới Ngụy Trang Đèo Pha Đin', x: 224, y: 64, width: 320, height: 88 },
  ],
  spring_1975_corridor: [
    { id: 'lm_tank_column_1975', label: 'Đội Hình Xe Tăng Tiến Quân Mùa Xuân 1975', x: 144, y: 176, width: 144, height: 48 },
    { id: 'lm_ho_chi_minh_campaign_hq', label: 'Sở Chỉ Huy Chiến Dịch Hồ Chí Minh', x: 240, y: 64, width: 288, height: 88 },
  ],
  doi_moi_avenue: [
    { id: 'lm_doi_moi_arcade_hall', label: 'Phố Thương Mại & Bảng Tin Đổi Mới 1986', x: 208, y: 64, width: 352, height: 88 },
    { id: 'lm_integration_clock_plaza', label: 'Đài Phun Nước & Biểu Tượng Hội Nhập', x: 320, y: 176, width: 96, height: 48 },
  ],
  nexus_2045_plaza: [
    { id: 'lm_solar_lotus_arch_2045', label: 'Vòm Sen Năng Lượng Mặt Trời 2045', x: 224, y: 52, width: 320, height: 104 },
    { id: 'lm_memory_holo_pylons', label: 'Trụ Quang Học Lưu Trữ Ký Ức Quốc Gia', x: 288, y: 176, width: 160, height: 56 },
  ],
};

export function drawTopDownTile(
  ctx: CanvasRenderingContext2D,
  tileId: number,
  screenX: number,
  screenY: number,
  isInterior = false,
  eraId: EraId = '1930',
  timeSec = 0,
  reducedMotion = false
): void {
  if (tileId <= 0) return;
  const x = Math.round(screenX);
  const y = Math.round(screenY);
  const variant = ((Math.abs(x >> 4) * 13 + Math.abs(y >> 4) * 7) & 3);

  ctx.save();
  if (!isInterior) {
    switch (tileId) {
      case 1: {
        // Cobblestone courtyard / era ground base with deterministic micro-variations
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x + 1, y + 1, 6, 6);
        ctx.fillRect(x + 8, y + 1, 7, 6);
        ctx.fillRect(x + 1, y + 8, 7, 7);
        ctx.fillRect(x + 9, y + 8, 6, 7);
        ctx.fillStyle =
          eraId === '1945'
            ? PALETTE_12.BRONZE_MUTED
            : eraId === '1986_2045'
              ? PALETTE_12.INK_PRIMARY
              : PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 2 + (variant & 1), y + 2, 2, 1);
        ctx.fillRect(x + 10 - (variant & 1), y + 10, 2, 1);
        if (variant === 2) {
          ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
          ctx.fillRect(x + 5, y + 12, 2, 1);
        }
        break;
      }
      case 2: {
        // Main stone street / avenue with subtle slab variation
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 1, y + 1, 14, 6);
        ctx.fillRect(x + 1, y + 9, 14, 6);
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y + 7, 16, 1);
        ctx.fillRect(x, y + 15, 16, 1);
        ctx.fillRect(x + (variant % 2 === 0 ? 8 : 6), y, 1, 7);
        ctx.fillRect(x + (variant % 2 === 0 ? 4 : 10), y + 8, 1, 7);
        if (eraId === '1986_2045' && variant === 1) {
          ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
          ctx.fillRect(x + 7, y + 3, 2, 1);
        }
        break;
      }
      case 3: {
        // Wooden pier planking (BRONZE_MUTED horizontal planks with dark seams)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        for (let py = 0; py < 16; py += 4) {
          ctx.fillRect(x, y + py, 16, 1);
        }
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 2 + (variant & 1), y + 2, 4, 1);
        ctx.fillRect(x + 9 - (variant & 1), y + 6, 5, 1);
        ctx.fillRect(x + 3, y + 10, 5, 1);
        break;
      }
      case 4: {
        // Harbor dark water (LACQUER_VOID with animated wave ripples)
        const waveShift = reducedMotion
          ? 0
          : Math.round(Math.sin(timeSec * 2.4 + variant) * 1);
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 2 + waveShift, y + 4, 6, 1);
        ctx.fillRect(x + 7 - waveShift, y + 11, 6, 1);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4 + waveShift, y + 4, 2, 1);
        ctx.fillRect(x + 9 - waveShift, y + 11, 2, 1);
        break;
      }
      case 5: {
        // Doorway stone threshold (INK_SECONDARY with BRONZE_ANTIQUE border)
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 16, 2);
        ctx.fillRect(x, y + 14, 16, 2);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4, y + 6, 8, 4);
        break;
      }
      case 6: {
        // Ceremonial stone plaza (INK_PRIMARY with CRIMSON_SEAL & BRONZE_BRIGHT inlay)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 2, y + 2, 12, 12);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 4, y + 4, 8, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 7, y + 7, 2, 2);
        break;
      }
      case 7: {
        // Wooden shipping crate (BRONZE_MUTED & BRONZE_ANTIQUE crate with diagonal brace)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 1, y + 1, 14, 14);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 2, y + 2, 12, 12);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 4, y + 4, 8, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 2, y + 2, 12, 1);
        ctx.fillRect(x + 5, y + 7, 6, 2);
        break;
      }
      case 8: {
        // Street oil lantern / barrel prop with subtle flame flicker
        const flicker = !reducedMotion && Math.sin(timeSec * 5 + variant) > 0.3 ? 1 : 0;
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x + 7, y + 6, 2, 9);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 4, y + 2, 8, 5);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 6, y + 3, 4, 3);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(x + 7, y + 4 - flicker, 2, 1 + flicker);
        break;
      }
      case 9: {
        // Shopfront lacquer wall (LACQUER_DEEP wall with BRONZE_ANTIQUE timber frame & lit window)
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 16, 2);
        ctx.fillRect(x, y + 14, 16, 2);
        ctx.fillRect(x + 1, y + 2, 2, 12);
        ctx.fillRect(x + 13, y + 2, 2, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 5, y + 5, 6, 6);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 7, y + 5, 2, 6);
        break;
      }
      case 10: {
        // Warehouse brick & timber wall (LACQUER_ELEVATED & BRONZE_MUTED structural wall)
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x, y, 16, 2);
        ctx.fillRect(x, y + 7, 16, 2);
        ctx.fillRect(x, y + 14, 16, 2);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 2, y + 3, 5, 3);
        ctx.fillRect(x + 9, y + 3, 5, 3);
        ctx.fillRect(x + 5, y + 10, 6, 3);
        break;
      }
      case 11: {
        // Tree trunk base (BRONZE_MUTED & BRONZE_ANTIQUE cylindrical trunk with roots)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 5, y + 1, 6, 14);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 3, y + 12, 10, 3);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 7, y + 3, 2, 9);
        break;
      }
      case 12: {
        // Antique bronze & crimson roof tiles (BRONZE_ANTIQUE & CRIMSON_SEAL pitched roof overhang)
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        for (let py = 0; py < 14; py += 4) {
          ctx.fillRect(x, y + py, 16, 2);
        }
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x, y + 14, 16, 2);
        break;
      }
      case 13: {
        // Tree canopy foliage (INK_PRIMARY & BRONZE_MUTED layered leaves)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x + 1, y + 1, 14, 14);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 2, y + 2, 12, 11);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 4, y + 3, 8, 6);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 6, y + 4, 3, 2);
        break;
      }
      case 14: {
        // Stone curb & cobbled border strip (GroundDetails)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 16, 2);
        ctx.fillRect(x, y + 14, 16, 2);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 2, y + 4, 5, 8);
        ctx.fillRect(x + 9, y + 4, 5, 8);
        break;
      }
      case 15: {
        // Mooring bollard & coiled hemp rope (LowProps)
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x + 4, y + 10, 8, 4);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 5, y + 4, 6, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4, y + 3, 8, 2);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(x + 3, y + 8, 10, 2);
        break;
      }
      case 17: {
        // Tân Trào banyan root network & mossy stone slab (Era 1945)
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 1, y + 3, 14, 3);
        ctx.fillRect(x + 4, y + 1, 3, 14);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 2, y + 4, 10, 1);
        ctx.fillRect(x + 5, y + 8, 8, 3);
        break;
      }
      case 18: {
        // Ba Đình ceremonial crimson & gold parade carpet (Era 1945)
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x, y, 16, 2);
        ctx.fillRect(x, y + 14, 16, 2);
        ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(x + 3, y + 5, 10, 6);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(x + 7, y + 7, 2, 2);
        break;
      }
      case 19: {
        // Pha Đin & Spring 1975 supply track / sandbag / camouflage mesh (Era 1954_1975)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 1, y + 2, 6, 5);
        ctx.fillRect(x + 8, y + 2, 7, 5);
        ctx.fillRect(x + 2, y + 9, 7, 5);
        ctx.fillRect(x + 10, y + 9, 5, 5);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 2, y + 3, 4, 2);
        ctx.fillRect(x + 9, y + 3, 5, 2);
        break;
      }
      case 20: {
        // Đổi Mới & Nexus 2045 solar-glass & bronze inlay tile (Era 1986_2045)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 1, y + 1, 14, 14);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 2, y + 2, 12, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4, y + 4, 8, 2);
        ctx.fillRect(x + 4, y + 10, 8, 2);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(x + 7, y + 7, 2, 2);
        break;
      }
      default: {
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 16, 2);
        break;
      }
    }
  } else {
    switch (tileId) {
      case 1:
      case 21: {
        // Wooden warehouse floorboard (LACQUER_ELEVATED with subtle horizontal plank seams)
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        for (let py = 3; py < 16; py += 4) {
          ctx.fillRect(x, y + py, 16, 1);
        }
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 2 + (variant & 1), y + 1, 5, 1);
        ctx.fillRect(x + 9 - (variant & 1), y + 9, 5, 1);
        break;
      }
      case 2:
      case 22: {
        // Central stone aisle (INK_PRIMARY & INK_SECONDARY stone runner)
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 1, y + 1, 14, 6);
        ctx.fillRect(x + 1, y + 9, 14, 6);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 1, 16);
        ctx.fillRect(x + 15, y, 1, 16);
        break;
      }
      case 23: {
        // Archive rug / mat (CRIMSON_SEAL & BRONZE_ANTIQUE woven mat under ledger table)
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 1, y + 1, 14, 1);
        ctx.fillRect(x + 1, y + 14, 14, 1);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 3, y + 3, 10, 10);
        break;
      }
      case 3:
      case 25: {
        // Interior perimeter lacquer wall (LACQUER_VOID & BRONZE_ANTIQUE wainscot wall)
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y, 16, 3);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 1, y + 4, 14, 9);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x, y + 14, 16, 2);
        break;
      }
      case 4:
      case 27: {
        // Overhead wooden ceiling beam (BRONZE_MUTED & LACQUER_VOID rafter beam)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x, y + 2, 16, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x, y + 4, 16, 3);
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y + 2, 16, 1);
        ctx.fillRect(x, y + 13, 16, 1);
        break;
      }
      case 5:
      case 24: {
        // Wooden crate prop (LowProps)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 2, y + 2, 12, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 3, y + 3, 10, 10);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 2, y + 2, 12, 1);
        break;
      }
      case 26: {
        // Large cargo crate stack (BRONZE_MUTED & BRONZE_ANTIQUE crate with PARCHMENT_WARM shipping label)
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x + 1, y + 1, 14, 14);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 2, y + 2, 12, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 3, y + 3, 10, 4);
        ctx.fillRect(x + 3, y + 8, 10, 5);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(x + 6, y + 4, 4, 3);
        break;
      }
      case 6:
      case 28: {
        // Archive desk/ledger table
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(x + 1, y + 3, 14, 10);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(x + 4, y + 5, 8, 5);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 5, y + 6, 3, 2);
        break;
      }
      case 7:
      case 29: {
        // Bookshelf
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x + 1, y, 14, 16);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 2, y + 3, 3, 4);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(x + 6, y + 3, 3, 4);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 10, y + 3, 4, 4);
        break;
      }
      case 8:
      case 30: {
        // Exit door mat/threshold
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 2, y + 3, 12, 10);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4, y + 5, 8, 6);
        break;
      }
      case 31: {
        // Interior brass lamp & scroll rack
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 3, y + 4, 10, 10);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(x + 5, y + 5, 6, 4);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 7, y + 2, 2, 3);
        break;
      }
      default: {
        // Subtle floor tile fallback
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x, y, 16, 16);
        break;
      }
    }
  }
  ctx.restore();
}

/**
 * Vẽ các công trình biểu tượng (Focal Landmarks) đặc trưng cho từng khu vực trong 14 bản đồ,
 * sử dụng bảng màu 12 sắc độ Sơn Mài & Giấy Dó (`PALETTE_12`).
 */
export function drawZoneLandmarks(
  ctx: CanvasRenderingContext2D,
  mapData: NormalizedMapData,
  cameraX: number,
  cameraY: number,
  timeSec = 0,
  reducedMotion = false
): void {
  const specs = ZONE_LANDMARKS[mapData.id] ?? [];
  if (specs.length === 0) return;

  const pulse = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 3) * 1);

  ctx.save();
  for (const lm of specs) {
    const lx = Math.round(lm.x - cameraX);
    const ly = Math.round(lm.y - cameraY);

    switch (lm.id) {
      case 'lm_customs_clocktower': {
        // Tháp Đồng Hồ Hải Quan Cửu Long (1930)
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(lx, ly, 48, 80);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 4, ly + 8, 40, 68);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 2, ly, 44, 10);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 14, ly + 18, 20, 20);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(lx + 16, ly + 20, 16, 16);
        // Kim đồng hồ
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(lx + 23, ly + 22, 2, 7);
        ctx.fillRect(lx + 23, ly + 27, 5, 2);
        break;
      }
      case 'lm_harbor_crane': {
        // Cần Cẩu Bốc Xếp Bến Cảng
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(lx + 8, ly + 44, 32, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 18, ly + 8, 8, 40);
        ctx.fillRect(lx + 6, ly + 6, 36, 6);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 36, ly + 12, 2, 20 + pulse);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 32, ly + 32 + pulse, 10, 8);
        break;
      }
      case 'lm_moored_sampans': {
        // Thuyền Tam Bản Neo Đậu Đêm
        const bob = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 2.2) * 1);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 4, ly + 14 + bob, 40, 14);
        ctx.fillRect(lx + 52, ly + 18 - bob, 40, 14);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 12, ly + 8 + bob, 22, 8);
        ctx.fillRect(lx + 60, ly + 12 - bob, 22, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 36, ly + 12 + bob, 3, 3);
        ctx.fillRect(lx + 84, ly + 16 - bob, 3, 3);
        break;
      }
      case 'lm_manifest_scales':
      case 'lm_tea_spice_bales': {
        // Cân Hàng & Kiện Trà Kho Hàng Số 4
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 4, ly + 8, 56, 32);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 8, ly + 12, 22, 24);
        ctx.fillRect(lx + 34, ly + 12, 22, 24);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(lx + 12, ly + 16, 14, 8);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 40, ly + 16, 10, 8);
        break;
      }
      case 'lm_canton_archway': {
        // Cổng Vòm Đá Phố Cổ Cửu Long
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 2, ly + 8, 8, 52);
        ctx.fillRect(lx + 38, ly + 8, 8, 52);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx, ly, 48, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 10, ly + 3, 28, 6);
        break;
      }
      case 'lm_shophouse_banners': {
        // Biển Hiệu Sơn Mài & Đèn Lồng Phố Cổ
        for (let offset = 16; offset < 120; offset += 32) {
          ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
          ctx.fillRect(lx + offset, ly + 10, 10, 28);
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(lx + offset + 2, ly + 14, 6, 18);
        }
        break;
      }
      case 'lm_herbal_tea_pavilion': {
        // Quán Trà Đèn Lồng Đỏ Bà Năm
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 8, ly + 8, 80, 32);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 12, ly + 4, 72, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 20, ly + 16, 12, 8);
        ctx.fillRect(lx + 56, ly + 16, 12, 8);
        break;
      }
      case 'lm_pier_signal_mast': {
        // Cột Đèn Báo Hiệu Cảng Thơm
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 13, ly + 8, 6, 52);
        ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(lx + 9, ly + 12, 14, 10);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(lx + 13, ly + 15, 6, 4);
        break;
      }
      case 'lm_night_ferry_sampan': {
        // Thuyền Đò Đêm Cửu Long
        const bob = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 2.5) * 1);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 4, ly + 12 + bob, 80, 18);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 22, ly + 4 + bob, 36, 12);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 68, ly + 8 + bob, 5, 5);
        break;
      }
      case 'lm_cipher_telegraph_wall':
      case 'lm_printing_stencil_rack': {
        // Bản Đồ Liên Lạc Bí Mật & Bàn Điện Báo / Khung In
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 4, ly + 6, lm.width - 8, 36);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(lx + 10, ly + 10, lm.width - 20, 24);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 16, ly + 16, 12, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 34, ly + 18, 16, 4);
        break;
      }
      case 'lm_stone_overpass_arch':
      case 'lm_colonial_watch_post': {
        // Vòm Đá Cổ Hẻm Tối & Trạm Gác
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(lx + 2, ly + 4, lm.width - 4, lm.height - 8);
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(lx + 12, ly + 18, lm.width - 24, lm.height - 22);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 8, ly + 8, lm.width - 16, 3);
        break;
      }
      case 'lm_tam_quan_gate': {
        // Cổng Tam Quan Sân Lưu Trữ
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 4, ly + 4, 88, 18);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 12, ly + 8, 72, 4);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 10, ly + 22, 10, 56);
        ctx.fillRect(lx + 76, ly + 22, 10, 56);
        break;
      }
      case 'lm_bronze_incense_urn': {
        // Đỉnh Đồng & Bia Đá Cổ
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 12, ly + 10, 24, 22);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 16, ly + 6, 16, 6);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(lx + 22, ly + 2 - pulse, 4, 4);
        break;
      }
      case 'lm_unification_conference_table': {
        // Bàn Hội Nghị Hợp Nhất 02/1930 & Cờ Đỏ Búa Liềm
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 36, ly + 4, 56, 22);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 60, ly + 10, 8, 8);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 16, ly + 36, 96, 28);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(lx + 28, ly + 42, 20, 12);
        ctx.fillRect(lx + 76, ly + 42, 20, 12);
        break;
      }
      case 'lm_secret_dossier_vault': {
        // Tủ Hồ Sơ Sơn Mài Khóa Đồng
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(lx + 4, ly + 6, 72, 44);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 8, ly + 10, 30, 36);
        ctx.fillRect(lx + 42, ly + 10, 30, 36);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 34, ly + 24, 4, 6);
        ctx.fillRect(lx + 42, ly + 24, 4, 6);
        break;
      }
      case 'lm_twin_banyan_tree': {
        // Cây Đa Tân Trào Cổ Thụ Đôi (08/1945)
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(lx + 28, ly + 38, 16, 44);
        ctx.fillRect(lx + 52, ly + 42, 14, 40);
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(lx + 4, ly + 4, 88, 42);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        ctx.fillRect(lx + 14, ly + 10, 68, 28);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 26, ly + 16, 44, 12);
        break;
      }
      case 'lm_dinh_tan_trao': {
        // Mái Đình Tân Trào Lịch Sử 08/1945
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 8, ly + 8, 272, 32);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 4, ly + 36, 280, 6);
        ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
        for (let px = 28; px < 268; px += 44) {
          ctx.fillRect(lx + px, ly + 42, 10, 46);
        }
        break;
      }
      case 'lm_independence_tribune': {
        // Lễ Đài Độc Lập Ba Đình 02/09/1945
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(lx + 16, ly + 20, 192, 76);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 32, ly + 28, 160, 20);
        ctx.fillRect(lx + 88, ly + 4, 48, 88);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 106, ly, 4, 24);
        // Cờ đỏ sao vàng tung bay trên lễ đài
        ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(lx + 110, ly + 2, 28, 16);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(lx + 121, ly + 7, 6, 6);
        break;
      }
      case 'lm_ba_dinh_flag_avenue': {
        // Hàng Cờ Đỏ Sao Vàng Quảng Trường Ba Đình
        for (let px = 16; px < 496; px += 64) {
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(lx + px, ly + 8, 3, 36);
          ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
          ctx.fillRect(lx + px + 3, ly + 8 + pulse, 18, 11);
          ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
          ctx.fillRect(lx + px + 10, ly + 12 + pulse, 4, 4);
        }
        break;
      }
      case 'lm_pack_bicycle_convoy': {
        // Đoàn Xe Đạp Thồ Điện Biên Phủ (1954)
        for (let px = 12; px < 116; px += 36) {
          ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
          ctx.fillRect(lx + px, ly + 24, 8, 8);
          ctx.fillRect(lx + px + 16, ly + 24, 8, 8);
          ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
          ctx.fillRect(lx + px + 4, ly + 12, 16, 12);
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(lx + px + 8, ly + 8, 8, 4);
        }
        break;
      }
      case 'lm_pha_din_command_bunker': {
        // Hầm Chỉ Huy & Lưới Ngụy Trang Đèo Pha Đin
        ctx.fillStyle = PALETTE_12.INK_PRIMARY;
        ctx.fillRect(lx + 12, ly + 12, 296, 64);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        for (let px = 20; px < 292; px += 32) {
          ctx.fillRect(lx + px, ly + 18, 24, 12);
        }
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 140, ly + 36, 40, 24);
        break;
      }
      case 'lm_tank_column_1975': {
        // Đội Hình Xe Tăng Tiến Quân Mùa Xuân 1975
        for (let px = 12; px < 120; px += 56) {
          ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
          ctx.fillRect(lx + px, ly + 22, 36, 14);
          ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
          ctx.fillRect(lx + px + 6, ly + 12, 20, 10);
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(lx + px + 26, ly + 15, 16, 4);
          ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
          ctx.fillRect(lx + px + 12, ly + 4, 12, 8);
        }
        break;
      }
      case 'lm_ho_chi_minh_campaign_hq': {
        // Sở Chỉ Huy Chiến Dịch Hồ Chí Minh (1975)
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(lx + 8, ly + 8, 272, 72);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(lx + 24, ly + 14, 240, 14);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(lx + 96, ly + 34, 96, 36);
        ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(lx + 116, ly + 42, 56, 6);
        break;
      }
      case 'lm_doi_moi_arcade_hall': {
        // Phố Thương Mại & Bảng Tin Đổi Mới 1986
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(lx + 8, ly + 8, 336, 72);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 16, ly + 14, 320, 10);
        for (let px = 28; px < 320; px += 56) {
          ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
          ctx.fillRect(lx + px, ly + 32, 36, 36);
          ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
          ctx.fillRect(lx + px + 4, ly + 36, 28, 8);
        }
        break;
      }
      case 'lm_integration_clock_plaza': {
        // Đài Phun Nước & Biểu Tượng Hội Nhập
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(lx + 12, ly + 8, 72, 32);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 20, ly + 14, 56, 20);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(lx + 42, ly + 10 - pulse, 12, 16);
        break;
      }
      case 'lm_solar_lotus_arch_2045': {
        // Vòm Sen Năng Lượng Mặt Trời 2045
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(lx + 16, ly + 16, 288, 80);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(lx + 32, ly + 8, 256, 14);
        ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(lx + 96, ly + 26, 128, 16);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        for (let px = 48; px < 272; px += 44) {
          ctx.fillRect(lx + px, ly + 48, 24, 36);
        }
        break;
      }
      case 'lm_memory_holo_pylons': {
        // Trụ Quang Học Lưu Trữ Ký Ức Quốc Gia 2045
        for (let px = 16; px < 148; px += 44) {
          ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
          ctx.fillRect(lx + px, ly + 28, 20, 24);
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(lx + px + 4, ly + 8 - pulse, 12, 20);
          ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
          ctx.fillRect(lx + px + 7, ly + 12 - pulse, 6, 12);
        }
        break;
      }
    }
  }
  ctx.restore();
}

export function drawArtifactSprite(
  ctx: CanvasRenderingContext2D,
  type: MilestoneArtifact['spriteType'],
  x: number,
  y: number,
  isCollected: boolean,
  timeSec: number
): void {
  const bob = Math.round(Math.sin(timeSec * 3.5) * 2);
  const drawY = y + bob;

  ctx.save();
  // Hào quang Chiaroscuro quanh cổ vật
  ctx.fillStyle = isCollected
    ? 'rgba(158, 51, 33, 0.26)'
    : 'rgba(212, 165, 93, 0.28)';
  ctx.fillRect(Math.round(x - 8), Math.round(drawY - 8), 16, 16);

  switch (type) {
    case 'lantern': {
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(x - 3, drawY - 6, 6, 2);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 4, drawY - 4, 8, 7);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 2, drawY - 3, 4, 5);
      break;
    }
    case 'scroll': {
      ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
      ctx.fillRect(x - 5, drawY - 4, 10, 8);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 1, drawY - 2, 3, 4);
      ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(x - 6, drawY - 5, 2, 10);
      ctx.fillRect(x + 4, drawY - 5, 2, 10);
      break;
    }
    case 'seal': {
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 3, drawY - 6, 6, 4);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 5, drawY - 2, 10, 6);
      break;
    }
    case 'radio': {
      ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(x - 5, drawY - 4, 10, 8);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 3, drawY - 8, 1, 4);
      ctx.fillRect(x - 3, drawY - 2, 4, 3);
      ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
      ctx.fillRect(x + 2, drawY - 2, 2, 2);
      break;
    }
    case 'flag': {
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 5, drawY - 8, 1, 14);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 4, drawY - 8, 10, 7);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x, drawY - 6, 3, 3);
      break;
    }
    case 'bicycle': {
      // Xe đạp thồ Điện Biên
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(x - 6, drawY, 4, 4);
      ctx.fillRect(x + 2, drawY, 4, 4);
      ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
      ctx.fillRect(x - 4, drawY - 5, 8, 5);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 2, drawY - 6, 4, 2);
      break;
    }
    case 'tank': {
      // Xe tăng 390/843
      ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(x - 7, drawY, 14, 4);
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(x - 4, drawY - 4, 8, 4);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x + 4, drawY - 3, 5, 2);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 1, drawY - 7, 4, 3);
      break;
    }
    case 'metro': {
      // Tàu cao tốc Kỷ nguyên số 2045
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(x - 7, drawY - 4, 14, 6);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 7, drawY - 1, 14, 2);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 5, drawY - 3, 3, 2);
      ctx.fillRect(x + 1, drawY - 3, 3, 2);
      break;
    }
  }
  ctx.restore();
}

export type CuratedPropId =
  | 'prop_harbor_lantern'
  | 'prop_cargo_crate'
  | 'prop_tea_table'
  | 'prop_printing_press'
  | 'prop_archive_ledger'
  | 'prop_banyan_tree_canopy'
  | 'prop_conference_table'
  | 'prop_time_rift_gate';

const CURATED_PROP_SLOT_INDEX: Record<CuratedPropId, number> = {
  prop_harbor_lantern: 0,
  prop_cargo_crate: 1,
  prop_tea_table: 2,
  prop_printing_press: 3,
  prop_archive_ledger: 4,
  prop_banyan_tree_canopy: 5,
  prop_conference_table: 6,
  prop_time_rift_gate: 7,
};

let cachedPropAtlasCanvas: HTMLCanvasElement | null = null;

/**
 * Khởi tạo duy nhất 1 lần (Singleton) bộ đệm Offscreen Canvas chứa 8 tĩnh vật chuẩn hóa 16x16.
 * Tuân thủ tuyệt đối quy tắc hiệu năng SPRITE_ATLAS.md (không cấp phát mỗi frame).
 */
export function getCuratedPropAtlasCanvas(): HTMLCanvasElement {
  if (cachedPropAtlasCanvas) {
    return cachedPropAtlasCanvas;
  }

  const canvas =
    typeof document !== 'undefined'
      ? document.createElement('canvas')
      : ({ width: 128, height: 16 } as unknown as HTMLCanvasElement);
  canvas.width = 128;
  canvas.height = 16;

  const ctx = typeof canvas.getContext === 'function' ? canvas.getContext('2d') : null;
  if (ctx) {
    ctx.imageSmoothingEnabled = false;
    // Slot 0: Harbor lantern (x = 0)
    drawTopDownTile(ctx, 8, 0, 0, false);
    // Slot 1: Cargo crate (x = 16)
    drawTopDownTile(ctx, 7, 16, 0, false);
    // Slot 2: Tea table (x = 32)
    drawTopDownTile(ctx, 28, 32, 0, true);
    // Slot 3: Printing press (x = 48)
    drawTopDownTile(ctx, 26, 48, 0, true);
    // Slot 4: Archive ledger (x = 64)
    drawTopDownTile(ctx, 6, 64, 0, true);
    // Slot 5: Banyan tree canopy (x = 80)
    drawTopDownTile(ctx, 13, 80, 0, false);
    // Slot 6: Conference table (x = 96)
    drawTopDownTile(ctx, 23, 96, 0, true);
    // Slot 7: Time rift gate (x = 112)
    drawTopDownTile(ctx, 6, 112, 0, false);
  }

  cachedPropAtlasCanvas = canvas;
  return cachedPropAtlasCanvas;
}

export function drawCuratedProp(
  ctx: CanvasRenderingContext2D,
  propId: CuratedPropId,
  x: number,
  y: number
): void {
  const atlas = getCuratedPropAtlasCanvas();
  const slotIdx = CURATED_PROP_SLOT_INDEX[propId] ?? 0;
  const sx = slotIdx * 16;
  ctx.save();
  ctx.imageSmoothingEnabled = false;
  if (typeof ctx.drawImage === 'function' && typeof document !== 'undefined') {
    try {
      ctx.drawImage(atlas, sx, 0, 16, 16, Math.round(x), Math.round(y), 16, 16);
      ctx.restore();
      return;
    } catch {
      // Fallback dưới môi trường giả lập headless không hỗ trợ drawImage
    }
  }
  drawTopDownTile(ctx, slotIdx + 1, Math.round(x), Math.round(y), false);
  ctx.restore();
}

export type DialoguePortraitArchetype =
  | 'liaison'
  | 'worker'
  | 'watchman'
  | 'keeper'
  | 'vendor'
  | 'boatman'
  | 'cadre_1945'
  | 'soldier_1954'
  | 'commander_1975'
  | 'reform_1986'
  | 'archivist_2045'
  | 'artifact_seal';

/**
 * Xác định kiểu chân dung pixel-art (40×40) dựa trên tên người nói và vai trò.
 */
export function resolveDialoguePortraitArchetype(
  speaker: string,
  role = '',
  isArtifact = false
): DialoguePortraitArchetype {
  const s = `${speaker} ${role}`.toLowerCase();
  if (s.includes('bia sử liệu') || s.includes('cổ vật')) {
    return 'artifact_seal';
  }
  if (s.includes('2045') || s.includes('kỷ nguyên số') || s.includes('giám tuyển')) {
    return 'archivist_2045';
  }
  if (s.includes('đổi mới') || s.includes('1986') || s.includes('kinh tế')) {
    return 'reform_1986';
  }
  if (s.includes('1975') || s.includes('thông tin') || s.includes('hồ chí minh')) {
    return 'commander_1975';
  }
  if (s.includes('pha đin') || s.includes('hậu cần') || s.includes('dân công') || s.includes('1954')) {
    return 'soldier_1954';
  }
  if (
    s.includes('việt minh') ||
    s.includes('tân trào') ||
    s.includes('ba đình') ||
    s.includes('tự vệ') ||
    s.includes('giải phóng')
  ) {
    return 'cadre_1945';
  }
  if (s.includes('chèo đò') || s.includes('bến thuyền')) {
    return 'boatman';
  }
  if (s.includes('hàng nước') || s.includes('bà ')) {
    return 'vendor';
  }
  if (s.includes('gác đèn') || s.includes('hoa tiêu')) {
    return 'watchman';
  }
  if (s.includes('công nhân') || s.includes('bốc xếp')) {
    return 'worker';
  }
  if (s.includes('thủ kho') || s.includes('ông từ') || s.includes('ký lục')) {
    return 'keeper';
  }
  if (isArtifact) {
    return 'artifact_seal';
  }
  return 'liaison';
}

/**
 * Vẽ chân dung bán thân (Bust Portrait 40×40 logic -> 80×80 CSS px) phong cách Eastward Pixel RPG
 * với ánh sáng chiaroscuro ấm, trang phục đặc trưng từng thời kỳ và cử động mắt/miệng tinh tế.
 */
export function drawDialoguePortrait(
  ctx: CanvasRenderingContext2D,
  archetype: DialoguePortraitArchetype,
  timeSec = 0,
  isSpeaking = false,
  reducedMotion = false
): void {
  ctx.save();
  ctx.imageSmoothingEnabled = false;

  // 1. Nền Sơn Mài sâu + Hào quang Chiaroscuro sau lưng nhân vật
  ctx.fillStyle = PALETTE_12.LACQUER_VOID;
  ctx.fillRect(0, 0, 40, 40);
  ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
  ctx.fillRect(2, 2, 36, 36);

  // Vầng sáng ấm phía sau đầu
  ctx.fillStyle =
    archetype === 'archivist_2045'
      ? PALETTE_12.CRIMSON_SEAL
      : PALETTE_12.BRONZE_MUTED;
  ctx.fillRect(8, 6, 24, 24);
  ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
  ctx.fillRect(11, 9, 18, 18);

  if (archetype === 'artifact_seal') {
    // Ấn chương Cột mốc Lịch sử
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(9, 7, 22, 26);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(9, 7, 22, 2);
    ctx.fillRect(9, 31, 22, 2);
    ctx.fillRect(9, 7, 2, 26);
    ctx.fillRect(29, 7, 2, 26);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillRect(15, 13, 10, 14);
    ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
    ctx.fillRect(17, 16, 6, 8);
    ctx.restore();
    return;
  }

  const breathY = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 3.2) * 0.6);
  const blink = !reducedMotion && Math.floor(timeSec * 2.4) % 9 === 0;
  const mouthOpen =
    isSpeaking && !reducedMotion && Math.floor(timeSec * 9) % 2 === 0;

  // 2. Vai & Trang phục theo từng nguyên mẫu nhân vật (y: 26..38)
  const shoulderY = 26 + breathY;
  if (archetype === 'liaison') {
    // Áo măng-tô tối + Khăn quàng đỏ son + Huy hiệu đồng
    ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    ctx.fillRect(8, shoulderY + 1, 24, 12);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(14, shoulderY, 12, 6);
    ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
    ctx.fillRect(16, shoulderY + 1, 8, 4);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(24, shoulderY + 5, 2, 2);
  } else if (archetype === 'worker') {
    // Áo công nhân nâu sẫm + Khăn vắt vai trắng ngà
    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
    ctx.fillRect(9, shoulderY, 6, 11);
    ctx.fillStyle = PALETTE_12.INK_PRIMARY;
    ctx.fillRect(16, shoulderY + 2, 8, 10);
  } else if (archetype === 'watchman') {
    // Áo tơi đi mưa tối + Đèn bão đồng sáng bên vai phải
    ctx.fillStyle = PALETTE_12.INK_PRIMARY;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(8, shoulderY + 1, 24, 3);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(30, shoulderY - 3, 6, 8);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillRect(31, shoulderY - 1, 4, 4);
  } else if (archetype === 'keeper' || archetype === 'reform_1986') {
    // Áo sơ mi giấy dó + Ghi-lê tối + Cà vạt đỏ son
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    ctx.fillRect(7, shoulderY, 26, 13);
    ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
    ctx.fillRect(15, shoulderY, 10, 9);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(19, shoulderY + 2, 2, 8);
  } else if (archetype === 'vendor') {
    // Áo bà ba nâu đồng + Khăn choàng ấm
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(7, shoulderY, 26, 13);
    ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
    ctx.fillRect(15, shoulderY, 10, 5);
  } else if (archetype === 'boatman') {
    // Áo nâu sồng + Khăn rằn
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
    ctx.fillRect(13, shoulderY, 14, 4);
  } else if (archetype === 'cadre_1945') {
    // Quân phục Việt Minh nâu vàng + Khăn đỏ sao vàng
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(15, shoulderY, 10, 5);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(10, shoulderY + 4, 3, 3);
  } else if (archetype === 'soldier_1954' || archetype === 'commander_1975') {
    // Quân phục chiến dịch + Phù hiệu đỏ trên cổ áo
    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
    ctx.fillRect(13, shoulderY + 1, 3, 3);
    ctx.fillRect(24, shoulderY + 1, 3, 3);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(9, shoulderY + 3, 22, 2);
  } else if (archetype === 'archivist_2045') {
    // Áo khoác cổ cao 2045 viền mạch vàng đồng sáng
    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(6, shoulderY, 28, 13);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(8, shoulderY, 2, 13);
    ctx.fillRect(30, shoulderY, 2, 13);
    ctx.fillRect(14, shoulderY + 2, 12, 2);
    ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
    ctx.fillRect(18, shoulderY + 5, 4, 4);
  }

  // 3. Cổ & Khuôn mặt nhân vật (y: 12..26)
  const headY = 12 + breathY;
  ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
  ctx.fillRect(17, headY + 12, 6, 4); // Cổ

  // Khuôn mặt (sáng phía trái, bóng ấm phía phải theo phong cách chiaroscuro)
  ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
  ctx.fillRect(13, headY, 14, 13);
  ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
  ctx.fillRect(21, headY + 1, 6, 12);
  ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
  ctx.fillRect(12, headY + 3, 1, 6); // Tai trái
  ctx.fillRect(27, headY + 3, 1, 6); // Tai phải

  // 4. Đôi mắt, Lông mày, Kính & Biểu cảm miệng
  ctx.fillStyle = PALETTE_12.INK_PRIMARY;
  ctx.fillRect(15, headY + 3, 3, 1); // Lông mày trái
  ctx.fillRect(22, headY + 3, 3, 1); // Lông mày phải

  if (blink) {
    ctx.fillRect(15, headY + 5, 3, 1);
    ctx.fillRect(22, headY + 5, 3, 1);
  } else {
    ctx.fillRect(16, headY + 4, 2, 3);
    ctx.fillRect(22, headY + 4, 2, 3);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillRect(16, headY + 4, 1, 1);
    ctx.fillRect(22, headY + 4, 1, 1);
  }

  // Kính tròn cho Thủ Kho / Chuyên Gia 1986 / Kính quang học 2045
  if (archetype === 'keeper' || archetype === 'reform_1986') {
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(14, headY + 4, 1, 3);
    ctx.fillRect(18, headY + 4, 4, 1);
    ctx.fillRect(25, headY + 4, 1, 3);
  } else if (archetype === 'archivist_2045') {
    ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
    ctx.fillRect(14, headY + 4, 12, 2);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillRect(15, headY + 4, 3, 1);
  }

  // Miệng (cử động khi đang thoại) + Râu cho Lão Chèo Đò / Người Gác Đèn
  if (archetype === 'watchman' || archetype === 'boatman') {
    ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
    ctx.fillRect(15, headY + 10, 10, 4);
  }
  ctx.fillStyle = mouthOpen ? PALETTE_12.CRIMSON_SEAL : PALETTE_12.INK_PRIMARY;
  ctx.fillRect(18, headY + 9, 4, mouthOpen ? 2 : 1);

  // 5. Mũ / Tóc đặc trưng từng thời kỳ (y: 5..14)
  if (archetype === 'liaison') {
    // Mũ phớt Fedora 1930
    ctx.fillStyle = PALETTE_12.INK_PRIMARY;
    ctx.fillRect(12, headY - 5, 16, 5);
    ctx.fillRect(9, headY - 1, 22, 2);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(12, headY - 2, 16, 1);
  } else if (archetype === 'worker' || archetype === 'watchman') {
    // Mũ lưỡi trai công nhân / hoa tiêu bến cảng
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    ctx.fillRect(12, headY - 4, 16, 4);
    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(11, headY - 1, 19, 2);
  } else if (archetype === 'boatman') {
    // Nón lá truyền thống
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(17, headY - 6, 6, 2);
    ctx.fillRect(13, headY - 4, 14, 2);
    ctx.fillRect(8, headY - 2, 24, 3);
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(10, headY, 20, 1);
  } else if (archetype === 'vendor') {
    // Khăn vấn tóc truyền thống
    ctx.fillStyle = PALETTE_12.INK_PRIMARY;
    ctx.fillRect(12, headY - 4, 16, 5);
    ctx.fillRect(11, headY, 2, 6);
    ctx.fillRect(27, headY, 2, 6);
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(13, headY - 2, 14, 2);
  } else if (archetype === 'cadre_1945') {
    // Mũ ca-lô Việt Minh gắn sao vàng
    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(12, headY - 5, 16, 5);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(18, headY - 4, 4, 3);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(19, headY - 3, 2, 2);
  } else if (archetype === 'soldier_1954' || archetype === 'commander_1975') {
    // Mũ cối bộ đội Cụ Hồ gắn sao vàng
    ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.fillRect(11, headY - 5, 18, 4);
    ctx.fillRect(9, headY - 1, 22, 2);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(18, headY - 4, 4, 3);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(19, headY - 3, 2, 2);
  } else {
    // Mái tóc gọn gàng của Học giả / Chuyên gia / Giám tuyển 2045
    ctx.fillStyle = PALETTE_12.INK_PRIMARY;
    ctx.fillRect(12, headY - 4, 16, 5);
    ctx.fillRect(12, headY, 2, 4);
    ctx.fillRect(26, headY, 2, 4);
    if (archetype === 'archivist_2045') {
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(14, headY - 3, 6, 1);
    }
  }

  // 6. Khung viền đồng sáng nội bộ 40×40
  ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
  ctx.fillRect(0, 0, 40, 1);
  ctx.fillRect(0, 39, 40, 1);
  ctx.fillRect(0, 0, 1, 40);
  ctx.fillRect(39, 0, 1, 40);

  ctx.restore();
}

