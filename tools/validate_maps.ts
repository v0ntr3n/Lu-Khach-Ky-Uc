/**
 * @file tools/validate_maps.ts
 * @description Công cụ CLI kiểm định toàn bộ 14 bản đồ Tiled (.tmj) của 4 Chương (1930 – 2045)
 * tại `public/maps/ch{1,2,3,4}/` và đồ thị liên thông `CHAPTER1_WORLD_GRAPH` / `ALL_ERA_ZONES`.
 * Chạy trực tiếp bằng: `node --experimental-strip-types tools/validate_maps.ts`
 */

import fs from 'node:fs';
import { register } from 'node:module';
import path from 'node:path';
import process from 'node:process';

const resolveHookCode = `
export async function resolve(specifier, context, nextResolve) {
  try {
    return await nextResolve(specifier, context);
  } catch (err) {
    if (
      err &&
      err.code === 'ERR_MODULE_NOT_FOUND' &&
      (specifier.startsWith('./') || specifier.startsWith('../')) &&
      !specifier.endsWith('.ts') &&
      !specifier.endsWith('.js') &&
      !specifier.endsWith('.json')
    ) {
      return nextResolve(specifier + '.ts', context);
    }
    throw err;
  }
}
`;

try {
  register(
    `data:text/javascript,${encodeURIComponent(resolveHookCode)}`,
    import.meta.url
  );
} catch {
  // Bỏ qua nếu đang chạy trong môi trường bundler hoặc Vitest
}

import type { NormalizedMapData, TiledMapJson } from '../src/types/game.ts';
import type { ValidatableMap } from '../src/world/MapValidator.ts';

export interface CliValidationReport {
  valid: boolean;
  zoneCount: number;
  errors: string[];
  warnings: string[];
}

export async function runMapValidationCli(
  silent = false
): Promise<CliValidationReport> {
  const {
    getAllBuiltInMaps,
    parseTiledMapJson,
    toValidatableMap,
  } = await import('../src/runtime/MapLoader.ts');
  const { validateMapData } = await import('../src/world/MapValidator.ts');
  const { CHAPTER1_WORLD_GRAPH } = await import('../src/world/WorldGraph.ts');

  const chapterDirs = ['ch1', 'ch2', 'ch3', 'ch4'];
  const errors: string[] = [];
  const warnings: string[] = [];

  const parsedFromDisk: Record<string, NormalizedMapData> = {};
  const validatableFromDisk: Record<string, ValidatableMap> = {};

  for (const ch of chapterDirs) {
    const mapsDir = path.resolve(process.cwd(), 'public/maps', ch);
    if (!fs.existsSync(mapsDir)) {
      continue;
    }

    const tmjFiles = fs
      .readdirSync(mapsDir)
      .filter((f) => f.endsWith('.tmj'))
      .sort();

    for (const file of tmjFiles) {
      const id = file.replace(/\.tmj$/, '');
      const fullPath = path.join(mapsDir, file);
      const raw = JSON.parse(fs.readFileSync(fullPath, 'utf-8')) as TiledMapJson;
      const norm = parseTiledMapJson(id, raw);
      parsedFromDisk[norm.id] = norm;
      validatableFromDisk[norm.id] = toValidatableMap(norm);
    }
  }

  // Đối chiếu với getAllBuiltInMaps()
  const builtInMaps = getAllBuiltInMaps();
  for (const [id, builtIn] of Object.entries(builtInMaps)) {
    if (!parsedFromDisk[id]) {
      errors.push(
        `Built-in map "${id}" is missing a corresponding .tmj file in public/maps/ch{1,2,3,4}/.`
      );
    } else if (
      parsedFromDisk[id].widthPx !== builtIn.widthPx ||
      parsedFromDisk[id].heightPx !== builtIn.heightPx
    ) {
      errors.push(
        `Dimension mismatch between disk .tmj and built-in map for "${id}".`
      );
    }
  }

  const tableRows: Array<{
    eraId: string;
    zoneId: string;
    dimensions: string;
    spawns: number;
    portals: number;
    interactables: number;
    status: string;
  }> = [];

  for (const [id, vMap] of Object.entries(validatableFromDisk)) {
    const res = validateMapData(vMap, validatableFromDisk);
    if (!res.valid) {
      errors.push(...res.errors);
    }
    warnings.push(...res.warnings);

    const norm = parsedFromDisk[id];
    tableRows.push({
      eraId: norm.eraId,
      zoneId: id,
      dimensions: `${norm.widthInTiles}x${norm.heightInTiles} (${norm.widthPx}x${norm.heightPx}px)`,
      spawns: norm.spawns.length,
      portals: norm.portals.length,
      interactables: norm.interactables.length,
      status: res.valid ? 'PASS' : `FAIL (${res.errors.length})`,
    });
  }

  const { QuestSystem } = await import('../src/systems/QuestSystem.ts');
  const questTargetRes = new QuestSystem().validateAllObjectiveTargets(parsedFromDisk);
  if (!questTargetRes.valid) {
    errors.push(...questTargetRes.errors);
  }

  const graphRes = CHAPTER1_WORLD_GRAPH.validateWorldGraph(parsedFromDisk);
  if (!graphRes.valid) {
    errors.push(...graphRes.errors);
  }

  if (!silent) {
    console.log(
      '\n=== LỮ KHÁCH KÝ ỨC — CHAPTERS I–IV TILED MAP & WORLD GRAPH VALIDATOR ==='
    );
    console.table(tableRows);
    console.log(
      `WorldGraph (${tableRows.length} Zones Across 4 Eras) Integrity: ${graphRes.valid ? 'PASS' : 'FAIL'}`
    );
    console.log(
      `QuestTargetRef Integrity (15 Objectives Across Chapters I–IV): ${questTargetRes.valid ? 'PASS' : 'FAIL'}`
    );
    if (errors.length > 0) {
      console.error('\nValidation Errors:');
      for (const err of errors) {
        console.error(`  - ${err}`);
      }
    } else {
      console.log(
        `\n✓ All ${tableRows.length} Tiled maps, WorldGraph routes, and QuestTargetRef links validated with 0 errors.`
      );
    }
  }

  return {
    valid: errors.length === 0,
    zoneCount: tableRows.length,
    errors,
    warnings,
  };
}

const isMainModule =
  typeof process !== 'undefined' &&
  Array.isArray(process.argv) &&
  process.argv[1] &&
  path.resolve(process.argv[1]) ===
    path.resolve(new URL(import.meta.url).pathname);

if (isMainModule) {
  runMapValidationCli(false)
    .then((report) => {
      process.exit(report.valid ? 0 : 1);
    })
    .catch((err) => {
      console.error(err);
      process.exit(1);
    });
}
