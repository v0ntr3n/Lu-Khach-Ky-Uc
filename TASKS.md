# TASKS.md — Backlog, Ownership & Milestone Tracking

**Repository:** `v0ntr3n/Lu-Khach-Ky-Uc`  
**Base Branch:** `main` (Baseline: `cc1e256`)

## Agent Team & Ownership

| Agent | Role | Owned Paths |
| :--- | :--- | :--- |
| **Lead Agent** | Lead Game Engineer + Technical Director | `src/types/`, `package.json`, `TASKS.md`, `docs/implementation/`, PR Integration |
| **Agent 1** (`runtime_world_engineer`) | Runtime & World Engineer | `src/runtime/`, `src/scenes/`, `src/world/`, `public/maps/`, `tools/validate_maps.ts` |
| **Agent 2** (`gameplay_systems_engineer`) | Gameplay & Systems Engineer | `src/core/`, `src/systems/`, `src/data/`, `src/main.ts` |
| **Agent 3** (`ui_art_engineer`) | UI/UX & Art Engineer | `src/ui/`, `src/engine/`, `index.html`, `public/tilesets/`, `public/assets/` |
| **Agent 4** (`qa_integration_engineer`) | QA, Integration & Performance Engineer | `tests/unit/`, `tests/maps/`, `tests/playwright/`, `.github/workflows/` |

---

## Milestone Backlog & Status

| Milestone | Priority | Owner | Status | Branch / PR | Verification Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **M0 — Gameplay Stabilization** | P0 | Agent 1, 2, 3, 4 | ✅ Completed | PR #3 (`76a4313`) | 8 P0 bugs fixed; `npm run build` pass; 12/12 Vitest unit tests + 5/5 Playwright E2E tests pass |
| **M1 — Phaser 3 + Tiled Runtime** | P0 | Agent 1, 2, 3, 4 | ✅ Completed | `feat/m1-phaser-tiled-runtime` | Harbor Square (64x48) + Warehouse Interior (36x28), 8-way move, collision, Y-sort, portals; 20/20 Vitest + 9/9 Playwright E2E pass |
| **M2 — World Exploration** | P1 | Agent 1, 2, 3, 4 | 🔄 Next | `feat/m2-world-exploration` | WorldGraph, connected zones, NPC routes, debug overlay, map validator CLI |
| **M3 — RPG Systems & UI** | P1 | Agent 1, 2, 3, 4 | ⏳ Planned | `feat/m3-rpg-systems-ui` | Quests, Dialogue choices, Inventory/Evidence, Journal, Map, Pause, SaveV2 |
| **M4 — Chapter I Vertical Slice** | P0 | Agent 1, 2, 3, 4 | ⏳ Planned | `feat/m4-chapter1-vertical-slice` | Hương Cảng 1930 complete playthrough, 3 NPCs, 3 evidence, side quest, stealth, dossier puzzle |
| **M5 — Art, Polish & Accessibility** | P1 | Agent 1, 2, 3, 4 | ⏳ Planned | `feat/m5-art-polish-accessibility` | Curated sprite atlas, 4-dir animations, rain/glow VFX, audio, reduced motion, hints |
| **M6 — Chapters II–IV & Final QA** | P1 | Agent 1, 2, 3, 4 | ⏳ Planned | `feat/m6-chapters-final-qa` | Chapters II (1945), III (1954-1975), IV (1986-2045) distinct mechanics, CI, final QA |
