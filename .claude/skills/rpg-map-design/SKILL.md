---
name: rpg-map-design
description: Design and validate Avatar/WorkAdventure-inspired top-down RPG maps, Tiled layers, named doors, NPCs, XY movement, scene transitions, quest reachability and map UX for Lữ Khách Ký Ức.
---

# RPG map designer — Lữ Khách Ký Ức (top-down)

1. Read **docs/plans/WORKADVENTURE_TOPDOWN.md** first; then RPG_EXPLORATION.md, WORLD_MAP.md (legacy side-scroller reference), SPRITE_ATLAS.md, src/main.ts, src/engine/PixelRenderer.ts and src/types/game.ts.
2. Treat the design as **orthogonal top-down / 3-4-looking sprite RPG** inspired by the map authoring/navigation patterns of WorkAdventure and Avatar TeaMobi. Four/eight input directions, XY positions, no gravity or jump. Do not port WorkAdventure networking/server.
3. Draw the zone topology **before** individual pixel art: stable map IDs, connected edges, named spawn points, two-way doors, checkpoints, optional routes and quest flags. Geography in Hương Cảng is fictionalized; Nghệ Tĩnh is a separate memory, never a walkable adjacent neighborhood.
4. Author 16x16 logical tile layers with Tiled TMJ/JSON; each map needs Ground, Structures, Foreground, Collision, NPCs, Interactables, Entrances and optional LightOccluders/PatrolPaths. Keep separate collider data, anchors and Y-sorted render order.
5. Inspect WorkAdventure's Map Starter Kit for examples of authored map areas/layers, and challanger/rpg-topdown for a Phaser 3 + Vite + Tiled example. Use them as references, not third-party runtime dependencies or automatic asset sources.
6. Map annotations: explicit dimension, collision bodies, exit -> destination map/spawn, interactable radius, NPC dialogue IDs, quest gate condition, checkpoint and patrol cone. Each entrance must have a safe walkable return spawn and transition grace period.
7. Validate: unique IDs, reachability of required objectives, no orphan/exits to nowhere, no spawn inside collision, no irreversible softlock, legal fast travel only between unlocked points and no teleport granting evidence.
8. Supply local map walkthroughs: main path, optional path, stealth path, enter/exit an interior, save/reload in another zone; add screenshots/debug collider overlay from real browser tests.
9. Cross-check control focus, keyboard diagonals normalization, mobile landscape joystick/pointercancel, pause/dialogue suppression, camera clamping, FPS and exact source labeling.
10. For production, stage a Phaser 3 two-map technical spike and keep existing renderer as fallback until the spike passes build and runtime QA. Do not claim implementation without executing tests.

Deliverables: zone graph, layer specs, stable object schemas, Tiled files, map validators, test walks, performance measurements and known limitations.
