export function selectInitialCameraView({
  currentView = null,
  pendingView = null,
  visualProfileView = null,
  scenarioView = null,
} = {}) {
  return currentView || pendingView || visualProfileView || scenarioView || null;
}
