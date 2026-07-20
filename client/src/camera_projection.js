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

function finitePointOrNull(x, y) {
  return Number.isFinite(x) && Number.isFinite(y) ? immutablePoint(x, y) : null;
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

/**
 * Project with the raw coefficients of the orthographic compatibility adapter.
 * Keeping these queries pure lets the live Camera and detached frame snapshots
 * share exactly one implementation without retaining a live Camera reference.
 */
export function projectOrthographic(state, point) {
  const x = requireFinite(point?.x, "presented point x");
  const y = requireFinite(point?.y, "presented point y");
  requireFinite(point?.heightPx, "presented point heightPx");
  return classifyProjectedPoint({
    x: (x - state.x) * state.zoom,
    y: (y - state.y) * state.zoom,
    depth: 1,
  }, {
    widthCssPx: state.viewW,
    heightCssPx: state.viewH,
  });
}

export function groundAtScreenOrthographic(state, screen) {
  if (!Number.isFinite(screen?.x) || !Number.isFinite(screen?.y)) return null;
  return finitePointOrNull(
    state.x + screen.x / state.zoom,
    state.y + screen.y / state.zoom,
  );
}

export function projectedExtentOrthographic(state, point, worldWidthPx, worldHeightPx) {
  const projected = projectOrthographic(state, point);
  const width = requireFinite(
    requireNonNegative(worldWidthPx, "world extent width") * state.zoom,
    "projected extent width",
  );
  const height = requireFinite(
    requireNonNegative(worldHeightPx, "world extent height") * state.zoom,
    "projected extent height",
  );
  return Object.freeze({
    width,
    height,
    scaleX: state.zoom,
    scaleY: state.zoom,
    visible: state.viewW > 0
      && state.viewH > 0
      && projected.depth > 0
      && projected.clip !== PROJECTION_CLIP.OUTSIDE_DEPTH
      && projected.x + width / 2 >= 0
      && projected.x - width / 2 <= state.viewW
      && projected.y + height / 2 >= 0
      && projected.y - height / 2 <= state.viewH,
  });
}

export function viewportGroundPolygonOrthographic(state) {
  if (state.viewW <= 0 || state.viewH <= 0 || state.worldW <= 0 || state.worldH <= 0) {
    return Object.freeze([]);
  }
  const maxX = state.x + state.viewW / state.zoom;
  const maxY = state.y + state.viewH / state.zoom;
  if (!Number.isFinite(maxX) || !Number.isFinite(maxY)) return Object.freeze([]);
  return clipGroundPolygonToBounds([
    { x: state.x, y: state.y },
    { x: maxX, y: state.y },
    { x: maxX, y: maxY },
    { x: state.x, y: maxY },
  ], {
    minX: 0,
    minY: 0,
    maxX: state.worldW,
    maxY: state.worldH,
  });
}

export function containsProjectedOrthographic(state, point, marginCssPx = 0) {
  const margin = requireNonNegative(marginCssPx, "projection margin");
  const projected = projectOrthographic(state, point);
  return state.viewW > 0
    && state.viewH > 0
    && projected.depth > 0
    && projected.clip !== PROJECTION_CLIP.OUTSIDE_DEPTH
    && projected.x >= -margin
    && projected.x <= state.viewW + margin
    && projected.y >= -margin
    && projected.y <= state.viewH + margin;
}

/** Create an immutable, renderer-neutral query surface pinned to one presented frame. */
export function createOrthographicProjectionSnapshot(rawState, fallbackReferenceDistancePx) {
  const state = Object.freeze({
    x: requireFinite(rawState?.x, "camera origin x"),
    y: requireFinite(rawState?.y, "camera origin y"),
    zoom: requireFinite(rawState?.zoom, "camera framing scale"),
    worldW: requireNonNegative(rawState?.worldW, "map width"),
    worldH: requireNonNegative(rawState?.worldH, "map height"),
    viewW: requireNonNegative(rawState?.viewW, "viewport width"),
    viewH: requireNonNegative(rawState?.viewH, "viewport height"),
  });
  if (state.zoom <= 0) throw new RangeError("camera framing scale must be positive");

  const camera = createCameraSnapshot(
    state.x + state.viewW / (2 * state.zoom),
    state.y + state.viewH / (2 * state.zoom),
    state.zoom,
  );

  const viewport = Object.freeze({
    widthCssPx: state.viewW,
    heightCssPx: state.viewH,
  });
  const mapBounds = state.worldW > 0 && state.worldH > 0
    ? Object.freeze({ minX: 0, minY: 0, maxX: state.worldW, maxY: state.worldH })
    : null;
  const candidateReferenceDistancePx = state.viewW / state.zoom;
  const referenceDistancePx = Number.isFinite(candidateReferenceDistancePx)
    && candidateReferenceDistancePx > 0
    ? candidateReferenceDistancePx
    : requireFinite(fallbackReferenceDistancePx, "audio reference distance");
  if (referenceDistancePx <= 0) throw new RangeError("audio reference distance must be positive");
  const viewportGroundPolygon = () => viewportGroundPolygonOrthographic(state);

  return Object.freeze({
    version: 1,
    camera,
    viewport,
    mapBounds,
    orthographic: Object.freeze({
      originX: state.x,
      originY: state.y,
      framingScale: state.zoom,
      worldWidthPx: state.worldW,
      worldHeightPx: state.worldH,
      viewportWidthCssPx: state.viewW,
      viewportHeightCssPx: state.viewH,
    }),
    project: (point) => projectOrthographic(state, point),
    groundAtScreen: (screen) => groundAtScreenOrthographic(state, screen),
    projectedExtent: (point, worldWidthPx, worldHeightPx) => (
      projectedExtentOrthographic(state, point, worldWidthPx, worldHeightPx)
    ),
    viewportGroundPolygon,
    viewportGroundBounds: () => boundsForGroundPolygon(viewportGroundPolygon()),
    containsProjected: (point, marginCssPx = 0) => (
      containsProjectedOrthographic(state, point, marginCssPx)
    ),
    snapshot: () => camera,
    audioListener: () => Object.freeze({
      x: camera.focus.x,
      y: camera.focus.y,
      referenceDistancePx,
    }),
  });
}
