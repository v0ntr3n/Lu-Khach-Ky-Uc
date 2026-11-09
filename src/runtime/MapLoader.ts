/**
 * @file src/runtime/MapLoader.ts
 * @description Bộ nạp và chuẩn hóa bản đồ Tiled JSON (.tmj) sang NormalizedMapData & ValidatableMap
 * cho Lữ Khách Ký Ức (M1 – M6). Bao gồm toàn bộ 8 khu vực liên thông của Chương I (Hương Cảng 1930)
 * và hỗ trợ nạp trước các khu vực kề cận (`preloadConnectedMaps`).
 */

import type {
  EraId,
  Facing4,
  InteractableDefinition,
  NormalizedMapData,
  NpcMapPlacement,
  NpcRoutePoint,
  PatrolDefinition,
  PortalDefinition,
  ProvenanceType,
  Rect2D,
  RomanMilestoneId,
  ShelterZone,
  SpawnPointDefinition,
  TiledMapJson,
  TiledObject,
  TiledProperty,
} from '../types/game';
import {
  registerValidatableMapFallback,
  type ValidatableMap,
} from '../world/MapValidator';
import { CHAPTER1_WORLD_GRAPH } from '../world/WorldGraph';

function extractProps(
  rawProps?: TiledProperty[] | Record<string, string | number | boolean>
): Record<string, string | number | boolean> {
  if (!rawProps) return {};
  if (Array.isArray(rawProps)) {
    const out: Record<string, string | number | boolean> = {};
    for (const p of rawProps) {
      if (p && typeof p.name === 'string') {
        out[p.name] = p.value;
      }
    }
    return out;
  }
  return { ...rawProps };
}

function mergeCollisionTileGrid(
  data: number[],
  cols: number,
  rows: number,
  tileSize: number
): Rect2D[] {
  const solid: boolean[][] = Array.from({ length: rows }, (_, r) =>
    Array.from({ length: cols }, (__, c) => (data[r * cols + c] ?? 0) > 0)
  );
  const used: boolean[][] = Array.from({ length: rows }, () =>
    Array(cols).fill(false)
  );
  const rects: Rect2D[] = [];

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      if (!solid[r][c] || used[r][c]) continue;

      let w = 1;
      while (c + w < cols && solid[r][c + w] && !used[r][c + w]) {
        w++;
      }

      let h = 1;
      outer: while (r + h < rows) {
        for (let k = 0; k < w; k++) {
          if (!solid[r + h][c + k] || used[r + h][c + k]) {
            break outer;
          }
        }
        h++;
      }

      for (let dr = 0; dr < h; dr++) {
        for (let dc = 0; dc < w; dc++) {
          used[r + dr][c + dc] = true;
        }
      }

      rects.push({
        x: c * tileSize,
        y: r * tileSize,
        width: w * tileSize,
        height: h * tileSize,
      });
    }
  }

  return rects;
}

/**
 * Phân tích cú pháp dữ liệu TiledMapJson (.tmj) thành cấu trúc NormalizedMapData chuẩn.
 */
export function parseTiledMap(id: string, raw: TiledMapJson): NormalizedMapData {
  const mapProps = extractProps(raw.properties);
  const widthInTiles = Number(raw.width) || 0;
  const heightInTiles = Number(raw.height) || 0;
  const tileSize = Number(raw.tilewidth) || 16;
  const widthPx = widthInTiles * tileSize;
  const heightPx = heightInTiles * tileSize;

  const mapId = String(mapProps.id || id).trim() || id;
  const title = String(mapProps.title || mapId);
  const eraId = (String(mapProps.eraId || '1930') as EraId) || '1930';
  const districtType = mapProps.districtType
    ? (String(mapProps.districtType) as NormalizedMapData['districtType'])
    : undefined;
  const description = mapProps.description
    ? String(mapProps.description)
    : undefined;

  const tileLayers: Record<string, number[]> = {};
  const obstacles: Rect2D[] = [];
  const shelters: ShelterZone[] = [];
  const occluders: Rect2D[] = [];
  const spawns: SpawnPointDefinition[] = [];
  const portals: PortalDefinition[] = [];
  const npcs: NpcMapPlacement[] = [];
  const interactables: InteractableDefinition[] = [];
  const patrols: PatrolDefinition[] = [];
  const checkpoints: SpawnPointDefinition[] = [];

  const layers = Array.isArray(raw.layers) ? raw.layers : [];

  for (const layer of layers) {
    if (layer.type === 'tilelayer') {
      const layerData = Array.isArray(layer.data) ? [...layer.data] : [];
      tileLayers[layer.name] = layerData;

      if (layer.name === 'Collision' && layerData.length > 0) {
        const merged = mergeCollisionTileGrid(
          layerData,
          widthInTiles,
          heightInTiles,
          tileSize
        );
        obstacles.push(...merged);
      }
      continue;
    }

    if (layer.type === 'objectgroup') {
      const objects: TiledObject[] = Array.isArray(layer.objects)
        ? layer.objects
        : [];

      if (layer.name === 'Collision') {
        for (const obj of objects) {
          obstacles.push({
            x: obj.x,
            y: obj.y,
            width: obj.width || tileSize,
            height: obj.height || tileSize,
          });
        }
        continue;
      }

      if (layer.name === 'LightOccluders') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const rect: Rect2D = {
            x: obj.x,
            y: obj.y,
            width: obj.width || tileSize,
            height: obj.height || tileSize,
          };
          if (obj.type === 'shelter' || p.kind === 'shelter') {
            shelters.push({
              xStart: obj.x,
              xEnd: obj.x + (obj.width || tileSize),
              label: String(p.label || obj.name || 'Mái Hiên'),
              bounds: rect,
            });
          } else {
            occluders.push(rect);
          }
        }
        continue;
      }

      if (layer.name === 'Entrances') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const objId = String(obj.name || obj.id).trim();
          const isPortal =
            obj.type === 'portal' ||
            Boolean(p.destinationMap) ||
            Boolean(p.destinationSpawn);

          if (isPortal) {
            portals.push({
              id: objId,
              x: obj.x,
              y: obj.y,
              width: obj.width || tileSize,
              height: obj.height || tileSize,
              destinationMap: String(p.destinationMap || ''),
              destinationSpawn: String(p.destinationSpawn || ''),
              requiresFlag: p.requiresFlag ? String(p.requiresFlag) : undefined,
              transition: p.transition === 'instant' ? 'instant' : 'fade',
              label: p.label ? String(p.label) : objId,
            });
          } else {
            spawns.push({
              id: objId,
              x: obj.x,
              y: obj.y,
              facing: (p.facing as Facing4) || 'down',
              label: p.label ? String(p.label) : undefined,
            });
          }
        }
        continue;
      }

      if (layer.name === 'NPCs') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const npcId = String(obj.name || p.npcId || obj.id).trim();
          const milestoneRoman = p.milestoneRoman
            ? (String(p.milestoneRoman) as RomanMilestoneId)
            : undefined;

          let route: NpcRoutePoint[] | undefined;
          if (typeof p.routeJson === 'string' && p.routeJson.trim().length > 0) {
            try {
              route = JSON.parse(p.routeJson) as NpcRoutePoint[];
            } catch {
              route = undefined;
            }
          } else if (Array.isArray(obj.polyline) && obj.polyline.length > 0) {
            route = obj.polyline.map((pt) => ({
              x: obj.x + pt.x,
              y: obj.y + pt.y,
              waitSec: Number(p.waitSec) || 1.0,
            }));
          }

          const npc: NpcMapPlacement = {
            id: npcId,
            npcId: String(p.npcId || npcId),
            name: String(p.name || p.label || npcId),
            role: String(p.role || 'Nhân chứng lịch sử'),
            x: obj.x,
            y: obj.y,
            facing: (p.facing as Facing4) || 'down',
            dialogueId: String(p.dialogueId || `${npcId}_dialogue`),
            fictionTag: (p.fictionTag as ProvenanceType) || 'fictional_npc',
            approachRadius: Number(p.approachRadius) || 36,
            milestoneRoman,
            route,
            moveSpeed: p.moveSpeed ? Number(p.moveSpeed) : undefined,
          };
          npcs.push(npc);

          if (!interactables.some((it) => it.id === npc.id)) {
            interactables.push({
              id: npc.id,
              x: npc.x,
              y: npc.y,
              width: obj.width || 16,
              height: obj.height || 16,
              radius: npc.approachRadius,
              kind: 'npc',
              label: npc.name,
              actionId: `talk_${npc.id}`,
              milestoneId: npc.milestoneRoman,
              dialogueId: npc.dialogueId,
            });
          }
        }
        continue;
      }

      if (layer.name === 'Interactables') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const itemId = String(obj.name || obj.id).trim();
          const existingIdx = interactables.findIndex((it) => it.id === itemId);
          const milestoneId =
            p.milestoneId || p.milestoneRoman
              ? (String(p.milestoneId || p.milestoneRoman) as RomanMilestoneId)
              : undefined;

          const item: InteractableDefinition = {
            id: itemId,
            x: obj.x,
            y: obj.y,
            width: obj.width || 16,
            height: obj.height || 16,
            radius: Number(p.radius) || 36,
            kind: (p.kind ||
              obj.type ||
              'prop') as InteractableDefinition['kind'],
            label: String(p.label || obj.name || itemId),
            actionId: String(p.actionId || `interact_${itemId}`),
            requiresFlag: p.requiresFlag ? String(p.requiresFlag) : undefined,
            grantsFlag: p.grantsFlag ? String(p.grantsFlag) : undefined,
            milestoneId,
            dialogueId: p.dialogueId ? String(p.dialogueId) : undefined,
            evidenceId: p.evidenceId ? String(p.evidenceId) : undefined,
            destinationMap: p.destinationMap
              ? String(p.destinationMap)
              : undefined,
            destinationSpawn: p.destinationSpawn
              ? String(p.destinationSpawn)
              : undefined,
          };

          if (existingIdx >= 0) {
            interactables[existingIdx] = item;
          } else {
            interactables.push(item);
          }

          if (
            item.kind === 'checkpoint' &&
            !checkpoints.some((cp) => cp.id === item.id)
          ) {
            checkpoints.push({
              id: item.id,
              x: item.x,
              y: item.y,
              facing: 'down',
              label: item.label,
            });
          }
        }
        continue;
      }

      if (layer.name === 'PatrolPaths') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const poly =
            Array.isArray(obj.polyline) && obj.polyline.length > 0
              ? obj.polyline.map((pt) => ({ x: obj.x + pt.x, y: obj.y + pt.y }))
              : [
                  { x: obj.x, y: obj.y },
                  { x: obj.x + (obj.width || 64), y: obj.y },
                ];
          patrols.push({
            id: String(obj.name || obj.id),
            waypoints: poly,
            speed: Number(p.speed) || 48,
            coneLength: Number(p.coneLength) || 96,
            coneAngleRad: Number(p.coneAngleRad) || 0.75,
            label: String(p.label || obj.name || 'Tuần Tra'),
            warningToast: String(
              p.warningToast ||
                'Ánh đèn tuần tra phát hiện! Hãy tìm chỗ ẩn nấp!'
            ),
          });
        }
        continue;
      }

      if (layer.name === 'Checkpoints') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const cpId = String(obj.name || obj.id);
          if (!checkpoints.some((c) => c.id === cpId)) {
            checkpoints.push({
              id: cpId,
              x: obj.x,
              y: obj.y,
              facing: (p.facing as Facing4) || 'down',
              label: p.label ? String(p.label) : obj.name,
            });
          }
        }
      }
    }
  }

  const defaultSpawnId =
    (mapProps.defaultSpawnId ? String(mapProps.defaultSpawnId) : undefined) ??
    spawns.find((s) => s.id === 'default_spawn')?.id ??
    spawns.find((s) => s.id === 'default')?.id ??
    spawns[0]?.id ??
    'default_spawn';

  return {
    id: mapId,
    title,
    eraId,
    districtType,
    description,
    widthInTiles,
    heightInTiles,
    widthPx,
    heightPx,
    tileSize,
    defaultSpawnId,
    tileLayers,
    obstacles,
    shelters,
    occluders,
    spawns,
    portals,
    npcs,
    interactables,
    patrols,
    checkpoints,
  };
}

/**
 * Bí danh linh hoạt của `parseTiledMap`, hỗ trợ cả `(id, raw)` và `(raw, id?)`.
 */
export function parseTiledMapJson(
  idOrRaw: string | TiledMapJson,
  rawOrId?: TiledMapJson | string
): NormalizedMapData {
  if (typeof idOrRaw === 'string') {
    return parseTiledMap(idOrRaw, rawOrId as TiledMapJson);
  }
  const props = extractProps(idOrRaw.properties);
  const resolvedId =
    typeof rawOrId === 'string' && rawOrId.trim().length > 0
      ? rawOrId
      : String(props.id || 'unknown_map');
  return parseTiledMap(resolvedId, idOrRaw);
}

/**
 * Chuyển đổi NormalizedMapData sang ValidatableMap để kiểm tra bằng MapValidator.
 */
export function toValidatableMap(norm: NormalizedMapData): ValidatableMap {
  // Đảm bảo bộ đệm dự phòng cho 8 bản đồ Chương I luôn sẵn sàng
  ensureFallbackRegistryInitialized();
  return {
    id: norm.id,
    width: norm.widthPx,
    height: norm.heightPx,
    tileSize: norm.tileSize,
    defaultSpawnId: norm.defaultSpawnId,
    spawns: norm.spawns,
    portals: norm.portals,
    interactables: norm.interactables,
    obstacles: norm.obstacles,
  };
}

function createGrid(cols: number, rows: number, defaultVal = 0): number[] {
  return new Array(cols * rows).fill(defaultVal);
}

function fillRect(
  grid: number[],
  cols: number,
  rows: number,
  xPx: number,
  yPx: number,
  wPx: number,
  hPx: number,
  tileSize: number,
  val: number
): void {
  const c0 = Math.max(0, Math.floor(xPx / tileSize));
  const c1 = Math.min(cols - 1, Math.floor((xPx + wPx - 1) / tileSize));
  const r0 = Math.max(0, Math.floor(yPx / tileSize));
  const r1 = Math.min(rows - 1, Math.floor((yPx + hPx - 1) / tileSize));
  for (let r = r0; r <= r1; r++) {
    for (let c = c0; c <= c1; c++) {
      grid[r * cols + c] = val;
    }
  }
}

function makeStandardLayers(
  cols: number,
  rows: number,
  layersInput: {
    ground: number[];
    groundDetails?: number[];
    lowProps?: number[];
    structures?: number[];
    foreground?: number[];
    collision: number[];
    lightOccluders?: TiledObject[];
    npcs?: TiledObject[];
    interactables?: TiledObject[];
    entrances: TiledObject[];
    patrolPaths?: TiledObject[];
    checkpoints?: TiledObject[];
  }
): TiledMapJson['layers'] {
  return [
    {
      id: 1,
      name: 'Ground',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: true,
      opacity: 1,
      data: layersInput.ground,
    },
    {
      id: 2,
      name: 'GroundDetails',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: true,
      opacity: 1,
      data: layersInput.groundDetails ?? createGrid(cols, rows, 0),
    },
    {
      id: 3,
      name: 'LowProps',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: true,
      opacity: 1,
      data: layersInput.lowProps ?? createGrid(cols, rows, 0),
    },
    {
      id: 4,
      name: 'Structures',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: true,
      opacity: 1,
      data: layersInput.structures ?? createGrid(cols, rows, 0),
    },
    {
      id: 5,
      name: 'Foreground',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: true,
      opacity: 1,
      data: layersInput.foreground ?? createGrid(cols, rows, 0),
    },
    {
      id: 6,
      name: 'Collision',
      type: 'tilelayer',
      width: cols,
      height: rows,
      visible: false,
      opacity: 0.5,
      data: layersInput.collision,
    },
    {
      id: 7,
      name: 'LightOccluders',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.lightOccluders ?? [],
    },
    {
      id: 8,
      name: 'NPCs',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.npcs ?? [],
    },
    {
      id: 9,
      name: 'Interactables',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.interactables ?? [],
    },
    {
      id: 10,
      name: 'Entrances',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.entrances,
    },
    {
      id: 11,
      name: 'PatrolPaths',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.patrolPaths ?? [],
    },
    {
      id: 12,
      name: 'Checkpoints',
      type: 'objectgroup',
      visible: true,
      opacity: 1,
      objects: layersInput.checkpoints ?? [],
    },
  ];
}

function buildHarborSquareRaw(): TiledMapJson {
  const cols = 64;
  const rows = 48;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 0, 336, 1024, 96, ts, 2);
  fillRect(ground, cols, rows, 64, 192, 960, 96, ts, 2);
  fillRect(ground, cols, rows, 384, 160, 64, 272, ts, 2);
  fillRect(ground, cols, rows, 800, 336, 96, 304, ts, 2);
  fillRect(ground, cols, rows, 0, 576, 1024, 96, ts, 3);
  fillRect(ground, cols, rows, 0, 672, 1024, 96, ts, 4);

  const groundDetails = createGrid(cols, rows, 0);
  fillRect(groundDetails, cols, rows, 144, 368, 64, 32, ts, 5);
  fillRect(groundDetails, cols, rows, 496, 400, 64, 48, ts, 5);
  fillRect(groundDetails, cols, rows, 912, 352, 64, 64, ts, 6);

  const lowProps = createGrid(cols, rows, 0);
  fillRect(lowProps, cols, rows, 224, 352, 32, 16, ts, 7);
  fillRect(lowProps, cols, rows, 704, 352, 32, 16, ts, 8);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 64, 64, 320, 96, ts, 9);
  fillRect(structures, cols, rows, 448, 64, 384, 96, ts, 9);
  fillRect(structures, cols, rows, 464, 304, 128, 96, ts, 10);
  fillRect(structures, cols, rows, 336, 304, 16, 16, ts, 11);
  fillRect(structures, cols, rows, 656, 304, 16, 16, ts, 11);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 64, 48, 320, 32, ts, 12);
  fillRect(foreground, cols, rows, 448, 48, 384, 32, ts, 12);
  fillRect(foreground, cols, rows, 464, 288, 128, 32, ts, 12);
  fillRect(foreground, cols, rows, 320, 272, 48, 48, ts, 13);
  fillRect(foreground, cols, rows, 640, 272, 48, 48, ts, 13);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 64, 64, 320, 96, ts, 16);
  fillRect(collision, cols, rows, 448, 64, 384, 96, ts, 16);
  fillRect(collision, cols, rows, 464, 304, 128, 96, ts, 16);
  fillRect(collision, cols, rows, 336, 304, 16, 16, ts, 16);
  fillRect(collision, cols, rows, 656, 304, 16, 16, ts, 16);
  fillRect(collision, cols, rows, 0, 672, 1024, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'harbor_square' },
      { name: 'title', type: 'string', value: 'Quảng Trường Bến Cảng Hương Cảng (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'hub' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      groundDetails,
      lowProps,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_awning_west',
          type: 'shelter',
          x: 128,
          y: 160,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Hiệu Sách Cửu Long' }],
        },
        {
          id: 102,
          name: 'shelter_warehouse_eaves',
          type: 'shelter',
          x: 464,
          y: 400,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Kho Hàng Bến Cảng' }],
        },
        {
          id: 103,
          name: 'shelter_pier_shed',
          type: 'shelter',
          x: 704,
          y: 336,
          width: 96,
          height: 64,
          properties: [{ name: 'label', type: 'string', value: 'Lán Tàu Cảng Hương Cảng' }],
        },
        {
          id: 104,
          name: 'occluder_warehouse_wall',
          type: 'occluder',
          x: 464,
          y: 304,
          width: 128,
          height: 96,
          properties: [{ name: 'label', type: 'string', value: 'Tường Kho Hàng' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_courier',
          type: 'npc',
          x: 256,
          y: 368,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_courier' },
            { name: 'name', type: 'string', value: 'Lâm Văn Bính (Giao Liên)' },
            { name: 'role', type: 'string', value: 'Giao liên bí mật Hương Cảng' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'ch1_courier_intro' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'I' },
          ],
        },
        {
          id: 202,
          name: 'npc_archivist',
          type: 'npc',
          x: 480,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_archivist' },
            { name: 'name', type: 'string', value: 'Ông Tư Lưu Trữ' },
            { name: 'role', type: 'string', value: 'Người giữ kho tư liệu Phố Cổ' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'ch1_archivist_sources' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'II' },
          ],
        },
        {
          id: 203,
          name: 'npc_observer',
          type: 'npc',
          x: 736,
          y: 368,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_observer' },
            { name: 'name', type: 'string', value: 'Chú Bảy Hoa Tiêu' },
            { name: 'role', type: 'string', value: 'Người quan sát bến tàu' },
            { name: 'facing', type: 'string', value: 'left' },
            { name: 'dialogueId', type: 'string', value: 'ch1_observer_patrol' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'III' },
          ],
        },
        {
          id: 204,
          name: 'npc_old_worker',
          type: 'npc',
          x: 304,
          y: 480,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_old_worker' },
            { name: 'name', type: 'string', value: 'Bác Khuân Vác Bến Cảng' },
            { name: 'role', type: 'string', value: 'Công nhân bốc xếp Cửu Long' },
            { name: 'facing', type: 'string', value: 'right' },
            { name: 'dialogueId', type: 'string', value: 'ch1_worker_rumor' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'moveSpeed', type: 'float', value: 36 },
            {
              name: 'routeJson',
              type: 'string',
              value: JSON.stringify([
                { x: 304, y: 480, waitSec: 1.0, facing: 'right' },
                { x: 432, y: 480, waitSec: 1.0, facing: 'left' },
              ]),
            },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'climax_gate_1930',
          type: 'climax_gate',
          x: 928,
          y: 368,
          width: 32,
          height: 32,
          properties: [
            { name: 'kind', type: 'string', value: 'climax_gate' },
            { name: 'label', type: 'string', value: 'Cổng Thời Gian 1930' },
            { name: 'actionId', type: 'string', value: 'open_climax' },
            { name: 'requiresFlag', type: 'string', value: 'ch1.all_milestones' },
          ],
        },
        {
          id: 302,
          name: 'prop_harbor_notice',
          type: 'prop',
          x: 384,
          y: 208,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'prop' },
            { name: 'label', type: 'string', value: 'Bảng Cáo Thị Bến Cảng' },
            { name: 'actionId', type: 'string', value: 'inspect_harbor_notice' },
          ],
        },
        {
          id: 303,
          name: 'cp_harbor_lantern',
          type: 'checkpoint',
          x: 192,
          y: 384,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Bến Cảng' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 160,
          y: 384,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_warehouse',
          type: 'spawn',
          x: 512,
          y: 432,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'down' }],
        },
        {
          id: 403,
          name: 'from_old_street',
          type: 'spawn',
          x: 912,
          y: 240,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 404,
          name: 'from_small_pier',
          type: 'spawn',
          x: 832,
          y: 544,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 405,
          name: 'exit_harbor_to_warehouse',
          type: 'portal',
          x: 512,
          y: 400,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'warehouse_interior' },
            { name: 'destinationSpawn', type: 'string', value: 'warehouse_south_door' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Kho Hàng Bến Cảng' },
          ],
        },
        {
          id: 406,
          name: 'portal_to_warehouse',
          type: 'portal',
          x: 512,
          y: 400,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'warehouse_interior' },
            { name: 'destinationSpawn', type: 'string', value: 'from_harbor' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Kho Hàng Số 4' },
          ],
        },
        {
          id: 407,
          name: 'portal_to_old_street',
          type: 'portal',
          x: 960,
          y: 240,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'old_street' },
            { name: 'destinationSpawn', type: 'string', value: 'from_harbor' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Phố Cổ Cửu Long' },
          ],
        },
        {
          id: 408,
          name: 'portal_to_small_pier',
          type: 'portal',
          x: 832,
          y: 608,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'small_pier' },
            { name: 'destinationSpawn', type: 'string', value: 'from_harbor' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Bến Thuyền Nhỏ (Tuyến Phụ)' },
          ],
        },
      ],
      patrolPaths: [
        {
          id: 501,
          name: 'patrol_harbor_alley',
          type: 'patrol',
          x: 560,
          y: 224,
          width: 160,
          height: 32,
          polyline: [
            { x: 0, y: 0 },
            { x: 160, y: 0 },
          ],
          properties: [
            { name: 'speed', type: 'float', value: 48 },
            { name: 'coneLength', type: 'float', value: 96 },
            { name: 'coneAngleRad', type: 'float', value: 0.75 },
            { name: 'label', type: 'string', value: 'Lính Tuần Tra Hương Cảng' },
            {
              name: 'warningToast',
              type: 'string',
              value: 'Ánh đèn tuần tra thực dân phát hiện bóng người! Hãy nấp vào mái hiên!',
            },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_harbor_lantern',
          type: 'checkpoint',
          x: 192,
          y: 384,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildWarehouseInteriorRaw(): TiledMapJson {
  const cols = 36;
  const rows = 28;
  const ts = 16;

  const ground = createGrid(cols, rows, 21);
  fillRect(ground, cols, rows, 240, 160, 96, 256, ts, 22);

  const groundDetails = createGrid(cols, rows, 0);
  fillRect(groundDetails, cols, rows, 256, 192, 64, 48, ts, 23);

  const lowProps = createGrid(cols, rows, 0);
  fillRect(lowProps, cols, rows, 192, 256, 32, 32, ts, 24);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 0, 0, 576, 48, ts, 25);
  fillRect(structures, cols, rows, 0, 48, 32, 368, ts, 25);
  fillRect(structures, cols, rows, 544, 48, 32, 368, ts, 25);
  fillRect(structures, cols, rows, 0, 416, 256, 32, ts, 25);
  fillRect(structures, cols, rows, 320, 416, 256, 32, ts, 25);
  fillRect(structures, cols, rows, 96, 160, 64, 64, ts, 26);
  fillRect(structures, cols, rows, 400, 160, 64, 64, ts, 26);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 32, 112, 512, 16, ts, 27);
  fillRect(foreground, cols, rows, 32, 304, 512, 16, ts, 27);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 0, 0, 576, 48, ts, 32);
  fillRect(collision, cols, rows, 0, 48, 32, 368, ts, 32);
  fillRect(collision, cols, rows, 544, 48, 32, 368, ts, 32);
  fillRect(collision, cols, rows, 0, 416, 256, 32, ts, 32);
  fillRect(collision, cols, rows, 320, 416, 256, 32, ts, 32);
  fillRect(collision, cols, rows, 96, 160, 64, 64, ts, 32);
  fillRect(collision, cols, rows, 400, 160, 64, 64, ts, 32);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'warehouse_interior' },
      {
        name: 'title',
        type: 'string',
        value: 'Kho Hàng Số 4 — Kho Hàng Bến Cảng (Nội Thất)',
      },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'interior' },
      { name: 'defaultSpawnId', type: 'string', value: 'warehouse_south_door' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'interior_tiles',
        image: '/tilesets/ch1/interior_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      groundDetails,
      lowProps,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_crate_shadow',
          type: 'shelter',
          x: 160,
          y: 160,
          width: 64,
          height: 64,
          properties: [{ name: 'label', type: 'string', value: 'Bóng Khuất Kiện Hàng' }],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'evidence_shipping_ledger',
          type: 'evidence',
          x: 272,
          y: 208,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            { name: 'label', type: 'string', value: 'Sổ Vận Đơn Cửu Long' },
            { name: 'actionId', type: 'string', value: 'inspect_shipping_ledger' },
            { name: 'grantsFlag', type: 'string', value: 'ch1.ledger_inspected' },
            { name: 'milestoneId', type: 'string', value: 'II' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_shipping_ledger' },
          ],
        },
        {
          id: 302,
          name: 'prop_cargo_manifest_crate',
          type: 'prop',
          x: 208,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'prop' },
            { name: 'label', type: 'string', value: 'Thùng Hàng Hương Cảng' },
            { name: 'actionId', type: 'string', value: 'inspect_cargo_crate' },
          ],
        },
        {
          id: 303,
          name: 'cp_warehouse_desk',
          type: 'checkpoint',
          x: 320,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Bàn Kiểm Kê Kho Hàng Số 4' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'warehouse_south_door',
          type: 'spawn',
          x: 272,
          y: 384,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 402,
          name: 'default_spawn',
          type: 'spawn',
          x: 272,
          y: 384,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 403,
          name: 'from_harbor',
          type: 'spawn',
          x: 272,
          y: 384,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 404,
          name: 'exit_warehouse_to_harbor',
          type: 'portal',
          x: 272,
          y: 416,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'harbor_square' },
            { name: 'destinationSpawn', type: 'string', value: 'from_warehouse' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Quảng Trường Bến Cảng' },
          ],
        },
        {
          id: 405,
          name: 'portal_to_harbor',
          type: 'portal',
          x: 272,
          y: 416,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'harbor_square' },
            { name: 'destinationSpawn', type: 'string', value: 'from_warehouse' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Quảng Trường Bến Cảng' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_warehouse_desk',
          type: 'checkpoint',
          x: 320,
          y: 272,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
      ],
    }),
  };
}

function buildOldStreetRaw(): TiledMapJson {
  const cols = 48;
  const rows = 40;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 0, 272, 768, 128, ts, 2);
  fillRect(ground, cols, rows, 352, 160, 96, 128, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 64, 64, 256, 96, ts, 9);
  fillRect(structures, cols, rows, 352, 64, 128, 96, ts, 10);
  fillRect(structures, cols, rows, 512, 64, 192, 96, ts, 9);
  fillRect(structures, cols, rows, 128, 464, 512, 96, ts, 9);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 64, 48, 256, 24, ts, 12);
  fillRect(foreground, cols, rows, 352, 48, 128, 24, ts, 12);
  fillRect(foreground, cols, rows, 512, 48, 192, 24, ts, 12);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 64, 64, 256, 96, ts, 16);
  fillRect(collision, cols, rows, 352, 64, 128, 96, ts, 16);
  fillRect(collision, cols, rows, 512, 64, 192, 96, ts, 16);
  fillRect(collision, cols, rows, 128, 464, 512, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'old_street' },
      { name: 'title', type: 'string', value: 'Phố Cổ Cửu Long (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'outdoor' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_tea_stall',
          type: 'shelter',
          x: 160,
          y: 400,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Quán Trà Phố Cổ' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_street_vendor',
          type: 'npc',
          x: 224,
          y: 384,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_street_vendor' },
            { name: 'name', type: 'string', value: 'Bà Năm Quán Nước' },
            { name: 'role', type: 'string', value: 'Chủ quán trà Phố Cổ' },
            { name: 'facing', type: 'string', value: 'up' },
            { name: 'dialogueId', type: 'string', value: 'ch1_vendor_talk' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'II' },
          ],
        },
        {
          id: 202,
          name: 'npc_liaison_inner',
          type: 'npc',
          x: 320,
          y: 288,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_liaison_inner' },
            { name: 'name', type: 'string', value: 'Đồng Chí Thanh (Liên Lạc Nội Tuyến)' },
            { name: 'role', type: 'string', value: 'Giao liên dẫn đường khu Phố Cổ' },
            { name: 'facing', type: 'string', value: 'right' },
            { name: 'dialogueId', type: 'string', value: 'ch1_liaison_inner_talk' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'moveSpeed', type: 'float', value: 40 },
            {
              name: 'routeJson',
              type: 'string',
              value: JSON.stringify([
                { x: 320, y: 288, waitSec: 0.8, facing: 'right' },
                { x: 480, y: 288, waitSec: 0.8, facing: 'left' },
              ]),
            },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'cp_old_street_tea',
          type: 'checkpoint',
          x: 192,
          y: 416,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Quán Trà Phố Cổ (Điểm Lưu)' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
        {
          id: 302,
          name: 'evidence_old_street_leaflet',
          type: 'evidence',
          x: 528,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            { name: 'label', type: 'string', value: 'Truyền Đơn Bến Cảng 1930' },
            { name: 'actionId', type: 'string', value: 'inspect_leaflet' },
            { name: 'grantsFlag', type: 'string', value: 'ch1.leaflet_read' },
            { name: 'milestoneId', type: 'string', value: 'II' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_street_leaflet' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 112,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_harbor',
          type: 'spawn',
          x: 112,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'from_safehouse',
          type: 'spawn',
          x: 384,
          y: 208,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'down' }],
        },
        {
          id: 404,
          name: 'from_shadow_alley',
          type: 'spawn',
          x: 640,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 405,
          name: 'portal_to_harbor_from_street',
          type: 'portal',
          x: 32,
          y: 320,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'harbor_square' },
            { name: 'destinationSpawn', type: 'string', value: 'from_old_street' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Quảng Trường Bến Cảng' },
          ],
        },
        {
          id: 406,
          name: 'portal_to_safehouse',
          type: 'portal',
          x: 384,
          y: 160,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'safehouse_interior' },
            { name: 'destinationSpawn', type: 'string', value: 'from_old_street' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Nhà An Toàn Giao Liên' },
          ],
        },
        {
          id: 407,
          name: 'portal_to_shadow_alley',
          type: 'portal',
          x: 704,
          y: 320,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'shadow_alley' },
            { name: 'destinationSpawn', type: 'string', value: 'from_old_street' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Hẻm Tối Tuần Tra' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_old_street_tea',
          type: 'checkpoint',
          x: 192,
          y: 416,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
      ],
    }),
  };
}

function buildSmallPierRaw(): TiledMapJson {
  const cols = 40;
  const rows = 30;
  const ts = 16;

  const ground = createGrid(cols, rows, 3);
  fillRect(ground, cols, rows, 0, 384, 640, 96, ts, 4);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 256, 160, 96, 64, ts, 10);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 256, 160, 96, 64, ts, 16);
  fillRect(collision, cols, rows, 0, 384, 640, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'small_pier' },
      { name: 'title', type: 'string', value: 'Bến Thuyền Nhỏ — Cảng Thơm (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'optional' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_pier_canopy',
          type: 'shelter',
          x: 160,
          y: 240,
          width: 80,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Bạt Bến Thuyền' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_pier_boatman',
          type: 'npc',
          x: 384,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_pier_boatman' },
            { name: 'name', type: 'string', value: 'Ông Sáu Đò Đêm' },
            { name: 'role', type: 'string', value: 'Người chèo thuyền tam bản' },
            { name: 'facing', type: 'string', value: 'right' },
            { name: 'dialogueId', type: 'string', value: 'ch1_boatman_shortcut' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'moveSpeed', type: 'float', value: 36 },
            {
              name: 'routeJson',
              type: 'string',
              value: JSON.stringify([
                { x: 384, y: 272, waitSec: 1.0, facing: 'right' },
                { x: 480, y: 272, waitSec: 1.0, facing: 'left' },
              ]),
            },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'cp_small_pier_post',
          type: 'checkpoint',
          x: 160,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Cọc Neo Bến Thuyền Nhỏ' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
        {
          id: 302,
          name: 'prop_pier_schedule',
          type: 'evidence',
          x: 416,
          y: 208,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            { name: 'label', type: 'string', value: 'Lịch Trình Tuần Tra Đêm' },
            { name: 'actionId', type: 'string', value: 'inspect_pier_schedule' },
            { name: 'grantsFlag', type: 'string', value: 'ch1.patrol_timing_known' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_patrol_schedule' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 112,
          y: 224,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_harbor',
          type: 'spawn',
          x: 112,
          y: 224,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'from_shadow_alley',
          type: 'spawn',
          x: 512,
          y: 128,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'down' }],
        },
        {
          id: 404,
          name: 'portal_to_harbor_from_pier',
          type: 'portal',
          x: 32,
          y: 224,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'harbor_square' },
            { name: 'destinationSpawn', type: 'string', value: 'from_small_pier' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Quảng Trường Bến Cảng' },
          ],
        },
        {
          id: 405,
          name: 'portal_to_alley_from_pier',
          type: 'portal',
          x: 512,
          y: 64,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'shadow_alley' },
            { name: 'destinationSpawn', type: 'string', value: 'from_small_pier' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Lối Tắt Hẻm Tối' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_small_pier_post',
          type: 'checkpoint',
          x: 160,
          y: 224,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildSafehouseInteriorRaw(): TiledMapJson {
  const cols = 32;
  const rows = 24;
  const ts = 16;

  const ground = createGrid(cols, rows, 21);
  fillRect(ground, cols, rows, 192, 128, 128, 224, ts, 22);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 0, 0, 512, 48, ts, 25);
  fillRect(structures, cols, rows, 0, 48, 32, 336, ts, 25);
  fillRect(structures, cols, rows, 480, 48, 32, 336, ts, 25);
  fillRect(structures, cols, rows, 32, 352, 192, 32, ts, 25);
  fillRect(structures, cols, rows, 288, 352, 192, 32, ts, 25);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 0, 0, 512, 48, ts, 32);
  fillRect(collision, cols, rows, 0, 48, 32, 336, ts, 32);
  fillRect(collision, cols, rows, 480, 48, 32, 336, ts, 32);
  fillRect(collision, cols, rows, 32, 352, 192, 32, ts, 32);
  fillRect(collision, cols, rows, 288, 352, 192, 32, ts, 32);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'safehouse_interior' },
      { name: 'title', type: 'string', value: 'Nhà An Toàn Giao Liên — Cửu Long (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'interior' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'interior_tiles',
        image: '/tilesets/ch1/interior_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      interactables: [
        {
          id: 301,
          name: 'evidence_safehouse_cipher',
          type: 'evidence',
          x: 240,
          y: 160,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            { name: 'label', type: 'string', value: 'Bản Mật Mã Liên Lạc Cửu Long' },
            { name: 'actionId', type: 'string', value: 'inspect_cipher_note' },
            { name: 'grantsFlag', type: 'string', value: 'ch1_safehouse_cipher' },
            { name: 'milestoneId', type: 'string', value: 'II' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_cipher_note' },
          ],
        },
        {
          id: 302,
          name: 'cp_safehouse_lamp',
          type: 'checkpoint',
          x: 160,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Dầu Nhà An Toàn' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 240,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 402,
          name: 'from_old_street',
          type: 'spawn',
          x: 240,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 403,
          name: 'portal_to_old_street_from_safehouse',
          type: 'portal',
          x: 240,
          y: 352,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'old_street' },
            { name: 'destinationSpawn', type: 'string', value: 'from_safehouse' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Phố Cổ Cửu Long' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_safehouse_lamp',
          type: 'checkpoint',
          x: 160,
          y: 224,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
      ],
    }),
  };
}

function buildShadowAlleyRaw(): TiledMapJson {
  const cols = 44;
  const rows = 32;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 16, 224, 672, 96, ts, 2);
  fillRect(ground, cols, rows, 336, 224, 64, 256, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 64, 48, 576, 64, ts, 9);
  fillRect(structures, cols, rows, 224, 160, 48, 48, ts, 10);
  fillRect(structures, cols, rows, 432, 160, 48, 48, ts, 10);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 64, 48, 576, 64, ts, 16);
  fillRect(collision, cols, rows, 224, 160, 48, 48, ts, 16);
  fillRect(collision, cols, rows, 432, 160, 48, 48, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'shadow_alley' },
      { name: 'title', type: 'string', value: 'Hẻm Tối Tuần Tra — Cửu Long (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'stealth' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_alley_alcove_1',
          type: 'shelter',
          x: 192,
          y: 112,
          width: 96,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Hốc Tường Khuất Sáng Tây' }],
        },
        {
          id: 102,
          name: 'shelter_alley_alcove_2',
          type: 'shelter',
          x: 400,
          y: 112,
          width: 96,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Hốc Tường Khuất Sáng Đông' }],
        },
        {
          id: 103,
          name: 'occluder_pillar_west',
          type: 'occluder',
          x: 224,
          y: 160,
          width: 48,
          height: 48,
        },
        {
          id: 104,
          name: 'occluder_pillar_east',
          type: 'occluder',
          x: 432,
          y: 160,
          width: 48,
          height: 48,
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'cp_shadow_alley_arch',
          type: 'checkpoint',
          x: 144,
          y: 192,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Vòm Đá Hẻm Tối' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 80,
          y: 256,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_old_street',
          type: 'spawn',
          x: 80,
          y: 256,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'from_small_pier',
          type: 'spawn',
          x: 352,
          y: 432,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 404,
          name: 'from_archive_courtyard',
          type: 'spawn',
          x: 624,
          y: 256,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 405,
          name: 'portal_to_old_street_from_alley',
          type: 'portal',
          x: 16,
          y: 256,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'old_street' },
            { name: 'destinationSpawn', type: 'string', value: 'from_shadow_alley' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Phố Cổ Cửu Long' },
          ],
        },
        {
          id: 406,
          name: 'portal_to_pier_from_alley',
          type: 'portal',
          x: 352,
          y: 464,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'small_pier' },
            { name: 'destinationSpawn', type: 'string', value: 'from_shadow_alley' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Bến Thuyền Nhỏ' },
          ],
        },
        {
          id: 407,
          name: 'portal_to_archive_courtyard',
          type: 'portal',
          x: 656,
          y: 256,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'archive_courtyard' },
            { name: 'destinationSpawn', type: 'string', value: 'from_shadow_alley' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Sân Kho Lưu Trữ' },
          ],
        },
      ],
      patrolPaths: [
        {
          id: 501,
          name: 'patrol_alley_1',
          type: 'patrol',
          x: 256,
          y: 240,
          width: 224,
          height: 32,
          polyline: [
            { x: 0, y: 0 },
            { x: 224, y: 0 },
          ],
          properties: [
            { name: 'speed', type: 'float', value: 52 },
            { name: 'coneLength', type: 'float', value: 96 },
            { name: 'coneAngleRad', type: 'float', value: 0.72 },
            { name: 'label', type: 'string', value: 'Mật Thám Tuần Tra Hẻm Tối' },
            {
              name: 'warningToast',
              type: 'string',
              value: 'Cảnh giác! Đèn tuần tra Hẻm Tối đang quét qua!',
            },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_shadow_alley_arch',
          type: 'checkpoint',
          x: 144,
          y: 192,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildArchiveCourtyardRaw(): TiledMapJson {
  const cols = 44;
  const rows = 34;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 32, 256, 640, 96, ts, 2);
  fillRect(ground, cols, rows, 320, 176, 96, 128, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 224, 64, 256, 112, ts, 9);
  fillRect(structures, cols, rows, 192, 320, 16, 16, ts, 11);
  fillRect(structures, cols, rows, 496, 320, 16, 16, ts, 11);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 224, 48, 256, 24, ts, 12);
  fillRect(foreground, cols, rows, 176, 288, 48, 48, ts, 13);
  fillRect(foreground, cols, rows, 480, 288, 48, 48, ts, 13);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 224, 64, 256, 112, ts, 16);
  fillRect(collision, cols, rows, 192, 320, 16, 16, ts, 16);
  fillRect(collision, cols, rows, 496, 320, 16, 16, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'archive_courtyard' },
      { name: 'title', type: 'string', value: 'Sân Kho Lưu Trữ Hương Cảng (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'outdoor' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_courtyard_west',
          type: 'shelter',
          x: 160,
          y: 192,
          width: 96,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Tây Sân Kho Lưu Trữ' }],
        },
        {
          id: 102,
          name: 'shelter_courtyard_east',
          type: 'shelter',
          x: 448,
          y: 192,
          width: 96,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Đông Sân Kho Lưu Trữ' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_archive_keeper',
          type: 'npc',
          x: 416,
          y: 240,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_archive_keeper' },
            { name: 'name', type: 'string', value: 'Cụ Đồ Nho (Thủ Từ)' },
            { name: 'role', type: 'string', value: 'Người trông coi Sân Lưu Trữ' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'ch1_keeper_gate' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'III' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'cp_archive_courtyard_bench',
          type: 'checkpoint',
          x: 256,
          y: 320,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Ghế Đá Sân Kho Lưu Trữ' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 112,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_shadow_alley',
          type: 'spawn',
          x: 112,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'from_secret_archive',
          type: 'spawn',
          x: 352,
          y: 216,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'down' }],
        },
        {
          id: 404,
          name: 'portal_to_alley_from_courtyard',
          type: 'portal',
          x: 32,
          y: 288,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'shadow_alley' },
            { name: 'destinationSpawn', type: 'string', value: 'from_archive_courtyard' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Hẻm Tối Tuần Tra' },
          ],
        },
        {
          id: 405,
          name: 'portal_to_secret_archive',
          type: 'portal',
          x: 352,
          y: 176,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'secret_archive' },
            { name: 'destinationSpawn', type: 'string', value: 'from_archive_courtyard' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Phòng Lưu Trữ Bí Mật' },
          ],
        },
      ],
      patrolPaths: [
        {
          id: 501,
          name: 'patrol_courtyard_1',
          type: 'patrol',
          x: 288,
          y: 256,
          width: 160,
          height: 32,
          polyline: [
            { x: 0, y: 0 },
            { x: 160, y: 0 },
          ],
          properties: [
            { name: 'speed', type: 'float', value: 44 },
            { name: 'coneLength', type: 'float', value: 84 },
            { name: 'coneAngleRad', type: 'float', value: 0.65 },
            { name: 'label', type: 'string', value: 'Lính Gác Sân Kho Lưu Trữ' },
            {
              name: 'warningToast',
              type: 'string',
              value: 'Lính gác Sân Lưu Trữ phát hiện! Hãy ẩn vào mái hiên!',
            },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_archive_courtyard_bench',
          type: 'checkpoint',
          x: 256,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
      ],
    }),
  };
}

function buildSecretArchiveRaw(): TiledMapJson {
  const cols = 32;
  const rows = 24;
  const ts = 16;

  const ground = createGrid(cols, rows, 21);
  fillRect(ground, cols, rows, 176, 80, 160, 272, ts, 22);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 0, 0, 512, 48, ts, 25);
  fillRect(structures, cols, rows, 0, 48, 32, 336, ts, 25);
  fillRect(structures, cols, rows, 480, 48, 32, 336, ts, 25);
  fillRect(structures, cols, rows, 32, 352, 192, 32, ts, 25);
  fillRect(structures, cols, rows, 288, 352, 192, 32, ts, 25);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 0, 0, 512, 48, ts, 32);
  fillRect(collision, cols, rows, 0, 48, 32, 336, ts, 32);
  fillRect(collision, cols, rows, 480, 48, 32, 336, ts, 32);
  fillRect(collision, cols, rows, 32, 352, 192, 32, ts, 32);
  fillRect(collision, cols, rows, 288, 352, 192, 32, ts, 32);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'secret_archive' },
      { name: 'title', type: 'string', value: 'Phòng Lưu Trữ Bí Mật — Hội Nghị Hợp Nhất (1930)' },
      { name: 'eraId', type: 'string', value: '1930' },
      { name: 'districtType', type: 'string', value: 'finale' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'interior_tiles',
        image: '/tilesets/ch1/interior_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      interactables: [
        {
          id: 301,
          name: 'puzzle_ch1_unification_dossier',
          type: 'puzzle',
          x: 240,
          y: 176,
          width: 32,
          height: 24,
          properties: [
            { name: 'kind', type: 'string', value: 'puzzle' },
            {
              name: 'label',
              type: 'string',
              value: 'Bàn Đối Chiếu Cương Lĩnh Hợp Nhất (02/1930)',
            },
            { name: 'actionId', type: 'string', value: 'open_dossier_puzzle' },
            { name: 'grantsFlag', type: 'string', value: 'ch1_dossier_verified' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_unification_appeal' },
            { name: 'milestoneId', type: 'string', value: 'III' },
          ],
        },
        {
          id: 302,
          name: 'climax_gate_1930',
          type: 'climax_gate',
          x: 368,
          y: 176,
          width: 32,
          height: 32,
          properties: [
            { name: 'kind', type: 'string', value: 'climax_gate' },
            {
              name: 'label',
              type: 'string',
              value: 'Cổng Thời Gian 1930 — Ngọn Đuốc Cương Lĩnh',
            },
            { name: 'actionId', type: 'string', value: 'trigger_climax' },
            { name: 'milestoneId', type: 'string', value: 'III' },
          ],
        },
        {
          id: 303,
          name: 'cp_secret_archive_table',
          type: 'checkpoint',
          x: 320,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Bàn Lưu Trữ Mật' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
        {
          id: 304,
          name: 'ev_ch1_unification_appeal',
          type: 'evidence',
          x: 160,
          y: 176,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Chính Cương Vắn Tắt & Sách Lược Vắn Tắt (2/1930)',
            },
            { name: 'actionId', type: 'string', value: 'inspect_unification_appeal' },
            { name: 'grantsFlag', type: 'string', value: 'ch1.appeal_collected' },
            { name: 'milestoneId', type: 'string', value: 'III' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch1_unification_appeal' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 240,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 402,
          name: 'from_archive_courtyard',
          type: 'spawn',
          x: 240,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
        {
          id: 403,
          name: 'portal_to_courtyard_from_secret',
          type: 'portal',
          x: 240,
          y: 352,
          width: 32,
          height: 16,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'archive_courtyard' },
            { name: 'destinationSpawn', type: 'string', value: 'from_secret_archive' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Sân Kho Lưu Trữ' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_secret_archive_table',
          type: 'checkpoint',
          x: 320,
          y: 224,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'up' }],
        },
      ],
    }),
  };
}

function buildTanTraoBanyanRaw(): TiledMapJson {
  const cols = 48;
  const rows = 36;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 64, 224, 672, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 224, 64, 320, 96, ts, 9);
  fillRect(structures, cols, rows, 160, 192, 16, 16, ts, 11);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 224, 48, 320, 24, ts, 12);
  fillRect(foreground, cols, rows, 144, 160, 48, 48, ts, 13);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 224, 64, 320, 96, ts, 16);
  fillRect(collision, cols, rows, 160, 192, 16, 16, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'tan_trao_banyan' },
      { name: 'title', type: 'string', value: 'Đình Tân Trào & Cây Đa Lịch Sử (08/1945)' },
      { name: 'eraId', type: 'string', value: '1945' },
      { name: 'districtType', type: 'string', value: 'hub' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_banyan_canopy',
          type: 'shelter',
          x: 128,
          y: 160,
          width: 96,
          height: 64,
          properties: [{ name: 'label', type: 'string', value: 'Tán Cây Đa Tân Trào' }],
        },
        {
          id: 102,
          name: 'shelter_dinh_tan_trao_porch',
          type: 'shelter',
          x: 256,
          y: 160,
          width: 160,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Hiên Đình Tân Trào' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_tan_trao_messenger',
          type: 'npc',
          x: 240,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_tan_trao_messenger' },
            { name: 'name', type: 'string', value: 'Giao Liên Hoàng Việt' },
            { name: 'role', type: 'string', value: 'Giao Liên Chiến Khu Tân Trào' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_tan_trao_messenger' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'IV' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch2_tan_trao_order',
          type: 'evidence',
          x: 368,
          y: 240,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Quân Lệnh Số 1 & Nghị Quyết Quốc Dân Đại Hội Tân Trào',
            },
            { name: 'actionId', type: 'string', value: 'inspect_tan_trao_order' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch2_tan_trao_order' },
            { name: 'milestoneId', type: 'string', value: 'IV' },
            { name: 'grantsFlag', type: 'string', value: 'ch2_read_order' },
          ],
        },
        {
          id: 302,
          name: 'puzzle_ch2_dispatch_network',
          type: 'puzzle',
          x: 496,
          y: 272,
          width: 32,
          height: 24,
          properties: [
            { name: 'kind', type: 'string', value: 'puzzle' },
            {
              name: 'label',
              type: 'string',
              value: 'Trạm Điều Phối Mạng Lưới Giao Liên Tổng Khởi Nghĩa (08/1945)',
            },
            { name: 'actionId', type: 'string', value: 'open_dossier_puzzle' },
            { name: 'grantsFlag', type: 'string', value: 'ch2_network_coordinated' },
            { name: 'milestoneId', type: 'string', value: 'V' },
          ],
        },
        {
          id: 303,
          name: 'cp_tan_trao_banyan',
          type: 'checkpoint',
          x: 192,
          y: 320,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Gốc Đa Tân Trào' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_ba_dinh',
          type: 'spawn',
          x: 640,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 403,
          name: 'portal_to_ba_dinh',
          type: 'portal',
          x: 688,
          y: 288,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'ba_dinh_square' },
            { name: 'destinationSpawn', type: 'string', value: 'from_tan_trao' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Tiến Về Quảng Trường Ba Đình' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_tan_trao_banyan',
          type: 'checkpoint',
          x: 192,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildBaDinhSquareRaw(): TiledMapJson {
  const cols = 52;
  const rows = 36;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 48, 224, 736, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 288, 64, 256, 96, ts, 9);

  const foreground = createGrid(cols, rows, 0);
  fillRect(foreground, cols, rows, 288, 48, 256, 24, ts, 12);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 288, 64, 256, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'ba_dinh_square' },
      { name: 'title', type: 'string', value: 'Quảng Trường Ba Đình Lịch Sử (02/09/1945)' },
      { name: 'eraId', type: 'string', value: '1945' },
      { name: 'districtType', type: 'string', value: 'finale' },
      { name: 'defaultSpawnId', type: 'string', value: 'from_tan_trao' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      foreground,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_ba_dinh_banner',
          type: 'shelter',
          x: 208,
          y: 176,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Khán Đài Quảng Trường Ba Đình' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_ba_dinh_witness',
          type: 'npc',
          x: 352,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_ba_dinh_witness' },
            { name: 'name', type: 'string', value: 'Cụ Nguyễn Văn Thanh' },
            { name: 'role', type: 'string', value: 'Nhân Chứng Quảng Trường Ba Đình' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_ba_dinh_witness' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'VI' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch2_independence_declaration',
          type: 'evidence',
          x: 464,
          y: 256,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            { name: 'label', type: 'string', value: 'Bản Tuyên Ngôn Độc Lập (02/09/1945)' },
            { name: 'actionId', type: 'string', value: 'inspect_declaration' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch2_tan_trao_order' },
            { name: 'milestoneId', type: 'string', value: 'VI' },
            { name: 'grantsFlag', type: 'string', value: 'ch2_read_declaration' },
          ],
        },
        {
          id: 302,
          name: 'climax_gate_1945',
          type: 'climax_gate',
          x: 608,
          y: 288,
          width: 32,
          height: 32,
          properties: [
            { name: 'kind', type: 'string', value: 'climax_gate' },
            {
              name: 'label',
              type: 'string',
              value: 'Cổng Thời Gian 1945 — Lễ Đài Độc Lập Ba Đình',
            },
            { name: 'actionId', type: 'string', value: 'trigger_climax' },
            { name: 'milestoneId', type: 'string', value: 'VI' },
          ],
        },
        {
          id: 303,
          name: 'cp_ba_dinh_tribune',
          type: 'checkpoint',
          x: 240,
          y: 320,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Quảng Trường Ba Đình' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_tan_trao',
          type: 'spawn',
          x: 128,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'portal_to_tan_trao',
          type: 'portal',
          x: 64,
          y: 288,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'tan_trao_banyan' },
            { name: 'destinationSpawn', type: 'string', value: 'from_ba_dinh' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Về Chiến Khu Tân Trào' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_ba_dinh_tribune',
          type: 'checkpoint',
          x: 240,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildPhaDinPassRaw(): TiledMapJson {
  const cols = 52;
  const rows = 36;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 64, 224, 704, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 192, 64, 384, 96, ts, 9);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 192, 64, 384, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'pha_din_pass' },
      { name: 'title', type: 'string', value: 'Đèo Pha Đin & Tuyến Tiếp Vận Điện Biên Phủ (1954)' },
      { name: 'eraId', type: 'string', value: '1954_1975' },
      { name: 'districtType', type: 'string', value: 'hub' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_pha_din_camouflage',
          type: 'shelter',
          x: 208,
          y: 160,
          width: 128,
          height: 56,
          properties: [{ name: 'label', type: 'string', value: 'Lưới Ngụy Trang Đèo Pha Đin' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_pha_din_quartermaster',
          type: 'npc',
          x: 256,
          y: 272,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_pha_din_quartermaster' },
            { name: 'name', type: 'string', value: 'Đội Trưởng Dân Công Bùi Sơn' },
            { name: 'role', type: 'string', value: 'Chỉ Huy Đoàn Xe Thồ Pha Đin' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_pha_din_quartermaster' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'VII' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch3_pha_din_dispatch',
          type: 'evidence',
          x: 384,
          y: 240,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Chỉ Thị "Đánh Chắc, Tiến Chắc" & Sổ Tiếp Vận Điện Biên (1954)',
            },
            { name: 'actionId', type: 'string', value: 'inspect_pha_din_dispatch' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch3_pha_din_dispatch' },
            { name: 'milestoneId', type: 'string', value: 'VII' },
            { name: 'grantsFlag', type: 'string', value: 'ch3_read_dispatch' },
          ],
        },
        {
          id: 302,
          name: 'puzzle_ch3_supply_logistics',
          type: 'puzzle',
          x: 512,
          y: 272,
          width: 32,
          height: 24,
          properties: [
            { name: 'kind', type: 'string', value: 'puzzle' },
            {
              name: 'label',
              type: 'string',
              value: 'Sa Bàn Điều Phối Tuyến Hậu Cần Chiến Lược (1954–1975)',
            },
            { name: 'actionId', type: 'string', value: 'open_dossier_puzzle' },
            { name: 'grantsFlag', type: 'string', value: 'ch3_logistics_allocated' },
            { name: 'milestoneId', type: 'string', value: 'VIII' },
          ],
        },
        {
          id: 303,
          name: 'cp_pha_din_depot',
          type: 'checkpoint',
          x: 192,
          y: 320,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Trạm Tiếp Vận Pha Đin' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_spring_1975',
          type: 'spawn',
          x: 656,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 403,
          name: 'portal_to_spring_1975',
          type: 'portal',
          x: 704,
          y: 288,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'spring_1975_corridor' },
            { name: 'destinationSpawn', type: 'string', value: 'from_pha_din' },
            { name: 'transition', type: 'string', value: 'fade' },
            {
              name: 'label',
              type: 'string',
              value: 'Tiến Tới Hành Lang Chiến Dịch Hồ Chí Minh (1975)',
            },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_pha_din_depot',
          type: 'checkpoint',
          x: 192,
          y: 320,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildSpring1975CorridorRaw(): TiledMapJson {
  const cols = 48;
  const rows = 34;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 48, 208, 672, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 224, 64, 320, 96, ts, 9);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 224, 64, 320, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'spring_1975_corridor' },
      { name: 'title', type: 'string', value: 'Hành Lang Chiến Dịch Hồ Chí Minh (30/04/1975)' },
      { name: 'eraId', type: 'string', value: '1954_1975' },
      { name: 'districtType', type: 'string', value: 'finale' },
      { name: 'defaultSpawnId', type: 'string', value: 'from_pha_din' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_command_bunker',
          type: 'shelter',
          x: 192,
          y: 160,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Lán Chỉ Huy Chiến Dịch' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_1975_signal_officer',
          type: 'npc',
          x: 336,
          y: 256,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_1975_signal_officer' },
            { name: 'name', type: 'string', value: 'Chiến Sĩ Thông Tin Lê Minh' },
            { name: 'role', type: 'string', value: 'Đài Thông Tin Chiến Dịch Hồ Chí Minh' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_1975_signal_officer' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'IX' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch3_campaign_telegram',
          type: 'evidence',
          x: 448,
          y: 240,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Mệnh Lệnh "Thần Tốc, Táo Bạo" — Chiến Dịch Hồ Chí Minh (04/1975)',
            },
            { name: 'actionId', type: 'string', value: 'inspect_campaign_telegram' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch3_pha_din_dispatch' },
            { name: 'milestoneId', type: 'string', value: 'IX' },
            { name: 'grantsFlag', type: 'string', value: 'ch3_read_telegram' },
          ],
        },
        {
          id: 302,
          name: 'climax_gate_1975',
          type: 'climax_gate',
          x: 592,
          y: 272,
          width: 32,
          height: 32,
          properties: [
            { name: 'kind', type: 'string', value: 'climax_gate' },
            {
              name: 'label',
              type: 'string',
              value: 'Cổng Thời Gian 1975 — Đại Thắng Mùa Xuân Thống Nhất',
            },
            { name: 'actionId', type: 'string', value: 'trigger_climax' },
            { name: 'milestoneId', type: 'string', value: 'IX' },
          ],
        },
        {
          id: 303,
          name: 'cp_spring_1975_post',
          type: 'checkpoint',
          x: 224,
          y: 304,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Sở Chỉ Huy Tiền Phương' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_pha_din',
          type: 'spawn',
          x: 128,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'portal_to_pha_din',
          type: 'portal',
          x: 64,
          y: 272,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'pha_din_pass' },
            { name: 'destinationSpawn', type: 'string', value: 'from_spring_1975' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Về Đèo Pha Đin' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_spring_1975_post',
          type: 'checkpoint',
          x: 224,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildDoiMoiAvenueRaw(): TiledMapJson {
  const cols = 48;
  const rows = 34;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 48, 208, 672, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 192, 64, 384, 96, ts, 9);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 192, 64, 384, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'doi_moi_avenue' },
      { name: 'title', type: 'string', value: 'Đại Lộ Đổi Mới & Hội Nhập (1986 – 2011)' },
      { name: 'eraId', type: 'string', value: '1986_2045' },
      { name: 'districtType', type: 'string', value: 'hub' },
      { name: 'defaultSpawnId', type: 'string', value: 'default_spawn' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_doi_moi_arcade',
          type: 'shelter',
          x: 192,
          y: 160,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Hiên Đại Lộ Đổi Mới' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_doi_moi_economist',
          type: 'npc',
          x: 256,
          y: 256,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_doi_moi_economist' },
            { name: 'name', type: 'string', value: 'Chuyên Gia Kinh Tế Trần Hòa' },
            { name: 'role', type: 'string', value: 'Ban Nghiên Cứu Đổi Mới 1986' },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_doi_moi_economist' },
            { name: 'fictionTag', type: 'string', value: 'fictional_npc' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'X' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch4_doi_moi_resolution',
          type: 'evidence',
          x: 368,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Nghị Quyết Đại Hội VI (12/1986) & Cương Lĩnh 1991/2011',
            },
            { name: 'actionId', type: 'string', value: 'inspect_doi_moi_resolution' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch4_doi_moi_resolution' },
            { name: 'milestoneId', type: 'string', value: 'X' },
            { name: 'grantsFlag', type: 'string', value: 'ch4_read_doi_moi' },
          ],
        },
        {
          id: 302,
          name: 'puzzle_ch4_causal_policy',
          type: 'puzzle',
          x: 496,
          y: 256,
          width: 32,
          height: 24,
          properties: [
            { name: 'kind', type: 'string', value: 'puzzle' },
            {
              name: 'label',
              type: 'string',
              value: 'Bảng Đối Chiếu Nhân Quả Chính Sách Đổi Mới (1986–2045)',
            },
            { name: 'actionId', type: 'string', value: 'open_dossier_puzzle' },
            { name: 'grantsFlag', type: 'string', value: 'ch4_policy_linked' },
            { name: 'milestoneId', type: 'string', value: 'XI' },
          ],
        },
        {
          id: 303,
          name: 'cp_doi_moi_station',
          type: 'checkpoint',
          x: 192,
          y: 304,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            { name: 'label', type: 'string', value: 'Đèn Báo An Toàn Đại Lộ Đổi Mới' },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_nexus_2045',
          type: 'spawn',
          x: 624,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'left' }],
        },
        {
          id: 403,
          name: 'portal_to_nexus_2045',
          type: 'portal',
          x: 672,
          y: 272,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'nexus_2045_plaza' },
            { name: 'destinationSpawn', type: 'string', value: 'from_doi_moi' },
            { name: 'transition', type: 'string', value: 'fade' },
            {
              name: 'label',
              type: 'string',
              value: 'Tiến Tới Quảng Trường Kỷ Nguyên Số (2021–2045)',
            },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_doi_moi_station',
          type: 'checkpoint',
          x: 192,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

function buildNexus2045PlazaRaw(): TiledMapJson {
  const cols = 48;
  const rows = 34;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 48, 208, 672, 160, ts, 2);

  const structures = createGrid(cols, rows, 0);
  fillRect(structures, cols, rows, 224, 64, 320, 96, ts, 9);

  const collision = createGrid(cols, rows, 0);
  fillRect(collision, cols, rows, 224, 64, 320, 96, ts, 16);

  return {
    width: cols,
    height: rows,
    tilewidth: ts,
    tileheight: ts,
    orientation: 'orthogonal',
    renderorder: 'right-down',
    properties: [
      { name: 'id', type: 'string', value: 'nexus_2045_plaza' },
      {
        name: 'title',
        type: 'string',
        value: 'Quảng Trường Kỷ Nguyên Số (2021 – Viễn Cảnh 2045)',
      },
      { name: 'eraId', type: 'string', value: '1986_2045' },
      { name: 'districtType', type: 'string', value: 'finale' },
      { name: 'defaultSpawnId', type: 'string', value: 'from_doi_moi' },
    ],
    tilesets: [
      {
        firstgid: 1,
        name: 'harbor_tiles',
        image: '/tilesets/ch1/harbor_tiles.png',
        tilewidth: 16,
        tileheight: 16,
        tilecount: 64,
        columns: 8,
      },
    ],
    layers: makeStandardLayers(cols, rows, {
      ground,
      structures,
      collision,
      lightOccluders: [
        {
          id: 101,
          name: 'shelter_solar_canopy_2045',
          type: 'shelter',
          x: 192,
          y: 160,
          width: 128,
          height: 48,
          properties: [{ name: 'label', type: 'string', value: 'Mái Vòm Năng Lượng 2045' }],
        },
      ],
      npcs: [
        {
          id: 201,
          name: 'npc_archivist_2045',
          type: 'npc',
          x: 336,
          y: 256,
          width: 16,
          height: 16,
          properties: [
            { name: 'npcId', type: 'string', value: 'npc_archivist_2045' },
            { name: 'name', type: 'string', value: 'Lưu Trữ Viên Ký Ức Số Minh Châu' },
            {
              name: 'role',
              type: 'string',
              value: 'Trung Tâm Lưu Trữ Quốc Gia 2045 (Viễn Cảnh)',
            },
            { name: 'facing', type: 'string', value: 'down' },
            { name: 'dialogueId', type: 'string', value: 'dlg_archivist_2045' },
            { name: 'fictionTag', type: 'string', value: 'speculative_2045' },
            { name: 'approachRadius', type: 'int', value: 36 },
            { name: 'milestoneRoman', type: 'string', value: 'XII' },
          ],
        },
      ],
      interactables: [
        {
          id: 301,
          name: 'ev_ch4_vision_2045',
          type: 'evidence',
          x: 448,
          y: 224,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'evidence' },
            {
              name: 'label',
              type: 'string',
              value: 'Văn Kiện Đại Hội XIII (2021) & Tầm Nhìn Việt Nam 2045',
            },
            { name: 'actionId', type: 'string', value: 'inspect_vision_2045' },
            { name: 'evidenceId', type: 'string', value: 'ev_ch4_doi_moi_resolution' },
            { name: 'milestoneId', type: 'string', value: 'XII' },
            { name: 'grantsFlag', type: 'string', value: 'ch4_read_vision_2045' },
          ],
        },
        {
          id: 302,
          name: 'climax_gate_2045',
          type: 'climax_gate',
          x: 592,
          y: 272,
          width: 32,
          height: 32,
          properties: [
            { name: 'kind', type: 'string', value: 'climax_gate' },
            {
              name: 'label',
              type: 'string',
              value: 'Cổng Thời Gian 2045 — Khát Vọng 100 Năm Lập Quốc',
            },
            { name: 'actionId', type: 'string', value: 'trigger_climax' },
            { name: 'milestoneId', type: 'string', value: 'XII' },
          ],
        },
        {
          id: 303,
          name: 'cp_nexus_2045_terminal',
          type: 'checkpoint',
          x: 224,
          y: 304,
          width: 16,
          height: 16,
          properties: [
            { name: 'kind', type: 'string', value: 'checkpoint' },
            {
              name: 'label',
              type: 'string',
              value: 'Trạm Lưu Trữ An Toàn Kỷ Nguyên Số 2045',
            },
            { name: 'actionId', type: 'string', value: 'activate_checkpoint' },
          ],
        },
      ],
      entrances: [
        {
          id: 401,
          name: 'default_spawn',
          type: 'spawn',
          x: 128,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 402,
          name: 'from_doi_moi',
          type: 'spawn',
          x: 128,
          y: 288,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
        {
          id: 403,
          name: 'portal_to_doi_moi',
          type: 'portal',
          x: 64,
          y: 272,
          width: 32,
          height: 32,
          properties: [
            { name: 'destinationMap', type: 'string', value: 'doi_moi_avenue' },
            { name: 'destinationSpawn', type: 'string', value: 'from_nexus_2045' },
            { name: 'transition', type: 'string', value: 'fade' },
            { name: 'label', type: 'string', value: 'Về Đại Lộ Đổi Mới (1986)' },
          ],
        },
      ],
      checkpoints: [
        {
          id: 601,
          name: 'cp_nexus_2045_terminal',
          type: 'checkpoint',
          x: 224,
          y: 304,
          width: 16,
          height: 16,
          properties: [{ name: 'facing', type: 'string', value: 'right' }],
        },
      ],
    }),
  };
}

const BUILTIN_RAW_MAPS: Record<string, TiledMapJson> = {
  harbor_square: buildHarborSquareRaw(),
  warehouse_interior: buildWarehouseInteriorRaw(),
  old_street: buildOldStreetRaw(),
  small_pier: buildSmallPierRaw(),
  safehouse_interior: buildSafehouseInteriorRaw(),
  shadow_alley: buildShadowAlleyRaw(),
  archive_courtyard: buildArchiveCourtyardRaw(),
  secret_archive: buildSecretArchiveRaw(),
  tan_trao_banyan: buildTanTraoBanyanRaw(),
  ba_dinh_square: buildBaDinhSquareRaw(),
  pha_din_pass: buildPhaDinPassRaw(),
  spring_1975_corridor: buildSpring1975CorridorRaw(),
  doi_moi_avenue: buildDoiMoiAvenueRaw(),
  nexus_2045_plaza: buildNexus2045PlazaRaw(),
};

const BUILTIN_PARSED_CACHE: Record<string, NormalizedMapData> = {};
let fallbackRegistryInitialized = false;

function ensureFallbackRegistryInitialized(): void {
  if (fallbackRegistryInitialized) return;
  fallbackRegistryInitialized = true;
  for (const id of Object.keys(BUILTIN_RAW_MAPS)) {
    const norm = getBuiltInMap(id);
    registerValidatableMapFallback({
      id: norm.id,
      width: norm.widthPx,
      height: norm.heightPx,
      tileSize: norm.tileSize,
      defaultSpawnId: norm.defaultSpawnId,
      spawns: norm.spawns,
      portals: norm.portals,
      interactables: norm.interactables,
      obstacles: norm.obstacles,
    });
  }
}

/**
 * Trả về dữ liệu JSON Tiled gốc của 8 bản đồ Chương I.
 */
export function getAllBuiltInRawMaps(): Record<string, TiledMapJson> {
  return { ...BUILTIN_RAW_MAPS };
}

/**
 * Truy xuất đồng bộ một bản đồ tích hợp sẵn của Chương I.
 */
export function getBuiltInMap(id: string): NormalizedMapData {
  if (!BUILTIN_PARSED_CACHE[id]) {
    const raw = BUILTIN_RAW_MAPS[id];
    if (!raw) {
      throw new Error(`Unknown built-in map ID: "${id}"`);
    }
    BUILTIN_PARSED_CACHE[id] = parseTiledMap(id, raw);
  }
  return BUILTIN_PARSED_CACHE[id];
}

/**
 * Trả về toàn bộ 8 bản đồ tích hợp sẵn của Chương I đã chuẩn hóa.
 */
export function getAllBuiltInMaps(): Record<string, NormalizedMapData> {
  ensureFallbackRegistryInitialized();
  const out: Record<string, NormalizedMapData> = {};
  for (const id of Object.keys(BUILTIN_RAW_MAPS)) {
    out[id] = getBuiltInMap(id);
  }
  return out;
}

/**
 * Nạp bản đồ bất đồng bộ từ URL Tiled (.tmj) hoặc dùng bản tích hợp sẵn khi chạy offline / unit test.
 */
export async function fetchAndParseMap(
  id: string,
  url = `/maps/ch1/${id}.tmj`
): Promise<NormalizedMapData> {
  if (typeof fetch === 'function') {
    try {
      const response = await fetch(url);
      if (response.ok) {
        const raw = (await response.json()) as TiledMapJson;
        const parsed = parseTiledMap(id, raw);
        BUILTIN_PARSED_CACHE[id] = parsed;
        return parsed;
      }
    } catch {
      // Fallback sang bản đồ tích hợp sẵn khi chạy trong môi trường không có HTTP server (Vitest / CLI)
    }
  }
  return getBuiltInMap(id);
}

/**
 * Nạp trước (preload) tất cả các bản đồ kề cận với `currentMapId` dựa trên đồ thị `CHAPTER1_WORLD_GRAPH`.
 */
export async function preloadConnectedMaps(
  currentMapId: string
): Promise<NormalizedMapData[]> {
  const connectedZones = CHAPTER1_WORLD_GRAPH.getConnectedZones(currentMapId);
  const loaded: NormalizedMapData[] = [];
  for (const zone of connectedZones) {
    const mapData = await fetchAndParseMap(zone.id);
    loaded.push(mapData);
  }
  return loaded;
}
