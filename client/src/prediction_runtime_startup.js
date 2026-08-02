export function newestPredictionSnapshot(current, candidate) {
  const candidateTick = Number(candidate?.tick);
  const currentTick = Number(current?.tick);
  return Number.isFinite(candidateTick) && (!Number.isFinite(currentTick) || candidateTick >= currentTick)
    ? candidate
    : current;
}

export function finishPredictionRuntimeInit(match, { token, adapter, ready, remountSettings }) {
  if (token !== match.predictionInitToken) {
    if (adapter !== match.predictionAdapter) adapter.destroy();
    return;
  }
  if (!match.predictionRuntimeEnabled()) {
    adapter.destroy();
    match.publishPredictionDebug();
    if (remountSettings) match.mountSettings({ keepOpen: true });
    return;
  }
  if (ready) {
    const snapshot = match.latestPredictionSnapshot;
    if (snapshot) {
      if (match.prediction.enabled) match.prediction.reconcilePredictor(snapshot);
      else if (match.progressPredictionEligible) adapter.reconcile(snapshot, []);
      match.applyPredictionFrame();
    }
    match.logPredictionStatus("ready");
  } else {
    match.prediction.recordDisableReason("wasm-unavailable");
    match.logPredictionStatus("disabled");
  }
  if (remountSettings) match.mountSettings({ keepOpen: true });
}
