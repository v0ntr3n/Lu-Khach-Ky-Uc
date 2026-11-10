/**
 * @file src/engine/SoundSynth.ts
 * @description Bộ tổng hợp âm thanh Chiptune Ngũ Cung 8-Bit (Web Audio API 0KB)
 * Không dùng tệp âm thanh ngoài, tự động tạo tiếng bước chân, thu thập cổ vật,
 * đóng dấu Sơn Mài và giai điệu đại cảnh Climax.
 */

export class SoundSynth {
  private ctx: AudioContext | null = null;
  private isMuted = false;
  private masterVolume = 0.85;
  private hasUserGesture = false;
  private gestureListenersBound = false;

  constructor() {
    this.bindGestureUnlockListeners();
  }

  private bindGestureUnlockListeners(): void {
    if (this.gestureListenersBound || typeof window === 'undefined') return;
    this.gestureListenersBound = true;

    const unlock = (): void => {
      this.hasUserGesture = true;
      if (this.ctx && this.ctx.state === 'suspended') {
        try {
          const resumePromise = this.ctx.resume();
          if (resumePromise && typeof resumePromise.catch === 'function') {
            resumePromise.catch(() => {});
          }
        } catch {
          // Bỏ qua nếu trình duyệt vẫn khóa autoplay
        }
      }
      window.removeEventListener('pointerdown', unlock);
      window.removeEventListener('mousedown', unlock);
      window.removeEventListener('keydown', unlock);
      window.removeEventListener('touchstart', unlock);
      window.removeEventListener('click', unlock);
    };

    window.addEventListener('pointerdown', unlock, { passive: true });
    window.addEventListener('mousedown', unlock, { passive: true });
    window.addEventListener('keydown', unlock, { passive: true });
    window.addEventListener('touchstart', unlock, { passive: true });
    window.addEventListener('click', unlock, { passive: true });
  }

  public unlockAudio(): void {
    this.hasUserGesture = true;
    this.ensureContext();
  }

  public toggleMute(): boolean {
    this.isMuted = !this.isMuted;
    return this.isMuted;
  }

  public getMuted(): boolean {
    return this.isMuted;
  }

  public setMuted(muted: boolean): void {
    this.isMuted = Boolean(muted);
  }

  public setVolume(vol: number): void {
    if (typeof vol === 'number' && Number.isFinite(vol)) {
      this.masterVolume = Math.min(1, Math.max(0, vol));
    }
  }

  public getVolume(): number {
    return this.masterVolume;
  }

  private isUserActivationReady(): boolean {
    if (this.hasUserGesture) {
      return true;
    }
    if (typeof navigator !== 'undefined' && 'userActivation' in navigator) {
      const ua = navigator.userActivation;
      if (ua) {
        return Boolean(ua.hasBeenActive || ua.isActive);
      }
    }
    // Trong môi trường kiểm thử (jsdom) không có navigator.userActivation, cho phép khởi tạo an toàn
    return true;
  }

  private ensureContext(): AudioContext | null {
    if (this.isMuted || this.masterVolume <= 0 || typeof window === 'undefined') {
      return null;
    }
    if (!this.isUserActivationReady()) {
      return null;
    }

    try {
      if (!this.ctx) {
        const AudioCtx =
          window.AudioContext ||
          (window as unknown as { webkitAudioContext?: typeof AudioContext })
            .webkitAudioContext;
        if (AudioCtx) {
          this.ctx = new AudioCtx();
        }
      }
      if (this.ctx && this.ctx.state === 'suspended') {
        const resumePromise = this.ctx.resume();
        if (resumePromise && typeof resumePromise.catch === 'function') {
          resumePromise.catch(() => {});
        }
      }
      return this.ctx;
    } catch {
      return null;
    }
  }

  private scheduleTone(options: {
    type: OscillatorType;
    startFreq: number;
    endFreq?: number;
    rampType?: 'exponential' | 'linear';
    baseGain: number;
    startOffsetSec?: number;
    durationSec: number;
    stopPaddingSec?: number;
  }): void {
    const ctx = this.ensureContext();
    if (!ctx) return;

    try {
      const peakGain = Math.max(0.0001, options.baseGain * this.masterVolume);
      const endGain = Math.max(0.00001, Math.min(0.001, peakGain * 0.05));
      const startTime = ctx.currentTime + (options.startOffsetSec ?? 0);
      const rampEnd = startTime + options.durationSec;
      const stopTime = rampEnd + (options.stopPaddingSec ?? 0.005);

      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = options.type;
      osc.frequency.setValueAtTime(options.startFreq, startTime);

      if (typeof options.endFreq === 'number') {
        if (options.rampType === 'linear') {
          osc.frequency.linearRampToValueAtTime(options.endFreq, rampEnd);
        } else {
          osc.frequency.exponentialRampToValueAtTime(
            Math.max(1, options.endFreq),
            rampEnd
          );
        }
      }

      gain.gain.setValueAtTime(peakGain, startTime);
      gain.gain.exponentialRampToValueAtTime(endGain, rampEnd);

      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(startTime);
      osc.stop(stopTime);
    } catch {
      // Bỏ qua lỗi tạo node âm thanh khi AudioContext bị khóa hoặc đóng
    }
  }

  public playMenuFocus(): void {
    // Tiếng gõ mộc / đồng Sơn Mài nhẹ khi di chuyển tiêu điểm menu (G4 -> C4)
    this.scheduleTone({
      type: 'triangle',
      startFreq: 392.0,
      endFreq: 261.63,
      rampType: 'exponential',
      baseGain: 0.032,
      durationSec: 0.028,
    });
  }

  public playMenuConfirm(): void {
    // Chuông đôi ngũ cung ấm áp khi xác nhận lựa chọn (C5 -> G5)
    const notes = [523.25, 783.99];
    notes.forEach((freq, idx) => {
      this.scheduleTone({
        type: 'triangle',
        startFreq: freq,
        baseGain: 0.065,
        startOffsetSec: idx * 0.065,
        durationSec: 0.16,
        stopPaddingSec: 0.01,
      });
    });
  }

  public playMenuCancel(): void {
    // Âm trầm ngũ cung hạ nhẹ khi đóng hoặc quay lại (A4 -> D4)
    this.scheduleTone({
      type: 'sine',
      startFreq: 440.0,
      endFreq: 293.66,
      rampType: 'exponential',
      baseGain: 0.05,
      durationSec: 0.11,
      stopPaddingSec: 0.01,
    });
  }

  public playStep(): void {
    this.scheduleTone({
      type: 'triangle',
      startFreq: 115,
      endFreq: 65,
      rampType: 'exponential',
      baseGain: 0.04,
      durationSec: 0.045,
      stopPaddingSec: 0.005,
    });
  }

  public playSealStamp(): void {
    // Âm thanh đóng dấu triện & chuông đồng ngũ cung (C5 -> G5 -> A5)
    const notes = [523.25, 783.99, 880.0];
    notes.forEach((freq, idx) => {
      this.scheduleTone({
        type: 'square',
        startFreq: freq,
        baseGain: 0.07,
        startOffsetSec: idx * 0.075,
        durationSec: 0.22,
        stopPaddingSec: 0.01,
      });
    });
  }

  public playAlertPushback(): void {
    this.scheduleTone({
      type: 'sawtooth',
      startFreq: 220,
      endFreq: 140,
      rampType: 'linear',
      baseGain: 0.06,
      durationSec: 0.19,
      stopPaddingSec: 0.01,
    });
  }

  public playDialogueBlip(): void {
    this.scheduleTone({
      type: 'sine',
      startFreq: 520,
      endFreq: 660,
      rampType: 'exponential',
      baseGain: 0.035,
      durationSec: 0.04,
      stopPaddingSec: 0.004,
    });
  }

  public playPortalTransition(): void {
    this.scheduleTone({
      type: 'triangle',
      startFreq: 220,
      endFreq: 330,
      rampType: 'exponential',
      baseGain: 0.055,
      durationSec: 0.125,
      stopPaddingSec: 0.005,
    });
  }

  public playQuestComplete(): void {
    // Khúc ngũ cung hoàn thành nhiệm vụ (C5 -> D5 -> G5)
    const phrase = [523.25, 587.33, 783.99];
    phrase.forEach((freq, idx) => {
      this.scheduleTone({
        type: 'triangle',
        startFreq: freq,
        baseGain: 0.08,
        startOffsetSec: idx * 0.085,
        durationSec: 0.24,
        stopPaddingSec: 0.01,
      });
    });
  }

  public playAmbientRainPulse(): void {
    this.scheduleTone({
      type: 'sine',
      startFreq: 196.0,
      endFreq: 146.83,
      rampType: 'exponential',
      baseGain: 0.018,
      durationSec: 0.17,
      stopPaddingSec: 0.01,
    });
  }

  public playClimaxFanfare(): void {
    // Khúc khải hoàn ngũ cung (C5 - D5 - F5 - G5 - C6)
    const fanfare = [523.25, 587.33, 698.46, 783.99, 1046.5];
    fanfare.forEach((freq, idx) => {
      const dur = idx === fanfare.length - 1 ? 0.55 : 0.18;
      this.scheduleTone({
        type: 'triangle',
        startFreq: freq,
        baseGain: 0.11,
        startOffsetSec: idx * 0.11,
        durationSec: dur,
        stopPaddingSec: 0.02,
      });
    });
  }
}
