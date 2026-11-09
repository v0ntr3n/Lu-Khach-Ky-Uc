/**
 * @file src/runtime/MapLoader.ts
 * @description Bộ nạp và chuẩn hóa bản đồ Tiled JSON (.tmj) sang NormalizedMapData & ValidatableMap
 * cho Lữ Khách Ký Ức (M1+). Hỗ trợ cả nạp bất đồng bộ qua fetch() trên trình duyệt và truy xuất
 * đồng bộ (built-in) phục vụ khởi tạo nhanh hoặc kiểm thử tự động.
 */

import type {
  EraId,
  Facing4,
  InteractableDefinition,
  NormalizedMapData,
  NpcMapPlacement,
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
import type { ValidatableMap } from '../world/MapValidator';

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
          const npc: NpcMapPlacement = {
            id: npcId,
            npcId: String(p.npcId || npcId),
            name: String(p.name || p.label || npcId),
            role: String(p.role || 'Nhân chứng lịch sử'),
            x: obj.x,
            y: obj.y,
            facing: (p.facing as Facing4) || 'down',
            dialogueId: String(p.dialogueId || `${ npcId }_dialogue`),
            fictionTag: (p.fictionTag as ProvenanceType) || 'fictional_npc',
            approachRadius: Number(p.approachRadius) || 36,
            milestoneRoman,
          };
          npcs.push(npc);

          // Đồng bộ NPC sang danh sách interactables để kiểm tra Reachability và tương tác phím [E]
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
          const milestoneId = (p.milestoneId || p.milestoneRoman)
            ? (String(p.milestoneId || p.milestoneRoman) as RomanMilestoneId)
            : undefined;

          const item: InteractableDefinition = {
            id: itemId,
            x: obj.x,
            y: obj.y,
            width: obj.width || 16,
            height: obj.height || 16,
            radius: Number(p.radius) || 36,
            kind: (p.kind || obj.type || 'prop') as InteractableDefinition['kind'],
            label: String(p.label || obj.name || itemId),
            actionId: String(p.actionId || `interact_${itemId}`),
            requiresFlag: p.requiresFlag ? String(p.requiresFlag) : undefined,
            grantsFlag: p.grantsFlag ? String(p.grantsFlag) : undefined,
            milestoneId,
            dialogueId: p.dialogueId ? String(p.dialogueId) : undefined,
            evidenceId: p.evidenceId ? String(p.evidenceId) : undefined,
            destinationMap: p.destinationMap ? String(p.destinationMap) : undefined,
            destinationSpawn: p.destinationSpawn ? String(p.destinationSpawn) : undefined,
          };

          if (existingIdx >= 0) {
            interactables[existingIdx] = item;
          } else {
            interactables.push(item);
          }
        }
        continue;
      }

      if (layer.name === 'PatrolPaths') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          const poly = Array.isArray(obj.polyline) && obj.polyline.length > 0
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
              p.warningToast || 'Ánh đèn tuần tra phát hiện! Hãy tìm chỗ ẩn nấp!'
            ),
          });
        }
        continue;
      }

      if (layer.name === 'Checkpoints') {
        for (const obj of objects) {
          const p = extractProps(obj.properties);
          checkpoints.push({
            id: String(obj.name || obj.id),
            x: obj.x,
            y: obj.y,
            facing: (p.facing as Facing4) || 'down',
          });
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
 * Chuyển đổi NormalizedMapData sang ValidatableMap để kiểm tra bằng MapValidator.
 */
export function toValidatableMap(norm: NormalizedMapData): ValidatableMap {
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

function buildHarborSquareRaw(): TiledMapJson {
  const cols = 64;
  const rows = 48;
  const ts = 16;

  const ground = createGrid(cols, rows, 1);
  fillRect(ground, cols, rows, 0, 336, 1024, 96, ts, 2);
  fillRect(ground, cols, rows, 64, 192, 896, 96, ts, 2);
  fillRect(ground, cols, rows, 384, 160, 64, 272, ts, 2);
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
    layers: [
      { id: 1, name: 'Ground', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: ground },
      { id: 2, name: 'GroundDetails', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: groundDetails },
      { id: 3, name: 'LowProps', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: lowProps },
      { id: 4, name: 'Structures', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: structures },
      { id: 5, name: 'Foreground', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: foreground },
      { id: 6, name: 'Collision', type: 'tilelayer', width: cols, height: rows, visible: false, opacity: 0.5, data: collision },
      {
        id: 7,
        name: 'LightOccluders',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
      },
      {
        id: 8,
        name: 'NPCs',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
        ],
      },
      {
        id: 9,
        name: 'Interactables',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
        ],
      },
      {
        id: 10,
        name: 'Entrances',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
        ],
      },
      {
        id: 11,
        name: 'PatrolPaths',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
      },
      {
        id: 12,
        name: 'Checkpoints',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
          {
            id: 601,
            name: 'checkpoint_harbor_hub',
            type: 'checkpoint',
            x: 160,
            y: 384,
            width: 16,
            height: 16,
            properties: [{ name: 'facing', type: 'string', value: 'right' }],
          },
        ],
      },
    ],
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
      { name: 'title', type: 'string', value: 'Kho Hàng Bến Cảng (Nội Thất)' },
      { name: 'eraId', type: 'string', value: '1930' },
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
    layers: [
      { id: 1, name: 'Ground', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: ground },
      { id: 2, name: 'GroundDetails', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: groundDetails },
      { id: 3, name: 'LowProps', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: lowProps },
      { id: 4, name: 'Structures', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: structures },
      { id: 5, name: 'Foreground', type: 'tilelayer', width: cols, height: rows, visible: true, opacity: 1, data: foreground },
      { id: 6, name: 'Collision', type: 'tilelayer', width: cols, height: rows, visible: false, opacity: 0.5, data: collision },
      {
        id: 7,
        name: 'LightOccluders',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
      },
      {
        id: 8,
        name: 'NPCs',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [],
      },
      {
        id: 9,
        name: 'Interactables',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
        ],
      },
      {
        id: 10,
        name: 'Entrances',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
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
        ],
      },
      {
        id: 11,
        name: 'PatrolPaths',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [],
      },
      {
        id: 12,
        name: 'Checkpoints',
        type: 'objectgroup',
        visible: true,
        opacity: 1,
        objects: [
          {
            id: 601,
            name: 'checkpoint_warehouse_entry',
            type: 'checkpoint',
            x: 272,
            y: 384,
            width: 16,
            height: 16,
            properties: [{ name: 'facing', type: 'string', value: 'up' }],
          },
        ],
      },
    ],
  };
}

const BUILTIN_RAW_MAPS: Record<string, TiledMapJson> = {
  harbor_square: buildHarborSquareRaw(),
  warehouse_interior: buildWarehouseInteriorRaw(),
};

const BUILTIN_PARSED_CACHE: Record<string, NormalizedMapData> = {};

/**
 * Truy xuất đồng bộ bản đồ tích hợp sẵn (`harbor_square` hoặc `warehouse_interior`).
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
 * Trả về toàn bộ tập bản đồ tích hợp sẵn đã chuẩn hóa.
 */
export function getAllBuiltInMaps(): Record<string, NormalizedMapData> {
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
      // Fallback sang bản đồ tích hợp sẵn khi chạy trong môi trường không có HTTP server (Vitest / file://)
    }
  }
  return getBuiltInMap(id);
}
