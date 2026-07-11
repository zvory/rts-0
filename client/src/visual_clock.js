// Render-only time. Network, health, input, and timeout clocks deliberately do not use this seam.

export class RenderClock {
  now() {
    return performance.now();
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
