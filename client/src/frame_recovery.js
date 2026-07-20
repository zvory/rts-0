import { collectMatchFrameContext } from "./frame_profiler.js";
import { buildFrameEntityViews } from "./frame_entity_views.js";
import { buildSelectionScene } from "./input/selection_projection.js";
import { prepareEntitySnapshots } from "./presentation/entity_snapshot.js";
import { buildRendererFeedbackView } from "./renderer/feedback_view_model.js";
import { PresentationFrameAssembler } from "./presentation/frame.js";
import { PresentationCoordinator } from "./presentation/coordinator.js";
import { PRESENTATION_OUTCOME, immediatePresentationSubmission } from "./presentation/submission.js";
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
  return runMatchFrame(match, now, { capture: true });
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
      if (!capture) match.autoSpectator?.update(dt);
      if (match.audio) {
        const listener = match.camera.audioListener?.();
        if (listener) match.audio.setListener(listener);
      }
    });
    if (!capture) time("match.input", () => match.input.update(dt));
    time(
      "match.minimapIntent",
      () => match.minimap.updateCommandTargetPreview?.(match.input?.isShiftHeld?.()),
    );
    time("match.predictionVisual", () => match.advancePredictionVisual());
    const frameViews = time("match.frameEntityViews", () => {
      const views = buildFrameEntityViews(match.state, { alpha });
      const prepared = prepareEntitySnapshots(views.interpolatedEntities);
      return Object.freeze({
        ...views,
        preparedEntities: prepared.entries,
        preparationDebug: prepared.debug,
      });
    });
    match.frameProfiler?.setContext({
      frameEntityVariantBuildCalls: frameViews.debug.entityVariantBuildCalls,
      frameEntityTraversals: frameViews.debug.entityTraversals,
      frameEntityViewCalls: frameViews.debug.entitiesInterpolatedCalls,
      frameSelectedEntityCalls: frameViews.debug.selectedEntitiesCalls,
    });
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.entityVariantBuildCalls",
      frameViews.debug.entityVariantBuildCalls,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.entityTraversals",
      frameViews.debug.entityTraversals,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.entitiesInterpolatedCalls",
      frameViews.debug.entitiesInterpolatedCalls,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.state.selectedEntitiesCalls",
      frameViews.debug.selectedEntitiesCalls,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.preparation.interactionContainers",
      frameViews.preparationDebug.interactionObjects + frameViews.preparationDebug.interactionArrays,
    );
    match.frameProfiler?.recordDiagnosticCounter?.(
      "entityViews.preparation.admittedNestedReuses",
      frameViews.preparationDebug.admittedNestedReuses,
    );
    time("match.fog", () => {
      match.fog.update(
        frameViews.fogSourceEntities,
        match.state.map.tileSize,
        match.state.visibleTiles,
        match.state.exploredTiles,
      );
    });

    const projection = match.camera.projectionSnapshot();
    const visualTimeMs = match.renderClock?.now?.() ?? now;
    const visualSamples = match.visualProfile?.staticSamples || [];
    const observerMapAnalysis = match.observerDiagnostics?.mapOverlayModel?.() || null;
    const screenOverlay = match.input?.screenOverlay?.snapshot?.() || null;
    const feedbackView = time(
      "match.rendererFeedbackView",
      () => buildRendererFeedbackView(match.state, {
        controlPolicy: match.controlPolicy,
        clientIntent: match.clientIntent,
        previewSurface: match.inputRouter?.activePreviewSurface?.(),
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
    const groundDecalBatch = time(
      "match.groundDecalReconciliation",
      () => match.state.reconcilePendingGroundDecals?.() || [],
    );
    const groundDecals = Array.isArray(groundDecalBatch)
      ? groundDecalBatch
      : groundDecalBatch.decals || [];
    const groundDecalRevision = Array.isArray(groundDecalBatch)
      ? 0
      : groundDecalBatch.revision || 0;
    const presentationFrame = time("match.presentationFrame", () => presentationAssembler.assemble({
      map: match.state.map,
      frameContext: frameViews,
      projection,
      fog: match.fog,
      feedback: feedbackView,
      rememberedBuildings: match.state.rememberedBuildings,
      trenches: match.state.trenches,
      groundDecals,
      groundDecalRevision,
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
    match.frameProfiler?.recordDiagnosticCounter?.("presentation.frames.assembled", 1);
    match.frameProfiler?.recordDiagnosticCounter?.(
      "presentation.records.dropped",
      presentationFrame.diagnosticsContext.droppedRecords,
    );

    const selectionScene = time("match.selectionScene", () => buildSelectionScene({
      entities: frameViews.interpolatedEntities,
      preparedEntities: frameViews.preparedEntities,
      projection,
      tileSize: match.state.map?.tileSize,
      generation: presentationFrame.generation,
      frameId: presentationFrame.frameId,
    }));

    let submission;
    try {
      submission = time("match.renderer", () => match.renderer.render(presentationFrame));
    } catch (error) {
      submission = immediatePresentationSubmission({
        generation: presentationFrame.generation,
        frameId: presentationFrame.frameId,
        status: PRESENTATION_OUTCOME.FAILED,
        error,
      });
    }
    const presentation = presentationCoordinatorFor(match).submit({
      frame: presentationFrame,
      selectionScene,
      submission,
    });
    time("match.hud", () => match.hud.update(frameViews, { profiler: match.frameProfiler }));
    time("match.minimap", () => match.minimap.render(frameViews, { profiler: match.frameProfiler }));
    time("match.observerAnalysis", () => match.observerDiagnostics?.update(frameViews, { profiler: match.frameProfiler }));
    if (!capture) time("match.healthPublish", () => match.health.publish());
    return presentation;
  } finally {
    const frameSummary = match.frameProfiler?.endFrame({ context: collectMatchFrameContext(match) });
    if (!capture) {
      match.health?.noteFrameSummary?.(frameSummary, {
        predictedSnapshotPresent: (match.state?.predictedById?.size || 0) > 0,
      });
    }
  }
}

export function recordFrameError(state, err) {
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

function presentationCoordinatorFor(match) {
  if (match.presentationCoordinator) return match.presentationCoordinator;
  match.presentationCoordinator = new PresentationCoordinator({
    publishSelectionScene: (scene) => match.input?.publishSelectionScene?.(scene),
    acknowledgeGroundDecals: (revision) => match.state?.acknowledgeReconciledGroundDecals?.(revision),
    recordCounter: (label, amount) => match.frameProfiler?.recordDiagnosticCounter?.(label, amount),
    recordFailure: (error) => {
      recordFrameError(
        match.frameErrors || (match.frameErrors = createFrameErrorState()),
        new Error(error?.message || "Renderer failed a presentation frame."),
      );
      if (match.renderer?.terminalFailure?.()) {
        if (typeof match.stop === "function") match.stop();
        else match.running = false;
      }
    },
    recordProtocolError: (message) => recordFrameError(
      match.frameErrors || (match.frameErrors = createFrameErrorState()),
      new Error(`Renderer protocol error: ${message}`),
    ),
  });
  return match.presentationCoordinator;
}
