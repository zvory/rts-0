const DEFAULT_BUCKETS = Object.freeze([1, 2, 4, 8, 12, 16, 24, 33, 50, 75, 100, 150, 250, 500, 1000]);

export class ReportWindowAggregate {
  constructor({ buckets = DEFAULT_BUCKETS, maxValue = 60_000 } = {}) {
    this.buckets = buckets;
    this.maxValue = Number.isFinite(maxValue) && maxValue > 0 ? maxValue : 60_000;
    this.count = 0;
    this.total = 0;
    this.max = 0;
    this.bucketCounts = new Uint32Array(buckets.length + 1);
  }

  add(value) {
    const number = Number(value);
    if (!Number.isFinite(number) || number < 0) return;
    const clamped = Math.min(number, this.maxValue);
    this.count += 1;
    this.total += clamped;
    this.max = Math.max(this.max, clamped);
    this.bucketCounts[this.bucketIndex(clamped)] += 1;
  }

  summary() {
    return {
      count: this.count,
      total: round(this.total),
      avg: this.count > 0 ? round(this.total / this.count) : 0,
      max: round(this.max),
      p95: this.percentile(0.95),
    };
  }

  reset() {
    this.count = 0;
    this.total = 0;
    this.max = 0;
    this.bucketCounts.fill(0);
  }

  bucketIndex(value) {
    for (let i = 0; i < this.buckets.length; i += 1) {
      if (value <= this.buckets[i]) return i;
    }
    return this.buckets.length;
  }

  percentile(percentile) {
    if (this.count <= 0) return 0;
    const target = Math.max(1, Math.ceil(this.count * percentile));
    let seen = 0;
    for (let i = 0; i < this.bucketCounts.length; i += 1) {
      seen += this.bucketCounts[i];
      if (seen >= target) return i < this.buckets.length ? this.buckets[i] : this.buckets.at(-1);
    }
    return 0;
  }
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value) : 0;
}
