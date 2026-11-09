# Agent workflow — Lữ Khách Ký Ức

Repository work is local-first TypeScript/Vite/Canvas 2D. Read \`docs/plans/WORKADVENTURE_TOPDOWN.md\` first for active map direction, then \`docs/plans/RPG_EXPLORATION.md\`, \`docs/plans/WORLD_MAP.md\` (archived 1D design) and \`docs/plans/SPRITE_ATLAS.md\` before gameplay, map or art changes.

Project skills:
- \`.claude/skills/rpg-map-design/SKILL.md\` for world graphs and map authoring.
- \`.claude/skills/pixel-atlas-curation/SKILL.md\` for dataset sprite selection and atlases.
- \`.claude/skills/rpg-gameplay-qa/SKILL.md\` for controls, quest logic, tests, browser audits.
- \`.claude/skills/pixelfx/SKILL.md\` for VFX (vendored upstream, MIT).

Rules:
1. The target map is top-down XY movement with Tiled-authored connected scenes; WorkAdventure is reference only, not an online runtime dependency. No direct player milestone unlocking from travel, map clicks or ordinary teleport. Presenter mode only with explicit isolation.
2. Keep historical facts cited and distinguish fiction/2045 aspirations.
3. UI overlays must block world inputs; stale async timers cannot change current scene.
4. Quest/evidence logic is separate from sprite rendering; do not treat generated sprite appearance as collision.
5. Add test coverage and run \`npm run build\` for code changes; never claim runtime checks without executing them.
6. Do not bulk-download the dataset into the repo or check in unreviewed assets.
7. One bounded PR per concern; fix P0 game-state bugs before VFX polish.

This is a planning and skill-index commit; it does not implement the proposed engine changes.
