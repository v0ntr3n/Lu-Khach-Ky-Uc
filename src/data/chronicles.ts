/**
 * @file src/data/chronicles.ts
 * @description Toàn bộ dữ liệu lịch sử 4 Chương, 12 Cột mốc La Mã (I - XII),
 * cơ chế tương tác Mức 1, Climax A+B và Trích dẫn Giáo trình Lịch sử Đảng CSVN (2021).
 */

import type { ChapterData, LookupEntry } from '../types/game';

export const PALETTE_12 = {
  LACQUER_VOID: '#110E0B',
  LACQUER_DEEP: '#1B1612',
  LACQUER_ELEVATED: '#241D17',
  INK_PRIMARY: '#232116',
  INK_SECONDARY: '#3E3A2C',
  BRONZE_MUTED: '#6E4720',
  BRONZE_ANTIQUE: '#8A5A2B',
  BRONZE_BRIGHT: '#D4A55D',
  CRIMSON_SEAL: '#9E3321',
  CRIMSON_VERMILION: '#DF6B57',
  PARCHMENT_WARM: '#E6E5DC',
  PARCHMENT_LIGHT: '#F4EFE4',
} as const;

/**
 * Hệ màu Môi trường mở rộng 20 sắc độ (Đối trọng Nóng – Lạnh Sơn Mài, Chàm Đêm, Xanh Rêu Cổ & Xám Đá Ướt)
 * giúp nhân vật chính (tông Sơn Mài – Đồng ấm) tách biệt rõ khỏi mặt nước đêm, màn mưa, tán cây và mặt đường đá.
 */
export const PALETTE_ENV = {
  INDIGO_NIGHT_DEEP: '#16202A',
  INDIGO_WATER_MID: '#243646',
  RAIN_SLATE_MIST: '#4F677A',
  MOSS_SHADOW: '#2D3B2F',
  MOSS_FOLIAGE: '#4A5D46',
  MOSS_CANOPY_HIGHLIGHT: '#687D5B',
  WET_STONE_DARK: '#272A2C',
  WET_STONE_GRAY: '#353839',
} as const;

export const PALETTE_20 = {
  ...PALETTE_12,
  ...PALETTE_ENV,
} as const;

export const CHAPTERS: ChapterData[] = [
  {
    id: '1930',
    chapterNum: 'CHƯƠNG I',
    years: '1930 – 1931',
    title: 'Đêm Mưa Cửu Long & Ngọn Đuốc Cương Lĩnh',
    subtitle: 'Hội nghị Hợp nhất thành lập Đảng (2/1930) & Cao trào Xô Viết Nghệ Tĩnh',
    locationLabel: 'Cửu Long (Hương Cảng) → Nghệ Tĩnh',
    coordinates: '22°19\'00"N · 114°10\'00"E',
    worldWidth: 1080,
    mechanicHint:
      'ĐI CẢNH MỨC 1: Di chuyển bằng [A/D] hoặc [←/→]. Đứng dưới Mái Hiên Bóng Tối hoặc giữ [S/↓] khi chùm đèn tuần tra quét qua. Bấm [E] thu thập 3 Mộc Bản (I, II, III).',
    shelters: [
      { xStart: 210, xEnd: 295, label: 'Mái hiên phố Cửu Long' },
      { xStart: 510, xEnd: 600, label: 'Hẻm tối bến cảng' },
      { xStart: 780, xEnd: 865, label: 'Căn nhà cơ sở bí mật' },
    ],
    hazards: [
      {
        id: 'patrol_1930_a',
        xCenter: 380,
        radius: 58,
        speed: 1.5,
        phaseOffset: 0,
        label: 'Đèn rọi tuần tra mật thám',
        warningToast: 'Ánh đèn tuần tra phát hiện! Lữ Khách lùi nhẹ về mái hiên ẩn nấp gần nhất.',
      },
      {
        id: 'patrol_1930_b',
        xCenter: 685,
        radius: 62,
        speed: 1.8,
        phaseOffset: 2.1,
        label: 'Đèn rọi bến cảng Hương Cảng',
        warningToast: 'Chùm sáng mật thám quét qua! Hãy chờ nhịp đèn đổi hướng hoặc nép dưới mái hiên.',
      },
    ],
    milestones: [
      {
        roman: 'I',
        eraId: '1930',
        year: '1920 – 1929',
        title: 'Chuẩn Bị Về Chính Trị, Tư Tưởng & Tổ Chức',
        worldX: 195,
        spriteType: 'lantern',
        npcSpeaker: 'Giao Liên Cơ Sở Hương Cảng',
        npcRole: 'Hội Việt Nam Cách mạng Thanh niên',
        dialogue:
          'Từ Đại hội Tua (12/1920) đến tác phẩm "Đường Kách mệnh" (1927), Nguyễn Ái Quốc đã truyền bá chủ nghĩa Mác - Lênin vào phong trào công nhân và phong trào yêu nước Việt Nam.',
        historicalSignificance:
          'Sự ra đời của 3 tổ chức cộng sản cuối năm 1929 đòi hỏi bức thiết phải thống nhất thành một chính đảng duy nhất.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 18 – 35',
      },
      {
        roman: 'II',
        eraId: '1930',
        year: '02 / 1930',
        title: 'Hội Nghị Thành Lập Đảng & Cương Lĩnh Đầu Tiên',
        worldX: 485,
        spriteType: 'scroll',
        npcSpeaker: 'Ký Lục Viên Hội Nghị Hợp Nhất',
        npcRole: 'Cửu Long, Hương Cảng (06/01 – 07/02/1930)',
        dialogue:
          'Dưới sự chủ trì của Nguyễn Ái Quốc, Hội nghị đã hợp nhất các tổ chức cộng sản, lấy tên là Đảng Cộng sản Việt Nam và thông qua Chánh cương vắn tắt, Sách lược vắn tắt.',
        historicalSignificance:
          'Chấm dứt khủng hoảng về đường lối cứu nước và giai cấp lãnh đạo kéo dài suốt nhiều thập kỷ đầu thế kỷ XX.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 36 – 43',
      },
      {
        roman: 'III',
        eraId: '1930',
        year: '1930 – 1931',
        title: 'Luận Cương Tháng 10/1930 & Xô Viết Nghệ Tĩnh',
        worldX: 765,
        spriteType: 'seal',
        npcSpeaker: 'Chiến Sĩ Tự Vệ Đỏ Nghệ Tĩnh',
        npcRole: 'Cao trào Cách mạng 1930 – 1931',
        dialogue:
          'Hội nghị Trung ương tháng 10/1930 do Trần Phú chủ trì thông qua Luận cương chính trị. Cao trào Xô Viết Nghệ Tĩnh đã khẳng định trên thực tế quyền lãnh đạo và sức mạnh của khối liên minh công - nông.',
        historicalSignificance:
          'Cuộc tổng diễn tập đầu tiên chuẩn bị cho thắng lợi của Cách mạng Tháng Tám năm 1945.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 44 – 56',
      },
    ],
    climax: {
      gateWorldX: 980,
      stepATitle: 'BƯỚC A · KHÔI PHỤC QUYẾT SÁCH LỊCH SỬ (02/1930)',
      stepAPrompt:
        'Để mở Cổng Thời Gian rời khỏi đêm mưa Cửu Long năm 1930, Lữ Khách hãy xác định nhiệm vụ chiến lược hàng đầu được Cương lĩnh chính trị đầu tiên của Đảng (2/1930) đặt lên vị trí tiên phong:',
      stepACitation: 'Đối chiếu: Giáo trình Lịch sử Đảng CSVN (2021), tr. 38 – 42',
      options: [
        {
          id: 'opt_1930_1',
          text: 'Đặt nhiệm vụ chống đế quốc, giành độc lập dân tộc lên hàng đầu ("Làm tư sản dân quyền cách mạng và thổ địa cách mạng để đi tới xã hội cộng sản").',
          isCorrect: true,
          feedback:
            'Chính xác! Cương lĩnh chính trị đầu tiên (2/1930) giải quyết đúng đắn mối quan hệ giữa vấn đề dân tộc và vấn đề giai cấp, đặt nhiệm vụ giải phóng dân tộc lên hàng đầu.',
        },
        {
          id: 'opt_1930_2',
          text: 'Chỉ tập trung đấu tranh nghị trường hợp pháp và cải cách thuế khóa.',
          isCorrect: false,
          feedback:
            'Chưa chính xác. Cương lĩnh xác định con đường bạo lực cách mạng của quần chúng để đánh đổ đế quốc và phong kiến.',
        },
        {
          id: 'opt_1930_3',
          text: 'Chờ cách mạng vô sản ở chính quốc thắng lợi hoàn toàn mới tiến hành vận động trong nước.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Nguyễn Ái Quốc khẳng định cách mạng thuộc địa có tính chủ động, có thể giành thắng lợi trước cách mạng ở chính quốc.',
        },
      ],
      stepBTitle: 'BƯỚC B · ĐẠI CẢNH BÙNG NỔ: THẮP ĐUỐC CƯƠNG LĨNH 1930',
      stepBSubtitle:
        'Bấm liên tiếp [SPACE / E] hoặc nút bên dưới để thổi bùng Ngọn Đuốc Cương Lĩnh Đầu Tiên và mở Cổng Thời Gian tiến tới năm 1945!',
      stepBActionLabel: '🔥 Thắp Sáng Ngọn Đuốc 1930',
      stepBCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), tr. 42 – 43: Ý nghĩa lịch sử của việc thành lập Đảng.',
      setPieceType: 'kowloon_torch',
    },
  },
  {
    id: '1945',
    chapterNum: 'CHƯƠNG II',
    years: '1939 – 1945',
    title: 'Tân Trào Chớp Thời Cơ & Mùa Thu Ba Đình',
    subtitle: 'Chuyển hướng chỉ đạo chiến lược (1939–1941) & Tổng khởi nghĩa Tháng Tám 1945',
    locationLabel: 'Pác Bó – Tân Trào → Quảng Trường Ba Đình',
    coordinates: '21°01\'42"N · 105°51\'15"E',
    worldWidth: 1080,
    mechanicHint:
      'ĐI CẢNH MỨC 1: Băng qua rừng Tân Trào, nối mạch đường dây liên lạc qua 3 mốc (IV, V, VI) trước khi Cửa Sổ Thời Cơ khép lại. Nép dưới bóng cây đa khi có tuần tra.',
    shelters: [
      { xStart: 220, xEnd: 310, label: 'Bóng đa Tân Trào' },
      { xStart: 500, xEnd: 590, label: 'Trạm liên lạc chiến khu' },
      { xStart: 770, xEnd: 860, label: 'Cửa ô Hà Nội' },
    ],
    hazards: [
      {
        id: 'patrol_1945_a',
        xCenter: 395,
        radius: 55,
        speed: 1.65,
        phaseOffset: 0.7,
        label: 'Trạm kiểm soát phát xít',
        warningToast: 'Phát hiện trạm gác! Lữ Khách lùi về bóng cây đa Tân Trào để giữ bí mật đường dây.',
      },
      {
        id: 'patrol_1945_b',
        xCenter: 690,
        radius: 60,
        speed: 1.9,
        phaseOffset: 2.5,
        label: 'Đội tuần tra thiết giáp',
        warningToast: 'Đường dây truyền tin bị gián đoạn tạm thời! Hãy lùi về trạm liên lạc gần nhất.',
      },
    ],
    milestones: [
      {
        roman: 'IV',
        eraId: '1945',
        year: '05 / 1941',
        title: 'Hội Nghị Trung Ương 8 & Mặt Trận Việt Minh',
        worldX: 195,
        spriteType: 'scroll',
        npcSpeaker: 'Cán Bộ Mặt Trận Việt Minh',
        npcRole: 'Khuổi Nậm, Pác Bó (Cao Bằng)',
        dialogue:
          'Hội nghị Trung ương 8 (5/1941) do Nguyễn Ái Quốc chủ trì đã hoàn chỉnh chuyển hướng chỉ đạo chiến lược: Đặt nhiệm vụ giải phóng dân tộc lên bức thiết nhất và thành lập Mặt trận Việt Minh.',
        historicalSignificance:
          '"Trong lúc này quyền lợi của bộ phận, của giai cấp phải đặt dưới sự sinh tử, tồn vong của quốc gia, của dân tộc."',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 83 – 93',
      },
      {
        roman: 'V',
        eraId: '1945',
        year: '03 – 08 / 1945',
        title: 'Chỉ Thị "Nhật – Pháp Bắn Nhau" & Tân Trào',
        worldX: 485,
        spriteType: 'radio',
        npcSpeaker: 'Chiến Sĩ Giải Phóng Quân',
        npcRole: 'Đình Tân Trào, Tuyên Quang (8/1945)',
        dialogue:
          'Đêm 9/3/1945 Nhật đảo chính Pháp, Ban Thường vụ Trung ương Đảng ra Chỉ thị "Nhật - Pháp bắn nhau và hành động của chúng ta" (12/3/1945), phát động cao trào kháng Nhật cứu nước.',
        historicalSignificance:
          'Hội nghị toàn quốc của Đảng tại Tân Trào (14–15/8/1945) quyết định phát động Tổng khởi nghĩa giành chính quyền trước khi quân Đồng minh vào Đông Dương.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 101 – 114',
      },
      {
        roman: 'VI',
        eraId: '1945',
        year: '02 / 09 / 1945',
        title: 'Tổng Khởi Nghĩa Tháng Tám & Tuyên Ngôn Độc Lập',
        worldX: 765,
        spriteType: 'flag',
        npcSpeaker: 'Đội Viên Tự Vệ Thủ Đô',
        npcRole: 'Quảng trường Ba Đình (02/09/1945)',
        dialogue:
          'Chỉ trong vòng 15 ngày cuối tháng 8/1945 (Hà Nội 19/8, Huế 23/8, Sài Gòn 25/8), Tổng khởi nghĩa đã thắng lợi trên phạm vi cả nước. Ngày 2/9/1945, Chủ tịch Hồ Chí Minh đọc Tuyên ngôn Độc lập.',
        historicalSignificance:
          'Đập tan xiềng xích nô lệ thực dân - phong kiến, khai sinh nước Việt Nam Dân chủ Cộng hòa — nhà nước công nông đầu tiên ở Đông Nam Á.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 114 – 124',
      },
    ],
    climax: {
      gateWorldX: 980,
      stepATitle: 'BƯỚC A · CHỚP "THỜI CƠ NGÀN NĂM CÓ MỘT" (08/1945)',
      stepAPrompt:
        'Tại Hội nghị toàn quốc ở Tân Trào (14–15/8/1945), Đảng xác định "Cửa sổ thời cơ" để phát động Tổng khởi nghĩa giành chính quyền trên toàn quốc nằm trong khoảng thời gian lịch sử nào?',
      stepACitation: 'Đối chiếu: Giáo trình Lịch sử Đảng CSVN (2021), tr. 109 – 115',
      options: [
        {
          id: 'opt_1945_1',
          text: 'Từ khi phát xít Nhật đầu hàng Đồng minh đến trước khi quân Đồng minh (Anh, Tưởng) kéo vào Đông Dương giải giáp quân Nhật.',
          isCorrect: true,
          feedback:
            'Chính xác! Đây là nghệ thuật chớp thời cơ kiệt xuất của Đảng: giành chính quyền với tư cách người chủ đất nước đón quân Đồng minh.',
        },
        {
          id: 'opt_1945_2',
          text: 'Đợi quân đội Đồng minh tiến vào giải giáp xong quân Nhật rồi mới tổ chức đàm phán trao trả độc lập.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Nếu chờ quân Đồng minh và quân Pháp quay lại, cách mạng sẽ đối mặt nguy cơ bị áp đặt chế độ thống trị mới.',
        },
        {
          id: 'opt_1945_3',
          text: 'Khởi nghĩa ngay từ đầu năm 1944 khi quân Nhật và quân Pháp vẫn còn liên kết chặt chẽ ở Đông Dương.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Tháng 5/1944 Hồ Chí Minh đã hoãn cuộc khởi nghĩa Cao - Bắc - Lạng vì điều kiện toàn quốc chưa chín muồi.',
        },
      ],
      stepBTitle: 'BƯỚC B · ĐẠI CẢNH BÙNG NỔ: KÉO QUỐC KỲ BA ĐÌNH (02/09/1945)',
      stepBSubtitle:
        'Bấm liên tiếp [SPACE / E] hoặc nút bên dưới để cùng biển người Ba Đình kéo lá cờ đỏ sao vàng tung bay trên lễ đài Độc Lập!',
      stepBActionLabel: '🚩 Kéo Quốc Kỳ Trên Lễ Đài Ba Đình',
      stepBCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), tr. 118 – 124: Tuyên ngôn Độc lập & Ý nghĩa Cách mạng Tháng Tám.',
      setPieceType: 'badinh_flag',
    },
  },
  {
    id: '1954_1975',
    chapterNum: 'CHƯƠNG III',
    years: '1945 – 1975',
    title: 'Lừng Lẫy Điện Biên & Đại Thắng Mùa Xuân 1975',
    subtitle: '30 năm chiến tranh cách mạng bảo vệ Tổ quốc & Thống nhất đất nước',
    locationLabel: 'Đèo Pha Đin – Điện Biên Phủ → Dinh Độc Lập',
    coordinates: '21°23\'00"N · 103°01\'00"E → 10°46\'37"N · 106°41\'43"E',
    worldWidth: 1080,
    mechanicHint:
      'ĐI CẢNH MỨC 1: Đẩy xe đạp thồ vượt tuyến lửa qua 3 mốc (VII, VIII, IX). Nép vào công sự giao thông hào khi pháo sáng chiếu rọi trên đèo.',
    shelters: [
      { xStart: 215, xEnd: 305, label: 'Hầm trú ẩn đèo Pha Đin' },
      { xStart: 495, xEnd: 585, label: 'Giao thông hào Điện Biên' },
      { xStart: 765, xEnd: 855, label: 'Trạm giao liên Trường Sơn' },
    ],
    hazards: [
      {
        id: 'patrol_1954_a',
        xCenter: 385,
        radius: 60,
        speed: 1.75,
        phaseOffset: 0.3,
        label: 'Pháo sáng tọa độ đèo Pha Đin',
        warningToast: 'Pháo sáng rọi trúng tuyến vận tải! Lữ Khách lùi về hầm hào gần nhất để bảo toàn đoàn xe thồ.',
      },
      {
        id: 'patrol_1975_b',
        xCenter: 680,
        radius: 64,
        speed: 2.0,
        phaseOffset: 1.9,
        label: 'Vùng hỏa lực ngăn chặn tuyến đường',
        warningToast: 'Tuyến đường bị pháo kích! Hãy chờ ngớt pháo sáng rồi tiến lên.',
      },
    ],
    milestones: [
      {
        roman: 'VII',
        eraId: '1954_1975',
        year: '1946 – 1954',
        title: 'Toàn Quốc Kháng Chiến & Chiến Thắng Điện Biên Phủ',
        worldX: 195,
        spriteType: 'bicycle',
        npcSpeaker: 'Dân Công Hỏa Tuyến Điện Biên',
        npcRole: 'Chiến dịch Điện Biên Phủ (13/3 – 07/5/1954)',
        dialogue:
          'Với đường lối kháng chiến "Toàn dân, toàn diện, trường kỳ, tự lực cánh sinh", quân và dân ta đã làm nên chiến thắng Điện Biên Phủ "lừng lẫy năm châu, chấn động địa cầu" ngày 7/5/1954.',
        historicalSignificance:
          'Buộc thực dân Pháp ký Hiệp định Giơnevơ (7/1954), giải phóng hoàn toàn miền Bắc để đi lên chủ nghĩa xã hội.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 2, tr. 125 – 181',
      },
      {
        roman: 'VIII',
        eraId: '1954_1975',
        year: '1954 – 1968',
        title: 'Chiến Lược Hai Miền, Nghị Quyết 15 & Đại Hội III',
        worldX: 485,
        spriteType: 'scroll',
        npcSpeaker: 'Chiến Sĩ Đường Hồ Chí Minh',
        npcRole: 'Tuyến vận tải chiến lược 559 Trường Sơn',
        dialogue:
          'Đại hội III của Đảng (9/1960) xác định đường lối tiến hành đồng thời hai chiến lược cách mạng ở hai miền: Miền Bắc là hậu phương lớn (vai trò quyết định nhất), miền Nam là tiền tuyến lớn (vai trò quyết định trực tiếp).',
        historicalSignificance:
          'Phong trào Đồng Khởi (1960) và Tổng tiến công và nổi dậy Xuân Mậu Thân 1968 làm phá sản các chiến lược chiến tranh của đế quốc Mỹ.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 2, tr. 182 – 220',
      },
      {
        roman: 'IX',
        eraId: '1954_1975',
        year: '30 / 04 / 1975',
        title: 'Điện Biên Phủ Trên Không (1972) & Đại Thắng Mùa Xuân 1975',
        worldX: 765,
        spriteType: 'tank',
        npcSpeaker: 'Chiến Sĩ Xe Tăng Chiến Dịch Hồ Chí Minh',
        npcRole: 'Cánh quân thần tốc tiến vào Sài Gòn (30/4/1975)',
        dialogue:
          'Sau thắng lợi "Điện Biên Phủ trên không" (12/1972) và Hiệp định Paris (1/1973), cuộc Tổng tiến công và nổi dậy mùa Xuân 1975 đỉnh cao là Chiến dịch Hồ Chí Minh đã toàn thắng lúc 11h30 ngày 30/4/1975.',
        historicalSignificance:
          'Kết thúc 21 năm kháng chiến chống Mỹ, hoàn thành cách mạng dân tộc dân chủ nhân dân trên cả nước, non sông thu về một mối.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 2, tr. 221 – 237',
      },
    ],
    climax: {
      gateWorldX: 980,
      stepATitle: 'BƯỚC A · QUYẾT ĐỊNH LỊCH SỬ TẠI MƯỜNG PHĂNG & MÙA XUÂN 1975',
      stepAPrompt:
        'Trong Chiến dịch Điện Biên Phủ (1954), quyết định thay đổi phương châm tác chiến nào của Đảng ủy và Bộ Chỉ huy chiến dịch đã trở thành nhân tố trực tiếp đưa chiến dịch đến toàn thắng?',
      stepACitation: 'Đối chiếu: Giáo trình Lịch sử Đảng CSVN (2021), tr. 173 – 176 & tr. 228 – 237',
      options: [
        {
          id: 'opt_1954_1',
          text: 'Chuyển từ phương châm "Đánh nhanh, giải quyết nhanh" sang phương châm "Đánh chắc, tiến chắc" (26/01/1954).',
          isCorrect: true,
          feedback:
            'Chính xác! Đại tướng Võ Nguyên Giáp và Đảng ủy mặt trận đã thực hiện lời căn dặn của Chủ tịch Hồ Chí Minh: "Chắc thắng mới đánh, không chắc thắng không đánh".',
        },
        {
          id: 'opt_1954_2',
          text: 'Dồn toàn lực đánh tổng lực ngay trong đêm 25/01/1954 bất chấp hệ thống pháo binh địch đã tăng cường.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Tập đoàn cứ điểm Điện Biên Phủ đã được địch củng cố vững chắc, đánh nhanh sẽ gây tổn thất lớn.',
        },
        {
          id: 'opt_1954_3',
          text: 'Rút toàn bộ quân chủ lực về đồng bằng và hủy bỏ chiến dịch Đông - Xuân 1953–1954.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Đảng quyết tâm tiêu diệt tập đoàn cứ điểm mạnh nhất Đông Dương bằng chiến thuật vây lấn, bóc vỏ từng cụm cứ điểm.',
        },
      ],
      stepBTitle: 'BƯỚC B · ĐẠI CẢNH BÙNG NỔ: THỐNG NHẤT NON SÔNG (30/04/1975)',
      stepBSubtitle:
        'Bấm liên tiếp [SPACE / E] hoặc nút bên dưới để điều khiển đoàn xe tăng thần tốc tiến qua cổng Dinh Độc Lập, nối liền một dải non sông!',
      stepBActionLabel: '🛡️ Tiến Quân Thống Nhất Đất Nước (30/4/1975)',
      stepBCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), tr. 230 – 237: Ý nghĩa lịch sử và kinh nghiệm lãnh đạo kháng chiến.',
      setPieceType: 'dienbien_tank',
    },
  },
  {
    id: '1986_2045',
    chapterNum: 'CHƯƠNG IV',
    years: '1986 – 2045',
    title: 'Đại Lộ Đổi Mới & Kỷ Nguyên Vươn Mình 2045',
    subtitle: 'Công cuộc Đổi mới toàn diện từ Đại hội VI (1986) đến Tầm nhìn 100 năm lập nước (2045)',
    locationLabel: 'Hà Nội 1986 → Siêu Đô Thị Số Việt Nam 2045',
    coordinates: '21°01\'42"N · 105°51\'15"E · NĂM 2045',
    worldWidth: 1080,
    mechanicHint:
      'ĐI CẢNH MỨC 1: Bước dọc đại lộ thời gian từ năm 1986 đến 2045. Mỗi khi thu thập mốc (X, XI, XII), kiến trúc phố phường Pixel phía sau sẽ tự động nâng cấp lên đô thị hiện đại!',
    shelters: [
      { xStart: 220, xEnd: 320, label: 'Hội trường Đại hội VI (1986)' },
      { xStart: 500, xEnd: 600, label: 'Khu công nghệ cao (2011)' },
      { xStart: 760, xEnd: 860, label: 'Trung tâm Dữ liệu Quốc gia (2045)' },
    ],
    hazards: [
      {
        id: 'patrol_1986_a',
        xCenter: 390,
        radius: 52,
        speed: 1.4,
        phaseOffset: 0.4,
        label: 'Xoáy khủng hoảng kinh tế - lạm phát 1980s',
        warningToast: 'Vùng nhiễu lạm phát thời kỳ bao cấp! Lữ Khách lùi về điểm tựa chính sách gần nhất.',
      },
      {
        id: 'patrol_2045_b',
        xCenter: 680,
        radius: 54,
        speed: 1.6,
        phaseOffset: 1.8,
        label: 'Thách thức tụt hậu & biến đổi toàn cầu',
        warningToast: 'Biến động kinh tế toàn cầu! Hãy ẩn vào vùng trụ cột chiến lược để vượt qua.',
      },
    ],
    milestones: [
      {
        roman: 'X',
        eraId: '1986_2045',
        year: '12 / 1986',
        title: 'Đại Hội VI & Khởi Xướng Đường Lối Đổi Mới Toàn Diện',
        worldX: 195,
        spriteType: 'scroll',
        npcSpeaker: 'Kiến Trúc Sư Kinh Tế Đổi Mới',
        npcRole: 'Đại hội đại biểu toàn quốc lần thứ VI (12/1986)',
        dialogue:
          'Với tinh thần "Nhìn thẳng vào sự thật, đánh giá đúng sự thật, nói rõ sự thật", Đại hội VI (12/1986) quyết định xóa bỏ cơ chế tập trung quan liêu bao cấp, phát triển nền kinh tế nhiều thành phần.',
        historicalSignificance:
          'Bước ngoặt lịch sử đưa đất nước thoát khỏi khủng hoảng kinh tế - xã hội và mở ra thời kỳ phát triển mới.',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 3, tr. 256 – 271',
      },
      {
        roman: 'XI',
        eraId: '1986_2045',
        year: '1991 – 2011',
        title: 'Cương Lĩnh 1991, Cương Lĩnh 2011 & Hội Nhập Quốc Tế',
        worldX: 485,
        spriteType: 'seal',
        npcSpeaker: 'Chuyên Gia Hội Nhập & Công Nghiệp Hóa',
        npcRole: 'Đại hội VII (1991) – Đại hội XI (2011)',
        dialogue:
          'Cương lĩnh năm 1991 và Cương lĩnh (bổ sung, phát triển năm 2011) đã định hình mô hình chủ nghĩa xã hội ở Việt Nam với 8 đặc trưng và 8 phương hướng cơ bản, đẩy mạnh CNH-HĐH gắn với kinh tế tri thức.',
        historicalSignificance:
          'Hoàn thiện thể chế kinh tế thị trường định hướng xã hội chủ nghĩa và hội nhập quốc tế sâu rộng (ASEAN, WTO, các FTA thế hệ mới).',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 3, tr. 272 – 358',
      },
      {
        roman: 'XII',
        eraId: '1986_2045',
        year: '2021 – 2045',
        title: 'Đại Hội XIII & Khát Vọng Việt Nam Hùng Cường 2045',
        worldX: 765,
        spriteType: 'metro',
        npcSpeaker: 'Lữ Khách Lưu Trữ Quốc Gia 2045',
        npcRole: 'Kỷ niệm 100 năm thành lập Nước (1945 – 2045)',
        dialogue:
          'Đại hội XIII (2021) xác lập mục tiêu chiến lược 100 năm dưới sự lãnh đạo của Đảng (2030) và 100 năm thành lập nước (2045): Trở thành nước phát triển, thu nhập cao theo định hướng xã hội chủ nghĩa.',
        historicalSignificance:
          '"Đất nước ta chưa bao giờ có được cơ đồ, tiềm lực, vị thế và uy tín quốc tế như ngày nay."',
        textbookCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 3 & Kết luận, tr. 359 – 436',
      },
    ],
    climax: {
      gateWorldX: 980,
      stepATitle: 'BƯỚC A · TỔNG KẾT ĐƯỜNG LỐI ĐỔI MỚI & TẦM NHÌN 2045',
      stepAPrompt:
        'Theo Nghị quyết Đại hội đại biểu toàn quốc lần thứ XIII của Đảng (2021), mục tiêu phát triển tổng quát của Việt Nam vào cột mốc năm 2045 (kỷ niệm 100 năm thành lập nước) là gì?',
      stepACitation: 'Đối chiếu: Giáo trình Lịch sử Đảng CSVN (2021), Chương 3, tr. 368 – 395',
      options: [
        {
          id: 'opt_2045_1',
          text: 'Trở thành nước phát triển, thu nhập cao theo định hướng xã hội chủ nghĩa (vào năm 2030 là nước đang phát triển có công nghiệp hiện đại, thu nhập trung bình cao).',
          isCorrect: true,
          feedback:
            'Chính xác! Đây là 2 cột mốc chiến lược 100 năm (2030 & 2045) được Đại hội XIII của Đảng xác lập rõ.',
        },
        {
          id: 'opt_2045_2',
          text: 'Quay trở lại cơ chế kế hoạch hóa tập trung bao cấp và đóng cửa thị trường nội địa.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Đảng kiên định phát triển nền kinh tế thị trường định hướng XHCN và hội nhập quốc tế chủ động, tích cực.',
        },
        {
          id: 'opt_2045_3',
          text: 'Chỉ phát triển kinh tế đơn thuần mà bỏ qua xây dựng văn hóa, quốc phòng và chỉnh đốn Đảng.',
          isCorrect: false,
          feedback:
            'Tư liệu chưa khớp! Phát triển kinh tế - xã hội là trung tâm; xây dựng Đảng là then chốt; phát triển văn hóa là nền tảng tinh thần; bảo đảm quốc phòng - an ninh là trọng yếu, thường xuyên.',
        },
      ],
      stepBTitle: 'BƯỚC B · ĐẠI CẢNH BÙNG NỔ: BÌNH MINH KỶ NGUYÊN SỐ 2045',
      stepBSubtitle:
        'Bấm liên tiếp [SPACE / E] hoặc nút bên dưới để kích hoạt Đoàn Tàu Cao Tốc 2045 và hoàn tất Biên Niên Sử 1930 – 2045!',
      stepBActionLabel: '🚄 Khởi Chạy Đại Lộ Kỷ Nguyên 2045',
      stepBCitation: 'Giáo trình Lịch sử Đảng CSVN (2021), Kết luận, tr. 396 – 436: Những thắng lợi vĩ đại và bài học kinh nghiệm.',
      setPieceType: 'doimoi_metro',
    },
  },
];

export const AI_LOOKUP_DATABASE: LookupEntry[] = [
  {
    keywords: ['cương lĩnh', '1930', 'hội nghị thành lập', 'chánh cương', 'hương cảng', 'cửu long'],
    question: 'Nội dung cốt lõi của Cương lĩnh chính trị đầu tiên của Đảng (2/1930) là gì?',
    answer:
      'Cương lĩnh chính trị đầu tiên (Chánh cương vắn tắt, Sách lược vắn tắt do Nguyễn Ái Quốc soạn thảo, thông qua tại Hội nghị thành lập Đảng tháng 2/1930) xác định mục tiêu chiến lược: "Làm tư sản dân quyền cách mạng và thổ địa cách mạng để đi tới xã hội cộng sản", đặt nhiệm vụ chống đế quốc giành độc lập dân tộc lên hàng đầu.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 36 – 43',
    relatedMilestone: 'II',
  },
  {
    keywords: ['luận cương', 'trần phú', '10/1930', 'xô viết nghệ tĩnh'],
    question: 'Điểm khác biệt và bổ sung giữa Cương lĩnh tháng 2/1930 và Luận cương tháng 10/1930?',
    answer:
      'Cả hai văn kiện đều xác định phương hướng chiến lược cách mạng tư sản dân quyền tiến lên XHCN. Tuy nhiên, Cương lĩnh tháng 2/1930 đặt nhiệm vụ giải phóng dân tộc lên hàng đầu và đoàn kết rộng rãi các tầng lớp yêu nước; trong khi Luận cương tháng 10/1930 nhấn mạnh hơn đấu tranh giai cấp và cách mạng ruộng đất.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 44 – 52',
    relatedMilestone: 'III',
  },
  {
    keywords: ['trung ương 8', '1941', 'pác bó', 'việt minh', 'chuyển hướng'],
    question: 'Vì sao Hội nghị Trung ương 8 (5/1941) có ý nghĩa quyết định đối với Cách mạng Tháng Tám?',
    answer:
      'Hội nghị Trung ương 8 (5/1941) tại Pác Bó do Nguyễn Ái Quốc chủ trì đã hoàn chỉnh chủ trương chuyển hướng chỉ đạo chiến lược: Dương cao ngọn cờ giải phóng dân tộc, tạm gác khẩu hiệu cách mạng ruộng đất, thành lập Mặt trận Việt Minh và chuẩn bị khởi nghĩa vũ trang từ khởi nghĩa từng phần tiến lên tổng khởi nghĩa.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 83 – 93',
    relatedMilestone: 'IV',
  },
  {
    keywords: ['tháng tám', '1945', 'tân trào', 'nhật pháp', 'thời cơ', 'ba đình'],
    question: 'Nghệ thuật chớp thời cơ trong Tổng khởi nghĩa Tháng Tám 1945 thể hiện như thế nào?',
    answer:
      'Ngay khi phát xít Nhật đầu hàng Đồng minh (8/1945) và trước khi quân Đồng minh vào Đông Dương, Đảng đã họp Hội nghị toàn quốc ở Tân Trào (14–15/8/1945), phát động toàn dân tổng khởi nghĩa giành chính quyền chỉ trong 15 ngày và khai sinh nước VNDCCH ngày 2/9/1945.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 1, tr. 101 – 124',
    relatedMilestone: 'VI',
  },
  {
    keywords: ['điện biên phủ', '1954', 'đánh chắc tiến chắc', 'kháng chiến chống pháp'],
    question: 'Ý nghĩa của quyết định chuyển sang phương châm "Đánh chắc, tiến chắc" tại Điện Biên Phủ?',
    answer:
      'Ngày 26/1/1954, Đại tướng Võ Nguyên Giáp và Đảng ủy Chiến dịch quyết định chuyển từ "Đánh nhanh, giải quyết nhanh" sang "Đánh chắc, tiến chắc". Quyết định lịch sử này bảo đảm nguyên tắc "Chắc thắng mới đánh", đưa chiến dịch Điện Biên Phủ đến toàn thắng sau 56 ngày đêm (7/5/1954).',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 2, tr. 172 – 181',
    relatedMilestone: 'VII',
  },
  {
    keywords: ['1975', 'đại hội iii', 'hai miền', 'hồ chí minh', 'thống nhất'],
    question: 'Mối quan hệ giữa hai chiến lược cách mạng ở hai miền Bắc - Nam (1954–1975)?',
    answer:
      'Đại hội III (9/1960) xác định: Cách mạng XHCN ở miền Bắc giữ vai trò quyết định nhất đối với sự phát triển của toàn bộ cách mạng Việt Nam; cách mạng dân tộc dân chủ nhân dân ở miền Nam giữ vai trò quyết định trực tiếp đối với sự nghiệp giải phóng miền Nam, thống nhất đất nước ngày 30/4/1975.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 2, tr. 191 – 237',
    relatedMilestone: 'IX',
  },
  {
    keywords: ['đổi mới', '1986', 'đại hội vi', 'kinh tế thị trường', '2045', 'đại hội xiii'],
    question: 'Nội dung trọng tâm của đường lối Đổi mới từ Đại hội VI (1986) đến tầm nhìn 2045?',
    answer:
      'Đại hội VI (12/1986) khởi xướng đường lối Đổi mới toàn diện, xóa bỏ cơ chế tập trung quan liêu bao cấp, phát triển kinh tế nhiều thành phần. Đến Đại hội XIII (2021), Đảng đặt mục tiêu đến năm 2030 (100 năm thành lập Đảng) là nước đang phát triển có công nghiệp hiện đại, và đến năm 2045 (100 năm thành lập Nước) trở thành nước phát triển, thu nhập cao.',
    citation: 'Giáo trình Lịch sử Đảng CSVN (2021), Chương 3, tr. 256 – 395',
    relatedMilestone: 'XII',
  },
];
