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
  const centerX = initialCamera.centerX;
  const centerY = initialCamera.centerY;
  if (Number.isFinite(centerX) && Number.isFinite(centerY)) {
    const current = camera.snapshot();
    const legacyZoom = initialCamera.zoom;
    return camera.restore({
      version: 1,
      focus: { x: centerX, y: centerY },
      // Server-authored scenario views set only a center. Preserve the old
      // `{ centerX, centerY, zoom }` launch shape too, because Match used to
      // pass it straight to Camera#setView.
      framingScale: Number.isFinite(legacyZoom) && legacyZoom > 0
        ? legacyZoom
        : current.framingScale,
      boundsPolicy: "mapOverscroll",
    });
  }
  return camera.restore(initialCamera);
}
