---
name: pixel-atlas-curation
description: Curate small reviewed pixel art sprite atlases from Angais/pixel-art-atlas-1.4m for the local TypeScript RPG. Use for static props, environment dressings, asset manifest, palette normalization and sprite sheet export.
---
# Pixel Atlas Curator — Lữ Khách Ký Ức

1. Read \`docs/plans/SPRITE_ATLAS.md\` and the current dataset card: https://huggingface.co/datasets/Angais/pixel-art-atlas-1.4m .
2. Search by subject/caption and preview candidates. Prefer prop/foliage/objects, not historically significant persons or animation frames. Never assume metadata is ground truth.
3. Choose a limited batch (e.g. 50–100); for each record exact dataset revision, row ID, original caption, scene, review decision and source/license notes. Avoid direct bulk dataset import.
4. Reject malformed alpha, misleading contents, illegible 1× silhouettes, inconsistent pixel grid, repeating duplicates, out-of-period objects.
5. Preserve source colors where useful; carefully adapt to warm lacquer/paper aesthetic and maintain readability. Compare at native 1× and actual game 2×/3× nearest-neighbor.
6. Output a packed transparent PNG + manifest JSON (rect, pivot, ID, hitbox reference, license/source record). Generate checksum and ensure atlas rectangle non-overlap with tests.
7. Never mutate gameplay flags/collision automatically from images. Coordinate with map-designer skill to attach entities separately.
8. In code changes, instantiate/cached atlas once and test fallback for missing assets. Provide pre/post screenshots and measured performance.

Deliverables: small reviewed candidate list, provenance manifest, optimized atlas and asset loading verification. Distinguish prototype-local use from later redistribution.
