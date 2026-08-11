const DEFAULT_MAX_PENDING = 16;
const DEFAULT_MAX_SAMPLES = 256;
const DEFAULT_MAX_DRAIN_POLLS = 120;

/**
 * Bounded, asynchronous WebGL2 GPU timing for opt-in diagnostics.
 *
 * Results are read only after QUERY_RESULT_AVAILABLE. This class never calls finish(),
 * never spins waiting for the GPU, and never overlaps TIME_ELAPSED queries.
 */
export class AsyncGpuTimerQueries {
  constructor(gl, { maxPending = DEFAULT_MAX_PENDING, maxSamples = DEFAULT_MAX_SAMPLES } = {}) {
    this.gl = gl || null;
    this.extension = gl?.getExtension?.("EXT_disjoint_timer_query_webgl2") || null;
    this.maxPending = boundedInteger(maxPending, 1, 128, DEFAULT_MAX_PENDING);
    this.maxSamples = boundedInteger(maxSamples, 1, 4096, DEFAULT_MAX_SAMPLES);
    this.pending = [];
    this.active = null;
    this.samples = new Map();
    this.dropped = 0;
    this.disjoint = 0;
    this.destroyed = false;
  }

  get supported() {
    return !!this.gl && !!this.extension && !this.destroyed;
  }

  measure(label, draw) {
    if (typeof draw !== "function") throw new TypeError("GPU timer measurement requires a callback.");
    const safeLabel = boundedLabel(label);
    if (!this.supported || this.active || this.pending.length >= this.maxPending) {
      if (this.supported) this.dropped += 1;
      return draw();
    }
    const query = this.gl.createQuery?.();
    if (!query) {
      this.dropped += 1;
      return draw();
    }
    try {
      this.gl.beginQuery(this.extension.TIME_ELAPSED_EXT, query);
    } catch {
      this.gl.deleteQuery?.(query);
      this.dropped += 1;
      return draw();
    }
    this.active = query;
    let completed = false;
    let ended = false;
    try {
      const result = draw();
      completed = true;
      return result;
    } finally {
      try {
        this.gl.endQuery(this.extension.TIME_ELAPSED_EXT);
        ended = true;
      } catch {
        this.gl.deleteQuery?.(query);
        this.dropped += 1;
      }
      this.active = null;
      if (completed && ended) this.pending.push({ label: safeLabel, query });
      else if (ended) this.gl.deleteQuery?.(query);
    }
  }

  poll() {
    if (!this.supported || this.pending.length === 0) return 0;
    let isDisjoint = false;
    try {
      isDisjoint = this.gl.getParameter(this.extension.GPU_DISJOINT_EXT);
    } catch {
      isDisjoint = true;
    }
    if (isDisjoint) {
      this.disjoint += 1;
      this._discardPending();
      return 0;
    }
    let collected = 0;
    const waiting = [];
    for (const entry of this.pending) {
      let available = false;
      try {
        available = this.gl.getQueryParameter(entry.query, this.gl.QUERY_RESULT_AVAILABLE);
      } catch {
        this.gl.deleteQuery?.(entry.query);
        this.dropped += 1;
        continue;
      }
      if (!available) {
        waiting.push(entry);
        continue;
      }
      let elapsedNs = Number.NaN;
      try {
        elapsedNs = Number(this.gl.getQueryParameter(entry.query, this.gl.QUERY_RESULT));
      } catch {
        this.dropped += 1;
      }
      this.gl.deleteQuery?.(entry.query);
      if (!Number.isFinite(elapsedNs) || elapsedNs < 0) continue;
      const values = this.samples.get(entry.label) || [];
      values.push(elapsedNs / 1_000_000);
      if (values.length > this.maxSamples) values.splice(0, values.length - this.maxSamples);
      this.samples.set(entry.label, values);
      collected += 1;
    }
    this.pending = waiting;
    return collected;
  }

  summary() {
    return Object.freeze({
      supported: this.supported,
      pending: this.pending.length,
      dropped: this.dropped,
      disjoint: this.disjoint,
      groups: [...this.samples.entries()].map(([label, values]) => summarize(label, values)),
    });
  }

  reset() {
    this._discardPending();
    this.samples.clear();
    this.dropped = 0;
    this.disjoint = 0;
  }

  destroy() {
    if (this.destroyed) return;
    this._discardPending();
    this.samples.clear();
    this.destroyed = true;
    this.gl = null;
    this.extension = null;
  }

  _discardPending() {
    for (const { query } of this.pending) this.gl?.deleteQuery?.(query);
    this.pending = [];
  }
}

/** Run opt-in samples from independent tasks rather than coupling sample count to display rAF. */
export async function runRafIndependentGpuSamples({
  timer,
  label,
  sample,
  count = 30,
  warmup = 3,
  yieldTask = defaultTaskYield,
  maxDrainPolls = DEFAULT_MAX_DRAIN_POLLS,
} = {}) {
  if (!(timer instanceof AsyncGpuTimerQueries)) throw new TypeError("GPU samples require an AsyncGpuTimerQueries timer.");
  if (typeof sample !== "function") throw new TypeError("GPU samples require a sample callback.");
  const sampleCount = boundedInteger(count, 1, 1000, 30);
  const warmupCount = boundedInteger(warmup, 0, 100, 3);
  for (let index = 0; index < warmupCount; index += 1) {
    sample(index, true);
    await yieldTask();
  }
  timer.reset();
  for (let index = 0; index < sampleCount; index += 1) {
    timer.measure(label, () => sample(index, false));
    timer.poll();
    await yieldTask();
  }
  const drainLimit = boundedInteger(maxDrainPolls, 0, 1000, DEFAULT_MAX_DRAIN_POLLS);
  for (let poll = 0; timer.pending.length > 0 && poll < drainLimit; poll += 1) {
    timer.poll();
    if (timer.pending.length > 0) await yieldTask();
  }
  return timer.summary();
}

function summarize(label, values) {
  const sorted = [...values].sort((a, b) => a - b);
  const sum = values.reduce((total, value) => total + value, 0);
  return Object.freeze({
    label,
    samples: values.length,
    avgMs: round(sum / Math.max(1, values.length)),
    p50Ms: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    maxMs: round(sorted.at(-1) || 0),
  });
}

function percentile(sorted, fraction) {
  if (sorted.length === 0) return 0;
  return round(sorted[Math.floor((sorted.length - 1) * fraction)]);
}

function boundedLabel(value) {
  return String(value || "gpu").replace(/[^a-zA-Z0-9_.-]/g, "_").slice(0, 64) || "gpu";
}

function boundedInteger(value, min, max, fallback) {
  const number = Number(value);
  return Number.isSafeInteger(number) ? Math.min(max, Math.max(min, number)) : fallback;
}

function round(value) {
  return Math.round(Number(value || 0) * 1000) / 1000;
}

function defaultTaskYield() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
