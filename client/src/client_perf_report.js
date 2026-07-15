import { ReportWindowAggregate } from "./report_window_aggregate.js";

export function clientPerfReportFields(frameProfiler) {
  const perf = frameProfiler?.reportSummary?.() || {};
  const context = perf.context || {};
  return {
    frameWorkMaxMs: clampU16(perf.frameWorkMaxMs),
    frameWorkP95Ms: clampU16(perf.frameWorkP95Ms),
    frameRafDispatchMaxMs: clampU16(perf.frameRafDispatchMaxMs),
    frameRafDispatchP95Ms: clampU16(perf.frameRafDispatchP95Ms),
    frameUnattributedMaxMs: clampU16(perf.frameUnattributedMaxMs),
    frameUnattributedP95Ms: clampU16(perf.frameUnattributedP95Ms),
    slowFrameCount: clampU32(perf.slowFrameCount),
    frameWorkBudgetMissCount: clampU32(perf.frameWorkBudgetMissCount),
    presentBudgetMissCount: clampU32(perf.presentBudgetMissCount),
    worstFramePhase: clampReportLabel(perf.worstFramePhase),
    worstFramePhaseMs: clampU16(perf.worstFramePhaseMs),
    rendererMaxMs: clampU16(perf.rendererMaxMs),
    rendererP95Ms: clampU16(perf.rendererP95Ms),
    rendererUpdateMaxMs: clampU16(perf.rendererUpdateMaxMs),
    rendererUpdateP95Ms: clampU16(perf.rendererUpdateP95Ms),
    rendererPresentMaxMs: clampU16(perf.rendererPresentMaxMs),
    rendererPresentP95Ms: clampU16(perf.rendererPresentP95Ms),
    topRendererPhase: clampReportLabel(perf.topRendererPhase),
    topRendererPhaseMs: clampU16(perf.topRendererPhaseMs),
    topRenderDiagnosticGroup: clampReportLabel(perf.topRenderDiagnosticGroup),
    topRenderDiagnosticGroupCount: clampU32(perf.topRenderDiagnosticGroupCount),
    clientFramePhases: clampPhaseRows(perf.clientFramePhases),
    rendererFramePhases: clampPhaseRows(perf.rendererFramePhases),
    renderDiagnosticCounters: clampCounterRows(perf.renderDiagnosticCounters),
    entityCount: clampU32(context.entityCount),
    selectedCount: clampU16(context.selectedCount),
    visibleTileCount: clampU32(context.visibleTileCount),
    viewportWidth: clampU16(context.viewportWidth),
    viewportHeight: clampU16(context.viewportHeight),
    devicePixelRatioX100: clampU16(Number(context.devicePixelRatio) * 100),
  };
}

export function createSnapshotProcessingReport() {
  return new SnapshotProcessingReport();
}

export function recordSnapshotProcessing(report, reconcilePrediction, applySnapshot, applyPredictionOverlay) {
  let predictionMs = report.measure(reconcilePrediction);
  report.recordSnapshotApply(applySnapshot);
  predictionMs += report.measure(applyPredictionOverlay);
  report.recordPredictionApply(predictionMs);
}

export function snapshotReportFields({ reportStats, transportStats, snapshotProcessing }) {
  const snapshotApply = snapshotProcessing?.snapshotApplySummary() || {};
  const predictionApply = snapshotProcessing?.predictionApplySummary() || {};
  return {
    snapshotBytesTotal: clampU32(transportStats?.snapshotBytesTotal),
    snapshotBytesMax: clampU32(transportStats?.snapshotBytesMax),
    snapshotBytesAvg: clampU32(transportStats?.snapshotBytesAvg),
    snapshotMessageCount: clampU32(transportStats?.snapshotMessageCount),
    snapshotByteSource: clampReportLabel(transportStats?.snapshotByteSource),
    snapshotCodec: clampReportLabel(transportStats?.snapshotCodec),
    snapshotCodecVersion: clampU16(transportStats?.snapshotCodecVersion),
    snapshotFrameKind: clampReportLabel(transportStats?.snapshotFrameKind),
    snapshotBytesP95: clampU32(transportStats?.snapshotBytesP95),
    snapshotSegmentBudgetBytes: clampU32(transportStats?.snapshotSegmentBudgetBytes),
    snapshotOverSegmentBudgetCount: clampU32(transportStats?.snapshotOverSegmentBudgetCount),
    snapshotOverSegmentBudgetPctX100: clampU16(transportStats?.snapshotOverSegmentBudgetPctX100),
    snapshotParseMaxMs: clampU16(transportStats?.snapshotParseMaxMs),
    snapshotParseP95Ms: clampU16(transportStats?.snapshotParseP95Ms),
    snapshotDecodeMaxMs: clampU16(transportStats?.snapshotDecodeMaxMs),
    snapshotDecodeP95Ms: clampU16(transportStats?.snapshotDecodeP95Ms),
    websocketExtensions: clampReportText(transportStats?.websocketExtensions),
    websocketCompression: clampReportLabel(transportStats?.websocketCompression),
    snapshotApplyMaxMs: clampU16(snapshotApply.max),
    snapshotApplyP95Ms: clampU16(snapshotApply.p95),
    predictionApplyMaxMs: clampU16(predictionApply.max),
    predictionApplyP95Ms: clampU16(predictionApply.p95),
    snapshotTickGapMax: clampU32(reportStats?.snapshotTickGapMax),
    staleSnapshotCount: clampU32(reportStats?.staleSnapshotCount),
    duplicateSnapshotCount: clampU32(reportStats?.duplicateSnapshotCount),
    skippedSnapshotCount: clampU32(reportStats?.skippedSnapshotCount),
    snapshotBurstCount: clampU32(reportStats?.snapshotBurstCount),
    snapshotBurstMax: clampU32(reportStats?.snapshotBurstMax),
  };
}

class SnapshotProcessingReport {
  constructor() {
    this.snapshotApplyMs = new ReportWindowAggregate();
    this.predictionApplyMs = new ReportWindowAggregate();
  }

  measure(fn) {
    const startedAt = performance.now();
    fn();
    return performance.now() - startedAt;
  }

  recordSnapshotApply(fn) {
    const elapsedMs = this.measure(fn);
    this.snapshotApplyMs.add(elapsedMs);
    return elapsedMs;
  }

  recordPredictionApply(elapsedMs) {
    this.predictionApplyMs.add(elapsedMs);
  }

  snapshotApplySummary() {
    return this.snapshotApplyMs.summary();
  }

  predictionApplySummary() {
    return this.predictionApplyMs.summary();
  }

  reset() {
    this.snapshotApplyMs.reset();
    this.predictionApplyMs.reset();
  }
}

function clampU16(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(65535, Math.round(n));
}

function clampU32(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(4294967295, Math.round(n));
}

function clampReportLabel(value) {
  return String(value || "").replace(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 64);
}

function clampReportText(value) {
  return String(value || "").replace(/[^A-Za-z0-9_.:;=, -]/g, "_").slice(0, 128);
}

function clampPhaseRows(rows) {
  if (!Array.isArray(rows)) return [];
  return rows.slice(0, 5).map((row) => ({
    label: clampReportLabel(row?.label),
    count: clampU32(row?.count),
    maxMs: clampU16(row?.maxMs),
    p95Ms: clampU16(row?.p95Ms),
  }));
}

function clampCounterRows(rows) {
  if (!Array.isArray(rows)) return [];
  return rows.slice(0, 5).map((row) => ({
    label: clampReportLabel(row?.label),
    samples: clampU32(row?.samples),
    frames: clampU32(row?.frames),
    total: clampU32(row?.total),
    maxFrame: clampU32(row?.maxFrame),
  }));
}
