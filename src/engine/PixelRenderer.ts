/**
 * @file src/engine/PixelRenderer.ts
 * @description Bộ dựng hình Canvas 2D Integer-Scaled (HD-2D Chiaroscuro Sơn Mài & Giấy Dó).
 * Hỗ trợ:
 * - Khung nhìn logic động (`setLogicalSize`, `GameShell` integer pixel scaling) không cắt xén biên
 * - Chế độ Khám phá 2D Top-Down (Orthogonal 16x16 Tilemap, Y-Depth Sorting, Foreground Occlusion, 4-Direction 16x22 Sprites)
 * - Khung viền đồng cổ 2 lớp quanh bản đồ nội thất nhỏ căn giữa (`cameraX < 0 || cameraY < 0`)
 * - Chế độ 4 lớp Parallax theo từng Chương (1930, 1945, 1954-1975, 1986-2045)
 * - Ánh sáng Chiaroscuro hướng tâm, chất lượng hiệu ứng (`effectsQuality: 'full' | 'subtle'`) & Đại Cảnh Climax
 */

import { PALETTE_12 } from '../data/chronicles';
import type {
  ChapterData,
  EffectsQuality,
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
  drawZoneLandmarks,
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
  trackedTargetPos?: Vec2 | null;
  trackedTargetId?: string | null;
  activeQuestTargetIds?: Set<string>;
  optionalTargetIds?: Set<string>;
  routePoints?: Vec2[];
  showHints?: boolean;
  debugOverlay?: boolean;
  patrolActors?: PatrolActorRenderState[];
  reducedMotion?: boolean;
  effectsQuality?: EffectsQuality;
  transitionFadeAlpha?: number;
}

export class PixelRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    if (
      !this.canvas.width ||
      !this.canvas.height ||
      (this.canvas.width === 300 && this.canvas.height === 150)
    ) {
      this.canvas.width = INTERNAL_WIDTH;
      this.canvas.height = INTERNAL_HEIGHT;
    }
    const context = this.canvas.getContext('2d');
    if (!context) {
      throw new Error('Không thể khởi tạo Canvas 2D cho PixelRenderer.');
    }
    this.ctx = context;
    this.ctx.imageSmoothingEnabled = false;
  }

  /**
   * Cập nhật độ phân giải logic của `#pixel-canvas` đồng bộ với `GameShell`.
   */
  public setLogicalSize(width: number, height: number): void {
    const safeW = Math.max(160, Math.round(width || INTERNAL_WIDTH));
    const safeH = Math.max(90, Math.round(height || INTERNAL_HEIGHT));
    if (this.canvas.width !== safeW) {
      this.canvas.width = safeW;
    }
    if (this.canvas.height !== safeH) {
      this.canvas.height = safeH;
    }
    this.ctx.imageSmoothingEnabled = false;
  }

  private getViewWidth(): number {
    return this.canvas.width || INTERNAL_WIDTH;
  }

  private getViewHeight(): number {
    return this.canvas.height || INTERNAL_HEIGHT;
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
    const effectsQuality: EffectsQuality = state.effectsQuality ?? 'full';

    ctx.imageSmoothingEnabled = false;

    // 1. Nền đêm Sơn Mài & Hào quang Chiaroscuro hướng tâm
    this.drawSkyAndChiaroscuro(chapter, climaxBurstProgress, timeSec, effectsQuality);

    // 2. Lớp kiến trúc nền xa (Parallax 0.25x)
    this.drawDistantParallax(
      chapter,
      cameraX,
      collectedMilestones,
      climaxBurstProgress,
      timeSec
    );

    // 3. Lớp kiến trúc trung cảnh & đường dây/cầu (Parallax 0.55x)
    this.drawMidgroundParallax(
      chapter,
      cameraX,
      collectedMilestones,
      climaxBurstProgress,
      timeSec
    );

    // 4. Mặt đất & Hiên nhà / Bóng đa / Giao thông hào ẩn nấp (Parallax 1.0x)
    this.drawGroundAndShelters(chapter, cameraX, playerX);

    // 5. Chùm đèn tuần tra / Pháo sáng (Mức 1 — chỉ đẩy lùi nhẹ nếu không ẩn nấp)
    if (climaxBurstProgress <= 0.05) {
      this.drawHazardSweeps(chapter, cameraX, timeSec);
    }

    // 6. Cột mốc La Mã (I - XII) & NPC Nhân chứng Lịch sử
    this.drawMilestonesAndNpcs(
      chapter,
      cameraX,
      collectedMilestones,
      nearbyMilestone,
      timeSec
    );

    // 7. Cổng Thời Gian Cuối Màn (Climax Gate)
    this.drawClimaxGate(
      chapter,
      cameraX,
      nearClimaxGate,
      climaxBurstProgress,
      timeSec
    );

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
    if (!state.reducedMotion) {
      this.drawWeatherParticles(chapter, timeSec, effectsQuality, false);
    }
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

    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();

    const {
      chapter,
      playerX,
      cameraX,
      facingRight,
      isMoving,
      isSheltered,
      collectedMilestones,
      climaxBurstProgress,
      timeSec,
    } = state;

    const playerY = state.playerY ?? GROUND_Y;
    const cameraY = state.cameraY ?? 0;
    const facing: Facing4 = state.playerFacing ?? (facingRight ? 'right' : 'left');
    const reducedMotion = Boolean(state.reducedMotion);
    const effectsQuality: EffectsQuality = state.effectsQuality ?? 'full';
    const isInterior =
      mapData.id.includes('interior') || mapData.id.includes('warehouse');
    const tileSize = mapData.tileSize || 16;

    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(0, 0, viewW, viewH);

    const startCol = Math.max(0, Math.floor(cameraX / tileSize));
    const endCol = Math.min(
      mapData.widthInTiles - 1,
      Math.ceil((cameraX + viewW) / tileSize)
    );
    const startRow = Math.max(0, Math.floor(cameraY / tileSize));
    const endRow = Math.min(
      mapData.heightInTiles - 1,
      Math.ceil((cameraY + viewH) / tileSize)
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
              isInterior,
              mapData.eraId,
              timeSec,
              reducedMotion
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
      if (sx + sw < 0 || sx > viewW || sy + sh < 0 || sy > viewH) continue;
      ctx.fillStyle = isSheltered
        ? 'rgba(212, 165, 93, 0.20)'
        : 'rgba(17, 14, 11, 0.28)';
      ctx.fillRect(sx, sy, sw, sh);
    }

    for (const portal of mapData.portals) {
      const px = Math.round(portal.x - cameraX);
      const py = Math.round(portal.y - cameraY);
      const pw = Math.max(16, Math.round(portal.width));
      const ph = Math.max(10, Math.round(portal.height));
      if (px + pw < -16 || px > viewW + 16 || py + ph < -16 || py > viewH + 16) {
        continue;
      }
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(px, py, pw, ph);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(px + 2, py + 2, pw - 4, 2);
      ctx.fillRect(px + 2, py + ph - 4, pw - 4, 2);
    }

    // 2.5. Dấu chân dẫn đường A* trên mặt đất (Breadcrumb Chevrons) khi bật chỉ dẫn
    if (
      state.showHints !== false &&
      state.routePoints &&
      state.routePoints.length > 1
    ) {
      this.drawRouteBreadcrumbs(
        state.routePoints,
        cameraX,
        cameraY,
        viewW,
        viewH,
        timeSec,
        reducedMotion
      );
    }

    const isTrackedEntity = (id?: string, x?: number, y?: number): boolean => {
      if (id && state.trackedTargetId && id === state.trackedTargetId) {
        return true;
      }
      if (
        state.trackedTargetPos &&
        typeof x === 'number' &&
        typeof y === 'number'
      ) {
        return Math.hypot(x - state.trackedTargetPos.x, y - state.trackedTargetPos.y) <= 18;
      }
      return false;
    };

    // 3. Y-Depth Sorting: Structures, Landmarks, Interactables, Milestones, NPCs, và Traveler
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
              draw: () =>
                drawTopDownTile(
                  ctx,
                  tileId,
                  sx,
                  sy,
                  isInterior,
                  mapData.eraId,
                  timeSec,
                  reducedMotion
                ),
            });
          }
        }
      }
    }

    // Công trình biểu tượng (Focal Landmarks) của khu vực
    depthQueue.push({
      footY: 162,
      draw: () =>
        drawZoneLandmarks(ctx, mapData, cameraX, cameraY, timeSec, reducedMotion),
    });

    // Interactables (Doors, Evidence, Climax Gate)
    for (const inter of mapData.interactables) {
      const ix = Math.round(inter.x - cameraX);
      const iy = Math.round(inter.y - cameraY);
      if (ix < -32 || ix > viewW + 32 || iy < -32 || iy > viewH + 32) continue;

      const tracked = isTrackedEntity(inter.id, inter.x, inter.y);
      const activeMain =
        state.activeQuestTargetIds?.has(inter.id) ||
        (inter.milestoneId ? state.activeQuestTargetIds?.has(inter.milestoneId) : false);
      const optionalLore = state.optionalTargetIds?.has(inter.id) ?? false;

      if (inter.kind === 'climax_gate') {
        depthQueue.push({
          footY: inter.y + 8,
          draw: () => {
            const pulse = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 5) * 2);
            ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
            ctx.fillRect(ix - 14, iy - 30, 4, 30);
            ctx.fillRect(ix + 10, iy - 30, 4, 30);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 16, iy - 34, 32, 4);
            ctx.fillStyle =
              climaxBurstProgress > 0
                ? PALETTE_12.PARCHMENT_LIGHT
                : PALETTE_12.CRIMSON_SEAL;
            ctx.fillRect(ix - 8, iy - 28, 16, 26);
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 3, iy - 24, 6 + pulse, 20);
            if (tracked || activeMain) {
              this.drawOverheadQuestMarker(
                ix,
                iy - 38,
                tracked ? 'tracked' : 'main',
                timeSec,
                reducedMotion
              );
            }
          },
        });
      } else if (inter.kind === 'puzzle') {
        depthQueue.push({
          footY: inter.y + 6,
          draw: () => {
            if (tracked) {
              ctx.fillStyle = 'rgba(212, 165, 93, 0.24)';
              ctx.fillRect(ix - 14, iy - 2, 28, 6);
            }
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
            if (tracked || activeMain) {
              this.drawOverheadQuestMarker(
                ix,
                iy - 18,
                tracked ? 'tracked' : 'main',
                timeSec,
                reducedMotion
              );
            }
          },
        });
      } else if (inter.kind === 'evidence') {
        const milestone = inter.milestoneId
          ? chapter.milestones.find((m) => m.roman === inter.milestoneId)
          : undefined;
        const isDone = inter.milestoneId
          ? collectedMilestones.has(inter.milestoneId)
          : false;
        depthQueue.push({
          footY: inter.y + 4,
          draw: () => {
            if (tracked && !isDone) {
              ctx.fillStyle = 'rgba(212, 165, 93, 0.26)';
              ctx.fillRect(ix - 9, iy - 1, 18, 5);
            }
            ctx.fillStyle = isDone ? PALETTE_12.CRIMSON_SEAL : PALETTE_12.BRONZE_MUTED;
            ctx.fillRect(ix - 7, iy - 5, 14, 7);
            drawArtifactSprite(
              ctx,
              milestone?.spriteType ?? 'scroll',
              ix,
              iy - 10,
              isDone,
              reducedMotion ? 0 : timeSec
            );
            if (isDone) {
              this.drawOverheadQuestMarker(ix, iy - 22, 'completed', timeSec, reducedMotion);
            } else if (tracked) {
              this.drawOverheadQuestMarker(ix, iy - 24, 'tracked', timeSec, reducedMotion);
            } else if (activeMain || inter.milestoneId) {
              this.drawOverheadQuestMarker(ix, iy - 23, 'main', timeSec, reducedMotion);
            } else if (optionalLore) {
              this.drawOverheadQuestMarker(ix, iy - 22, 'optional', timeSec, reducedMotion);
            }
          },
        });
      } else if (inter.kind === 'door') {
        depthQueue.push({
          footY: inter.y + 4,
          draw: () => {
            ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
            ctx.fillRect(ix - 6, iy - 2, 12, 3);
            if (tracked) {
              this.drawOverheadQuestMarker(ix, iy - 12, 'tracked', timeSec, reducedMotion);
            }
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
      if (nx < -28 || nx > viewW + 28 || ny < -28 || ny > viewH + 28) continue;

      const role = this.resolveNpcRole(npc);
      const tracked =
        isTrackedEntity(npc.id, npc.x, npc.y) ||
        isTrackedEntity(npc.npcId, npc.x, npc.y);
      const isMilestoneDone = npc.milestoneRoman
        ? collectedMilestones.has(npc.milestoneRoman)
        : false;
      const activeMain =
        state.activeQuestTargetIds?.has(npc.id) ||
        state.activeQuestTargetIds?.has(npc.npcId) ||
        (npc.milestoneRoman ? !isMilestoneDone : false);
      const optionalLore =
        state.optionalTargetIds?.has(npc.id) ||
        state.optionalTargetIds?.has(npc.npcId) ||
        Boolean(npc.dialogueId && !npc.milestoneRoman && !activeMain);

      depthQueue.push({
        footY: npc.y,
        draw: () => {
          if (tracked && !isMilestoneDone) {
            ctx.fillStyle = 'rgba(212, 165, 93, 0.28)';
            ctx.fillRect(nx - 8, ny - 2, 16, 4);
          }
          drawCharacter4Dir(
            ctx,
            role,
            nx,
            ny,
            npc.facing,
            false,
            false,
            timeSec,
            reducedMotion
          );
          if (isMilestoneDone) {
            this.drawOverheadQuestMarker(nx, ny - 26, 'completed', timeSec, reducedMotion);
          } else if (tracked) {
            this.drawOverheadQuestMarker(nx, ny - 27, 'tracked', timeSec, reducedMotion);
          } else if (activeMain) {
            this.drawOverheadQuestMarker(nx, ny - 26, 'main', timeSec, reducedMotion);
          } else if (optionalLore) {
            this.drawOverheadQuestMarker(nx, ny - 25, 'optional', timeSec, reducedMotion);
          }
        },
      });
    }

    // Nhân vật Lữ Khách Ký Ức 2045
    const screenPlayerX = Math.round(playerX - cameraX);
    const screenPlayerY = Math.round(playerY - cameraY);
    const animateWalk = isMoving && !reducedMotion;
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
          timeSec,
          reducedMotion
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
        state.patrolActors,
        reducedMotion
      );
    }

    // 6. Hào quang Chiaroscuro quanh Lữ Khách & Vầng sáng đèn lồng/mưa đêm PixelFX
    this.drawTopDownChiaroscuro(
      screenPlayerX,
      screenPlayerY,
      climaxBurstProgress,
      isInterior,
      effectsQuality
    );
    this.drawEnvironmentalVfx(
      mapData,
      cameraX,
      cameraY,
      timeSec,
      isInterior,
      reducedMotion,
      effectsQuality
    );

    // 6.5. Khung viền đồng cổ 2 lớp quanh bản đồ nhỏ căn giữa (khi cameraX < 0 || cameraY < 0)
    if (cameraX < 0 || cameraY < 0) {
      this.drawCenteredMapBorder(cameraX, cameraY, mapData.widthPx, mapData.heightPx);
    }

    // 6.8. Mũi tên chỉ báo rìa màn hình khi mục tiêu đang theo dõi nằm ngoài khung nhìn camera
    if (state.showHints !== false && state.trackedTargetPos) {
      this.drawOffscreenTargetIndicator(
        state.trackedTargetPos,
        cameraX,
        cameraY,
        viewW,
        viewH,
        timeSec,
        reducedMotion
      );
    }

    // 7. Đại cảnh Climax nếu kích hoạt
    if (climaxBurstProgress > 0) {
      this.drawClimaxSetPiece(chapter, climaxBurstProgress, timeSec);
    }

    // 8. Hạt thời tiết (chỉ vẽ ngoài trời và khi không bật giảm chuyển động)
    if (!isInterior && !reducedMotion) {
      this.drawWeatherParticles(chapter, timeSec, effectsQuality, true);
    }

    // 8.5. Lớp phủ kiểm định hình học bản đồ (Debug Overlay [F3] — văn bản hiển thị qua DOM #dev-debug-readout)
    if (state.debugOverlay) {
      this.drawDebugOverlay(mapData, state, cameraX, cameraY);
    }

    // Lưu ý: Nhãn tương tác [E] ngữ cảnh được dựng sắc nét trên lớp phủ DOM `#world-interaction-prompt`
    // thay vì vẽ chữ 7px lên Canvas!

    // 10. Hiệu ứng mờ chuyển cảnh Sơn Mài (Scene Transition Ink-Lacquer Fade)
    if ((state.transitionFadeAlpha ?? 0) > 0.01 && !reducedMotion) {
      ctx.save();
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.globalAlpha = Math.max(0, Math.min(1, state.transitionFadeAlpha ?? 0));
      ctx.fillRect(0, 0, viewW, viewH);
      ctx.restore();
    }
  }

  /**
   * Vẽ chuỗi dấu dẫn đường A* bằng đồng sáng trên mặt đất (tối đa 7 bước phía trước Lữ Khách).
   */
  private drawRouteBreadcrumbs(
    routePoints: Vec2[],
    cameraX: number,
    cameraY: number,
    viewW: number,
    viewH: number,
    timeSec: number,
    reducedMotion: boolean
  ): void {
    const { ctx } = this;
    const maxSteps = Math.min(routePoints.length - 1, 8);
    const activePulseIdx = reducedMotion ? 1 : Math.floor(timeSec * 4) % Math.max(1, maxSteps);

    for (let i = 1; i <= maxSteps; i++) {
      const pt = routePoints[i];
      if (!pt) continue;
      const sx = Math.round(pt.x - cameraX);
      const sy = Math.round(pt.y - cameraY);
      if (sx < -8 || sx > viewW + 8 || sy < -8 || sy > viewH + 8) continue;

      const isLit = i === activePulseIdx + 1;
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.fillRect(sx - 2, sy - 2, 4, 4);
      ctx.fillStyle = isLit ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(sx - 1, sy - 1, 2, 2);
    }
  }

  /**
   * Vẽ huy hiệu trạng thái nhiệm vụ sắc nét phía trên đầu NPC và điểm tương tác:
   * - `tracked`: Mũi tên vàng đồng sáng nhấp nhô (`▼`)
   * - `main`: Hình thoi son - vàng đồng nhiệm vụ chính (`◆`)
   * - `optional`: Hình thoi rỗng giấy dó - đồng cổ cho nhánh phụ / tư liệu (`◇`)
   * - `completed`: Ấn chương đỏ son đã hoàn thành (`✓`)
   */
  private drawOverheadQuestMarker(
    x: number,
    y: number,
    kind: 'tracked' | 'main' | 'optional' | 'completed',
    timeSec: number,
    reducedMotion: boolean
  ): void {
    const { ctx } = this;
    const bob =
      reducedMotion || kind === 'completed'
        ? 0
        : Math.round(Math.sin(timeSec * 4.5 + x * 0.05) * 1.5);
    const my = y + bob;

    if (kind === 'tracked') {
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.fillRect(x - 4, my - 4, 8, 7);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 3, my - 3, 6, 2);
      ctx.fillRect(x - 2, my - 1, 4, 2);
      ctx.fillRect(x - 1, my + 1, 2, 2);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(x - 1, my - 2, 2, 2);
    } else if (kind === 'main') {
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.fillRect(x - 3, my - 3, 6, 6);
      ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
      ctx.fillRect(x - 2, my - 2, 4, 4);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(x - 1, my - 3, 2, 6);
      ctx.fillRect(x - 3, my - 1, 6, 2);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(x - 1, my - 1, 2, 2);
    } else if (kind === 'optional') {
      ctx.fillStyle = PALETTE_12.LACQUER_VOID;
      ctx.fillRect(x - 3, my - 3, 6, 6);
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(x - 2, my - 2, 4, 4);
      ctx.fillStyle = PALETTE_12.PARCHMENT_WARM;
      ctx.fillRect(x - 1, my - 1, 2, 2);
    } else {
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(x - 3, my - 2, 6, 5);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(x - 1, my - 1, 2, 2);
    }
  }

  /**
   * Vẽ cọc tiêu chỉ hướng rìa màn hình khi mục tiêu đang theo dõi nằm ngoài khung nhìn camera.
   */
  private drawOffscreenTargetIndicator(
    targetPos: Vec2,
    cameraX: number,
    cameraY: number,
    viewW: number,
    viewH: number,
    timeSec: number,
    reducedMotion: boolean
  ): void {
    const sx = Math.round(targetPos.x - cameraX);
    const sy = Math.round(targetPos.y - cameraY);
    if (sx >= 12 && sx <= viewW - 12 && sy >= 12 && sy <= viewH - 12) {
      return;
    }

    const { ctx } = this;
    const pulse = reducedMotion ? 0 : Math.round(Math.sin(timeSec * 5) * 1);
    const edgeX = Math.max(14, Math.min(viewW - 14, sx)) + pulse;
    const edgeY = Math.max(18, Math.min(viewH - 18, sy));

    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(edgeX - 5, edgeY - 5, 10, 10);
    ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(edgeX - 4, edgeY - 4, 8, 8);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(edgeX - 2, edgeY - 2, 4, 4);
    ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
    ctx.fillRect(edgeX - 1, edgeY - 1, 2, 2);
  }

  /**
   * Vẽ khung kiến trúc đồng cổ 2 lớp (`PALETTE_12.BRONZE_ANTIQUE`) bao quanh biên bản đồ
   * khi bản đồ nội thất nhỏ được căn giữa trong khung nhìn lớn (`cameraX < 0 || cameraY < 0`).
   */
  private drawCenteredMapBorder(
    cameraX: number,
    cameraY: number,
    mapWidthPx: number,
    mapHeightPx: number
  ): void {
    const { ctx } = this;
    const mx = Math.round(-cameraX);
    const my = Math.round(-cameraY);
    const mw = Math.round(mapWidthPx);
    const mh = Math.round(mapHeightPx);

    ctx.save();
    ctx.lineWidth = 1;

    // Khe rãnh tối ngăn cách nền với khung đồng
    ctx.strokeStyle = PALETTE_12.LACQUER_VOID;
    ctx.strokeRect(mx - 1.5, my - 1.5, mw + 3, mh + 3);

    // Đường viền đồng cổ lớp trong (sát biên bản đồ)
    ctx.strokeStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.strokeRect(mx - 0.5, my - 0.5, mw + 1, mh + 1);

    // Đường viền đồng cổ lớp ngoài (cách 1px tạo gờ kiến trúc 2 lớp)
    ctx.strokeStyle = PALETTE_12.BRONZE_ANTIQUE;
    ctx.strokeRect(mx - 2.5, my - 2.5, mw + 5, mh + 5);

    // Chốt góc vàng đồng sáng tại 4 góc khung
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(mx - 3, my - 3, 2, 2);
    ctx.fillRect(mx + mw + 1, my - 3, 2, 2);
    ctx.fillRect(mx - 3, my + mh + 1, 2, 2);
    ctx.fillRect(mx + mw + 1, my + mh + 1, 2, 2);

    ctx.restore();
  }

  private drawEnvironmentalVfx(
    mapData: NormalizedMapData,
    cameraX: number,
    cameraY: number,
    timeSec: number,
    isInterior: boolean,
    reducedMotion: boolean,
    effectsQuality: EffectsQuality = 'full'
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();
    const haloAlpha = effectsQuality === 'subtle' ? 0.1 : 0.18;
    const haloRadius = effectsQuality === 'subtle' ? 20 : 24;
    const ambientCount = effectsQuality === 'subtle' ? 5 : 10;

    ctx.save();

    // 1. Vầng sáng đèn lồng ấm (Warm Lantern & Lamp Pools) quanh Checkpoints và Puzzles
    for (const inter of mapData.interactables) {
      if (inter.kind !== 'checkpoint' && inter.kind !== 'puzzle') continue;
      const lx = Math.round(inter.x - cameraX);
      const ly = Math.round(inter.y - cameraY - 8);
      if (lx < -32 || lx > viewW + 32 || ly < -32 || ly > viewH + 32) continue;
      const halo = ctx.createRadialGradient(lx, ly, 2, lx, ly, haloRadius);
      halo.addColorStop(0, `rgba(212, 168, 79, ${haloAlpha})`);
      halo.addColorStop(1, 'rgba(212, 168, 79, 0)');
      ctx.fillStyle = halo;
      ctx.fillRect(
        lx - haloRadius,
        ly - haloRadius,
        haloRadius * 2,
        haloRadius * 2
      );
    }

    // 2. Hiệu ứng môi trường theo từng Thời kỳ Lịch sử (chỉ khi ngoài trời & không bật giảm chuyển động)
    if (!isInterior && !reducedMotion) {
      for (let i = 0; i < ambientCount; i++) {
        const phase = (timeSec * 2.4 + i * 0.37) % 1;
        const rx = Math.round(((i * 97 + 44) % mapData.widthPx) - cameraX);
        const ry = Math.round(((i * 61 + 120) % mapData.heightPx) - cameraY);
        if (rx < 4 || rx > viewW - 4 || ry < 4 || ry > viewH - 4) continue;

        if (mapData.eraId === '1930') {
          // 1930: Vòng gợn mưa 3-frame trên mặt đá Cửu Long (Hương Cảng)
          const radius = phase < 0.33 ? 1 : phase < 0.66 ? 2 : 3;
          ctx.fillStyle =
            phase < 0.66
              ? 'rgba(230, 229, 220, 0.35)'
              : 'rgba(212, 165, 93, 0.22)';
          ctx.fillRect(rx - radius, ry, 1, 1);
          ctx.fillRect(rx + radius, ry, 1, 1);
          ctx.fillRect(rx, ry - 1, 1, 1);
        } else if (mapData.eraId === '1945') {
          // 1945: Lá đa vàng thu Tân Trào & Nắng thu Ba Đình (#D4A55D)
          const driftX = Math.round(Math.sin(timeSec * 1.8 + i) * 3);
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(rx + driftX, ry, 2, 1);
          ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
          ctx.fillRect(rx + driftX + 1, ry + 1, 1, 1);
        } else if (mapData.eraId === '1954_1975') {
          // 1954–1975: Sương núi Pha Đin & Tàn lửa đuốc hành quân (#DF6B57 / #8A5A2B)
          const riseY = Math.round(phase * 6);
          ctx.fillStyle =
            i % 2 === 0 ? PALETTE_12.CRIMSON_VERMILION : PALETTE_12.BRONZE_ANTIQUE;
          ctx.fillRect(rx, ry - riseY, 2, 1);
        } else {
          // 1986–2045: Hạt sáng lưu trữ số vàng đồng Sơn Mài (#D4A55D / #F4EFE4)
          ctx.fillStyle =
            phase < 0.5 ? PALETTE_12.BRONZE_BRIGHT : PALETTE_12.PARCHMENT_LIGHT;
          ctx.fillRect(rx, ry, 1, 2);
          ctx.fillRect(rx - 1, ry + 1, 3, 1);
        }
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

    // 1. Obstacles (Collision walls/props)
    ctx.fillStyle = 'rgba(196, 58, 43, 0.32)';
    ctx.strokeStyle = '#E05A47';
    for (const obs of map.obstacles) {
      const rx = Math.round(obs.x - camX);
      const ry = Math.round(obs.y - camY);
      ctx.fillRect(rx, ry, obs.width, obs.height);
      ctx.strokeRect(
        rx + 0.5,
        ry + 0.5,
        Math.max(1, obs.width - 1),
        Math.max(1, obs.height - 1)
      );
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
      ctx.strokeRect(
        rx + 0.5,
        ry + 0.5,
        Math.max(1, occ.width - 1),
        Math.max(1, occ.height - 1)
      );
    }

    // 4. Portals (chỉ vẽ khung hình học, nhãn văn bản hiển thị ở #dev-debug-readout)
    ctx.strokeStyle = '#4ADE80';
    for (const p of map.portals) {
      const rx = Math.round(p.x - camX);
      const ry = Math.round(p.y - camY);
      ctx.strokeRect(
        rx + 0.5,
        ry + 0.5,
        Math.max(1, p.width - 1),
        Math.max(1, p.height - 1)
      );
    }

    // 5. Spawns & Checkpoints (dấu chữ thập hình học)
    ctx.strokeStyle = '#38BDF8';
    for (const sp of [...map.spawns, ...map.checkpoints]) {
      const sx = Math.round(sp.x - camX);
      const sy = Math.round(sp.y - camY);
      ctx.beginPath();
      ctx.moveTo(sx - 4, sy);
      ctx.lineTo(sx + 4, sy);
      ctx.moveTo(sx, sy - 4);
      ctx.lineTo(sx, sy + 4);
      ctx.stroke();
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

    ctx.restore();
  }

  private resolveNpcRole(npc: NpcMapPlacement): CharacterAtlasRole {
    const key = `${npc.npcId} ${npc.role}`.toLowerCase();
    if (
      key.includes('guard') ||
      key.includes('patrol') ||
      key.includes('tuần tra')
    ) {
      return 'guard';
    }
    if (
      key.includes('courier') ||
      key.includes('liaison') ||
      key.includes('messenger') ||
      key.includes('signal') ||
      key.includes('giao liên') ||
      key.includes('liên lạc') ||
      key.includes('thông tin')
    ) {
      return 'courier';
    }
    if (
      key.includes('worker') ||
      key.includes('observer') ||
      key.includes('vendor') ||
      key.includes('boatman') ||
      key.includes('watchman') ||
      key.includes('quartermaster') ||
      key.includes('công nhân') ||
      key.includes('phu') ||
      key.includes('bến') ||
      key.includes('lái đò') ||
      key.includes('tiếp vận')
    ) {
      return 'observer';
    }
    if (
      key.includes('archivist') ||
      key.includes('scholar') ||
      key.includes('archive') ||
      key.includes('economist') ||
      key.includes('lưu trữ') ||
      key.includes('học giả')
    ) {
      return 'archivist';
    }
    return 'historian';
  }

  /**
   * Vẽ biểu tượng cảnh báo điểm ảnh (`!` hoặc `?`) sắc nét phía trên đầu lính tuần tra
   * thay cho phông chữ nhỏ bị nhòe.
   */
  private drawPixelAlertIcon(
    gx: number,
    topY: number,
    isAlert: boolean
  ): void {
    const { ctx } = this;
    const bx = Math.round(gx - 4);
    const by = Math.round(topY - 9);

    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(bx, by, 8, 9);

    ctx.fillStyle = isAlert
      ? PALETTE_12.CRIMSON_VERMILION
      : PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(bx, by, 8, 1);
    ctx.fillRect(bx, by + 8, 8, 1);
    ctx.fillRect(bx, by, 1, 9);
    ctx.fillRect(bx + 7, by, 1, 9);

    if (isAlert) {
      // Dấu chấm than (!) 2px đậm rõ
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(bx + 3, by + 2, 2, 3);
      ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
      ctx.fillRect(bx + 3, by + 6, 2, 1);
    } else {
      // Dấu chấm hỏi (?) điểm ảnh
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(bx + 2, by + 2, 4, 1);
      ctx.fillRect(bx + 5, by + 3, 1, 1);
      ctx.fillRect(bx + 3, by + 4, 2, 1);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(bx + 3, by + 6, 2, 1);
    }
  }

  private drawTopDownPatrols(
    mapData: NormalizedMapData,
    chapter: ChapterData,
    cameraX: number,
    cameraY: number,
    timeSec: number,
    patrolActors?: PatrolActorRenderState[],
    reducedMotion = false
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();

    if (patrolActors && patrolActors.length > 0) {
      for (const guard of patrolActors) {
        const gx = Math.round(guard.x - cameraX);
        const gy = Math.round(guard.y - cameraY);
        if (gx < -96 || gx > viewW + 96 || gy < -96 || gy > viewH + 96) {
          continue;
        }

        ctx.save();
        ctx.beginPath();
        ctx.moveTo(gx, gy - 8);
        ctx.arc(
          gx,
          gy - 8,
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

        // Chuyển góc radian sang hướng 4 chiều cho sprite lính tuần tra ('guard')
        const deg = ((guard.facingAngleRad * 180) / Math.PI + 360) % 360;
        const guardFacing: Facing4 =
          deg >= 45 && deg < 135
            ? 'down'
            : deg >= 135 && deg < 225
              ? 'left'
              : deg >= 225 && deg < 315
                ? 'up'
                : 'right';

        drawCharacter4Dir(
          ctx,
          'guard',
          gx,
          gy,
          guardFacing,
          !reducedMotion,
          false,
          timeSec,
          reducedMotion
        );

        // Thanh nghi ngờ & Biểu tượng cảnh báo điểm ảnh (! / ?) trên mũ lính tuần tra
        if (guard.suspicionLevel > 0 || guard.alertState !== 'calm') {
          const barY = gy - 26;
          ctx.fillStyle = PALETTE_12.LACQUER_VOID;
          ctx.fillRect(gx - 7, barY, 14, 3);
          ctx.fillStyle =
            guard.alertState === 'alert'
              ? PALETTE_12.CRIMSON_VERMILION
              : PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(
            gx - 6,
            barY + 1,
            Math.max(2, Math.round(12 * Math.min(1, guard.suspicionLevel))),
            1
          );

          this.drawPixelAlertIcon(gx, barY - 2, guard.alertState === 'alert');
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
      const sweepX =
        h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
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
    isInterior: boolean,
    effectsQuality: EffectsQuality = 'full'
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();
    const scaleFactor = Math.max(
      1,
      Math.hypot(viewW, viewH) / Math.hypot(INTERNAL_WIDTH, INTERNAL_HEIGHT)
    );
    const outerRadius =
      (isInterior ? 125 : 150 + burst * 50) * scaleFactor;
    const innerRadius = Math.round(18 * scaleFactor);

    const cx = Math.max(0, Math.min(viewW, playerScreenX));
    const cy = Math.max(0, Math.min(viewH, playerScreenY - 10));
    const grad = ctx.createRadialGradient(
      cx,
      cy,
      innerRadius,
      cx,
      cy,
      outerRadius
    );
    if (effectsQuality === 'subtle') {
      grad.addColorStop(0, 'rgba(212, 165, 93, 0.06)');
      grad.addColorStop(0.65, 'rgba(17, 14, 11, 0.10)');
      grad.addColorStop(1, 'rgba(17, 14, 11, 0.28)');
    } else {
      grad.addColorStop(0, 'rgba(212, 165, 93, 0.10)');
      grad.addColorStop(0.65, 'rgba(17, 14, 11, 0.14)');
      grad.addColorStop(1, 'rgba(17, 14, 11, 0.42)');
    }
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, viewW, viewH);
  }

  /**
   * Giữ lại hàm nội bộ không hoạt động (no-op) cho tương thích ngược:
   * Nhãn tương tác ngữ cảnh trong gameplay Top-Down nay được dựng trên DOM `#world-interaction-prompt`.
   */
  protected drawPromptBadge(_x: number, _y: number, _text: string): void {
    // No-op: handled by crisp DOM overlay #world-interaction-prompt
  }

  private drawSkyAndChiaroscuro(
    chapter: ChapterData,
    burst: number,
    timeSec: number,
    effectsQuality: EffectsQuality = 'full'
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();
    const radiusScale = Math.max(1, viewW / INTERNAL_WIDTH);

    ctx.fillStyle = PALETTE_12.LACQUER_VOID;
    ctx.fillRect(0, 0, viewW, viewH);

    // Vầng sáng Chiaroscuro tâm khung hình
    const grad = ctx.createRadialGradient(
      viewW * 0.5,
      viewH * 0.42,
      8 * radiusScale,
      viewW * 0.5,
      viewH * 0.42,
      (155 + burst * 60) * radiusScale
    );

    const alphaScale = effectsQuality === 'subtle' ? 0.65 : 1;

    if (burst > 0.1) {
      grad.addColorStop(0, `rgba(212, 165, 93, ${(0.48 * alphaScale).toFixed(2)})`);
      grad.addColorStop(0.5, `rgba(158, 51, 33, ${(0.25 * alphaScale).toFixed(2)})`);
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    } else if (chapter.id === '1945') {
      grad.addColorStop(0, `rgba(212, 165, 93, ${(0.26 * alphaScale).toFixed(2)})`);
      grad.addColorStop(0.6, `rgba(158, 51, 33, ${(0.12 * alphaScale).toFixed(2)})`);
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    } else {
      grad.addColorStop(0, `rgba(212, 165, 93, ${(0.2 * alphaScale).toFixed(2)})`);
      grad.addColorStop(0.65, `rgba(36, 29, 23, ${(0.15 * alphaScale).toFixed(2)})`);
      grad.addColorStop(1, 'rgba(17, 14, 11, 0)');
    }

    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, viewW, viewH);

    // Vòng cầu thiên thể Da Vinci / Trăng cổ điển góc trên
    ctx.strokeStyle = 'rgba(212, 165, 93, 0.16)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(
      Math.round(viewW * 0.81),
      38,
      22 + Math.sin(timeSec) * 1.5,
      0,
      Math.PI * 2
    );
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
    const viewW = this.getViewWidth();
    const offset = Math.floor(cameraX * 0.22);

    ctx.fillStyle = PALETTE_12.LACQUER_DEEP;

    if (chapter.id === '1930') {
      const cols = Math.ceil(viewW / 42) + 2;
      for (let i = 0; i < cols; i++) {
        const bx = i * 42 - (offset % 42);
        const bh = 36 + ((i * 17) % 28);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(bx, GROUND_Y - bh, 34, bh);
        if (i % 2 === 0) {
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          ctx.fillRect(bx + 8, GROUND_Y - bh + 10, 4, 5);
          ctx.fillRect(bx + 18, GROUND_Y - bh + 18, 4, 5);
        }
      }
    } else if (chapter.id === '1945') {
      const cols = Math.ceil(viewW / 56) + 2;
      for (let i = 0; i < cols; i++) {
        const mx = i * 56 - (offset % 56);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.beginPath();
        ctx.moveTo(mx - 20, GROUND_Y);
        ctx.lineTo(mx + 22, GROUND_Y - 58 - (i % 3) * 12);
        ctx.lineTo(mx + 64, GROUND_Y);
        ctx.fill();
      }
    } else if (chapter.id === '1954_1975') {
      const cols = Math.ceil(viewW / 52) + 2;
      for (let i = 0; i < cols; i++) {
        const mx = i * 52 - (offset % 52);
        ctx.fillStyle = PALETTE_12.LACQUER_DEEP;
        ctx.beginPath();
        ctx.moveTo(mx - 15, GROUND_Y);
        ctx.lineTo(mx + 20, GROUND_Y - 65 - (i % 2) * 16);
        ctx.lineTo(mx + 55, GROUND_Y);
        ctx.fill();
      }
    } else {
      const unlockedCount =
        (collected.has('X') ? 1 : 0) +
        (collected.has('XI') ? 1 : 0) +
        (collected.has('XII') ? 1 : 0) +
        (burst > 0 ? 1 : 0);

      const cols = Math.ceil(viewW / 38) + 2;
      for (let i = 0; i < cols; i++) {
        const bx = i * 38 - (offset % 38);
        const baseH = 26 + ((i * 13) % 18);
        const modernBonus = unlockedCount * (10 + (i % 3) * 6);
        const totalH = Math.min(112, baseH + modernBonus);

        ctx.fillStyle =
          unlockedCount >= 2
            ? PALETTE_12.LACQUER_ELEVATED
            : PALETTE_12.LACQUER_DEEP;
        ctx.fillRect(bx, GROUND_Y - totalH, 28, totalH);

        if (unlockedCount >= 1) {
          ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
          for (let wy = GROUND_Y - totalH + 6; wy < GROUND_Y - 12; wy += 8) {
            ctx.fillRect(bx + 5, wy, 3, 4);
            ctx.fillRect(bx + 16, wy, 3, 4);
          }
        }
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
    const viewW = this.getViewWidth();
    const offset = Math.floor(cameraX * 0.55);

    if (chapter.id === '1945') {
      ctx.strokeStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let x = 0; x <= viewW; x += 16) {
        const sag = Math.sin((x + offset) * 0.05) * 4;
        if (x === 0) ctx.moveTo(x, 68 + sag);
        else ctx.lineTo(x, 68 + sag);
      }
      ctx.stroke();

      const pulseX = Math.floor((timeSec * 90) % viewW);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(pulseX, 66, 4, 3);
    } else if (chapter.id === '1986_2045') {
      if (collected.has('XI') || collected.has('XII') || burst > 0) {
        ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
        ctx.fillRect(0, 88, viewW, 4);
        for (let px = 20 - (offset % 60); px < viewW; px += 60) {
          ctx.fillRect(px, 92, 5, GROUND_Y - 92);
        }
        const trainX = Math.floor(((timeSec * 55) % (viewW + 90)) - 45);
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
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();

    ctx.fillStyle = PALETTE_12.LACQUER_ELEVATED;
    ctx.fillRect(0, GROUND_Y, viewW, Math.max(32, viewH - GROUND_Y));

    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(0, GROUND_Y, viewW, 2);

    const tileOffset = Math.floor(cameraX) % 16;
    ctx.fillStyle = PALETTE_12.INK_SECONDARY;
    for (let x = -tileOffset; x < viewW; x += 16) {
      ctx.fillRect(x, GROUND_Y + 6, 10, 2);
      ctx.fillRect(x + 6, GROUND_Y + 14, 10, 2);
    }

    for (const shelter of chapter.shelters) {
      const sx = Math.round(shelter.xStart - cameraX);
      const sw = Math.round(shelter.xEnd - shelter.xStart);
      if (sx + sw < -20 || sx > viewW + 20) continue;

      const playerInside = playerX >= shelter.xStart && playerX <= shelter.xEnd;

      ctx.fillStyle = playerInside
        ? 'rgba(212, 165, 93, 0.18)'
        : 'rgba(36, 29, 23, 0.72)';
      ctx.fillRect(sx, GROUND_Y - 32, sw, 32);

      ctx.fillStyle = playerInside
        ? PALETTE_12.BRONZE_BRIGHT
        : PALETTE_12.BRONZE_MUTED;
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
    const viewW = this.getViewWidth();

    for (const h of chapter.hazards) {
      const sweepX =
        h.xCenter + Math.sin(timeSec * h.speed + h.phaseOffset) * h.radius;
      const screenX = Math.round(sweepX - cameraX);
      if (screenX < -50 || screenX > viewW + 50) continue;

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

      ctx.fillStyle = PALETTE_12.CRIMSON_VERMILION;
      ctx.fillRect(screenX - 18, GROUND_Y - 1, 36, 2);
      ctx.restore();
    }
  }

  private drawMilestonesAndNpcs(
    chapter: ChapterData,
    cameraX: number,
    collected: Set<RomanMilestoneId>,
    _nearby: MilestoneArtifact | null,
    timeSec: number
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();

    for (const m of chapter.milestones) {
      const mx = Math.round(m.worldX - cameraX);
      if (mx < -30 || mx > viewW + 30) continue;

      const isCollected = collected.has(m.roman);

      ctx.fillStyle = isCollected
        ? PALETTE_12.CRIMSON_SEAL
        : PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(mx - 8, GROUND_Y - 10, 16, 10);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(mx - 10, GROUND_Y - 11, 20, 2);

      drawNpcSprite(ctx, mx + 16, GROUND_Y, false);
      drawArtifactSprite(
        ctx,
        m.spriteType,
        mx,
        GROUND_Y - 22,
        isCollected,
        timeSec
      );
    }
  }

  private drawClimaxGate(
    chapter: ChapterData,
    cameraX: number,
    _nearGate: boolean,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const gx = Math.round(chapter.climax.gateWorldX - cameraX);
    if (gx < -40 || gx > viewW + 40) return;

    ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
    ctx.fillRect(gx - 16, GROUND_Y - 54, 6, 54);
    ctx.fillRect(gx + 10, GROUND_Y - 54, 6, 54);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(gx - 20, GROUND_Y - 58, 40, 6);

    const pulse = Math.sin(timeSec * 5) * 2;
    ctx.fillStyle =
      burst > 0 ? PALETTE_12.PARCHMENT_LIGHT : PALETTE_12.CRIMSON_SEAL;
    ctx.fillRect(gx - 8, GROUND_Y - 48, 16, 46);
    ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
    ctx.fillRect(gx - 3 - pulse * 0.5, GROUND_Y - 44, 6 + pulse, 38);
  }

  private drawClimaxSetPiece(
    chapter: ChapterData,
    burst: number,
    timeSec: number
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();
    const centerX = Math.round(viewW * 0.5);
    const stageBaseY = Math.min(viewH - 24, Math.max(GROUND_Y, Math.round(viewH * 0.82)));
    const type = chapter.climax.setPieceType;

    ctx.save();
    if (type === 'kowloon_torch') {
      const flameH = Math.round(35 + burst * 45 + Math.sin(timeSec * 14) * 5);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(centerX - 15, stageBaseY - flameH, 30, flameH);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(centerX - 8, stageBaseY - flameH + 10, 16, flameH - 10);
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(centerX - 4, stageBaseY - flameH + 20, 8, flameH - 22);
    } else if (type === 'badinh_flag') {
      const poleX = centerX;
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(poleX, 28, 3, stageBaseY - 28);
      const flagY = Math.round(stageBaseY - 25 - burst * (stageBaseY - 56));
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(poleX + 3, flagY, 34, 22);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(poleX + 16, flagY + 7, 8, 8);
    } else if (type === 'dienbien_tank') {
      const tankX = Math.round(viewW * 0.12 + burst * viewW * 0.55);
      ctx.fillStyle = PALETTE_12.BRONZE_MUTED;
      ctx.fillRect(tankX, stageBaseY - 12, 32, 12);
      ctx.fillStyle = PALETTE_12.BRONZE_ANTIQUE;
      ctx.fillRect(tankX + 6, stageBaseY - 20, 18, 8);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(tankX + 24, stageBaseY - 18, 14, 3);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(tankX + 10, stageBaseY - 32, 14, 9);
      ctx.fillStyle = PALETTE_12.BRONZE_BRIGHT;
      ctx.fillRect(tankX + 15, stageBaseY - 29, 4, 4);
    } else if (type === 'doimoi_metro') {
      const trainX = Math.round(-60 + burst * (viewW - 40));
      ctx.fillStyle = PALETTE_12.PARCHMENT_LIGHT;
      ctx.fillRect(trainX, 78, 84, 10);
      ctx.fillStyle = PALETTE_12.CRIMSON_SEAL;
      ctx.fillRect(trainX, 83, 84, 3);
      const spacing = Math.max(48, Math.floor((viewW - 90) / 4));
      for (let f = 0; f < 5; f++) {
        const fx = 45 + f * spacing;
        const fy = 34 + (f % 2) * 16;
        const r = Math.round((burst * 18 + f * 3) % 18);
        ctx.fillStyle =
          f % 2 === 0 ? PALETTE_12.BRONZE_BRIGHT : PALETTE_12.CRIMSON_VERMILION;
        ctx.fillRect(fx - r, fy, 2, 2);
        ctx.fillRect(fx + r, fy, 2, 2);
        ctx.fillRect(fx, fy - r, 2, 2);
        ctx.fillRect(fx, fy + r, 2, 2);
      }
    }
    ctx.restore();
  }

  private drawWeatherParticles(
    chapter: ChapterData,
    timeSec: number,
    effectsQuality: EffectsQuality = 'full',
    isTopDown = true
  ): void {
    const { ctx } = this;
    const viewW = this.getViewWidth();
    const viewH = this.getViewHeight();
    const maxY = isTopDown ? viewH : GROUND_Y;
    const particleCount = effectsQuality === 'subtle' ? 14 : 28;

    for (let i = 0; i < particleCount; i++) {
      const px = Math.floor(
        (i * 53 + timeSec * (chapter.id === '1930' ? -45 : 18)) % viewW
      );
      const py = Math.floor(
        (i * 31 + timeSec * (chapter.id === '1930' ? 95 : 28)) % maxY
      );
      const wrappedX = px < 0 ? px + viewW : px;
      const wrappedY = py < 0 ? py + maxY : py;

      if (chapter.id === '1930') {
        ctx.fillStyle = 'rgba(230, 229, 220, 0.32)';
        ctx.fillRect(wrappedX, wrappedY, 1, 4);
      } else if (chapter.id === '1945') {
        ctx.fillStyle = 'rgba(212, 165, 93, 0.55)';
        ctx.fillRect(wrappedX, wrappedY, 2, 2);
      } else if (chapter.id === '1954_1975') {
        ctx.fillStyle =
          i % 2 === 0 ? PALETTE_12.CRIMSON_VERMILION : PALETTE_12.BRONZE_BRIGHT;
        ctx.fillRect(wrappedX, wrappedY, 2, 1);
      } else {
        ctx.fillStyle = 'rgba(212, 165, 93, 0.65)';
        ctx.fillRect(wrappedX, wrappedY, 1, 2);
      }
    }
  }
}
