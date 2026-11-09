/**
 * @file src/systems/EvidenceSystem.ts
 * @description Hệ thống quản lý Chứng cứ & Tư liệu Lịch sử (Evidence System) cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Phân biệt rõ ràng giữa Sự kiện Lịch sử có trích dẫn trang từ Giáo trình Lịch sử Đảng CSVN (2021),
 * Bối cảnh nhân vật hư cấu (fictional_npc) và Viễn cảnh 2045 (speculative_2045).
 */

import type { EraId, EvidenceItem } from '../types/game';

export const EVIDENCE_CATALOG: readonly EvidenceItem[] = [
  {
    id: 'ev_ch1_shipping_ledger',
    eraId: '1930',
    title: 'Sổ Vận Đơn Cửu Long & Báo Thanh Niên (1925–1929)',
    category: 'document',
    zoneId: 'warehouse_interior',
    shortSummary:
      'Sổ ghi nhận các chuyến tàu bí mật chuyển báo Thanh Niên và tác phẩm Đường Kách Mệnh từ Quảng Châu qua Hương Cảng về trong nước.',
    historicalQuote:
      '“Báo Thanh Niên đánh dấu sự ra đời của báo chí cách mạng Việt Nam, truyền bá chủ nghĩa Mác - Lênin vào phong trào công nhân và phong trào yêu nước.”',
    historicalSignificance:
      'Chuẩn bị đầy đủ về chính trị, tư tưởng và tổ chức cho sự ra đời của Đảng Cộng sản Việt Nam đầu năm 1930.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2, tr. 44–56',
    provenance: {
      type: 'historical_fact',
      label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 44–56)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2, tr. 44–56',
    },
    relatedMilestone: 'I',
    grantsFlag: 'ch1_inspected_ledger',
  },
  {
    id: 'ev_ch1_street_leaflet',
    eraId: '1930',
    title: 'Truyền Đơn Công Nhân Cảng Hương Cảng (1929)',
    category: 'document',
    zoneId: 'old_street',
    shortSummary:
      'Tờ truyền đơn in thạch bản phản ánh làn sóng đấu tranh của công nhân và yêu cầu cấp bách phải thống nhất ba tổ chức cộng sản cuối năm 1929.',
    historicalQuote:
      '“Sự tồn tại của ba tổ chức cộng sản hoạt động riêng rẽ có nguy cơ dẫn đến chia rẽ lớn; yêu cầu bức thiết của cách mạng là phải có một Đảng thống nhất.”',
    historicalSignificance:
      'Khẳng định tính tất yếu khách quan của việc triệu tập Hội nghị hợp nhất các tổ chức cộng sản.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.c, tr. 50–56',
    provenance: {
      type: 'historical_fact',
      label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 50–56)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.2.c, tr. 50–56',
    },
    grantsFlag: 'ch1_found_leaflet',
  },
  {
    id: 'ev_ch1_cipher_note',
    eraId: '1930',
    title: 'Mật Tín Triệu Tập Hội Nghị Hợp Nhất (06/01 – 07/02/1930)',
    category: 'cipher',
    zoneId: 'safehouse_interior',
    shortSummary:
      'Bản mật tín của phái viên Quốc tế Cộng sản Nguyễn Ái Quốc triệu tập đại biểu Đông Dương Cộng sản Đảng và An Nam Cộng sản Đảng tại bán đảo Cửu Long.',
    historicalQuote:
      '“Hội nghị thảo luận và nhất trí bỏ mọi thành kiến xung đột cũ, thành thật hợp tác để thống nhất các nhóm cộng sản ở Đông Dương, đặt tên Đảng là Đảng Cộng sản Việt Nam.”',
    historicalSignificance:
      'Chấm dứt tình trạng khủng hoảng về đường lối và tổ chức lãnh đạo cách mạng Việt Nam kéo dài nhiều thập kỷ.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–61',
    provenance: {
      type: 'historical_fact',
      label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 57–61)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.a, tr. 57–61',
    },
    relatedMilestone: 'II',
    grantsFlag: 'ch1_safehouse_cipher',
  },
  {
    id: 'ev_ch1_patrol_schedule',
    eraId: '1930',
    title: 'Lịch Đổi Ca Tuần Tra Bến Thuyền Cửu Long',
    category: 'dispatch',
    zoneId: 'small_pier',
    shortSummary:
      'Tờ ghi chú giờ đổi ca đèn tuần tra tại Hẻm Tối do Lão Chèo Đò ghi lại giúp giao liên chọn ngõ tắt an toàn sang Sân Kho Lưu Trữ.',
    historicalQuote:
      '“Khi hồi còi tàu đêm vang lên ở bến cảng, lính tuần tra ở Hẻm Tối sẽ đổi gác trong ba phút — đó là lúc đường ven bến an toàn nhất.”',
    historicalSignificance:
      'Chi tiết dẫn truyện hư cấu tôn vinh sự đùm bọc, che chở của quần chúng lao động đối với các chiến sĩ giao liên năm 1930.',
    textbookCitation:
      'Bối cảnh hư cấu lịch sử (Fictional Narrative Prop) hỗ trợ lối đi chiến thuật Chương I',
    provenance: {
      type: 'fictional_npc',
      label: 'Vật phẩm Dẫn truyện Hư cấu (Fictional Gameplay Prop)',
      historicalNote:
        'Chi tiết lịch tuần tra và nhân vật Lão Chèo Đò được sáng tạo để phục vụ cơ chế tìm đường an toàn (Alternative Route).',
    },
    grantsFlag: 'ch1_pier_schedule',
  },
  {
    id: 'ev_ch1_unification_appeal',
    eraId: '1930',
    title: 'Chánh Cương Vắn Tắt, Sách Lược Vắn Tắt & Luận Cương (1930)',
    category: 'artifact',
    zoneId: 'secret_archive',
    shortSummary:
      'Tập văn kiện Cương lĩnh chính trị đầu tiên của Đảng (02/1930) cùng Luận cương chính trị (10/1930) và tư liệu cao trào Xô viết Nghệ - Tĩnh (1930–1931).',
    historicalQuote:
      '“Chủ trương làm tư sản dân quyền cách mạng và thổ địa cách mạng để đi tới xã hội cộng sản; đánh đổ đế quốc chủ nghĩa Pháp và bọn phong kiến, làm cho nước Nam được hoàn toàn độc lập.”',
    historicalSignificance:
      'Xác lập đường lối chiến lược đúng đắn của cách mạng Việt Nam và mở đầu cao trào cách mạng 1930–1931.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b & II.1, tr. 61–76',
    provenance: {
      type: 'historical_fact',
      label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 61–76)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục I.3.b & II.1, tr. 61–76',
    },
    relatedMilestone: 'III',
    grantsFlag: 'ch1_read_appeal',
  },
  {
    id: 'ev_ch2_tan_trao_order',
    eraId: '1945',
    title: 'Quân Lệnh Số 1 & Thư Kêu Gọi Tổng Khởi Nghĩa (08/1945)',
    category: 'document',
    zoneId: 'tan_trao_banyan',
    shortSummary:
      'Bản Quân lệnh số 1 của Ủy ban Khởi nghĩa toàn quốc phát đi từ Tân Trào và Lời kêu gọi Tổng khởi nghĩa của Hồ Chí Minh.',
    historicalQuote:
      '“Giờ quyết định cho vận mệnh dân tộc ta đã đến. Toàn quốc đồng bào hãy đứng dậy đem sức ta mà tự giải phóng cho ta!”',
    historicalSignificance:
      'Phát lệnh Tổng khởi nghĩa giành chính quyền trên phạm vi cả nước trong Cách mạng Tháng Tám 1945.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.b, tr. 108–118',
    provenance: {
      type: 'historical_fact',
      label: 'Văn kiện Lịch sử Gốc (Giáo trình 2021, tr. 108–118)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương I, mục II.3.b, tr. 108–118',
    },
    relatedMilestone: 'V',
    grantsFlag: 'ch2_tan_trao_order_collected',
  },
  {
    id: 'ev_ch3_pha_din_dispatch',
    eraId: '1954_1975',
    title: 'Công Điện Chiến Dịch Điện Biên Phủ & Đại Thắng Mùa Xuân (1954–1975)',
    category: 'dispatch',
    zoneId: 'pha_din_pass',
    shortSummary:
      'Tập lưu trữ quyết định phương châm “Đánh chắc, tiến chắc” tại Điện Biên Phủ (1954) và mệnh lệnh “Thần tốc, táo bạo” trong Chiến dịch Hồ Chí Minh (1975).',
    historicalQuote:
      '“Thần tốc, thần tốc hơn nữa; táo bạo, táo bạo hơn nữa; tranh thủ từng giờ, từng phút, xốc tới mặt trận, giải phóng miền Nam. Quyết chiến và toàn thắng!”',
    historicalSignificance:
      'Hoàn thành sự nghiệp giải phóng dân tộc, thống nhất Tổ quốc vào mùa Xuân năm 1975.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, tr. 176–235',
    provenance: {
      type: 'historical_fact',
      label: 'Tư liệu Lịch sử (Giáo trình 2021, tr. 176–235)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương II, tr. 176–235',
    },
    relatedMilestone: 'VIII',
    grantsFlag: 'ch3_dispatch_collected',
  },
  {
    id: 'ev_ch4_doi_moi_resolution',
    eraId: '1986_2045',
    title: 'Văn Kiện Đại Hội VI (1986) & Tầm Nhìn Đại Hội XIII (2030–2045)',
    category: 'artifact',
    zoneId: 'doi_moi_avenue',
    shortSummary:
      'Hồ sơ đối chiếu đường lối Đổi mới toàn diện tại Đại hội VI (12/1986) với mục tiêu phát triển đất nước đến năm 2030 và tầm nhìn 2045 tại Đại hội XIII (2021).',
    historicalQuote:
      '“Nhìn thẳng vào sự thật, đánh giá đúng sự thật, nói rõ sự thật; đổi mới toàn diện và đồng bộ, trọng tâm là đổi mới tư duy kinh tế.”',
    historicalSignificance:
      'Mở ra thời kỳ Đổi mới, hội nhập quốc tế và xác lập khát vọng phát triển đất nước phồn vinh, hạnh phúc đến năm 2045.',
    textbookCitation:
      'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 261–393',
    provenance: {
      type: 'historical_fact',
      label: 'Văn kiện Lịch sử & Tầm nhìn 2045 (Giáo trình 2021, tr. 261–393)',
      citation:
        'Giáo trình Lịch sử Đảng Cộng sản Việt Nam (2021), Chương III, tr. 261–393',
      historicalNote:
        'Mục tiêu 2045 dựa trên Nghị quyết Đại hội XIII (2021, tr. 385–393); bối cảnh Trung tâm Lưu trữ Số 2045 là khung dẫn truyện giả tưởng (speculative_2045).',
    },
    relatedMilestone: 'XI',
    grantsFlag: 'ch4_resolution_collected',
  },
];

export class EvidenceSystem {
  public getAllEvidence(): readonly EvidenceItem[] {
    return EVIDENCE_CATALOG;
  }

  public getEvidenceById(id: string): EvidenceItem | undefined {
    return EVIDENCE_CATALOG.find((item) => item.id === id);
  }

  public getEvidenceForEra(eraId: EraId): EvidenceItem[] {
    return EVIDENCE_CATALOG.filter((item) => item.eraId === eraId);
  }

  public getCollectedEvidence(
    collectedIds: ReadonlySet<string>,
    isPresenterMode = false
  ): EvidenceItem[] {
    if (isPresenterMode) {
      return [...EVIDENCE_CATALOG];
    }
    return EVIDENCE_CATALOG.filter((item) => collectedIds.has(item.id));
  }
}
