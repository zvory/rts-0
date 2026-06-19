export function clientPerfReportFields(frameProfiler) {
  const perf = frameProfiler?.reportSummary?.() || {};
  const context = perf.context || {};
  return {
    frameWorkMaxMs: clampU16(perf.frameWorkMaxMs),
    frameWorkP95Ms: clampU16(perf.frameWorkP95Ms),
    slowFrameCount: clampU32(perf.slowFrameCount),
    worstFramePhase: clampReportLabel(perf.worstFramePhase),
    worstFramePhaseMs: clampU16(perf.worstFramePhaseMs),
    rendererMaxMs: clampU16(perf.rendererMaxMs),
    rendererP95Ms: clampU16(perf.rendererP95Ms),
    entityCount: clampU32(context.entityCount),
    selectedCount: clampU16(context.selectedCount),
    visibleTileCount: clampU32(context.visibleTileCount),
    viewportWidth: clampU16(context.viewportWidth),
    viewportHeight: clampU16(context.viewportHeight),
    devicePixelRatioX100: clampU16(Number(context.devicePixelRatio) * 100),
  };
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
