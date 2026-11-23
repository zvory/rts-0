// A bounded aggregate independent of the ten-second exceptional-report window.
const GAP_BUCKETS = [8, 16, 17, 24, 33, 50, 100, 250, 1000, 65535];

export class ClientPerformanceWindow {
  constructor() { this.reset(); }
  reset() {
    this.frameCount = 0;
    this.frameTotalMs = 0;
    this.frameGapMaxMs = 0;
    this.framesOver33Ms = 0;
    this.framesOver50Ms = 0;
    this.framesOver100Ms = 0;
    this.buckets = new Uint32Array(GAP_BUCKETS.length);
  }
  add(gapMs) {
    if (!Number.isFinite(gapMs) || gapMs < 0) return;
    this.frameCount += 1;
    this.frameTotalMs += gapMs;
    this.frameGapMaxMs = Math.max(this.frameGapMaxMs, gapMs);
    if (gapMs > 33) this.framesOver33Ms += 1;
    if (gapMs > 50) this.framesOver50Ms += 1;
    if (gapMs > 100) this.framesOver100Ms += 1;
    const bucket = GAP_BUCKETS.findIndex((limit) => gapMs <= limit);
    this.buckets[bucket < 0 ? this.buckets.length - 1 : bucket] += 1;
  }
  percentile(p) {
    if (!this.frameCount) return 0;
    const target = Math.ceil(this.frameCount * p);
    let seen = 0;
    for (let i = 0; i < this.buckets.length; i += 1) {
      seen += this.buckets[i];
      if (seen >= target) return GAP_BUCKETS[i];
    }
    return 0;
  }
  summary() {
    return {
      frameCount: u32(this.frameCount), frameTotalMs: u32(this.frameTotalMs),
      frameGapMaxMs: u32(this.frameGapMaxMs),
      frameGapP95Ms: this.percentile(0.95), frameGapP99Ms: this.percentile(0.99),
      framesOver33Ms: u32(this.framesOver33Ms), framesOver50Ms: u32(this.framesOver50Ms),
      framesOver100Ms: u32(this.framesOver100Ms),
    };
  }
}

function u32(n) { return Math.min(4294967295, Math.max(0, Math.round(n))); }
