/**
 * @file src/types/game.ts
 * @description Định nghĩa kiểu dữ liệu cốt lõi cho "Lữ Khách Ký Ức (1930 – 2045)"
 * theo chuẩn Kiểu 1 (Vite + TypeScript Modular Architecture).
 */

export type EraId = '1930' | '1945' | '1954_1975' | '1986_2045';

export type RomanMilestoneId =
  | 'I'
  | 'II'
  | 'III'
  | 'IV'
  | 'V'
  | 'VI'
  | 'VII'
  | 'VIII'
  | 'IX'
  | 'X'
  | 'XI'
  | 'XII';

export interface ShelterZone {
  xStart: number;
  xEnd: number;
  label: string;
}

export interface HazardSweep {
  id: string;
  xCenter: number;
  radius: number;
  speed: number;
  phaseOffset: number;
  label: string;
  warningToast: string;
}

export interface MilestoneArtifact {
  roman: RomanMilestoneId;
  eraId: EraId;
  year: string;
  title: string;
  worldX: number;
  spriteType: 'scroll' | 'lantern' | 'seal' | 'radio' | 'flag' | 'bicycle' | 'tank' | 'metro';
  npcSpeaker: string;
  npcRole: string;
  dialogue: string;
  historicalSignificance: string;
  textbookCitation: string;
}

export interface ClimaxOption {
  id: string;
  text: string;
  isCorrect: boolean;
  feedback: string;
}

export interface ChapterClimax {
  gateWorldX: number;
  stepATitle: string;
  stepAPrompt: string;
  stepACitation: string;
  options: ClimaxOption[];
  stepBTitle: string;
  stepBSubtitle: string;
  stepBActionLabel: string;
  stepBCitation: string;
  setPieceType: 'kowloon_torch' | 'badinh_flag' | 'dienbien_tank' | 'doimoi_metro';
}

export interface ChapterData {
  id: EraId;
  chapterNum: string;
  years: string;
  title: string;
  subtitle: string;
  locationLabel: string;
  coordinates: string;
  worldWidth: number;
  mechanicHint: string;
  shelters: ShelterZone[];
  hazards: HazardSweep[];
  milestones: MilestoneArtifact[];
  climax: ChapterClimax;
}

export interface LookupEntry {
  keywords: string[];
  question: string;
  answer: string;
  citation: string;
  relatedMilestone: RomanMilestoneId;
}
