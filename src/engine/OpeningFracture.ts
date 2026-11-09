/**
 * @file src/engine/OpeningFracture.ts
 * @description Hoạt cảnh Mở Màn Nứt Vỡ Thời Gian (0.0s -> 4.8s, 5 Pha, 16 Mảnh Đa Giác Răng Cưa)
 * với bộ sinh số giả ngẫu nhiên cố định hạt giống (mulberry32) và hỗ trợ Reduced Motion / Skip.
 */

export type FractureLifecycleState =
  | 'idle'
  | 'initializing'
  | 'cracking'
  | 'propagating'
  | 'tearing'
  | 'revealed'
  | 'disposed';

interface Point2D {
  x: number;
  y: number;
}

interface ShardPolygon {
  points: Point2D[];
  vx: number;
  vy: number;
  rotSpeed: number;
  delay: number;
  cx: number;
  cy: number;
}

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class OpeningFractureController {
  private overlayEl: HTMLElement | null;
  private canvasEl: HTMLCanvasElement | null;
  private ctx: CanvasRenderingContext2D | null = null;
  private state: FractureLifecycleState = 'idle';
  private rafId: number | null = null;
  private startTimeMs = 0;
  private isDisposed = false;
  private onComplete?: () => void;

  private mainCrack: Point2D[] = [];
  private branches: Point2D[][] = [];
  private shards: ShardPolygon[] = [];

  constructor(overlaySelector: string, canvasSelector: string, onComplete?: () => void) {
    this.overlayEl = document.querySelector<HTMLElement>(overlaySelector);
    this.canvasEl = document.querySelector<HTMLCanvasElement>(canvasSelector);
    this.onComplete = onComplete;
  }

  public start(): void {
    if (!this.overlayEl || !this.canvasEl) {
      this.dispose();
      return;
    }

    if (
      typeof window !== 'undefined' &&
      window.matchMedia &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches
    ) {
      this.dispose();
      return;
    }

    this.ctx = this.canvasEl.getContext('2d');
    if (!this.ctx) {
      this.dispose();
      return;
    }

    this.resize();
    this.buildDeterministicGeometry();
    window.addEventListener('resize', this.handleResize);

    const skipBtn = this.overlayEl.querySelector<HTMLButtonElement>('#skip-opening-btn');
    if (skipBtn) {
      skipBtn.addEventListener('click', this.handleSkip);
    }
    window.addEventListener('keydown', this.handleKeyDown);

    this.setState('initializing');
    this.startTimeMs = performance.now();
    this.renderFrame(0);
    this.rafId = requestAnimationFrame(this.tick);
  }

  private readonly handleSkip = (): void => {
    this.dispose();
  };

  private readonly handleKeyDown = (e: KeyboardEvent): void => {
    if (e.key === 'Escape') {
      this.dispose();
    }
  };

  private readonly handleResize = (): void => {
    this.resize();
    this.buildDeterministicGeometry();
  };

  private resize(): void {
    if (!this.canvasEl) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.canvasEl.width = Math.floor(window.innerWidth * dpr);
    this.canvasEl.height = Math.floor(window.innerHeight * dpr);
  }

  private buildDeterministicGeometry(): void {
    if (!this.canvasEl) return;
    const w = this.canvasEl.width;
    const h = this.canvasEl.height;
    const rand = mulberry32(20451930);

    // Vết nứt chính chạy chéo qua tâm màn hình
    this.mainCrack = [];
    const steps = 18;
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const jitterX = (rand() - 0.5) * w * 0.06 * Math.sin(t * Math.PI);
      const jitterY = (rand() - 0.5) * h * 0.05 * Math.sin(t * Math.PI);
      this.mainCrack.push({
        x: w * (0.18 + t * 0.64) + jitterX,
        y: h * (0.12 + t * 0.76) + jitterY,
      });
    }

    // Các nhánh nứt thứ cấp
    this.branches = [];
    for (let b = 0; b < 10; b++) {
      const anchorIdx = 2 + Math.floor(rand() * (steps - 3));
      const anchor = this.mainCrack[anchorIdx];
      const angle = (b % 2 === 0 ? 1 : -1) * (0.45 + rand() * 0.85);
      const branch: Point2D[] = [{ x: anchor.x, y: anchor.y }];
      let currX = anchor.x;
      let currY = anchor.y;
      const segs = 6;
      for (let s = 1; s <= segs; s++) {
        const len = Math.hypot(w, h) * (0.055 + rand() * 0.045);
        currX += Math.cos(angle + (rand() - 0.5) * 0.45) * len;
        currY += Math.sin(angle + (rand() - 0.5) * 0.45) * len;
        branch.push({ x: currX, y: currY });
      }
      this.branches.push(branch);
    }

    // 16 mảnh đa giác răng cưa (4x4 grid jittered)
    this.shards = [];
    const cols = 4;
    const rows = 4;
    const grid: Point2D[][] = [];
    for (let r = 0; r <= rows; r++) {
      const rowPts: Point2D[] = [];
      for (let c = 0; c <= cols; c++) {
        const isEdgeX = c === 0 || c === cols;
        const isEdgeY = r === 0 || r === rows;
        const jx = isEdgeX ? 0 : (rand() - 0.5) * (w / cols) * 0.36;
        const jy = isEdgeY ? 0 : (rand() - 0.5) * (h / rows) * 0.36;
        rowPts.push({
          x: (c / cols) * w + jx,
          y: (r / rows) * h + jy,
        });
      }
      grid.push(rowPts);
    }

    const centerX = w * 0.5;
    const centerY = h * 0.5;
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const pts = [grid[r][c], grid[r][c + 1], grid[r + 1][c + 1], grid[r + 1][c]];
        const cx = (pts[0].x + pts[1].x + pts[2].x + pts[3].x) * 0.25;
        const cy = (pts[0].y + pts[1].y + pts[2].y + pts[3].y) * 0.25;
        const dx = cx - centerX;
        const dy = cy - centerY;
        const dist = Math.max(40, Math.hypot(dx, dy));
        const speed = Math.hypot(w, h) * (0.75 + rand() * 0.55);
        this.shards.push({
          points: pts,
          cx,
          cy,
          vx: (dx / dist) * speed,
          vy: (dy / dist) * speed,
          rotSpeed: (rand() - 0.5) * 1.6,
          delay: (dist / Math.hypot(centerX, centerY)) * 0.22,
        });
      }
    }
  }

  private setState(next: FractureLifecycleState): void {
    if (this.state === next) return;
    this.state = next;
    if (this.overlayEl) {
      this.overlayEl.dataset.state = next;
    }
  }

  private readonly tick = (nowMs: number): void => {
    if (this.isDisposed || !this.ctx || !this.canvasEl) return;

    const elapsedSec = (nowMs - this.startTimeMs) * 0.001;

    if (elapsedSec >= 4.8) {
      this.dispose();
      return;
    }

    if (elapsedSec < 0.6) {
      this.setState('initializing');
    } else if (elapsedSec < 1.5) {
      this.setState('cracking');
    } else if (elapsedSec < 2.9) {
      this.setState('propagating');
    } else if (elapsedSec < 4.4) {
      this.setState('tearing');
    } else {
      this.setState('revealed');
    }

    this.renderFrame(elapsedSec);
    this.rafId = requestAnimationFrame(this.tick);
  };

  private renderFrame(t: number): void {
    if (!this.ctx || !this.canvasEl) return;
    const ctx = this.ctx;
    const w = this.canvasEl.width;
    const h = this.canvasEl.height;

    ctx.clearRect(0, 0, w, h);

    if (t < 0.6) {
      // Phase A — True Black (0.0s – 0.6s): Khung nhìn phủ #000000 tuyệt đối
      ctx.fillStyle = '#000000';
      ctx.fillRect(0, 0, w, h);
    } else if (t < 2.9) {
      // Phase B & C (0.6s – 2.9s): Nền #0D0A08 và vết nứt vàng đồng Sơn Mài (#D4A55D)
      ctx.fillStyle = '#0D0A08';
      ctx.fillRect(0, 0, w, h);

      const crackProgress = Math.min(1, (t - 0.6) / 0.9);
      this.strokePolylinePartial(ctx, this.mainCrack, crackProgress, 3.2);

      if (t >= 1.5) {
        const branchProgress = Math.min(1, (t - 1.5) / 1.4);
        for (const br of this.branches) {
          this.strokePolylinePartial(ctx, br, branchProgress, 1.8);
        }
      }
    } else {
      // Pha D & E: 16 mảnh đa giác văng ra 4 phía để lộ trực tiếp Pixel Canvas bên dưới
      const tearNorm = Math.min(1, (t - 2.9) / 1.5);
      const fadeOut = t >= 4.4 ? Math.max(0, 1 - (t - 4.4) / 0.4) : 1;

      ctx.save();
      ctx.globalAlpha = fadeOut;

      for (const shard of this.shards) {
        const localT = Math.max(0, Math.min(1, (tearNorm - shard.delay) / (1 - shard.delay)));
        const ease = localT * localT * (2.6 - 1.6 * localT);

        ctx.save();
        ctx.translate(shard.cx + shard.vx * ease, shard.cy + shard.vy * ease);
        ctx.rotate(shard.rotSpeed * ease);
        ctx.scale(Math.max(0.1, 1 - ease * 0.28), Math.max(0.1, 1 - ease * 0.28));

        ctx.beginPath();
        shard.points.forEach((pt, idx) => {
          const lx = pt.x - shard.cx;
          const ly = pt.y - shard.cy;
          if (idx === 0) ctx.moveTo(lx, ly);
          else ctx.lineTo(lx, ly);
        });
        ctx.closePath();

        ctx.fillStyle = '#110E0B';
        ctx.fill();
        ctx.lineWidth = 1.5;
        ctx.strokeStyle = 'rgba(212, 165, 93, 0.65)';
        ctx.stroke();
        ctx.restore();
      }

      ctx.restore();
    }
  }

  private strokePolylinePartial(
    ctx: CanvasRenderingContext2D,
    pts: Point2D[],
    progress: number,
    width: number
  ): void {
    if (pts.length < 2 || progress <= 0) return;
    const maxIdx = (pts.length - 1) * progress;
    const wholeIdx = Math.floor(maxIdx);
    const frac = maxIdx - wholeIdx;

    ctx.save();
    ctx.beginPath();
    ctx.moveTo(pts[0].x, pts[0].y);
    for (let i = 1; i <= wholeIdx; i++) {
      ctx.lineTo(pts[i].x, pts[i].y);
    }
    if (wholeIdx < pts.length - 1 && frac > 0) {
      const a = pts[wholeIdx];
      const b = pts[wholeIdx + 1];
      ctx.lineTo(a.x + (b.x - a.x) * frac, a.y + (b.y - a.y) * frac);
    }

    // Đục thủng nền đen để lộ canvas game bên dưới
    ctx.globalCompositeOperation = 'destination-out';
    ctx.lineWidth = width * 2.2;
    ctx.strokeStyle = '#000000';
    ctx.stroke();

    // Vẽ viền sáng vàng đồng Sơn Mài
    ctx.globalCompositeOperation = 'source-over';
    ctx.lineWidth = width;
    ctx.strokeStyle = '#D4A55D';
    ctx.stroke();
    ctx.restore();
  }

  public dispose(): void {
    if (this.isDisposed) return;
    this.isDisposed = true;

    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }

    window.removeEventListener('resize', this.handleResize);
    window.removeEventListener('keydown', this.handleKeyDown);

    this.setState('disposed');
    if (this.overlayEl && this.overlayEl.parentNode) {
      this.overlayEl.parentNode.removeChild(this.overlayEl);
    }
    this.overlayEl = null;
    this.canvasEl = null;
    this.ctx = null;

    this.onComplete?.();
  }
}
