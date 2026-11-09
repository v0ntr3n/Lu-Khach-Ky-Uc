/**
 * @file src/types/game.ts
 * @description Định nghĩa kiểu dữ liệu cốt lõi cho "Lữ Khách Ký Ức (1930 – 2045)"
 * Hỗ trợ cả baseline M0 và kiến trúc RPG Exploration Top-Down (M1 – M6).
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

/**
 * Trạng thái điều khiển độc quyền của game.
 * Chỉ khi mode === 'exploring' người chơi mới có thể di chuyển và tương tác thế giới.
 */
export type GameMode =
  | 'opening'
  | 'exploring'
  | 'dialogue'
  | 'climax'
  | 'lookup'
  | 'journal'
  | 'map'
  | 'inventory'
  | 'puzzle'
  | 'paused'
  | 'cutscene'
  | 'transition';

export type Facing4 = 'down' | 'up' | 'left' | 'right';

export type Direction8 =
  | 'none'
  | 'n'
  | 'ne'
  | 'e'
  | 'se'
  | 's'
  | 'sw'
  | 'w'
  | 'nw';

export interface Vec2 {
  x: number;
  y: number;
}

export interface Rect2D {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type ProvenanceType = 'historical_fact' | 'fictional_npc' | 'speculative_2045';

export interface ProvenanceMetadata {
  type: ProvenanceType;
  label: string;
  citation?: string;
  historicalNote?: string;
}

export interface ShelterZone {
  xStart: number;
  xEnd: number;
  label: string;
  bounds?: Rect2D;
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
  worldY?: number;
  mapId?: string;
  spriteType: 'scroll' | 'lantern' | 'seal' | 'radio' | 'flag' | 'bicycle' | 'tank' | 'metro';
  npcSpeaker: string;
  npcRole: string;
  dialogue: string;
  historicalSignificance: string;
  textbookCitation: string;
  provenance?: ProvenanceMetadata;
}

export interface ClimaxOption {
  id: string;
  text: string;
  isCorrect: boolean;
  feedback: string;
}

export interface ChapterClimax {
  gateWorldX: number;
  gateWorldY?: number;
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
  worldHeight?: number;
  mechanicHint: string;
  shelters: ShelterZone[];
  hazards: HazardSweep[];
  milestones: MilestoneArtifact[];
  climax: ChapterClimax;
}

export interface LookupEntry {
  keywords: string[];
  question: number | string;
  answer: string;
  citation: string;
  relatedMilestone: RomanMilestoneId;
}

export interface PortalDefinition {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  destinationMap: string;
  destinationSpawn: string;
  requiresFlag?: string;
  transition?: 'fade' | 'instant';
  label?: string;
}

export interface SpawnPointDefinition {
  id: string;
  x: number;
  y: number;
  facing?: Facing4;
  label?: string;
}

export interface NpcRoutePoint {
  x: number;
  y: number;
  waitSec?: number;
  facing?: Facing4;
}

export interface NpcMapPlacement {
  id: string;
  npcId: string;
  name: string;
  role: string;
  x: number;
  y: number;
  facing: Facing4;
  dialogueId: string;
  fictionTag: ProvenanceType;
  approachRadius: number;
  milestoneRoman?: RomanMilestoneId;
  route?: NpcRoutePoint[];
  moveSpeed?: number;
}

export interface InteractableDefinition {
  id: string;
  x: number;
  y: number;
  width?: number;
  height?: number;
  radius?: number;
  kind: 'npc' | 'evidence' | 'door' | 'prop' | 'checkpoint' | 'puzzle' | 'climax_gate';
  label: string;
  actionId: string;
  requiresFlag?: string;
  grantsFlag?: string;
  milestoneId?: RomanMilestoneId;
  dialogueId?: string;
  evidenceId?: string;
  destinationMap?: string;
  destinationSpawn?: string;
}

export interface PatrolWaypoint {
  x: number;
  y: number;
  waitSec?: number;
}

export interface PatrolDefinition {
  id: string;
  waypoints: PatrolWaypoint[];
  speed: number;
  coneLength: number;
  coneAngleRad: number;
  label: string;
  warningToast: string;
}

export interface TiledProperty {
  name: string;
  type: 'string' | 'int' | 'float' | 'bool';
  value: string | number | boolean;
}

export interface TiledObject {
  id: number | string;
  name: string;
  type: string;
  x: number;
  y: number;
  width: number;
  height: number;
  properties?: TiledProperty[] | Record<string, string | number | boolean>;
  polyline?: Vec2[];
}

export interface TiledLayer {
  id: number;
  name: string;
  type: 'tilelayer' | 'objectgroup';
  width?: number;
  height?: number;
  data?: number[];
  objects?: TiledObject[];
  visible?: boolean;
  opacity?: number;
}

export interface TiledTilesetRef {
  firstgid: number;
  name: string;
  image: string;
  tilewidth: number;
  tileheight: number;
  tilecount: number;
  columns: number;
}

export interface TiledMapJson {
  width: number;
  height: number;
  tilewidth: number;
  tileheight: number;
  orientation: 'orthogonal';
  renderorder?: 'right-down';
  properties?: TiledProperty[] | Record<string, string | number | boolean>;
  tilesets: TiledTilesetRef[];
  layers: TiledLayer[];
}

export interface NormalizedMapData {
  id: string;
  title: string;
  eraId: EraId;
  districtType?: 'hub' | 'outdoor' | 'interior' | 'optional' | 'stealth' | 'finale';
  description?: string;
  widthInTiles: number;
  heightInTiles: number;
  widthPx: number;
  heightPx: number;
  tileSize: number;
  defaultSpawnId: string;
  tileLayers: Record<string, number[]>;
  obstacles: Rect2D[];
  shelters: ShelterZone[];
  occluders: Rect2D[];
  spawns: SpawnPointDefinition[];
  portals: PortalDefinition[];
  npcs: NpcMapPlacement[];
  interactables: InteractableDefinition[];
  patrols: PatrolDefinition[];
  checkpoints: SpawnPointDefinition[];
}

export interface WorldZoneEdge {
  fromZoneId: string;
  toZoneId: string;
  portalId: string;
  destinationSpawn: string;
  requiresFlag?: string;
  isOptionalRoute?: boolean;
  label: string;
}

export interface WorldZoneNode {
  id: string;
  title: string;
  eraId: EraId;
  districtType: 'hub' | 'outdoor' | 'interior' | 'optional' | 'stealth' | 'finale';
  description: string;
  gridPos: Vec2; // Tọa độ hiển thị trên sơ đồ World Map (M)
  checkpointId?: string;
  checkpointSpawnId?: string;
  objectiveCount: number;
  edges: WorldZoneEdge[];
}

export type QuestObjectiveStatus = 'locked' | 'active' | 'completed';

export interface QuestObjective {
  id: string;
  title: string;
  description: string;
  zoneId: string;
  requiresFlags?: string[];
  requiresEvidence?: string[];
  requiresMilestones?: RomanMilestoneId[];
  completedByFlag?: string;
  completedByEvidence?: string;
  completedByMilestone?: RomanMilestoneId;
  isOptional?: boolean;
  hintText?: string;
}

export interface QuestDefinition {
  id: string;
  eraId: EraId;
  title: string;
  subtitle: string;
  category: 'main' | 'side';
  summary: string;
  objectives: QuestObjective[];
  rewardFlag?: string;
  provenance: ProvenanceMetadata;
}

export interface DialogueChoice {
  id: string;
  text: string;
  nextNodeId: string | null;
  requiresFlag?: string;
  requiresEvidence?: string;
  grantsFlag?: string;
  grantsEvidence?: string;
  grantsMilestone?: RomanMilestoneId;
  responseToast?: string;
}

export interface DialogueNode {
  id: string;
  speaker: string;
  role: string;
  text: string;
  provenance: ProvenanceMetadata;
  choices: DialogueChoice[];
}

export interface DialogueTree {
  id: string;
  eraId: EraId;
  npcId: string;
  rootNodeId: string;
  nodes: Record<string, DialogueNode>;
}

export interface EvidenceItem {
  id: string;
  eraId: EraId;
  title: string;
  category: 'document' | 'cipher' | 'artifact' | 'dispatch';
  zoneId: string;
  shortSummary: string;
  historicalQuote: string;
  historicalSignificance: string;
  textbookCitation: string;
  provenance: ProvenanceMetadata;
  relatedMilestone?: RomanMilestoneId;
  grantsFlag?: string;
}

export interface GameSettings {
  muted: boolean;
  reducedMotion: boolean;
  textScale: 'normal' | 'large';
  showHints: boolean;
}

export interface SaveDataV1 {
  version?: 1;
  chapterId: EraId;
  playerX: number;
  collectedMilestones: RomanMilestoneId[];
}

export interface SaveDataV2 {
  version: 2;
  timestamp: number;
  eraId: EraId;
  currentMapId: string;
  spawnId: string;
  playerPos: Vec2;
  playerFacing: Facing4;
  collectedMilestones: RomanMilestoneId[];
  collectedEvidence: string[];
  questFlags: string[];
  discoveredMaps: string[];
  unlockedCheckpoints: string[];
  completedChapters: EraId[];
  activeQuestIds?: string[];
  completedQuestIds?: string[];
  completedObjectiveIds?: string[];
  dialogueHistory?: string[];
  settings?: GameSettings;
}

export interface HudCallbacks {
  onSelectEra: (eraId: EraId) => void;
  onTeleportToMilestone: (milestone: MilestoneArtifact) => void;
  onTriggerClimaxStepB: () => void;
  onAdvanceNextChapter: () => void;
  onToggleLecturerUnlockAll: () => void;
  onToggleMute: () => boolean;
  onOverlayStateChange?: (isOpen: boolean, overlayName: GameMode) => void;
  onFastTravelToZone?: (zoneId: string, spawnId: string) => void;
  onToggleDebugOverlay?: () => boolean;
  onSelectDialogueChoice?: (choiceId: string) => void;
  onManualSave?: () => void;
  onResetSave?: () => void;
  onUpdateSettings?: (partial: Partial<GameSettings>) => void;
}

