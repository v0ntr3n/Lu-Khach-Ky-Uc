# World / map design — first playable slice

## Decision: 2D connected districts, NOT a single scroll strip
The existing game is left/right side-scrolling at 320×180 with a playerX scalar and 1080 worldWidth. Preserve this foundation initially. **World map = connected directed graph of scenes; each scene = a camera-scrolling 2D tilemap** with physical blockers, exits, interactable entities, NPCs and quest flags. It is not a geo-accurate Google map or a single rendered background.

In RPG exploration mode, allow revisiting discovered zones and entering interiors. Press M to display a node map. Fast travel is Hub ↔ previously unlocked checkpoints only after unlocking; the presenter's teleport stays isolated from game progression.

## Chapter I topology (prototype geography; fictionalized)
\`\`\`text
                              [Archive House — interior]
                               secret / quest gated door
                                        |
[Old Street / NPC] ---- [Shadow Alley / patrol] ---- [Archive Courtyard]
        |                      |                       |
        |                 alternative stealth          |
        |                      |                       |
[Harbor Square / spawn] ---- [Dock Warehouse] ---- [Hidden Service Passage]
        |
    [Small Pier / optional side quest]
\`\`\`
This is a **topological map for gameplay**, not a claim that the named locations were arranged this way in historical 1930 Hương Cảng. Do not mix Nghệ Tĩnh into a contiguous walking map from Hương Cảng; it is a separate historical memory/scene transition.

## World map displayed to player
- Current zone: gold filled marker; visited: warm paper markers; locked: silhouettes/no spoilers; optional: smaller dotted markers.
- Exits visible only when discovered. Hover/select gives area name, objective count, known connected areas, and checkpoint state.
- Never show every puzzle item by default; optional exploration is rewarded.
- Accessibility: text list navigation; keyboard and touch; no color-only state encoding.

## Tiles and coordinate model
- 16px logical tiles; 320×180 internal viewport. Tilemap may be much wider than viewport, e.g. zone A 80×12 tiles (1280×192 world px), cropped smoothly into the 320×180 camera.
- Define tile layers: \`background\`, \`backProps\`, \`ground\`, \`collision\`, \`interactables\`, \`foreground\`, \`lightOccluders\`. Separate visual tile IDs from collision flags.
- Each zone has explicit spawn points and exits with destination scene ID / spawn ID, so transitions never use arbitrary worldX hardcodes.
- A/D move horizontally; E interact; down crouch only behind shelter/occluder and reduces visibility. For slice v1 **do not promise open vertical/top-down movement**; interiors and branches are reached by door/ladder/zone transitions. Later extend to movement lanes or jumping only after tested controls.
- Background parallax is scenery; collision must use static deterministic metadata, not sampling Canvas pixels.
- Target fixed logical simulation; player bounding boxes and interactable radii measured in world pixels, separate from pixel art scale.
- Do not infer gameplay mechanics from image captions; tilemaps are authored/reviewed.

## Suggested authored data contract
\`\`\`ts
type ZoneId = "harbor" | "old_street" | "shadow_alley" |
              "warehouse" | "pier" | "courtyard" | "archive_interior";
type Exit = { id: string; x: number; y: number; width: number; height: number;
              to: ZoneId; spawn: string; requireFlag?: string; discoveredBy?: string; };
type Interactable = { id: string; x: number; y: number; kind: "npc"|"evidence"|"door"|"prop";
                      requireFlag?: string; grantsFlag?: string; };
type Zone = { id: ZoneId; width: number; height: number; tileSize: 16;
              layers: Record<string, number[]>; exits: Exit[];
              spawnPoints: Record<string, {x:number;y:number}>; interactables: Interactable[]; };
\`\`\`

## Chapter I example progression
1. Spawn in Harbor Square; speak to courier. Journal receives \`quest:three-traces\`.
2. Examine shipping ledger at Dock Warehouse OR help observer to learn patrol safe interval. Reaching the old street does not require succeeding a reaction-time test.
3. Talk to archivist on Old Street; solve 1–2 environment interactions to unlock Courtyard.
4. Enter archive via flagged door only after valid evidence dependencies. Optional Pier route reveals an extra cited dossier.
5. Assemble the dossier from three independently found clues. Completing puzzle unlocks cinematic and checkpoint to hub.
6. Map lists discovered paths; normal player mode never sets evidence flags from clicking a node.

## Data authoring & tests
- Author one map file per zone in \`src/data/maps/\` as typed JSON/TS; optional Tiled editor export can be normalized at build time.
- Validate unique zone IDs, reachable main objective, no orphan exits, valid matching spawn points and all required flags attainable.
- Test shortest-path reachability over the zone graph; test a side-route; test save/load and backtracking.
- Show debug overlay with bounds/exits/visibility cone only in local dev mode.
- Maintain 2–3 traversable choices where meaningful; avoid empty 1000px corridors.
