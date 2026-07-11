export function selectInitialCameraView({
  currentView = null,
  pendingView = null,
  visualProfileView = null,
  scenarioView = null,
} = {}) {
  return currentView || pendingView || visualProfileView || scenarioView || null;
}

export function restoreInitialCameraView(camera, initialCamera) {
  if (!camera || !initialCamera) return false;
  const centerX = Number(initialCamera.centerX);
  const centerY = Number(initialCamera.centerY);
  if (Number.isFinite(centerX) && Number.isFinite(centerY)) {
    const current = camera.snapshot();
    return camera.restore({
      version: 1,
      focus: { x: centerX, y: centerY },
      framingScale: current.framingScale,
      boundsPolicy: "mapOverscroll",
    });
  }
  return camera.restore(initialCamera);
}
