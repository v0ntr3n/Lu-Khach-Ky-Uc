---
name: rpg-map-design
description: Design and validate semi-open 2D RPG maps, multi-zone transitions, tile collision, branching exploration and map UX for Lữ Khách Ký Ức. Use for Chapter I map, scene topology, Tiled/JSON map layouts or navigation bugs.
---
# RPG map designer — Lữ Khách Ký Ức

1. Read \`docs/plans/WORLD_MAP.md\`, \`docs/plans/RPG_EXPLORATION.md\`, \`src/main.ts\`, \`src/engine/PixelRenderer.ts\`, \`src/types/game.ts\`.
2. Draw a *topology* before pixels: zone IDs, edges, gating flags, optional paths, side quest and guaranteed return routes. Hương Cảng and Nghệ Tĩnh are different memories, not adjacent physical zones.
3. Author 16×16 collision map + decorative/background/foreground layers per zone; 320×180 is the current logical viewport. Do not assume 3D movement or fully top-down maps.
4. Specify player spawns, exits (to + spawn), checkpoints, quest items, NPC interaction triggers, patrol cones and occluders.
5. Validate quest solvability using graph reachability and flag dependencies; no softlocks, inaccessible required interactables, unintentional teleport unlocks or orphan exits.
6. Separate scene spatial state, quest flags and UI. Debug visualization must show camera viewport, colliders, entrances, stealth cones.
7. Supply a table of map coordinates, playable routes and 5 mandatory test walks (main route, alternate stealth, side route, return-to-hub, save/reload).
8. Iterate with actual browser playtests at desktop and mobile landscape sizes; record screenshot and measurements, not subjective claims.

Deliverables: annotated zone graph, typed map JSON/TS files, tests, screenshot walkthrough and limitations. Do not redesign gameplay in the renderer alone.
