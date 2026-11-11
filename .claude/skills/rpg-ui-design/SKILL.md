---
name: rpg-ui-design
description: Design, audit, and refine HUD, modal, journal, dialogue, and responsive UI surfaces for Lữ Khách Ký Ức using the Sơn Mài & Giấy Dó token system, Vietnamese typography floor, and viewport overlay anti-collision rules. Use before modifying CSS, HTML HUD/modals, or src/ui/* components.
---
# RPG UI Design — Lữ Khách Ký Ức (Sơn Mài & Giấy Dó)

Apply these project-specific UI/UX rules whenever designing, editing, or reviewing HUD panels, modals (`Nhật Ký`, `Túi Tư Liệu`, `Sơ Đồ Khu Vực`, `Hướng Dẫn`, `Cài Đặt`), `DialoguePanel`, `MiniMapPanel`, or stage overlays in `index.html` and `src/ui/*`.

## 1. Material Direction & Semantic Token Discipline (Light / Dark / System)

1. **Dual-Material Aesthetic**:
   - **Dark (`[data-theme='dark']`, Sơn Mài Đêm)**: deep lacquer surfaces (`--surface-primary`, `--surface-hud`, `--surface-elevated`), antique/bright bronze borders (`--border-default`, `--border-strong`), gilded accents (`--text-accent`), and warm parchment ink (`--text-primary`).
   - **Light (`[data-theme='light']`, Giấy Dó Ngày)**: archival *giấy dó* surfaces (`--surface-primary`, `--surface-hud`, `--surface-inset`), vermilion seals (`--accent-primary`, `--text-crimson`), deep ink typography (`--text-primary`, `--text-secondary`), and bronze rules (`--border-default`, `--border-strong`).
2. **Zero Hardcoded Component Colors**:
   - Never introduce raw `#hex` or `rgb()`/`rgba()` literals inside component CSS rules or runtime-injected stylesheets (`tests/unit/m13_journal_theme_system.test.ts` enforces zero hardcoded colors outside `:root` and `[data-theme]` token blocks).
   - Always consume centralized semantic variables (`var(--bg-canvas)`, `var(--bg-page)`, `var(--bg-overlay)`, `var(--surface-primary)`, `var(--surface-hud)`, `var(--surface-elevated)`, `var(--surface-inset)`, `var(--surface-hover)`, `var(--border-subtle)`, `var(--border-default)`, `var(--border-strong)`, `var(--text-primary)`, `var(--text-secondary)`, `var(--text-muted)`, `var(--text-accent)`, `var(--text-crimson)`, `var(--text-on-accent)`, `var(--accent-primary)`, `var(--shadow-panel)`, `var(--shadow-hud)`, `var(--shadow-toast)`, `var(--shadow-dialogue)`).
3. **Theme Geometry Parity & Canvas Isolation**:
   - Switching between `system`, `light`, and `dark` (`ThemeManager`) must never change element dimensions (`padding`, `border-width`, `font-size`, `width`, `height`).
   - Protect pixel-art canvases (`#pixel-canvas`, `#main-menu-bg-canvas`, `#dialogue-portrait-canvas` with `filter: none !important`) so UI theme tokens never tint or dim world rendering.
   - Maintain WCAG AA contrast (`>= 4.5:1` on text and interactive controls) in both Light and Dark modes.

## 2. Vietnamese Typography & Legibility Floor

1. **Vietnamese-Safe Font Hierarchy**:
   - `--font-display` / `--font-serif` (`'Cormorant Garamond'`, `'Newsreader'`) for chapter headers, speaker nameplates, literary dialogue body, and historical quotes.
   - `--font-ui` (`'Be Vietnam Pro'`) for HUD plaques, tabs, buttons, objective trackers, and instructions; `--font-mono` (`'JetBrains Mono'`) only for keycaps (`[E]`, `[J]`, `[M]`), Roman numerals, and compact step counters. Never use bitmap fonts that clip stacked Vietnamese diacritics (`ấ`, `ệ`, `ử`, `ữ`).
2. **Minimum Size & Case Rules**:
   - Never set `font-size` below `0.8125rem` (`13px`) on visible HUD/UI text (`M14-UT-06` regression gate). Keep interactive modal buttons (`#btn-close-dialogue`, `.ctrl-btn`, `.rpg-tab-btn`) at `>= 0.875rem–1rem` with `line-height >= 1.25`.
   - Avoid `text-transform: uppercase` on Vietnamese HUD labels and prose so tone marks remain scannable at a glance.

## 3. Canvas-First HUD & Progressive Disclosure

1. **85%+ Unobstructed Gameplay Viewport**:
   - During `exploring` mode, keep persistent chrome minimal: top-left `#hud-left-stack` (`#minimap-panel` + `#hud-objective-tracker`), top-center `#stage-location-badge`, and top-right utility triggers.
   - `#minimap-panel:not(.is-collapsed)` and `#hud-objective-tracker` must maintain identical `left` and `width` (`<= 1px` tolerance), with `#hud-objective-tracker` single-line (`<= 44px` tall).
2. **Eliminate Redundant Chrome Titles**:
   - `#stage-location-badge` displays the full location + historical year (e.g. `Quảng Trường Bến Cảng Hương Cảng · 1930`).
   - `#minimap-zone-title` uses `formatCompactMinimapTitle()` to render the concise landmark name (e.g. `Quảng Trường Bến Cảng`) with `title` tooltip storing the full formal name—never clip mid-word behind a stale narrow `max-width`.
3. **Single-Row Desktop Coach Bar & Concise Copy**:
   - On desktop viewports (`>= 960px` wide and `> 500px` tall), `#tutorial-coach-bar.is-open` must collapse into a single horizontal row (`<= 52px` tall) so southern harbor/courtyard footpaths stay unobstructed.
   - Keep button and tab labels concise (`1–3` words + `[Key]`), placing deeper historical provenance inside expandable drawers (`Tư liệu [T]`) or 2-column Journal/Codex inspectors.

## 4. Viewport Overlay Anti-Collision Matrix

Every transient or modal surface must obey spatial exclusion so two overlays never collide at the same viewport anchor:

| Active State / Surface | Primary Anchor | Required Reposition / Suppression Rule |
| :--- | :--- | :--- |
| Default `exploring` | `#stage-toast` at `bottom: 18px; left: 18px` | Clear of top-left `#hud-left-stack` and top-center `#stage-location-badge`. |
| `body.is-tutorial-bar-open` | `#tutorial-coach-bar` at `bottom: 12px–18px; left: 50%` | Lift `#stage-toast` above coach bar (`bottom: 66px` single-row desktop / `bottom: 112px` stacked layout). |
| `body.is-dialogue-open` (`#dialogue-modal.is-open`) | `#dialogue-modal` at `bottom: 18px; left: 50%` | Relocate `#stage-toast` to top-center (`top: 48px; left: 50%; bottom: auto`) below `#stage-location-badge` so NPC portraits, speaker nameplates, and choices are never covered. |
| `body.is-touch-controls` (mobile / coarse pointer) | `#touch-controls` at bottom corners | Keep `#stage-toast` at top-center (`top: 44px–48px; left: 50%`) clear of virtual D-pad and action buttons. |
| Modal Dialogs (`#journal-modal`, `#instructions-modal`, `#settings-modal`, `#world-map-modal`) | Centered `.rpg-modal-dialog` (`z-index: 300+`) | Block world movement inputs; trap `Escape` to close topmost modal; style `.rpg-tab-btn` and `[data-guide-tab]` with `display: inline-flex; align-items: center; justify-content: center; min-height: 42px` for reliable pointer/touch hitboxes. |

## 5. Pre-PR UI Verification Checklist

1. Run `npm run validate:maps`, `npm run build`, `npm run test:unit`, and `npm run test:e2e`.
2. Confirm `normalizeDocumentTitle()` keeps `document.title` aligned to `Lữ Khách Ký Ức (1930 – 2045) — Hành Trình Lịch Sử Sơn Mài & Giấy Dó` across static and Worker shells.
3. Audit `1920×1080`, `1280×720`, and `844×390` mobile landscape in both Light and Dark modes for ellipsis truncation, diacritic clipping, tab hitbox alignment, and `#stage-toast` vs `#dialogue-modal` separation.
