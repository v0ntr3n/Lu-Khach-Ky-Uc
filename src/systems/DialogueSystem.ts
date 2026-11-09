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
        role: 'Đầu mối Liên lạc Cửu Long (Nhân vật dẫn truyện hư cấu)',
        text: '“Đồng chí tới đúng giờ lắm! Mưa đêm Hương Cảng đang giúp chúng ta che mắt mật thám. Từ năm 1925, báo Thanh Niên và cuốn Đường Kách Mệnh do đồng chí Nguyễn Ái Quốc biên soạn đã theo các chuyến tàu bí mật từ Quảng Châu qua bến cảng này về nước.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, tr. 44–49)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2, tr. 44–49',
          historicalNote:
            'Nhân vật Lâm Văn Bính là giao liên hư cấu đại diện cho mạng lưới liên lạc bí mật tại Hương Cảng năm 1930.',
        },
        choices: [
          {
            id: 'c_liaison_ask_warehouse',
            text: 'Hỏi về nơi cất giữ Sổ Vận Đơn và báo Thanh Niên tại bến cảng',
            nextNodeId: 'warehouse_guide',
            grantsFlag: 'ch1_met_liaison',
            grantsMilestone: 'I',
            responseToast: '✓ Đã nhận mật hiệu liên lạc tại Quảng Trường Bến Cảng!',
          },
          {
            id: 'c_liaison_ask_unification',
            text: 'Vì sao phải gấp rút triệu tập Hội nghị hợp nhất đầu năm 1930?',
            nextNodeId: 'unification_context',
            grantsFlag: 'ch1_met_liaison',
            grantsMilestone: 'I',
          },
          {
            id: 'c_liaison_leave',
            text: 'Tôi sẽ đi kiểm tra quanh bến cảng ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch1_met_liaison',
          },
        ],
      },
      warehouse_guide: {
        id: 'warehouse_guide',
        speaker: 'Giao Liên Lâm Văn Bính',
        role: 'Đầu mối Liên lạc Cửu Long (Nhân vật dẫn truyện hư cấu)',
        text: '“Cánh cửa Kho Hàng Bến Cảng nằm ngay phía đông quảng trường (x=512, y=400). Trong đó có Sổ Vận Đơn ghi lại các chuyến hàng chuyển báo Thanh Niên. Sau khi thu thập xong, hãy sang Phố Cổ Cửu Long để vào Nhà An Toàn.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn Dẫn truyện Hư cấu (Fictional Gameplay Guidance)',
          historicalNote:
            'Hướng dẫn không gian bản đồ giúp người chơi tìm Kho Hàng Số 4 và Phố Cổ Cửu Long.',
        },
        choices: [
          {
            id: 'c_liaison_to_context',
            text: 'Cho tôi hỏi thêm về bối cảnh ba tổ chức cộng sản cuối năm 1929',
            nextNodeId: 'unification_context',
          },
          {
            id: 'c_liaison_done_1',
            text: 'Rõ rồi, tôi vào Kho Hàng Bến Cảng ngay (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      unification_context: {
        id: 'unification_context',
        speaker: 'Giao Liên Lâm Văn Bính',
        role: 'Tư liệu Lịch sử Đảng (1929 – 1930)',
        text: '“Cuối năm 1929, ba tổ chức cộng sản lần lượt ra đời: Đông Dương Cộng sản Đảng (6/1929), An Nam Cộng sản Đảng (11/1929) và Đông Dương Cộng sản Liên đoàn (9/1929). Hoạt động phân tán có nguy cơ chia rẽ lớn, nên Nguyễn Ái Quốc đã chủ động triệu tập Hội nghị hợp nhất tại Cửu Long.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, tr. 50–61)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.c & I.3.a, tr. 50–61',
        },
        choices: [
          {
            id: 'c_liaison_to_warehouse',
            text: 'Tôi cần kiểm tra tư liệu ở đâu trước tiên?',
            nextNodeId: 'warehouse_guide',
          },
          {
            id: 'c_liaison_done_2',
            text: 'Cảm ơn đồng chí, tôi lên đường ngay (Kết thúc)',
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
        role: 'Công nhân Bốc xếp Cửu Long (Nhân vật hư cấu)',
        text: '“Đêm mưa gió thế này mà chú em vẫn đi lại ngoài bến à? Cẩn thận đấy, đèn tuần tra quét dọc kho hàng gắt lắm. Cứ nép dưới Mái Hiên hoặc lán gỗ ven bến thì đèn rọi không thấy đâu.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân vật Quần chúng Hư cấu (Fictional Historical Witness)',
          historicalNote:
            'Tái hiện tinh thần cảnh giác và sự che chở của công nhân bến cảng đối với các liên lạc viên cách mạng.',
        },
        choices: [
          {
            id: 'c_worker_ask_movement',
            text: 'Hỏi về phong trào công nhân và chủ trương “vô sản hóa” (1928–1929)',
            nextNodeId: 'worker_movement',
            grantsFlag: 'ch1_talked_worker',
          },
          {
            id: 'c_worker_ask_pier',
            text: 'Có lối nào vòng tránh Hẻm Tối tuần tra không bác?',
            nextNodeId: 'worker_shortcut',
            grantsFlag: 'ch1_talked_worker',
          },
          {
            id: 'c_worker_leave',
            text: 'Cháu cảm ơn bác (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      worker_movement: {
        id: 'worker_movement',
        speaker: 'Bác Công Nhân Bến Cảng',
        role: 'Tư liệu Phong trào Công nhân (1928–1929)',
        text: '“Từ khi Hội Việt Nam Cách mạng Thanh niên thực hiện chủ trương ‘vô sản hóa’ năm 1928, nhiều hội viên đã vào tận nhà máy, bến cảng, hầm mỏ cùng ăn, cùng ở, cùng làm với công nhân để tuyên truyền cách mạng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, tr. 48–50)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.b, tr. 48–50',
        },
        choices: [
          {
            id: 'c_worker_back_shortcut',
            text: 'Hỏi thêm về ngõ đi xuống Bến Thuyền Nhỏ',
            nextNodeId: 'worker_shortcut',
          },
          {
            id: 'c_worker_done_1',
            text: 'Cháu đã ghi nhớ tư liệu này (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      worker_shortcut: {
        id: 'worker_shortcut',
        speaker: 'Bác Công Nhân Bến Cảng',
        role: 'Công nhân Bốc xếp Cửu Long (Nhân vật hư cấu)',
        text: '“Chú cứ đi xuống mép nam quảng trường là tới Bến Thuyền Nhỏ. Hỏi Lão Chèo Đò dưới đó, ông ấy nắm rõ giờ đổi ca tuần tra và có đường tắt dẫn thẳng tới Sân Kho Lưu Trữ.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn Lối đi Phụ (Fictional Route Hint)',
        },
        choices: [
          {
            id: 'c_worker_done_2',
            text: 'Cảm ơn bác, cháu sẽ ghé Bến Thuyền Nhỏ (Kết thúc)',
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
        role: 'Hoa tiêu Đêm Hương Cảng (Nhân vật hư cấu)',
        text: '“Ngọn đèn bão ở đầu mỗi khu phố chính là Đèn Báo An Toàn (Checkpoint). Khi kích hoạt đèn bằng phím [E], bạn có thể mở Bản Đồ Khu Vực [M] để dịch chuyển nhanh giữa các điểm đã mở khóa.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Hướng dẫn Cơ chế Khám phá (Gameplay Tutorial NPC)',
        },
        choices: [
          {
            id: 'c_watchman_ask_stealth',
            text: 'Làm sao để tránh đèn rọi tuần tra trên đường phố?',
            nextNodeId: 'stealth_tip',
            grantsFlag: 'ch1_talked_watchman',
          },
          {
            id: 'c_watchman_leave',
            text: 'Cảm ơn ông, tôi đã hiểu (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      stealth_tip: {
        id: 'stealth_tip',
        speaker: 'Người Gác Đèn Bến Cảng',
        role: 'Hoa tiêu Đêm Hương Cảng (Nhân vật hư cấu)',
        text: '“Nhớ kỹ: cúi người bằng phím [S] giữa đường trống KHÔNG giúp bạn tàng hình trước đèn tuần tra đâu! Bạn bắt buộc phải đứng bên trong vùng Mái Hiên hoặc Bóng Râm mới thực sự an toàn.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Hướng dẫn Cơ chế Ẩn nấp (Stealth Rule Reminder)',
        },
        choices: [
          {
            id: 'c_watchman_done',
            text: 'Tôi sẽ luôn tìm Mái Hiên khi đèn quét qua (Kết thúc)',
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
        role: 'Người giữ Sổ Vận Đơn Kho Số 4 (Nhân vật hư cấu)',
        text: '“Trên bàn gỗ phía bắc nhà kho là Sổ Vận Đơn ghi chép các kiện hàng sách báo từ Quảng Châu. Mỗi số báo Thanh Niên từ tháng 6/1925 đều được đóng gói kỹ trong thùng gỗ để chuyển về Sài Gòn, Hải Phòng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, tr. 44–49)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.a, tr. 44–49',
        },
        choices: [
          {
            id: 'c_keeper_ask_thanh_nien',
            text: 'Hỏi về vai trò của báo Thanh Niên và tác phẩm Đường Kách Mệnh',
            nextNodeId: 'thanh_nien_detail',
            grantsFlag: 'ch1_talked_warehouse_keeper',
          },
          {
            id: 'c_keeper_leave',
            text: 'Để tôi kiểm tra bàn Sổ Vận Đơn ngay (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      thanh_nien_detail: {
        id: 'thanh_nien_detail',
        speaker: 'Thủ Kho Lưu Trữ Cửu Long',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021)',
        text: '“Báo Thanh Niên ra số đầu ngày 21/6/1925. Còn cuốn Đường Kách Mệnh (1927) tập hợp các bài giảng của Nguyễn Ái Quốc tại Quảng Châu, chỉ rõ cách mạng muốn thắng lợi trước hết phải có ‘Đảng cách mệnh’ vững mạnh.”',
        provenance: {
          type: 'historical_fact',
          label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 45–48)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.a, tr. 45–48',
        },
        choices: [
          {
            id: 'c_keeper_done',
            text: 'Cảm ơn đồng chí thủ kho (Kết thúc)',
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
        role: 'Cơ sở Cảnh giới Phố Cổ Cửu Long (Nhân vật hư cấu)',
        text: '“Khách đi đường ghé uống chén trà nóng đã! Căn Nhà An Toàn Giao Liên nằm ngay phía đông bắc con phố này, còn đi thẳng về phía đông là vào Hẻm Tối có lính tuần tra đấy.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân vật Quần chúng Hư cấu (Fictional Historical Witness)',
        },
        choices: [
          {
            id: 'c_vendor_ask_safehouse',
            text: 'Hỏi mật hiệu vào Nhà An Toàn Giao Liên',
            nextNodeId: 'safehouse_hint',
            grantsFlag: 'ch1_talked_vendor',
          },
          {
            id: 'c_vendor_ask_leaflet',
            text: 'Hỏi về tờ truyền đơn dán trên bảng tin đầu phố',
            nextNodeId: 'leaflet_hint',
            grantsFlag: 'ch1_talked_vendor',
          },
          {
            id: 'c_vendor_leave',
            text: 'Cảm ơn bà, cháu đi ngay (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      safehouse_hint: {
        id: 'safehouse_hint',
        speaker: 'Bà Hàng Nước Phố Cổ',
        role: 'Cơ sở Cảnh giới Phố Cổ Cửu Long (Nhân vật hư cấu)',
        text: '“Cửa Nhà An Toàn mở sẵn cho người có tín hiệu từ bến cảng. Trong đó đồng chí Trần Thị Liên đang giữ bức Mật Tín Triệu Tập Hội Nghị Hợp Nhất.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn Nhiệm vụ Chính (Main Quest Guidance)',
        },
        choices: [
          {
            id: 'c_vendor_done_1',
            text: 'Cháu sẽ vào Nhà An Toàn ngay (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      leaflet_hint: {
        id: 'leaflet_hint',
        speaker: 'Bà Hàng Nước Phố Cổ',
        role: 'Cơ sở Cảnh giới Phố Cổ Cửu Long (Nhân vật hư cấu)',
        text: '“Tờ truyền đơn công nhân dán ở bảng tin gần đây phản ánh phong trào đấu tranh sôi nổi năm 1929. Cháu nhớ lại gần bảng tin bấm [E] để lưu vào Túi Tư Liệu [I] nhé.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Gợi ý Thu thập Chứng cứ (Evidence Hint)',
        },
        choices: [
          {
            id: 'c_vendor_done_2',
            text: 'Cháu cảm ơn bà (Kết thúc)',
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
        role: 'Phụ trách Nhà An Toàn Cửu Long (Nhân vật hư cấu)',
        text: '“Chào đồng chí! Từ ngày 06/01 đến ngày 07/02/1930 tại bán đảo Cửu Long (Hương Cảng), đồng chí Nguyễn Ái Quốc đã chủ trì Hội nghị hợp nhất các tổ chức cộng sản thành một chính đảng duy nhất: Đảng Cộng sản Việt Nam.”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, tr. 57–61)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–61',
          historicalNote:
            'Nhân vật Trần Thị Liên là giao liên hư cấu; thời gian, địa điểm và nội dung Hội nghị thành lập Đảng là sự kiện lịch sử xác thực.',
        },
        choices: [
          {
            id: 'c_inner_ask_delegates',
            text: 'Những tổ chức nào đã tham dự Hội nghị hợp nhất tại Cửu Long?',
            nextNodeId: 'delegates_node',
            grantsFlag: 'ch1_safehouse_cipher',
            grantsEvidence: 'ev_ch1_cipher_note',
            grantsMilestone: 'II',
            responseToast: '✓ Đã khôi phục Cột mốc II & Mật Tín Hội Nghị Hợp Nhất!',
          },
          {
            id: 'c_inner_ask_next',
            text: 'Sau khi hợp nhất, văn kiện Cương lĩnh được lưu giữ ở đâu?',
            nextNodeId: 'archive_direction',
            grantsFlag: 'ch1_safehouse_cipher',
            grantsEvidence: 'ev_ch1_cipher_note',
            grantsMilestone: 'II',
          },
          {
            id: 'c_inner_leave',
            text: 'Tôi sẽ tiếp tục hành trình tới Phòng Lưu Trữ Bí Mật (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      delegates_node: {
        id: 'delegates_node',
        speaker: 'Giao Liên Trần Thị Liên',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 57–61)',
        text: '“Tham dự Hội nghị có đại biểu của Đông Dương Cộng sản Đảng (Trịnh Đình Cửu, Nguyễn Đức Cảnh) và An Nam Cộng sản Đảng (Châu Văn Liêm, Nguyễn Thiệu) dưới sự chủ trì của Nguyễn Ái Quốc. Đến ngày 24/02/1930, Đông Dương Cộng sản Liên đoàn cũng chính thức gia nhập Đảng Cộng sản Việt Nam.”',
        provenance: {
          type: 'historical_fact',
          label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 57–65)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–65',
        },
        choices: [
          {
            id: 'c_inner_to_archive_dir',
            text: 'Chỉ đường cho tôi tới Phòng Lưu Trữ Bí Mật',
            nextNodeId: 'archive_direction',
          },
          {
            id: 'c_inner_done_1',
            text: 'Tôi đã ghi chép đầy đủ vào Sổ Tay [J] (Kết thúc)',
            nextNodeId: null,
          },
        ],
      },
      archive_direction: {
        id: 'archive_direction',
        speaker: 'Giao Liên Trần Thị Liên',
        role: 'Phụ trách Nhà An Toàn Cửu Long (Nhân vật hư cấu)',
        text: '“Đồng chí hãy rời Nhà An Toàn ra Phố Cổ, đi qua Hẻm Tối Tuần Tra (hoặc đi đường vòng qua Bến Thuyền Nhỏ) để tới Sân Kho Lưu Trữ và vào Phòng Lưu Trữ Bí Mật khôi phục Cột mốc III.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn Lộ trình (Gameplay Route Guidance)',
        },
        choices: [
          {
            id: 'c_inner_done_2',
            text: 'Rõ rồi, tôi xuất phát ngay (Kết thúc)',
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
        role: 'Người đưa đò đêm Cửu Long (Nhân vật hư cấu)',
        text: '“Chú em muốn sang Sân Kho Lưu Trữ mà không muốn đụng độ đèn tuần tra ở Hẻm Tối chứ gì? Ta đã ghi sẵn Lịch Đổi Ca Tuần Tra để trên bàn gỗ cuối cầu tàu kia kìa.”',
        provenance: {
          type: 'fictional_npc',
          label: 'Nhân vật Phụ Hư cấu (Fictional Side-Quest NPC)',
          historicalNote:
            'Nhân vật hư cấu hỗ trợ nhiệm vụ phụ Đêm Gác Bến Thuyền Nhỏ.',
        },
        choices: [
          {
            id: 'c_boatman_accept',
            text: 'Cảm ơn cụ! Cháu sẽ lấy Lịch Đổi Ca trên cầu tàu ngay',
            nextNodeId: 'boatman_route_detail',
            grantsFlag: 'ch1_talked_boatman',
            responseToast: '📜 Đã cập nhật Nhiệm Vụ Phụ: Đêm Gác Bến Thuyền Nhỏ!',
          },
          {
            id: 'c_boatman_leave',
            text: 'Cháu chỉ ghé qua thăm bến thuyền thôi (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch1_talked_boatman',
          },
        ],
      },
      boatman_route_detail: {
        id: 'boatman_route_detail',
        speaker: 'Lão Chèo Đò Bến Thuyền',
        role: 'Người đưa đò đêm Cửu Long (Nhân vật hư cấu)',
        text: '“Sau khi lấy tờ Lịch Đổi Ca trên bàn gỗ, chú cứ đi theo cổng phía đông Bến Thuyền Nhỏ là thông thẳng sang Sân Kho Lưu Trữ an toàn tuyệt đối!”',
        provenance: {
          type: 'fictional_npc',
          label: 'Chỉ dẫn Lối đi Phụ An toàn (Optional Safe Route)',
        },
        choices: [
          {
            id: 'c_boatman_done',
            text: 'Cháu cảm ơn cụ nhiều lắm (Kết thúc)',
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
        role: 'Người trông coi Sân Kho Lưu Trữ (Nhân vật hư cấu)',
        text: '“Đây là Sân Kho Lưu Trữ Cửu Long. Cánh cửa phía đông dẫn vào Phòng Lưu Trữ Bí Mật, nơi đặt bản Chánh cương vắn tắt, Sách lược vắn tắt (02/1930), Luận cương chính trị (10/1930) và Cổng Thời Gian Chương I.”',
        provenance: {
          type: 'historical_fact',
          label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 61–76)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b & II.1, tr. 61–76',
        },
        choices: [
          {
            id: 'c_archive_compare_1930',
            text: 'Hỏi về nội dung cốt lõi của Cương lĩnh chính trị đầu tiên (02/1930)',
            nextNodeId: 'platform_core',
            grantsFlag: 'ch1_archive_clearance',
          },
          {
            id: 'c_archive_enter',
            text: 'Tôi sẽ tiến vào Phòng Lưu Trữ Bí Mật ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch1_archive_clearance',
          },
        ],
      },
      platform_core: {
        id: 'platform_core',
        speaker: 'Ông Từ Giữ Kho Lưu Trữ',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 61–65)',
        text: '“Cương lĩnh chính trị đầu tiên xác định nhiệm vụ hàng đầu là chống đế quốc giành độc lập dân tộc (‘làm cho nước Nam được hoàn toàn độc lập’), đồng thời đoàn kết rộng rãi công nhân, nông dân và các tầng lớp yêu nước dưới sự lãnh đạo của Đảng.”',
        provenance: {
          type: 'historical_fact',
          label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 61–65)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b, tr. 61–65',
        },
        choices: [
          {
            id: 'c_archive_done',
            text: 'Cảm ơn ông, tôi đã sẵn sàng bước vào Phòng Lưu Trữ Bí Mật (Kết thúc)',
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
        role: 'Liên lạc viên Khu Giải phóng Việt Bắc (Nhân vật hư cấu)',
        text: '“Chào đồng chí! Từ Hội nghị Trung ương 8 tại Pác Bó (05/1941), Đảng đã đặt nhiệm vụ giải phóng dân tộc lên hàng đầu và thành lập Mặt trận Việt Minh. Nay tại Tân Trào (08/1945), lệnh Tổng khởi nghĩa đang chờ phát đi khắp cả nước!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, Chương I, tr. 88–112)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.2–II.3, tr. 88–112',
          historicalNote:
            'Nhân vật Hoàng Quốc Bảo là giao liên hư cấu đại diện cho lực lượng liên lạc Việt Minh năm 1941–1945.',
        },
        choices: [
          {
            id: 'c_ch2_ask_tw8',
            text: 'Hỏi về sự chuyển hướng chiến lược tại Hội nghị Trung ương 8 (05/1941)',
            nextNodeId: 'tw8_detail',
            grantsFlag: 'ch2_met_messenger',
            grantsMilestone: 'IV',
            responseToast: '✓ Đã khôi phục Cột mốc IV: Hội nghị Trung ương 8 (05/1941)!',
          },
          {
            id: 'c_ch2_leave',
            text: 'Tôi sẽ tới Bàn Điều Phối Mạng Lưới Giao Liên ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch2_met_messenger',
            grantsMilestone: 'IV',
          },
        ],
      },
      tw8_detail: {
        id: 'tw8_detail',
        speaker: 'Giao Liên Việt Minh Hoàng Quốc Bảo',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 88–95)',
        text: '“Hội nghị Trung ương 8 khẳng định: ‘Trong lúc này quyền lợi của bộ phận, của giai cấp phải đặt dưới sự sinh tử, tồn vong của quốc gia, của dân tộc.’ Hãy lại Bàn Điều Phối Mạng Lưới Giao Liên phía đông để phát Quân lệnh số 1!”',
        provenance: {
          type: 'historical_fact',
          label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 88–95)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.2.b, tr. 88–95',
        },
        choices: [
          {
            id: 'c_ch2_done',
            text: 'Rõ rồi, tôi tới Bàn Điều Phối ngay (Kết thúc)',
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
        role: 'Nhân chứng Ngày Độc Lập 02/09/1945 (Nhân vật hư cấu)',
        text: '“Giữa rừng cờ đỏ sao vàng tại Quảng trường Ba Đình ngày 02/09/1945, Chủ tịch Hồ Chí Minh thay mặt Chính phủ lâm thời đọc bản Tuyên ngôn Độc lập, trịnh trọng tuyên bố với thế giới sự ra đời của nước Việt Nam Dân chủ Cộng hòa!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, Chương I, tr. 117–124)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.c, tr. 117–124',
        },
        choices: [
          {
            id: 'c_badinh_ask_significance',
            text: 'Hỏi về ý nghĩa lịch sử của Cách mạng Tháng Tám năm 1945',
            nextNodeId: 'badinh_significance',
            grantsFlag: 'ch2_met_witness',
            grantsMilestone: 'VI',
            responseToast: '✓ Đã khôi phục Cột mốc VI: Tuyên ngôn Độc lập (02/09/1945)!',
          },
          {
            id: 'c_badinh_leave',
            text: 'Tôi sẽ tiến tới Cổng Thời Gian Chương II ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch2_met_witness',
            grantsMilestone: 'VI',
          },
        ],
      },
      badinh_significance: {
        id: 'badinh_significance',
        speaker: 'Đội Viên Tự Vệ Thành Hoàng Diệu',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 119–124)',
        text: '“Cách mạng Tháng Tám năm 1945 đã đập tan xiềng xích nô lệ của thực dân đế quốc trong gần một thế kỷ, chấm dứt sự tồn tại của chế độ quân chủ chuyên chế ngót nghìn năm, đưa nhân dân ta từ thân phận nô lệ lên địa vị người làm chủ đất nước.”',
        provenance: {
          type: 'historical_fact',
          label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 119–124)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.d, tr. 119–124',
        },
        choices: [
          {
            id: 'c_badinh_done',
            text: 'Cảm ơn đồng chí, tôi tới Cổng Thời Gian đây (Kết thúc)',
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
        role: 'Chỉ huy Đội Xe thồ Đèo Pha Đin (Nhân vật hư cấu)',
        text: '“Từ Lời kêu gọi Toàn quốc kháng chiến (19/12/1946) với đường lối kháng chiến toàn dân, toàn diện, lâu dài và dựa vào sức mình là chính, hôm nay hàng vạn dân công và xe đạp thồ đang vượt đèo Pha Đin tiếp lửa cho chiến trường Điện Biên Phủ!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, Chương II, tr. 142–182)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục I.1–I.3, tr. 142–182',
        },
        choices: [
          {
            id: 'c_phadin_ask_1946',
            text: 'Hỏi về đường lối kháng chiến chống thực dân Pháp (1946–1954)',
            nextNodeId: 'phadin_doctrine',
            grantsFlag: 'ch3_met_quartermaster',
            grantsMilestone: 'VII',
            responseToast: '✓ Đã khôi phục Cột mốc VII: Đường lối Kháng chiến Toàn dân!',
          },
          {
            id: 'c_phadin_leave',
            text: 'Tôi sẽ tới Sa Bàn Tiếp Vận Pha Đin ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch3_met_quartermaster',
            grantsMilestone: 'VII',
          },
        ],
      },
      phadin_doctrine: {
        id: 'phadin_doctrine',
        speaker: 'Đội Trưởng Dân Công Hậu Cần',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 142–182)',
        text: '“Đường lối kháng chiến của Đảng được kết tinh trong Lời kêu gọi Toàn quốc kháng chiến của Hồ Chí Minh, Chỉ thị Toàn dân kháng chiến của Trung ương Đảng và tác phẩm Kháng chiến nhất định thắng lợi của Tổng Bí thư Trường Chinh.”',
        provenance: {
          type: 'historical_fact',
          label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 142–150)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục I.1.b, tr. 142–150',
        },
        choices: [
          {
            id: 'c_phadin_done',
            text: 'Tôi sẽ tới Sa Bàn Tiếp Vận để điều phối chiến dịch (Kết thúc)',
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
        role: 'Binh chủng Thông tin Liên lạc Mùa Xuân 1975 (Nhân vật hư cấu)',
        text: '“Bức công điện lịch sử của Đại tướng Võ Nguyên Giáp vừa truyền đi trên toàn mặt trận: ‘Thần tốc, thần tốc hơn nữa; táo bạo, táo bạo hơn nữa... Quyết chiến và toàn thắng!’ Ngày 30/04/1975 đã thu non sông về một mối!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, Chương II, tr. 228–238)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục II.2.c, tr. 228–238',
        },
        choices: [
          {
            id: 'c_1975_ask_victory',
            text: 'Hỏi về ý nghĩa của Đại thắng mùa Xuân năm 1975',
            nextNodeId: 'victory_1975_node',
            grantsFlag: 'ch3_met_signal',
            grantsMilestone: 'IX',
            responseToast: '✓ Đã khôi phục Cột mốc IX: Đại thắng mùa Xuân 30/04/1975!',
          },
          {
            id: 'c_1975_leave',
            text: 'Tôi sẽ tiến tới Cổng Thời Gian Chương III ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch3_met_signal',
            grantsMilestone: 'IX',
          },
        ],
      },
      victory_1975_node: {
        id: 'victory_1975_node',
        speaker: 'Chiến Sĩ Thông Tin Chiến Dịch Hồ Chí Minh',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 232–238)',
        text: '“Thắng lợi của cuộc kháng chiến chống Mỹ, cứu nước đã kết thúc 21 năm chiến đấu kiên cường, hoàn thành cuộc cách mạng dân tộc dân chủ nhân dân trên phạm vi cả nước, mở ra kỷ nguyên cả nước độc lập, thống nhất, đi lên chủ nghĩa xã hội.”',
        provenance: {
          type: 'historical_fact',
          label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 232–238)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, mục II.3, tr. 232–238',
        },
        choices: [
          {
            id: 'c_1975_done',
            text: 'Cảm ơn đồng chí, tôi tới Cổng Thời Gian ngay (Kết thúc)',
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
        role: 'Cán bộ Nghiên cứu Chính sách Kinh tế 1986 (Nhân vật hư cấu)',
        text: '“Tháng 12/1986, Đại hội đại biểu toàn quốc lần thứ VI của Đảng đã họp tại Hà Nội với tinh thần ‘nhìn thẳng vào sự thật, đánh giá đúng sự thật, nói rõ sự thật’, khởi xướng đường lối Đổi mới toàn diện đất nước!”',
        provenance: {
          type: 'historical_fact',
          label: 'Sự kiện Lịch sử (Giáo trình 2021, Chương III, tr. 261–268)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục I.2.a, tr. 261–268',
        },
        choices: [
          {
            id: 'c_ch4_ask_doimoi',
            text: 'Hỏi về trọng tâm đổi mới kinh tế tại Đại hội VI (1986)',
            nextNodeId: 'doimoi_detail',
            grantsFlag: 'ch4_met_economist',
            grantsMilestone: 'X',
            responseToast: '✓ Đã khôi phục Cột mốc X: Đại hội VI Khởi xướng Đổi mới (12/1986)!',
          },
          {
            id: 'c_ch4_leave',
            text: 'Tôi sẽ tới Bàn Ma Trận Quyết Sách Đổi Mới ngay (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch4_met_economist',
            grantsMilestone: 'X',
          },
        ],
      },
      doimoi_detail: {
        id: 'doimoi_detail',
        speaker: 'Chuyên Gia Nghiên Cứu Đổi Mới',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 261–268)',
        text: '“Đại hội VI chủ trương thực hiện Ba chương trình kinh tế lớn (Lương thực - thực phẩm, Hàng tiêu dùng, Hàng xuất khẩu), xóa bỏ cơ chế tập trung quan liêu bao cấp và phát triển nền kinh tế nhiều thành phần.”',
        provenance: {
          type: 'historical_fact',
          label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 261–268)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục I.2.a, tr. 261–268',
        },
        choices: [
          {
            id: 'c_ch4_done',
            text: 'Tôi sẽ tới giải Ma Trận Quyết Sách ngay (Kết thúc)',
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
        role: 'Điều phối viên Trung tâm Ký Ức Số (Bối cảnh Viễn cảnh 2045)',
        text: '“Chào mừng Lữ Khách trở về Quảng trường Kỷ Nguyên Số năm 2045! Không gian kiến trúc bạn đang đứng là hình ảnh viễn cảnh nghệ thuật (speculative_2045), được kiến tạo từ mục tiêu phát triển đất nước đến năm 2030, tầm nhìn 2045 trong Nghị quyết Đại hội XIII (2021).”',
        provenance: {
          type: 'speculative_2045',
          label: 'Viễn cảnh Nghệ thuật 2045 dựa trên Văn kiện Đại hội XIII',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục II.4, tr. 385–393',
          historicalNote:
            'Không gian năm 2045 là bối cảnh dẫn truyện tương lai; còn mục tiêu năm 2030 và 2045 là nội dung văn kiện lịch sử Đại hội XIII (01/2021).',
        },
        choices: [
          {
            id: 'c_2045_ask_xii',
            text: 'Đối chiếu mục tiêu cụ thể năm 2030 và 2045 của Đại hội XIII (01/2021)',
            nextNodeId: 'congress_xiii_fact',
            grantsFlag: 'ch4_met_archivist_2045',
            grantsMilestone: 'XII',
            responseToast: '✓ Đã khôi phục Cột mốc XII: Đại hội XIII & Tầm nhìn Việt Nam 2045!',
          },
          {
            id: 'c_2045_leave',
            text: 'Tôi đã sẵn sàng bước tới Cổng Thời Gian 2045 (Kết thúc)',
            nextNodeId: null,
            grantsFlag: 'ch4_met_archivist_2045',
            grantsMilestone: 'XII',
          },
        ],
      },
      congress_xiii_fact: {
        id: 'congress_xiii_fact',
        speaker: 'Giám Tuyển Lưu Trữ Quốc Gia 2045',
        role: 'Tư liệu Giáo trình Lịch sử Đảng (2021, tr. 385–393)',
        text: '“Nghị quyết Đại hội XIII xác định mục tiêu đến năm 2030 (kỷ niệm 100 năm thành lập Đảng) là nước đang phát triển, có công nghiệp hiện đại, thu nhập trung bình cao; và đến năm 2045 (kỷ niệm 100 năm thành lập nước) trở thành nước phát triển, thu nhập cao.”',
        provenance: {
          type: 'historical_fact',
          label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 385–393)',
          citation:
            'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, mục II.4, tr. 385–393',
        },
        choices: [
          {
            id: 'c_2045_done',
            text: 'Hoàn tất đối chiếu và mở Cổng Thời Gian 2045 (Kết thúc)',
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
