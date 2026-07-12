import { CAMERA } from "./config.js";
import {
  boundsForGroundPolygon,
  classifyProjectedPoint,
  clipGroundPolygonToBounds,
} from "./camera_projection.js";

export const FIXED_PERSPECTIVE = Object.freeze({
  fovYRad: 42 * Math.PI / 180,
  pitchRad: 60 * Math.PI / 180,
});

function finite(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}
function required(value, label) {
  const number = finite(value);
  if (number == null) throw new TypeError(`${label} must be finite`);
  return number;
}

function nonNegative(value, label) {
  const number = required(value, label);
  if (number < 0) throw new RangeError(`${label} must be non-negative`);
  return number;
}

function positive(value, label) {
  const number = required(value, label);
  if (number <= 0) throw new RangeError(`${label} must be positive`);
  return number;
}

function cameraSnapshot(focus, framingScale) {
  return Object.freeze({
    version: 1,
    focus: Object.freeze({ x: focus.x, y: focus.y }),
    framingScale,
    boundsPolicy: "mapOverscroll",
  });
}

function coefficients(state) {
  const focalLengthCssPx = state.viewportHeightCssPx > 0
    ? state.viewportHeightCssPx / (2 * Math.tan(FIXED_PERSPECTIVE.fovYRad / 2))
    : 1;
  const distanceWorldPx = focalLengthCssPx / state.framingScale;
  const sinPitch = Math.sin(FIXED_PERSPECTIVE.pitchRad);
  const cosPitch = Math.cos(FIXED_PERSPECTIVE.pitchRad);
  return Object.freeze({
    fovYRad: FIXED_PERSPECTIVE.fovYRad,
    pitchRad: FIXED_PERSPECTIVE.pitchRad,
    focalLengthCssPx,
    distanceWorldPx,
    sinPitch,
    cosPitch,
    nearDepthWorldPx: Math.max(0.5, distanceWorldPx * 0.01),
    farDepthWorldPx: Math.max(1000, distanceWorldPx + Math.hypot(state.mapWidthPx, state.mapHeightPx) * 2),
  });
}

function projectPoint(state, coeff, point) {
  const x = required(point?.x, "presented point x");
  const y = required(point?.y, "presented point y");
  const heightPx = point?.heightPx == null ? 0 : required(point.heightPx, "presented point height");
  const lateral = x - state.focusX;
  const forward = y - state.focusY;
  const depth = coeff.distanceWorldPx + forward * coeff.cosPitch - heightPx * coeff.sinPitch;
  const cameraUp = heightPx * coeff.cosPitch + forward * coeff.sinPitch;
  const safeDepth = Math.abs(depth) > 1e-9 ? depth : 1e-9;
  return classifyProjectedPoint({
    x: state.viewportWidthCssPx / 2 + coeff.focalLengthCssPx * lateral / safeDepth,
    y: state.viewportHeightCssPx / 2 - coeff.focalLengthCssPx * cameraUp / safeDepth,
    depth,
  }, {
    widthCssPx: state.viewportWidthCssPx,
    heightCssPx: state.viewportHeightCssPx,
    nearDepth: coeff.nearDepthWorldPx,
    farDepth: coeff.farDepthWorldPx,
  });
}

function groundHit(state, coeff, screen) {
  const x = finite(screen?.x);
  const y = finite(screen?.y);
  if (x == null || y == null) return null;
  const localX = (x - state.viewportWidthCssPx / 2) / coeff.focalLengthCssPx;
  const localUp = (state.viewportHeightCssPx / 2 - y) / coeff.focalLengthCssPx;
  const rayHeight = localUp * coeff.cosPitch - coeff.sinPitch;
  if (rayHeight >= -1e-9) return null;
  const cameraHeight = coeff.distanceWorldPx * coeff.sinPitch;
  const distance = -cameraHeight / rayHeight;
  const point = {
    x: state.focusX + distance * localX,
    y: state.focusY - coeff.distanceWorldPx * coeff.cosPitch
      + distance * (coeff.cosPitch + localUp * coeff.sinPitch),
  };
  return Number.isFinite(point.x) && Number.isFinite(point.y) ? Object.freeze(point) : null;
}

function projectionSnapshot(rawState) {
  const state = Object.freeze({
    focusX: required(rawState.focusX, "camera focus x"),
    focusY: required(rawState.focusY, "camera focus y"),
    framingScale: positive(rawState.framingScale, "camera framing scale"),
    viewportWidthCssPx: nonNegative(rawState.viewportWidthCssPx, "viewport width"),
    viewportHeightCssPx: nonNegative(rawState.viewportHeightCssPx, "viewport height"),
    mapWidthPx: nonNegative(rawState.mapWidthPx, "map width"),
    mapHeightPx: nonNegative(rawState.mapHeightPx, "map height"),
  });
  const coeff = coefficients(state);
  const camera = cameraSnapshot({ x: state.focusX, y: state.focusY }, state.framingScale);
  const viewport = Object.freeze({
    widthCssPx: state.viewportWidthCssPx,
    heightCssPx: state.viewportHeightCssPx,
  });
  const mapBounds = state.mapWidthPx > 0 && state.mapHeightPx > 0
    ? Object.freeze({ minX: 0, minY: 0, maxX: state.mapWidthPx, maxY: state.mapHeightPx })
    : null;
  const perspective = Object.freeze({
    fovYRad: coeff.fovYRad,
    pitchRad: coeff.pitchRad,
    focalLengthCssPx: coeff.focalLengthCssPx,
    distanceWorldPx: coeff.distanceWorldPx,
    nearDepthWorldPx: coeff.nearDepthWorldPx,
    farDepthWorldPx: coeff.farDepthWorldPx,
  });
  const viewportGroundPolygon = () => {
    const corners = [
      { x: 0, y: 0 },
      { x: state.viewportWidthCssPx, y: 0 },
      { x: state.viewportWidthCssPx, y: state.viewportHeightCssPx },
      { x: 0, y: state.viewportHeightCssPx },
    ];
    const hits = corners.map((point) => groundHit(state, coeff, point)).filter(Boolean);
    if (hits.length < 3) return Object.freeze([]);
    return mapBounds ? clipGroundPolygonToBounds(hits, mapBounds) : Object.freeze(hits);
  };
  const project = (point) => projectPoint(state, coeff, point);
  const groundAtScreen = (screen) => groundHit(state, coeff, screen);
  return Object.freeze({
    version: 1,
    camera,
    viewport,
    mapBounds,
    perspective,
    project,
    groundAtScreen,
    projectedExtent(point, worldWidthPx, worldHeightPx) {
      const width = nonNegative(worldWidthPx, "projected width");
      const height = nonNegative(worldHeightPx, "projected height");
      const center = project(point);
      const left = project({ x: point.x - width / 2, y: point.y, heightPx: point.heightPx || 0 });
      const right = project({ x: point.x + width / 2, y: point.y, heightPx: point.heightPx || 0 });
      const near = project({ x: point.x, y: point.y - height / 2, heightPx: point.heightPx || 0 });
      const far = project({ x: point.x, y: point.y + height / 2, heightPx: point.heightPx || 0 });
      const projectedWidth = Math.abs(right.x - left.x);
      const projectedHeight = Math.abs(far.y - near.y);
      return Object.freeze({
        width: projectedWidth,
        height: projectedHeight,
        scaleX: width > 0 ? projectedWidth / width : state.framingScale,
        scaleY: height > 0 ? projectedHeight / height : state.framingScale * coeff.sinPitch,
        visible: center.visible,
      });
    },
    viewportGroundPolygon,
    viewportGroundBounds: () => boundsForGroundPolygon(viewportGroundPolygon()),
    containsProjected(point, marginCssPx = 0) {
      const margin = nonNegative(marginCssPx, "projection margin");
      const projected = project(point);
      return projected.depth >= coeff.nearDepthWorldPx && projected.depth <= coeff.farDepthWorldPx
        && projected.x >= -margin && projected.x <= state.viewportWidthCssPx + margin
        && projected.y >= -margin && projected.y <= state.viewportHeightCssPx + margin;
    },
    snapshot: () => camera,
    audioListener: () => Object.freeze({
      x: state.focusX,
      y: state.focusY,
      referenceDistancePx: state.viewportWidthCssPx / state.framingScale,
    }),
  });
}

export class FixedPerspectiveCamera {
  constructor(viewportWidthCssPx = 0, viewportHeightCssPx = 0, options = {}) {
    this.focusX = 0;
    this.focusY = 0;
    this.framingScale = 1;
    this.viewportWidthCssPx = nonNegative(viewportWidthCssPx, "viewport width");
    this.viewportHeightCssPx = nonNegative(viewportHeightCssPx, "viewport height");
    this.mapWidthPx = 0;
    this.mapHeightPx = 0;
    this.minZoom = positive(options.minZoom ?? CAMERA.minZoom, "minimum framing scale");
    this.maxZoom = Math.max(this.minZoom, positive(options.maxZoom ?? CAMERA.maxZoom, "maximum framing scale"));
    this._listeners = new Set();
  }

  update(dt, input) {
    const elapsed = finite(dt);
    if (elapsed == null || elapsed < 0 || !input) return;
    let dx = 0;
    let dy = 0;
    if (input.keys?.left) dx -= 1;
    if (input.keys?.right) dx += 1;
    if (input.keys?.up) dy -= 1;
    if (input.keys?.down) dy += 1;
    const mouse = input.mouse;
    if (mouse && this.viewportWidthCssPx > 0 && this.viewportHeightCssPx > 0) {
      if (mouse.x <= CAMERA.edgeScrollPx) dx -= 1;
      else if (mouse.x >= this.viewportWidthCssPx - CAMERA.edgeScrollPx) dx += 1;
      if (mouse.y <= CAMERA.edgeScrollPx) dy -= 1;
      else if (mouse.y >= this.viewportHeightCssPx - CAMERA.edgeScrollPx) dy += 1;
    }
    if (dx || dy) this._mutate(() => {
      const speed = CAMERA.panSpeed * elapsed / this.framingScale;
      this.focusX += dx * speed;
      this.focusY += dy * speed;
    });
  }

  project(point) { return this.projectionSnapshot().project(point); }
  groundAtScreen(screen) { return this.projectionSnapshot().groundAtScreen(screen); }
  projectedExtent(point, width, height) { return this.projectionSnapshot().projectedExtent(point, width, height); }
  viewportGroundPolygon() { return this.projectionSnapshot().viewportGroundPolygon(); }
  viewportGroundBounds() { return this.projectionSnapshot().viewportGroundBounds(); }
  containsProjected(point, margin = 0) { return this.projectionSnapshot().containsProjected(point, margin); }

  focusAt(point) {
    const x = required(point?.x, "camera focus x");
    const y = required(point?.y, "camera focus y");
    this._mutate(() => { this.focusX = x; this.focusY = y; });
  }

  fitWorldPoints(points, { paddingCssPx = 0 } = {}) {
    if (!Array.isArray(points)) throw new TypeError("fit points must be an array");
    const padding = nonNegative(paddingCssPx, "fit padding");
    const accepted = points.filter((point) => finite(point?.x) != null && finite(point?.y) != null);
    if (!accepted.length) return false;
    const availableWidth = this.viewportWidthCssPx - padding * 2;
    const availableHeight = this.viewportHeightCssPx - padding * 2;
    if (availableWidth <= 0 || availableHeight <= 0) return false;
    const xs = accepted.map((point) => point.x);
    const ys = accepted.map((point) => point.y);
    const width = Math.max(...xs) - Math.min(...xs);
    const height = Math.max(...ys) - Math.min(...ys);
    const scaleX = width > 0 ? availableWidth / width : this.maxZoom;
    const scaleY = height > 0 ? availableHeight / (height * Math.sin(FIXED_PERSPECTIVE.pitchRad)) : this.maxZoom;
    this._mutate(() => {
      this.framingScale = this._clampScale(Math.min(scaleX, scaleY));
      this.focusX = (Math.min(...xs) + Math.max(...xs)) / 2;
      this.focusY = (Math.min(...ys) + Math.max(...ys)) / 2;
    });
    return true;
  }

  panByScreenDelta(dx, dy) {
    const deltaX = typeof dx === "object" ? dx?.x : dx;
    const deltaY = typeof dx === "object" ? dx?.y : dy;
    const x = required(deltaX, "pan delta x");
    const y = required(deltaY, "pan delta y");
    const before = this.groundAtScreen({ x: this.viewportWidthCssPx / 2 - x, y: this.viewportHeightCssPx / 2 - y });
    if (!before) return;
    this._mutate(() => {
      this.focusX += before.x - this.focusX;
      this.focusY += before.y - this.focusY;
    });
  }

  dollyBy(factor, anchorScreen = null) {
    const multiplier = positive(factor, "dolly factor");
    const anchor = anchorScreen || { x: this.viewportWidthCssPx / 2, y: this.viewportHeightCssPx / 2 };
    const before = this.groundAtScreen(anchor);
    this._mutate(() => { this.framingScale = this._clampScale(this.framingScale * multiplier); });
    const after = this.groundAtScreen(anchor);
    if (before && after) this._mutate(() => {
      this.focusX += before.x - after.x;
      this.focusY += before.y - after.y;
    });
  }

  resize(width, height) {
    const w = nonNegative(width, "viewport width");
    const h = nonNegative(height, "viewport height");
    this._mutate(() => { this.viewportWidthCssPx = w; this.viewportHeightCssPx = h; });
  }

  setMapBounds(width, height) {
    const w = nonNegative(width, "map width");
    const h = nonNegative(height, "map height");
    this._mutate(() => { this.mapWidthPx = w; this.mapHeightPx = h; });
  }

  snapshot() { return cameraSnapshot({ x: this.focusX, y: this.focusY }, this.framingScale); }
  projectionSnapshot() { return projectionSnapshot(this); }

  restore(snapshot) {
    if (snapshot?.version !== 1 || snapshot.boundsPolicy !== "mapOverscroll") return false;
    const x = finite(snapshot.focus?.x);
    const y = finite(snapshot.focus?.y);
    const scale = finite(snapshot.framingScale);
    if (x == null || y == null || scale == null || scale <= 0) return false;
    this._mutate(() => { this.focusX = x; this.focusY = y; this.framingScale = this._clampScale(scale); });
    return true;
  }

  audioListener() { return this.projectionSnapshot().audioListener(); }

  subscribe(listener) {
    if (typeof listener !== "function") throw new TypeError("camera listener must be a function");
    this._listeners.add(listener);
    let active = true;
    return () => { if (active) this._listeners.delete(listener); active = false; };
  }

  _clampScale(value) { return Math.max(this.minZoom, Math.min(this.maxZoom, value)); }

  _clampFocus() {
    if (this.mapWidthPx <= 0 || this.mapHeightPx <= 0) return;
    const visibleWidth = this.viewportWidthCssPx / this.framingScale;
    const visibleHeight = this.viewportHeightCssPx / (this.framingScale * Math.sin(FIXED_PERSPECTIVE.pitchRad));
    this.focusX = Math.max(-visibleWidth / 4, Math.min(this.mapWidthPx + visibleWidth / 4, this.focusX));
    this.focusY = Math.max(-visibleHeight / 4, Math.min(this.mapHeightPx + visibleHeight / 4, this.focusY));
  }

  _mutate(change) {
    const before = this.snapshot();
    change();
    this._clampFocus();
    const after = this.snapshot();
    if (JSON.stringify(before) === JSON.stringify(after)) return;
    for (const listener of [...this._listeners]) {
      try { listener(after); } catch (error) { console.error("Camera semantic listener failed", error); }
    }
  }
}
