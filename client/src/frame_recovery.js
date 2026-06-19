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

function runMatchFrame(match, now) {
  const dt = (now - match.lastFrame) / 1000;
  const frameGapMs = now - match.lastFrame;
  match.lastFrame = now;
  if (Number.isFinite(frameGapMs) && frameGapMs >= 0) {
    match.health.noteFrameGap(frameGapMs, now);
  }
  match.health.refreshLatency();

  const alpha = match.computeAlpha();

  match.camera.update(dt, match.input);
  if (match.audio) {
    match.audio.setListener(
      match.camera.x + match.camera.viewW / (2 * match.camera.zoom),
      match.camera.y + match.camera.viewH / (2 * match.camera.zoom),
      match.camera.zoom,
      match.camera.viewW,
    );
  }
  match.input.update(dt);
  match.advancePredictionVisual();
  match.fog.update(match.ownEntities(), match.state.map.tileSize, match.state.visibleTiles);

  match.renderer.render(match.state, match.camera, match.fog, alpha, {
    clientIntent: match.clientIntent,
  });
  match.hud.update();
  match.minimap.render();
  match.observerAnalysisOverlay?.update();
  match.health.publish();
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
