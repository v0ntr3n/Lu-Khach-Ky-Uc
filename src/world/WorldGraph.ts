/**
 * @file src/world/WorldGraph.ts
 * @description Đồ thị liên thông các khu vực (Zone Topology Graph) cho cả 4 Chương (1930 – 2045):
 * - Chương I (1930): 8 khu vực liên thông tại Hương Cảng (`CHAPTER1_ZONES`).
 * - Chương II (1945): 2 khu vực liên thông (`tan_trao_banyan` <-> `ba_dinh_square`).
 * - Chương III (1954–1975): 2 khu vực liên thông (`pha_din_pass` <-> `spring_1975_corridor`).
 * - Chương IV (1986–2045): 2 khu vực liên thông (`doi_moi_avenue` <-> `nexus_2045_plaza`).
 * - Tổng cộng 14 khu vực (`ALL_ERA_ZONES`).
 */

import type {
  EraId,
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

export const CHAPTER2_ZONES: WorldZoneNode[] = [
  {
    id: 'tan_trao_banyan',
    title: 'Đình Tân Trào & Cây Đa Lịch Sử (08/1945)',
    eraId: '1945',
    districtType: 'hub',
    description:
      'Chiến khu Tân Trào tháng 8/1945, nơi Quốc dân Đại hội thông qua Lệnh Tổng khởi nghĩa và kết nối mạng lưới giao liên toàn quốc.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_tan_trao_banyan',
    checkpointSpawnId: 'default_spawn',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'tan_trao_banyan',
        toZoneId: 'ba_dinh_square',
        portalId: 'portal_to_ba_dinh',
        destinationSpawn: 'from_tan_trao',
        label: 'Tiến Về Quảng Trường Ba Đình',
      },
    ],
  },
  {
    id: 'ba_dinh_square',
    title: 'Quảng Trường Ba Đình Lịch Sử (02/09/1945)',
    eraId: '1945',
    districtType: 'finale',
    description:
      'Quảng trường Ba Đình ngày 02/09/1945, nơi Chủ tịch Hồ Chí Minh đọc bản Tuyên ngôn Độc lập khai sinh nước Việt Nam Dân chủ Cộng hòa.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_ba_dinh_tribune',
    checkpointSpawnId: 'from_tan_trao',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'ba_dinh_square',
        toZoneId: 'tan_trao_banyan',
        portalId: 'portal_to_tan_trao',
        destinationSpawn: 'from_ba_dinh',
        label: 'Về Chiến Khu Tân Trào',
      },
    ],
  },
];

export const CHAPTER3_ZONES: WorldZoneNode[] = [
  {
    id: 'pha_din_pass',
    title: 'Đèo Pha Đin & Tuyến Tiếp Vận Điện Biên Phủ (1954)',
    eraId: '1954_1975',
    districtType: 'hub',
    description:
      'Tuyến đường huyết mạch lên chiến trường Điện Biên Phủ năm 1954, nơi các đoàn dân công xe thồ vượt đèo bảo đảm hậu cần chiến dịch.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_pha_din_depot',
    checkpointSpawnId: 'default_spawn',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'pha_din_pass',
        toZoneId: 'spring_1975_corridor',
        portalId: 'portal_to_spring_1975',
        destinationSpawn: 'from_pha_din',
        label: 'Tiến Tới Hành Lang Chiến Dịch Hồ Chí Minh (1975)',
      },
    ],
  },
  {
    id: 'spring_1975_corridor',
    title: 'Hành Lang Chiến Dịch Hồ Chí Minh (30/04/1975)',
    eraId: '1954_1975',
    districtType: 'finale',
    description:
      'Sở chỉ huy tiền phương và hành lang tiến quân thần tốc mùa Xuân năm 1975, hoàn thành sự nghiệp thống nhất đất nước.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_spring_1975_post',
    checkpointSpawnId: 'from_pha_din',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'spring_1975_corridor',
        toZoneId: 'pha_din_pass',
        portalId: 'portal_to_pha_din',
        destinationSpawn: 'from_spring_1975',
        label: 'Về Đèo Pha Đin',
      },
    ],
  },
];

export const CHAPTER4_ZONES: WorldZoneNode[] = [
  {
    id: 'doi_moi_avenue',
    title: 'Đại Lộ Đổi Mới & Hội Nhập (1986 – 2011)',
    eraId: '1986_2045',
    districtType: 'hub',
    description:
      'Không gian đô thị chuyển mình từ Đại hội VI (1986) qua Cương lĩnh 1991/2011, phản ánh công cuộc Đổi mới toàn diện và hội nhập quốc tế.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_doi_moi_station',
    checkpointSpawnId: 'default_spawn',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'doi_moi_avenue',
        toZoneId: 'nexus_2045_plaza',
        portalId: 'portal_to_nexus_2045',
        destinationSpawn: 'from_doi_moi',
        label: 'Tiến Tới Quảng Trường Kỷ Nguyên Số (2021–2045)',
      },
    ],
  },
  {
    id: 'nexus_2045_plaza',
    title: 'Quảng Trường Kỷ Nguyên Số (2021 – Viễn Cảnh 2045)',
    eraId: '1986_2045',
    districtType: 'finale',
    description:
      'Không gian viễn cảnh 2045 kết nối văn kiện Đại hội XIII (2021) với mục tiêu 100 năm thành lập nước, nơi lưu trữ trọn vẹn ký ức lịch sử.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_nexus_2045_terminal',
    checkpointSpawnId: 'from_doi_moi',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'nexus_2045_plaza',
        toZoneId: 'doi_moi_avenue',
        portalId: 'portal_to_doi_moi',
        destinationSpawn: 'from_nexus_2045',
        label: 'Về Đại Lộ Đổi Mới (1986)',
      },
    ],
  },
];

export const ALL_ERA_ZONES: readonly WorldZoneNode[] = [
  ...CHAPTER1_ZONES,
  ...CHAPTER2_ZONES,
  ...CHAPTER3_ZONES,
  ...CHAPTER4_ZONES,
];

export function getZonesForEra(eraId: EraId): WorldZoneNode[] {
  return ALL_ERA_ZONES.filter((z) => z.eraId === eraId).map((z) => ({
    ...z,
    gridPos: { ...z.gridPos },
    edges: z.edges.map((e) => ({ ...e })),
  }));
}

export function getDefaultZoneForEra(eraId: EraId): string {
  switch (eraId) {
    case '1930':
      return 'harbor_square';
    case '1945':
      return 'tan_trao_banyan';
    case '1954_1975':
      return 'pha_din_pass';
    case '1986_2045':
      return 'doi_moi_avenue';
    default:
      return 'harbor_square';
  }
}

export class WorldGraph {
  private readonly primaryZones: WorldZoneNode[] = [];
  private readonly zonesById = new Map<string, WorldZoneNode>();

  constructor(zones: readonly WorldZoneNode[] = CHAPTER1_ZONES) {
    for (const z of ALL_ERA_ZONES) {
      this.zonesById.set(z.id, {
        ...z,
        gridPos: { ...z.gridPos },
        edges: z.edges.map((e) => ({ ...e })),
      });
    }
    for (const z of zones) {
      const cloned: WorldZoneNode = {
        ...z,
        gridPos: { ...z.gridPos },
        edges: z.edges.map((e) => ({ ...e })),
      };
      this.primaryZones.push(cloned);
      this.zonesById.set(z.id, cloned);
    }
  }

  public getZone(zoneId: string): WorldZoneNode | undefined {
    return this.zonesById.get(zoneId);
  }

  public getAllZones(): WorldZoneNode[] {
    return [...this.primaryZones];
  }

  public getAllEraZones(): WorldZoneNode[] {
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
   * Kiểm định tính toàn vẹn của đồ thị thế giới đối chiếu với tập bản đồ `allMaps`.
   */
  public validateWorldGraph(
    allMaps: Record<string, NormalizedMapData>
  ): { valid: boolean; errors: string[] } {
    const errors: string[] = [];

    const zonesToValidate: WorldZoneNode[] = [...this.primaryZones];
    for (const z of this.zonesById.values()) {
      if (
        !zonesToValidate.some((p) => p.id === z.id) &&
        Boolean(allMaps[z.id])
      ) {
        zonesToValidate.push(z);
      }
    }

    for (const zone of zonesToValidate) {
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
