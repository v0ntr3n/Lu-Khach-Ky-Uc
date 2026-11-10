/**
 * @file src/engine/SpriteAtlas.ts
 * @description Bộ dựng ma trận điểm ảnh 16-bit (Procedural Pixel Art Sprites & Top-Down Tiles)
 * tuân thủ tuyệt đối Bảng Màu 12 Sắc Độ Sơn Mài & Giấy Dó (DESIGN.md & SPRITE_ATLAS.md).
 * Nhân vật chính & NPC được thiết kế ở kích thước chuẩn 16×22 (hoặc 16×21) điểm ảnh với
 * phom dáng (silhouette) tách biệt rõ nét theo từng vai trò lịch sử.
 */

import { PALETTE_12 } from '../data/chronicles';
import type { Facing4, MilestoneArtifact } from '../types/game';

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

export function drawTopDownTile(
  ctx: CanvasRenderingContext2D,
  tileId: number,
  screenX: number,
  screenY: number,
  isInterior = false
): void {
  if (tileId <= 0) return;
  const x = Math.round(screenX);
  const y = Math.round(screenY);

  ctx.save();
  if (!isInterior) {
    switch (tileId) {
      case 1: {
        // Cobblestone courtyard pavement (LACQUER_DEEP base with subtle stone mortar)
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x + 1, y + 1, 6, 6);
        ctx.fillRect(x + 8, y + 1, 7, 6);
        ctx.fillRect(x + 1, y + 8, 7, 7);
        ctx.fillRect(x + 9, y + 8, 6, 7);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 2, y + 2, 2, 1);
        ctx.fillRect(x + 10, y + 10, 2, 1);
        break;
      }
      case 2: {
        // Main stone street (LACQUER_ELEVATED with warm stone slab lines)
        ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.INK_SECONDARY;
        ctx.fillRect(x + 1, y + 1, 14, 6);
        ctx.fillRect(x + 1, y + 9, 14, 6);
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y + 7, 16, 1);
        ctx.fillRect(x, y + 15, 16, 1);
        ctx.fillRect(x + 8, y, 1, 7);
        ctx.fillRect(x + 4, y + 8, 1, 7);
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
        ctx.fillRect(x + 2, y + 2, 4, 1);
        ctx.fillRect(x + 9, y + 6, 5, 1);
        ctx.fillRect(x + 3, y + 10, 5, 1);
        break;
      }
      case 4: {
        // Harbor dark water (LACQUER_VOID with BRONZE_BRIGHT / BRONZE_MUTED wave ripples)
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x, y, 16, 16);
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(x + 2, y + 4, 6, 1);
        ctx.fillRect(x + 7, y + 11, 6, 1);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 4, y + 4, 2, 1);
        ctx.fillRect(x + 9, y + 11, 2, 1);
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
        // Street oil lantern / barrel prop
        ctx.fillStyle = PALETTE_12.LACQUER_VOID;
        ctx.fillRect(x + 7, y + 6, 2, 9);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(x + 4, y + 2, 8, 5);
        ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(x + 6, y + 3, 4, 3);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillRect(x + 7, y + 4, 2, 1);
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
        ctx.fillRect(x + 2, y + 1, 5, 1);
        ctx.fillRect(x + 9, y + 9, 5, 1);
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
