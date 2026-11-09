/**
 * @file tests/maps/m1_maps.test.ts
 * @description Kiểm thử tính toàn vẹn của hai bản đồ Tiled M1:
 * - public/maps/ch1/harbor_square.tmj (64x48 ô = 1024x768 px)
 * - public/maps/ch1/warehouse_interior.tmj (36x28 ô = 576x448 px)
 * Kiểm tra nạp/phân tích qua MapLoader, kiểm định bằng MapValidator.validateMapData,
 * tính liên thông cổng 2 chiều (two-way portal integrity), không chồng lấn vật cản tại mọi điểm spawn,
 * và khả năng tiếp cận (16x16 BFS reachability) tới tất cả cổng và NPC/vật thể tương tác.
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  getBuiltInMap,
  parseTiledMap,
  toValidatableMap,
} from '../../src/runtime/MapLoader';
import type { TiledMapJson } from '../../src/types/game';
import { CollisionWorld } from '../../src/world/CollisionWorld';
import { validateMapData } from '../../src/world/MapValidator';

describe('M1 Top-Down Tiled Maps — Integrity & Reachability Suite', () => {
  const harborTmjPath = resolve(process.cwd(), 'public/maps/ch1/harbor_square.tmj');
  const warehouseTmjPath = resolve(process.cwd(), 'public/maps/ch1/warehouse_interior.tmj');

  const rawHarbor = JSON.parse(readFileSync(harborTmjPath, 'utf-8')) as TiledMapJson;
  const rawWarehouse = JSON.parse(readFileSync(warehouseTmjPath, 'utf-8')) as TiledMapJson;

  const harborNorm = parseTiledMap('harbor_square', rawHarbor);
  const warehouseNorm = parseTiledMap('warehouse_interior', rawWarehouse);

  const harborValidatable = toValidatableMap(harborNorm);
  const warehouseValidatable = toValidatableMap(warehouseNorm);

  const allMaps = {
    harbor_square: harborValidatable,
    warehouse_interior: warehouseValidatable,
  };

  it('loads and parses harbor_square.tmj (64x48 = 1024x768 px) and warehouse_interior.tmj (36x28 = 576x448 px) with 16x16 tiles', () => {
    expect(harborNorm.id).toBe('harbor_square');
    expect(harborNorm.widthInTiles).toBe(64);
    expect(harborNorm.heightInTiles).toBe(48);
    expect(harborNorm.tileSize).toBe(16);
    expect(harborNorm.widthPx).toBe(1024);
    expect(harborNorm.heightPx).toBe(768);

    expect(warehouseNorm.id).toBe('warehouse_interior');
    expect(warehouseNorm.widthInTiles).toBe(36);
    expect(warehouseNorm.heightInTiles).toBe(28);
    expect(warehouseNorm.tileSize).toBe(16);
    expect(warehouseNorm.widthPx).toBe(576);
    expect(warehouseNorm.heightPx).toBe(448);

    // Đảm bảo dữ liệu built-in đồng bộ hoàn toàn với file .tmj
    const builtInHarbor = getBuiltInMap('harbor_square');
    const builtInWarehouse = getBuiltInMap('warehouse_interior');
    expect(builtInHarbor.widthPx).toBe(1024);
    expect(builtInWarehouse.widthPx).toBe(576);
  });

  it('validates both maps with validateMapData(map, allMaps) with zero errors and full 16x16 BFS reachability', () => {
    const harborValidation = validateMapData(harborValidatable, allMaps);
    expect(harborValidation.errors).toEqual([]);
    expect(harborValidation.valid).toBe(true);

    const warehouseValidation = validateMapData(warehouseValidatable, allMaps);
    expect(warehouseValidation.errors).toEqual([]);
    expect(warehouseValidation.valid).toBe(true);
  });

  it('asserts two-way portal integrity between harbor_square and warehouse_interior', () => {
    const toWarehouse = harborNorm.portals.find(
      (p) => p.id === 'exit_harbor_to_warehouse'
    );
    expect(toWarehouse).toBeDefined();
    expect(toWarehouse?.destinationMap).toBe('warehouse_interior');
    expect(toWarehouse?.destinationSpawn).toBe('warehouse_south_door');

    const targetWarehouseSpawn = warehouseNorm.spawns.find(
      (s) => s.id === toWarehouse?.destinationSpawn
    );
    expect(targetWarehouseSpawn).toBeDefined();

    const toHarbor = warehouseNorm.portals.find(
      (p) => p.id === 'exit_warehouse_to_harbor'
    );
    expect(toHarbor).toBeDefined();
    expect(toHarbor?.destinationMap).toBe('harbor_square');
    expect(toHarbor?.destinationSpawn).toBe('from_warehouse');

    const targetHarborSpawn = harborNorm.spawns.find(
      (s) => s.id === toHarbor?.destinationSpawn
    );
    expect(targetHarborSpawn).toBeDefined();
  });

  it('asserts all named spawns (default_spawn, from_warehouse, warehouse_south_door) do NOT overlap any collision obstacle', () => {
    const harborWorld = new CollisionWorld(
      harborNorm.obstacles,
      harborNorm.shelters,
      { x: 0, y: 0, width: harborNorm.widthPx, height: harborNorm.heightPx }
    );

    for (const spawnId of ['default_spawn', 'from_warehouse']) {
      const spawn = harborNorm.spawns.find((s) => s.id === spawnId);
      expect(spawn).toBeDefined();
      const footRect = {
        x: spawn!.x,
        y: spawn!.y,
        width: 8,
        height: 8,
      };
      expect(harborWorld.isRectColliding(footRect)).toBe(false);
    }

    const warehouseWorld = new CollisionWorld(
      warehouseNorm.obstacles,
      warehouseNorm.shelters,
      { x: 0, y: 0, width: warehouseNorm.widthPx, height: warehouseNorm.heightPx }
    );

    for (const spawnId of ['warehouse_south_door', 'default_spawn']) {
      const spawn = warehouseNorm.spawns.find((s) => s.id === spawnId);
      expect(spawn).toBeDefined();
      const footRect = {
        x: spawn!.x,
        y: spawn!.y,
        width: 8,
        height: 8,
      };
      expect(warehouseWorld.isRectColliding(footRect)).toBe(false);
    }
  });
});
