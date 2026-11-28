// Experimental ground-plane homography. Angle is degrees away from top-down.
export function mapTiltDegrees(value) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.min(45, number)) : 0;
}

export function tiltScreen(state, point, inverse = false) {
  const angle = mapTiltDegrees(state.mapTilt) * Math.PI / 180;
  if (!angle || !state.viewH) return { x: point.x, y: point.y, depth: 1 };
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
  // Keep horizon queries finite so an off-screen entity cannot abort selection.
  const divisor = Math.abs(depth) < Number.EPSILON
    ? (depth < 0 ? -Number.EPSILON : Number.EPSILON) : depth;
  return { x: cx + x / divisor, y: cy + y * cosine / divisor, depth };
}
