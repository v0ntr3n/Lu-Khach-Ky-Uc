/**
 * @file src/core/InputManager.ts
 * @description Quản lý bàn phím và điều khiển cảm ứng/con trỏ cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Đảm bảo khóa điều khiển thế giới khi mở overlay, chống lặp phím (key-repeat) cho phím tương tác,
 * hỗ trợ phím số 1–4 chọn nhanh hội thoại/câu hỏi và tự động xóa trạng thái phím khi mất tiêu điểm.
 */

import type { Vec2 } from '../types/game';

export interface InputCallbacks {
  onInteract?: () => void;
  onToggleLookup?: () => void;
  onEscape?: () => void;
  onToggleJournal?: () => void;
  onToggleMap?: () => void;
  onToggleMinimap?: () => void;
  onToggleInventory?: () => void;
  onTogglePause?: () => void;
  onToggleDebugOverlay?: () => void;
  onToggleHints?: () => void;
  onToggleFullscreen?: () => void;
  onTogglePresenterShortcut?: () => void;
  onSkipTutorial?: () => void;
  onNumberKey?: (num: 1 | 2 | 3 | 4) => void;
  onActionKey?: (key: string) => void;
}

const MOVEMENT_KEYS = new Set<string>([
  'w',
  'a',
  's',
  'd',
  'arrowup',
  'arrowdown',
  'arrowleft',
  'arrowright',
]);

const ACTION_KEYS = new Set<string>([
  'e',
  ' ',
  'q',
  'j',
  'm',
  'n',
  'i',
  'h',
  'f',
  'x',
  'escape',
  'tab',
  'f3',
  '1',
  '2',
  '3',
  '4',
]);

const VIRTUAL_HOLD_BUTTONS: ReadonlyArray<{ id: string; key: string }> = [
  { id: 'ctrl-left', key: 'arrowleft' },
  { id: 'ctrl-right', key: 'arrowright' },
  { id: 'ctrl-shelter', key: 'arrowdown' },
  { id: 'ctrl-up', key: 'arrowup' },
  { id: 'ctrl-down', key: 'arrowdown' },
];

export class InputManager {
  public readonly keysPressed = new Set<string>();
  public readonly consumedActionKeys = new Set<string>();
  private readonly boundTouchElements = new WeakSet<HTMLElement>();
  private callbacks: InputCallbacks;

  constructor(callbacks: InputCallbacks = {}) {
    this.callbacks = callbacks;
    this.bindKeyboardListeners();
    this.bindTouchControls();
  }

  public setCallbacks(callbacks: InputCallbacks): void {
    this.callbacks = { ...this.callbacks, ...callbacks };
  }

  private bindKeyboardListeners(): void {
    if (typeof window === 'undefined') return;

    window.addEventListener('keydown', this.handleKeyDown);
    window.addEventListener('keyup', this.handleKeyUp);
    window.addEventListener('blur', this.handleWindowBlur);

    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', this.handleVisibilityChange);
    }
  }

  public bindTouchControls(): void {
    if (typeof document === 'undefined') return;

    for (const { id, key } of VIRTUAL_HOLD_BUTTONS) {
      const el = document.getElementById(id);
      if (!el || this.boundTouchElements.has(el)) continue;
      this.boundTouchElements.add(el);

      const press = (e: Event) => {
        e.preventDefault?.();
        this.keysPressed.add(key);
      };
      const release = () => {
        this.keysPressed.delete(key);
      };

      el.addEventListener('pointerdown', press);
      el.addEventListener('pointerup', release);
      el.addEventListener('pointerleave', release);
      el.addEventListener('pointercancel', release);
      el.addEventListener('lostpointercapture', release);
    }

    const interactBtn = document.getElementById('ctrl-interact');
    if (interactBtn && !this.boundTouchElements.has(interactBtn)) {
      this.boundTouchElements.add(interactBtn);
      interactBtn.addEventListener('click', () => {
        this.callbacks.onInteract?.();
      });
    }
  }

  public rebindTouchControls(): void {
    this.bindTouchControls();
  }

  private readonly handleKeyDown = (e: KeyboardEvent): void => {
    const key = e.key === ' ' ? ' ' : e.key.toLowerCase();

    if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === 'p') {
      e.preventDefault();
      if (e.repeat || this.consumedActionKeys.has('presenter_shortcut')) {
        return;
      }
      this.consumedActionKeys.add('presenter_shortcut');
      this.callbacks.onTogglePresenterShortcut?.();
      return;
    }

    const target = e.target as HTMLElement | null;
    const tag = target?.tagName;
    if (
      key !== 'escape' &&
      key !== 'f3' &&
      (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target?.isContentEditable)
    ) {
      return;
    }

    if (ACTION_KEYS.has(key)) {
      if (
        (key === 'f' || key === 'x') &&
        (e.ctrlKey || e.metaKey || e.altKey)
      ) {
        return;
      }
      if (e.repeat || this.consumedActionKeys.has(key)) {
        return;
      }
      this.consumedActionKeys.add(key);

      if (key === 'e' || key === ' ') {
        e.preventDefault();
        this.callbacks.onInteract?.();
      } else if (key === 'q') {
        e.preventDefault();
        this.callbacks.onToggleLookup?.();
      } else if (key === 'escape') {
        this.callbacks.onEscape?.();
      } else if (key === 'j') {
        e.preventDefault();
        this.callbacks.onToggleJournal?.();
      } else if (key === 'm') {
        e.preventDefault();
        this.callbacks.onToggleMap?.();
      } else if (key === 'n') {
        e.preventDefault();
        this.callbacks.onToggleMinimap?.();
      } else if (key === 'i') {
        e.preventDefault();
        this.callbacks.onToggleInventory?.();
      } else if (key === 'h') {
        e.preventDefault();
        this.callbacks.onToggleHints?.();
      } else if (key === 'f') {
        e.preventDefault();
        this.callbacks.onToggleFullscreen?.();
      } else if (key === 'x') {
        this.callbacks.onSkipTutorial?.();
      } else if (key === 'tab') {
        e.preventDefault();
        this.callbacks.onTogglePause?.();
      } else if (key === 'f3') {
        e.preventDefault();
        this.callbacks.onToggleDebugOverlay?.();
      } else if (key === '1' || key === '2' || key === '3' || key === '4') {
        this.callbacks.onNumberKey?.(Number(key) as 1 | 2 | 3 | 4);
      }

      this.callbacks.onActionKey?.(key);
      return;
    }

    if (MOVEMENT_KEYS.has(key)) {
      this.keysPressed.add(key);
    }
  };

  private readonly handleKeyUp = (e: KeyboardEvent): void => {
    const key = e.key === ' ' ? ' ' : e.key.toLowerCase();
    this.keysPressed.delete(key);
    this.consumedActionKeys.delete(key);
    if (key === 'p' || key === 'control' || key === 'meta' || key === 'shift') {
      this.consumedActionKeys.delete('presenter_shortcut');
    }
  };

  private readonly handleWindowBlur = (): void => {
    this.clearKeys();
  };

  private readonly handleVisibilityChange = (): void => {
    this.clearKeys();
  };

  public clearKeys(): void {
    this.keysPressed.clear();
    this.consumedActionKeys.clear();
  }

  public getMoveVector(allowWorldInput = true): Vec2 {
    if (!allowWorldInput) {
      return { x: 0, y: 0 };
    }

    const left = this.keysPressed.has('a') || this.keysPressed.has('arrowleft');
    const right = this.keysPressed.has('d') || this.keysPressed.has('arrowright');
    const up = this.keysPressed.has('w') || this.keysPressed.has('arrowup');
    const down = this.keysPressed.has('s') || this.keysPressed.has('arrowdown');

    let x = (right ? 1 : 0) - (left ? 1 : 0);
    let y = (down ? 1 : 0) - (up ? 1 : 0);

    if (x !== 0 && y !== 0) {
      x *= Math.SQRT1_2;
      y *= Math.SQRT1_2;
    }

    return { x, y };
  }

  public isMoving(allowWorldInput = true): boolean {
    if (!allowWorldInput) {
      return false;
    }
    return (
      this.keysPressed.has('a') ||
      this.keysPressed.has('arrowleft') ||
      this.keysPressed.has('d') ||
      this.keysPressed.has('arrowright') ||
      this.keysPressed.has('w') ||
      this.keysPressed.has('arrowup') ||
      this.keysPressed.has('s') ||
      this.keysPressed.has('arrowdown')
    );
  }

  public isCrouching(allowWorldInput = true): boolean {
    if (!allowWorldInput) {
      return false;
    }
    return this.keysPressed.has('s') || this.keysPressed.has('arrowdown');
  }

  public destroy(): void {
    if (typeof window !== 'undefined') {
      window.removeEventListener('keydown', this.handleKeyDown);
      window.removeEventListener('keyup', this.handleKeyUp);
      window.removeEventListener('blur', this.handleWindowBlur);
    }
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.handleVisibilityChange);
    }
    this.clearKeys();
  }
}
