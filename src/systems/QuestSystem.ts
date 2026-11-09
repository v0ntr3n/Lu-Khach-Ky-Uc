/**
 * @file src/systems/QuestSystem.ts
 * @description Hệ thống Nhiệm vụ & Mục tiêu (Quest System) cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Theo dõi tiến trình nhiệm vụ chính (Main Quests) và nhiệm vụ phụ (Side Quests),
 * kiểm tra điều kiện tiên quyết (flags, evidence, milestones) và cung cấp gợi ý đường đi.
 */

import type { GameStore } from '../core/GameStore';
import type {
  EraId,
  QuestDefinition,
  QuestObjective,
  QuestObjectiveStatus,
} from '../types/game';

export const QUEST_CATALOG: readonly QuestDefinition[] = [
  {
    id: 'q_ch1_main_unification',
    eraId: '1930',
    title: 'Hội Nghị Hợp Nhất Cửu Long (Xuân 1930)',
    subtitle: 'Chương I · Hương Cảng 1930',
    category: 'main',
    summary:
      'Khôi phục 3 cột mốc lịch sử trọng đại dẫn tới Hội nghị thành lập Đảng Cộng sản Việt Nam (06/01 – 07/02/1930), Cương lĩnh chính trị đầu tiên và Luận cương tháng 10/1930.',
    rewardFlag: 'ch1_main_quest_complete',
    provenance: {
      type: 'historical_fact',
      label: 'Nhiệm vụ Lịch sử Chính (Giáo trình 2021, Chương I, tr. 35–80)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, tr. 35–80',
    },
    objectives: [
      {
        id: 'obj_ch1_meet_liaison',
        title: 'Bắt liên lạc tại Quảng Trường Bến Cảng',
        description:
          'Gặp Giao Liên tại Quảng Trường Bến Cảng Hương Cảng để nhận mật hiệu và khôi phục Cột mốc I.',
        zoneId: 'harbor_square',
        completedByMilestone: 'I',
        completedByFlag: 'ch1_met_liaison',
        hintText:
          'Tìm Giao Liên Lâm Văn Bính gần cột đèn bến cảng Hương Cảng và nhấn [E].',
      },
      {
        id: 'obj_ch1_inspect_warehouse_ledger',
        title: 'Kiểm tra Sổ Vận Đơn & Báo Thanh Niên tại Kho Hàng Số 4',
        description:
          'Tiến vào Kho Hàng Bến Cảng để kiểm tra sổ vận đơn chuyển báo Thanh Niên và Đường Kách Mệnh.',
        zoneId: 'warehouse_interior',
        requiresMilestones: ['I'],
        completedByEvidence: 'ev_ch1_shipping_ledger',
        completedByFlag: 'ch1_inspected_ledger',
        hintText:
          'Đi qua cửa Kho Hàng Bến Cảng (x=512, y=400) để thu thập tư liệu vận chuyển báo Thanh Niên.',
      },
      {
        id: 'obj_ch1_decipher_safehouse',
        title: 'Giải mã Mật Tín tại Nhà An Toàn Giao Liên',
        description:
          'Đến Nhà An Toàn tại Phố Cổ Cửu Long để khôi phục Mật tín triệu tập Hội nghị hợp nhất (Cột mốc II).',
        zoneId: 'safehouse_interior',
        requiresEvidence: ['ev_ch1_shipping_ledger'],
        completedByMilestone: 'II',
        completedByEvidence: 'ev_ch1_cipher_note',
        hintText:
          'Từ Quảng Trường đi sang Phố Cổ Cửu Long rồi vào Nhà An Toàn Giao Liên.',
      },
      {
        id: 'obj_ch1_reach_secret_archive',
        title: 'Hoàn tất Văn Kiện Hợp Nhất tại Phòng Lưu Trữ Bí Mật',
        description:
          'Thu thập Chánh cương vắn tắt, Sách lược vắn tắt và Luận cương chính trị (Cột mốc III) để mở Cổng Thời Gian.',
        zoneId: 'secret_archive',
        requiresMilestones: ['I', 'II'],
        completedByMilestone: 'III',
        completedByEvidence: 'ev_ch1_unification_appeal',
        hintText:
          'Vượt qua Hẻm Tối Tuần Tra (hoặc đi vòng qua Bến Thuyền Nhỏ) tới Sân Kho Lưu Trữ và vào Phòng Lưu Trữ Bí Mật.',
      },
    ],
  },
  {
    id: 'q_ch1_side_dock_watch',
    eraId: '1930',
    title: 'Đêm Gác Bến Thuyền Nhỏ (Nhiệm Vụ Phụ)',
    subtitle: 'Chương I · Lối Đi Vòng An Toàn',
    category: 'side',
    summary:
      'Tìm hiểu lịch đổi ca tuần tra từ Lão Chèo Đò tại Bến Thuyền Nhỏ để mở lộ trình an toàn tránh đèn rọi ở Hẻm Tối.',
    rewardFlag: 'ch1_side_quest_complete',
    provenance: {
      type: 'fictional_npc',
      label: 'Nhiệm vụ Phụ Dẫn truyện Hư cấu (Fictional Side Quest)',
      historicalNote:
        'Nhiệm vụ phụ giúp người chơi khám phá ngõ tắt ven bến thuyền và hiểu thêm về sự che chở của nhân dân lao động.',
    },
    objectives: [
      {
        id: 'obj_ch1_talk_pier_boatman',
        title: 'Hỏi thăm Lão Chèo Đò tại Bến Thuyền Nhỏ',
        description:
          'Trò chuyện với Lão Chèo Đò ở Bến Thuyền Nhỏ phía nam Quảng Trường.',
        zoneId: 'small_pier',
        isOptional: true,
        completedByFlag: 'ch1_talked_boatman',
        hintText:
          'Rẽ xuống lối nam Quảng Trường Bến Cảng để tới Bến Thuyền Nhỏ.',
      },
      {
        id: 'obj_ch1_check_patrol_schedule',
        title: 'Thu thập Lịch Đổi Ca Tuần Tra trên cầu tàu',
        description:
          'Lấy tờ Lịch Đổi Ca Tuần Tra trên bàn gỗ cuối cầu tàu Bến Thuyền Nhỏ.',
        zoneId: 'small_pier',
        requiresFlags: ['ch1_talked_boatman'],
        isOptional: true,
        completedByEvidence: 'ev_ch1_patrol_schedule',
        hintText:
          'Kiểm tra bàn gỗ cuối cầu tàu Bến Thuyền Nhỏ để lấy Lịch Đổi Ca.',
      },
    ],
  },
  {
    id: 'q_ch2_main_august_revolution',
    eraId: '1945',
    title: 'Chuyển Hướng Chiến Lược & Tổng Khởi Nghĩa (1939 – 1945)',
    subtitle: 'Chương II · Tân Trào – Quảng Trường Ba Đình',
    category: 'main',
    summary:
      'Điều phối mạng lưới giao liên từ Cây Đa Tân Trào về Quảng trường Ba Đình, khôi phục 3 cột mốc Hội nghị Trung ương 8 (05/1941), Lệnh Tổng khởi nghĩa (08/1945) và Tuyên ngôn Độc lập (02/09/1945).',
    rewardFlag: 'ch2_main_quest_complete',
    provenance: {
      type: 'historical_fact',
      label: 'Nhiệm vụ Lịch sử Chính (Giáo trình 2021, Chương I, tr. 81–126)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.2–II.3, tr. 81–126',
    },
    objectives: [
      {
        id: 'obj_ch2_pac_bo',
        title: 'Bắt liên lạc tại Cây Đa Tân Trào & Khôi phục Nghị quyết Trung ương 8 (05/1941)',
        description:
          'Gặp Giao Liên Việt Minh tại Tân Trào để khôi phục Cột mốc IV: Đặt nhiệm vụ giải phóng dân tộc lên hàng đầu.',
        zoneId: 'tan_trao_banyan',
        completedByMilestone: 'IV',
        completedByFlag: 'ch2_met_messenger',
        hintText:
          'Gặp Giao Liên Việt Minh tại Cây Đa Tân Trào và nhấn [E] để khôi phục Cột mốc IV.',
      },
      {
        id: 'obj_ch2_tan_trao',
        title: 'Điều phối Mạng Lưới Giao Liên & Quân Lệnh Số 1 tại Lán Nà Nưa (08/1945)',
        description:
          'Giải bài toán Điều phối Mạng lưới Giao liên Tổng khởi nghĩa tại Tân Trào để khôi phục Cột mốc V.',
        zoneId: 'tan_trao_banyan',
        requiresMilestones: ['IV'],
        completedByMilestone: 'V',
        completedByFlag: 'ch2_network_coordinated',
        completedByEvidence: 'ev_ch2_tan_trao_order',
        hintText:
          'Tương tác với Bàn Điều Phối Mạng Lưới Giao Liên tại Tân Trào [E] để phát lệnh Tổng khởi nghĩa (Cột mốc V).',
      },
      {
        id: 'obj_ch2_ba_dinh',
        title: 'Tiến về Quảng Trường Ba Đình & Khôi phục Tuyên ngôn Độc lập (02/09/1945)',
        description:
          'Đi qua cổng tiến về Quảng trường Ba Đình, gặp Nhân chứng Lịch sử khôi phục Cột mốc VI và mở Cổng Thời Gian Chương II.',
        zoneId: 'ba_dinh_square',
        requiresMilestones: ['IV', 'V'],
        completedByMilestone: 'VI',
        completedByFlag: 'ch2_met_witness',
        hintText:
          'Đi qua lối chuyển cảnh sang Quảng Trường Ba Đình, khôi phục Cột mốc VI và mở Cổng Thời Gian.',
      },
    ],
  },
  {
    id: 'q_ch3_main_reunification',
    eraId: '1954_1975',
    title: 'Hai Chiến Lược Cách Mạng & Thống Nhất Đất Nước (1945 – 1975)',
    subtitle: 'Chương III · Đèo Pha Đin – Hành Lang Mùa Xuân 1975',
    category: 'main',
    summary:
      'Điều phối tuyến tiếp vận chiến dịch Điện Biên Phủ trên đèo Pha Đin và tiến vào Hành lang Mùa Xuân 1975, khôi phục Cột mốc VII, VIII, IX.',
    rewardFlag: 'ch3_main_quest_complete',
    provenance: {
      type: 'historical_fact',
      label: 'Nhiệm vụ Lịch sử Chính (Giáo trình 2021, Chương II, tr. 127–238)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, tr. 127–238',
    },
    objectives: [
      {
        id: 'obj_ch3_resistance_1946',
        title: 'Gặp Đội Trưởng Dân Công tại Đèo Pha Đin & Khôi phục Cột mốc VII',
        description:
          'Khôi phục Lời kêu gọi Toàn quốc kháng chiến (19/12/1946) và đường lối kháng chiến toàn dân, toàn diện.',
        zoneId: 'pha_din_pass',
        completedByMilestone: 'VII',
        completedByFlag: 'ch3_met_quartermaster',
        hintText:
          'Trò chuyện với Đội Trưởng Dân Công Hậu Cần tại Đèo Pha Đin [E] để khôi phục Cột mốc VII.',
      },
      {
        id: 'obj_ch3_dien_bien_1954',
        title: 'Giải Sa Bàn Tiếp Vận Pha Đin — Chiến thắng Điện Biên Phủ (07/05/1954)',
        description:
          'Điều phối phương châm "Đánh chắc, tiến chắc" và tuyến xe thồ Điện Biên trên Sa bàn Hậu cần để khôi phục Cột mốc VIII.',
        zoneId: 'pha_din_pass',
        requiresMilestones: ['VII'],
        completedByMilestone: 'VIII',
        completedByFlag: 'ch3_logistics_allocated',
        completedByEvidence: 'ev_ch3_pha_din_dispatch',
        hintText:
          'Tương tác với Sa Bàn Tiếp Vận Chiến Dịch tại Đèo Pha Đin [E] để khôi phục Cột mốc VIII.',
      },
      {
        id: 'obj_ch3_spring_1975',
        title: 'Tiến vào Hành Lang Mùa Xuân 1975 & Chiến dịch Hồ Chí Minh (30/04/1975)',
        description:
          'Chuyển cảnh sang Hành Lang Mùa Xuân 1975, khôi phục Cột mốc IX và mở Cổng Thời Gian Chương III.',
        zoneId: 'spring_1975_corridor',
        requiresMilestones: ['VII', 'VIII'],
        completedByMilestone: 'IX',
        completedByFlag: 'ch3_met_signal',
        hintText:
          'Đi qua cổng sang Hành Lang Mùa Xuân 1975, khôi phục Cột mốc IX và mở Cổng Thời Gian.',
      },
    ],
  },
  {
    id: 'q_ch4_main_doi_moi',
    eraId: '1986_2045',
    title: 'Đổi Mới Toàn Diện & Khát Vọng Việt Nam 2045 (1986 – 2045)',
    subtitle: 'Chương IV · Đại Lộ Đổi Mới 1986 – Quảng Trường Kỷ Nguyên Số 2045',
    category: 'main',
    summary:
      'Khôi phục Nghị quyết Đại hội VI (1986), giải Ma trận Nhân quả Quyết sách Đổi mới và tiến tới Quảng trường Kỷ Nguyên Số 2045 hoàn thành trọn vẹn 12/12 Cột mốc.',
    rewardFlag: 'ch4_main_quest_complete',
    provenance: {
      type: 'historical_fact',
      label: 'Nhiệm vụ Lịch sử Chính & Tầm nhìn 2045 (Giáo trình 2021, Chương III, tr. 258–395)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 258–395',
    },
    objectives: [
      {
        id: 'obj_ch4_doi_moi_1986',
        title: 'Gặp Chuyên Gia Kinh Tế tại Đại Lộ Đổi Mới & Khôi phục Đại hội VI (12/1986)',
        description:
          'Khôi phục Cột mốc X: Đổi mới tư duy kinh tế, xóa bỏ cơ chế tập trung quan liêu bao cấp.',
        zoneId: 'doi_moi_avenue',
        completedByMilestone: 'X',
        completedByFlag: 'ch4_met_economist',
        hintText:
          'Trò chuyện với Chuyên Gia Nghiên Cứu Đổi Mới tại Đại Lộ Đổi Mới 1986 [E] để khôi phục Cột mốc X.',
      },
      {
        id: 'obj_ch4_platform_2011',
        title: 'Giải Ma Trận Nhân Quả Quyết Sách — Cương lĩnh 1991 & 2011',
        description:
          'Liên kết chuỗi nhân quả quyết sách Đổi mới và mô hình kinh tế thị trường định hướng XHCN để khôi phục Cột mốc XI.',
        zoneId: 'doi_moi_avenue',
        requiresMilestones: ['X'],
        completedByMilestone: 'XI',
        completedByFlag: 'ch4_policy_linked',
        completedByEvidence: 'ev_ch4_doi_moi_resolution',
        hintText:
          'Tương tác với Bàn Ma Trận Quyết Sách Đổi Mới [E] để khôi phục Cột mốc XI.',
      },
      {
        id: 'obj_ch4_vision_2045',
        title: 'Tiến tới Quảng Trường Kỷ Nguyên Số 2045 & Khôi phục Tầm nhìn Đại hội XIII',
        description:
          'Bước sang Quảng Trường Kỷ Nguyên Số 2045, khôi phục Cột mốc XII và hoàn thành Biên niên sử 1930 – 2045.',
        zoneId: 'nexus_2045_plaza',
        requiresMilestones: ['X', 'XI'],
        completedByMilestone: 'XII',
        completedByFlag: 'ch4_met_archivist_2045',
        hintText:
          'Đi qua cổng sang Quảng Trường Kỷ Nguyên Số 2045, khôi phục Cột mốc XII và mở Cổng Thời Gian.',
      },
    ],
  },
];

export class QuestSystem {
  public getAllQuests(): readonly QuestDefinition[] {
    return QUEST_CATALOG;
  }

  public getQuestsForEra(eraId: EraId): QuestDefinition[] {
    return QUEST_CATALOG.filter((q) => q.eraId === eraId);
  }

  private isObjectiveCompletedRaw(obj: QuestObjective, store: GameStore): boolean {
    if (store.isPresenterMode) {
      return true;
    }
    if (store.completedObjectives.has(obj.id)) {
      return true;
    }
    const activeMilestones = store.getActiveMilestones();
    if (obj.completedByMilestone && activeMilestones.has(obj.completedByMilestone)) {
      return true;
    }
    if (obj.completedByEvidence && store.collectedEvidence.has(obj.completedByEvidence)) {
      return true;
    }
    if (obj.completedByFlag && store.questFlags.has(obj.completedByFlag)) {
      return true;
    }
    return false;
  }

  private arePrerequisitesMet(obj: QuestObjective, store: GameStore): boolean {
    if (store.isPresenterMode) {
      return true;
    }
    const activeMilestones = store.getActiveMilestones();
    if (obj.requiresMilestones) {
      for (const m of obj.requiresMilestones) {
        if (!activeMilestones.has(m)) return false;
      }
    }
    if (obj.requiresEvidence) {
      for (const ev of obj.requiresEvidence) {
        if (!store.collectedEvidence.has(ev)) return false;
      }
    }
    if (obj.requiresFlags) {
      for (const flag of obj.requiresFlags) {
        if (!store.questFlags.has(flag)) return false;
      }
    }
    return true;
  }

  public getObjectiveStatus(
    obj: QuestObjective,
    store: GameStore
  ): QuestObjectiveStatus {
    if (this.isObjectiveCompletedRaw(obj, store)) {
      return 'completed';
    }
    if (this.arePrerequisitesMet(obj, store)) {
      return 'active';
    }
    return 'locked';
  }

  public evaluateProgress(
    store: GameStore,
    eraId: EraId
  ): {
    newlyCompletedObjectives: QuestObjective[];
    newlyCompletedQuests: QuestDefinition[];
  } {
    const newlyCompletedObjectives: QuestObjective[] = [];
    const newlyCompletedQuests: QuestDefinition[] = [];

    if (store.isPresenterMode) {
      return { newlyCompletedObjectives, newlyCompletedQuests };
    }

    const quests = this.getQuestsForEra(eraId);
    for (const quest of quests) {
      let allCompleted = true;
      for (const obj of quest.objectives) {
        const done = this.isObjectiveCompletedRaw(obj, store);
        if (done) {
          if (!store.completedObjectives.has(obj.id)) {
            store.completedObjectives.add(obj.id);
            newlyCompletedObjectives.push(obj);
          }
        } else {
          allCompleted = false;
        }
      }

      if (allCompleted && !store.completedQuests.has(quest.id)) {
        store.completedQuests.add(quest.id);
        if (quest.rewardFlag) {
          store.questFlags.add(quest.rewardFlag);
        }
        newlyCompletedQuests.push(quest);
      }
    }

    return { newlyCompletedObjectives, newlyCompletedQuests };
  }

  public getCurrentActiveObjective(
    eraId: EraId,
    store: GameStore
  ): { quest: QuestDefinition; objective: QuestObjective } | null {
    const quests = this.getQuestsForEra(eraId);

    // Ưu tiên nhiệm vụ chính trước, nhiệm vụ phụ sau
    const sorted = [...quests].sort((a, b) =>
      a.category === b.category ? 0 : a.category === 'main' ? -1 : 1
    );

    for (const quest of sorted) {
      for (const obj of quest.objectives) {
        if (this.getObjectiveStatus(obj, store) === 'active') {
          return { quest, objective: obj };
        }
      }
    }

    // Fallback về mục tiêu chưa hoàn thành đầu tiên
    for (const quest of sorted) {
      for (const obj of quest.objectives) {
        if (this.getObjectiveStatus(obj, store) !== 'completed') {
          return { quest, objective: obj };
        }
      }
    }

    return null;
  }

  public getHintForActiveObjective(eraId: EraId, store: GameStore): string {
    const active = this.getCurrentActiveObjective(eraId, store);
    if (!active) {
      return 'Đã hoàn thành mọi mục tiêu nhiệm vụ của chương! Hãy tiến tới Cổng Thời Gian cuối màn và nhấn [E].';
    }
    return (
      active.objective.hintText ??
      active.objective.description ??
      active.objective.title
    );
  }
}
