/**
 * @file src/systems/DialogueSystem.ts
 * @description Hệ thống Hội thoại Phân nhánh Đa lựa chọn (Dialogue System) cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Hỗ trợ cây hội thoại nhiều nút, điều kiện hiển thị lựa chọn theo cờ nhiệm vụ / chứng cứ,
 * cấp cờ tiến trình (grantsFlag / grantsEvidence / grantsMilestone) và gắn nhãn nguồn gốc sử liệu (Provenance).
 */

import type {
  DialogueChoice,
  DialogueNode,
  DialogueTree,
} from '../types/game';

export const DIALOGUE_TREES: readonly DialogueTree[] = [
  {
    id: 'dlg_harbor_liaison',
    eraId: '1930',
    npcId: 'npc_harbor_liaison',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Giao Liên Lâm Văn Bính',
        role: 'Giao liên Cửu Long',
        text: '“Đồng chí tới đúng giờ lắm! Mưa đêm Hương Cảng đang giúp chúng ta che mắt mật thám. Từ năm 1925, báo Thanh Niên và cuốn Đường Kách Mệnh do Nguyễn Ái Quốc biên soạn đã theo tàu bí mật từ Quảng Châu qua bến này về nước.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 44–49)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2, tr. 44–49',
          historicalNote:
            'Nhân vật Lâm Văn Bính là giao liên hư cấu đại diện cho mạng lưới liên lạc bí mật tại Hương Cảng năm 1930.',
        },
        choices: [
          {
            id: 'c_liaison_ask_warehouse',
            text: 'Hỏi chỗ cất Sổ Vận Đơn',
            nextNodeId: 'warehouse_guide',
            grantsFlag: 'ch1_met_liaison',
            grantsMilestone: 'I',
            responseToast: '✓ Đã nhận mật hiệu liên lạc tại Quảng Trường Bến Cảng!',
          },
          {
            id: 'c_liaison_ask_unification',
            text: 'Vì sao phải hợp nhất gấp?',
            nextNodeId: 'unification_context',
            grantsFlag: 'ch1_met_liaison',
            grantsMilestone: 'I',
          },
          {
            id: 'c_liaison_leave',
            text: 'Đi kiểm tra quanh bến cảng',
            nextNodeId: null,
            grantsFlag: 'ch1_met_liaison',
          },
        ],
      },
      warehouse_guide: {
        id: 'warehouse_guide',
        speaker: 'Giao Liên Lâm Văn Bính',
        role: 'Giao liên Cửu Long',
        text: '“Cửa Kho Hàng nằm ngay phía đông quảng trường, nơi cất Sổ Vận Đơn ghi các chuyến tàu chở báo Thanh Niên. Lấy sổ xong, đồng chí hãy sang Phố Cổ Cửu Long để vào Nhà An Toàn.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn hành trình (Nhân vật hư cấu)',
          historicalNote:
            'Hướng dẫn không gian bản đồ giúp người chơi tìm Kho Hàng Số 4 và Phố Cổ Cửu Long.',
        },
        choices: [
          {
            id: 'c_liaison_to_context',
            text: 'Hỏi về ba tổ chức 1929',
            nextNodeId: 'unification_context',
          },
          {
            id: 'c_liaison_done_1',
            text: 'Vào Kho Hàng Bến Cảng ngay',
            nextNodeId: null,
          },
        ],
      },
      unification_context: {
        id: 'unification_context',
        speaker: 'Giao Liên Lâm Văn Bính',
        role: 'Giao liên Cửu Long',
        text: '“Cuối năm 1929, ba tổ chức cộng sản lần lượt ra đời là Đông Dương Cộng sản Đảng, An Nam Cộng sản Đảng và Đông Dương Cộng sản Liên đoàn. Để tránh phân tán lực lượng, Nguyễn Ái Quốc đã chủ động triệu tập Hội nghị hợp nhất tại Cửu Long.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 50–61)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.c & I.3.a, tr. 50–61',
        },
        choices: [
          {
            id: 'c_liaison_to_warehouse',
            text: 'Kiểm tra tư liệu ở đâu?',
            nextNodeId: 'warehouse_guide',
          },
          {
            id: 'c_liaison_done_2',
            text: 'Rõ rồi, tôi lên đường ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_old_worker',
    eraId: '1930',
    npcId: 'npc_old_worker',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Bác Công Nhân Bến Cảng',
        role: 'Công nhân bốc xếp Cửu Long',
        text: '“Đêm mưa gió thế này mà chú em vẫn đi lại ngoài bến à? Đèn tuần tra quét rát lắm, cứ nép dưới mái hiên hoặc lán gỗ ven bến thì không ai rọi thấy đâu.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân chứng bến cảng (Hư cấu)',
          historicalNote:
            'Tái hiện tinh thần cảnh giác và sự che chở của công nhân bến cảng đối với các liên lạc viên cách mạng.',
        },
        choices: [
          {
            id: 'c_worker_ask_movement',
            text: 'Hỏi về phong trào vô sản hóa',
            nextNodeId: 'worker_movement',
            grantsFlag: 'ch1_talked_worker',
          },
          {
            id: 'c_worker_ask_pier',
            text: 'Có lối vòng tránh tuần tra không?',
            nextNodeId: 'worker_shortcut',
            grantsFlag: 'ch1_talked_worker',
          },
          {
            id: 'c_worker_leave',
            text: 'Cảm ơn bác, cháu đi tiếp',
            nextNodeId: null,
          },
        ],
      },
      worker_movement: {
        id: 'worker_movement',
        speaker: 'Bác Công Nhân Bến Cảng',
        role: 'Công nhân bốc xếp Cửu Long',
        text: '“Từ năm 1928, Hội Việt Nam Cách mạng Thanh niên phát động phong trào ‘vô sản hóa’. Nhiều anh em hội viên đã vào tận bến cảng, nhà máy, hầm mỏ cùng lao động để truyền bá cách mạng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 48–50)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.b, tr. 48–50',
        },
        choices: [
          {
            id: 'c_worker_back_shortcut',
            text: 'Hỏi ngõ xuống Bến Thuyền Nhỏ',
            nextNodeId: 'worker_shortcut',
          },
          {
            id: 'c_worker_done_1',
            text: 'Cháu đã ghi nhớ tư liệu này',
            nextNodeId: null,
          },
        ],
      },
      worker_shortcut: {
        id: 'worker_shortcut',
        speaker: 'Bác Công Nhân Bến Cảng',
        role: 'Công nhân bốc xếp Cửu Long',
        text: '“Chú cứ đi xuống mép nam quảng trường là tới Bến Thuyền Nhỏ. Hỏi Lão Chèo Đò dưới đó, ông ấy nắm rõ giờ đổi ca và có đường tắt sang Sân Kho Lưu Trữ.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn lối tắt (Hư cấu)',
        },
        choices: [
          {
            id: 'c_worker_done_2',
            text: 'Ghé xuống Bến Thuyền Nhỏ ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_dock_watchman',
    eraId: '1930',
    npcId: 'npc_dock_watchman',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Người Gác Đèn Bến Cảng',
        role: 'Hoa tiêu đêm Hương Cảng',
        text: '“Ngọn đèn bão đầu mỗi khu phố là Đèn Báo An Toàn. Hễ thắp sáng đèn bằng phím [E], chú có thể mở Bản Đồ [M] để đi nhanh giữa các bến đã qua.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Hướng dẫn khám phá (Hư cấu)',
        },
        choices: [
          {
            id: 'c_watchman_ask_stealth',
            text: 'Cách tránh đèn rọi tuần tra?',
            nextNodeId: 'stealth_tip',
            grantsFlag: 'ch1_talked_watchman',
          },
          {
            id: 'c_watchman_leave',
            text: 'Cảm ơn ông, tôi hiểu rồi',
            nextNodeId: null,
          },
        ],
      },
      stealth_tip: {
        id: 'stealth_tip',
        speaker: 'Người Gác Đèn Bến Cảng',
        role: 'Hoa tiêu đêm Hương Cảng',
        text: '“Nhớ kỹ nhé: cúi người bằng phím [S] giữa đường trống không che được mắt lính tuần tra đâu! Chú phải bước hẳn vào dưới Mái Hiên hoặc Bóng Râm mới thực sự khuất bóng.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Mẹo ẩn nấp (Hư cấu)',
        },
        choices: [
          {
            id: 'c_watchman_done',
            text: 'Tôi sẽ bám theo mái hiên',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_warehouse_keeper',
    eraId: '1930',
    npcId: 'npc_warehouse_keeper',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Thủ Kho Lưu Trữ Cửu Long',
        role: 'Thủ kho Kho Số 4',
        text: '“Trên bàn gỗ phía bắc là Sổ Vận Đơn ghi chép các kiện sách báo từ Quảng Châu. Từng số báo Thanh Niên từ tháng 6/1925 đều được đóng thùng kín kẽ tại đây trước khi chuyển về Sài Gòn, Hải Phòng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 44–49)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.a, tr. 44–49',
        },
        choices: [
          {
            id: 'c_keeper_ask_thanh_nien',
            text: 'Hỏi về báo Thanh Niên',
            nextNodeId: 'thanh_nien_detail',
            grantsFlag: 'ch1_talked_warehouse_keeper',
          },
          {
            id: 'c_keeper_leave',
            text: 'Kiểm tra bàn Sổ Vận Đơn',
            nextNodeId: null,
          },
        ],
      },
      thanh_nien_detail: {
        id: 'thanh_nien_detail',
        speaker: 'Thủ Kho Lưu Trữ Cửu Long',
        role: 'Thủ kho Kho Số 4',
        text: '“Báo Thanh Niên ra số đầu ngày 21/6/1925, mở đường truyền bá lý luận giải phóng dân tộc. Còn cuốn Đường Kách Mệnh (1927) chỉ rõ cách mạng muốn thắng lợi trước hết phải có ‘Đảng cách mệnh’ vững mạnh.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 45–48)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.a, tr. 45–48',
        },
        choices: [
          {
            id: 'c_keeper_done',
            text: 'Cảm ơn đồng chí thủ kho',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_street_vendor',
    eraId: '1930',
    npcId: 'npc_street_vendor',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Bà Hàng Nước Phố Cổ',
        role: 'Cơ sở cảnh giới Phố Cổ',
        text: '“Khách đi đường ghé uống chén trà nóng cho ấm bụng đã! Nhà An Toàn nằm ngay góc đông bắc phố này, còn đi thẳng về hướng đông là lọt vào Hẻm Tối có lính tuần tra đấy.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân vật cảnh giới (Hư cấu)',
        },
        choices: [
          {
            id: 'c_vendor_ask_safehouse',
            text: 'Hỏi mật hiệu vào Nhà An Toàn',
            nextNodeId: 'safehouse_hint',
            grantsFlag: 'ch1_talked_vendor',
          },
          {
            id: 'c_vendor_ask_leaflet',
            text: 'Hỏi về tờ truyền đơn đầu phố',
            nextNodeId: 'leaflet_hint',
            grantsFlag: 'ch1_talked_vendor',
          },
          {
            id: 'c_vendor_leave',
            text: 'Cảm ơn bà, cháu đi ngay',
            nextNodeId: null,
          },
        ],
      },
      safehouse_hint: {
        id: 'safehouse_hint',
        speaker: 'Bà Hàng Nước Phố Cổ',
        role: 'Cơ sở cảnh giới Phố Cổ',
        text: '“Cửa Nhà An Toàn mở sẵn cho người có tín hiệu từ bến cảng. Trong đó đồng chí Trần Thị Liên đang giữ bức Mật Tín Triệu Tập Hội Nghị Hợp Nhất.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn nhiệm vụ (Hư cấu)',
        },
        choices: [
          {
            id: 'c_vendor_done_1',
            text: 'Vào Nhà An Toàn ngay',
            nextNodeId: null,
          },
        ],
      },
      leaflet_hint: {
        id: 'leaflet_hint',
        speaker: 'Bà Hàng Nước Phố Cổ',
        role: 'Cơ sở cảnh giới Phố Cổ',
        text: '“Tờ truyền đơn dán trên bảng tin đầu phố ghi lại làn sóng đấu tranh công nhân năm 1929. Cháu lại gần bấm [E] để cất vào Túi Tư Liệu [I] nhé.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Gợi ý chứng cứ (Hư cấu)',
        },
        choices: [
          {
            id: 'c_vendor_done_2',
            text: 'Cháu cảm ơn bà',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_liaison_inner',
    eraId: '1930',
    npcId: 'npc_liaison_inner',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Giao Liên Trần Thị Liên',
        role: 'Phụ trách Nhà An Toàn',
        text: '“Mừng đồng chí tới nơi an toàn! Từ ngày 06/01 đến 07/02/1930 tại Cửu Long (Hương Cảng), đồng chí Nguyễn Ái Quốc đã chủ trì Hội nghị hợp nhất các tổ chức cộng sản thành Đảng Cộng sản Việt Nam.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 57–61)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–61',
          historicalNote:
            'Nhân vật Trần Thị Liên là giao liên hư cấu; thời gian, địa điểm và nội dung Hội nghị thành lập Đảng là sự kiện lịch sử xác thực.',
        },
        choices: [
          {
            id: 'c_inner_ask_delegates',
            text: 'Ai tham dự Hội nghị hợp nhất?',
            nextNodeId: 'delegates_node',
            grantsFlag: 'ch1_safehouse_cipher',
            grantsEvidence: 'ev_ch1_cipher_note',
            grantsMilestone: 'II',
            responseToast: '✓ Đã khôi phục Cột mốc II & Mật Tín Hội Nghị Hợp Nhất!',
          },
          {
            id: 'c_inner_ask_next',
            text: 'Cương lĩnh được lưu giữ ở đâu?',
            nextNodeId: 'archive_direction',
            grantsFlag: 'ch1_safehouse_cipher',
            grantsEvidence: 'ev_ch1_cipher_note',
            grantsMilestone: 'II',
          },
          {
            id: 'c_inner_leave',
            text: 'Tới Phòng Lưu Trữ Bí Mật',
            nextNodeId: null,
          },
        ],
      },
      delegates_node: {
        id: 'delegates_node',
        speaker: 'Giao Liên Trần Thị Liên',
        role: 'Phụ trách Nhà An Toàn',
        text: '“Dự Hội nghị có đại biểu Đông Dương Cộng sản Đảng (Trịnh Đình Cửu, Nguyễn Đức Cảnh) và An Nam Cộng sản Đảng (Châu Văn Liêm, Nguyễn Thiệu) do Nguyễn Ái Quốc chủ trì. Đến ngày 24/02/1930, Đông Dương Cộng sản Liên đoàn cũng chính thức gia nhập Đảng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 57–65)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–65',
        },
        choices: [
          {
            id: 'c_inner_to_archive_dir',
            text: 'Hỏi đường tới Phòng Lưu Trữ',
            nextNodeId: 'archive_direction',
          },
          {
            id: 'c_inner_done_1',
            text: 'Đã ghi chép vào Sổ Tay [J]',
            nextNodeId: null,
          },
        ],
      },
      archive_direction: {
        id: 'archive_direction',
        speaker: 'Giao Liên Trần Thị Liên',
        role: 'Phụ trách Nhà An Toàn',
        text: '“Đồng chí hãy rời Nhà An Toàn ra Phố Cổ, vượt Hẻm Tối Tuần Tra hoặc đi vòng qua Bến Thuyền Nhỏ. Điểm đến cuối là Phòng Lưu Trữ Bí Mật để khôi phục Cương lĩnh đầu tiên.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn lộ trình (Hư cấu)',
        },
        choices: [
          {
            id: 'c_inner_done_2',
            text: 'Rõ rồi, tôi xuất phát ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_pier_boatman',
    eraId: '1930',
    npcId: 'npc_pier_boatman',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Lão Chèo Đò Bến Thuyền',
        role: 'Người đưa đò đêm Cửu Long',
        text: '“Chú em muốn sang Sân Kho Lưu Trữ mà tránh đèn tuần tra ở Hẻm Tối chứ gì? Ta đã chép sẵn Lịch Đổi Ca để trên chiếc bàn gỗ cuối cầu tàu kia kìa.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân vật nhiệm vụ phụ (Hư cấu)',
          historicalNote:
            'Nhân vật hư cấu hỗ trợ nhiệm vụ phụ Đêm Gác Bến Thuyền Nhỏ.',
        },
        choices: [
          {
            id: 'c_boatman_accept',
            text: 'Lấy Lịch Đổi Ca trên cầu tàu',
            nextNodeId: 'boatman_route_detail',
            grantsFlag: 'ch1_talked_boatman',
            responseToast: '📜 Đã cập nhật Nhiệm Vụ Phụ: Đêm Gác Bến Thuyền Nhỏ!',
          },
          {
            id: 'c_boatman_leave',
            text: 'Cháu chỉ ghé thăm bến thuyền',
            nextNodeId: null,
            grantsFlag: 'ch1_talked_boatman',
          },
        ],
      },
      boatman_route_detail: {
        id: 'boatman_route_detail',
        speaker: 'Lão Chèo Đò Bến Thuyền',
        role: 'Người đưa đò đêm Cửu Long',
        text: '“Lấy xong tờ Lịch Đổi Ca trên bàn gỗ, chú cứ theo cổng phía đông Bến Thuyền Nhỏ là thông thẳng sang Sân Kho Lưu Trữ an toàn!”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn lối đi an toàn (Hư cấu)',
        },
        choices: [
          {
            id: 'c_boatman_done',
            text: 'Cháu cảm ơn cụ nhiều lắm',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_archive_keeper',
    eraId: '1930',
    npcId: 'npc_archive_keeper',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Ông Từ Giữ Kho Lưu Trữ',
        role: 'Quản kho Lưu Trữ Cửu Long',
        text: '“Đây là Sân Kho Lưu Trữ Cửu Long. Cánh cửa phía đông dẫn vào Phòng Lưu Trữ Bí Mật, nơi cất giữ Chánh cương vắn tắt, Sách lược vắn tắt (02/1930) và Luận cương chính trị (10/1930).”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 61–76)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b & II.1, tr. 61–76',
        },
        choices: [
          {
            id: 'c_archive_compare_1930',
            text: 'Hỏi cốt lõi Cương lĩnh 02/1930',
            nextNodeId: 'platform_core',
            grantsFlag: 'ch1_archive_clearance',
          },
          {
            id: 'c_archive_enter',
            text: 'Vào Phòng Lưu Trữ Bí Mật',
            nextNodeId: null,
            grantsFlag: 'ch1_archive_clearance',
          },
        ],
      },
      platform_core: {
        id: 'platform_core',
        speaker: 'Ông Từ Giữ Kho Lưu Trữ',
        role: 'Quản kho Lưu Trữ Cửu Long',
        text: '“Cương lĩnh chính trị đầu tiên xác định nhiệm vụ hàng đầu là chống đế quốc giành độc lập dân tộc (‘làm cho nước Nam được hoàn toàn độc lập’). Đảng đồng thời đoàn kết rộng rãi công nông và các tầng lớp yêu nước.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 61–65)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b, tr. 61–65',
        },
        choices: [
          {
            id: 'c_archive_done',
            text: 'Bước vào Phòng Lưu Trữ ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_tan_trao_messenger',
    eraId: '1945',
    npcId: 'npc_tan_trao_messenger',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Giao Liên Việt Minh Hoàng Quốc Bảo',
        role: 'Giao liên Khu giải phóng Việt Bắc',
        text: '“Chào đồng chí! Từ Hội nghị Trung ương 8 tại Pác Bó (05/1941), Đảng đã đặt nhiệm vụ giải phóng dân tộc lên hàng đầu và lập Mặt trận Việt Minh. Nay tại Tân Trào (08/1945), lệnh Tổng khởi nghĩa đang chờ phát đi khắp cả nước!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 88–112)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.2–II.3, tr. 88–112',
          historicalNote:
            'Nhân vật Hoàng Quốc Bảo là giao liên hư cấu đại diện cho lực lượng liên lạc Việt Minh năm 1941–1945.',
        },
        choices: [
          {
            id: 'c_ch2_ask_tw8',
            text: 'Hỏi về Hội nghị Trung ương 8',
            nextNodeId: 'tw8_detail',
            grantsFlag: 'ch2_met_messenger',
            grantsMilestone: 'IV',
            responseToast: '✓ Đã khôi phục Cột mốc IV: Hội nghị Trung ương 8 (05/1941)!',
          },
          {
            id: 'c_ch2_leave',
            text: 'Tới Bàn Điều Phối Giao Liên',
            nextNodeId: null,
            grantsFlag: 'ch2_met_messenger',
            grantsMilestone: 'IV',
          },
        ],
      },
      tw8_detail: {
        id: 'tw8_detail',
        speaker: 'Giao Liên Việt Minh Hoàng Quốc Bảo',
        role: 'Giao liên Khu giải phóng Việt Bắc',
        text: '“Hội nghị Trung ương 8 khẳng định quyền lợi của bộ phận, của giai cấp phải đặt dưới sự sinh tử, tồn vong của quốc gia, dân tộc. Đồng chí hãy lại Bàn Điều Phối phía đông để phát Quân lệnh số 1!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 88–95)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.2.b, tr. 88–95',
        },
        choices: [
          {
            id: 'c_ch2_done',
            text: 'Tới Bàn Điều Phối ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_ba_dinh_witness',
    eraId: '1945',
    npcId: 'npc_ba_dinh_witness',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Đội Viên Tự Vệ Thành Hoàng Diệu',
        role: 'Tự vệ Hoàng Diệu · 02/09/1945',
        text: '“Giữa rừng cờ đỏ sao vàng tại Quảng trường Ba Đình ngày 02/09/1945, Chủ tịch Hồ Chí Minh đã đọc bản Tuyên ngôn Độc lập. Nước Việt Nam Dân chủ Cộng hòa chính thức khai sinh trước toàn thế giới!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 117–124)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.c, tr. 117–124',
        },
        choices: [
          {
            id: 'c_badinh_ask_significance',
            text: 'Hỏi ý nghĩa Cách mạng Tháng Tám',
            nextNodeId: 'badinh_significance',
            grantsFlag: 'ch2_met_witness',
            grantsMilestone: 'VI',
            responseToast: '✓ Đã khôi phục Cột mốc VI: Tuyên ngôn Độc lập (02/09/1945)!',
          },
          {
            id: 'c_badinh_leave',
            text: 'Tiến tới Cổng Thời Gian Chương II',
            nextNodeId: null,
            grantsFlag: 'ch2_met_witness',
            grantsMilestone: 'VI',
          },
        ],
      },
      badinh_significance: {
        id: 'badinh_significance',
        speaker: 'Đội Viên Tự Vệ Thành Hoàng Diệu',
        role: 'Tự vệ Hoàng Diệu · 02/09/1945',
        text: '“Cách mạng Tháng Tám năm 1945 đã đập tan xiềng xích thực dân gần một thế kỷ và chấm dứt chế độ quân chủ hàng nghìn năm. Nhân dân Việt Nam từ thân phận nô lệ đã vươn lên làm chủ đất nước.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 119–124)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.d, tr. 119–124',
        },
        choices: [
          {
            id: 'c_badinh_done',
            text: 'Bước tới Cổng Thời Gian ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_pha_din_quartermaster',
    eraId: '1954_1975',
    npcId: 'npc_pha_din_quartermaster',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Đội Trưởng Dân Công Hậu Cần',
        role: 'Chỉ huy dân công Đèo Pha Đin',
        text: '“Từ Lời kêu gọi Toàn quốc kháng chiến (19/12/1946) với đường lối toàn dân, toàn diện, lâu dài, tự lực cánh sinh, cả dân tộc đã bền bỉ chiến đấu. Hôm nay hàng vạn dân công xe thồ đang vượt đèo Pha Đin tiếp lửa cho Điện Biên Phủ!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 142–182)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục I.1–I.3, tr. 142–182',
        },
        choices: [
          {
            id: 'c_phadin_ask_1946',
            text: 'Hỏi về đường lối kháng chiến',
            nextNodeId: 'phadin_doctrine',
            grantsFlag: 'ch3_met_quartermaster',
            grantsMilestone: 'VII',
            responseToast: '✓ Đã khôi phục Cột mốc VII: Đường lối Kháng chiến Toàn dân!',
          },
          {
            id: 'c_phadin_leave',
            text: 'Tới Sa Bàn Tiếp Vận Pha Đin',
            nextNodeId: null,
            grantsFlag: 'ch3_met_quartermaster',
            grantsMilestone: 'VII',
          },
        ],
      },
      phadin_doctrine: {
        id: 'phadin_doctrine',
        speaker: 'Đội Trưởng Dân Công Hậu Cần',
        role: 'Chỉ huy dân công Đèo Pha Đin',
        text: '“Đường lối kháng chiến kết tinh trong Lời kêu gọi Toàn quốc kháng chiến của Hồ Chí Minh, Chỉ thị Toàn dân kháng chiến và tác phẩm Kháng chiến nhất định thắng lợi. Hãy tới Sa Bàn Tiếp Vận để điều phối chiến dịch!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 142–150)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục I.1.b, tr. 142–150',
        },
        choices: [
          {
            id: 'c_phadin_done',
            text: 'Điều phối Sa Bàn Tiếp Vận',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_1975_signal_officer',
    eraId: '1954_1975',
    npcId: 'npc_1975_signal_officer',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Chiến Sĩ Thông Tin Chiến Dịch Hồ Chí Minh',
        role: 'Chiến sĩ thông tin Mùa Xuân 1975',
        text: '“Bức công điện của Đại tướng Võ Nguyên Giáp vừa truyền đi toàn mặt trận: ‘Thần tốc, thần tốc hơn nữa; táo bạo, táo bạo hơn nữa... Quyết chiến và toàn thắng!’ Ngày 30/04/1975 đã thu non sông về một mối!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 228–238)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục II.2.c, tr. 228–238',
        },
        choices: [
          {
            id: 'c_1975_ask_victory',
            text: 'Hỏi ý nghĩa Đại thắng Mùa Xuân',
            nextNodeId: 'victory_1975_node',
            grantsFlag: 'ch3_met_signal',
            grantsMilestone: 'IX',
            responseToast: '✓ Đã khôi phục Cột mốc IX: Đại thắng mùa Xuân 30/04/1975!',
          },
          {
            id: 'c_1975_leave',
            text: 'Tới Cổng Thời Gian Chương III',
            nextNodeId: null,
            grantsFlag: 'ch3_met_signal',
            grantsMilestone: 'IX',
          },
        ],
      },
      victory_1975_node: {
        id: 'victory_1975_node',
        speaker: 'Chiến Sĩ Thông Tin Chiến Dịch Hồ Chí Minh',
        role: 'Chiến sĩ thông tin Mùa Xuân 1975',
        text: '“Thắng lợi của cuộc kháng chiến chống Mỹ, cứu nước đã kết thúc 21 năm chiến đấu kiên cường, hoàn thành cách mạng dân tộc dân chủ nhân dân trên cả nước. Kỷ nguyên độc lập, thống nhất và đi lên chủ nghĩa xã hội chính thức mở ra.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 232–238)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục II.3, tr. 232–238',
        },
        choices: [
          {
            id: 'c_1975_done',
            text: 'Bước tới Cổng Thời Gian ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_doi_moi_economist',
    eraId: '1986_2045',
    npcId: 'npc_doi_moi_economist',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Chuyên Gia Nghiên Cứu Đổi Mới',
        role: 'Chuyên gia chính sách Đổi Mới 1986',
        text: '“Tháng 12/1986, Đại hội VI của Đảng họp tại Hà Nội với tinh thần ‘nhìn thẳng vào sự thật, đánh giá đúng sự thật, nói rõ sự thật’. Từ đây, công cuộc Đổi mới toàn diện đất nước chính thức khởi xướng!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 261–268)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục I.2.a, tr. 261–268',
        },
        choices: [
          {
            id: 'c_ch4_ask_doimoi',
            text: 'Hỏi trọng tâm đổi mới kinh tế',
            nextNodeId: 'doimoi_detail',
            grantsFlag: 'ch4_met_economist',
            grantsMilestone: 'X',
            responseToast: '✓ Đã khôi phục Cột mốc X: Đại hội VI Khởi xướng Đổi mới (12/1986)!',
          },
          {
            id: 'c_ch4_leave',
            text: 'Tới Bàn Ma Trận Quyết Sách',
            nextNodeId: null,
            grantsFlag: 'ch4_met_economist',
            grantsMilestone: 'X',
          },
        ],
      },
      doimoi_detail: {
        id: 'doimoi_detail',
        speaker: 'Chuyên Gia Nghiên Cứu Đổi Mới',
        role: 'Chuyên gia chính sách Đổi Mới 1986',
        text: '“Đại hội VI chủ trương thực hiện Ba chương trình kinh tế lớn: Lương thực - thực phẩm, Hàng tiêu dùng và Hàng xuất khẩu. Cơ chế tập trung quan liêu bao cấp được xóa bỏ để phát triển kinh tế nhiều thành phần.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 261–268)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục I.2.a, tr. 261–268',
        },
        choices: [
          {
            id: 'c_ch4_done',
            text: 'Giải Ma Trận Quyết Sách ngay',
            nextNodeId: null,
          },
        ],
      },
    },
  },
  {
    id: 'dlg_archivist_2045',
    eraId: '1986_2045',
    npcId: 'npc_archivist_2045',
    rootNodeId: 'root',
    nodes: {
      root: {
        id: 'root',
        speaker: 'Giám Tuyển Lưu Trữ Quốc Gia 2045',
        role: 'Giám tuyển Trung tâm Ký Ức Số 2045',
        text: '“Chào mừng Lữ Khách trở về Quảng trường Kỷ Nguyên Số 2045! Không gian viễn cảnh này được kiến tạo từ mục tiêu phát triển đất nước đến năm 2030, tầm nhìn 2045 trong Nghị quyết Đại hội XIII (2021).”',
        provenance: {
          type: 'speculative_2045',
          label: 'Viễn cảnh Nghệ thuật 2045 · Đại hội XIII',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục II.4, tr. 385–393',
          historicalNote:
            'Không gian năm 2045 là bối cảnh dẫn truyện tương lai; còn mục tiêu năm 2030 và 2045 là nội dung văn kiện lịch sử Đại hội XIII (01/2021).',
        },
        choices: [
          {
            id: 'c_2045_ask_xii',
            text: 'Đối chiếu mục tiêu 2030 & 2045',
            nextNodeId: 'congress_xiii_fact',
            grantsFlag: 'ch4_met_archivist_2045',
            grantsMilestone: 'XII',
            responseToast: '✓ Đã khôi phục Cột mốc XII: Đại hội XIII & Tầm nhìn Việt Nam 2045!',
          },
          {
            id: 'c_2045_leave',
            text: 'Tới Cổng Thời Gian 2045',
            nextNodeId: null,
            grantsFlag: 'ch4_met_archivist_2045',
            grantsMilestone: 'XII',
          },
        ],
      },
      congress_xiii_fact: {
        id: 'congress_xiii_fact',
        speaker: 'Giám Tuyển Lưu Trữ Quốc Gia 2045',
        role: 'Giám tuyển Trung tâm Ký Ức Số 2045',
        text: '“Nghị quyết Đại hội XIII xác định đến năm 2030 (100 năm thành lập Đảng) Việt Nam là nước đang phát triển, có công nghiệp hiện đại, thu nhập trung bình cao. Đến năm 2045 (100 năm thành lập nước), nước ta trở thành nước phát triển, thu nhập cao.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sử liệu Giáo trình 2021 (tr. 385–393)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục II.4, tr. 385–393',
        },
        choices: [
          {
            id: 'c_2045_done',
            text: 'Mở Cổng Thời Gian 2045',
            nextNodeId: null,
          },
        ],
      },
    },
  },
];

const DIALOGUE_ID_ALIASES: Record<string, string> = {
  npc_courier: 'dlg_harbor_liaison',
  npc_harbor_liaison: 'dlg_harbor_liaison',
  ch1_courier_intro: 'dlg_harbor_liaison',
  npc_old_worker: 'dlg_old_worker',
  ch1_worker_rumor: 'dlg_old_worker',
  npc_observer: 'dlg_dock_watchman',
  npc_dock_watchman: 'dlg_dock_watchman',
  ch1_observer_patrol: 'dlg_dock_watchman',
  npc_archivist: 'dlg_warehouse_keeper',
  npc_warehouse_keeper: 'dlg_warehouse_keeper',
  ch1_archivist_sources: 'dlg_warehouse_keeper',
  npc_street_vendor: 'dlg_street_vendor',
  ch1_vendor_talk: 'dlg_street_vendor',
  npc_liaison_inner: 'dlg_liaison_inner',
  ch1_liaison_inner_talk: 'dlg_liaison_inner',
  npc_pier_boatman: 'dlg_pier_boatman',
  ch1_boatman_shortcut: 'dlg_pier_boatman',
  npc_archive_keeper: 'dlg_archive_keeper',
  ch1_keeper_gate: 'dlg_archive_keeper',
  npc_tan_trao_messenger: 'dlg_tan_trao_messenger',
  npc_ba_dinh_witness: 'dlg_ba_dinh_witness',
  npc_pha_din_quartermaster: 'dlg_pha_din_quartermaster',
  npc_1975_signal_officer: 'dlg_1975_signal_officer',
  npc_doi_moi_economist: 'dlg_doi_moi_economist',
  npc_archivist_2045: 'dlg_archivist_2045',
};

export class DialogueSystem {
  private activeTree: DialogueTree | null = null;
  private activeNode: DialogueNode | null = null;

  public getTree(idOrNpcId: string): DialogueTree | undefined {
    const mappedId = DIALOGUE_ID_ALIASES[idOrNpcId] ?? idOrNpcId;
    return DIALOGUE_TREES.find(
      (t) =>
        t.id === mappedId ||
        t.npcId === mappedId ||
        t.id === idOrNpcId ||
        t.npcId === idOrNpcId
    );
  }

  public startDialogue(idOrNpcId: string): DialogueNode | null {
    const tree = this.getTree(idOrNpcId);
    if (!tree) {
      this.activeTree = null;
      this.activeNode = null;
      return null;
    }
    const rootNode = tree.nodes[tree.rootNodeId] ?? Object.values(tree.nodes)[0] ?? null;
    this.activeTree = tree;
    this.activeNode = rootNode;
    return rootNode;
  }

  public getCurrentTree(): DialogueTree | null {
    return this.activeTree;
  }

  public getCurrentNode(): DialogueNode | null {
    return this.activeNode;
  }

  public getAvailableChoices(
    node: DialogueNode,
    questFlags: ReadonlySet<string>,
    collectedEvidence: ReadonlySet<string>
  ): DialogueChoice[] {
    return node.choices.filter((c) => {
      if (c.requiresFlag && !questFlags.has(c.requiresFlag)) {
        return false;
      }
      if (c.requiresEvidence && !collectedEvidence.has(c.requiresEvidence)) {
        return false;
      }
      return true;
    });
  }

  public selectChoice(
    choiceId: string,
    questFlags: ReadonlySet<string>,
    collectedEvidence: ReadonlySet<string>
  ): {
    nextNode: DialogueNode | null;
    choice: DialogueChoice | null;
    ended: boolean;
  } {
    if (!this.activeTree || !this.activeNode) {
      return { nextNode: null, choice: null, ended: true };
    }

    const available = this.getAvailableChoices(
      this.activeNode,
      questFlags,
      collectedEvidence
    );
    const choice =
      available.find((c) => c.id === choiceId) ??
      this.activeNode.choices.find((c) => c.id === choiceId) ??
      null;

    if (!choice) {
      return { nextNode: this.activeNode, choice: null, ended: false };
    }

    if (!choice.nextNodeId) {
      this.activeTree = null;
      this.activeNode = null;
      return { nextNode: null, choice, ended: true };
    }

    const nextNode = this.activeTree.nodes[choice.nextNodeId] ?? null;
    if (!nextNode) {
      this.activeTree = null;
      this.activeNode = null;
      return { nextNode: null, choice, ended: true };
    }

    this.activeNode = nextNode;
    return { nextNode, choice, ended: false };
  }

  public closeDialogue(): void {
    this.activeTree = null;
    this.activeNode = null;
  }
}
