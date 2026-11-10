/**
 * @file src/ui/ThemeManager.ts
 * @description Quản lý đồng bộ Hệ thống Giao diện Sáng / Tối / Hệ thống (Light / Dark / System Mode)
 * theo phong cách Eastward Pixel RPG (Sơn Mài & Giấy Dó).
 * - Áp dụng tức thì thuộc tính [data-theme="light"|"dark"] và [data-theme-mode="system"|"light"|"dark"]
 *   lên document.documentElement và document.body mà không cần tải lại trang.
 * - Lắng nghe thay đổi prefers-color-scheme của hệ điều hành khi ở chế độ 'system'.
 * - Tuyệt đối không áp dụng bộ lọc (filter) lên #pixel-canvas hoặc #main-menu-bg-canvas.
 */

import { THEME_KEY_V1 } from '../core/SaveManager';
import type { EffectiveTheme, ThemeMode } from '../types/game';

export type ThemeChangeCallback = (
  effectiveTheme: EffectiveTheme,
  mode: ThemeMode
) => void;

export function resolveEffectiveTheme(mode: ThemeMode = 'system'): EffectiveTheme {
  if (mode === 'light') return 'light';
  if (mode === 'dark') return 'dark';
  if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
    try {
      const mql = window.matchMedia('(prefers-color-scheme: light)');
      if (mql && mql.matches) {
        return 'light';
      }
    } catch {
      // Fallback sang dark nếu môi trường không hỗ trợ matchMedia
    }
  }
  return 'dark';
}

export class ThemeManager {
  private currentMode: ThemeMode = 'system';
  private effectiveTheme: EffectiveTheme = 'dark';
  private mediaQueryList: MediaQueryList | null = null;
  private listeners = new Set<ThemeChangeCallback>();

  constructor(initialMode: ThemeMode = 'system') {
    this.setupSystemMediaListener();
    this.setThemeMode(initialMode, false);
  }

  private setupSystemMediaListener(): void {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }
    try {
      this.mediaQueryList = window.matchMedia('(prefers-color-scheme: light)');
      const handleChange = () => {
        if (this.currentMode === 'system') {
          this.applyToDom('system', true);
        }
      };
      if (typeof this.mediaQueryList.addEventListener === 'function') {
        this.mediaQueryList.addEventListener('change', handleChange);
      } else if (typeof (this.mediaQueryList as any).addListener === 'function') {
        (this.mediaQueryList as any).addListener(handleChange);
      }
    } catch {
      this.mediaQueryList = null;
    }
  }

  public getThemeMode(): ThemeMode {
    return this.currentMode;
  }

  public getEffectiveTheme(): EffectiveTheme {
    return this.effectiveTheme;
  }

  public onChange(cb: ThemeChangeCallback): () => void {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  }

  public setThemeMode(mode: ThemeMode, persist = true): EffectiveTheme {
    const normalizedMode: ThemeMode =
      mode === 'light' || mode === 'dark' || mode === 'system' ? mode : 'system';
    this.currentMode = normalizedMode;

    if (persist && typeof localStorage !== 'undefined') {
      try {
        localStorage.setItem(THEME_KEY_V1, normalizedMode);
      } catch {
        // Bỏ qua lỗi lưu trữ
      }
    }

    return this.applyToDom(normalizedMode, true);
  }

  private applyToDom(mode: ThemeMode, notify: boolean): EffectiveTheme {
    const effective = resolveEffectiveTheme(mode);
    this.effectiveTheme = effective;

    if (typeof document !== 'undefined') {
      const root = document.documentElement;
      if (root) {
        root.dataset.theme = effective;
        root.dataset.themeMode = mode;
        root.style.colorScheme = effective;
      }
      if (document.body) {
        document.body.dataset.theme = effective;
        document.body.dataset.themeMode = mode;
      }
    }

    if (notify) {
      for (const cb of this.listeners) {
        cb(effective, mode);
      }
    }

    return effective;
  }
}
