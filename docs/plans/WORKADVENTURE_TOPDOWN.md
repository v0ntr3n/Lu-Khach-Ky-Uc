# WorkAdventure-inspired top-down RPG map plan — Lữ Khách Ký Ức

> **Status:** implementation plan only, NOT a completed game feature.  
> **Decision date:** 2026-11-10.  
> **Baseline:** main at d04d3b06a78a89ad1b671c0a4c5f5d103fba3cd6.  
> **Goal:** local-first, offline-capable, single-player historical RPG exploration with connected top-down maps inspired by the navigation and map authoring of Avatar TeaMobi and WorkAdventure.

## 1. Architectural decision / what changed

**NEW TARGET:** replace the existing 1-axis side-scroller with an **orthogonal top-down / 3/4-view 2D RPG**. Player can navigate X/Y in four or eight input directions (initial animation can use four facing directions), enter buildings, follow connected roads, talk to NPCs, explore alleys and revisit zones. Streets should feel like a small lived-in town, not a 1080 px corridor or a world-menu screenshot.

**Supersedes the movement and tilemap model** in WORLD_MAP.md, which described A/D side-scrolling and vertical movement as a future possibility. Other valuable concepts in that document (zone graph, fictionalized geography, quest safety, hidden shortcuts) remain valid. This is a design target, NOT a claim that the current engine supports top-down motion.

**Keep:** TypeScript + Vite, historic chronicles and milestone IDs, Sơn Mài/Giấy Dó visual identity, opening fracture, SoundSynth, authored dialogue/story material and curated sprite pipeline.

**Replace/refactor:** playerX-only state and ground-y physics, side-scrolling camera, fixed 1D milestone positions, one-sprite NPC approach, sidebar-driven travel, render-dependent progression.

**Not in scope:** multiplayer, avatars synchronized over a server, WebRTC, chat rooms, authentication, real-time map editing, copying WorkAdventure engine/assets, MMORPG services or changing established historical outcomes.

## 2. Source references and boundaries

| Source | What we learn | What we should NOT copy |
| --- | --- | --- |
| [WorkAdventure](https://github.com/workadventure/workadventure) | Top-down walkable world, spatial encounters, map transitions, interactive areas | Network stack, live video, realtime presence, full platform dependencies |
| [WorkAdventure Map Starter Kit](https://github.com/workadventure/map-starter-kit) | Tiled TMJ maps, separate tilesets and scripts, named areas and scripted map interactions | Its runtime API, server-side app or assets without checking their individual licenses |
| [WorkAdventure Tiled tutorial](https://docs.workadventu.re/map-building/tiled-editor/) | Layer/object authoring workflow and map publishing techniques | Assumptions that their hosted services are needed for a local game |
| [challanger/rpg-topdown](https://github.com/challanger/rpg-topdown) | Small Phaser 3 + TypeScript + Vite example: 8-way movement, Tiled collision, interior doors with target map/spawn | Unrelated combat systems and art |
| [Angais pixel-art-atlas-1.4m](https://huggingface.co/datasets/Angais/pixel-art-atlas-1.4m) | Curated static prop sprites | Bulk raw dataset, unreviewed/provenance-unclear art |

WorkAdventure is a **reference for UX and map structure**, not an implementation dependency. A local single-player game MUST run without WorkAdventure services or network connection once built and cached/downloaded.

## 3. Rendering / engine decision: Phaser 3 spike, not an unconditional rewrite

**Preferred technical spike:** Phaser 3 (TypeScript) + Tiled (.tmj/.json) + original DOM overlay HUD. This supplies camera follow, tile collision, animation, layered sprites, interaction overlap and easy scene transitions. Existing Canvas2D renderer can temporarily power the old gameplay route and the opening cinematic until the spike passes.

**Alternative:** extend the current PixelRenderer with TileMapLoader, WorldCamera, CollisionWorld and y-depth sprite sorting. Choose this if the Phaser spike fails bundle, input, integration or visual-quality gates; do not claim either approach is faster until measured.

**Not recommended for vertical slice:** importing the full WorkAdventure/Reldens server or migrating everything to RPG-JS just to obtain a map. Keep historical quest/state systems owned by this project.

**Go/no-go proof (PR-M1):** outdoor 64x48 tile map + interior map; eight-way normalized input; solid building collisions; player enters building and returns at exact named spawn; y-sorting works; desktop 60 FPS reference target measured; current opening/HUD can mount in same app; TypeScript build and automated smoke tests pass. Do not replace current renderer before these checks.

## 4. Map authoring specification (Avatar-like town)

- **Projection:** orthogonal top-down with 3/4-looking sprites and roofs, NOT isometric diamonds. Tile movement occurs in the XY plane; there is no gravity/jump.
- **Tile grid:** 16x16 logical pixels for ground and collisions, compatible with the dataset prop scale. Player sprites may be 16x24 or 24x32; only feet occupy collision space.
- **Suggested first outdoor map:** harbor square 64x48 tiles = 1024x768 logical pixels. Old street 48x40; harbor warehouse 36x28; archive interior 32x24. Sizes are editable estimates, not finalized level measurements.
- **Viewport:** prototype can retain 320x180 logical pixels, integer-scale when possible; test how small 16x16 sprites appear on common monitors. UI text is native-resolution DOM, not pixel text painted into the game canvas.
- **Movement:** WASD/arrows, 8-direction movement with diagonal vector normalized; four or eight facing animations depending on approved sprite atlas. No sudden instant turns during dialogue/cutscenes.
- **Camera:** smooth follow with a tiny dead-zone; clamp to map bounds; never reveal void; prevent snap jitter on 1-pixel sprite movement.
- **World order:** ground -> below-player props -> entities sorted by footY -> foreground/roofs -> light/rain overlay. A tall tree is obstructive at its trunk, not across the full artwork.
- **Interior:** separate scene files, entrance named object with toMap/toSpawn, one short transition, return cooldown to avoid door bounce.
- **World-map UI:** M opens discovered districts and known routes; it does NOT auto-collect evidence or fast-travel to undiscovered spots.

### Tiled layer contract

| Layer | Type | Purpose |
| --- | --- | --- |
| Ground | Tile layer | Streets, pavement, interior floor, harbor road |
| GroundDetails | Tile layer | Cracks, puddles, markings, visual only |
| LowProps | Tile layer/object group | Crates, benches, small items behind player |
| Structures | Tile layer | Walls, house bases, roofs with explicit collision metadata |
| Foreground | Tile layer | Roof edges, tree canopy, overhangs in front of player |
| Collision | Tile layer or object layer | SOLID world geometry; never inferred from pixels |
| LightOccluders | Object layer | Cover/line-of-sight blockers and lantern shadow masks |
| NPCs | Object layer | Stable NPC IDs, facing, path IDs and interaction radii |
| Interactables | Object layer | Evidence, signs, desks, event triggers and quest flags |
| Entrances | Object layer | Named spawn/exit and two-way destination links |
| PatrolPaths | Object layer | Waypoints, speed, cone size and zone of surveillance |
| Checkpoints | Object layer | Safe return positions, unlocked save points |

**Required editor metadata:** all objects have unique stable IDs; doors have destinationMap/destinationSpawn; each destination spawn exists and is walkable; interactables have actionId and (optional) requiredQuestFlag; NPCs have dialogueId, fictionTag and approach radius; patrols have waypoints.

### Sample Tiled object properties (design contract, not existing runtime JSON)

~~~json
{
  "id": "exit_harbor_to_warehouse",
  "type": "portal",
  "x": 512, "y": 400, "width": 32, "height": 16,
  "properties": {
    "destinationMap": "ch1_warehouse",
    "destinationSpawn": "warehouse_south_door",
    "requiresFlag": "ch1.courier_met",
    "transition": "fade"
  }
}
~~~

Editor JSON/TMJ must be **validated or normalized at build time**; never scatter magic pixel coordinates in game logic. Do not serialize human-readable history text into tile properties: reference quest/lore IDs instead.

## 5. Chapter I map — 1930 Hương Cảng (fictionalized town topology)

~~~text
                  [Old Street + NPC] ───── [Shadow Alley / patrol]
                      │    │                        │
                      │    └─ [Safe House Interior] │
                      │                             │
 [Small Pier / optional] ─ [Harbor Square / HUB] ─ [Dock Warehouse]
                               │                     │
                               └──── [Service Lane] ─┘
                                         │
                               [Archive Courtyard]
                                         │ (quest gate)
                               [Secret Archive Interior]
                                         │
                                  [1930 Dossier]
~~~

This is **gameplay topology, not a factually accurate road map of 1930 Hong Kong**. Do not show historical Nghệ Tĩnh as a neighboring tilemap of Hương Cảng; it is a separate memory/cinematic transition.

### Player route and purpose of each district

| Zone | What a player can do | Key dependencies |
| --- | --- | --- |
| Harbor Square | Spawn, talk to fictional courier, locate main streets | Tutorial and quest start |
| Old Street | Talk to archivist, inspect notice board, discover rumor/evidence | Courier topic unlocked |
| Small Pier | Optional dock-observer quest and shortcut clue | None; can revisit |
| Shadow Alley | Patrol light cone, cover, alternate safe walkway | Stealth or optional bypass |
| Dock Warehouse | Find shipping ledger, inspect barrels/crates, acquire clue | Interact with correct props |
| Safe House | Optional character story, first interior transition | Enter/return door pair |
| Archive Courtyard | Check quest flags, find entrance, checkpoint | Three evidence collected |
| Secret Archive | Assemble factual dossier through evidence-board puzzle | Puzzle solved then climax |

**Slice reduction for first playable:** implement **Harbor Square 64x48**, **Old Street / Alley segment**, and **Archive Interior** first; use a warehouse door as the first interior/exit test. Other districts become map data and art backlog, not initial blockers.

### NPC roles

Courier = main mission; Archivist = source comparison; Dock Observer = optional safer passage. NPCs are invented historical-fiction characters, labelled accordingly. Historic facts need separately verified provenance.

## 6. Interactions, gameplay and HUD

**Interaction:** approaching an entity shows short prompt (“E · Trò chuyện”, “E · Xem văn kiện”, “E · Vào nhà”). Interact once per key press; repeated keydown must not duplicate dialogue or claim evidence. Inventory/journal opened from DOM overlay. Input automatically locks when dialogue, puzzle, pause or cutscene is active.

**Evidence:** no UI navigation, movement or PresenterMode teleport can grant real evidence. Only validated quest actions can change progression. Every evidence ID exists once in the save state and has its cited/fictional provenance label.

**Stealth:** line of sight is a geometric cone; structural walls and authored occluders block it. Crouch lowers detection but does not grant invincibility outside cover. There is alert -> investigation -> reset/cooldown -> checkpoint; retain found clues on reset.

**Town feel:** a few purposefully moving/idle NPCs, ambient animation (lanterns, rain, open doors), place-name transitions, grounded props, optional discoverable interior, 2+ routes for one meaningful objective. Avoid populating streets with decorative non-interactable “NPC clones.”

**HUD:**
- Top left: chapter + current location.
- Top right: 1 short objective, no full 12-milestone sidebar.
- On proximity: E prompt; on detection: short stealth indicator.
- Dialogue: bottom portrait and multi-choice card with readable Vietnamese typography.
- J = Journal/Codex; M = discovered map; I = Items; Esc = pause; Q = inspection lens.
- Presenter's “skip / unlock all” isolated in explicit PresenterMode, never persisted as normal progress.

## 7. Sprite pipeline

Follow [SPRITE_ATLAS.md](SPRITE_ATLAS.md): curate a **small review set** of static lamps, props, crates, documents, foliage, benches and roof details from Angais dataset; preserve row IDs, provenance and review status. Do not bulk-download 1.4M assets or commit the raw dataset. Create authored character walk cycles with 4 facing directions (idle and walking) and unique NPC silhouettes.

Use atlas image + frame metadata + anchors + separate collision geometry; nearest-neighbor sampling; cache textures once. Align art into consistent historical architecture and Sơn Mài/Giấy Dó palette; no one-click random sprite scattering.

Rain and glow can use selected PixelFX-like effects, but only if measured within frame budget; game feel and readability come first.

## 8. Migration against the actual repository

| Current file | Migration target | Reuse vs rewrite |
| --- | --- | --- |
| src/main.ts | App boot + GameStore + InputManager | Rewrite 1D position and keyboard routing, preserve event semantics |
| src/engine/PixelRenderer.ts | Legacy renderer until Phaser spike passes; later RenderScene adapter | Reuse opening / visual assets; replace groundY + 1D camera |
| src/engine/SpriteAtlas.ts | Sprite catalog/atlas loader | Keep procedural fallback only |
| src/engine/OpeningFracture.ts | DOM opening overlay mounted around Phaser canvas | Reuse with first-frame/skip fixes |
| src/engine/SoundSynth.ts | Audio bridge | Reuse |
| src/data/chronicles.ts | Lore/Codex + factual milestones | Reuse IDs/content after provenance review, decouple from world coordinates |
| src/ui/HudController.ts | DOM HUD + Dialog + Journal + Map components | Refactor fixed sidebar |
| src/types/game.ts | SceneId, XY positions, NPC, Portal, QuestFlag, SaveV2 | Extend; do not overload Milestone ID as map ID |

**Important save migration:** save v1 (chapter + scalar playerX) cannot be treated as XY positions. Version a new save schema; migrate only safe chapter/evidence state or start at a named spawn. Never silently put a player inside collision.

### Canonical target directory layout

**Single source of truth for the target directory structure (M1+).** [RPG_EXPLORATION.md](RPG_EXPLORATION.md) references this exact layout instead of maintaining a divergent path list. These are **planned paths**, not existing files. M1 must create only what is needed to validate the two-map spike; later PRs introduce the other modules. Keep the current source tree working until the spike passes.

~~~text
src/
  main.ts                       # existing Vite entrypoint; becomes thin bootstrap
  core/
    GameApp.ts                   # lifecycle and game mode/state transitions
    GameStore.ts                 # authoritative serializable game state
    InputManager.ts              # keyboard/touch + UI focus gating
    SaveManager.ts               # versioned local persistence
  runtime/
    PhaserRuntime.ts             # Phaser 3 initialization; isolated spike
    MapLoader.ts                 # parse/load Tiled TMJ/JSON and tilesets
    CameraController.ts          # follow/zoom/clamp
    SceneTransition.ts           # named portal + spawn and safe transitions
  scenes/
    BootScene.ts
    HarborScene.ts               # M1 outdoor test scene
    WarehouseInteriorScene.ts    # M1 indoor test scene
  world/
    WorldGraph.ts                # discoverable zones / connections
    CollisionWorld.ts            # world collision data; no pixel sampling
    MapValidator.ts              # map IDs, exits, spawns, reachability
  systems/
    QuestSystem.ts
    DialogueSystem.ts
    InteractionSystem.ts
    InventorySystem.ts
    EvidenceSystem.ts
    StealthSystem.ts
  data/
    chronicles.ts                # existing milestone and historical content
    chapters/
    quests/
    npcs/
    locales/
    provenance/
  engine/
    OpeningFracture.ts           # existing opening, keep DOM overlay
    SoundSynth.ts                # existing audio bridge
    PixelRenderer.ts             # existing Canvas 2D legacy/fallback
    SpriteAtlas.ts               # existing procedural sprite fallback
  ui/
    GameHUD.ts
    JournalPanel.ts
    WorldMapPanel.ts
    InventoryPanel.ts
    DialoguePanel.ts
    PausePanel.ts
    HudController.ts             # existing UI; migrate incrementally
  types/
    game.ts                      # existing; extend for XY, MapId, Quest, SaveV2
public/
  maps/ch1/
    harbor_square.tmj            # M1 outside
    warehouse_interior.tmj      # M1 inside
  tilesets/ch1/
    harbor_tiles.png
    interior_tiles.png
  assets/
    atlases/
    portraits/
    backgrounds/
    audio/
tests/
  unit/
  maps/
  playwright/
tools/
  validate_maps.ts               # map validator CLI (later PR)
~~~

**Ownership rules and non-goals:**
- `src/runtime/` owns **Phaser integration**, including scene transitions and camera. Do not duplicate `WorldCamera`, `SceneManager`, `TileMapLoader` under `src/world/`.
- `src/world/` owns **engine-independent map geometry/graph/validation**. `src/systems/` owns gameplay rules; only `src/core/SaveManager.ts` writes saves (no `src/systems/Save.ts`).
- **Tiled authored maps** live in `public/maps/ch1/` and **tileset PNGs** in `public/tilesets/ch1/`; no canonical `src/data/maps/` copy. Game content/quests/lore live in `src/data/`, referenced by stable IDs from Tiled objects.
- `src/engine/` contains retained **legacy/presentation modules** and remains in place during M1. Do not destructively rename existing runtime classes before proving the Phaser 3 spike.
- `src/ui/` owns the **DOM accessibility layer**, which reads state but cannot unlock evidence. `public/assets/` contains curated only, not the complete Hugging Face dataset.
- `tests/unit`, `tests/maps` and `tests/playwright` are the only planned test folders. Use imports and naming consistent with this tree; no empty scaffolding required.

**M1 minimum subset:** `src/runtime/{PhaserRuntime,MapLoader,CameraController,SceneTransition}.ts`, `src/scenes/{BootScene,HarborScene,WarehouseInteriorScene}.ts`, two map TMJs under `public/maps/ch1/`, their reviewed tilesets under `public/tilesets/ch1/`, and corresponding tests. Additional game systems are added only when independently required.


## 9. PR roadmap and acceptance gates

| PR | Deliverable | Must pass |
| --- | --- | --- |
| M0 — P0 containment | Fix progression bypass, crouch invulnerability, modal inputs, hidden CSS, stale transitions, presenter mode | Regression tests + current production build |
| M1 — top-down technical spike | Phaser 3 + 2 Tiled scenes, eight-way move, walls, camera, named door/return | Build + automated movement/portal tests + measured baseline |
| M2 — data-driven maps | Outdoor district authoring, layer rules, zone graph, portal validator, debug collision view | Reachability and spawn/exit validation |
| M3 — state / RPG systems | Quest, NPC dialog, evidence, save v2, HUD, map and journal | Reload and gating integration tests |
| M4 — Chapter I playable | 3 NPCs, 3 evidence, 1 optional quest, 1 stealth, dossier puzzle, climax | Fresh playthrough with no cheat and no softlock |
| M5 — art, audio and accessibility | Curated sprites, 4-facing character animations, rain, soundscape, touch, reduced motion | Real-browser screenshot/UX and performance measurements |
| M6 — documentation/CI | README update, authoring docs, source review, CI, GitHub Pages if chosen | Green tests + reproducible build on fresh checkout |

**Exit criteria for first map prototype:**
- [ ] Build succeeds in fresh install; works without account/backend.
- [ ] WASD + arrow keys move in 8 directions at equal normal/diagonal speed.
- [ ] Player cannot walk through walls, roof bases, water or NPC blocking zones.
- [ ] Sprite y-order is correct before/behind trees, roofs and objects.
- [ ] Map and named spawn round-trip through a building without portal ping-pong.
- [ ] Camera clamps to finite map boundaries; no empty void at edges.
- [ ] Nearby E shows a prompt, triggers exactly once, and focus stays accessible.
- [ ] Overlay pause/dialogue stops world movement; blur/pointercancel never sticks input.
- [ ] Quest/evidence flags remain correct after save/reload and map revisits.
- [ ] Main quest graph reachable; optional alternative route exists; no softlock.
- [ ] 60 FPS desktop target and 30 FPS mobile landscape target are **measured**, not assumed.
- [ ] Map coordinates, character art, quote/citation labels and fictional locations pass review.

## 10. Agent instructions and conflict resolution

1. Read this file **before** WORLD_MAP.md, RPG_EXPLORATION.md and the map design skill whenever working on map movement/architecture.
2. If there is conflict, this ADR governs **top-down movement, XY coordinates, tile layers, map scene transitions and Phaser spike**. Existing docs govern historical source/provenance, quest safety and art curation unless explicitly revised here.
3. Do not implement all seven PRs as one change or delete the working renderer before the top-down proof works.
4. WorkAdventure is inspiration; do not add a dependency on its hosted platform, WebRTC or live services.
5. Follow actual test/screenshot evidence, and record meaningful failure cases in each PR.
