/**
 * @file src/data/provenance/ch1_provenance.ts
 * @description Danh mục nguồn gốc sử liệu (Provenance Manifest) cho Chương I: Hương Cảng 1930.
 * Phân định rõ ràng giữa Sự kiện Lịch sử xác thực (historical_fact) có trích dẫn trang từ
 * Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021) và Nhân vật/Chi tiết dẫn truyện hư cấu (fictional_npc).
 */

import type { ProvenanceMetadata } from '../../types/game';

export const CH1_PROVENANCE_MANIFEST: Record<string, ProvenanceMetadata> = {
  milestone_I_thanh_nien: {
    type: 'historical_fact',
    label: 'Sự kiện Lịch sử',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2, tr. 44–56',
    historicalNote:
      'Nguyễn Ái Quốc sáng lập Hội Việt Nam Cách mạng Thanh niên (6/1925), xuất bản báo Thanh Niên (21/6/1925) và tác phẩm Đường Kách Mệnh (1927).',
  },
  milestone_II_unification_conference: {
    type: 'historical_fact',
    label: 'Sự kiện Lịch sử',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–61',
    historicalNote:
      'Hội nghị hợp nhất các tổ chức cộng sản diễn ra từ ngày 06/01 đến ngày 07/02/1930 tại bán đảo Cửu Long (Hương Cảng) dưới sự chủ trì của Nguyễn Ái Quốc.',
  },
  milestone_III_first_platform: {
    type: 'historical_fact',
    label: 'Văn kiện Lịch sử Gốc',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b & II.1, tr. 61–76',
    historicalNote:
      'Chánh cương vắn tắt, Sách lược vắn tắt do Nguyễn Ái Quốc soạn thảo (02/1930), Luận cương chính trị (10/1930) và cao trào Xô viết Nghệ - Tĩnh (1930–1931).',
  },
  fictional_liaison_network: {
    type: 'fictional_npc',
    label: 'Nhân vật Giao Liên & Quần chúng Hư cấu (Fictional Historical Composite)',
    historicalNote:
      'Các nhân vật Lâm Văn Bính, Trần Thị Liên, Bác Công Nhân, Lão Chèo Đò được sáng tạo để đại diện cho mạng lưới giao liên bí mật và quần chúng lao động năm 1930.',
  },
  speculative_2045_frame: {
    type: 'speculative_2045',
    label: 'Khung Dẫn truyện Lưu trữ Số 2045 (Speculative 2045 Framing)',
    citation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 385–393',
    historicalNote:
      'Nhân vật Lữ Khách Ký Ức thuộc thế hệ kỷ niệm 100 năm lập quốc (2045) theo tầm nhìn Đại hội XIII.',
  },
};
