/**
 * @file src/data/chapters/ch2_3_4_mechanics.ts
 * @description Cơ chế Giải đố / Chiến lược riêng biệt cho Chương II (1945), Chương III (1954–1975)
 * và Chương IV (1986–2045) theo đúng thiết kế Four Era Mechanics trong docs/plans/RPG_EXPLORATION.md:
 * - Chương II (1945): Điều Phối Mạng Lưới Giao Liên & Mật Điện Tổng Khởi Nghĩa (Network Coordination)
 * - Chương III (1954–1975): Sa Bàn Điều Phối Tuyến Tiếp Vận Pha Đin & Chiến Lược Hai Miền (Supply Logistics)
 * - Chương IV (1986–2045): Ma Trận Nhân Quả Quyết Sách Đổi Mới & Tầm Nhìn 2045 (Causal Policy Matrix)
 */

import type { DossierPuzzleDefinition, EraId } from '../../types/game';
import { CH1_DOSSIER_PUZZLE } from './ch1_1930';

export const CH2_DISPATCH_NETWORK_PUZZLE: DossierPuzzleDefinition = {
  id: 'puzzle_ch2_dispatch_network',
  eraId: '1945',
  zoneId: 'tan_trao_banyan',
  title: 'ĐIỀU PHỐI MẠNG LƯỚI GIAO LIÊN & MẬT ĐIỆN TỔNG KHỞI NGHĨA (08/1945)',
  subtitle:
    'Xác lập cửa sổ thời cơ, trình tự nổi dậy giành chính quyền và đích đến pháp lý tại Quảng trường Ba Đình',
  instruction:
    'Hãy điều phối mạng lưới liên lạc từ Tân Trào tới các địa phương theo đúng tiến trình lịch sử Cách mạng Tháng Tám 1945:',
  slots: [
    {
      id: 'slot_ch2_time_window',
      label: '1. Cửa sổ thời cơ chiến lược phát lệnh Tổng khởi nghĩa',
      prompt:
        'Hội nghị toàn quốc của Đảng tại Tân Trào (14–15/08/1945) xác định thời cơ “ngàn năm có một” để phát động Tổng khởi nghĩa là khi nào?',
      options: [
        {
          id: 'opt_ch2_time_authentic',
          slotId: 'slot_ch2_time_window',
          text: 'Chớp đúng thời cơ phát xít Nhật đầu hàng Đồng minh (8/1945), phát động toàn dân nổi dậy trước khi quân Đồng minh vào Đông Dương.',
          isAuthentic: true,
          explanation:
            'Chính xác: Đảng kịp thời phát động toàn dân tổng khởi nghĩa giành chính quyền trong khoảng trống quyền lực sau khi Nhật đầu hàng.',
        },
        {
          id: 'opt_ch2_time_wrong_1',
          slotId: 'slot_ch2_time_window',
          text: 'Chờ quân đội Đồng minh tiến vào giải giáp quân Nhật rồi mới thương lượng xin chuyển giao chính quyền.',
          isAuthentic: false,
          explanation:
            'Sai: Nếu chờ quân Đồng minh vào mới hành động sẽ đánh mất thời cơ tự giải phóng dân tộc.',
        },
        {
          id: 'opt_ch2_time_wrong_2',
          slotId: 'slot_ch2_time_window',
          text: 'Chỉ tổ chức khởi nghĩa cục bộ ở vùng rừng núi Việt Bắc mà không tiến vào các đô thị lớn.',
          isAuthentic: false,
          explanation:
            'Sai: Tổng khởi nghĩa Tháng Tám kết hợp chặt chẽ cả nông thôn và thành thị trên toàn quốc.',
        },
      ],
    },
    {
      id: 'slot_ch2_route_sequence',
      label: '2. Tuyến truyền tin & nhịp độ giành chính quyền tại các đô thị trọng điểm',
      prompt:
        'Sau khi xuất quân từ Tân Trào, thắng lợi dây chuyền tại ba đô thị lớn diễn ra theo trình tự thời gian nào?',
      options: [
        {
          id: 'opt_ch2_route_authentic',
          slotId: 'slot_ch2_route_sequence',
          text: 'Phối hợp khởi nghĩa từng phần tiến lên Tổng khởi nghĩa thắng lợi dây chuyền: Hà Nội (19/8) -> Huế (23/8) -> Sài Gòn (25/8).',
          isAuthentic: true,
          explanation:
            'Chính xác: Thắng lợi ở Hà Nội ngày 19/8 có ý nghĩa quyết định, cổ vũ Huế (23/8) và Sài Gòn (25/8) giành chính quyền.',
        },
        {
          id: 'opt_ch2_route_wrong_1',
          slotId: 'slot_ch2_route_sequence',
          text: 'Khởi nghĩa tại Sài Gòn trước từ tháng 3/1945 rồi mới lan ra Huế và Hà Nội vào cuối tháng 9/1945.',
          isAuthentic: false,
          explanation:
            'Sai: Tháng 3/1945 là thời điểm ban hành Chỉ thị “Nhật - Pháp bắn nhau và hành động của chúng ta”, mở đầu cao trào kháng Nhật cứu nước.',
        },
        {
          id: 'opt_ch2_route_wrong_2',
          slotId: 'slot_ch2_route_sequence',
          text: 'Tạm ngừng mọi liên lạc giữa ba kỳ để mỗi địa phương tự chờ lệnh sau tháng 10/1945.',
          isAuthentic: false,
          explanation:
            'Sai: Tổng khởi nghĩa giành thắng lợi nhanh gọn trên toàn quốc chỉ trong vòng 15 ngày cuối tháng 8/1945.',
        },
      ],
    },
    {
      id: 'slot_ch2_state_foundation',
      label: '3. Đích đến pháp lý & lịch sử tại Quảng trường Ba Đình',
      prompt:
        'Sự kiện đỉnh cao khép lại thắng lợi của Cách mạng Tháng Tám tại Quảng trường Ba Đình ngày 02/09/1945 là gì?',
      options: [
        {
          id: 'opt_ch2_state_authentic',
          slotId: 'slot_ch2_state_foundation',
          text: 'Tuyên ngôn Độc lập ngày 02/09/1945 khai sinh nước Việt Nam Dân chủ Cộng hòa — Nhà nước công nông đầu tiên ở Đông Nam Á.',
          isAuthentic: true,
          explanation:
            'Chính xác: Chủ tịch Hồ Chí Minh đọc Tuyên ngôn Độc lập, mở ra kỷ nguyên độc lập, tự do cho dân tộc Việt Nam.',
        },
        {
          id: 'opt_ch2_state_wrong_1',
          slotId: 'slot_ch2_state_foundation',
          text: 'Duy trì nguyên vẹn chính quyền bảo hộ và chế độ quân chủ chuyên chế cũ.',
          isAuthentic: false,
          explanation:
            'Sai: Cách mạng Tháng Tám đã đập tan xiềng xích thực dân gần một thế kỷ và chấm dứt chế độ quân chủ phong kiến.',
        },
        {
          id: 'opt_ch2_state_wrong_2',
          slotId: 'slot_ch2_state_foundation',
          text: 'Ký Hiệp định sơ bộ chuyển giao toàn bộ quyền hành chính cho quân đội nước ngoài.',
          isAuthentic: false,
          explanation:
            'Sai: Tuyên ngôn Độc lập khẳng định nước Việt Nam có quyền hưởng tự do và độc lập, và sự thật đã thành một nước tự do, độc lập.',
        },
      ],
    },
  ],
  progressiveHint:
    'Hãy chọn phương án chớp thời cơ Nhật đầu hàng Đồng minh (8/1945), trình tự giành chính quyền Hà Nội (19/8) -> Huế (23/8) -> Sài Gòn (25/8), và Tuyên ngôn Độc lập ngày 02/09/1945.',
  textbookCitation:
    'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3, tr. 95–124',
  provenance: {
    type: 'historical_fact',
    label: 'Sự kiện Lịch sử',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3, tr. 95–124',
  },
  rewardFlag: 'ch2_network_coordinated',
  rewardEvidenceId: 'ev_ch2_tan_trao_order',
  rewardMilestone: 'V',
};

export const CH3_SUPPLY_LOGISTICS_PUZZLE: DossierPuzzleDefinition = {
  id: 'puzzle_ch3_supply_logistics',
  eraId: '1954_1975',
  zoneId: 'pha_din_pass',
  title: 'SA BÀN ĐIỀU PHỐI TUYẾN TIẾP VẬN PHA ĐIN & CHIẾN LƯỢC HAI MIỀN (1954 – 1975)',
  subtitle:
    'Phân bổ hậu cần chiến dịch Điện Biên Phủ (1954) và điều phối thế trận hai miền Nam – Bắc tiến tới Đại thắng mùa Xuân 1975',
  instruction:
    'Hãy xác lập phương án điều phối chiến dịch và hậu cần chính xác trên sa bàn lịch sử:',
  slots: [
    {
      id: 'slot_ch3_dien_bien_doctrine',
      label: '1. Phương châm tác chiến & bảo đảm hậu cần tại Điện Biên Phủ (1954)',
      prompt:
        'Quyết định lịch sử nào của Đảng ủy và Bộ Chỉ huy chiến dịch Điện Biên Phủ đã bảo đảm thắng lợi trước tập đoàn cứ điểm mạnh nhất Đông Dương?',
      options: [
        {
          id: 'opt_ch3_dbp_authentic',
          slotId: 'slot_ch3_dien_bien_doctrine',
          text: 'Chuyển từ "Đánh nhanh, giải quyết nhanh" sang "Đánh chắc, tiến chắc", huy động sức mạnh toàn dân và đoàn xe thồ vượt đèo Pha Đin.',
          isAuthentic: true,
          explanation:
            'Chính xác: Quyết định chuyển phương châm tác chiến cùng mạng lưới hậu cần nhân dân đã làm nên chiến thắng Điện Biên Phủ 07/05/1954.',
        },
        {
          id: 'opt_ch3_dbp_wrong_1',
          slotId: 'slot_ch3_dien_bien_doctrine',
          text: 'Dốc toàn lực đánh ồ ạt trong một đêm duy nhất mà không cần chuẩn bị công sự hay tuyến tiếp lương.',
          isAuthentic: false,
          explanation:
            'Sai: Phương án đánh nhanh khi địch đã tăng cường phòng ngự không bảo đảm chắc thắng nên đã được thay đổi kịp thời.',
        },
        {
          id: 'opt_ch3_dbp_wrong_2',
          slotId: 'slot_ch3_dien_bien_doctrine',
          text: 'Rút toàn bộ chủ lực về đồng bằng và hủy bỏ chiến dịch Đông – Xuân 1953–1954.',
          isAuthentic: false,
          explanation:
            'Sai: Quân và dân ta kiên quyết bao vây, tiêu diệt hoàn toàn tập đoàn cứ điểm Điện Biên Phủ sau 56 ngày đêm.',
        },
      ],
    },
    {
      id: 'slot_ch3_two_strategies',
      label: '2. Đường lối tiến hành đồng thời hai chiến lược cách mạng (1954 – 1975)',
      prompt:
        'Đại hội III của Đảng (09/1960) xác định vị trí, vai trò của cách mạng hai miền Nam – Bắc như thế nào?',
      options: [
        {
          id: 'opt_ch3_two_strat_authentic',
          slotId: 'slot_ch3_two_strategies',
          text: 'Miền Bắc xây dựng chủ nghĩa xã hội làm hậu phương lớn; miền Nam đẩy mạnh cách mạng dân tộc dân chủ nhân dân làm tiền tuyến lớn.',
          isAuthentic: true,
          explanation:
            'Chính xác: Hai chiến lược có mối quan hệ mật thiết, cùng hướng tới mục tiêu giải phóng miền Nam, thống nhất Tổ quốc.',
        },
        {
          id: 'opt_ch3_two_strat_wrong_1',
          slotId: 'slot_ch3_two_strategies',
          text: 'Tách rời hoàn toàn nhiệm vụ của hai miền, miền Bắc không chi viện sức người sức của cho miền Nam.',
          isAuthentic: false,
          explanation:
            'Sai: Tuyến chi viện chiến lược Trường Sơn (Đường 559) và Đường Hồ Chí Minh trên biển luôn nối liền hậu phương lớn với tiền tuyến lớn.',
        },
        {
          id: 'opt_ch3_two_strat_wrong_2',
          slotId: 'slot_ch3_two_strategies',
          text: 'Ngừng xây dựng kinh tế ở miền Bắc trong suốt 20 năm để chỉ duy trì phòng thủ thụ động.',
          isAuthentic: false,
          explanation:
            'Sai: Miền Bắc vừa sản xuất, xây dựng CNXH vừa chiến đấu chống chiến tranh phá hoại và chi viện đắc lực cho miền Nam.',
        },
      ],
    },
    {
      id: 'slot_ch3_spring_1975_thrust',
      label: '3. Mệnh lệnh hiệp đồng binh chủng Mùa Xuân 1975',
      prompt:
        'Sau chiến thắng Tây Nguyên và Huế – Đà Nẵng, Chiến dịch Hồ Chí Minh lịch sử được triển khai với phương châm hành động nào?',
      options: [
        {
          id: 'opt_ch3_1975_authentic',
          slotId: 'slot_ch3_spring_1975_thrust',
          text: 'Phương châm "Thần tốc, táo bạo, bất ngờ, chắc thắng" trong Chiến dịch Hồ Chí Minh lịch sử kết thúc thắng lợi ngày 30/04/1975.',
          isAuthentic: true,
          explanation:
            'Chính xác: Cuộc Tổng tiến công và nổi dậy Xuân 1975 toàn thắng ngày 30/04/1975, thu non sông về một mối.',
        },
        {
          id: 'opt_ch3_1975_wrong_1',
          slotId: 'slot_ch3_spring_1975_thrust',
          text: 'Dừng lại ở vĩ tuyến 17 để đàm phán kéo dài thêm nhiều năm sau mùa mưa 1975.',
          isAuthentic: false,
          explanation:
            'Sai: Bộ Chính trị quyết định giải phóng hoàn toàn miền Nam ngay trong tháng 4/1975 trước mùa mưa.',
        },
        {
          id: 'opt_ch3_1975_wrong_2',
          slotId: 'slot_ch3_spring_1975_thrust',
          text: 'Chỉ sử dụng lực lượng nhỏ lẻ mà không tổ chức 5 cánh quân hợp围 tiến vào Sài Gòn.',
          isAuthentic: false,
          explanation:
            'Sai: Chiến dịch Hồ Chí Minh hiệp đồng 5 cánh quân lớn kết hợp nổi dậy của quần chúng tiến thẳng vào trung tâm Sài Gòn.',
        },
      ],
    },
  ],
  progressiveHint:
    'Hãy chọn phương châm "Đánh chắc, tiến chắc" tại Điện Biên Phủ (1954), đường lối hậu phương lớn miền Bắc – tiền tuyến lớn miền Nam (Đại hội III, 1960), và mệnh lệnh "Thần tốc, táo bạo, bất ngờ, chắc thắng" (Xuân 1975).',
  textbookCitation:
    'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, tr. 127–238',
  provenance: {
    type: 'historical_fact',
    label: 'Sự kiện Lịch sử',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, tr. 127–238',
  },
  rewardFlag: 'ch3_logistics_allocated',
  rewardEvidenceId: 'ev_ch3_pha_din_dispatch',
  rewardMilestone: 'VIII',
};

export const CH4_CAUSAL_POLICY_PUZZLE: DossierPuzzleDefinition = {
  id: 'puzzle_ch4_causal_policy',
  eraId: '1986_2045',
  zoneId: 'doi_moi_avenue',
  title: 'MA TRẬN NHÂN QUẢ QUYẾT SÁCH ĐỔI MỚI & TẦM NHÌN 2045 (1986 – 2045)',
  subtitle:
    'Liên kết nguyên nhân – quyết sách – thành tựu từ Đại hội VI (1986), Cương lĩnh 2011 đến mục tiêu phát triển Đại hội XIII (2030–2045)',
  instruction:
    'Hãy ghép nối đúng chuỗi nhân quả quyết sách Đổi mới và phân biệt rõ văn kiện lịch sử với viễn cảnh nghệ thuật 2045:',
  slots: [
    {
      id: 'slot_ch4_doi_moi_1986',
      label: '1. Bước ngoặt đổi mới tư duy kinh tế tại Đại hội VI (12/1986)',
      prompt:
        'Trước cuộc khủng hoảng kinh tế - xã hội trầm trọng đầu thập niên 1980, Đại hội VI (12/1986) đã đề ra quyết sách đột phá nào?',
      options: [
        {
          id: 'opt_ch4_1986_authentic',
          slotId: 'slot_ch4_doi_moi_1986',
          text: 'Xóa bỏ cơ chế tập trung quan liêu, bao cấp; phát triển kinh tế nhiều thành phần vận hành theo cơ chế thị trường có sự quản lý của Nhà nước.',
          isAuthentic: true,
          explanation:
            'Chính xác: Đại hội VI khởi xướng đường lối Đổi mới toàn diện, trọng tâm là đổi mới tư duy kinh tế.',
        },
        {
          id: 'opt_ch4_1986_wrong_1',
          slotId: 'slot_ch4_doi_moi_1986',
          text: 'Tiếp tục duy trì cơ chế phân phối tem phiếu bao cấp và đóng cửa hoàn toàn giao thương quốc tế.',
          isAuthentic: false,
          explanation:
            'Sai: Cơ chế quan liêu bao cấp chính là lực cản được Đại hội VI kiên quyết xóa bỏ.',
        },
        {
          id: 'opt_ch4_1986_wrong_2',
          slotId: 'slot_ch4_doi_moi_1986',
          text: 'Từ bỏ vai trò quản lý kinh tế vĩ mô của Nhà nước và mục tiêu chủ nghĩa xã hội.',
          isAuthentic: false,
          explanation:
            'Sai: Đổi mới luôn giữ vững định hướng xã hội chủ nghĩa và vai trò quản lý của Nhà nước pháp quyền.',
        },
      ],
    },
    {
      id: 'slot_ch4_platform_2011',
      label: '2. Mô hình phát triển tổng quát trong Cương lĩnh (Bổ sung, phát triển 2011)',
      prompt:
        'Cương lĩnh xây dựng đất nước trong thời kỳ quá độ lên CNXH (1991 và bổ sung, phát triển năm 2011) xác định mô hình kinh tế - xã hội như thế nào?',
      options: [
        {
          id: 'opt_ch4_2011_authentic',
          slotId: 'slot_ch4_platform_2011',
          text: 'Phát triển nền kinh tế thị trường định hướng xã hội chủ nghĩa gắn với tiến bộ, công bằng xã hội và hội nhập quốc tế sâu rộng.',
          isAuthentic: true,
          explanation:
            'Chính xác: Đẩy mạnh công nghiệp hóa, hiện đại hóa gắn với kinh tế tri thức và hội nhập quốc tế.',
        },
        {
          id: 'opt_ch4_2011_wrong_1',
          slotId: 'slot_ch4_platform_2011',
          text: 'Tăng trưởng kinh tế bằng mọi giá mà bỏ qua bảo vệ môi trường và an sinh xã hội.',
          isAuthentic: false,
          explanation:
            'Sai: Đảng xác định phát triển nhanh phải đi đôi với phát triển bền vững và thực hiện tiến bộ, công bằng xã hội ngay trong từng bước phát triển.',
        },
        {
          id: 'opt_ch4_2011_wrong_2',
          slotId: 'slot_ch4_platform_2011',
          text: 'Chỉ phát triển nông nghiệp truyền thống và dừng mọi tiến trình công nghiệp hóa.',
          isAuthentic: false,
          explanation:
            'Sai: Nhiệm vụ trung tâm là đẩy mạnh công nghiệp hóa, hiện đại hóa đất nước.',
        },
      ],
    },
    {
      id: 'slot_ch4_vision_2045',
      label: '3. Mục tiêu phát triển đến năm 2045 theo Nghị quyết Đại hội XIII (2021)',
      prompt:
        'Nghị quyết Đại hội XIII (01/2021) xác định mục tiêu phát triển đất nước đến năm 2045 và cách phân định nguồn gốc tư liệu 2045 trong hồ sơ là gì?',
      options: [
        {
          id: 'opt_ch4_2045_authentic',
          slotId: 'slot_ch4_vision_2045',
          text: 'Phấn đấu đến năm 2045 (kỷ niệm 100 năm thành lập nước) trở thành nước phát triển, thu nhập cao — bối cảnh 2045 trong game là viễn cảnh nghệ thuật dựa trên mục tiêu văn kiện.',
          isAuthentic: true,
          explanation:
            'Chính xác: Mục tiêu 2030 và 2045 là quyết sách lịch sử tại Đại hội XIII; còn hình ảnh đô thị 2045 trong game được gắn nhãn speculative_2045 rõ ràng.',
        },
        {
          id: 'opt_ch4_2045_wrong_1',
          slotId: 'slot_ch4_vision_2045',
          text: 'Xem các chi tiết công nghệ giả tưởng năm 2045 là sự kiện lịch sử đã xảy ra trong quá khứ.',
          isAuthentic: false,
          explanation:
            'Sai: Nguyên tắc Provenance của trò chơi phân biệt nghiêm ngặt giữa văn kiện lịch sử 2021 và viễn cảnh tương lai 2045.',
        },
        {
          id: 'opt_ch4_2045_wrong_2',
          slotId: 'slot_ch4_vision_2045',
          text: 'Đặt mục tiêu đến năm 2045 mới bắt đầu bước vào thời kỳ Đổi mới.',
          isAuthentic: false,
          explanation:
            'Sai: Công cuộc Đổi mới đã được khởi xướng từ Đại hội VI (12/1986).',
        },
      ],
    },
  ],
  progressiveHint:
    'Hãy liên kết Đại hội VI (1986) xóa bỏ cơ chế quan liêu bao cấp -> Cương lĩnh 2011 phát triển kinh tế thị trường định hướng XHCN & hội nhập quốc tế -> Đại hội XIII (2021) đặt mục tiêu đến năm 2045 trở thành nước phát triển, thu nhập cao.',
  textbookCitation:
    'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 241–398',
  provenance: {
    type: 'historical_fact',
    label: 'Văn kiện Đại hội VI – XIII & Tầm nhìn 2045',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 241–398',
    historicalNote:
      'Phân định rõ: mục tiêu 2045 trích từ Nghị quyết Đại hội XIII (2021), còn không gian Quảng trường Kỷ Nguyên Số 2045 là viễn cảnh nghệ thuật (speculative_2045).',
  },
  rewardFlag: 'ch4_policy_linked',
  rewardEvidenceId: 'ev_ch4_doi_moi_resolution',
  rewardMilestone: 'XI',
};

export const ERA_PUZZLES_BY_ID: Record<string, DossierPuzzleDefinition> = {
  [CH1_DOSSIER_PUZZLE.id]: CH1_DOSSIER_PUZZLE,
  [CH2_DISPATCH_NETWORK_PUZZLE.id]: CH2_DISPATCH_NETWORK_PUZZLE,
  [CH3_SUPPLY_LOGISTICS_PUZZLE.id]: CH3_SUPPLY_LOGISTICS_PUZZLE,
  [CH4_CAUSAL_POLICY_PUZZLE.id]: CH4_CAUSAL_POLICY_PUZZLE,
  secret_archive: CH1_DOSSIER_PUZZLE,
  tan_trao_banyan: CH2_DISPATCH_NETWORK_PUZZLE,
  ba_dinh_square: CH2_DISPATCH_NETWORK_PUZZLE,
  pha_din_pass: CH3_SUPPLY_LOGISTICS_PUZZLE,
  spring_1975_corridor: CH3_SUPPLY_LOGISTICS_PUZZLE,
  doi_moi_avenue: CH4_CAUSAL_POLICY_PUZZLE,
  nexus_2045_plaza: CH4_CAUSAL_POLICY_PUZZLE,
};

export function getEraPuzzleForMapOrId(
  idOrMapId: string,
  eraId: EraId
): DossierPuzzleDefinition {
  if (ERA_PUZZLES_BY_ID[idOrMapId]) {
    return ERA_PUZZLES_BY_ID[idOrMapId];
  }
  switch (eraId) {
    case '1945':
      return CH2_DISPATCH_NETWORK_PUZZLE;
    case '1954_1975':
      return CH3_SUPPLY_LOGISTICS_PUZZLE;
    case '1986_2045':
      return CH4_CAUSAL_POLICY_PUZZLE;
    case '1930':
    default:
      return CH1_DOSSIER_PUZZLE;
  }
}
