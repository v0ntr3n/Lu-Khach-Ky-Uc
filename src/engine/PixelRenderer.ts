/**
 * @file src/engine/PixelRenderer.ts
 * @description Bộ dựng hình Canvas 2D 320x180 Integer-Scaled (HD-2D Chiaroscuro Sơn Mài & Giấy Dó).
 * Hỗ trợ:
 * - 4 lớp Parallax theo từng Chương (1930, 1945, 1954-1975, 1986-2045)
 * - Ánh sáng Chiaroscuro hướng tâm & đèn rọi tuần tra Mức 1 (Story-Driven)
 * - 4 Đại Cảnh Tương Tác Bùng Nổ (Climax Set-Pieces: Đuốc 1930, Cờ Ba Đình 1945, Xe tăng 1975, Tàu cao tốc 2045)
 */

import { PALETTE_12 } from '../data/chronicles';
import type { ChapterData, MilestoneArtifact, RomanMilestoneId } from '../types/game';
import { drawArtifactSprite, drawNpcSprite, drawTravelerSprite } from './SpriteAtlas';

export const INTERNAL_WIDTH = 320;
export const INTERNAL_HEIGHT = 180;
export const GROUND_Y = 148;

export interface RenderState {
  chapter: ChapterData;
  playerX: number;
  cameraX: number;
  facingRight: boolean;
  isMoving: boolean;
  isSheltered: boolean;
  collectedMilestones: Set<RomanMilestoneId>;
  nearbyMilestone: MilestoneArtifact | null;
  nearClimaxGate: boolean;
  climaxBurstProgress: number; // 0.0 -> 1.0 khi kích hoạt Bước B Đại Cảnh
  timeSec: number;
}

export class PixelRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.canvas.width = INTERNAL_WIDTH;
    this.canvas.height = INTERNAL_HEIGHT;
    const context = this.canvas.getContext('2d');
    if (!context) {
      throw new Error('Không thể khởi tạo Canvas 2D cho PixelRenderer.');
    }
    this.ctx = context;
    this.ctx.imageSmoothingEnabled = false;
  }

  public render(state: RenderState): void {
    const { ctx } = this;
    const {
      chapter,
      playerX,
      cameraX,
      facingRight,
      isMoving,
      isSheltered,
      collectedMilestones,
      nearbyMilestone,
      nearClimaxGate,
      climaxBurstProgress,
      timeSec,
    } = state;

    ctx.imageSmoothingEnabled = false;

    // 1. Nền đêm Sơn Mài & Hào quang Chiaroscuro hướng tâm
    this.drawSkyAndChiaroscuro(chapter, climaxBurstProgress, timeSec);

    // 2. Lớp kiến trúc nền xa (Parallax 0.25x)
    this.drawDistantParallax(chapter, cameraX, collectedMilestones, climaxBurstProgress, timeSec);

    // 3. Lớp kiến trúc trung cảnh & đường dây/cầu (Parallax 0.55x)
    this.drawMidgroundParallax(chapter, cameraX, collectedMilestones, climaxBurstProgress, timeSec);

    // 4. Mặt đất & Hiên nhà / Bóng đa / Giao thông hào ẩn nấp (Parallax 1.0x)
    this.drawGroundAndShelters(chapter, cameraX, playerX);

    // 5. Chùm đèn tuần tra / Pháo sáng (Mức 1 — chỉ đẩy lùi nhẹ nếu không ẩn nấp)
    if (climaxBurstProgress <= 0.05) {
      this.drawHazardSweeps(chapter, cameraX, timeSec);
    }

    // 6. Cột mốc La Mã (I - XII) & NPC Nhân chứng Lịch sử
    this.drawMilestonesAndNpcs(chapter, cameraX, collectedMilestones, nearbyMilestone, timeSec);

    // 7. Cổng Thời Gian Cuối Màn (Climax Gate)
    this.drawClimaxGate(chapter, cameraX, nearClimaxGate, climaxBurstProgress, timeSec);

    // 8. Nhân vật Lữ Khách Ký Ức 2045
    const screenPlayerX = Math.round(playerX - cameraX);
    drawTravelerSprite(
      ctx,
      screenPlayerX,
      GROUND_Y,
      facingRight,
      isMoving,
      isSheltered,
      timeSec
    );

    // 9. Đại Cảnh Bùng Nổ Cuối Chương (Climax Step B Set-Piece)
    if (climaxBurstProgress > 0) {
      this.drawClimaxSetPiece(chapter, climaxBurstProgress, timeSec);
    }

    // 10. Thời tiết & Hạt bụi điểm ảnh Sơn Mài (Rain / Autumn / Embers / Digital Stars)
    this.drawWeatherParticles(chapter, timeSec);
  }

  private drawSkyAndChiaroscuro(
    chapter: ChapterData,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(0, 0, INTERNAL_WIDTH, INTERNAL_HEIGHT);

    // Vầng sáng Chiaroscuro tâm khung hình
    const grad = ctx.createRadialGradient(
      INTERNAL_WIDTH * 0.5,
      INTERNAL_HEIGHT * 0.42,
      8,
      INTERNAL_WIDTH * 0.5,
      INTERNAL_HEIGHT * 0.42,
      155 + burst * 60
    );

    if (burst > 0.1) {
      grad.addColorStop(0, 'rgba(212, 165, 93, 0.48)');
      grad.addColorStop(0.5, 'rgba(158, 51, 33, 0.25)');
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    } else if (chapter.id === '1945') {
      grad.addColorStop(0, 'rgba(212, 165, 93, 0.26)');
      grad.addColorStop(0.6, 'rgba(158, 51, 33, 0.12)');
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    } else {
      grad.addColorStop(0, 'rgba(212, 165, 93, 0.20)');
      grad.addColorStop(0.65, 'rgba(36, 29, 23, 0.15)');
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    }

    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, INTERNAL_WIDTH, INTERNAL_HEIGHT);

    // Vòng cầu thiên thể Da Vinci / Trăng cổ điển góc trên
    ctx.strokeStyle = 'rgba(212, 165, 93, 0.16)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(260, 38, 22 + Math.sin(timeSec) * 1.5, 0, Math.PI * 2);
    ctx.stroke();
  }

  private drawDistantParallax(
    chapter: ChapterData,
    cameraX: number,
    collected: Set<RomanMilestoneId>,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const offset = Math.floor(cameraX * 0.22);

    ctx.fillStyle = PALETTE_12.LACQUER_DEEP;

    if (chapter.id === '1930') {
      // Mái nhà cổ & bến cảng Cửu Long Hương Cảng
      for (let i = 0; i < 12; i++) {
        const bx = i * 42 - (offset % 42);
        const bh = 36 + ((i * 17) % 28);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(bx, GROUND_Y - bh, 34, bh);
        // Cửa sổ đèn dầu vàng đồng
        if (i % 2 === 0) {
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(bx + 8, GROUND_Y - bh + 10, 4, 5);
          ctx.fillRect(bx + 18, GROUND_Y - bh + 18, 4, 5);
        }
      }
    } else if (chapter.id === '1945') {
      // Dãy núi Việt Bắc (Pác Bó - Tân Trào) nối về Quảng trường Ba Đình
      for (let i = 0; i < 9; i++) {
        const mx = i * 56 - (offset % 56);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.beginPath();
        ctx.moveTo(mx - 20, GROUND_Y);
        ctx.lineTo(mx + 22, GROUND_Y - 58 - (i % 3) * 12);
        ctx.lineTo(mx + 64, GROUND_Y);
        ctx.fill();
      }
    } else if (chapter.id === '1954_1975') {
      // Đèo Pha Đin & Lòng chảo Điện Biên
      for (let i = 0; i < 10; i++) {
        const mx = i * 52 - (offset % 52);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.beginPath();
        ctx.moveTo(mx - 15, GROUND_Y);
        ctx.lineTo(mx + 20, GROUND_Y - 65 - (i % 2) * 16);
        ctx.lineTo(mx + 55, GROUND_Y);
        ctx.fill();
      }
    } else {
      // Chương IV (1986 -> 2045): Kiến trúc đô thị tự động mọc cao lên khi thu thập mốc X, XI, XII!
      const unlockedCount =
        (collected.has('X') ? 1 : 0) +
        (collected.has('XI') ? 1 : 0) +
        (collected.has('XII') ? 1 : 0) +
        (burst > 0 ? 1 : 0);

      for (let i = 0; i < 12; i++) {
        const bx = i * 38 - (offset % 38);
        const baseH = 26 + ((i * 13) % 18);
        const modernBonus = unlockedCount * (10 + (i % 3) * 6);
        const totalH = Math.min(112, baseH + modernBonus);

        ctx.fillStyle = unlockedCount >= 2 ? PALETTE_12.LACQUER_ELEVATED : PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(bx, GROUND_Y - totalH, 28, totalH);

        // Đèn tòa nhà thông minh 2045
        if (unlockedCount >= 1) {
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          for (let wy = GROUND_Y - totalH + 6; wy < GROUND_Y - 12; wy += 8) {
            ctx.fillRect(bx + 5, wy, 3, 4);
            ctx.fillRect(bx + 16, wy, 3, 4);
          }
        }
        // Đỉnh tháp công nghệ số 2045
        if (unlockedCount >= 3 && i % 3 === 0) {
          ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
          ctx.fillRect(bx + 12, GROUND_Y - totalH - 8, 2, 8);
          if (Math.floor(timeSec * 4) % 2 === 0) {
            ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
            ctx.fillRect(bx + 12, GROUND_Y - totalH - 10, 2, 2);
          }
        }
      }
    }
  }

  private drawMidgroundParallax(
    chapter: ChapterData,
    cameraX: number,
    collected: Set<RomanMilestoneId>,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const offset = Math.floor(cameraX * 0.55);

    if (chapter.id === '1945') {
      // Đường dây truyền tin chiến khu Tân Trào
      ctx.strokeStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let x = 0; x <= INTERNAL_WIDTH; x += 16) {
        const sag = Math.sin((x + offset) * 0.05) * 4;
        if (x === 0) ctx.moveTo(x, 68 + sag);
        else ctx.lineTo(x, 68 + sag);
      }
      ctx.stroke();

      // Xung tín hiệu điện報 chạy dọc dây
      const pulseX = Math.floor((timeSec * 90) % INTERNAL_WIDTH);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(pulseX, 66, 4, 3);
    } else if (chapter.id === '1986_2045') {
      // Cầu cạn Metro 2045 khi đã mở khóa mốc XI hoặc XII
      if (collected.has('XI') || collected.has('XII') || burst > 0) {
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(0, 88, INTERNAL_WIDTH, 4);
        for (let px = 20 - (offset % 60); px < INTERNAL_WIDTH; px += 60) {
          ctx.fillRect(px, 92, 5, GROUND_Y - 92);
        }
        // Đoàn tàu điện lướt trên cầu cạn
        const trainX = Math.floor(((timeSec * 55) % (INTERNAL_WIDTH + 90)) - 45);
        ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
        ctx.fillRect(trainX, 81, 42, 7);
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(trainX, 85, 42, 2);
      }
    }
  }

  private drawGroundAndShelters(
    chapter: ChapterData,
    cameraX: number,
    playerX: number
  ): void {
    const { ctx } = this;

    // Mặt đường Sơn Mài & gạch cổ
    ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
    ctx.fillRect(0, GROUND_Y, INTERNAL_WIDTH, INTERNAL_HEIGHT - GROUND_Y);

    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(0, GROUND_Y, INTERNAL_WIDTH, 2);

    const tileOffset = Math.floor(cameraX) % 16;
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    for (let x = -tileOffset; x < INTERNAL_WIDTH; x += 16) {
      ctx.fillRect(x, GROUND_Y + 6, 10, 2);
      ctx.fillRect(x + 6, GROUND_Y + 14, 10, 2);
    }

    // Vẽ các Vùng Ẩn Nấp (Mái hiên Cửu Long / Bóng đa Tân Trào / Giao thông hào Điện Biên)
    for (const shelter of chapter.shelters) {
      const sx = Math.round(shelter.xStart - cameraX);
      const sw = Math.round(shelter.xEnd - shelter.xStart);
      if (sx + sw < -20 || sx > INTERNAL_WIDTH + 20) continue;

      const playerInside = playerX >= shelter.xStart && playerX <= shelter.xEnd;

      // Khung mái hiên / công sự
      ctx.fillStyle = playerInside
        ? 'rgba(212, 165, 93, 0.18)'
        : 'rgba(36, 29, 23, 0.72)';
      ctx.fillRect(sx, GROUND_Y - 32, sw, 32);

      // Mái ngói cổ hoặc tán đa
      ctx.fillStyle = playerInside ? PALETTE_12.BRONZE_BRIGHT : PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(sx - 3, GROUND_Y - 34, sw + 6, 4);
      ctx.fillRect(sx + 2, GROUND_Y - 32, 2, 32);
      ctx.fillRect(sx + sw - 4, GROUND_Y - 32, 2, 32);
    }
  }

  private drawHazardSweeps(
    chapter: ChapterData,
    cameraX: number,
    timeSec: number
  ): void {
    const { ctx } = this;

    for (const h of chapter.hazards) {
      const sweepX = h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
      const screenX = Math.round(sweepX - cameraX);
      if (screenX < -50 || screenX > INTERNAL_WIDTH + 50) continue;

      ctx.save();
      ctx.beginPath();
      ctx.moveTo(screenX, 16);
      ctx.lineTo(screenX - 24, GROUND_Y);
      ctx.lineTo(screenX + 24, GROUND_Y);
      ctx.closePath();

      const grad = ctx.createLinearGradient(screenX, 16, screenX, GROUND_Y);
      grad.addColorStop(0, 'rgba(223, 107, 87, 0.42)');
      grad.addColorStop(1, 'rgba(212, 165, 93, 0.14)');
      ctx.fillStyle = grad;
      ctx.fill();

      // Vệt đỏ cảnh báo trên mặt đất
      ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
      ctx.fillRect(screenX - 18, GROUND_Y - 1, 36, 2);
      ctx.restore();
    }
  }

  private drawMilestonesAndNpcs(
    chapter: ChapterData,
    cameraX: number,
    collected: Set<RomanMilestoneId>,
    nearby: MilestoneArtifact | null,
    timeSec: number
  ): void {
    const { ctx } = this;

    for (const m of chapter.milestones) {
      const mx = Math.round(m.worldX - cameraX);
      if (mx < -30 || mx > INTERNAL_WIDTH + 30) continue;

      const isCollected = collected.has(m.roman);
      const isNearby = nearby?.roman === m.roman;

      // Bệ đá cổ vật
      ctx.fillStyle = isCollected ? PALETTE_12.CRIMSON_SEAL : PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(mx - 8, GROUND_Y - 10, 16, 10);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(mx - 10, GROUND_Y - 11, 20, 2);

      // Nhân chứng lịch sử đứng cạnh cột mốc
      drawNpcSprite(ctx, mx + 16, GROUND_Y, false);

      // Cổ vật lơ lửng phía trên bệ
      drawArtifactSprite(ctx, m.spriteType, mx, GROUND_Y - 22, isCollected, timeSec);

      // Nhãn số La Mã (I - XII)
      ctx.font = 'bold 7px monospace';
      ctx.textAlign = 'center';
      ctx.fillStyle = isCollected ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.BRONZE_BRIGHT;
      ctx.fillText(isCollected ? `✓ ${m.roman}` : m.roman, mx, GROUND_Y - 34);

      if (isNearby) {
        ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
        ctx.fillRect(mx - 18, GROUND_Y - 48, 36, 9);
        ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
        ctx.fillText('[E] KHÔI PHỤC', mx, GROUND_Y - 41);
      }
    }
  }

  private drawClimaxGate(
    chapter: ChapterData,
    cameraX: number,
    nearGate: boolean,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const gx = Math.round(chapter.climax.gateWorldX - cameraX);
    if (gx < -40 || gx > INTERNAL_WIDTH + 40) return;

    // Cổng Thời Gian Sơn Mài & Vàng Đồng
    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(gx - 16, GROUND_Y - 54, 6, 54);
    ctx.fillRect(gx + 10, GROUND_Y - 54, 6, 54);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(gx - 20, GROUND_Y - 58, 40, 6);

    // Vết nứt không gian bên trong cổng
    const pulse = Math.sin(timeSec * 5) * 2;
    ctx.fillStyle = burst > 0 ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(gx - 8, GROUND_Y - 48, 16, 46);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(gx - 3 - pulse * 0.5, GROUND_Y - 44, 6 + pulse, 38);

    if (nearGate) {
      ctx.font = 'bold 7px monospace';
      ctx.textAlign = 'center';
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(gx - 28, GROUND_Y - 70, 56, 10);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillText('[E] CỔNG THỜI GIAN', gx, GROUND_Y - 63);
    }
  }

  private drawClimaxSetPiece(
    chapter: ChapterData,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const type = chapter.climax.setPieceType;

    ctx.save();
    if (type === 'kowloon_torch') {
      // Đại cảnh Chương I: Ngọn đuốc Cương lĩnh 1930 rực cháy giữa đêm mưa
      const flameH = Math.round(35 + burst * 45 + Math.sin(timeSec * 14) * 5);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(145, GROUND_Y - flameH, 30, flameH);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(152, GROUND_Y - flameH + 10, 16, flameH - 10);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(156, GROUND_Y - flameH + 20, 8, flameH - 22);
    } else if (type === 'badinh_flag') {
      // Đại cảnh Chương II: Kéo Quốc kỳ tại Quảng trường Ba Đình 2/9/1945
      const poleX = 160;
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(poleX, 28, 3, GROUND_Y - 28);
      const flagY = Math.round(GROUND_Y - 25 - burst * 92);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(poleX + 3, flagY, 34, 22);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(poleX + 16, flagY + 7, 8, 8);
    } else if (type === 'dienbien_tank') {
      // Đại cảnh Chương III: Xe tăng tiến vào Dinh Độc Lập 30/4/1975
      const tankX = Math.round(40 + burst * 175);
      ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(tankX, GROUND_Y - 12, 32, 12);
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(tankX + 6, GROUND_Y - 20, 18, 8);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(tankX + 24, GROUND_Y - 18, 14, 3);
      // Lá cờ chiến thắng trên tháp pháo
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(tankX + 10, GROUND_Y - 32, 14, 9);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(tankX + 15, GROUND_Y - 29, 4, 4);
    } else if (type === 'doimoi_metro') {
      // Đại cảnh Chương IV: Đoàn tàu cao tốc & Pháo hoa Kỷ nguyên 2045
      const trainX = Math.round(-60 + burst * 280);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(trainX, 78, 84, 10);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(trainX, 83, 84, 3);
      // Pháo hoa vàng đồng & đỏ son trên bầu trời 2045
      for (let f = 0; f < 5; f++) {
        const fx = 55 + f * 52;
        const fy = 34 + (f % 2) * 16;
        const r = Math.round((burst * 18 + f * 3) % 18);
        ctx.fillStyle = f % 2 === 0 ? PALETTE_12.BRONZE_BRIGHT : PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(fx - r, fy, 2, 2);
        ctx.fillRect(fx + r, fy, 2, 2);
        ctx.fillRect(fx, fy - r, 2, 2);
        ctx.fillRect(fx, fy + r, 2, 2);
      }
    }
    ctx.restore();
  }

  private drawWeatherParticles(chapter: ChapterData, timeSec: number): void {
    const { ctx } = this;
    for (let i = 0; i < 28; i++) {
      const px = Math.floor((i * 37 + timeSec * (chapter.id === '1930' ? -45 : 18)) % INTERNAL_WIDTH);
      const py = Math.floor((i * 23 + timeSec * (chapter.id === '1930' ? 95 : 28)) % GROUND_Y);
      const wrappedX = px < 0 ? px + INTERNAL_WIDTH : px;

      if (chapter.id === '1930') {
        // Mưa đêm Cửu Long
        ctx.fillStyle = 'rgba(230, 229, 220, 0.32)';
        ctx.fillRect(wrappedX, py, 1, 4);
      } else if (chapter.id === '1945') {
        // Nắng thu Ba Đình
        ctx.fillStyle = 'rgba(212, 165, 93, 0.55)';
        ctx.fillRect(wrappedX, py, 2, 2);
      } else if (chapter.id === '1954_1975') {
        // Tàn lửa pháo Điện Biên
        ctx.fillStyle = i % 2 === 0 ? PALETTE_12.CRIMSON_VERMILION : PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(wrappedX, py, 2, 1);
      } else {
        // Hạt dữ liệu Kỷ nguyên số 2045
        ctx.fillStyle = 'rgba(212, 165, 93, 0.65)';
        ctx.fillRect(wrappedX, py, 1, 2);
      }
    }
  }
}
