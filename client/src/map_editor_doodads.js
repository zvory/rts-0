import { DOODAD_TYPE, DOODAD_TYPE_IDS } from "./config.js";

export const MAP_EDITOR_MAX_DOODADS = 4096;
export const MAP_EDITOR_DEFAULT_FLOWER_COLOR = "#e8b84a";
export const MAP_EDITOR_DOODAD_TYPES = DOODAD_TYPE;
export const MAP_EDITOR_DOODAD_CATALOG = Object.freeze([
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, label: "Oak", kind: "tree" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, label: "Pine", kind: "tree" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE, label: "Spruce", kind: "tree" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, label: "Alder", kind: "tree" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, label: "Single flowers", kind: "wildflower" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_CLUSTER, label: "Flower cluster", kind: "wildflower" }),
]);

const TYPE_IDS = new Set(DOODAD_TYPE_IDS);
const MAX_U32 = 0xffff_ffff;

export function isMapEditorDoodadType(typeId) {
  return TYPE_IDS.has(typeId);
}

export function isWildflowerDoodadType(typeId) {
  return typeof typeId === "string" && typeId.startsWith("wildflower.") && TYPE_IDS.has(typeId);
}

export function canonicalDoodadColor(value, fallback = null) {
  const text = String(value || "").trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(text)) return text;
  const short = text.match(/^#([0-9a-f])([0-9a-f])([0-9a-f])$/);
  if (short) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`;
  return fallback;
}

/** Normalize imported authoring records and deterministically repair missing/duplicate ids. */
export function normalizeMapEditorDoodads(records, worldDimensions, { max = MAP_EDITOR_MAX_DOODADS } = {}) {
  const limit = Math.max(0, Math.min(MAP_EDITOR_MAX_DOODADS, Math.trunc(Number(max)) || 0));
  const dimensions = normalizedWorldDimensions(worldDimensions);
  if (!Array.isArray(records) || !dimensions || limit <= 0) return [];
  const retained = [];
  const usedIds = new Set();
  const deferred = [];
  for (const source of records.slice(0, limit)) {
    const record = normalizedDoodadFields(source, dimensions);
    if (!record) continue;
    const id = positiveSafeInteger(source?.id);
    if (id && !usedIds.has(id)) {
      usedIds.add(id);
      retained.push({ ...record, id });
    } else {
      deferred.push(record);
    }
  }
  for (const record of deferred) {
    const id = allocateMapEditorDoodadId(retained, usedIds);
    if (!id) break;
    usedIds.add(id);
    retained.push({ ...record, id });
  }
  retained.sort((left, right) => left.id - right.id);
  return retained.slice(0, limit);
}

export function allocateMapEditorDoodadId(records, existing = null) {
  const used = existing || new Set((records || []).map((record) => positiveSafeInteger(record?.id)).filter(Boolean));
  for (let id = 1; id <= MAX_U32; id += 1) if (!used.has(id)) return id;
  return 0;
}

export function createMapEditorDoodads(draft, placements, {
  typeId,
  color = null,
  max = MAP_EDITOR_MAX_DOODADS,
} = {}) {
  if (!Array.isArray(draft?.doodads) || !Array.isArray(placements) || !isMapEditorDoodadType(typeId)) return [];
  const dimensions = draftWorldDimensions(draft);
  const available = Math.max(0, Math.min(MAP_EDITOR_MAX_DOODADS, max) - draft.doodads.length);
  if (!dimensions || !available) return [];
  const used = new Set(draft.doodads.map((record) => record.id));
  const added = [];
  for (const placement of placements) {
    if (added.length >= available) break;
    const fields = normalizedDoodadFields({ ...placement, typeId, color }, dimensions);
    if (!fields) continue;
    const id = allocateMapEditorDoodadId(draft.doodads, used);
    if (!id) break;
    used.add(id);
    const record = { id, ...fields };
    draft.doodads.push(record);
    added.push(record);
  }
  draft.doodads.sort((left, right) => left.id - right.id);
  return added;
}

export function removeMapEditorDoodads(draft, ids) {
  if (!Array.isArray(draft?.doodads)) return [];
  const removed = new Set(Array.from(ids || [], positiveSafeInteger).filter(Boolean));
  if (!removed.size) return [];
  const existing = new Set(draft.doodads.map((record) => record.id));
  const actual = [...removed].filter((id) => existing.has(id));
  if (!actual.length) return [];
  draft.doodads = draft.doodads.filter((record) => !removed.has(record.id));
  return actual;
}

export function doodadIdsWithinRadius(records, point, radius) {
  const x = Number(point?.x);
  const y = Number(point?.y);
  const r = Math.max(0, Number(radius) || 0);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return [];
  const squared = r * r;
  return (records || []).filter((record) => {
    const dx = record.x - x;
    const dy = record.y - y;
    return dx * dx + dy * dy <= squared;
  }).map((record) => record.id);
}

export function doodadIdsWithinRect(records, from, to) {
  const x0 = Number(from?.x);
  const y0 = Number(from?.y);
  const x1 = Number(to?.x);
  const y1 = Number(to?.y);
  if (![x0, y0, x1, y1].every(Number.isFinite)) return [];
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  return (records || [])
    .filter((record) => record.x >= minX && record.x <= maxX && record.y >= minY && record.y <= maxY)
    .map((record) => record.id);
}

export function symmetricDoodadPlacements(worldDimensions, points, symmetry = "none") {
  const dimensions = normalizedWorldDimensions(worldDimensions);
  if (!dimensions || !Array.isArray(points)) return [];
  const transforms = symmetryTransforms(symmetry);
  const placements = [];
  const seen = new Set();
  for (const point of points) {
    const source = boundedPoint(point, dimensions);
    if (!source) continue;
    for (const transform of transforms) {
      const next = transformPoint(source, dimensions, transform);
      if (!next) continue;
      const key = `${next.x},${next.y}`;
      if (seen.has(key)) continue;
      seen.add(key);
      placements.push(next);
    }
  }
  return placements;
}

/** Stateful fixed-distance path sampler. Segment subdivision does not change its output. */
export function createDoodadSprayStroke(point, { radius = 48, density = 4, seed = 1 } = {}) {
  const start = finitePoint(point);
  if (!start) return null;
  const normalizedRadius = Math.max(4, Math.min(256, Number(radius) || 48));
  const normalizedDensity = Math.max(1, Math.min(12, Math.trunc(Number(density)) || 4));
  const stroke = {
    last: start,
    distanceToNext: spraySpacing(normalizedRadius, normalizedDensity),
    radius: normalizedRadius,
    density: normalizedDensity,
    seed: positiveSafeInteger(seed) || 1,
    ordinal: 0,
  };
  return { stroke, placements: [sprayPlacement(stroke, start)] };
}

export function extendDoodadSprayStroke(stroke, point) {
  const end = finitePoint(point);
  if (!stroke?.last || !end) return [];
  const placements = [];
  let from = stroke.last;
  let dx = end.x - from.x;
  let dy = end.y - from.y;
  let remaining = Math.hypot(dx, dy);
  while (remaining + 1e-9 >= stroke.distanceToNext) {
    const ratio = stroke.distanceToNext / remaining;
    from = { x: from.x + dx * ratio, y: from.y + dy * ratio };
    placements.push(sprayPlacement(stroke, from));
    dx = end.x - from.x;
    dy = end.y - from.y;
    remaining = Math.hypot(dx, dy);
    stroke.distanceToNext = spraySpacing(stroke.radius, stroke.density);
  }
  stroke.distanceToNext -= remaining;
  stroke.last = end;
  return placements;
}

function normalizedDoodadFields(source, dimensions) {
  const typeId = String(source?.typeId || "");
  const x = Number(source?.x);
  const y = Number(source?.y);
  if (
    !TYPE_IDS.has(typeId) || !Number.isInteger(x) || !Number.isInteger(y)
    || x < 0 || y < 0 || x >= dimensions.width || y >= dimensions.height
  ) return null;
  const record = { typeId, x, y };
  if (isWildflowerDoodadType(typeId)) {
    const color = canonicalDoodadColor(source?.color);
    if (color) record.color = color;
  }
  return record;
}

function spraySpacing(radius, density) {
  return Math.max(4, radius / (density * 0.75));
}

function sprayPlacement(stroke, centre) {
  const angle = hashUnit(stroke.seed, stroke.ordinal * 2) * Math.PI * 2;
  const distance = Math.sqrt(hashUnit(stroke.seed, stroke.ordinal * 2 + 1)) * stroke.radius;
  stroke.ordinal += 1;
  return {
    x: Math.round(centre.x + Math.cos(angle) * distance),
    y: Math.round(centre.y + Math.sin(angle) * distance),
  };
}

function hashUnit(seed, index) {
  let value = (seed ^ Math.imul(index + 1, 0x9e3779b1)) >>> 0;
  value ^= value >>> 16;
  value = Math.imul(value, 0x21f0aaad);
  value ^= value >>> 15;
  value = Math.imul(value, 0x735a2d97);
  value ^= value >>> 15;
  return (value >>> 0) / 0x100000000;
}

function symmetryTransforms(symmetry) {
  if (symmetry === "horizontal") return ["identity", "horizontal"];
  if (symmetry === "vertical") return ["identity", "vertical"];
  if (symmetry === "halfTurn") return ["identity", "rotate180"];
  if (symmetry === "threeWay") return ["identity", "rotate120", "rotate240"];
  if (symmetry === "radial") return ["identity", "rotate90", "rotate180", "rotate270"];
  if (symmetry === "diagonalMain") return ["identity", "diagonalMain"];
  if (symmetry === "diagonalAnti") return ["identity", "diagonalAnti"];
  return ["identity"];
}

function transformPoint(point, dimensions, transform) {
  const maxX = dimensions.width - 1;
  const maxY = dimensions.height - 1;
  if (transform === "horizontal") return { x: point.x, y: maxY - point.y };
  if (transform === "vertical") return { x: maxX - point.x, y: point.y };
  if (transform === "rotate90") return boundedPoint({ x: maxY - point.y, y: point.x }, dimensions);
  if (transform === "rotate180") return { x: maxX - point.x, y: maxY - point.y };
  if (transform === "rotate270") return boundedPoint({ x: point.y, y: maxX - point.x }, dimensions);
  if (transform === "diagonalMain") return { x: point.y, y: point.x };
  if (transform === "diagonalAnti") return { x: maxY - point.y, y: maxX - point.x };
  if (transform === "rotate120" || transform === "rotate240") {
    const centreX = maxX / 2;
    const centreY = maxY / 2;
    const sine = transform === "rotate120" ? Math.sqrt(3) / 2 : -Math.sqrt(3) / 2;
    return boundedPoint({
      x: Math.round(centreX + (point.x - centreX) * -0.5 - (point.y - centreY) * sine),
      y: Math.round(centreY + (point.x - centreX) * sine + (point.y - centreY) * -0.5),
    }, dimensions);
  }
  return { ...point };
}

function boundedPoint(point, dimensions) {
  const x = Math.round(Number(point?.x));
  const y = Math.round(Number(point?.y));
  return Number.isFinite(x) && Number.isFinite(y)
    && x >= 0 && y >= 0 && x < dimensions.width && y < dimensions.height
    ? { x, y }
    : null;
}

function normalizedWorldDimensions(value) {
  if (typeof value === "number") {
    const size = Math.trunc(value);
    return size > 0 ? { width: size, height: size } : null;
  }
  const width = Math.trunc(Number(value?.width));
  const height = Math.trunc(Number(value?.height));
  return width > 0 && height > 0 ? { width, height } : null;
}

function draftWorldDimensions(draft) {
  const widthTiles = Math.trunc(Number(draft?.width))
    || (typeof draft?.terrain?.[0] === "string" ? [...draft.terrain[0]].length : 0);
  const heightTiles = Math.trunc(Number(draft?.height))
    || (Array.isArray(draft?.terrain) ? draft.terrain.length : 0);
  return normalizedWorldDimensions({ width: widthTiles * 32, height: heightTiles * 32 });
}

function finitePoint(point) {
  const x = Number(point?.x);
  const y = Number(point?.y);
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function positiveSafeInteger(value) {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 && number <= MAX_U32 ? number : 0;
}
