const DEFAULT_BUCKETS_MS = Object.freeze([1, 2, 4, 8, 12, 16, 24, 33, 50, 75, 100, 150, 250, 500, 1000]);
const DEFAULT_SLOW_FRAME_MS = 33;
const DEFAULT_SLOW_PHASE_MS = 8;
const MAX_RECENT_FRAMES = 12;
const MAX_LABEL_LENGTH = 64;

export class FrameProfiler {
  constructor({
    now = () => performance.now(),
    slowFrameMs = DEFAULT_SLOW_FRAME_MS,
    slowPhaseMs = DEFAULT_SLOW_PHASE_MS,
    maxRecentFrames = MAX_RECENT_FRAMES,
  } = {}) {
    this.now = now;
    this.slowFrameMs = slowFrameMs;
    this.slowPhaseMs = slowPhaseMs;
    this.maxRecentFrames = maxRecentFrames;
    this.phases = new Map();
    this.frameCount = 0;
    this.slowFrameCount = 0;
    this.worstPhaseCounts = new Map();
    this.recentFrames = [];
    this.latestContext = {};
    this.activeFrame = null;
  }

  beginFrame({ at = this.now(), frameGapMs = null } = {}) {
    if (this.activeFrame) this.endFrame({ at });
    this.activeFrame = {
      startedAt: at,
      frameGapMs: finiteOrNull(frameGapMs),
      phaseMs: new Map(),
      worstPhase: null,
      worstPhaseMs: 0,
    };
    if (Number.isFinite(frameGapMs)) {
      this.recordPhase("frame.gap", frameGapMs, { slowMs: this.slowFrameMs, includeInWorst: false });
    }
  }

  time(label, fn) {
    const startedAt = this.now();
    try {
      return fn();
    } finally {
      this.recordPhase(label, this.now() - startedAt);
    }
  }

  recordPhase(label, durationMs, { slowMs = this.slowPhaseMs, includeInWorst = true } = {}) {
    const safeLabel = normalizeLabel(label);
    const ms = clampDuration(durationMs);
    if (ms == null) return;
    this.aggregateFor(safeLabel).add(ms, slowMs);
    if (includeInWorst && this.activeFrame) {
      this.activeFrame.phaseMs.set(safeLabel, (this.activeFrame.phaseMs.get(safeLabel) || 0) + ms);
      if (ms >= this.activeFrame.worstPhaseMs) {
        this.activeFrame.worstPhaseMs = ms;
        this.activeFrame.worstPhase = safeLabel;
      }
    }
  }

  endFrame({ at = this.now(), context = null } = {}) {
    const frame = this.activeFrame;
    if (!frame) return null;
    this.activeFrame = null;
    if (context) this.setContext(context);
    const totalWorkMs = Math.max(0, at - frame.startedAt);
    this.recordPhase("frame.work", totalWorkMs, { slowMs: this.slowFrameMs, includeInWorst: false });
    this.frameCount += 1;
    const slow = (Number.isFinite(frame.frameGapMs) && frame.frameGapMs >= this.slowFrameMs)
      || totalWorkMs >= this.slowFrameMs;
    if (slow) this.slowFrameCount += 1;
    if (frame.worstPhase) {
      this.worstPhaseCounts.set(frame.worstPhase, (this.worstPhaseCounts.get(frame.worstPhase) || 0) + 1);
    }
    const summary = {
      at: round1(at),
      frameGapMs: round1(frame.frameGapMs),
      frameWorkMs: round1(totalWorkMs),
      slow,
      worstPhase: frame.worstPhase,
      worstPhaseMs: round1(frame.worstPhaseMs),
      context: this.latestContext,
    };
    this.recentFrames.push(summary);
    if (this.recentFrames.length > this.maxRecentFrames) {
      this.recentFrames.splice(0, this.recentFrames.length - this.maxRecentFrames);
    }
    return summary;
  }

  setContext(context) {
    if (!context || typeof context !== "object") return;
    this.latestContext = sanitizeContext({ ...this.latestContext, ...context });
  }

  summary() {
    const phaseRows = Array.from(this.phases.entries())
      .map(([label, aggregate]) => ({ label, ...aggregate.summary() }))
      .sort((a, b) => b.totalMs - a.totalMs || b.maxMs - a.maxMs || a.label.localeCompare(b.label));
    const worstPhase = Array.from(this.worstPhaseCounts.entries())
      .map(([label, count]) => ({ label, count }))
      .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))[0] || null;
    return {
      schemaVersion: 1,
      frameCount: this.frameCount,
      slowFrameCount: this.slowFrameCount,
      slowFrameMs: this.slowFrameMs,
      slowPhaseMs: this.slowPhaseMs,
      worstPhase,
      context: this.latestContext,
      phases: phaseRows,
      recentFrames: this.recentFrames.slice(),
    };
  }

  text() {
    const summary = this.summary();
    const lines = [
      `frames=${summary.frameCount} slow=${summary.slowFrameCount} slowFrameMs=${summary.slowFrameMs}`,
      `context=${JSON.stringify(summary.context)}`,
      "phase\tcount\ttotalMs\tmaxMs\tp50Ms\tp95Ms\tslowCount",
    ];
    for (const phase of summary.phases) {
      lines.push([
        phase.label,
        phase.count,
        phase.totalMs,
        phase.maxMs,
        phase.p50Ms,
        phase.p95Ms,
        phase.slowCount,
      ].join("\t"));
    }
    lines.push(`recent=${JSON.stringify(summary.recentFrames)}`);
    return lines.join("\n");
  }

  copy() {
    const text = this.text();
    const clipboard = globalThis.navigator?.clipboard;
    if (clipboard && typeof clipboard.writeText === "function") {
      void clipboard.writeText(text);
    } else if (typeof console !== "undefined" && typeof console.log === "function") {
      console.log(text);
    }
    return text;
  }

  reset() {
    this.phases.clear();
    this.frameCount = 0;
    this.slowFrameCount = 0;
    this.worstPhaseCounts.clear();
    this.recentFrames = [];
    this.activeFrame = null;
  }

  debugSurface() {
    return {
      summary: () => this.summary(),
      text: () => this.text(),
      copy: () => this.copy(),
      reset: () => this.reset(),
    };
  }

  aggregateFor(label) {
    let aggregate = this.phases.get(label);
    if (!aggregate) {
      aggregate = new PhaseAggregate();
      this.phases.set(label, aggregate);
    }
    return aggregate;
  }
}

class PhaseAggregate {
  constructor() {
    this.count = 0;
    this.totalMs = 0;
    this.maxMs = 0;
    this.slowCount = 0;
    this.buckets = new Uint32Array(DEFAULT_BUCKETS_MS.length + 1);
  }

  add(durationMs, slowMs) {
    this.count += 1;
    this.totalMs += durationMs;
    this.maxMs = Math.max(this.maxMs, durationMs);
    if (durationMs >= slowMs) this.slowCount += 1;
    this.buckets[bucketIndex(durationMs)] += 1;
  }

  summary() {
    return {
      count: this.count,
      totalMs: round1(this.totalMs),
      avgMs: this.count > 0 ? round1(this.totalMs / this.count) : 0,
      maxMs: round1(this.maxMs),
      p50Ms: percentileBucket(this.buckets, this.count, 0.5),
      p95Ms: percentileBucket(this.buckets, this.count, 0.95),
      slowCount: this.slowCount,
    };
  }
}

export function collectMatchFrameContext(match) {
  const state = match?.state || {};
  const camera = match?.camera || {};
  const renderer = match?.renderer?.app?.renderer || {};
  const canvas = match?.renderer?.app?.view || {};
  const prediction = match?.prediction?.debugSummary?.() || {};
  return {
    matchTick: finiteOrNull(match?.lastSnapshotTick),
    selectedCount: sizeOf(state.selection),
    rememberedBuildingCount: Array.isArray(state.rememberedBuildings) ? state.rememberedBuildings.length : 0,
    visibleTileCount: countVisibleTiles(state.visibleTiles),
    viewportWidth: finiteOrNull(camera.viewW),
    viewportHeight: finiteOrNull(camera.viewH),
    cameraZoom: finiteOrNull(camera.zoom),
    canvasWidth: finiteOrNull(canvas.width ?? renderer.width),
    canvasHeight: finiteOrNull(canvas.height ?? renderer.height),
    devicePixelRatio: finiteOrNull(globalThis.window?.devicePixelRatio),
    predictionMode: String(prediction.mode || "disabled"),
    hidden: !!globalThis.document?.hidden,
    focused: typeof globalThis.document?.hasFocus === "function" ? !!globalThis.document.hasFocus() : true,
  };
}

function bucketIndex(durationMs) {
  for (let i = 0; i < DEFAULT_BUCKETS_MS.length; i += 1) {
    if (durationMs <= DEFAULT_BUCKETS_MS[i]) return i;
  }
  return DEFAULT_BUCKETS_MS.length;
}

function percentileBucket(buckets, count, percentile) {
  if (count <= 0) return 0;
  const target = Math.max(1, Math.ceil(count * percentile));
  let seen = 0;
  for (let i = 0; i < buckets.length; i += 1) {
    seen += buckets[i];
    if (seen >= target) return i < DEFAULT_BUCKETS_MS.length ? DEFAULT_BUCKETS_MS[i] : `>${DEFAULT_BUCKETS_MS.at(-1)}`;
  }
  return 0;
}

function countVisibleTiles(tiles) {
  if (!tiles || typeof tiles.length !== "number") return 0;
  let count = 0;
  for (let i = 0; i < tiles.length; i += 1) {
    if (tiles[i]) count += 1;
  }
  return count;
}

function normalizeLabel(label) {
  const normalized = String(label || "unknown").replace(/[^A-Za-z0-9_.:-]/g, "_");
  return normalized.slice(0, MAX_LABEL_LENGTH) || "unknown";
}

function clampDuration(durationMs) {
  const ms = Number(durationMs);
  if (!Number.isFinite(ms) || ms < 0) return null;
  return Math.min(ms, 60_000);
}

function finiteOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function sizeOf(value) {
  if (!value) return 0;
  if (typeof value.size === "number") return value.size;
  if (typeof value.length === "number") return value.length;
  return 0;
}

function round1(value) {
  return Number.isFinite(value) ? Math.round(value * 10) / 10 : null;
}

function sanitizeContext(context) {
  const out = {};
  for (const [key, value] of Object.entries(context)) {
    if (value == null || typeof value === "boolean" || typeof value === "string") {
      out[key] = value;
    } else if (typeof value === "number") {
      out[key] = Number.isFinite(value) ? Math.round(value * 10) / 10 : null;
    }
  }
  return out;
}
