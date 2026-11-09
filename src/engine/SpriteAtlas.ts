/**
 * @file src/engine/SpriteAtlas.ts
 * @description Bộ dựng ma trận điểm ảnh 16-bit (Procedural Pixel Art Sprites)
 * tuân thủ tuyệt đối Bảng Màu 12 Sắc Độ Sơn Mài & Giấy Dó (DESIGN.md).
 */

import { PALETTE_12 } from '../data/chronicles';
import type { MilestoneArtifact } from '../types/game';

const COLOR_MAP: Record<string, string> = {
  '.': '',
  'K': PALETTE_12.LACQUER_VOID,      // #110E0B - Viền tối Sơn Mài
  'D': PALETTE_12.LACQUER_ELEVATED,  // #241D17 - Áo tối
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
