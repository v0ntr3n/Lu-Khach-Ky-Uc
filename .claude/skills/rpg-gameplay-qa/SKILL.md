---
name: rpg-gameplay-qa
description: Audit RPG Exploration game loops, UI modal state, checkpoints, stealth, evidence gating, controls and responsive playtests. Use before every gameplay PR or when asked to find softlocks, bypasses, bug regressions or missing tests.
---
# RPG Gameplay QA — Lữ Khách Ký Ức

1. Inspect real code first: \`src/main.ts\`, \`src/ui/HudController.ts\`, \`src/engine/PixelRenderer.ts\`, \`src/data/chronicles.ts\` and changed files; do not repeat existing checklists without verifying them.
2. Draw a state transition table for Opening, Exploring, Dialogue, Inventory, Map, Puzzle, Cutscene, Paused, Transition. Only Exploring receives movement.
3. Test player collision and stealth: holding crouch in open view should not grant absolute safety; hide behind valid occluders; hazard reactions need cooldown and safe respawn.
4. Test progress exploits: clicking map/sidebar, reopening dialogue, reloading saved data, teleporting in Presenter Mode, wrong answer retries and early climax. Standard mode must require quest flags.
5. Test stale timers and input cancellation: scene switch before pending callback, Esc mid-cinematic, pointercancel, blur, multi-touch, repeat E, reduced motion.
6. For each bug, provide exact file and line, minimal reproduction, expected/actual behavior, severity, proposed fix and a regression test.
7. Run \`npm run build\`, unit tests and browser smoke tests when available. Never mark pass when tool/runtime was not used.
8. Record desktop 320×180 logical viewport render, wide viewport, mobile landscape, overlay z-index and keyboard focus behavior.

Deliverables: prioritized issue table with evidence, test results, and clearly separated observed vs unverified risks. Keep game state logic outside render callbacks.
