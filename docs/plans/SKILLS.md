# Agent skills included in repository

Native project skills under \`.claude/skills/\`:
- \`rpg-map-design/SKILL.md\`: topology, connected rooms, tilemaps, walkthrough and collision.
- \`pixel-atlas-curation/SKILL.md\`: review Angais 1.4M candidates, manifest, palette, export pipeline.
- \`rpg-gameplay-qa/SKILL.md\`: deterministic gameplay audit and Playwright/user-facing evidence.
- \`pixelfx/SKILL.md\`: upstream Patch PixelFX skill, vendored unchanged under MIT. See accompanying LICENSE. Source: https://github.com/patch-gg/pixelfx/tree/main/skills/pixelfx

Read \`AGENTS.md\` for project rules. These are **instruction skills, not executable code and not additional dependencies**. PixelFX's library is not automatically installed by including its skill; tools must clone/install it separately and import or export only selected effects. External \`pbakaus/impeccable\` can be installed separately for comprehensive UI audits; its full plugin is not vendored here.

Build \`npm run build\` remains the baseline; no runtime code modified by this documentation commit.
