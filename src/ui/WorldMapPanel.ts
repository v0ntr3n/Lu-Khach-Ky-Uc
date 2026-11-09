/**
 * @file src/ui/WorldMapPanel.ts
 * @description Giao diện Sơ Đồ Liên Thông Khu Vực (Phím M) theo phong cách Sơn Mài & Giấy Dó.
 * Chỉ hiển thị chi tiết các khu vực đã khám phá, cho phép Dịch chuyển nhanh (Fast Travel)
 * giữa các Đèn Báo An Toàn (Checkpoint) đã kích hoạt, và bảo mật khu vực chưa mở.
 */

import type { WorldZoneNode } from '../types/game';

export const CHAPTER1_WORLD_ZONES: WorldZoneNode[] = [
  {
    id: 'small_pier',
    title: 'Cầu Tàu Nhỏ Đêm Mưa (Small Pier)',
    eraId: '1930',
    districtType: 'optional',
    description: 'Mũi cầu tàu gỗ vươn ra mặt nước cảng Cửu Long, nơi cất giấu kiện tư liệu mật.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_small_pier_post',
    checkpointSpawnId: 'cp_small_pier_post',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'small_pier',
        toZoneId: 'harbor_square',
        portalId: 'portal_pier_to_harbor',
        destinationSpawn: 'from_small_pier',
        isOptionalRoute: true,
        label: 'Quảng trường Bến Cảng',
      },
    ],
  },
  {
    id: 'harbor_square',
    title: 'Quảng Trường Bến Cảng (Harbor Square)',
    eraId: '1930',
    districtType: 'hub',
    description: 'Trung tâm bến cảng Cửu Long (Hương Cảng) đêm mưa tháng 2/1930.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_harbor_lantern',
    checkpointSpawnId: 'cp_harbor_lantern',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'harbor_square',
        toZoneId: 'warehouse_interior',
        portalId: 'portal_to_warehouse',
        destinationSpawn: 'from_harbor',
        label: 'Kho Hàng Lưu Trữ',
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
        label: 'Cầu Tàu Nhỏ',
      },
    ],
  },
  {
    id: 'warehouse_interior',
    title: 'Kho Hàng Lưu Trữ (Warehouse Interior)',
    eraId: '1930',
    districtType: 'interior',
    description: 'Kho gỗ kín gió sát bến cảng, nơi bàn kiểm định sổ bộ và ấn chương mật.',
    gridPos: { x: 2, y: 0 },
    checkpointId: 'cp_warehouse_desk',
    checkpointSpawnId: 'cp_warehouse_desk',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'warehouse_interior',
        toZoneId: 'harbor_square',
        portalId: 'portal_to_harbor',
        destinationSpawn: 'from_warehouse',
        label: 'Quảng trường Bến Cảng',
      },
    ],
  },
  {
    id: 'safehouse_interior',
    title: 'Nhà An Toàn Bí Mật (Safehouse Interior)',
    eraId: '1930',
    districtType: 'interior',
    description: 'Căn gác nhỏ của cơ sở liên lạc Cửu Long, lưu giữ bản thảo Cương lĩnh đầu tiên.',
    gridPos: { x: 0, y: 1 },
    checkpointId: 'cp_safehouse_lamp',
    checkpointSpawnId: 'cp_safehouse_lamp',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'safehouse_interior',
        toZoneId: 'old_street',
        portalId: 'portal_safehouse_to_street',
        destinationSpawn: 'from_safehouse',
        label: 'Phố Cổ Cửu Long',
      },
    ],
  },
  {
    id: 'old_street',
    title: 'Phố Cổ Cửu Long (Old Street)',
    eraId: '1930',
    districtType: 'outdoor',
    description: 'Tuyến phố đèn lồng đỏ nối bến cảng với Nhà An Toàn và Hẻm Tối tuần tra.',
    gridPos: { x: 1, y: 1 },
    checkpointId: 'cp_old_street_tea',
    checkpointSpawnId: 'cp_old_street_tea',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'old_street',
        toZoneId: 'harbor_square',
        portalId: 'portal_street_to_harbor',
        destinationSpawn: 'from_old_street',
        label: 'Quảng trường Bến Cảng',
      },
      {
        fromZoneId: 'old_street',
        toZoneId: 'safehouse_interior',
        portalId: 'portal_street_to_safehouse',
        destinationSpawn: 'from_old_street',
        label: 'Nhà An Toàn',
      },
      {
        fromZoneId: 'old_street',
        toZoneId: 'shadow_alley',
        portalId: 'portal_street_to_alley',
        destinationSpawn: 'from_old_street',
        label: 'Hẻm Tối Tuần Tra',
      },
    ],
  },
  {
    id: 'shadow_alley',
    title: 'Hẻm Tối Tuần Tra (Shadow Alley)',
    eraId: '1930',
    districtType: 'stealth',
    description: 'Lối tắt nhiều mái hiên che bóng tối dẫn sang Sân Viện Lưu Trữ.',
    gridPos: { x: 2, y: 1 },
    checkpointId: 'cp_shadow_alley_arch',
    checkpointSpawnId: 'cp_shadow_alley_arch',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'shadow_alley',
        toZoneId: 'old_street',
        portalId: 'portal_alley_to_street',
        destinationSpawn: 'from_shadow_alley',
        label: 'Phố Cổ Cửu Long',
      },
      {
        fromZoneId: 'shadow_alley',
        toZoneId: 'archive_courtyard',
        portalId: 'portal_alley_to_courtyard',
        destinationSpawn: 'from_shadow_alley',
        label: 'Sân Viện Lưu Trữ',
      },
    ],
  },
  {
    id: 'archive_courtyard',
    title: 'Sân Viện Lưu Trữ (Archive Courtyard)',
    eraId: '1930',
    districtType: 'outdoor',
    description: 'Sân đá cổ tĩnh mịch dưới tán cây đại thụ trước thềm Phòng Hội Nghị Hợp Nhất.',
    gridPos: { x: 1, y: 2 },
    checkpointId: 'cp_archive_courtyard_bench',
    checkpointSpawnId: 'cp_archive_courtyard_bench',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'archive_courtyard',
        toZoneId: 'shadow_alley',
        portalId: 'portal_courtyard_to_alley',
        destinationSpawn: 'from_courtyard',
        label: 'Hẻm Tối Tuần Tra',
      },
      {
        fromZoneId: 'archive_courtyard',
        toZoneId: 'secret_archive',
        portalId: 'portal_courtyard_to_secret',
        destinationSpawn: 'from_courtyard',
        label: 'Phòng Hội Nghị Hợp Nhất',
      },
    ],
  },
  {
    id: 'secret_archive',
    title: 'Phòng Hội Nghị Hợp Nhất (Secret Archive)',
    eraId: '1930',
    districtType: 'finale',
    description: 'Không gian lịch sử nơi thông qua Cương lĩnh chính trị đầu tiên và mở Cổng Thời Gian.',
    gridPos: { x: 2, y: 2 },
    checkpointId: 'cp_secret_archive_table',
    checkpointSpawnId: 'cp_secret_archive_table',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'secret_archive',
        toZoneId: 'archive_courtyard',
        portalId: 'portal_secret_to_courtyard',
        destinationSpawn: 'from_secret_archive',
        label: 'Sân Viện Lưu Trữ',
      },
    ],
  },
];

export const ALL_WORLD_ZONES: WorldZoneNode[] = [
  ...CHAPTER1_WORLD_ZONES,
  {
    id: 'tan_trao_banyan',
    title: 'Đình Tân Trào & Cây Đa Lịch Sử (8/1945)',
    eraId: '1945',
    districtType: 'hub',
    description: 'Thủ đô Khu giải phóng Việt Bắc, nơi phát lệnh Tổng khởi nghĩa Tháng Tám 1945.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_tan_trao_banyan',
    checkpointSpawnId: 'cp_tan_trao_banyan',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'tan_trao_banyan',
        toZoneId: 'ba_dinh_square',
        portalId: 'portal_tantrao_to_badinh',
        destinationSpawn: 'from_tan_trao',
        label: 'Quảng trường Ba Đình (2/9/1945)',
      },
    ],
  },
  {
    id: 'ba_dinh_square',
    title: 'Quảng Trường Ba Đình Lịch Sử (2/9/1945)',
    eraId: '1945',
    districtType: 'finale',
    description: 'Nơi Chủ tịch Hồ Chí Minh đọc Tuyên ngôn Độc lập khai sinh nước Việt Nam Dân chủ Cộng hòa.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_ba_dinh_square',
    checkpointSpawnId: 'cp_ba_dinh_square',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'ba_dinh_square',
        toZoneId: 'tan_trao_banyan',
        portalId: 'portal_badinh_to_tantrao',
        destinationSpawn: 'from_ba_dinh',
        label: 'Đình Tân Trào',
      },
    ],
  },
  {
    id: 'pha_din_pass',
    title: 'Đèo Pha Đin & Tuyến Lửa Điện Biên (1954)',
    eraId: '1954_1975',
    districtType: 'hub',
    description: 'Tuyến đường huyết mạch nơi đoàn dân công xe đạp thồ và pháo binh tiến vào lòng chảo Điện Biên.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_pha_din_pass',
    checkpointSpawnId: 'cp_pha_din_pass',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'pha_din_pass',
        toZoneId: 'spring_1975_corridor',
        portalId: 'portal_phadin_to_spring1975',
        destinationSpawn: 'from_pha_din',
        label: 'Hành Lang Mùa Xuân 1975',
      },
    ],
  },
  {
    id: 'spring_1975_corridor',
    title: 'Hành Lang Thần Tốc Mùa Xuân 1975',
    eraId: '1954_1975',
    districtType: 'finale',
    description: 'Bản đồ tác chiến Chiến dịch Hồ Chí Minh lịch sử tiến về Dinh Độc Lập trưa 30/4/1975.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_spring_1975_corridor',
    checkpointSpawnId: 'cp_spring_1975_corridor',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'spring_1975_corridor',
        toZoneId: 'pha_din_pass',
        portalId: 'portal_spring1975_to_phadin',
        destinationSpawn: 'from_spring_1975',
        label: 'Đèo Pha Đin (1954)',
      },
    ],
  },
  {
    id: 'doi_moi_avenue',
    title: 'Đại Lộ Đổi Mới (Đại Hội VI — 12/1986)',
    eraId: '1986_2045',
    districtType: 'hub',
    description: 'Không gian chuyển mình lịch sử từ cơ chế bao cấp sang nền kinh tế hàng hóa nhiều thành phần.',
    gridPos: { x: 0, y: 0 },
    checkpointId: 'cp_doi_moi_avenue',
    checkpointSpawnId: 'cp_doi_moi_avenue',
    objectiveCount: 2,
    edges: [
      {
        fromZoneId: 'doi_moi_avenue',
        toZoneId: 'nexus_2045_plaza',
        portalId: 'portal_doimoi_to_nexus2045',
        destinationSpawn: 'from_doi_moi',
        label: 'Quảng Trường Kỷ Nguyên Số 2045',
      },
    ],
  },
  {
    id: 'nexus_2045_plaza',
    title: 'Quảng Trường Kỷ Nguyên Số 2045',
    eraId: '1986_2045',
    districtType: 'finale',
    description: 'Viện Lưu Trữ Quốc Gia 2045 kỷ niệm 100 năm lập nước, nơi hội tụ 12 cột mốc lịch sử.',
    gridPos: { x: 1, y: 0 },
    checkpointId: 'cp_nexus_2045_plaza',
    checkpointSpawnId: 'cp_nexus_2045_plaza',
    objectiveCount: 1,
    edges: [
      {
        fromZoneId: 'nexus_2045_plaza',
        toZoneId: 'doi_moi_avenue',
        portalId: 'portal_nexus2045_to_doimoi',
        destinationSpawn: 'from_nexus_2045',
        label: 'Đại Lộ Đổi Mới (1986)',
      },
    ],
  },
];

const ERA_WORLD_MAP_TITLES: Record<WorldZoneNode['eraId'], string> = {
  '1930': 'SƠ ĐỒ LIÊN THÔNG KHU VỰC — HƯƠNG CẢNG 1930',
  '1945': 'SƠ ĐỒ LIÊN THÔNG KHU VỰC — TÂN TRÀO & BA ĐÌNH 1945',
  '1954_1975': 'SƠ ĐỒ LIÊN THÔNG KHU VỰC — ĐIỆN BIÊN PHỦ & MÙA XUÂN 1975',
  '1986_2045': 'SƠ ĐỒ LIÊN THÔNG KHU VỰC — ĐẠI LỘ ĐỔI MỚI & KỶ NGUYÊN 2045',
};

const DISTRICT_BADGE_LABELS: Record<WorldZoneNode['districtType'], string> = {
  hub: 'TRUNG TÂM (HUB)',
  outdoor: 'NGOÀI TRỜI',
  interior: 'NỘI THẤT',
  optional: 'NHÁNH TÙY CHỌN',
  stealth: 'ẨN NẤP TUẦN TRA',
  finale: 'ĐIỂM HỘI TỤ',
};

export interface WorldMapPanelCallbacks {
  onFastTravel?: (zoneId: string, spawnId: string) => void;
  onClose?: () => void;
}

export class WorldMapPanel {
  private callbacks: WorldMapPanelCallbacks;
  private currentMapId = 'harbor_square';
  private discoveredMaps: Set<string> = new Set(['harbor_square']);
  private unlockedCheckpoints: Set<string> = new Set(['cp_harbor_lantern']);
  private isPresenterMode = false;

  constructor(callbacks: WorldMapPanelCallbacks) {
    this.callbacks = callbacks;
    this.bindStaticEvents();
    this.render();
  }

  private bindStaticEvents(): void {
    const closeBtn = document.getElementById('btn-close-world-map');
    closeBtn?.addEventListener('click', () => {
      this.toggle(false);
      this.callbacks.onClose?.();
    });
  }

  public isOpen(): boolean {
    const modal = document.getElementById('world-map-modal');
    return Boolean(modal && modal.classList.contains('is-open') && !modal.hidden);
  }

  public toggle(force?: boolean): boolean {
    const modal = document.getElementById('world-map-modal');
    const openBtn = document.getElementById('btn-open-map');
    if (!modal) return false;

    const currentlyOpen = this.isOpen();
    const next = force ?? !currentlyOpen;

    modal.classList.toggle('is-open', next);
    modal.hidden = !next;
    if (next) {
      modal.removeAttribute('hidden');
      this.render();
    } else {
      modal.setAttribute('hidden', '');
    }

    openBtn?.classList.toggle('is-active', next);
    openBtn?.setAttribute('aria-expanded', String(next));
    return next;
  }

  public updateState(
    currentMapId: string,
    discoveredMaps: ReadonlySet<string>,
    unlockedCheckpoints: ReadonlySet<string>,
    isPresenterMode = false
  ): void {
    this.currentMapId = currentMapId;
    this.discoveredMaps = new Set(discoveredMaps);
    this.discoveredMaps.add(currentMapId);
    this.unlockedCheckpoints = new Set(unlockedCheckpoints);
    this.isPresenterMode = isPresenterMode;
    this.render();
  }

  private isZoneDiscovered(zone: WorldZoneNode): boolean {
    return this.isPresenterMode || this.discoveredMaps.has(zone.id);
  }

  private isCheckpointUnlocked(zone: WorldZoneNode): boolean {
    if (this.isPresenterMode) return true;
    if (zone.checkpointId && this.unlockedCheckpoints.has(zone.checkpointId)) return true;
    if (zone.checkpointSpawnId && this.unlockedCheckpoints.has(zone.checkpointSpawnId)) return true;
    if (this.unlockedCheckpoints.has(zone.id)) return true;
    return false;
  }

  public render(): void {
    const titleEl = document.getElementById('world-map-title');
    const summaryEl = document.getElementById('world-map-summary');
    const graphEl = document.getElementById('world-map-graph');
    const detailEl = document.getElementById('world-map-zone-detail');

    const activeNode = ALL_WORLD_ZONES.find((z) => z.id === this.currentMapId);
    const activeEraId: WorldZoneNode['eraId'] = activeNode?.eraId ?? '1930';
    const eraZones = ALL_WORLD_ZONES.filter((z) => z.eraId === activeEraId);

    if (titleEl) {
      titleEl.textContent = ERA_WORLD_MAP_TITLES[activeEraId];
    }

    const totalZones = eraZones.length;
    const discoveredCount = eraZones.filter((z) => this.isZoneDiscovered(z)).length;
    const unlockedCount = eraZones.filter((z) => this.isCheckpointUnlocked(z)).length;

    if (summaryEl) {
      summaryEl.textContent = `Đã khám phá ${discoveredCount}/${totalZones} khu vực · ${unlockedCount}/${totalZones} Đèn Báo An Toàn${
        this.isPresenterMode ? ' · [CHẾ ĐỘ GIẢNG VIÊN: MỞ TOÀN BỘ]' : ''
      }`;
    }

    if (graphEl) {
      graphEl.innerHTML = '';
      for (const zone of eraZones) {
        const isCurrent = zone.id === this.currentMapId;
        const isDiscovered = this.isZoneDiscovered(zone);
        const hasCheckpoint = this.isCheckpointUnlocked(zone);

        const nodeEl = document.createElement('div');
        nodeEl.className = [
          'world-map-node',
          isCurrent ? 'is-current' : '',
          isDiscovered ? 'is-discovered' : 'is-locked',
          hasCheckpoint ? 'has-checkpoint' : '',
        ]
          .filter(Boolean)
          .join(' ');
        nodeEl.dataset.zoneId = zone.id;
        nodeEl.style.gridColumn = String(zone.gridPos.x + 1);
        nodeEl.style.gridRow = String(zone.gridPos.y + 1);

        if (isDiscovered) {
          const connectedLabels = zone.edges.map((e) => e.label).join(' · ');
          nodeEl.innerHTML = `
            <div class="wm-node-top">
              <span class="wm-district-badge">${DISTRICT_BADGE_LABELS[zone.districtType]}</span>
              ${isCurrent ? '<span class="wm-current-seal">📍 ĐANG ĐỨNG</span>' : ''}
            </div>
            <strong class="wm-node-title">${zone.title}</strong>
            <span class="wm-node-links">↔ ${connectedLabels}</span>
            <span class="wm-node-cp">${
              hasCheckpoint ? '🏮 Đèn An Toàn: ĐÃ MỞ' : '🔒 Đèn An Toàn: Chưa kích hoạt'
            }</span>
          `;
        } else {
          nodeEl.innerHTML = `
            <div class="wm-node-top">
              <span class="wm-district-badge">KHU VỰC ẨN</span>
            </div>
            <strong class="wm-node-title">Chưa khám phá</strong>
            <span class="wm-node-links">Hãy tìm lối thông từ các khu vực lân cận</span>
          `;
        }

        graphEl.appendChild(nodeEl);
      }
    }

    if (detailEl) {
      detailEl.innerHTML = '';
      const otherUnlockedZones = ALL_WORLD_ZONES.filter(
        (z) => z.eraId !== activeEraId && this.isZoneDiscovered(z) && this.isCheckpointUnlocked(z)
      );
      const displayZones = [...eraZones, ...otherUnlockedZones];

      for (const zone of displayZones) {
        const isCurrent = zone.id === this.currentMapId;
        const isDiscovered = this.isZoneDiscovered(zone);
        const canFastTravel = isDiscovered && this.isCheckpointUnlocked(zone);
        const spawnId = zone.checkpointSpawnId ?? zone.checkpointId ?? 'default_spawn';

        const rowEl = document.createElement('div');
        rowEl.className = [
          'wm-zone-card',
          isCurrent ? 'is-current' : '',
          isDiscovered ? 'is-discovered' : 'is-locked',
        ]
          .filter(Boolean)
          .join(' ');

        if (isDiscovered) {
          rowEl.innerHTML = `
            <div class="wm-zone-info">
              <div class="wm-zone-header">
                <strong>${zone.title}</strong>
                <span class="wm-district-badge">${DISTRICT_BADGE_LABELS[zone.districtType]}</span>
              </div>
              <p class="wm-zone-desc">${zone.description}</p>
            </div>
            <div class="wm-zone-actions"></div>
          `;
          const actionsBox = rowEl.querySelector('.wm-zone-actions');
          if (actionsBox) {
            if (canFastTravel) {
              const btn = document.createElement('button');
              btn.type = 'button';
              btn.className = `ctrl-btn ${isCurrent ? '' : 'is-primary'}`;
              btn.dataset.fastTravelZone = zone.id;
              btn.dataset.fastTravelSpawn = spawnId;
              btn.textContent = isCurrent ? '📍 Vị Trí Hiện Tại' : '🏮 Dịch Chuyển Nhanh';
              btn.addEventListener('click', () => {
                this.toggle(false);
                this.callbacks.onFastTravel?.(zone.id, spawnId);
              });
              actionsBox.appendChild(btn);
            } else {
              const lockNote = document.createElement('span');
              lockNote.className = 'wm-lock-reason';
              lockNote.textContent = 'Cần khám phá trực tiếp & kích hoạt Đèn Báo An Toàn';
              actionsBox.appendChild(lockNote);
            }
          }
        } else {
          rowEl.innerHTML = `
            <div class="wm-zone-info">
              <div class="wm-zone-header">
                <strong>Khu vực chưa khám phá</strong>
                <span class="wm-district-badge">CHƯA MỞ</span>
              </div>
              <p class="wm-zone-desc">Cần khám phá trực tiếp & kích hoạt Đèn Báo An Toàn</p>
            </div>
          `;
        }

        detailEl.appendChild(rowEl);
      }
    }
  }
}
