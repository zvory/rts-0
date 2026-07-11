// Renderer-neutral projection helpers. Public screen values are viewport-local CSS pixels;
// renderer matrices, canvas offsets, backing dimensions, and DPR never enter these shapes.

export const PROJECTION_CLIP = Object.freeze({
  INSIDE: "inside",
  OUTSIDE_VIEWPORT: "outsideViewport",
  OUTSIDE_DEPTH: "outsideDepth",
  BEHIND_CAMERA: "behindCamera",
});

const POLYGON_EPSILON = 1e-7;

function requireFinite(value, name) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TypeError(`${name} must be finite`);
  }
  return value;
}

function requireNonNegative(value, name) {
  const number = requireFinite(value, name);
  if (number < 0) throw new RangeError(`${name} must be non-negative`);
  return number;
}

function immutablePoint(x, y) {
  return Object.freeze({ x, y });
}

export function classifyProjectedPoint(
  point,
  { widthCssPx, heightCssPx, nearDepth = 0, farDepth = Number.POSITIVE_INFINITY },
) {
  const x = requireFinite(point?.x, "projected x");
  const y = requireFinite(point?.y, "projected y");
  const depth = requireFinite(point?.depth, "projected depth");
  const width = requireNonNegative(widthCssPx, "viewport width");
  const height = requireNonNegative(heightCssPx, "viewport height");
  const near = requireNonNegative(nearDepth, "near depth");
  const far = farDepth === Number.POSITIVE_INFINITY
    ? farDepth
    : requireFinite(farDepth, "far depth");
  if (far <= near) throw new RangeError("far depth must be greater than near depth");

  let clip = PROJECTION_CLIP.INSIDE;
  if (depth <= 0) clip = PROJECTION_CLIP.BEHIND_CAMERA;
  else if (depth < near || depth > far) clip = PROJECTION_CLIP.OUTSIDE_DEPTH;
  else if (width <= 0 || height <= 0 || x < 0 || x > width || y < 0 || y > height) {
    clip = PROJECTION_CLIP.OUTSIDE_VIEWPORT;
  }
  return Object.freeze({
    x,
    y,
    depth,
    clip,
    visible: clip === PROJECTION_CLIP.INSIDE,
  });
}

function intersectBoundary(start, end, axis, value) {
  const delta = end[axis] - start[axis];
  if (Math.abs(delta) <= POLYGON_EPSILON) return immutablePoint(start.x, start.y);
  const t = (value - start[axis]) / delta;
  return immutablePoint(
    start.x + (end.x - start.x) * t,
    start.y + (end.y - start.y) * t,
  );
}

function clipAgainstBoundary(points, inside, axis, value) {
  if (points.length === 0) return points;
  const clipped = [];
  let start = points[points.length - 1];
  let startInside = inside(start);
  for (const end of points) {
    const endInside = inside(end);
    if (endInside !== startInside) clipped.push(intersectBoundary(start, end, axis, value));
    if (endInside) clipped.push(end);
    start = end;
    startInside = endInside;
  }
  return clipped;
}

function samePoint(a, b) {
  return Math.abs(a.x - b.x) <= POLYGON_EPSILON
    && Math.abs(a.y - b.y) <= POLYGON_EPSILON;
}

function signedArea(points) {
  let twiceArea = 0;
  for (let index = 0; index < points.length; index += 1) {
    const current = points[index];
    const next = points[(index + 1) % points.length];
    twiceArea += current.x * next.y - next.x * current.y;
  }
  return twiceArea / 2;
}

function normalizeClockwise(points) {
  const deduped = [];
  for (const point of points) {
    if (deduped.length === 0 || !samePoint(deduped[deduped.length - 1], point)) {
      deduped.push(immutablePoint(point.x, point.y));
    }
  }
  if (deduped.length > 1 && samePoint(deduped[0], deduped[deduped.length - 1])) {
    deduped.pop();
  }
  if (deduped.length < 3 || Math.abs(signedArea(deduped)) <= POLYGON_EPSILON) {
    return Object.freeze([]);
  }

  // World y increases downward, so positive Cartesian signed area is clockwise on screen.
  if (signedArea(deduped) < 0) deduped.reverse();
  let first = 0;
  for (let index = 1; index < deduped.length; index += 1) {
    if (
      deduped[index].y < deduped[first].y - POLYGON_EPSILON
      || (
        Math.abs(deduped[index].y - deduped[first].y) <= POLYGON_EPSILON
        && deduped[index].x < deduped[first].x
      )
    ) first = index;
  }
  const stable = [...deduped.slice(first), ...deduped.slice(0, first)];
  return Object.freeze(stable);
}

export function clipGroundPolygonToBounds(points, bounds) {
  if (!Array.isArray(points)) throw new TypeError("ground polygon must be an array");
  const minX = requireFinite(bounds?.minX, "bounds minX");
  const minY = requireFinite(bounds?.minY, "bounds minY");
  const maxX = requireFinite(bounds?.maxX, "bounds maxX");
  const maxY = requireFinite(bounds?.maxY, "bounds maxY");
  if (maxX <= minX || maxY <= minY) return Object.freeze([]);

  let clipped = points.map((point, index) => immutablePoint(
    requireFinite(point?.x, `polygon[${index}].x`),
    requireFinite(point?.y, `polygon[${index}].y`),
  ));
  clipped = clipAgainstBoundary(clipped, (point) => point.x >= minX, "x", minX);
  clipped = clipAgainstBoundary(clipped, (point) => point.x <= maxX, "x", maxX);
  clipped = clipAgainstBoundary(clipped, (point) => point.y >= minY, "y", minY);
  clipped = clipAgainstBoundary(clipped, (point) => point.y <= maxY, "y", maxY);
  return normalizeClockwise(clipped);
}

export function boundsForGroundPolygon(points) {
  if (!Array.isArray(points) || points.length < 3) return null;
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  for (let index = 0; index < points.length; index += 1) {
    const x = requireFinite(points[index]?.x, `polygon[${index}].x`);
    const y = requireFinite(points[index]?.y, `polygon[${index}].y`);
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  if (maxX <= minX || maxY <= minY) return null;
  return Object.freeze({ minX, minY, maxX, maxY });
}

export function createCameraSnapshot(focusX, focusY, framingScale) {
  const scale = requireFinite(framingScale, "camera framing scale");
  if (scale <= 0) throw new RangeError("camera framing scale must be positive");
  return Object.freeze({
    version: 1,
    focus: immutablePoint(
      requireFinite(focusX, "camera focus x"),
      requireFinite(focusY, "camera focus y"),
    ),
    framingScale: scale,
    boundsPolicy: "mapOverscroll",
  });
}
