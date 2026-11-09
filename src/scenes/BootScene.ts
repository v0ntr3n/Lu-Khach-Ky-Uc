/**
 * @file src/scenes/BootScene.ts
 * @description Scene khởi tạo tài nguyên ban đầu (Phaser 3 BootScene) cho Lữ Khách Ký Ức (M1).
 * Nạp dữ liệu bản đồ Tiled tích hợp sẵn (`harbor_square`, `warehouse_interior`), khởi tạo
 * bảng màu / texture cơ sở và chuyển sang `HarborScene`.
 */

import { getAllBuiltInMaps } from '../runtime/MapLoader';
import {
  BaseTopDownScene,
  computeEntityDepth,
  LAYER_DEPTHS,
} from '../runtime/PhaserRuntime';

export { computeEntityDepth, LAYER_DEPTHS };

export class BootScene extends BaseTopDownScene {
  public static readonly KEY = 'BootScene';

  constructor() {
    super({ key: BootScene.KEY });
  }

  public override preload(): void {
    const maps = getAllBuiltInMaps();
    this.registry.set('maps', maps);
  }

  public override create(): void {
    this.scene.start('HarborScene', { spawnId: 'default_spawn' });
  }
}
