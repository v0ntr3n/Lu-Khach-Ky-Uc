/**
 * @file src/engine/PixelRenderer.ts
 * @description Bộ dựng hình Canvas 2D 320x180 Integer-Scaled (HD-2D Chiaroscuro Sơn Mài & Giấy Dó).
 * Hỗ trợ:
 * - Chế độ Khám phá 2D Top-Down (Orthogonal 16x16 Tilemap, Y-Depth Sorting, Foreground Occlusion, 4-Direction Sprites)
 * - Chế độ 4 lớp Parallax theo từng Chương (1930, 1945, 1954-1975, 1986-2045)
 * - Ánh sáng Chiaroscuro hướng tâm & đèn rọi tuần tra Mức 1 (Story-Driven)
 * - 4 Đại Cảnh Tương Tác Bùng Nổ (Climax Set-Pieces: Đuốc 1930, Cờ Ba Đình 1945, Xe tăng 1975, Tàu cao tốc 2045)
 */

import { PALETTE_12 } from '../data/chronicles';
import type {
  ChapterData,
  Facing4,
  MilestoneArtifact,
  NormalizedMapData,
  NpcMapPlacement,
  PatrolActorRenderState,
  RomanMilestoneId,
  Vec2,
} from '../types/game';
import {
  type CharacterAtlasRole,
  drawArtifactSprite,
  drawCharacter4Dir,
  drawNpcSprite,
  drawTopDownTile,
  drawTravelerSprite,
} from './SpriteAtlas';

export const INTERNAL_WIDTH = 320;
export const INTERNAL_HEIGHT = 180;
export const GROUND_Y = 148;

export interface RenderState {
  chapter: ChapterData;
  playerX: number;
  playerY?: number;
  cameraX: number;
  cameraY?: number;
  facingRight: boolean;
  playerFacing?: Facing4;
  isMoving: boolean;
  isSheltered: boolean;
  collectedMilestones: Set<RomanMilestoneId>;
  nearbyMilestone: MilestoneArtifact | null;
  nearClimaxGate: boolean;
  climaxBurstProgress: number; // 0.0 -> 1.0 khi kích hoạt Bước B Đại Cảnh
  timeSec: number;
  mapData?: NormalizedMapData;
  nearbyPrompt?: string | null;
  nearbyTargetPos?: Vec2 | null;
  debugOverlay?: boolean;
  patrolActors?: PatrolActorRenderState[];
  reducedMotion?: boolean;
  transitionFadeAlpha?: number;
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
    if (state.mapData) {
      this.renderTopDown(state);
      return;
    }

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

  public renderTopDownWorld(state: RenderState): void {
    this.renderTopDown(state);
  }

  public renderTopDown(state: RenderState): void {
    const { ctx } = this;
    const mapData = state.mapData;
    if (!mapData) {
      this.render(state);
      return;
    }

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

    const playerY = state.playerY ?? GROUND_Y;
    const cameraY = state.cameraY ?? 0;
    const facing: Facing4 = state.playerFacing ?? (facingRight ? 'right' : 'left');
    const isInterior = mapData.id.includes('interior') || mapData.id.includes('warehouse');
    const tileSize = mapData.tileSize || 16;

    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(0, 0, INTERNAL_WIDTH, INTERNAL_HEIGHT);

    const startCol = Math.max(0, Math.floor(cameraX / tileSize));
    const endCol = Math.min(
      mapData.widthInTiles - 1,
      Math.ceil((cameraX + INTERNAL_WIDTH) / tileSize)
    );
    const startRow = Math.max(0, Math.floor(cameraY / tileSize));
    const endRow = Math.min(
      mapData.heightInTiles - 1,
      Math.ceil((cameraY + INTERNAL_HEIGHT) / tileSize)
    );

    const drawTileLayer = (layerName: string) => {
      const layer = mapData.tileLayers[layerName];
      if (!layer) return;
      for (let r = startRow; r <= endRow; r++) {
        for (let c = startCol; c <= endCol; c++) {
          const tileId = layer[r * mapData.widthInTiles + c] ?? 0;
          if (tileId > 0) {
            drawTopDownTile(
              ctx,
              tileId,
              Math.round(c * tileSize - cameraX),
              Math.round(r * tileSize - cameraY),
              isInterior
            );
          }
        }
      }
    };

    // 1. Lớp mặt đất & chi tiết mặt đất
    drawTileLayer('Ground');
    drawTileLayer('GroundDetails');
    drawTileLayer('LowProps');

    // 2. Vẽ khung chỉ báo vùng ẩn nấp (Shelters) & ngưỡng cửa dịch chuyển (Portals)
    for (const shelter of mapData.shelters) {
      if (!shelter.bounds) continue;
      const sx = Math.round(shelter.bounds.x - cameraX);
      const sy = Math.round(shelter.bounds.y - cameraY);
      const sw = Math.round(shelter.bounds.width);
      const sh = Math.round(shelter.bounds.height);
      if (sx + sw < 0 || sx > INTERNAL_WIDTH || sy + sh < 0 || sy > INTERNAL_HEIGHT) continue;
      ctx.fillStyle = isSheltered ? 'rgba(212, 165, 93, 0.20)' : 'rgba(17, 14, 11, 0.28)';
      ctx.fillRect(sx, sy, sw, sh);
    }

    for (const portal of mapData.portals) {
      const px = Math.round(portal.x - cameraX);
      const py = Math.round(portal.y - cameraY);
      const pw = Math.max(16, Math.round(portal.width));
      const ph = Math.max(10, Math.round(portal.height));
      if (px + pw < -16 || px > INTERNAL_WIDTH + 16 || py + ph < -16 || py > INTERNAL_HEIGHT + 16) {
        continue;
      }
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(px, py, pw, ph);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(px + 2, py + 2, pw - 4, 2);
      ctx.fillRect(px + 2, py + ph - 4, pw - 4, 2);
    }

    // 3. Y-Depth Sorting: Structures, Interactables, Milestones, NPCs, và Traveler
    const depthQueue: Array<{ footY: number; draw: () => void }> = [];

    const structuresLayer = mapData.tileLayers['Structures'];
    if (structuresLayer) {
      for (let r = startRow; r <= endRow; r++) {
        for (let c = startCol; c <= endCol; c++) {
          const tileId = structuresLayer[r * mapData.widthInTiles + c] ?? 0;
          if (tileId > 0) {
            const footY = (r + 1) * tileSize;
            const sx = Math.round(c * tileSize - cameraX);
            const sy = Math.round(r * tileSize - cameraY);
            depthQueue.push({
              footY,
              draw: () => drawTopDownTile(ctx, tileId, sx, sy, isInterior),
            });
          }
        }
      }
    }

    // Interactables (Doors, Evidence, Climax Gate)
    for (const inter of mapData.interactables) {
      const ix = Math.round(inter.x - cameraX);
      const iy = Math.round(inter.y - cameraY);
      if (ix < -32 || ix > INTERNAL_WIDTH + 32 || iy < -32 || iy > INTERNAL_HEIGHT + 32) continue;

      if (inter.kind === 'climax_gate') {
        depthQueue.push({
          footY: inter.y + 8,
          draw: () => {
            const pulse = Math.round(Math.sin(timeSec * 5) * 2);
            ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
            ctx.fillRect(ix - 14, iy - 30, 4, 30);
            ctx.fillRect(ix + 10, iy - 30, 4, 30);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 16, iy - 34, 32, 4);
            ctx.fillStyle =
              climaxBurstProgress > 0 ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.CRIMSON_SEAL;
            ctx.fillRect(ix - 8, iy - 28, 16, 26);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 3, iy - 24, 6 + pulse, 20);
          },
        });
      } else if (inter.kind === 'puzzle') {
        depthQueue.push({
          footY: inter.y + 6,
          draw: () => {
            // Bàn Kiểm Định Hồ Sơ Lịch Sử (Ornate Parchment Dossier Table & Gold Seal)
            ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
            ctx.fillRect(ix - 12, iy - 8, 24, 12);
            ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
            ctx.fillRect(ix - 11, iy - 7, 22, 9);
            ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
            ctx.fillRect(ix - 7, iy - 6, 14, 7);
            ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
            ctx.fillRect(ix - 2, iy - 4, 4, 3);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 1, iy - 13, 2, 4);
          },
        });
      } else if (inter.kind === 'evidence') {
        const milestone = inter.milestoneId
          ? chapter.milestones.find((m) => m.roman === inter.milestoneId)
          : undefined;
        const isDone = inter.milestoneId ? collectedMilestones.has(inter.milestoneId) : false;
        depthQueue.push({
          footY: inter.y + 4,
          draw: () => {
            ctx.fillStyle = isDone ? PALETTE_12.CRIMSON_SEAL : PALETTE_12.BRONZE_MUTED;
            ctx.fillRect(ix - 7, iy - 5, 14, 7);
            drawArtifactSprite(
              ctx,
              milestone?.spriteType ?? 'scroll',
              ix,
              iy - 10,
              isDone,
              timeSec
            );
            if (inter.milestoneId) {
              ctx.font = 'bold 7px monospace';
              ctx.textAlign = 'center';
              ctx.fillStyle = isDone ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.BRONZE_BRIGHT;
              ctx.fillText(isDone ? `✓ ${inter.milestoneId}` : inter.milestoneId, ix, iy - 20);
            }
          },
        });
      } else if (inter.kind === 'door') {
        depthQueue.push({
          footY: inter.y + 4,
          draw: () => {
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 6, iy - 2, 12, 3);
          },
        });
      } else if (inter.kind === 'checkpoint') {
        depthQueue.push({
          footY: inter.y + 4,
          draw: () => {
            // Trụ đèn báo an toàn (Safety Lantern Checkpoint)
            ctx.fillStyle = PALETTE_12.LACQUER_VOID;
            ctx.fillRect(ix - 3, iy - 2, 6, 4);
            ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
            ctx.fillRect(ix - 1, iy - 12, 2, 10);
            ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
            ctx.fillRect(ix - 3, iy - 16, 6, 5);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 2, iy - 15, 4, 3);
          },
        });
      }
    }

    // NPCs trên bản đồ
    for (const npc of mapData.npcs) {
      const nx = Math.round(npc.x - cameraX);
      const ny = Math.round(npc.y - cameraY);
      if (nx < -24 || nx > INTERNAL_WIDTH + 24 || ny < -24 || ny > INTERNAL_HEIGHT + 24) continue;

      const role = this.resolveNpcRole(npc);
      depthQueue.push({
        footY: npc.y,
        draw: () => {
          drawCharacter4Dir(ctx, role, nx, ny, npc.facing, false, false, timeSec);
        },
      });
    }

    // Nhân vật Lữ Khách Ký Ức 2045
    const screenPlayerX = Math.round(playerX - cameraX);
    const screenPlayerY = Math.round(playerY - cameraY);
    const animateWalk = isMoving && !state.reducedMotion;
    depthQueue.push({
      footY: playerY,
      draw: () => {
        drawCharacter4Dir(
          ctx,
          'traveler',
          screenPlayerX,
          screenPlayerY,
          facing,
          animateWalk,
          isSheltered,
          timeSec
        );
      },
    });

    depthQueue.sort((a, b) => a.footY - b.footY);
    for (const item of depthQueue) {
      item.draw();
    }

    // 4. Lớp Foreground (Mái hiên & Tán cây cổ thụ che phủ phía trước nhân vật khi đi qua bên dưới)
    const foregroundLayer = mapData.tileLayers['Foreground'];
    if (foregroundLayer) {
      const playerUnderOccluder = mapData.occluders.some(
        (occ) =>
          playerX >= occ.x &&
          playerX <= occ.x + occ.width &&
          playerY >= occ.y &&
          playerY <= occ.y + occ.height
      );
      ctx.save();
      if (playerUnderOccluder) {
        ctx.globalAlpha = 0.76;
      }
      drawTileLayer('Foreground');
      ctx.restore();
    }

    // 5. Đèn rọi tuần tra & Lính tuần tra góc nhìn nón (Patrol Cones / Hazard Sweeps)
    if (climaxBurstProgress <= 0.05) {
      this.drawTopDownPatrols(
        mapData,
        chapter,
        cameraX,
        cameraY,
        timeSec,
        state.patrolActors
      );
    }

    // 6. Hào quang Chiaroscuro quanh Lữ Khách & Vầng sáng đèn lồng/mưa đêm PixelFX
    this.drawTopDownChiaroscuro(screenPlayerX, screenPlayerY, climaxBurstProgress, isInterior);
    this.drawEnvironmentalVfx(
      mapData,
      cameraX,
      cameraY,
      timeSec,
      isInterior,
      Boolean(state.reducedMotion)
    );

    // 7. Đại cảnh Climax nếu kích hoạt
    if (climaxBurstProgress > 0) {
      this.drawClimaxSetPiece(chapter, climaxBurstProgress, timeSec);
    }

    // 8. Hạt thời tiết (chỉ vẽ ngoài trời và khi không bật giảm chuyển động)
    if (!isInterior && !state.reducedMotion) {
      this.drawWeatherParticles(chapter, timeSec);
    }

    // 8.5. Lớp phủ kiểm định bản đồ (Debug Overlay [F3])
    if (state.debugOverlay) {
      this.drawDebugOverlay(mapData, state, cameraX, cameraY);
    }

    // 9. Nhãn tương tác [E] ngữ cảnh
    const promptLabel =
      state.nearbyPrompt ??
      (nearbyMilestone
        ? `[E] ${nearbyMilestone.roman} · ${nearbyMilestone.title}`
        : nearClimaxGate
          ? '[E] CỔNG THỜI GIAN'
          : null);

    if (promptLabel) {
      const targetX = state.nearbyTargetPos
        ? Math.round(state.nearbyTargetPos.x - cameraX)
        : screenPlayerX;
      const targetY = state.nearbyTargetPos
        ? Math.round(state.nearbyTargetPos.y - cameraY) - 22
        : screenPlayerY - 22;
      this.drawPromptBadge(targetX, targetY, promptLabel);
    }

    // 10. Hiệu ứng mờ chuyển cảnh Sơn Mài (Scene Transition Ink-Lacquer Fade)
    if ((state.transitionFadeAlpha ?? 0) > 0.01 && !state.reducedMotion) {
      ctx.save();
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.globalAlpha = Math.max(0, Math.min(1, state.transitionFadeAlpha ?? 0));
      ctx.fillRect(0, 0, INTERNAL_WIDTH, INTERNAL_HEIGHT);
      ctx.restore();
    }
  }

  private drawEnvironmentalVfx(
    mapData: NormalizedMapData,
    cameraX: number,
    cameraY: number,
    timeSec: number,
    isInterior: boolean,
    reducedMotion: boolean
  ): void {
    const { ctx } = this;
    ctx.save();

    // 1. Vầng sáng đèn lồng ấm (Warm Lantern & Lamp Pools) quanh Checkpoints và Portals
    for (const inter of mapData.interactables) {
      if (inter.kind !== 'checkpoint' && inter.kind !== 'puzzle') continue;
      const lx = Math.round(inter.x - cameraX);
      const ly = Math.round(inter.y - cameraY - 8);
      if (lx < -32 || lx > INTERNAL_WIDTH + 32 || ly < -32 || ly > INTERNAL_HEIGHT + 32) continue;
      const halo = ctx.createRadialGradient(lx, ly, 2, lx, ly, 24);
      halo.addColorStop(0, 'rgba(212, 168, 79, 0.18)');
      halo.addColorStop(1, 'rgba(212, 168, 79, 0)');
      ctx.fillStyle = halo;
      ctx.fillRect(lx - 24, ly - 24, 48, 48);
    }

    // 2. Vòng gợn mưa 3-frame trên mặt đá (PixelFX Rain Splash Ripples) khi ngoài trời & không giảm chuyển động
    if (!isInterior && !reducedMotion) {
      for (let i = 0; i < 10; i++) {
        const phase = (timeSec * 2.4 + i * 0.37) % 1;
        const rx = Math.round(((i * 97 + 44) % mapData.widthPx) - cameraX);
        const ry = Math.round(((i * 61 + 120) % mapData.heightPx) - cameraY);
        if (rx < 4 || rx > INTERNAL_WIDTH - 4 || ry < 4 || ry > INTERNAL_HEIGHT - 4) continue;
        const radius = phase < 0.33 ? 1 : phase < 0.66 ? 2 : 3;
        ctx.fillStyle =
          phase < 0.66 ? 'rgba(230, 229, 220, 0.35)' : 'rgba(212, 165, 93, 0.22)';
        ctx.fillRect(rx - radius, ry, 1, 1);
        ctx.fillRect(rx + radius, ry, 1, 1);
        ctx.fillRect(rx, ry - 1, 1, 1);
      }
    }

    ctx.restore();
  }

  private drawDebugOverlay(
    map: NormalizedMapData,
    state: RenderState,
    camX: number,
    camY: number
  ): void {
    const { ctx } = this;
    const playerY = state.playerY ?? GROUND_Y;

    ctx.save();
    ctx.lineWidth = 1;
    ctx.font = '6px monospace';
    ctx.textAlign = 'left';

    // 1. Obstacles (Collision walls/props)
    ctx.fillStyle = 'rgba(196, 58, 43, 0.32)';
    ctx.strokeStyle = '#E05A47';
    for (const obs of map.obstacles) {
      const rx = Math.round(obs.x - camX);
      const ry = Math.round(obs.y - camY);
      ctx.fillRect(rx, ry, obs.width, obs.height);
      ctx.strokeRect(rx + 0.5, ry + 0.5, Math.max(1, obs.width - 1), Math.max(1, obs.height - 1));
    }

    // 2. Shelters
    ctx.fillStyle = 'rgba(58, 125, 92, 0.28)';
    ctx.strokeStyle = '#3A7D5C';
    for (const sh of map.shelters) {
      if (!sh.bounds) continue;
      const rx = Math.round(sh.bounds.x - camX);
      const ry = Math.round(sh.bounds.y - camY);
      ctx.fillRect(rx, ry, sh.bounds.width, sh.bounds.height);
      ctx.strokeRect(
        rx + 0.5,
        ry + 0.5,
        Math.max(1, sh.bounds.width - 1),
        Math.max(1, sh.bounds.height - 1)
      );
    }

    // 3. Occluders
    ctx.strokeStyle = '#6B9BD2';
    for (const occ of map.occluders) {
      const rx = Math.round(occ.x - camX);
      const ry = Math.round(occ.y - camY);
      ctx.strokeRect(rx + 0.5, ry + 0.5, Math.max(1, occ.width - 1), Math.max(1, occ.height - 1));
    }

    // 4. Portals
    ctx.strokeStyle = '#4ADE80';
    ctx.fillStyle = '#4ADE80';
    for (const p of map.portals) {
      const rx = Math.round(p.x - camX);
      const ry = Math.round(p.y - camY);
      ctx.strokeRect(rx + 0.5, ry + 0.5, Math.max(1, p.width - 1), Math.max(1, p.height - 1));
      ctx.fillText(p.destinationMap, rx + 2, ry - 2);
    }

    // 5. Spawns & Checkpoints
    ctx.strokeStyle = '#38BDF8';
    ctx.fillStyle = '#38BDF8';
    for (const sp of [...map.spawns, ...map.checkpoints]) {
      const sx = Math.round(sp.x - camX);
      const sy = Math.round(sp.y - camY);
      ctx.beginPath();
      ctx.moveTo(sx - 4, sy);
      ctx.lineTo(sx + 4, sy);
      ctx.moveTo(sx, sy - 4);
      ctx.lineTo(sx, sy + 4);
      ctx.stroke();
      ctx.fillText(sp.id, sx + 5, sy - 2);
    }

    // 6. NPC approach radius & routes + Interactable radius
    ctx.strokeStyle = 'rgba(212, 168, 79, 0.45)';
    for (const npc of map.npcs) {
      const nx = Math.round(npc.x - camX);
      const ny = Math.round(npc.y - camY);
      ctx.beginPath();
      ctx.arc(nx, ny, npc.approachRadius || 28, 0, Math.PI * 2);
      ctx.stroke();

      if (npc.route && npc.route.length > 1) {
        ctx.beginPath();
        npc.route.forEach((pt, idx) => {
          const px = Math.round(pt.x - camX);
          const py = Math.round(pt.y - camY);
          if (idx === 0) ctx.moveTo(px, py);
          else ctx.lineTo(px, py);
        });
        ctx.stroke();
      }
    }

    for (const inter of map.interactables) {
      const ix = Math.round(inter.x - camX);
      const iy = Math.round(inter.y - camY);
      ctx.beginPath();
      ctx.arc(ix, iy, inter.radius ?? 26, 0, Math.PI * 2);
      ctx.stroke();
    }

    // 7. Player collision footbox
    const px = Math.round(state.playerX - camX);
    const py = Math.round(playerY - camY);
    ctx.strokeStyle = '#F4E7C3';
    ctx.strokeRect(px - 6 + 0.5, py - 6 + 0.5, 11, 7);

    // 8. Top-left debug badge
    const badgeText = `DEBUG OVERLAY [F3]: ON | MAP: ${map.id} | POS: (${Math.round(
      state.playerX
    )}, ${Math.round(playerY)})`;
    ctx.fillStyle = 'rgba(17, 14, 11, 0.85)';
    ctx.fillRect(4, 4, 240, 11);
    ctx.fillStyle = '#F4EFE4';
    ctx.fillText(badgeText, 7, 12);

    ctx.restore();
  }

  private resolveNpcRole(npc: NpcMapPlacement): CharacterAtlasRole {
    const key = `${npc.npcId} ${npc.role}`.toLowerCase();
    if (key.includes('courier') || key.includes('giao liên')) return 'courier';
    if (key.includes('archivist') || key.includes('lưu trữ')) return 'archivist';
    if (key.includes('observer') || key.includes('bến')) return 'observer';
    return 'historian';
  }

  private drawTopDownPatrols(
    mapData: NormalizedMapData,
    chapter: ChapterData,
    cameraX: number,
    cameraY: number,
    timeSec: number,
    patrolActors?: PatrolActorRenderState[]
  ): void {
    const { ctx } = this;

    if (patrolActors && patrolActors.length > 0) {
      for (const guard of patrolActors) {
        const gx = Math.round(guard.x - cameraX);
        const gy = Math.round(guard.y - cameraY);
        if (gx < -96 || gx > INTERNAL_WIDTH + 96 || gy < -96 || gy > INTERNAL_HEIGHT + 96) {
          continue;
        }

        ctx.save();
        ctx.beginPath();
        ctx.moveTo(gx, gy - 6);
        ctx.arc(
          gx,
          gy - 6,
          guard.coneLength,
          guard.facingAngleRad - guard.coneAngleRad / 2,
          guard.facingAngleRad + guard.coneAngleRad / 2
        );
        ctx.closePath();

        if (guard.alertState === 'alert') {
          ctx.fillStyle = 'rgba(196, 58, 43, 0.38)';
          ctx.strokeStyle = 'rgba(224, 90, 71, 0.9)';
        } else if (guard.alertState === 'suspicious') {
          ctx.fillStyle = 'rgba(234, 138, 46, 0.28)';
          ctx.strokeStyle = 'rgba(234, 138, 46, 0.75)';
        } else {
          ctx.fillStyle = 'rgba(212, 168, 79, 0.16)';
          ctx.strokeStyle = 'rgba(212, 168, 79, 0.45)';
        }
        ctx.lineWidth = 1;
        ctx.fill();
        ctx.stroke();

        // Chuyển góc radian sang hướng 4 chiều cho sprite lính tuần tra
        const deg = ((guard.facingAngleRad * 180) / Math.PI + 360) % 360;
        const guardFacing: Facing4 =
          deg >= 45 && deg < 135
            ? 'down'
            : deg >= 135 && deg < 225
              ? 'left'
              : deg >= 225 && deg < 315
                ? 'up'
                : 'right';

        drawCharacter4Dir(ctx, 'observer', gx, gy, guardFacing, true, false, timeSec);

        // Thanh nghi ngờ / Biểu tượng cảnh báo (? / !)
        if (guard.suspicionLevel > 0 || guard.alertState !== 'calm') {
          ctx.fillStyle = PALETTE_12.LACQUER_VOID;
          ctx.fillRect(gx - 7, gy - 24, 14, 3);
          ctx.fillStyle =
            guard.alertState === 'alert'
              ? PALETTE_12.CRIMSON_VERMILION
              : PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(
            gx - 6,
            gy - 23,
            Math.max(2, Math.round(12 * Math.min(1, guard.suspicionLevel))),
            1
          );

          ctx.font = 'bold 8px monospace';
          ctx.textAlign = 'center';
          ctx.fillText(
            guard.alertState === 'alert' ? '!' : '?',
            gx,
            gy - 26
          );
        }
        ctx.restore();
      }
      return;
    }

    if (mapData.patrols.length > 0) {
      for (const p of mapData.patrols) {
        if (p.waypoints.length === 0) continue;
        const w0 = p.waypoints[0];
        const w1 = p.waypoints[p.waypoints.length - 1] ?? w0;
        const t = 0.5 + 0.5 * Math.sin(timeSec * (p.speed * 0.04));
        const wx = w0.x + (w1.x - w0.x) * t;
        const wy = w0.y + (w1.y - w0.y) * t;
        const sx = Math.round(wx - cameraX);
        const sy = Math.round(wy - cameraY);

        ctx.save();
        const grad = ctx.createRadialGradient(sx, sy, 2, sx, sy, 26);
        grad.addColorStop(0, 'rgba(223, 107, 87, 0.38)');
        grad.addColorStop(1, 'rgba(212, 165, 93, 0)');
        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(sx, sy, 26, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();
      }
      return;
    }

    for (const h of chapter.hazards) {
      const sweepX = h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
      const sx = Math.round(sweepX - cameraX);
      const sy = Math.round(GROUND_Y - 16 - cameraY);
      ctx.save();
      const grad = ctx.createRadialGradient(sx, sy, 2, sx, sy, 28);
      grad.addColorStop(0, 'rgba(223, 107, 87, 0.35)');
      grad.addColorStop(1, 'rgba(212, 165, 93, 0)');
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.arc(sx, sy, 28, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }
  }

  private drawTopDownChiaroscuro(
    playerScreenX: number,
    playerScreenY: number,
    burst: number,
    isInterior: boolean
  ): void {
    const { ctx } = this;
    const cx = Math.max(0, Math.min(INTERNAL_WIDTH, playerScreenX));
    const cy = Math.max(0, Math.min(INTERNAL_HEIGHT, playerScreenY - 8));
    const grad = ctx.createRadialGradient(cx, cy, 18, cx, cy, isInterior ? 125 : 150 + burst * 50);
    grad.addColorStop(0, 'rgba(212, 165, 93, 0.10)');
    grad.addColorStop(0.65, 'rgba(17, 14, 11, 0.14)');
    grad.addColorStop(1, 'rgba(17, 14, 11, 0.42)');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, INTERNAL_WIDTH, INTERNAL_HEIGHT);
  }

  private drawPromptBadge(x: number, y: number, text: string): void {
    const { ctx } = this;
    ctx.save();
    ctx.font = 'bold 7px monospace';
    ctx.textAlign = 'center';
    const label = text.length > 26 ? `${text.slice(0, 25)}…` : text;
    const w = Math.max(44, label.length * 4.4 + 10);
    const bx = Math.round(Math.max(4, Math.min(INTERNAL_WIDTH - w - 4, x - w * 0.5)));
    const by = Math.round(Math.max(4, Math.min(INTERNAL_HEIGHT - 14, y - 9)));

    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(bx, by, Math.round(w), 10);
    ctx.strokeStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.lineWidth = 1;
    ctx.strokeRect(bx + 0.5, by + 0.5, Math.round(w) - 1, 9);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillText(label, Math.round(bx + w * 0.5), by + 7);
    ctx.restore();
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

      // Xung tín hiệu điện báo chạy dọc dây
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
