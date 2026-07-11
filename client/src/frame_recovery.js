import { collectMatchFrameContext } from "./frame_profiler.js";
import { buildFrameEntityViews } from "./frame_entity_views.js";
import { buildSelectionScene } from "./input/selection_projection.js";
import { buildRendererFeedbackView } from "./renderer/feedback_view_model.js";
import { PresentationFrameAssembler } from "./presentation/frame.js";
import { STATS } from "./config.js";

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
        const listener = match.camera.audioListener?.();
        if (listener) match.audio.setListener(listener);
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

    const projection = match.camera.projectionSnapshot();
    const visualTimeMs = match.renderClock?.now?.() ?? now;
    const visualSamples = match.visualProfile?.staticSamples || [];
    const observerMapAnalysis = match.observerDiagnostics?.mapOverlayModel?.() || null;
    const screenOverlay = match.input?.screenOverlay?.snapshot?.() || null;
    const feedbackView = time(
      "match.rendererFeedbackView",
      () => buildRendererFeedbackView(match.state, {
        clientIntent: match.clientIntent,
        entities: frameViews.interpolatedEntities,
        selectedEntities: frameViews.selectedEntities,
        now: visualTimeMs,
      }),
    );
    const presentationAssembler = match.presentationAssembler || new PresentationFrameAssembler({
      map: match.state.map,
      entityStats: STATS,
    });
    match.presentationAssembler = presentationAssembler;
    const groundDecals = time(
      "match.groundDecalReconciliation",
      () => match.state.reconcilePendingGroundDecals?.() || [],
    );
    const presentationFrame = time("match.presentationFrame", () => presentationAssembler.assemble({
      map: match.state.map,
      frameContext: frameViews,
      projection,
      fog: match.fog,
      feedback: feedbackView,
      rememberedBuildings: match.state.rememberedBuildings,
      trenches: match.state.trenches,
      groundDecals,
      selectionIds: match.state.selection,
      players: match.state.players,
      playerId: match.state.playerId,
      spectator: match.state.spectator,
      visualSamples,
      observerMapAnalysis,
      screenOverlay,
      visualTimeMs,
      mode: capture ? "fixedCapture" : "live",
      sourceTick: match.state.tick,
    }));
    match.presentationFrame = presentationFrame;
    match.staticMapPresentation = presentationAssembler.staticMap;
    match.frameProfiler?.recordDiagnosticCounter?.("presentation.frames.assembled", 1);
    match.frameProfiler?.recordDiagnosticCounter?.(
      "presentation.records.dropped",
      presentationFrame.diagnosticsContext.droppedRecords,
    );

    const selectionScene = time("match.selectionScene", () => buildSelectionScene({
      entities: frameViews.interpolatedEntities,
      projection,
      tileSize: match.state.map?.tileSize,
      generation: presentationFrame.generation,
      frameId: presentationFrame.frameId,
    }));

    const renderResult = time("match.renderer", () => match.renderer.render(presentationFrame));
    if (renderResult?.presented !== false) match.input?.publishSelectionScene?.(selectionScene);
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
