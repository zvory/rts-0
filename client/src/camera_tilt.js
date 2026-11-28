// Experimental ground-plane homography. Angle is degrees away from top-down.
export function mapTiltDegrees(value) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.min(45, number)) : 0;
}

export function tiltScreen(state, point, inverse = false) {
  const angle = mapTiltDegrees(state.mapTilt) * Math.PI / 180;
  if (!angle || !state.viewH) return { x: point.x, y: point.y };
  const cx = state.viewW / 2;
  const cy = state.viewH / 2;
  const sinDistance = Math.sin(angle) / (state.viewH * 3);
  const cosine = Math.cos(angle);
  const x = point.x - cx;
  const y = point.y - cy;
  if (inverse) {
    const groundY = y / (cosine + y * sinDistance);
    const depth = 1 - groundY * sinDistance;
    return { x: cx + x * depth, y: cy + groundY };
  }
  const depth = 1 - y * sinDistance;
  return { x: cx + x / depth, y: cy + y * cosine / depth };
}
