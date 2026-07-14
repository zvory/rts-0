export function requestPauseGame(match) {
  if (!match.capabilities.matchControls?.pause) return;
  if (match.livePauseState.paused || !match.livePauseState.canPause) {
    match.syncLivePauseUi();
    return;
  }
  match.closeSettingsMenu();
  match.net.pauseGame();
  match.livePauseState = { ...match.livePauseState, canPause: false };
  match.syncLivePauseUi();
}

export function requestUnpauseGame(match) {
  if (!match.capabilities.matchControls?.pause) return;
  if (!match.livePauseState.paused || !match.livePauseState.canUnpause) return;
  match.net.unpauseGame();
  match.livePauseState = { ...match.livePauseState, canUnpause: false };
  match.syncLivePauseUi();
}

export function applyLivePauseState(match, state) {
  const wasPaused = match.livePauseState.paused === true;
  match.livePauseState = {
    paused: state?.paused === true,
    pausedBy: Number.isInteger(state?.pausedBy) ? state.pausedBy : null,
    pausesRemaining: Number.isInteger(state?.pausesRemaining) ? state.pausesRemaining : null,
    pauseLimit: Number.isInteger(state?.pauseLimit) ? state.pauseLimit : null,
    canPause: state?.canPause === true,
    canUnpause: state?.canUnpause === true,
  };
  if (match.livePauseState.paused) {
    suspendPredictionVisuals(match);
  } else if (wasPaused) {
    match.predictionVisualSuspended = true;
    pausePredictionVisualClock(match);
    clearPredictedMovementOverlay(match);
  }
  match.livePauseOverlay?.applyLivePauseState(match.livePauseState);
  match.syncLivePauseUi();
}

export function predictionVisualsPaused(match) {
  return match.livePauseState?.paused === true || match.predictionVisualSuspended === true;
}

export function notePredictionAuthoritativeSnapshot(match) {
  if (match.livePauseState.paused) return;
  match.predictionVisualSuspended = false;
  match.state?.setProgressPredictionPaused?.(false);
}

export function pausePredictionVisualClock(match) {
  match.predictionAdapter?.pauseVisualClock?.();
  match.state?.setProgressPredictionPaused?.(true);
}

export function suspendPredictionVisuals(match) {
  match.predictionVisualSuspended = true;
  pausePredictionVisualClock(match);
  clearPredictedMovementOverlay(match);
  match.publishPredictionDebug();
}

export function clearPredictedMovementOverlay(match) {
  match.applyPredictionDisplayOverlay({ predictedSnapshot: null });
}

export function livePauseActionLabel(match) {
  const remaining = match.livePauseState.pausesRemaining;
  if (Number.isInteger(remaining)) return `Pause (${remaining})`;
  return "Pause";
}

export function livePauseActionTitle(match) {
  const remaining = match.livePauseState.pausesRemaining;
  if (!Number.isInteger(remaining)) return "Pause the live match.";
  if (remaining <= 0) return "No pauses remaining.";
  return `${remaining} pause${remaining === 1 ? "" : "s"} remaining.`;
}
