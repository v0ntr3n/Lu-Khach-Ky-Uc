/**
 * @file src/systems/InteractionSystem.ts
 * @description Hệ thống dò tìm đối tượng tương tác gần nhất (Portal, NPC, Di vật/Tư liệu, Điểm Lưu Checkpoint, Cổng Thời Gian)
 * trong không gian 2D Top-Down cho "Lữ Khách Ký Ức (1930 – 2045)".
 */

import type {
  ChapterData,
  InteractableDefinition,
  MilestoneArtifact,
  NormalizedMapData,
  NpcMapPlacement,
  PortalDefinition,
  Vec2,
} from '../types/game';

export interface InteractionCandidate {
  id: string;
  kind:
    | 'npc'
    | 'evidence'
    | 'door'
    | 'prop'
    | 'checkpoint'
    | 'puzzle'
    | 'climax_gate'
    | 'portal';
  label: string;
  promptText: string;
  x: number;
  y: number;
  distance: number;
  portal?: PortalDefinition;
  npc?: NpcMapPlacement;
  interactable?: InteractableDefinition;
  milestone?: MilestoneArtifact;
}

export class InteractionSystem {
  public findNearestCandidate(
    playerPos: Vec2,
    mapData: NormalizedMapData,
    chapter: ChapterData
  ): InteractionCandidate | null {
    const candidates: InteractionCandidate[] = [];

    // 1. Kiểm tra các cửa / lối chuyển cảnh (Portals) trong bán kính 28px
    for (const portal of mapData.portals) {
      const cx = portal.x + portal.width * 0.5;
      const cy = portal.y + portal.height * 0.5;
      const centerDist = Math.hypot(playerPos.x - cx, playerPos.y - cy);
      const dx = Math.max(portal.x - playerPos.x, 0, playerPos.x - (portal.x + portal.width));
      const dy = Math.max(portal.y - playerPos.y, 0, playerPos.y - (portal.y + portal.height));
      const rectDist = Math.hypot(dx, dy);
      const dist = Math.min(centerDist, rectDist);

      if (dist <= 28) {
        const label =
          portal.label ||
          (portal.destinationMap === 'warehouse_interior'
            ? 'Nhà Kho Bến Cảng'
            : portal.destinationMap === 'harbor_square'
              ? 'Quảng Trường Bến Cảng'
              : portal.destinationMap);
        candidates.push({
          id: portal.id,
          kind: 'portal',
          label,
          promptText: `[E] Vào: ${label}`,
          x: cx,
          y: cy,
          distance: dist,
          portal,
        });
      }
    }

    // 2. Kiểm tra các NPC trên bản đồ
    for (const npc of mapData.npcs) {
      const radius = npc.approachRadius || 32;
      const dist = Math.hypot(playerPos.x - npc.x, playerPos.y - npc.y);
      if (dist <= radius) {
        const milestone = npc.milestoneRoman
          ? chapter.milestones.find((m) => m.roman === npc.milestoneRoman)
          : undefined;
        candidates.push({
          id: npc.id,
          kind: 'npc',
          label: npc.name,
          promptText: `[E] Trò chuyện: ${npc.name}`,
          x: npc.x,
          y: npc.y,
          distance: dist,
          npc,
          milestone,
        });
      }
    }

    // 3. Kiểm tra các vật thể tương tác (Interactables) trên bản đồ
    for (const item of mapData.interactables) {
      const ix = item.x + (item.width ? item.width * 0.5 : 0);
      const iy = item.y + (item.height ? item.height * 0.5 : 0);
      const radius = item.radius || 32;
      const dist = Math.hypot(playerPos.x - ix, playerPos.y - iy);
      if (dist <= radius) {
        const milestone = item.milestoneId
          ? chapter.milestones.find((m) => m.roman === item.milestoneId)
          : undefined;
        let promptText = `[E] Khôi phục: ${item.label}`;
        if (item.kind === 'door') {
          promptText = `[E] Vào: ${item.label}`;
        } else if (item.kind === 'npc') {
          promptText = `[E] Trò chuyện: ${item.label}`;
        } else if (item.kind === 'checkpoint') {
          promptText = `Nhấn [E] — Kích hoạt Điểm Lưu / Đèn Báo An Toàn: ${item.label}`;
        } else if (item.kind === 'climax_gate') {
          promptText = '[E] Cổng Thời Gian';
        }

        candidates.push({
          id: item.id,
          kind: item.kind,
          label: item.label,
          promptText,
          x: ix,
          y: iy,
          distance: dist,
          interactable: item,
          milestone,
        });
      }
    }

    // 4. Kiểm tra các Cột mốc La Mã của chương hiện tại trên bản đồ tương ứng
    for (const m of chapter.milestones) {
      const targetMapId = m.mapId ?? 'harbor_square';
      if (mapData.id !== targetMapId) continue;

      const mx = m.worldX;
      const my =
        m.worldY ?? (Math.abs(playerPos.y - 148) <= 12 ? playerPos.y : 384);
      const dist = Math.hypot(playerPos.x - mx, playerPos.y - my);
      if (dist <= 32) {
        candidates.push({
          id: `milestone_${m.roman}`,
          kind: 'evidence',
          label: `Mốc ${m.roman} · ${m.title}`,
          promptText: `[E] Khôi phục: Mốc ${m.roman}`,
          x: mx,
          y: my,
          distance: dist,
          milestone: m,
        });
      }
    }

    // 5. Kiểm tra Cổng Thời Gian Cuối Chương (Climax Gate)
    if (mapData.id === 'harbor_square' || mapData.id === 'archive_courtyard') {
      const gx = chapter.climax.gateWorldX;
      const gy =
        chapter.climax.gateWorldY ??
        (Math.abs(playerPos.y - 148) <= 12 ? playerPos.y : 384);
      const gateDist = Math.hypot(playerPos.x - gx, playerPos.y - gy);
      if (gateDist <= 38) {
        candidates.push({
          id: `climax_gate_${chapter.id}`,
          kind: 'climax_gate',
          label: 'Cổng Thời Gian',
          promptText: '[E] Cổng Thời Gian',
          x: gx,
          y: gy,
          distance: gateDist,
        });
      }
    }

    if (candidates.length === 0) {
      return null;
    }

    candidates.sort((a, b) => a.distance - b.distance);
    return candidates[0];
  }
}
