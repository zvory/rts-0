export function newestPredictionSnapshot(current, candidate) {
  const candidateTick = Number(candidate?.tick);
  const currentTick = Number(current?.tick);
  return Number.isFinite(candidateTick) && (!Number.isFinite(currentTick) || candidateTick >= currentTick)
    ? candidate
    : current;
}

export function usablePredictionAdapter(match) {
  if (match.predictionAdapter?.disabledReason) match.resetPredictionAdapter();
  return match.predictionAdapter;
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
      else if (match.progressPredictionEligible) {
        try {
          adapter.reconcile(snapshot, []);
        } catch {
          match.prediction.recordDisableReason("progress-reconcile-failed");
          match.state?.clearPredictionFrame?.();
          match.logPredictionStatus("progress-reconcile-failed");
          if (remountSettings) match.mountSettings({ keepOpen: true });
          return;
        }
      }
      match.applyPredictionFrame();
    }
    match.logPredictionStatus("ready");
  } else {
    match.prediction.recordDisableReason("wasm-unavailable");
    match.logPredictionStatus("disabled");
  }
  if (remountSettings) match.mountSettings({ keepOpen: true });
}

export function recoverPredictionRuntimeAfterBudget(match, diagnostics) {
  if (!match.prediction.enabled) return false;
  if (diagnostics?.lastReplayBudgetExceeded !== true) {
    match.predictionBudgetRecoveryAttempted = false;
    return false;
  }
  if (match.predictionBudgetRecoveryAttempted) return false;
  match.predictionBudgetRecoveryAttempted = true;
  match.prediction.recordReplayBudgetExceeded({
    elapsedMs: diagnostics.lastTickMs,
    replayTicks: diagnostics.lastReplayTicks,
  });
  match.prediction.reset({ enabled: true, preserveClientSeq: true, reason: "replay-budget-exceeded" });
  match.resetPredictionAdapter();
  match.applyPredictionDisplayOverlay({ predictionFrame: null });
  match.initPredictionAdapter();
  match.publishPredictionDebug();
  match.logPredictionStatus("tracking-replay-budget-exceeded");
  return true;
}
