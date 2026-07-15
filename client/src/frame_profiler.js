const DEFAULT_BUCKETS_MS = Object.freeze([1, 2, 4, 8, 12, 16, 17, 24, 33, 50, 75, 100, 150, 250, 500, 1000]);
const DEFAULT_SLOW_FRAME_MS = 33;
const DEFAULT_SLOW_PHASE_MS = 8;
const FRAME_WORK_BUDGET_MS = 1000 / 60;
const MAX_RECENT_FRAMES = 12;
const MAX_RECENT_LONG_FRAMES = 8;
const MAX_LABEL_LENGTH = 64;
const MAX_DIAGNOSTIC_COUNTERS = 160;
const MAX_FRAME_DIAGNOSTIC_COUNTERS = 8;
const MAX_REPORT_PHASE_GROUPS = 5;
const MAX_REPORT_COUNTER_GROUPS = 5;
const FRAME_UNATTRIBUTED_LABEL = "frame.unattributed";

const REPORT_FRAME_PHASE_LABELS = new Set([
  "frame.rafDispatch",
  FRAME_UNATTRIBUTED_LABEL,
  "match.healthFrameGap",
  "match.latencyRefresh",
  "match.alpha",
  "match.camera",
  "match.input",
  "match.minimapIntent",
  "match.predictionVisual",
  "match.frameEntityViews",
  "match.fog",
  "match.renderer",
  "match.hud",
  "match.minimap",
  "match.observerAnalysis",
  "match.healthPublish",
]);

const REPORT_RENDERER_PHASE_LABELS = new Set([
  "renderer.update",
  "renderer.present",
  "renderer.entityPrep",
  "renderer.feedbackView",
  "renderer.groundDecals",
  "renderer.trenches",
  "renderer.trenchOccupants",
  "renderer.miningPrep",
  "renderer.resourcesBuildings",
  "renderer.units",
  "renderer.selectionHp",
  "renderer.shotReveals",
  "renderer.sweeps",
  "renderer.effectsOverlays",
  "renderer.fogDraw",
  "renderer.feedbackOverlays",
  "renderer.placement",
]);

const REPORT_COUNTER_GROUPS = Object.freeze([
  "renderer.pixi.displayObject",
  "renderer.rig.redraw",
  "renderer.rig.instance",
  "renderer.graphics.clear",
  "renderer.redraw",
  "renderer.groundDecals",
  "renderer.trenches",
  "renderer.trenchOccupants",
  "minimap.cache",
  "minimap.invalidate",
  "entityViews.state",
  "entityViews.cache",
  "entityViews.uncached",
  "hud.dirty",
  "observer.dirty",
  "commands",
]);

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
    this.reportPhases = new Map();
    this.frameCount = 0;
    this.slowFrameCount = 0;
    this.reportFrameCount = 0;
    this.reportSlowFrameCount = 0;
    this.frameWorkBudgetMissCount = 0;
    this.presentBudgetMissCount = 0;
    this.reportFrameWorkBudgetMissCount = 0;
    this.reportPresentBudgetMissCount = 0;
    this.worstPhaseCounts = new Map();
    this.reportWorstPhaseCounts = new Map();
    this.recentFrames = [];
    this.recentLongFrames = [];
    this.latestContext = {};
    this.activeFrame = null;
    this.diagnosticCounters = new Map();
    this.reportDiagnosticCounters = new Map();
  }

  beginFrame({ at = this.now(), frameGapMs = null, scheduledAt = null } = {}) {
    if (this.activeFrame) this.endFrame({ at });
    const rafDispatchMs = Number.isFinite(scheduledAt) ? Math.max(0, at - scheduledAt) : null;
    this.activeFrame = {
      startedAt: at,
      scheduledAt: finiteOrNull(scheduledAt),
      rafDispatchMs: finiteOrNull(rafDispatchMs),
      frameGapMs: finiteOrNull(frameGapMs),
      phaseMs: new Map(),
      worstPhase: null,
      worstPhaseMs: 0,
      diagnosticCounters: new Map(),
    };
    if (Number.isFinite(frameGapMs)) {
      this.recordPhase("frame.gap", frameGapMs, { slowMs: this.slowFrameMs, includeInWorst: false });
    }
    if (Number.isFinite(rafDispatchMs)) {
      this.recordPhase("frame.rafDispatch", rafDispatchMs, { slowMs: this.slowPhaseMs });
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
    this.aggregateFor(safeLabel, this.reportPhases).add(ms, slowMs);
    if (safeLabel === "renderer.present" && ms > FRAME_WORK_BUDGET_MS) {
      this.presentBudgetMissCount += 1;
      this.reportPresentBudgetMissCount += 1;
    }
    if (includeInWorst && this.activeFrame) {
      this.activeFrame.phaseMs.set(safeLabel, (this.activeFrame.phaseMs.get(safeLabel) || 0) + ms);
      if (ms >= this.activeFrame.worstPhaseMs) {
        this.activeFrame.worstPhaseMs = ms;
        this.activeFrame.worstPhase = safeLabel;
      }
    }
  }

  recordDiagnosticCounter(label, amount = 1) {
    const safeLabel = normalizeLabel(label);
    const value = clampCounter(amount);
    if (value == null) return;
    const actualLabel = this.counterFor(safeLabel).add(value);
    this.counterFor(actualLabel, this.reportDiagnosticCounters).add(value);
    if (this.activeFrame) {
      this.activeFrame.diagnosticCounters.set(
        actualLabel,
        (this.activeFrame.diagnosticCounters.get(actualLabel) || 0) + value,
      );
    }
  }

  endFrame({ at = this.now(), context = null } = {}) {
    const frame = this.activeFrame;
    if (!frame) return null;
    if (context) this.setContext(context);
    const totalWorkMs = Math.max(0, at - frame.startedAt);
    const topLevelPhaseMs = topLevelFramePhaseTotalMs(frame.phaseMs);
    const unattributedFrameMs = Math.max(0, totalWorkMs - topLevelPhaseMs);
    this.recordPhase("frame.work", totalWorkMs, { slowMs: this.slowFrameMs, includeInWorst: false });
    this.recordPhase(FRAME_UNATTRIBUTED_LABEL, unattributedFrameMs, { slowMs: this.slowPhaseMs });
    this.activeFrame = null;
    this.frameCount += 1;
    this.reportFrameCount += 1;
    if (totalWorkMs > FRAME_WORK_BUDGET_MS) {
      this.frameWorkBudgetMissCount += 1;
      this.reportFrameWorkBudgetMissCount += 1;
    }
    const slow = (Number.isFinite(frame.frameGapMs) && frame.frameGapMs >= this.slowFrameMs)
      || totalWorkMs >= this.slowFrameMs;
    if (slow) {
      this.slowFrameCount += 1;
      this.reportSlowFrameCount += 1;
    }
    if (frame.worstPhase) {
      this.worstPhaseCounts.set(frame.worstPhase, (this.worstPhaseCounts.get(frame.worstPhase) || 0) + 1);
      this.reportWorstPhaseCounts.set(
        frame.worstPhase,
        (this.reportWorstPhaseCounts.get(frame.worstPhase) || 0) + 1,
      );
    }
    for (const [label, value] of frame.diagnosticCounters) {
      this.counterFor(label).addFrame(value);
      this.counterFor(label, this.reportDiagnosticCounters).addFrame(value);
    }
    const longFrameContext = longFrameContextFrom(frame);
    const summary = {
      at: round1(at),
      scheduledAt: round1(frame.scheduledAt),
      rafDispatchMs: round1(frame.rafDispatchMs),
      frameGapMs: round1(frame.frameGapMs),
      frameWorkMs: round1(totalWorkMs),
      topLevelPhaseMs: round1(topLevelPhaseMs),
      unattributedFrameMs: round1(unattributedFrameMs),
      slow,
      worstPhase: frame.worstPhase,
      worstPhaseMs: round1(frame.worstPhaseMs),
      topPhase: longFrameContext.topPhase,
      rendererNestedPhase: longFrameContext.rendererNestedPhase,
      minimapNestedPhase: longFrameContext.minimapNestedPhase,
      diagnosticCounters: longFrameContext.diagnosticCounters,
      context: this.latestContext,
    };
    this.recentFrames.push(summary);
    if (this.recentFrames.length > this.maxRecentFrames) {
      this.recentFrames.splice(0, this.recentFrames.length - this.maxRecentFrames);
    }
    if (slow) {
      this.recentLongFrames.push(summary);
      if (this.recentLongFrames.length > MAX_RECENT_LONG_FRAMES) {
        this.recentLongFrames.splice(0, this.recentLongFrames.length - MAX_RECENT_LONG_FRAMES);
      }
    }
    return summary;
  }

  setContext(context) {
    if (!context || typeof context !== "object") return;
    this.latestContext = sanitizeContext({ ...this.latestContext, ...context });
  }

  summary() {
    return {
      schemaVersion: 1,
      frameCount: this.frameCount,
      slowFrameCount: this.slowFrameCount,
      slowFrameMs: this.slowFrameMs,
      slowPhaseMs: this.slowPhaseMs,
      frameWorkBudgetMs: round1(FRAME_WORK_BUDGET_MS),
      frameWorkBudgetMissCount: this.frameWorkBudgetMissCount,
      presentBudgetMissCount: this.presentBudgetMissCount,
      worstPhase: worstPhaseFrom(this.worstPhaseCounts),
      context: this.latestContext,
      phases: phaseRowsFrom(this.phases),
      renderDiagnostics: diagnosticSummaryFrom(this.diagnosticCounters, this.frameCount),
      recentFrames: this.recentFrames.slice(),
      recentLongFrames: this.recentLongFrames.slice(),
    };
  }

  reportSummary() {
    const frameWork = this.reportPhases.get("frame.work");
    const frameUnattributed = this.reportPhases.get(FRAME_UNATTRIBUTED_LABEL);
    const frameRafDispatch = this.reportPhases.get("frame.rafDispatch");
    const renderer = this.reportPhases.get("match.renderer");
    const rendererUpdate = this.reportPhases.get("renderer.update");
    const rendererPresent = this.reportPhases.get("renderer.present");
    const worstFramePhase = reportWorstPhaseFrom(this.reportWorstPhaseCounts);
    const worstAggregate = worstFramePhase ? this.reportPhases.get(worstFramePhase.label) : null;
    const rendererFramePhases = reportPhaseRowsFrom(this.reportPhases, REPORT_RENDERER_PHASE_LABELS);
    const renderDiagnosticCounters = reportCounterGroupsFrom(
      this.reportDiagnosticCounters,
      this.reportFrameCount,
    );
    return {
      schemaVersion: 1,
      frameCount: this.reportFrameCount,
      slowFrameCount: this.reportSlowFrameCount,
      frameWorkBudgetMs: round1(FRAME_WORK_BUDGET_MS),
      frameWorkBudgetMissCount: this.reportFrameWorkBudgetMissCount,
      presentBudgetMissCount: this.reportPresentBudgetMissCount,
      frameWorkMaxMs: aggregateMaxMs(frameWork),
      frameWorkP95Ms: aggregatePercentileMs(frameWork, 0.95),
      frameUnattributedMaxMs: aggregateMaxMs(frameUnattributed),
      frameUnattributedP95Ms: aggregatePercentileMs(frameUnattributed, 0.95),
      frameRafDispatchMaxMs: aggregateMaxMs(frameRafDispatch),
      frameRafDispatchP95Ms: aggregatePercentileMs(frameRafDispatch, 0.95),
      worstFramePhase: worstFramePhase?.label || "",
      worstFramePhaseMs: aggregateMaxMs(worstAggregate),
      rendererMaxMs: aggregateMaxMs(renderer),
      rendererP95Ms: aggregatePercentileMs(renderer, 0.95),
      rendererUpdateMaxMs: aggregateMaxMs(rendererUpdate),
      rendererUpdateP95Ms: aggregatePercentileMs(rendererUpdate, 0.95),
      rendererPresentMaxMs: aggregateMaxMs(rendererPresent),
      rendererPresentP95Ms: aggregatePercentileMs(rendererPresent, 0.95),
      context: this.latestContext,
      clientFramePhases: reportPhaseRowsFrom(this.reportPhases, REPORT_FRAME_PHASE_LABELS),
      rendererFramePhases,
      topRendererPhase: rendererFramePhases[0]?.label || "",
      topRendererPhaseMs: rendererFramePhases[0]?.maxMs || 0,
      renderDiagnostics: diagnosticSummaryFrom(this.reportDiagnosticCounters, this.reportFrameCount),
      renderDiagnosticCounters,
      topRenderDiagnosticGroup: renderDiagnosticCounters[0]?.label || "",
      topRenderDiagnosticGroupCount: renderDiagnosticCounters[0]?.total || 0,
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
    const diagnostics = summary.renderDiagnostics?.counters || [];
    if (diagnostics.length > 0) {
      lines.push("diagnostic\tframes\ttotal\tmaxFrame\tavgPerFrame");
      for (const counter of diagnostics) {
        lines.push([
          counter.label,
          counter.frames,
          counter.total,
          counter.maxFrame,
          counter.avgPerFrame,
        ].join("\t"));
      }
    }
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
    this.reportPhases.clear();
    this.frameCount = 0;
    this.slowFrameCount = 0;
    this.reportFrameCount = 0;
    this.reportSlowFrameCount = 0;
    this.frameWorkBudgetMissCount = 0;
    this.presentBudgetMissCount = 0;
    this.reportFrameWorkBudgetMissCount = 0;
    this.reportPresentBudgetMissCount = 0;
    this.worstPhaseCounts.clear();
    this.reportWorstPhaseCounts.clear();
    this.recentFrames = [];
    this.recentLongFrames = [];
    this.activeFrame = null;
    this.diagnosticCounters.clear();
    this.reportDiagnosticCounters.clear();
  }

  resetReportWindow() {
    this.reportPhases.clear();
    this.reportFrameCount = 0;
    this.reportSlowFrameCount = 0;
    this.reportFrameWorkBudgetMissCount = 0;
    this.reportPresentBudgetMissCount = 0;
    this.reportWorstPhaseCounts.clear();
    this.reportDiagnosticCounters.clear();
  }

  debugSurface() {
    return {
      summary: () => this.summary(),
      text: () => this.text(),
      copy: () => this.copy(),
      reset: () => this.reset(),
      reportSummary: () => this.reportSummary(),
    };
  }

  aggregateFor(label, phases = this.phases) {
    let aggregate = phases.get(label);
    if (!aggregate) {
      aggregate = new PhaseAggregate();
      phases.set(label, aggregate);
    }
    return aggregate;
  }

  counterFor(label, counters = this.diagnosticCounters) {
    const safeLabel = counters.has(label) || counters.size < MAX_DIAGNOSTIC_COUNTERS
      ? label
      : "diagnostics.overflow";
    let counter = counters.get(safeLabel);
    if (!counter) {
      counter = new CounterAggregate(safeLabel);
      counters.set(safeLabel, counter);
    }
    return counter;
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

  percentileMs(percentile) {
    return percentileBucketDuration(this.buckets, this.count, percentile);
  }
}

class CounterAggregate {
  constructor(label) {
    this.label = label;
    this.samples = 0;
    this.total = 0;
    this.maxSample = 0;
    this.frames = 0;
    this.maxFrame = 0;
  }

  add(amount) {
    this.samples += 1;
    this.total += amount;
    this.maxSample = Math.max(this.maxSample, amount);
    return this.label;
  }

  addFrame(amount) {
    this.frames += 1;
    this.maxFrame = Math.max(this.maxFrame, amount);
  }

  summary(frameCount) {
    return {
      label: this.label,
      samples: this.samples,
      frames: this.frames,
      total: round1(this.total),
      maxSample: round1(this.maxSample),
      maxFrame: round1(this.maxFrame),
      avgPerFrame: frameCount > 0 ? round1(this.total / frameCount) : 0,
      avgActiveFrame: this.frames > 0 ? round1(this.total / this.frames) : 0,
    };
  }
}

export function collectMatchFrameContext(match) {
  const state = match?.state || {};
  const camera = match?.camera || {};
  const projection = camera.projectionSnapshot?.() || null;
  const cameraSnapshot = projection?.camera || camera.snapshot?.() || null;
  const renderer = match?.renderer?.app?.renderer || {};
  const canvas = match?.renderer?.app?.view || {};
  const prediction = match?.prediction?.debugSummary?.() || {};
  const mode = match?.devWatch?.kind === "scenario"
    ? "dev-scenario"
    : match?.replayViewer
      ? "replay"
      : state.spectator
        ? "spectator"
        : "live";
  return {
    matchMode: mode,
    workloadId: boundedString(globalThis.window?.__rtsPerfWorkloadId),
    matchTick: finiteOrNull(match?.lastSnapshotTick),
    entityCount: sizeOf(state._curById),
    selectedCount: sizeOf(state.selection),
    rememberedBuildingCount: Array.isArray(state.rememberedBuildings) ? state.rememberedBuildings.length : 0,
    visibleTileCount: countVisibleTiles(state.visibleTiles),
    predictedEntityCount: sizeOf(state.predictedById),
    viewportWidth: finiteOrNull(projection?.viewport?.widthCssPx),
    viewportHeight: finiteOrNull(projection?.viewport?.heightCssPx),
    cameraZoom: finiteOrNull(cameraSnapshot?.framingScale),
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

function percentileBucketDuration(buckets, count, percentile) {
  if (count <= 0) return 0;
  const target = Math.max(1, Math.ceil(count * percentile));
  let seen = 0;
  for (let i = 0; i < buckets.length; i += 1) {
    seen += buckets[i];
    if (seen >= target) {
      return i < DEFAULT_BUCKETS_MS.length ? DEFAULT_BUCKETS_MS[i] : DEFAULT_BUCKETS_MS.at(-1);
    }
  }
  return 0;
}

function phaseRowsFrom(phases) {
  return Array.from(phases.entries())
    .map(([label, aggregate]) => ({ label, ...aggregate.summary() }))
    .sort((a, b) => b.totalMs - a.totalMs || b.maxMs - a.maxMs || a.label.localeCompare(b.label));
}

function reportPhaseRowsFrom(phases, allowedLabels) {
  return Array.from(phases.entries())
    .filter(([label]) => allowedLabels.has(label))
    .map(([label, aggregate]) => {
      const summary = aggregate.summary();
      return {
        label,
        count: summary.count,
        maxMs: summary.maxMs,
        p95Ms: summary.p95Ms,
      };
    })
    .sort((a, b) => b.maxMs - a.maxMs || b.p95Ms - a.p95Ms || b.count - a.count || a.label.localeCompare(b.label))
    .slice(0, MAX_REPORT_PHASE_GROUPS);
}

function topLevelFramePhaseTotalMs(phaseMs) {
  let total = 0;
  for (const [label, ms] of phaseMs || []) {
    if (label.startsWith("match.") && Number.isFinite(ms)) total += ms;
  }
  return total;
}

function diagnosticSummaryFrom(counters, frameCount) {
  const rows = Array.from(counters.values())
    .map((counter) => counter.summary(frameCount))
    .sort((a, b) => b.total - a.total || b.maxFrame - a.maxFrame || a.label.localeCompare(b.label));
  return {
    schemaVersion: 1,
    counters: rows,
    topCounters: rows.slice(0, MAX_FRAME_DIAGNOSTIC_COUNTERS),
  };
}

function reportCounterGroupsFrom(counters, frameCount) {
  const groups = new Map();
  for (const counter of counters.values()) {
    const groupLabel = reportCounterGroupFor(counter.label);
    if (!groupLabel) continue;
    const group = groups.get(groupLabel) || {
      label: groupLabel,
      samples: 0,
      frames: 0,
      total: 0,
      maxFrame: 0,
    };
    group.samples += counter.samples;
    group.frames += counter.frames;
    group.total += counter.total;
    group.maxFrame = Math.max(group.maxFrame, counter.maxFrame);
    groups.set(groupLabel, group);
  }
  return Array.from(groups.values())
    .map((group) => ({
      label: group.label,
      samples: group.samples,
      frames: Math.min(group.frames, frameCount),
      total: round1(group.total),
      maxFrame: round1(group.maxFrame),
    }))
    .sort((a, b) => b.total - a.total || b.maxFrame - a.maxFrame || a.label.localeCompare(b.label))
    .slice(0, MAX_REPORT_COUNTER_GROUPS);
}

function reportCounterGroupFor(label) {
  for (const group of REPORT_COUNTER_GROUPS) {
    if (label === group || label.startsWith(`${group}.`)) return group;
  }
  return null;
}

function worstPhaseFrom(counts) {
  return Array.from(counts.entries())
    .map(([label, count]) => ({ label, count }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))[0] || null;
}

function reportWorstPhaseFrom(counts) {
  return Array.from(counts.entries())
    .filter(([label]) => REPORT_FRAME_PHASE_LABELS.has(label) || REPORT_RENDERER_PHASE_LABELS.has(label))
    .map(([label, count]) => ({ label, count }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))[0] || null;
}

function longFrameContextFrom(frame) {
  return {
    topPhase: slowestPhaseFrom(frame.phaseMs, (label) => label.startsWith("match.")),
    rendererNestedPhase: slowestPhaseFrom(frame.phaseMs, (label) => label.startsWith("renderer.")),
    minimapNestedPhase: slowestPhaseFrom(frame.phaseMs, (label) => label.startsWith("minimap.")),
    diagnosticCounters: topFrameCounters(frame.diagnosticCounters),
  };
}

function slowestPhaseFrom(phaseMs, predicate) {
  let best = null;
  for (const [label, ms] of phaseMs || []) {
    if (!predicate(label)) continue;
    if (!best || ms > best.ms || (ms === best.ms && label < best.label)) {
      best = { label, ms };
    }
  }
  return best ? { label: best.label, ms: round1(best.ms) } : null;
}

function topFrameCounters(counters) {
  return Array.from(counters || [])
    .map(([label, total]) => ({ label, total: round1(total) }))
    .sort((a, b) => b.total - a.total || a.label.localeCompare(b.label))
    .slice(0, MAX_FRAME_DIAGNOSTIC_COUNTERS);
}

function aggregateMaxMs(aggregate) {
  return aggregate ? round1(aggregate.maxMs) : 0;
}

function aggregatePercentileMs(aggregate, percentile) {
  return aggregate ? aggregate.percentileMs(percentile) : 0;
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

function clampCounter(amount) {
  const value = Number(amount);
  if (!Number.isFinite(value) || value <= 0) return null;
  return Math.min(value, 1_000_000);
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
      out[key] = typeof value === "string" ? boundedString(value) : value;
    } else if (typeof value === "number") {
      out[key] = Number.isFinite(value) ? Math.round(value * 10) / 10 : null;
    }
  }
  return out;
}

function boundedString(value) {
  if (typeof value !== "string") return "";
  return value.slice(0, MAX_LABEL_LENGTH);
}
