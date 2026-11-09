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
| **M1 — Phaser 3 + Tiled Runtime** | P0 | Agent 1, 2, 3, 4 | ✅ Completed | PR #4 (`0e16535`) | Harbor Square (64x48) + Warehouse Interior (36x28), 8-way move, collision, Y-sort, portals; 20/20 Vitest + 9/9 Playwright E2E pass |
| **M2 — World Exploration** | P1 | Agent 1, 2, 3, 4 | ✅ Completed | PR #5 (`02b3b28`) | 8 Chapter I zones in `public/maps/ch1/*.tmj`, `WorldGraph.ts`, `WorldMapPanel.ts` (`[M]`), debug overlay (`[F3]`), `tools/validate_maps.ts` (8/8 PASS); 24/24 Vitest + 12/12 Playwright E2E pass |
| **M3 — RPG Systems & UI** | P1 | Agent 1, 2, 3, 4 | ✅ Completed | PR #6 (`a73dfc0`) | `QuestSystem`, `DialogueSystem`, `EvidenceSystem`, `InventorySystem`, Fullscreen HUD + Ribbon, `JournalPanel` (`[J]`), `InventoryPanel` (`[I]`), `PausePanel` (`[Tab]`), `SaveDataV2`; 28/28 Vitest + 15/15 Playwright E2E pass |
| **M4 — Chapter I Vertical Slice** | P0 | Agent 1, 2, 3, 4 | ✅ Completed | PR #7 (`219a7ca`) | Hương Cảng 1930 complete playthrough, `StealthSystem.ts`, `PuzzlePanel.ts` (`CH1_DOSSIER_PUZZLE`), `src/data/{chapters,quests,npcs,provenance}/`; 30/30 Vitest + 17/17 Playwright E2E pass |
| **M5 — Art, Polish & Accessibility** | P1 | Agent 1, 2, 3, 4 | ✅ Completed | PR #8 (`2f33944`) | Curated sprite atlas (`chapter1_props.png`/`json`), `AssetLoader.ts`, `sprite_atlas_manifest.ts`, 4-dir animations, rain/glow VFX, audio, keyboard `[1..4]`/`[H]`, responsive mobile landscape; 33/33 Vitest + 20/20 Playwright E2E pass |
| **M6 — Chapters II–IV & Final QA** | P1 | Agent 1, 2, 3, 4 | ✅ Completed | `feat/m6-chapters-final-qa` | 14 connected Tiled `.tmj` maps across 4 Eras, distinct Chapter II/III/IV mechanics (`CH2_DISPATCH_NETWORK_PUZZLE`, `CH3_SUPPLY_LOGISTICS_PUZZLE`, `CH4_CAUSAL_POLICY_PUZZLE`), provenance audit (`historical_fact`, `fictional_npc`, `speculative_2045`), `.github/workflows/ci.yml`, 14/14 maps valid, 36/36 Vitest + 23/23 Playwright E2E pass |

