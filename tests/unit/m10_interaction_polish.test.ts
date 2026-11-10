import { beforeEach, describe, expect, it } from 'vitest';
import { CHAPTERS } from '../../src/data/chronicles.ts';
import { PixelRenderer } from '../../src/engine/PixelRenderer.ts';
import { getBuiltInMap } from '../../src/runtime/MapLoader.ts';
import { GameHUD } from '../../src/ui/GameHUD.ts';

function createMockCanvas(): HTMLCanvasElement {
  const ctx = {
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
    clearRect: () => {},
    createRadialGradient: () => ({
      addColorStop: () => {}
    })
  } as unknown as CanvasRenderingContext2D;

  return {
    width: 480,
    height: 270,
    getContext: () => ctx
  } as unknown as HTMLCanvasElement;
}

describe('M10: RPG Interaction Polish — NPC Affordances, Feedback & Rewarding Exploration (#15)', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="hud-layer">
        <div id="region-entry-banner" hidden>
          <span id="region-entry-era"></span>
          <strong id="region-entry-title"></strong>
        </div>
        <div id="world-interaction-prompt" hidden>
          <kbd class="prompt-key">E</kbd>
          <span id="world-interaction-label"></span>
        </div>
      </div>
    `;
  });

  it('displays ceremonial region entry banner on zone entry', () => {
    const hud = new GameHUD();
    hud.showRegionEntryBanner('Quảng Trường Bến Cảng', 'CHƯƠNG I · 1930');

    const banner = document.getElementById('region-entry-banner');
    const eraEl = document.getElementById('region-entry-era');
    const titleEl = document.getElementById('region-entry-title');

    expect(banner?.hidden).toBe(false);
    expect(banner?.classList.contains('is-visible')).toBe(true);
    expect(eraEl?.textContent).toBe('CHƯƠNG I · 1930');
    expect(titleEl?.textContent).toBe('Quảng Trường Bến Cảng');
  });

  it('clamps screen-space [E] interaction prompt within safe viewport margins', () => {
    const hud = new GameHUD();
    hud.updateInteractionPrompt('[E] Trò chuyện với Lâm Văn Bính', -120, -40, 1280, 720);

    const promptEl = document.getElementById('world-interaction-prompt')!;
    const labelEl = document.getElementById('world-interaction-label')!;
    expect(promptEl.hidden).toBe(false);
    expect(labelEl.textContent).toBe('Trò chuyện với Lâm Văn Bính');
    expect(parseInt(promptEl.style.left, 10)).toBeGreaterThanOrEqual(32);
    expect(parseInt(promptEl.style.top, 10)).toBeGreaterThanOrEqual(56);

    hud.updateInteractionPrompt('[E] Mở cửa', 2000, 1500, 1280, 720);
    expect(parseInt(promptEl.style.left, 10)).toBeLessThanOrEqual(1280 - 32);
    expect(parseInt(promptEl.style.top, 10)).toBeLessThanOrEqual(720 - 48);
  });

  it('renders overhead quest/NPC state markers, A* route breadcrumbs, and off-screen target beacon cleanly', () => {
    const canvas = createMockCanvas();
    const renderer = new PixelRenderer(canvas);
    const map = getBuiltInMap('harbor_square');

    expect(() =>
      renderer.render({
        chapter: CHAPTERS[0],
        playerX: 160,
        playerY: 384,
        cameraX: 0,
        cameraY: 180,
        facingRight: true,
        playerFacing: 'right',
        isMoving: true,
        isSheltered: false,
        collectedMilestones: new Set(['I']),
        nearbyMilestone: null,
        nearClimaxGate: false,
        climaxBurstProgress: 0,
        timeSec: 2.5,
        mapData: map,
        trackedTargetPos: { x: 920, y: 120 }, // off-screen to trigger edge indicator
        trackedTargetId: 'npc_harbor_liaison',
        activeQuestTargetIds: new Set(['npc_harbor_liaison', 'II']),
        optionalTargetIds: new Set(['npc_dock_worker']),
        routePoints: [
          { x: 160, y: 384 },
          { x: 192, y: 384 },
          { x: 224, y: 384 },
          { x: 256, y: 368 }
        ],
        showHints: true,
        reducedMotion: false
      })
    ).not.toThrow();
  });
});
