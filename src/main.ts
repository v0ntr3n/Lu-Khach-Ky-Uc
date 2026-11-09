/**
 * @file src/main.ts
 * @description Điểm khởi chạy ứng dụng Vite cho "Lữ Khách Ký Ức (1930 – 2045)".
 * Khởi tạo LuKhachKyUcGame từ src/core/GameApp.ts và gắn vào window.__LU_KHACH_GAME__
 * phục vụ kiểm thử tự động.
 */

import { LuKhachKyUcGame } from './core/GameApp';
import { GameStore } from './core/GameStore';
import { InputManager } from './core/InputManager';
import { SaveManager } from './core/SaveManager';

declare global {
  interface Window {
    __LU_KHACH_GAME__?: LuKhachKyUcGame;
  }
}

export { LuKhachKyUcGame, GameStore, InputManager, SaveManager };

function bootstrapGame(): void {
  if (typeof document === 'undefined' || !document.getElementById('pixel-canvas')) {
    return;
  }
  if (window.__LU_KHACH_GAME__) {
    return;
  }
  window.__LU_KHACH_GAME__ = new LuKhachKyUcGame();
}

if (typeof window !== 'undefined' && typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', bootstrapGame);
  } else {
    bootstrapGame();
  }
}
