/**
 * @file src/ui/FullscreenManager.ts
 * @description Quản lý trạng thái Toàn màn hình (Native Fullscreen API + Viewport Fallback an toàn).
 * Đảm bảo chỉ gọi `requestFullscreen()` bên trong cử chỉ người dùng (user gesture) và tự động
 * chuyển sang chế độ tràn khung nhìn (`body.is-viewport-fullscreen`) khi trình duyệt chặn quyền,
 * chạy trên iOS Safari hoặc môi trường kiểm thử Headless Chromium.
 */

export interface FullscreenState {
  isNativeSupported: boolean;
  isBrowserFullscreen: boolean;
  isViewportFallback: boolean;
}

interface VendorFullscreenElement extends HTMLElement {
  webkitRequestFullscreen?: () => Promise<void> | void;
}

interface VendorFullscreenDocument extends Document {
  webkitFullscreenEnabled?: boolean;
  webkitFullscreenElement?: Element | null;
  webkitExitFullscreen?: () => Promise<void> | void;
}

export class FullscreenManager {
  private targetElement: HTMLElement | null;
  private readonly onChange?: (state: FullscreenState) => void;
  private state: FullscreenState;
  private readonly boundHandleFullscreenChange: () => void;

  constructor(
    targetElement?: HTMLElement | null,
    onChange?: (state: FullscreenState) => void
  ) {
    this.targetElement =
      targetElement ??
      (typeof document !== 'undefined'
        ? (document.getElementById('game-shell') ?? document.documentElement)
        : null);
    this.onChange = onChange;

    this.state = {
      isNativeSupported: this.detectNativeSupport(),
      isBrowserFullscreen: this.readBrowserFullscreen(),
      isViewportFallback:
        typeof document !== 'undefined' &&
        Boolean(document.body?.classList.contains('is-viewport-fullscreen')),
    };

    this.boundHandleFullscreenChange = () => {
      const activeBrowserFs = this.readBrowserFullscreen();
      this.state.isBrowserFullscreen = activeBrowserFs;
      if (activeBrowserFs && this.state.isViewportFallback) {
        this.state.isViewportFallback = false;
        if (typeof document !== 'undefined') {
          document.body?.classList.remove('is-viewport-fullscreen');
        }
      }
      this.syncBodyClasses();
      this.emitChange();
    };

    if (typeof document !== 'undefined') {
      this.syncBodyClasses();
      document.addEventListener('fullscreenchange', this.boundHandleFullscreenChange);
      document.addEventListener(
        'webkitfullscreenchange',
        this.boundHandleFullscreenChange
      );
    }
  }

  private resolveTargetElement(): HTMLElement | null {
    if (this.targetElement) {
      return this.targetElement;
    }
    if (typeof document !== 'undefined') {
      this.targetElement =
        document.getElementById('game-shell') ?? document.documentElement;
    }
    return this.targetElement;
  }

  private detectNativeSupport(): boolean {
    if (typeof document === 'undefined') {
      return false;
    }
    const doc = document as VendorFullscreenDocument;
    if (doc.fullscreenEnabled === false && !doc.webkitFullscreenEnabled) {
      return false;
    }
    const el = this.resolveTargetElement() as VendorFullscreenElement | null;
    if (!el) {
      return false;
    }
    return (
      typeof el.requestFullscreen === 'function' ||
      typeof el.webkitRequestFullscreen === 'function'
    );
  }

  private readBrowserFullscreen(): boolean {
    if (typeof document === 'undefined') {
      return false;
    }
    const doc = document as VendorFullscreenDocument;
    return Boolean(doc.fullscreenElement ?? doc.webkitFullscreenElement);
  }

  private syncBodyClasses(): void {
    if (typeof document === 'undefined' || !document.body) {
      return;
    }
    document.body.classList.toggle(
      'is-browser-fullscreen',
      this.state.isBrowserFullscreen
    );
    document.body.classList.toggle(
      'is-viewport-fullscreen',
      this.state.isViewportFallback
    );
  }

  private emitChange(): void {
    this.onChange?.(this.getState());
  }

  public getState(): FullscreenState {
    return {
      isNativeSupported: this.state.isNativeSupported,
      isBrowserFullscreen: this.state.isBrowserFullscreen,
      isViewportFallback: this.state.isViewportFallback,
    };
  }

  public isFullscreenActive(): boolean {
    return this.state.isBrowserFullscreen || this.state.isViewportFallback;
  }

  /**
   * Yêu cầu bật Toàn màn hình (chỉ gọi từ trình xử lý sự kiện người dùng / user gesture).
   * Nếu API trình duyệt từ chối hoặc không hỗ trợ, tự động kích hoạt `is-viewport-fullscreen`
   * mà không ném ngoại lệ.
   */
  public async request(): Promise<boolean> {
    if (typeof document === 'undefined') {
      return false;
    }

    const el = this.resolveTargetElement() as VendorFullscreenElement | null;
    const doc = document as VendorFullscreenDocument;
    const canTryNative =
      el !== null &&
      !(doc.fullscreenEnabled === false && !doc.webkitFullscreenEnabled) &&
      (typeof el.requestFullscreen === 'function' ||
        typeof el.webkitRequestFullscreen === 'function');

    this.state.isNativeSupported = canTryNative;

    if (canTryNative && el) {
      try {
        if (typeof el.requestFullscreen === 'function') {
          await el.requestFullscreen();
        } else if (typeof el.webkitRequestFullscreen === 'function') {
          await el.webkitRequestFullscreen();
        }
        this.state.isBrowserFullscreen = true;
        this.state.isViewportFallback = false;
        this.syncBodyClasses();
        this.emitChange();
        return true;
      } catch {
        // Fallback sang chế độ tràn khung nhìn (100vw x 100dvh) khi bị chặn quyền hoặc Headless
      }
    }

    this.state.isBrowserFullscreen = false;
    this.state.isViewportFallback = true;
    this.syncBodyClasses();
    this.emitChange();
    return true;
  }

  /**
   * Thoát khỏi chế độ Toàn màn hình (cả Native Fullscreen lẫn Viewport Fallback).
   */
  public async exit(): Promise<boolean> {
    if (typeof document === 'undefined') {
      this.state.isBrowserFullscreen = false;
      this.state.isViewportFallback = false;
      return false;
    }

    const doc = document as VendorFullscreenDocument;
    this.state.isViewportFallback = false;
    document.body?.classList.remove('is-viewport-fullscreen');

    if (doc.fullscreenElement || doc.webkitFullscreenElement || this.state.isBrowserFullscreen) {
      try {
        if (doc.fullscreenElement && typeof doc.exitFullscreen === 'function') {
          await doc.exitFullscreen();
        } else if (
          doc.webkitFullscreenElement &&
          typeof doc.webkitExitFullscreen === 'function'
        ) {
          await doc.webkitExitFullscreen();
        }
      } catch {
        // Bỏ qua lỗi khi trình duyệt đã tự thoát fullscreen trước đó
      }
    }

    this.state.isBrowserFullscreen = this.readBrowserFullscreen() ? false : false;
    this.syncBodyClasses();
    this.emitChange();
    return this.isFullscreenActive();
  }

  /**
   * Đảo trạng thái Toàn màn hình hiện hành và trả về trạng thái kích hoạt mới (`true` = đang bật).
   */
  public async toggle(): Promise<boolean> {
    if (this.isFullscreenActive()) {
      await this.exit();
      return this.isFullscreenActive();
    }
    return this.request();
  }

  public destroy(): void {
    if (typeof document !== 'undefined') {
      document.removeEventListener('fullscreenchange', this.boundHandleFullscreenChange);
      document.removeEventListener(
        'webkitfullscreenchange',
        this.boundHandleFullscreenChange
      );
    }
  }
}
