# Lữ Khách Ký Ức — RPG Exploration implementation plan

> Planning document only. Baseline: main @ 21028242 (2026-11-09). Target: local-first single-player web game; no backend, payments, publishing or commercial assumptions.

## Vision and non-goals
Turn the existing left/right historical demo into a **2D semi-open-world narrative exploration RPG**: interact with believable NPCs, investigate evidence, traverse connected districts, solve environmental puzzles, collect memories and reconstruct historically grounded dossiers. Historical events are not rewriteable. Fictional agents, dialogue and the speculative 2045 hub must be labelled as creative framing. No combat, crafting economy, procedural world generation, multiplayer, open internet AI or full 3D engine in the initial vertical slice.

## Baseline audit (from source)
- \`src/main.ts\` combines game loop, input, player position, scene progression and hazards; \`teleportToMilestone()\` also adds to collected state; moving and crouching work during modal dialogues.
- \`isPlayerInShelter()\` treats crouching anywhere as safe; climax gate has no prerequisite check; delayed chapter transition has no stale scene guard.
- \`src/engine/PixelRenderer.ts\` renders 320×180 Canvas 2D, weather, parallax, player, hazards and four one-off set-pieces. Keep it initially, refactor for tilemap/scene graph.
- \`src/engine/SpriteAtlas.ts\` is hard-coded pixel matrices; keep as fallback while asset atlas is introduced.
- \`src/ui/HudController.ts\` renders permanently visible sidebar and lookup with keyword filtering (not a live AI service).
- CSS \`#climax-step-b { display:flex }\` may override HTML \`hidden\`; \`OpeningFracture.ts\` says true black but draws #0D0A08.
- No repository CI/test workflow observed at baseline; no browser or build verification implied.

## UI/UX target
Game world takes the main viewport (desktop landscape first). Only a small chapter/location label top-left, current objective top-right, contextual E prompt near interactables. Dialogue appears as an accessible DOM overlay with speaker portrait, history/skip options; J opens Journal, M opens local World Map, I opens Inventory, Tab/Esc opens Pause. Hide large navigation while exploring. All overlays implement focus management and suppress world input. Support scalable readable fonts, reduced motion, keyboard remapping and touch controls on supported mobile layouts.

## Gameplay loop
1. **Explore:** navigate a room/district and discover NPCs, landmarks, hidden paths.
2. **Investigate:** talk, inspect, compare source fragments, update evidence board.
3. **Decide a route:** accessible main path plus optional safer/longer path; no arbitrary world teleports in normal mode.
4. **Resolve:** complete meaningful puzzle, traversal or stealth situation; feedback must explain outcome.
5. **Record:** archive verified dossier, gain optional insight/tools and unlock next area.
6. **Return:** revisit an area from the 2045 hub for side quests and missed artifacts.

## Four era mechanics
| Era | Specific loop | Finale |
|---|---|---|
| 1930 Hương Cảng | environmental investigation, patrol stealth, provenance puzzle | reconstruct First Political Platform dossier |
| 1945 | messenger route planning, coded communication, NPC coordination | communication network and Ba Đình interactive scene |
| 1954–1975 | supply-route logistics, terrain constraints, documentary connections | separate Điện Biên Phủ and 1975 scenes |
| 1986–2045 | timeline causal-link puzzle, urban growth, policy trade-off exploration | verified Đổi mới dossier + clearly labelled speculative future |

## Vertical slice: Chapter I (12–18 minutes target, to be playtested)
- Hub prologue: enter 1930 memory, teach movement/interactions.
- Zone A **Hương Cảng Bến Cảng**: meet fictional courier, inspect a crate/manifest, learn camera and prompt, read patrol light cones, open an optional warehouse service passage.
- Zone B **Phố Cổ / Hẻm**: find librarian contact, investigate three independent evidence items, acquire key, unlock secret archive; optional dock-worker errand reveals patrol timing.
- Zone C **Kho Lưu Trữ Bí Mật**: match each document to time/source/context, return feedback without punitive reset, then assemble dossier.
- Cinematic close: 1930 dossier seal and memory tear; unlock hub/codex and first milestone. The geography is intentionally an abstract fictionalized micro-setting, **not an authoritative 1930 city map**.
- Three NPC roles: courier, archivist, dock observer. All invented NPCs are labelled fictional, with historical facts sourced independently.
- No quiz-gated cliff: puzzle information comes from exploration; a journal hint is available after repeated failure.

## Suggested architecture
\`src/core/{GameApp,GameState,InputManager,SaveManager}.ts\`
\`src/world/{WorldGraph,SceneManager,TileMap,Collision,Camera2D}.ts\`
\`src/systems/{Quest,Dialogue,Interaction,Inventory,Evidence,Stealth}.ts\`
\`src/ui/{HUD,Journal,WorldMap,Inventory,Dialogue,Pause}.ts\`
\`src/data/{chapters,quests,npcs,locales,provenance}/\`
\`public/assets/{atlases,portraits,backgrounds,audio}/\`
\`tests/{unit,playwright}/\`
Keep one authority for world state; render must not mutate progression. Keep game state separate from DOM state. Use typed events. Use dt clamping/fixed simulation updates for consistent collision/stealth behavior.

## Implementation PR sequence
- **PR-A Gameplay containment:** protect climax prerequisites, separate Presenter Mode, fix crouch stealth, modal/game input, hidden CSS, timeouts, pointer cancellation, scene-specific checkpoint; add regression tests.
- **PR-B State & quests:** typed GameState, QuestSystem, Inventory/Evidence, save schema with version/migration and tests.
- **PR-C World map:** scene graph, 16×16 tile metadata, collision and triggers, travel transitions, coordinate conversion, minimap/journal. Preserve old renderer fallback.
- **PR-D UI:** fullscreen view, context HUD, accessible dialogues, overlays and pause.
- **PR-E Art pipeline:** curate a limited atlas from Angais dataset, preserve source metadata, batch draw, palette correction and asset fallback.
- **PR-F Chapter I slice:** populate all zones and three NPCs, implement stealth + evidence puzzle + side quest + finale.
- **PR-G Polish/QA:** animation timing, audio, perf budget, visual consistency, real browser tests, historical source review.

## Acceptance (not claims of completed work)
- Local \`npm install && npm run build\` succeeds; add CI and tests before merge.
- Core traversal, puzzle, side quest and finale complete without cheats; normal route cannot skip quest conditions.
- Dialogue/pause/map prevent player motion; remapped and touch controls cannot leave stuck input.
- Saving in zone B and loading returns correct NPC/evidence/checkpoint state; stale timers cannot change chapters.
- Target measured 60 FPS on reference desktop and graceful 30 FPS mobile, not assumed.
- No large raw dataset or unreviewed generated sprites committed; no runtime Hugging Face dependency.
- Distinguish documented facts, fictionalized interactions and 2045 aspirations on every relevant dossier.
- Keep \`README.md\` and \`DESIGN.md\` updated when implementation begins.
See [WORLD_MAP.md](WORLD_MAP.md), [SPRITE_ATLAS.md](SPRITE_ATLAS.md) and [SKILLS.md](SKILLS.md).
