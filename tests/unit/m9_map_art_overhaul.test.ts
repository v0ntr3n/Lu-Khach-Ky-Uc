import { describe, expect, it } from 'vitest';
import { getAllBuiltInMaps } from '../../src/runtime/MapLoader.ts';
import { ZONE_LANDMARKS, drawZoneLandmarks, drawTopDownTile } from '../../src/engine/SpriteAtlas.ts';

function createMockCtx(): CanvasRenderingContext2D {
  return {
    fillStyle: '#000000',
    strokeStyle: '#000000',
    lineWidth: 1,
    globalAlpha: 1,
    imageSmoothingEnabled: false,
    fillRect: () => {},
    strokeRect: () => {},
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    arc: () => {},
    fill: () => {},
    stroke: () => {},
    save: () => {},
    restore: () => {},
    translate: () => {},
    scale: () => {},
    clearRect: () => {}
  } as unknown as CanvasRenderingContext2D;
}

describe('M9: Era-Specific Map Art Overhaul (#14)', () => {
  const allMaps = getAllBuiltInMaps();
  const mapIds = Object.keys(allMaps);

  it('defines 14 built-in maps across Chapters I-IV', () => {
    expect(mapIds).toHaveLength(14);
  });

  it('populates Ground, GroundDetails, LowProps, Structures, and Foreground across all 14 maps', () => {
    for (const [id, map] of Object.entries(allMaps)) {
      const groundNonZero = map.tileLayers.Ground.filter((t) => t > 0).length;
      const detailsNonZero = map.tileLayers.GroundDetails.filter((t) => t > 0).length;
      const lowPropsNonZero = map.tileLayers.LowProps.filter((t) => t > 0).length;
      const structuresNonZero = map.tileLayers.Structures.filter((t) => t > 0).length;
      const foregroundNonZero = map.tileLayers.Foreground.filter((t) => t > 0).length;

      expect(groundNonZero, `Ground should be populated in ${id}`).toBeGreaterThan(0);
      expect(detailsNonZero, `GroundDetails should be populated in ${id}`).toBeGreaterThan(0);
      expect(lowPropsNonZero, `LowProps should be populated in ${id}`).toBeGreaterThan(0);
      expect(structuresNonZero, `Structures should be populated in ${id}`).toBeGreaterThan(0);
      expect(foregroundNonZero, `Foreground should be populated in ${id}`).toBeGreaterThan(0);
    }
  });

  it('defines at least 2 historical focal landmarks per zone across all 14 zones', () => {
    for (const id of mapIds) {
      const landmarks = ZONE_LANDMARKS[id];
      expect(landmarks, `Missing ZONE_LANDMARKS entry for ${id}`).toBeDefined();
      expect(landmarks.length, `Expected >= 2 landmarks for ${id}`).toBeGreaterThanOrEqual(2);
    }
  });

  it('renders zone landmarks and era-specific tiles cleanly in both normal and reducedMotion modes', () => {
    const ctx = createMockCtx();
    for (const map of Object.values(allMaps)) {
      expect(() => drawZoneLandmarks(ctx, map, 0, 0, 1.5, false)).not.toThrow();
      expect(() => drawZoneLandmarks(ctx, map, 0, 0, 1.5, true)).not.toThrow();
    }

    const tileIds = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 31];
    const eras = ['1930', '1945', '1954_1975', '1986_2045'];
    for (const era of eras) {
      for (const tileId of tileIds) {
        expect(() => drawTopDownTile(ctx, tileId, 32, 48, false, era, 2.25, false)).not.toThrow();
        expect(() => drawTopDownTile(ctx, tileId, 32, 48, true, era, 2.25, true)).not.toThrow();
      }
    }
  });
});
