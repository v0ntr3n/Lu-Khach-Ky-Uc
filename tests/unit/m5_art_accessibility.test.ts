/**
 * @file tests/unit/m5_art_accessibility.test.ts
 * @description Bộ kiểm thử đơn vị (Vitest) cho Cột mốc M5 — Art, Gameplay Polish & Accessibility:
 * 1. Curated Sprite Atlas & Provenance Manifest (CURATED_SPRITE_ATLAS_MANIFEST, validateSpriteAtlasManifest,
 *    chapter1_props.json, chapter1_props.png, validateAtlasRects, và nguyên tắc Original Pixel Art cho nhân vật lịch sử).
 * 2. Hoạt ảnh 4 hướng & Trợ năng Giảm chuyển động (computeAnimationFrame4Dir).
 * 3. Bộ tổng hợp âm thanh Ngũ Cung 8-Bit (SoundSynth: playStep, playSealStamp, playClimaxFanfare, playAlertPushback,
 *    playDialogueBlip, playPortalTransition, playQuestComplete).
 */

import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  CURATED_SPRITE_ATLAS_MANIFEST,
  validateSpriteAtlasManifest,
} from '../../src/data/provenance/sprite_atlas_manifest';
import { SoundSynth } from '../../src/engine/SoundSynth';
import {
  ASSET_LOADER,
  type AtlasDescriptor,
  validateAtlasRects,
} from '../../src/runtime/AssetLoader';
import { computeAnimationFrame4Dir } from '../../src/runtime/PhaserRuntime';

describe('M5 Art, Atlas Curation & Accessibility — Unit Test Suite', () => {
  describe('1. Curated Sprite Atlas & Provenance Manifest', () => {
    it('validates CURATED_SPRITE_ATLAS_MANIFEST (>= 16 entries, 0 overlaps, Original Pixel Art for characters) and on-disk chapter1_props.json / .png', () => {
      expect(CURATED_SPRITE_ATLAS_MANIFEST.length).toBeGreaterThanOrEqual(16);

      const manifestCheck = validateSpriteAtlasManifest(
        CURATED_SPRITE_ATLAS_MANIFEST,
        128,
        128
      );
      expect(manifestCheck.errors).toEqual([]);
      expect(manifestCheck.valid).toBe(true);

      // Các nhân vật lịch sử / NPC 4 hướng bắt buộc dùng 'Lu-Khach-Ky-Uc Original Pixel Art', không dùng AI đại trà
      const charEntries = CURATED_SPRITE_ATLAS_MANIFEST.filter((e) =>
        e.id.startsWith('char_')
      );
      expect(charEntries).toHaveLength(4);
      for (const c of charEntries) {
        expect(c.source).toBe('Lu-Khach-Ky-Uc Original Pixel Art');
        expect(c.reviewed).toBe(true);
      }

      // Kiểm tra tệp thực tế trên đĩa: public/assets/atlases/chapter1_props.json & .png
      const jsonPath = path.resolve(
        process.cwd(),
        'public/assets/atlases/chapter1_props.json'
      );
      const pngPath = path.resolve(
        process.cwd(),
        'public/assets/atlases/chapter1_props.png'
      );

      expect(fs.existsSync(jsonPath)).toBe(true);
      expect(fs.existsSync(pngPath)).toBe(true);
      expect(fs.statSync(pngPath).size).toBeGreaterThan(64);

      const rawJson = JSON.parse(
        fs.readFileSync(jsonPath, 'utf-8')
      ) as AtlasDescriptor;
      expect(rawJson.meta.filter).toBe('NEAREST');
      expect(rawJson.meta.size).toEqual({ w: 128, h: 128 });

      const rectValidation = validateAtlasRects(rawJson);
      expect(rectValidation.errors).toEqual([]);
      expect(rectValidation.valid).toBe(true);
      expect(rectValidation.frameCount).toBeGreaterThanOrEqual(16);

      // Kiểm tra bộ đệm Singleton của AssetLoader không khởi tạo lại texture ở mỗi frame
      const tex1 = ASSET_LOADER.getOrCreateTexture('chapter1_props');
      const countAfterFirst = ASSET_LOADER.imageInstantiationCount;
      const tex2 = ASSET_LOADER.getOrCreateTexture('chapter1_props');
      expect(tex2).toBe(tex1);
      expect(ASSET_LOADER.imageInstantiationCount).toBe(countAfterFirst);
    });
  });

  describe('2. 4-Direction Animation & Reduced Motion (computeAnimationFrame4Dir)', () => {
    it('maps down/up/left/right to distinct direction rows, advances frames when moving, and locks frameIndex=0 & bobOffsetPx=0 when reducedMotion is enabled', () => {
      const downIdle = computeAnimationFrame4Dir('down', false, 0.5, false);
      const upIdle = computeAnimationFrame4Dir('up', false, 0.5, false);
      const leftIdle = computeAnimationFrame4Dir('left', false, 0.5, false);
      const rightIdle = computeAnimationFrame4Dir('right', false, 0.5, false);

      expect(
        new Set([
          downIdle.directionIndex,
          upIdle.directionIndex,
          leftIdle.directionIndex,
          rightIdle.directionIndex,
        ]).size
      ).toBe(4);

      // Khi di chuyển bình thường (reducedMotion = false), frameIndex thay đổi theo thời gian
      const walkF0 = computeAnimationFrame4Dir('right', true, 0.0, false);
      const walkF1 = computeAnimationFrame4Dir('right', true, 0.13, false);
      expect(walkF0.frameIndex).toBe(0);
      expect(walkF1.frameIndex).toBe(1);
      expect(walkF1.bobOffsetPx).not.toBe(0);

      // Khi bật Reduced Motion (reducedMotion = true), cố định ở frameIndex = 0 và bobOffsetPx = 0
      const reducedWalk = computeAnimationFrame4Dir('right', true, 0.13, true);
      expect(reducedWalk.frameIndex).toBe(0);
      expect(reducedWalk.bobOffsetPx).toBe(0);
    });
  });

  describe('3. SoundSynth Audio Methods', () => {
    it('executes all pentatonic SFX methods cleanly when muted and unmuted', () => {
      const synth = new SoundSynth();

      expect(() => {
        synth.setMuted(false);
        synth.playStep();
        synth.playSealStamp();
        synth.playClimaxFanfare();
        synth.playAlertPushback();
        synth.playDialogueBlip();
        synth.playPortalTransition();
        synth.playQuestComplete();

        synth.setMuted(true);
        expect(synth.getMuted()).toBe(true);
        synth.playStep();
        synth.playSealStamp();
        synth.playClimaxFanfare();
        synth.playAlertPushback();
        synth.playDialogueBlip();
        synth.playPortalTransition();
        synth.playQuestComplete();
      }).not.toThrow();
    });
  });
});
