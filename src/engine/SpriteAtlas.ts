/**
 * @file src/engine/SpriteAtlas.ts
 * @description Bộ dựng ma trận điểm ảnh 16-bit (Procedural Pixel Art Sprites & Top-Down Tiles)
 * tuân thủ tuyệt đối Bảng Màu 12 Sắc Độ Sơn Mài & Giấy Dó (DESIGN.md & SPRITE_ATLAS.md).
 */

import { PALETTE_12 } from '../data/chronicles';
import type { Facing4, MilestoneArtifact } from '../types/game';

export type CharacterAtlasRole = 'traveler' | 'courier' | 'archivist' | 'observer' | 'historian';

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

/**
 * Sprite Lữ Khách Ký Ức 2045 (12x18 pixel matrix)
 * Đội mũ/kính lưu trữ viên 2045, khăn quàng đỏ son, áo choàng lữ hành đồng cổ
 */
const TRAVELER_IDLE: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPPK..',
  '..KPWPPWPK..',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  '.KBBGKKGBBK.',
  '.KBBBBBBBBK.',
  '.KBBBBBBBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '..KDD..DDK..',
  '..KDD..DDK..',
  '..KMM..MMK..',
  '..KKK..KKK..',
];

const TRAVELER_WALK_1: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPPK..',
  '..KPWPPWPK..',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  'KBBBGKKGBBBK',
  '.KBBBBBBBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '.KDD....DDK.',
  '.KDD.....DDK',
  '.KMM.....MMK',
  '.KKK.....KKK',
];

const TRAVELER_WALK_2: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPPK..',
  '..KPWPPWPK..',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  'KBBGKKKKGBBK',
  '.KBBBBBBBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '.KDD....DDK.',
  'KDD.....DDK.',
  'KMM.....MMK.',
  'KKK.....KKK.',
];

const TRAVELER_UP_IDLE: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KBBBBBBK..',
  '..KGGGGGGK..',
  '..KBBBBBBK..',
  '...KRRRRK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  '.KBBBBBBBBK.',
  '.KBBMMMMBBK.',
  '.KBBBBBBBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '..KDD..DDK..',
  '..KDD..DDK..',
  '..KMM..MMK..',
  '..KKK..KKK..',
];

const TRAVELER_UP_WALK_1: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KBBBBBBK..',
  '..KGGGGGGK..',
  '..KBBBBBBK..',
  '...KRRRRK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  'KBBBBBBBBBBK',
  '.KBBMMMMBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '.KDD....DDK.',
  '.KMM.....DDK',
  '.KKK.....MMK',
  '.........KKK',
];

const TRAVELER_UP_WALK_2: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KBBBBBBK..',
  '..KGGGGGGK..',
  '..KBBBBBBK..',
  '...KRRRRK...',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  'KBBBBBBBBBBK',
  '.KBBMMMMBBK.',
  '..KBBBBBBK..',
  '..KDDDDDDK..',
  '.KDD....DDK.',
  'KDD.....MMK.',
  'KMM.....KKK.',
  'KKK.........',
];

const TRAVELER_SIDE_IDLE: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPK...',
  '..KPPPPWK...',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '..KBBBBBBK..',
  '..KBBGGBBK..',
  '..KBBBBBBK..',
  '..KBBBBBBK..',
  '...KBBBBK...',
  '...KDDDDK...',
  '...KDDDDK...',
  '...KDDDDK...',
  '...KMMMMK...',
  '...KKKKKK...',
];

const TRAVELER_SIDE_WALK_1: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPK...',
  '..KPPPPWK...',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '..KBBBBBBK..',
  '.KBBBGGBBBK.',
  '..KBBBBBBK..',
  '...KBBBBK...',
  '...KDDDDK...',
  '..KDD..DDK..',
  '.KDD....DDK.',
  '.KMM....MMK.',
  '.KKK....KKK.',
];

const TRAVELER_SIDE_WALK_2: string[] = [
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPK...',
  '..KPPPPWK...',
  '...KPPPPK...',
  '..KRRRRRRK..',
  '..KBBBBBBK..',
  '..KBBGGBBK..',
  '..KBBBBBBK..',
  '...KBBBBK...',
  '...KDDDDK...',
  '..KDD..DDK..',
  '..KDD...DDK.',
  '..KMM...MMK.',
  '..KKK...KKK.',
];

const TRAVELER_CROUCH: string[] = [
  '............',
  '............',
  '............',
  '....KKKK....',
  '...KBBBBK...',
  '..KGGGGGGK..',
  '..KPPPPPPK..',
  '..KRRRRRRK..',
  '.KBBBBBBBBK.',
  'KBBBBBBBBBBK',
  'KBBBBBBBBBBK',
  '.KDDDDDDDDK.',
  '..KMM..MMK..',
  '..KKK..KKK..',
];

const NPC_HISTORIAN: string[] = [
  '....KKKK....',
  '...KMMMMK...',
  '..KMMMMMMK..',
  '..KPPPPPPK..',
  '..KPKPPKPK..',
  '...KPPPPK...',
  '..KGGGGGGK..',
  '.KDDDDDDDDK.',
  '.KDDGKKGDDK.',
  '.KDDDDDDDDK.',
  '..KDDDDDDK..',
  '..KDDDDDDK..',
  '..KMM..MMK..',
  '..KMM..MMK..',
  '..KKK..KKK..',
];

interface RolePaletteRemap {
  hat: string;
  trim: string;
  scarf: string;
  coat: string;
}

const ROLE_REMAP: Record<CharacterAtlasRole, RolePaletteRemap | null> = {
  traveler: null,
  courier: { hat: 'D', trim: 'V', scarf: 'R', coat: 'S' },
  archivist: { hat: 'M', trim: 'G', scarf: 'P', coat: 'D' },
  observer: { hat: 'B', trim: 'M', scarf: 'G', coat: 'M' },
  historian: { hat: 'M', trim: 'G', scarf: 'G', coat: 'D' },
};

function remapMatrixForRole(matrix: string[], role: CharacterAtlasRole): string[] {
  const remap = ROLE_REMAP[role];
  if (!remap) return matrix;
  return matrix.map((row, idx) => {
    let out = '';
    for (let c = 0; c < row.length; c++) {
      const ch = row[c];
      if (idx <= 2 && ch === 'B') out += remap.hat;
      else if (idx <= 3 && ch === 'G') out += remap.trim;
      else if (ch === 'R') out += remap.scarf;
      else if (ch === 'B') out += remap.coat;
      else out += ch;
    }
    return out;
  });
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
  role: CharacterAtlasRole,
  x: number,
  y: number,
  facing: Facing4,
  isMoving: boolean,
  isSheltered: boolean,
  timeSec: number
): void {
  const stepPhase = Math.floor(timeSec * 8) % 4;
  let baseMatrix: string[];
  let flipX = false;

  if (isSheltered && role === 'traveler') {
    baseMatrix = TRAVELER_CROUCH;
    flipX = facing === 'left';
  } else if (facing === 'up') {
    baseMatrix = !isMoving
      ? TRAVELER_UP_IDLE
      : stepPhase === 1
        ? TRAVELER_UP_WALK_1
        : stepPhase === 3
          ? TRAVELER_UP_WALK_2
          : TRAVELER_UP_IDLE;
  } else if (facing === 'left' || facing === 'right') {
    flipX = facing === 'left';
    baseMatrix = !isMoving
      ? TRAVELER_SIDE_IDLE
      : stepPhase === 1
        ? TRAVELER_SIDE_WALK_1
        : stepPhase === 3
          ? TRAVELER_SIDE_WALK_2
          : TRAVELER_SIDE_IDLE;
  } else {
    baseMatrix = !isMoving
      ? TRAVELER_IDLE
      : stepPhase === 1
        ? TRAVELER_WALK_1
        : stepPhase === 3
          ? TRAVELER_WALK_2
          : TRAVELER_IDLE;
  }

  const finalMatrix = remapMatrixForRole(baseMatrix, role);

  // Bóng đổ tiếp đất dưới chân nhân vật
  ctx.save();
  ctx.fillStyle = 'rgba(17, 14, 11, 0.55)';
  ctx.fillRect(Math.round(x - 5), Math.round(y - 2), 10, 3);
  ctx.restore();

  drawMatrixSprite(ctx, finalMatrix, x - 6, y - finalMatrix.length, 1, flipX);
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
  const frame = isSheltered
    ? TRAVELER_CROUCH
    : isMoving && Math.floor(timeSec * 8) % 2 === 1
      ? TRAVELER_WALK_1
      : TRAVELER_IDLE;

  drawMatrixSprite(ctx, frame, x - 6, y - frame.length, 1, !facingRight);
}

export function drawNpcSprite(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  facingRight: boolean
): void {
  drawMatrixSprite(ctx, NPC_HISTORIAN, x - 6, y - NPC_HISTORIAN.length, 1, !facingRight);
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
