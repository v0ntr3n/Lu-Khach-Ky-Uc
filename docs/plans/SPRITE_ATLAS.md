# Local-first sprite atlas integration plan

Source under evaluation: https://huggingface.co/datasets/Angais/pixel-art-atlas-1.4m
Reference VFX library: https://github.com/patch-gg/pixelfx

**Scope:** LOCAL prototype for this repository. Do not fetch the whole dataset into the game bundle, host a dataset proxy, build a paid asset marketplace or commit raw millions of generated images. Check the dataset card and upstream terms before shipping or redistributing selected images; local/personal scope does not erase third-party conditions.

## Roles: what comes from where?
- **Dataset candidates:** crates, lanterns, books, maps, signs, barrels, foliage and other static props.
- **Original authored assets:** historical characters and NPC turnarounds, animation cycles, architecture silhouettes, dialogue portraits and factual documents. Avoid generic AI sprite substitutions for actual historical figures.
- **PixelFX:** loop/one-shot effects such as rain splash, candle ember, paper reveal and archive seal; inspect effects at native resolution before adopting. Avoid fantasy combat VFX where historically inappropriate.
- **Tilemap:** reusable authored wall/ground/roof tile sets, not arbitrary unaligned individual objects.

## Curate (offline where possible)
1. Search dataset metadata / optional semantic gallery for candidates, using English tags: oil lamp, sealed scroll, crate, chest, rope, ledger, ink bottle, dock, barrel.
2. Export only selected candidates to a temporary local staging folder outside \`public\`.
3. Keep a manifest of \`dataset_name\`, dataset revision, \`row_id\`, prompt/caption, original license claim, selection date, curator notes, target sprite ID and intended scene.
4. Inspect each PNG manually: alpha, silhouette at 1×, grid alignment, malformed edges, duplicates, anachronisms, visual coherence. Reject any questionable provenance/content.
5. Preserve original PNG in provenance-safe local working assets; output a reviewed transformed copy. For 16×16 candidates, do not smooth scale. Extend palette deliberately rather than blindly force all 12 colors.
6. Pack into one/more atlases (e.g. 512×512) plus JSON rectangles, pivot, frame counts, collision metadata and explicit nearest-neighbor policy.
7. Write regression checks: duplicate IDs, rectangle overlap, coordinates within texture bounds, missing assets, alpha border, atlas size. Do not commit a raw 694MB dataset.
8. Put only reviewed small atlases in \`public/assets/atlases/\`. Cache ImageBitmap/HTMLImageElement once, batch render; fall back to procedural SpriteAtlas for missing IDs.

## Example manifest schema
\`\`\`json
{
  "id": "prop_oil_lamp_a",
  "source": "Angais/pixel-art-atlas-1.4m",
  "source_revision": "PIN_EXACT_REVISION",
  "row_id": "PIN_ROW_ID",
  "caption": "oil lamp",
  "usage": ["chapter1:warehouse"],
  "reviewed": false,
  "license_note": "check upstream dataset card and generator terms",
  "atlas": "chapter1_props.png",
  "rect": {"x":0,"y":0,"w":16,"h":16},
  "pivot": {"x":8,"y":15}
}
\`\`\`

## Performance & gameplay rule
The renderer must not create a new \`Image\`/canvas per frame. Use nearest-neighbor only, pixel-perfect pivots and deterministic draw order. Objects need gameplay collision and interaction definitions independent of their sprites. The asset curator **must not silently alter quest coordinates, trigger volumes or map connectivity**. 

## Proposed PR order
1. Curation report with thumbnails/manifest and 50–100 manually accepted static sprites.
2. AssetLoader and atlas renderer integration with cache and tests.
3. Environment pass for harbor/warehouse/old street, then animation/VFX review at native scale.
