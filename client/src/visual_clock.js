// Render-only time. Network, health, input, and timeout clocks deliberately do not use this seam.

export class RenderClock {
  constructor(resumeAtMs = null) {
    const performanceMs = performance.now();
    this.anchorVisualMs = Number.isFinite(resumeAtMs) ? resumeAtMs : performanceMs;
    this.anchorPerformanceMs = performanceMs;
    this.rate = 1;
  }

  now() {
    return this.anchorVisualMs + (performance.now() - this.anchorPerformanceMs) * this.rate;
  }

  setRate(rate) {
    if (!Number.isFinite(rate) || rate < 0) {
      throw new RangeError("Render clock rate must be a non-negative finite number.");
    }
    const performanceMs = performance.now();
    this.anchorVisualMs += (performanceMs - this.anchorPerformanceMs) * this.rate;
    this.anchorPerformanceMs = performanceMs;
    this.rate = rate;
  }
}

export function syncRenderClockToRoomTime(renderClock, state) {
  const speed = typeof state?.speed === "number" ? state.speed : NaN;
  const ended = state?.ended === true
    || (Number(state?.durationTicks) > 0 && Number(state?.currentTick) >= Number(state?.durationTicks));
  if (state?.paused === true || (Number.isFinite(speed) && speed <= 0) || ended) {
    renderClock?.setRate?.(0);
  } else if (Number.isFinite(speed)) {
    renderClock?.setRate?.(speed);
  }
}

export class CaptureRenderClock {
  constructor(initialMs = 0) {
    if (!Number.isFinite(initialMs) || initialMs < 0) throw new TypeError("Capture visual time must be a non-negative finite number.");
    this.valueMs = initialMs;
  }

  now() {
    return this.valueMs;
  }

  advanceTo(valueMs) {
    if (!Number.isFinite(valueMs) || valueMs < this.valueMs) {
      throw new RangeError("Capture visual time must advance monotonically.");
    }
    this.valueMs = valueMs;
    return this.valueMs;
  }
}
