import {
  boundsForGroundPolygon,
  containsProjectedOrthographic,
  createCameraSnapshot,
  groundAtScreenOrthographic,
  projectOrthographic,
  projectedExtentOrthographic,
  viewportGroundPolygonOrthographic,
} from "../camera_projection.js";

export const RENDERER_PROJECTION_VERSION = 2;

export function createRendererProjectionRecord(projection) {
  if (!projection || projection.version !== 1) {
    throw new TypeError("Renderer projection requires ProjectionSnapshotV1.");
  }
  const camera = plainRecord(projection.camera);
  const viewport = plainRecord(projection.viewport);
  const mapBounds = projection.mapBounds == null ? null : plainRecord(projection.mapBounds);
  const record = {
    version: RENDERER_PROJECTION_VERSION,
    kind: projection.perspective == null ? "orthographic" : "perspective",
    camera,
    viewport,
    mapBounds,
  };
  if (projection.perspective != null) {
    record.perspective = plainRecord(projection.perspective);
  } else {
    record.orthographic = plainRecord(projection.orthographic);
  }
  return Object.freeze(record);
}

export function createRendererProjectionQueries(record) {
  if (!record || record.version !== RENDERER_PROJECTION_VERSION) {
    throw new TypeError("Renderer projection queries require RendererProjectionV2.");
  }
  if (record.kind !== "orthographic") {
    throw new TypeError("Pixi compatibility projection requires orthographic camera data.");
  }
  const scale = positive(record.orthographic?.framingScale, "camera framing scale");
  const width = nonNegative(record.orthographic?.viewportWidthCssPx, "viewport width");
  const height = nonNegative(record.orthographic?.viewportHeightCssPx, "viewport height");
  const focusX = finite(record.camera?.focus?.x, "camera focus x");
  const focusY = finite(record.camera?.focus?.y, "camera focus y");
  const state = Object.freeze({
    x: finite(record.orthographic?.originX, "camera origin x"),
    y: finite(record.orthographic?.originY, "camera origin y"),
    zoom: scale,
    worldW: nonNegative(record.orthographic?.worldWidthPx, "map width"),
    worldH: nonNegative(record.orthographic?.worldHeightPx, "map height"),
    viewW: width,
    viewH: height,
  });
  const camera = createCameraSnapshot(focusX, focusY, scale);
  const polygon = () => viewportGroundPolygonOrthographic(state);
  return Object.freeze({
    state,
    project: (point) => projectOrthographic(state, point),
    groundAtScreen: (screen) => groundAtScreenOrthographic(state, screen),
    projectedExtent: (point, worldWidthPx, worldHeightPx) => (
      projectedExtentOrthographic(state, point, worldWidthPx, worldHeightPx)
    ),
    viewportGroundPolygon: polygon,
    viewportGroundBounds: () => boundsForGroundPolygon(polygon()),
    containsProjected: (point, marginCssPx = 0) => containsProjectedOrthographic(state, point, marginCssPx),
    snapshot: () => camera,
  });
}

function finite(value, label) {
  if (!Number.isFinite(value)) throw new TypeError(`${label} must be finite.`);
  return value;
}

function nonNegative(value, label) {
  const number = finite(value, label);
  if (number < 0) throw new RangeError(`${label} must be non-negative.`);
  return number;
}

function positive(value, label) {
  const number = finite(value, label);
  if (number <= 0) throw new RangeError(`${label} must be positive.`);
  return number;
}

function plainRecord(value) {
  const clone = typeof structuredClone === "function"
    ? structuredClone(value)
    : JSON.parse(JSON.stringify(value));
  return freezePlain(clone);
}

function freezePlain(value) {
  if (!value || typeof value !== "object") return value;
  for (const entry of Object.values(value)) freezePlain(entry);
  return Object.freeze(value);
}
