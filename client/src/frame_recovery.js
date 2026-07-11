import { collectMatchFrameContext } from "./frame_profiler.js";
import { buildFrameEntityViews } from "./frame_entity_views.js";

const FRAME_ERROR_LOG_INTERVAL_MS = 5000;

export function createFrameErrorState() {
  return { count: 0, lastLogAt: -Infinity, lastMessage: "" };
}

export function runMatchFrameSafely(match, now) {
  if (!match.running) return;

  try {
    runMatchFrame(match, now);
  } catch (err) {
    recordFrameError(match.frameErrors, err);
  } finally {
    if (match.running) match.rafId = requestAnimationFrame(match.tickFn);
  }
}

export function runMatchCaptureFrame(match, now) {
  if (!match.running) return;
  runMatchFrame(match, now, { capture: true });
}

function runMatchFrame(match, now, { capture = false } = {}) {
  const frameStartedAt = match.frameProfiler?.now?.() ?? now;
  const dt = (now - match.lastFrame) / 1000;
  const frameGapMs = now - match.lastFrame;
  match.frameProfiler?.beginFrame({ at: frameStartedAt, frameGapMs, scheduledAt: now });
  const time = (label, fn) => match.frameProfiler ? match.frameProfiler.time(label, fn) : fn();
  match.lastFrame = now;
  try {
    if (!capture && Number.isFinite(frameGapMs) && frameGapMs >= 0) {
      time("match.healthFrameGap", () => match.health.noteFrameGap(frameGapMs, now));
    }
    if (!capture) time("match.latencyRefresh", () => match.health.refreshLatency());

    const alpha = capture ? 1 : time("match.alpha", () => match.computeAlpha());

    time("match.camera", () => {
      if (!capture) match.camera.update(dt, match.input);
      if (match.audio) {
        match.audio.setListener(
          match.camera.x + match.camera.viewW / (2 * match.camera.zoom),
          match.camera.y + match.camera.viewH / (2 * match.camera.zoom),
          match.camera.zoom,
          match.camera.viewW,
        );
      }
    });
    if (!capture) time("match.input", () => match.input.update(dt));
    time("match.minimapIntent", () => match.minimap.updateCommandTargetPreview?.());
    time("match.predictionVisual", () => match.advancePredictionVisual());
    const frameViews = time(
      "match.frameEntityViews",
      () => buildFrameEntityViews(match.state, { alpha }),
    );
    match.frameProfiler?.setContext({
      frameEntityViewCalls: frameViews.debug.entitiesInterpolatedCalls,
      frameSelectedEntityCalls: frameViews.debug.selectedEntitiesCalls,
    });
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.entitiesInterpolatedCalls",
      frameViews.debug.entitiesInterpolatedCalls,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.selectedEntitiesCalls",
      frameViews.debug.selectedEntitiesCalls,
    );
    time("match.fog", () => {
      match.fog.update(frameViews.fogSourceEntities, match.state.map.tileSize, match.state.visibleTiles);
    });

    time("match.renderer", () => {
      match.renderer.render(match.state, match.camera, match.fog, alpha, {
        clientIntent: match.clientIntent,
        frameViews,
        profiler: match.frameProfiler,
        visualSamples: match.visualProfile?.staticSamples || null,
        visualUnitOverrides: match.visualProfile?.unitOverrides || null,
        visualFrameStripOverrides: match.visualProfile?.frameStripOverrides || null,
        observerMapAnalysis: match.observerDiagnostics?.mapOverlayModel?.() || null,
      });
    });
    time("match.hud", () => match.hud.update(frameViews, { profiler: match.frameProfiler }));
    time("match.minimap", () => match.minimap.render(frameViews, { profiler: match.frameProfiler }));
    time("match.observerAnalysis", () => match.observerDiagnostics?.update(frameViews, { profiler: match.frameProfiler }));
    if (!capture) time("match.healthPublish", () => match.health.publish());
  } finally {
    const frameSummary = match.frameProfiler?.endFrame({ context: collectMatchFrameContext(match) });
    if (!capture) {
      match.health?.noteFrameSummary?.(frameSummary, {
        predictedSnapshotPresent: (match.state?.predictedById?.size || 0) > 0,
      });
    }
  }
}

function recordFrameError(state, err) {
  const now = typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
  state.count += 1;
  state.lastMessage = err?.stack || err?.message || String(err);
  globalThis.__rtsFrameErrors = {
    count: state.count,
    latest: state.lastMessage,
  };
  if (state.count <= 3 || now - state.lastLogAt >= FRAME_ERROR_LOG_INTERVAL_MS) {
    state.lastLogAt = now;
    console.error("[RTS_FRAME] recovered from frame error", err);
  }
}
