import { STATS, entityBodyRadius } from "../config.js";
import { KIND, isBuilding, isResource, isUnit } from "../protocol.js";
import { clickGeometry } from "./click_geometry.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE } from "./constants.js";

const CIRCLE_SEGMENTS = 16;
const EPSILON = 1e-7;

export function buildSelectionScene({
  entities,
  rememberedEntities = null,
  preparedEntities = null,
  projection,
  tileSize = DEFAULT_TILE_SIZE,
  generation = 1,
  frameId = 0,
  experimentalGraphicsEnabled = false,
} = {}) {
  if (!projection || projection.version !== 1 || typeof projection.project !== "function") {
    throw new TypeError("SelectionSceneV1 requires a ProjectionSnapshotV1");
  }
  const proxies = [];
  const sourceEntities = Array.isArray(entities) ? entities : [];
  for (let index = 0; index < sourceEntities.length; index += 1) {
    const proxy = selectionProxyForEntity(sourceEntities[index], tileSize, preparedEntities?.[index], experimentalGraphicsEnabled);
    if (proxy) proxies.push(proxy);
  }
  const rememberedProxies = [];
  for (const entity of Array.isArray(rememberedEntities) ? rememberedEntities : []) {
    const proxy = selectionProxyForEntity(entity, tileSize);
    if (proxy) rememberedProxies.push(proxy);
  }
  return Object.freeze({
    version: 1,
    generation: finiteInteger(generation, 1),
    frameId: finiteInteger(frameId, 0),
    projection,
    proxies: Object.freeze(proxies),
    rememberedProxies: Object.freeze(rememberedProxies),
  });
}

export function selectionProxyForEntity(entity, tileSize = DEFAULT_TILE_SIZE, prepared = null, experimentalGraphicsEnabled = false) {
  if (
    !entity ||
    entity.shotReveal ||
    entity.visionOnly ||
    !Number.isInteger(entity.id) ||
    !Number.isFinite(entity.x) ||
    !Number.isFinite(entity.y) ||
    typeof entity.kind !== "string"
  ) return null;
  if (isResource(entity.kind) && entity.remaining === 0) return null;

  const stat = STATS[entity.kind] || {};
  const size = finitePositive(entityBodyRadius(entity), DEFAULT_HIT_RADIUS);
  const mapTileSize = finitePositive(tileSize, DEFAULT_TILE_SIZE);
  let footprint;
  if (isBuilding(entity.kind)) {
    footprint = polygonFootprint(
      entity.x,
      entity.y,
      finitePositive(stat.footW, 1) * mapTileSize,
      finitePositive(stat.footH, 1) * mapTileSize,
      finiteFacing(entity.facing),
    );
  } else if (stat.body && Number.isFinite(stat.body.length) && Number.isFinite(stat.body.width)) {
    const clearance = Number.isFinite(stat.body.clearance) ? Math.max(0, stat.body.clearance) : 0;
    footprint = polygonFootprint(
      entity.x,
      entity.y,
      Math.max(1, stat.body.length + clearance * 2),
      Math.max(1, stat.body.width + clearance * 2),
      finiteFacing(entity.facing),
    );
  } else {
    footprint = Object.freeze({ kind: "circle", radiusPx: size });
  }

  const heightPx = isBuilding(entity.kind)
    ? Math.max(8, Math.max(finitePositive(stat.footW, 1), finitePositive(stat.footH, 1)) * mapTileSize * 0.5)
    : Math.max(8, size);
  const pointClick = clickGeometry(entity, footprint);
  if (experimentalGraphicsEnabled && entity.kind === KIND.RIFLEMAN) {
    // Authored body bounds only; collision, marquee and selection rings keep the ground footprint.
    pointClick.uprightClick = true;
    pointClick.clickFootprint = polygonFootprint(entity.x, entity.y - 20, 28, 52, 0);
    pointClick.directClickFootprint = polygonFootprint(entity.x, entity.y - 20, 22, 46, 0);
  }
  return Object.freeze({
    version: 1,
    id: entity.id,
    kind: entity.kind,
    owner: Number.isFinite(entity.owner) ? entity.owner : 0,
    selectClass: selectionClass(entity.kind),
    facing: finiteFacing(entity.facing),
    setupState: typeof entity.setupState === "string" ? entity.setupState : null,
    anchor: Object.freeze({ x: entity.x, y: entity.y, heightPx }),
    footprint,
    ...pointClick,
    minScreenRadiusCssPx: 6,
    interaction: prepared?.source === entity && prepared.interaction
      ? prepared.interaction
      : detachPlainRecord(entity),
  });
}

export function projectSelectionProxy(proxy, projection) {
  if (!proxy || !projection || typeof projection.project !== "function") return null;
  let anchor;
  try {
    anchor = projection.project(proxy.anchor);
  } catch {
    return null;
  }
  if (!projectedDepthAdmitted(anchor)) return null;

  const screenPolygon = projectFootprint(proxy.footprint, proxy.anchor, projection);
  if (!screenPolygon) return null;
  const clickFootprint = proxy.clickFootprint || proxy.footprint;
  const directFootprint = proxy.directClickFootprint || proxy.footprint;
  const screenClickPolygon = clickFootprint === proxy.footprint ? screenPolygon
    : projectFootprint(clickFootprint, proxy.anchor, projection, proxy.uprightClick);
  const screenDirectClickPolygon = directFootprint === proxy.footprint ? screenPolygon
    : directFootprint === clickFootprint ? screenClickPolygon
    : projectFootprint(directFootprint, proxy.anchor, projection, proxy.uprightClick);
  return Object.freeze({
    proxy,
    anchor,
    screenPolygon,
    screenClickPolygon,
    screenDirectClickPolygon,
    minScreenRadiusCssPx: finitePositive(proxy.minScreenRadiusCssPx, 6),
    clickMinScreenRadiusCssPx: finitePositive(proxy.clickMinScreenRadiusCssPx, finitePositive(proxy.minScreenRadiusCssPx, 6)),
    directClickMinScreenRadiusCssPx: finitePositive(proxy.directClickMinScreenRadiusCssPx, finitePositive(proxy.minScreenRadiusCssPx, 6)),
  });
}

function projectFootprint(footprint, anchor, projection, upright = false) {
  const worldPoints = footprint?.kind === "polygon"
    ? footprint.points : circlePoints(anchor, footprint?.radiusPx);
  const points = [];
  const foot = upright ? projection.project(anchor) : null;
  const uprightScale = upright ? projection.camera.framingScale / foot.depth : 0;
  for (const point of worldPoints) {
    let projected;
    try {
      projected = upright ? { ...foot,
        x: foot.x + (point.x - anchor.x) * uprightScale,
        y: foot.y + (point.y - anchor.y) * uprightScale,
      } : projection.project({ x: point.x, y: point.y, heightPx: 0 });
    } catch {
      return null;
    }
    // Do not join across near/far-plane gaps, including expanded click areas.
    if (!projectedDepthAdmitted(projected)) return null;
    points.push(Object.freeze({ x: projected.x, y: projected.y, depth: projected.depth }));
  }
  return points.length >= 3 ? Object.freeze(points) : null;
}

export function pickSelectionProxy(scene, screen, {
  eligible = () => true,
  preference = () => 0,
} = {}) {
  if (!finitePoint(screen)) return null;
  const hits = [];
  for (const proxy of scene?.proxies || []) {
    if (!eligible(proxy)) continue;
    const projected = projectSelectionProxy(proxy, scene.projection);
    if (!projected || !screenPointHitsProjectedProxy(screen, projected)) continue;
    const directDistance = distanceToDirectHitArea(screen, projected);
    hits.push({
      projected,
      direct: directDistance <= EPSILON,
      directDistance,
      preference: finiteNumber(preference(proxy), 0),
      distance: Math.hypot(screen.x - projected.anchor.x, screen.y - projected.anchor.y),
    });
  }
  hits.sort((a, b) =>
    Number(b.direct) - Number(a.direct) ||
    b.preference - a.preference ||
    (a.direct ? 0 : a.directDistance - b.directDistance) ||
    a.distance - b.distance ||
    a.projected.anchor.depth - b.projected.anchor.depth ||
    a.projected.proxy.id - b.projected.proxy.id);
  return hits[0]?.projected.proxy || null;
}

export function selectionProxiesInScreenRect(scene, rect, {
  eligible = () => true,
  anchor = null,
} = {}) {
  const normalized = normalizeScreenRect(rect);
  if (!normalized) return [];
  const start = finitePoint(anchor) ? anchor : { x: normalized.minX, y: normalized.minY };
  const matches = [];
  for (const proxy of scene?.proxies || []) {
    if (!eligible(proxy)) continue;
    const projected = projectSelectionProxy(proxy, scene.projection);
    if (!projected || !projectedProxyIntersectsScreenRect(projected, normalized)) continue;
    matches.push({
      proxy,
      distance: Math.hypot(start.x - projected.anchor.x, start.y - projected.anchor.y),
    });
  }
  matches.sort((a, b) => a.distance - b.distance || a.proxy.id - b.proxy.id);
  return matches.map(({ proxy }) => proxy);
}

export function proxyIntersectsViewport(scene, proxy) {
  const viewport = scene?.projection?.viewport;
  const width = Number(viewport?.widthCssPx);
  const height = Number(viewport?.heightCssPx);
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return false;
  const projected = projectSelectionProxy(proxy, scene.projection);
  return !!projected && projectedProxyIntersectsScreenRect(projected, {
    minX: 0,
    minY: 0,
    maxX: width,
    maxY: height,
  });
}

export function groundCoverageForScreenRect(scene, rect) {
  const normalized = normalizeScreenRect(rect);
  const groundAtScreen = scene?.projection?.groundAtScreen;
  if (!normalized || typeof groundAtScreen !== "function") {
    return Object.freeze({ groundPolygon: Object.freeze([]), groundBounds: null });
  }
  const corners = [
    { x: normalized.minX, y: normalized.minY },
    { x: normalized.maxX, y: normalized.minY },
    { x: normalized.maxX, y: normalized.maxY },
    { x: normalized.minX, y: normalized.maxY },
  ];
  const polygon = [];
  for (const corner of corners) {
    const point = groundAtScreen(corner);
    if (finitePoint(point)) polygon.push(Object.freeze({ x: point.x, y: point.y }));
  }
  if (polygon.length < 3) {
    return Object.freeze({ groundPolygon: Object.freeze(polygon), groundBounds: null });
  }
  const xs = polygon.map((point) => point.x);
  const ys = polygon.map((point) => point.y);
  return Object.freeze({
    groundPolygon: Object.freeze(polygon),
    groundBounds: Object.freeze({
      minX: Math.min(...xs),
      minY: Math.min(...ys),
      maxX: Math.max(...xs),
      maxY: Math.max(...ys),
    }),
  });
}

function screenPointHitsProjectedProxy(point, projected) {
  return !!projected.screenClickPolygon && (
    Math.hypot(point.x - projected.anchor.x, point.y - projected.anchor.y)
      <= projected.clickMinScreenRadiusCssPx
    || pointInPolygon(point, projected.screenClickPolygon)
  );
}

function distanceToDirectHitArea(point, projected) {
  const polygon = projected.screenDirectClickPolygon;
  // A depth-clipped expanded polygon cannot manufacture an anchor-only hit.
  if (!polygon) return Number.POSITIVE_INFINITY;
  const radius = projected.directClickMinScreenRadiusCssPx;
  let distance = Math.max(0, Math.hypot(point.x - projected.anchor.x, point.y - projected.anchor.y) - radius);
  if (distance === 0 || pointInPolygon(point, polygon)) return 0;
  for (let i = 0; i < polygon.length; i += 1) {
    const a = polygon[i], b = polygon[(i + 1) % polygon.length];
    const dx = b.x - a.x, dy = b.y - a.y;
    const lengthSq = dx * dx + dy * dy;
    const t = lengthSq > 0 ? Math.max(0, Math.min(1,
      ((point.x - a.x) * dx + (point.y - a.y) * dy) / lengthSq)) : 0;
    distance = Math.min(distance, Math.hypot(point.x - a.x - t * dx, point.y - a.y - t * dy));
  }
  return distance;
}

function projectedProxyIntersectsScreenRect(projected, rect) {
  const nearestX = Math.min(Math.max(projected.anchor.x, rect.minX), rect.maxX);
  const nearestY = Math.min(Math.max(projected.anchor.y, rect.minY), rect.maxY);
  if (
    Math.hypot(projected.anchor.x - nearestX, projected.anchor.y - nearestY) <=
    projected.minScreenRadiusCssPx
  ) return true;
  return polygonIntersectsRect(projected.screenPolygon, rect);
}

function polygonIntersectsRect(points, rect) {
  if (points.some((point) => pointInRect(point, rect))) return true;
  const corners = [
    { x: rect.minX, y: rect.minY },
    { x: rect.maxX, y: rect.minY },
    { x: rect.maxX, y: rect.maxY },
    { x: rect.minX, y: rect.maxY },
  ];
  if (corners.some((point) => pointInPolygon(point, points))) return true;
  for (let index = 0; index < points.length; index += 1) {
    const a = points[index];
    const b = points[(index + 1) % points.length];
    for (let edge = 0; edge < corners.length; edge += 1) {
      if (segmentsIntersect(a, b, corners[edge], corners[(edge + 1) % corners.length])) return true;
    }
  }
  return false;
}

function pointInPolygon(point, points) {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const a = points[i];
    const b = points[j];
    if (pointOnSegment(point, a, b)) return true;
    const crosses = ((a.y > point.y) !== (b.y > point.y)) &&
      (point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x);
    if (crosses) inside = !inside;
  }
  return inside;
}

function pointOnSegment(point, a, b) {
  const cross = (point.y - a.y) * (b.x - a.x) - (point.x - a.x) * (b.y - a.y);
  if (Math.abs(cross) > EPSILON) return false;
  return point.x >= Math.min(a.x, b.x) - EPSILON && point.x <= Math.max(a.x, b.x) + EPSILON &&
    point.y >= Math.min(a.y, b.y) - EPSILON && point.y <= Math.max(a.y, b.y) + EPSILON;
}

function segmentsIntersect(a, b, c, d) {
  const abC = orientation(a, b, c);
  const abD = orientation(a, b, d);
  const cdA = orientation(c, d, a);
  const cdB = orientation(c, d, b);
  if (abC * abD < 0 && cdA * cdB < 0) return true;
  return (Math.abs(abC) <= EPSILON && pointOnSegment(c, a, b)) ||
    (Math.abs(abD) <= EPSILON && pointOnSegment(d, a, b)) ||
    (Math.abs(cdA) <= EPSILON && pointOnSegment(a, c, d)) ||
    (Math.abs(cdB) <= EPSILON && pointOnSegment(b, c, d));
}

function orientation(a, b, c) {
  return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
}

function polygonFootprint(cx, cy, width, height, facing) {
  const halfW = width * 0.5;
  const halfH = height * 0.5;
  const cos = Math.cos(facing);
  const sin = Math.sin(facing);
  const points = [
    [-halfW, -halfH],
    [halfW, -halfH],
    [halfW, halfH],
    [-halfW, halfH],
  ].map(([x, y]) => Object.freeze({
    x: cx + x * cos - y * sin,
    y: cy + x * sin + y * cos,
  }));
  return Object.freeze({ kind: "polygon", points: Object.freeze(points) });
}

function circlePoints(anchor, radiusValue) {
  const radius = finitePositive(radiusValue, DEFAULT_HIT_RADIUS);
  const points = [];
  for (let index = 0; index < CIRCLE_SEGMENTS; index += 1) {
    const angle = index * Math.PI * 2 / CIRCLE_SEGMENTS;
    points.push({ x: anchor.x + Math.cos(angle) * radius, y: anchor.y + Math.sin(angle) * radius });
  }
  return points;
}

function projectedDepthAdmitted(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y) && Number.isFinite(point?.depth) &&
    point.depth > 0 && point.clip !== "behindCamera" && point.clip !== "outsideDepth";
}

function normalizeScreenRect(rect) {
  const x0 = Number(rect?.minX ?? rect?.x ?? rect?.x0);
  const y0 = Number(rect?.minY ?? rect?.y ?? rect?.y0);
  const x1 = Number(rect?.maxX ?? (Number.isFinite(rect?.w) ? x0 + rect.w : rect?.x1));
  const y1 = Number(rect?.maxY ?? (Number.isFinite(rect?.h) ? y0 + rect.h : rect?.y1));
  if (![x0, y0, x1, y1].every(Number.isFinite)) return null;
  return {
    minX: Math.min(x0, x1),
    minY: Math.min(y0, y1),
    maxX: Math.max(x0, x1),
    maxY: Math.max(y0, y1),
  };
}

function pointInRect(point, rect) {
  return point.x >= rect.minX && point.x <= rect.maxX && point.y >= rect.minY && point.y <= rect.maxY;
}

function selectionClass(kind) {
  if (isUnit(kind)) return "unit";
  if (isBuilding(kind)) return "building";
  if (isResource(kind)) return "resource";
  return "other";
}

function detachPlainRecord(value, seen = new WeakMap()) {
  if (value == null || typeof value !== "object") return value;
  if (seen.has(value)) return seen.get(value);
  if (Array.isArray(value)) {
    const out = [];
    seen.set(value, out);
    for (const item of value) out.push(detachPlainRecord(item, seen));
    return Object.freeze(out);
  }
  const out = {};
  seen.set(value, out);
  for (const [key, item] of Object.entries(value)) out[key] = detachPlainRecord(item, seen);
  return Object.freeze(out);
}

function finiteFacing(value) {
  return Number.isFinite(value) ? value : 0;
}

function finitePositive(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function finiteInteger(value, fallback) {
  return Number.isInteger(value) ? value : fallback;
}

function finiteNumber(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}
