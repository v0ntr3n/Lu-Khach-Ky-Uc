/**
 * @file src/world/WorldGraph.ts
 * @description Đồ thị liên thông 8 khu vực (Zone Topology Graph) của Chương I — Hương Cảng 1930:
 * - Quản lý các đỉnh (WorldZoneNode) và cạnh nối (WorldZoneEdge) giữa 8 bản đồ Tiled.
 * - Kiểm tra điều kiện đi qua cổng (`canTraverseEdge`), điều kiện dịch chuyển nhanh hợp lệ giữa các
 *   checkpoint đã mở khóa (`canFastTravel`, có tách biệt chế độ Giảng viên `isPresenterMode`).
 * - Tìm đường đi ngắn nhất bằng BFS (`findShortestRoute`) cho cả tuyến chính và tuyến phụ.
 * - Kiểm định tính toàn vẹn của đồ thị đối chiếu với dữ liệu bản đồ thực tế (`validateWorldGraph`).
 */

import type {
  NormalizedMapData,
  WorldZoneEdge,
  WorldZoneNode,
} from '../types/game';

export const CHAPTER1_ZONES: WorldZoneNode[] = [
  {
    id: 'harbor_square',
    title: 'Quảng Trường Bến Cảng Hương Cảng (1930)',
    eraId: '1930',
    districtType: 'hub',
    description:
      'Trung tâm bến cảng sầm uất của Hương Cảng năm 1930, nơi Lữ Khách gặp Giao liên bí mật và mở đầu hành trình tìm kiếm tư liệu Hội nghị Hợp nhất.',
    gridPos: { x: 1, y: 1 },
    checkpointId: 'cp_harbor_lantern',
    checkpointSpawnId: 'default_spawn',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'harbor_square',
        toZoneId: 'warehouse_interior',
        portalId: 'portal_to_warehouse',
        destinationSpawn: 'from_harbor',
        label: 'Kho Hàng Số 4',
      },
      {
        fromZoneId: 'harbor_square',
        toZoneId: 'old_street',
        portalId: 'portal_to_old_street',
        destinationSpawn: 'from_harbor',
        label: 'Phố Cổ Cửu Long',
      },
      {
        fromZoneId: 'harbor_square',
        toZoneId: 'small_pier',
        portalId: 'portal_to_small_pier',
        destinationSpawn: 'from_harbor',
        isOptionalRoute: true,
        label: 'Bến Thuyền Nhỏ (Tuyến Phụ)',
      },
    ],
  },
  {
    id: 'warehouse_interior',
    title: 'Kho Hàng Số 4 — Kho Hàng Bến Cảng (Nội Thất)',
    eraId: '1930',
    districtType: 'interior',
    description:
      'Kho chứa kiện hàng xuất nhập khẩu tại bến cảng, nơi cất giấu Sổ Vận Đơn Cửu Long ghi dấu chuyến tàu chở đại biểu.',
    gridPos: { x: 0, y: 1 },
    checkpointId: 'cp_warehouse_desk',
    checkpointSpawnId: 'from_harbor',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'warehouse_interior',
        toZoneId: 'harbor_square',
        portalId: 'portal_to_harbor',
        destinationSpawn: 'from_warehouse',
        label: 'Quảng Trường Bến Cảng',
      },
    ],
  },
  {
    id: 'old_street',
    title: 'Phố Cổ Cửu Long (1930)',
    eraId: '1930',
    districtType: 'outdoor',
    description:
      'Dãy phố đèn lồng rực rỡ nhưng ẩn chứa nhiều tai mắt mật thám, nối liền Nhà An Toàn Giao Liên với Hẻm Tối Tuần Tra.',
    gridPos: { x: 2, y: 1 },
    checkpointId: 'cp_old_street_tea',
    checkpointSpawnId: 'from_harbor',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'old_street',
        toZoneId: 'harbor_square',
        portalId: 'portal_to_harbor_from_street',
        destinationSpawn: 'from_old_street',
        label: 'Quảng Trường Bến Cảng',
      },
      {
        fromZoneId: 'old_street',
        toZoneId: 'safehouse_interior',
        portalId: 'portal_to_safehouse',
        destinationSpawn: 'from_old_street',
        label: 'Nhà An Toàn Giao Liên',
      },
      {
        fromZoneId: 'old_street',
        toZoneId: 'shadow_alley',
        portalId: 'portal_to_shadow_alley',
        destinationSpawn: 'from_old_street',
        label: 'Hẻm Tối Tuần Tra',
      },
    ],
  },
  {
    id: 'small_pier',
    title: 'Bến Thuyền Nhỏ — Cảng Thơm (1930)',
    eraId: '1930',
    districtType: 'optional',
    description:
      'Bến đỗ của những chiếc thuyền tam bản nhỏ, cung cấp lịch trình tuần tra đêm và lối tắt vòng qua tuyến phố chính.',
    gridPos: { x: 2, y: 2 },
    checkpointId: 'cp_small_pier_post',
    checkpointSpawnId: 'from_harbor',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'small_pier',
        toZoneId: 'harbor_square',
        portalId: 'portal_to_harbor_from_pier',
        destinationSpawn: 'from_small_pier',
        label: 'Quảng Trường Bến Cảng',
      },
      {
        fromZoneId: 'small_pier',
        toZoneId: 'shadow_alley',
        portalId: 'portal_to_alley_from_pier',
        destinationSpawn: 'from_small_pier',
        isOptionalRoute: true,
        label: 'Lối Tắt Hẻm Tối',
      },
    ],
  },
  {
    id: 'safehouse_interior',
    title: 'Nhà An Toàn Giao Liên — Cửu Long (1930)',
    eraId: '1930',
    districtType: 'interior',
    description:
      'Căn gác nhỏ bí mật của cơ sở giao liên tại Hương Cảng, nơi lưu giữ Bản Mật Mã Liên Lạc Cửu Long.',
    gridPos: { x: 2, y: 0 },
    checkpointId: 'cp_safehouse_lamp',
    checkpointSpawnId: 'from_old_street',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'safehouse_interior',
        toZoneId: 'old_street',
        portalId: 'portal_to_old_street_from_safehouse',
        destinationSpawn: 'from_safehouse',
        label: 'Phố Cổ Cửu Long',
      },
    ],
  },
  {
    id: 'shadow_alley',
    title: 'Hẻm Tối Tuần Tra — Cửu Long (1930)',
    eraId: '1930',
    districtType: 'stealth',
    description:
      'Con hẻm đá hẹp có lính tuần tra cầm đèn rọi quét ngang, đòi hỏi Lữ Khách tận dụng mái hiên và hốc tường khuất sáng để vượt qua.',
    gridPos: { x: 3, y: 1 },
    checkpointId: 'cp_shadow_alley_arch',
    checkpointSpawnId: 'from_old_street',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'shadow_alley',
        toZoneId: 'old_street',
        portalId: 'portal_to_old_street_from_alley',
        destinationSpawn: 'from_shadow_alley',
        label: 'Phố Cổ Cửu Long',
      },
      {
        fromZoneId: 'shadow_alley',
        toZoneId: 'small_pier',
        portalId: 'portal_to_pier_from_alley',
        destinationSpawn: 'from_shadow_alley',
        isOptionalRoute: true,
        label: 'Bến Thuyền Nhỏ',
      },
      {
        fromZoneId: 'shadow_alley',
        toZoneId: 'archive_courtyard',
        portalId: 'portal_to_archive_courtyard',
        destinationSpawn: 'from_shadow_alley',
        label: 'Sân Kho Lưu Trữ',
      },
    ],
  },
  {
    id: 'archive_courtyard',
    title: 'Sân Kho Lưu Trữ Hương Cảng (1930)',
    eraId: '1930',
    districtType: 'outdoor',
    description:
      'Khoảng sân tĩnh lặng dưới bóng cây cổ thụ dẫn vào Phòng Lưu Trữ Bí Mật, nơi kiểm tra các mảnh ghép lịch sử trước khi bước vào hội nghị.',
    gridPos: { x: 4, y: 1 },
    checkpointId: 'cp_archive_courtyard_bench',
    checkpointSpawnId: 'from_shadow_alley',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'archive_courtyard',
        toZoneId: 'shadow_alley',
        portalId: 'portal_to_alley_from_courtyard',
        destinationSpawn: 'from_archive_courtyard',
        label: 'Hẻm Tối Tuần Tra',
      },
      {
        fromZoneId: 'archive_courtyard',
        toZoneId: 'secret_archive',
        portalId: 'portal_to_secret_archive',
        destinationSpawn: 'from_archive_courtyard',
        label: 'Phòng Lưu Trữ Bí Mật',
      },
    ],
  },
  {
    id: 'secret_archive',
    title: 'Phòng Lưu Trữ Bí Mật — Hội Nghị Hợp Nhất (1930)',
    eraId: '1930',
    districtType: 'finale',
    description:
      'Không gian tái hiện Hội nghị Hợp nhất các tổ chức Cộng sản đầu xuân 1930 tại Cửu Long (Hương Cảng), nơi hoàn thiện Hồ sơ Cương lĩnh Chính trị đầu tiên.',
    gridPos: { x: 5, y: 1 },
    checkpointId: 'cp_secret_archive_table',
    checkpointSpawnId: 'from_archive_courtyard',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'secret_archive',
        toZoneId: 'archive_courtyard',
        portalId: 'portal_to_courtyard_from_secret',
        destinationSpawn: 'from_secret_archive',
        label: 'Sân Kho Lưu Trữ',
      },
    ],
  },
];

export class WorldGraph {
  private readonly zonesById = new Map<string, WorldZoneNode>();

  constructor(zones: readonly WorldZoneNode[] = CHAPTER1_ZONES) {
    for (const z of zones) {
      this.zonesById.set(z.id, {
        ...z,
        gridPos: { ...z.gridPos },
        edges: z.edges.map((e) => ({ ...e })),
      });
    }
  }

  public getZone(zoneId: string): WorldZoneNode | undefined {
    return this.zonesById.get(zoneId);
  }

  public getAllZones(): WorldZoneNode[] {
    return Array.from(this.zonesById.values());
  }

  public getConnectedZones(zoneId: string): WorldZoneNode[] {
    const node = this.zonesById.get(zoneId);
    if (!node) return [];
    const connected: WorldZoneNode[] = [];
    const seen = new Set<string>();
    for (const edge of node.edges) {
      if (seen.has(edge.toZoneId)) continue;
      const target = this.zonesById.get(edge.toZoneId);
      if (target) {
        seen.add(edge.toZoneId);
        connected.push(target);
      }
    }
    return connected;
  }

  public canTraverseEdge(
    edge: WorldZoneEdge,
    questFlags: ReadonlySet<string> = new Set()
  ): boolean {
    if (!edge.requiresFlag) {
      return true;
    }
    return questFlags.has(edge.requiresFlag);
  }

  /**
   * Kiểm tra điều kiện dịch chuyển nhanh (Fast Travel) giữa các khu vực trên Bản Đồ Thế Giới (phím M).
   * - Trong chế độ chơi thường: bắt buộc khu vực đích đã được khám phá (`discoveredMaps`) VÀ
   *   điểm lưu (`checkpointId` / `toZoneId`) đã được mở khóa.
   * - Trong chế độ Giảng viên (`isPresenterMode === true`): cho phép di chuyển trình chiếu tới mọi khu vực hợp lệ.
   */
  public canFastTravel(
    _fromZoneId: string,
    toZoneId: string,
    discoveredMaps: ReadonlySet<string>,
    unlockedCheckpoints: ReadonlySet<string>,
    isPresenterMode = false
  ): { allowed: boolean; reason?: string; destinationSpawn: string } {
    const targetZone = this.zonesById.get(toZoneId);
    if (!targetZone) {
      return {
        allowed: false,
        reason: `Khu vực "${toZoneId}" không tồn tại trong đồ thị thế giới.`,
        destinationSpawn: 'default_spawn',
      };
    }

    const destinationSpawn = targetZone.checkpointSpawnId ?? 'default_spawn';

    if (isPresenterMode) {
      return {
        allowed: true,
        destinationSpawn,
      };
    }

    if (!discoveredMaps.has(toZoneId)) {
      return {
        allowed: false,
        reason: 'Khu vực này chưa được khám phá.',
        destinationSpawn,
      };
    }

    const hasCheckpoint =
      unlockedCheckpoints.has(toZoneId) ||
      Boolean(
        targetZone.checkpointId &&
          unlockedCheckpoints.has(targetZone.checkpointId)
      ) ||
      Boolean(
        targetZone.checkpointSpawnId &&
          unlockedCheckpoints.has(targetZone.checkpointSpawnId)
      );

    if (!hasCheckpoint) {
      return {
        allowed: false,
        reason: 'Chưa kích hoạt Điểm Lưu (Checkpoint) tại khu vực này.',
        destinationSpawn,
      };
    }

    return {
      allowed: true,
      destinationSpawn,
    };
  }

  /**
   * Tìm đường đi ngắn nhất (theo số chặng khu vực) từ `fromZoneId` tới `toZoneId` bằng thuật toán BFS.
   */
  public findShortestRoute(
    fromZoneId: string,
    toZoneId: string,
    questFlags?: ReadonlySet<string>,
    includeOptional = true
  ): string[] | null {
    if (!this.zonesById.has(fromZoneId) || !this.zonesById.has(toZoneId)) {
      return null;
    }
    if (fromZoneId === toZoneId) {
      return [fromZoneId];
    }

    const queue: Array<{ id: string; path: string[] }> = [
      { id: fromZoneId, path: [fromZoneId] },
    ];
    const visited = new Set<string>([fromZoneId]);
    let head = 0;

    while (head < queue.length) {
      const current = queue[head++];
      const node = this.zonesById.get(current.id);
      if (!node) continue;

      for (const edge of node.edges) {
        if (!includeOptional && edge.isOptionalRoute) {
          continue;
        }
        if (questFlags && !this.canTraverseEdge(edge, questFlags)) {
          continue;
        }
        if (visited.has(edge.toZoneId)) {
          continue;
        }
        const nextPath = [...current.path, edge.toZoneId];
        if (edge.toZoneId === toZoneId) {
          return nextPath;
        }
        visited.add(edge.toZoneId);
        queue.push({ id: edge.toZoneId, path: nextPath });
      }
    }

    return null;
  }

  /**
   * Kiểm định tính toàn vẹn của đồ thị thế giới đối chiếu với tập bản đồ `allMaps`:
   * - Mọi khu vực trong đồ thị đều tồn tại trong `allMaps`.
   * - Mọi cạnh nối (`edge.portalId`) đều tồn tại trong `allMaps[fromZoneId].portals`.
   * - Mọi điểm đến (`edge.destinationSpawn`) đều tồn tại trong `allMaps[toZoneId].spawns`.
   * - Đảm bảo cả tuyến chính (`includeOptional = false`) và tuyến phụ (`includeOptional = true`)
   *   đều có đường đi hợp lệ từ `harbor_square` tới `secret_archive`.
   */
  public validateWorldGraph(
    allMaps: Record<string, NormalizedMapData>
  ): { valid: boolean; errors: string[] } {
    const errors: string[] = [];

    for (const zone of this.zonesById.values()) {
      const mapData = allMaps[zone.id];
      if (!mapData) {
        errors.push(`Zone "${zone.id}" is missing from loaded maps.`);
        continue;
      }

      if (
        zone.checkpointSpawnId &&
        !mapData.spawns.some((s) => s.id === zone.checkpointSpawnId)
      ) {
        errors.push(
          `Zone "${zone.id}" checkpointSpawnId "${zone.checkpointSpawnId}" does not exist in map spawns.`
        );
      }

      for (const edge of zone.edges) {
        const portal = mapData.portals.find((p) => p.id === edge.portalId);
        if (!portal) {
          errors.push(
            `Zone "${zone.id}" edge to "${edge.toZoneId}" references missing portal "${edge.portalId}".`
          );
        } else {
          if (portal.destinationMap !== edge.toZoneId) {
            errors.push(
              `Zone "${zone.id}" portal "${edge.portalId}" destinationMap ("${portal.destinationMap}") does not match edge toZoneId ("${edge.toZoneId}").`
            );
          }
          if (portal.destinationSpawn !== edge.destinationSpawn) {
            errors.push(
              `Zone "${zone.id}" portal "${edge.portalId}" destinationSpawn ("${portal.destinationSpawn}") does not match edge destinationSpawn ("${edge.destinationSpawn}").`
            );
          }
        }

        const targetMap = allMaps[edge.toZoneId];
        if (!targetMap) {
          errors.push(
            `Zone "${zone.id}" edge references unknown target map "${edge.toZoneId}".`
          );
        } else if (
          !targetMap.spawns.some((s) => s.id === edge.destinationSpawn)
        ) {
          errors.push(
            `Zone "${zone.id}" edge to "${edge.toZoneId}" references missing destinationSpawn "${edge.destinationSpawn}".`
          );
        }
      }
    }

    const mainRoute = this.findShortestRoute(
      'harbor_square',
      'secret_archive',
      undefined,
      false
    );
    if (!mainRoute) {
      errors.push(
        'Main route from "harbor_square" to "secret_archive" (without optional routes) is unreachable.'
      );
    }

    const anyRoute = this.findShortestRoute(
      'harbor_square',
      'secret_archive',
      undefined,
      true
    );
    if (!anyRoute) {
      errors.push(
        'Route from "harbor_square" to "secret_archive" (including optional routes) is unreachable.'
      );
    }

    return {
      valid: errors.length === 0,
      errors,
    };
  }
}

export const CHAPTER1_WORLD_GRAPH = new WorldGraph(CHAPTER1_ZONES);
