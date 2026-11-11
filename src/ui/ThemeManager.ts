/**
 * @file src/ui/ThemeManager.ts
 * @description Quản lý đồng bộ Hệ thống Giao diện Sáng / Tối / Hệ thống (Light / Dark / System Mode)
 * theo phong cách Eastward Pixel RPG (Sơn Mài & Giấy Dó) & chuẩn Modern Web Guidance
 * (`dark-mode`, `dark-mode-toggle`, `prevent-page-scroll-under-modal-dialog`, `defer-rendering-heavy-content`).
 * - Đồng bộ `<meta name="color-scheme">` (`light dark` khi theo hệ thống, `light` hoặc `dark` khi ghim thủ công)
 *   cùng thuộc tính `[data-theme="light"|"dark"]` và `[data-theme-mode="system"|"light"|"dark"]`.
 * - Chỉ lưu override (`light` / `dark`) vào `localStorage`, xoá key khi quay về `system`, giữ nguyên override
 *   khi hệ điều hành đổi giao diện trùng khớp, và đồng bộ đa tab qua sự kiện `storage`.
 * - Bổ sung CSS nền tảng (`color-scheme`, `accent-color`, `scrollbar-color`, `overscroll-behavior: contain`,
 *   `content-visibility: auto`) hoàn toàn dùng biến semantic token, không chứa mã màu cứng.
 * - Tuyệt đối không áp dụng bộ lọc (filter) lên #pixel-canvas hoặc #main-menu-bg-canvas.
 */

import { THEME_KEY_V1 } from '../core/SaveManager';
import type { EffectiveTheme, ThemeMode } from '../types/game';

export type ThemeChangeCallback = (
  effectiveTheme: EffectiveTheme,
  mode: ThemeMode
) => void;

export const MODERN_WEB_PLATFORM_STYLE_ID = 'rpg-modern-web-platform';

export const MODERN_WEB_PLATFORM_CSS = `
:root {
  color-scheme: light dark;
  accent-color: var(--accent-primary);
  scrollbar-color: var(--border-default) var(--surface-primary);
}

:root:has(> head > meta[name='color-scheme'][content='dark']),
[data-theme='dark'] {
  color-scheme: dark;
  scrollbar-color: var(--border-default) var(--surface-subtle);
}

:root:has(> head > meta[name='color-scheme'][content='light']),
[data-theme='light'] {
  color-scheme: light;
  scrollbar-color: var(--border-strong) var(--surface-elevated);
}

dialog:modal {
  overscroll-behavior: contain;
}

dialog::backdrop {
  overflow: hidden;
  overscroll-behavior: contain;
}

.rpg-modal-dialog,
.world-map-dialog,
.climax-dialog,
#journal-quest-list,
#codex-milestone-list,
#lookup-results-list,
#lookup-drawer,
#presenter-drawer {
  scrollbar-width: thin;
  scrollbar-color: var(--border-default) var(--surface-subtle);
  overscroll-behavior: contain;
}

.lookup-entry-card,
.codex-chapter-group {
  content-visibility: auto;
  contain-intrinsic-block-size: auto 5.5rem;
}
`;

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

export function syncColorSchemeMeta(
  mode: ThemeMode,
  effective: EffectiveTheme,
  doc: Document = document
): HTMLMetaElement | null {
  if (!doc.head) return null;
  let meta = doc.querySelector<HTMLMetaElement>('meta[name="color-scheme"]');
  if (!meta) {
    meta = doc.createElement('meta');
    meta.name = 'color-scheme';
    doc.head.prepend(meta);
  }
  meta.content = mode === 'system' ? 'light dark' : effective;
  return meta;
}

export function ensureModernWebPlatformStyles(
  doc: Document = document
): HTMLStyleElement | null {
  if (!doc.head) return null;
  let styleEl = doc.getElementById(
    MODERN_WEB_PLATFORM_STYLE_ID
  ) as HTMLStyleElement | null;
  if (!styleEl) {
    styleEl = doc.createElement('style');
    styleEl.id = MODERN_WEB_PLATFORM_STYLE_ID;
    styleEl.textContent = MODERN_WEB_PLATFORM_CSS;
    doc.head.appendChild(styleEl);
  } else if (styleEl.textContent !== MODERN_WEB_PLATFORM_CSS) {
    styleEl.textContent = MODERN_WEB_PLATFORM_CSS;
  }
  return styleEl;
}

export class ThemeManager {
  private currentMode: ThemeMode = 'system';
  private effectiveTheme: EffectiveTheme = 'dark';
  private mediaQueryList: MediaQueryList | null = null;
  private boundMediaChange: (() => void) | null = null;
  private boundStorageChange: ((event: StorageEvent) => void) | null = null;
  private listeners = new Set<ThemeChangeCallback>();

  constructor(initialMode: ThemeMode = 'system') {
    this.setupSystemMediaListener();
    this.setupCrossTabStorageListener();
    this.setThemeMode(initialMode, false);
  }

  private setupSystemMediaListener(): void {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }
    try {
      this.mediaQueryList = window.matchMedia('(prefers-color-scheme: light)');
      this.boundMediaChange = () => {
        if (this.currentMode === 'system') {
          this.applyToDom('system', true);
        }
      };
      if (typeof this.mediaQueryList.addEventListener === 'function') {
        this.mediaQueryList.addEventListener('change', this.boundMediaChange);
      } else if (
        typeof (
          this.mediaQueryList as MediaQueryList & {
            addListener?: (cb: () => void) => void;
          }
        ).addListener === 'function'
      ) {
        (
          this.mediaQueryList as MediaQueryList & {
            addListener: (cb: () => void) => void;
          }
        ).addListener(this.boundMediaChange);
      }
    } catch {
      this.mediaQueryList = null;
    }
  }

  private setupCrossTabStorageListener(): void {
    if (typeof window === 'undefined' || typeof window.addEventListener !== 'function') {
      return;
    }
    this.boundStorageChange = (event: StorageEvent) => {
      if (event.key !== THEME_KEY_V1 && event.key !== null) return;
      const rawVal = event.key === null ? null : event.newValue;
      const nextMode: ThemeMode =
        rawVal === 'light' || rawVal === 'dark' ? rawVal : 'system';
      if (nextMode !== this.currentMode) {
        this.currentMode = nextMode;
        this.applyToDom(nextMode, true);
      }
    };
    window.addEventListener('storage', this.boundStorageChange);
  }

  public getThemeMode(): ThemeMode {
    return this.currentMode;
  }

  public getEffectiveTheme(): EffectiveTheme {
    return this.effectiveTheme;
  }

  public getSystemEffectiveTheme(): EffectiveTheme {
    return resolveEffectiveTheme('system');
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
        if (normalizedMode === 'system') {
          localStorage.removeItem(THEME_KEY_V1);
        } else {
          localStorage.setItem(THEME_KEY_V1, normalizedMode);
        }
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
      syncColorSchemeMeta(mode, effective, document);
      ensureModernWebPlatformStyles(document);
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

  public destroy(): void {
    if (this.mediaQueryList && this.boundMediaChange) {
      if (typeof this.mediaQueryList.removeEventListener === 'function') {
        this.mediaQueryList.removeEventListener('change', this.boundMediaChange);
      }
    }
    if (typeof window !== 'undefined' && this.boundStorageChange) {
      window.removeEventListener('storage', this.boundStorageChange);
    }
    this.listeners.clear();
  }
}
