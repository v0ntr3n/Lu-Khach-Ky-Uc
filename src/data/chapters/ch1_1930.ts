/**
 * @file src/data/chapters/ch1_1930.ts
 * @description Dữ liệu Câu đố Đối chiếu Hồ sơ Lịch sử (Dossier Reconstruction Puzzle)
 * và cấu hình Tổng kết Chương I: Hương Cảng 1930.
 */

import type { DossierPuzzleDefinition } from '../../types/game';
import { CH1_PROVENANCE_MANIFEST } from '../provenance/ch1_provenance';

export const CH1_DOSSIER_PUZZLE: DossierPuzzleDefinition = {
  id: 'puzzle_ch1_unification_dossier',
  eraId: '1930',
  zoneId: 'secret_archive',
  title: 'ĐỐI CHIẾU VĂN KIỆN HỘI NGHỊ HỢP NHẤT (02/1930)',
  subtitle:
    'Xác thực 3 luận điểm cốt lõi trong Chánh cương vắn tắt & Sách lược vắn tắt do Nguyễn Ái Quốc soạn thảo',
  instruction:
    'Hãy chọn đúng nhận định lịch sử xác thực cho từng mục dưới đây dựa theo Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021, tr. 61–65):',
  slots: [
    {
      id: 'slot_strategic_goal',
      label: '1. Phương hướng chiến lược của cách mạng Việt Nam',
      prompt:
        'Chánh cương vắn tắt của Đảng (02/1930) xác định phương hướng chiến lược của cách mạng Việt Nam như thế nào?',
      options: [
        {
          id: 'opt_goal_authentic',
          slotId: 'slot_strategic_goal',
          text: 'Làm tư sản dân quyền cách mạng và thổ địa cách mạng để đi tới xã hội cộng sản.',
          isAuthentic: true,
          explanation:
            'Chính xác (Giáo trình 2021, tr. 61): Đường lối giương cao ngọn cờ độc lập dân tộc gắn liền với chủ nghĩa xã hội.',
        },
        {
          id: 'opt_goal_wrong_1',
          slotId: 'slot_strategic_goal',
          text: 'Chỉ tiến hành cải lương kinh tế và xin mở rộng quyền tự trị trong khuôn khổ Đông Dương.',
          isAuthentic: false,
          explanation:
            'Sai: Đây là khuynh hướng cải lương tư sản, không phải đường lối cách mạng vô sản trong Cương lĩnh 02/1930.',
        },
        {
          id: 'opt_goal_wrong_2',
          slotId: 'slot_strategic_goal',
          text: 'Hoãn cách mạng giải phóng dân tộc cho tới khi hoàn thành công nghiệp hóa.',
          isAuthentic: false,
          explanation:
            'Sai: Cương lĩnh đặt nhiệm vụ giải phóng dân tộc lên vị trí hàng đầu.',
        },
      ],
    },
    {
      id: 'slot_primary_task',
      label: '2. Nhiệm vụ hàng đầu về chính trị',
      prompt:
        'Nhiệm vụ chống đế quốc và chống phong kiến được Cương lĩnh đầu tiên xác định ra sao?',
      options: [
        {
          id: 'opt_task_authentic',
          slotId: 'slot_primary_task',
          text: 'Đánh đổ đế quốc chủ nghĩa Pháp và bọn phong kiến, làm cho nước Nam được hoàn toàn độc lập.',
          isAuthentic: true,
          explanation:
            'Chính xác (Giáo trình 2021, tr. 62): Chống đế quốc giành độc lập dân tộc được đặt ở vị trí ưu tiên hàng đầu.',
        },
        {
          id: 'opt_task_wrong_1',
          slotId: 'slot_primary_task',
          text: 'Thỏa hiệp với chính quyền thực dân để đổi lấy tự do buôn bán tại các thương cảng.',
          isAuthentic: false,
          explanation:
            'Sai: Cương lĩnh kiên quyết chủ trương đánh đổ ách thống trị của đế quốc Pháp.',
        },
        {
          id: 'opt_task_wrong_2',
          slotId: 'slot_primary_task',
          text: 'Tập trung đấu tranh nghị trường tại Pháp trước khi tổ chức lực lượng trong nước.',
          isAuthentic: false,
          explanation:
            'Sai: Cách mạng Việt Nam dựa vào thực lực của quần chúng công nông và nhân dân trong nước.',
        },
      ],
    },
    {
      id: 'slot_leadership_force',
      label: '3. Lực lượng cách mạng và vai trò lãnh đạo',
      prompt:
        'Cương lĩnh xác định vai trò lãnh đạo của Đảng và khối đoàn kết lực lượng cách mạng như thế nào?',
      options: [
        {
          id: 'opt_force_authentic',
          slotId: 'slot_leadership_force',
          text: 'Đảng là đội tiên phong của vô sản giai cấp, phải thu phục đại bộ phận dân cày và đoàn kết các lực lượng yêu nước.',
          isAuthentic: true,
          explanation:
            'Chính xác (Giáo trình 2021, tr. 62–64): Công nông là gốc cách mệnh, đồng thời liên lạc với tiểu tư sản, trí thức, trung nông.',
        },
        {
          id: 'opt_force_wrong_1',
          slotId: 'slot_leadership_force',
          text: 'Chỉ dựa vào một nhóm trí thức nhỏ ở hải ngoại, tách rời công nhân và nông dân.',
          isAuthentic: false,
          explanation:
            'Sai: Đảng lấy công nhân và nông dân làm lực lượng cơ bản, rộng rãi.',
        },
        {
          id: 'opt_force_wrong_2',
          slotId: 'slot_leadership_force',
          text: 'Giao quyền lãnh đạo cách mạng cho tầng lớp địa chủ và tư sản mại bản.',
          isAuthentic: false,
          explanation:
            'Sai: Quyền lãnh đạo thuộc về Đảng Cộng sản Việt Nam — đội tiên phong của giai cấp vô sản.',
        },
      ],
    },
  ],
  progressiveHint:
    'Gợi ý Sử Học (Giáo trình 2021, tr. 61–65): Cương lĩnh chính trị đầu tiên xác định rõ đường lối "tư sản dân quyền cách mạng và thổ địa cách mạng để đi tới xã hội cộng sản", đặt nhiệm vụ chống đế quốc giành độc lập dân tộc lên hàng đầu, và khẳng định Đảng là đội tiên phong của giai cấp vô sản gắn bó mật thiết với công nông.',
  textbookCitation:
    'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b, tr. 61–65',
  provenance: CH1_PROVENANCE_MANIFEST.milestone_III_first_platform,
  rewardFlag: 'ch1_dossier_verified',
  rewardEvidenceId: 'ev_ch1_unification_appeal',
  rewardMilestone: 'III',
};
