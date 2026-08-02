import { DOODAD_TYPE, DOODAD_TYPE_IDS } from "./config.js";

export const MAP_EDITOR_MAX_DOODADS = 4096;
export const MAP_EDITOR_DEFAULT_FLOWER_COLOR = "#e8b84a";
export const MAP_EDITOR_DOODAD_TYPES = DOODAD_TYPE;
export const MAP_EDITOR_DOODAD_CATALOG = Object.freeze([
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, label: "Oak", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, label: "Pine", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_BIRCH, label: "Birch", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE, label: "Spruce", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ASPEN, label: "Aspen", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, label: "Alder", kind: "tree", perspective: "threeQuarter" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK_TOPDOWN, label: "Oak", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE_TOPDOWN, label: "Pine", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_BIRCH_TOPDOWN, label: "Birch", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE_TOPDOWN, label: "Spruce", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ASPEN_TOPDOWN, label: "Aspen", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER_TOPDOWN, label: "Alder", kind: "tree", perspective: "topDown" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, label: "Single flowers", kind: "wildflower" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_CLUSTER, label: "Flower cluster", kind: "wildflower" }),
]);

const TYPE_IDS = new Set(DOODAD_TYPE_IDS);

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
export function normalizeMapEditorDoodads(records, worldSize, { max = MAP_EDITOR_MAX_DOODADS } = {}) {
  const limit = Math.max(0, Math.min(MAP_EDITOR_MAX_DOODADS, Math.trunc(Number(max)) || 0));
  const extent = Math.trunc(Number(worldSize));
  if (!Array.isArray(records) || extent <= 0 || limit <= 0) return [];
  const retained = [];
  const usedIds = new Set();
  const deferred = [];
  for (const source of records.slice(0, limit)) {
    const record = normalizedDoodadFields(source, extent);
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
  for (let id = 1; id <= Number.MAX_SAFE_INTEGER; id += 1) if (!used.has(id)) return id;
  return 0;
}

export function createMapEditorDoodads(draft, placements, {
  typeId,
  color = null,
  max = MAP_EDITOR_MAX_DOODADS,
} = {}) {
  if (!Array.isArray(draft?.doodads) || !Array.isArray(placements) || !isMapEditorDoodadType(typeId)) return [];
  const worldSize = (draft.terrain?.length || 0) * 32;
  const available = Math.max(0, Math.min(MAP_EDITOR_MAX_DOODADS, max) - draft.doodads.length);
  if (!worldSize || !available) return [];
  const used = new Set(draft.doodads.map((record) => record.id));
  const added = [];
  for (const placement of placements) {
    if (added.length >= available) break;
    const fields = normalizedDoodadFields({ ...placement, typeId, color }, worldSize);
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

export function moveMapEditorDoodad(draft, id, point) {
  const doodadId = positiveSafeInteger(id);
  const record = draft?.doodads?.find((candidate) => candidate.id === doodadId);
  const worldSize = (draft?.terrain?.length || 0) * 32;
  const x = Math.round(Number(point?.x));
  const y = Math.round(Number(point?.y));
  if (!record || !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0 || x >= worldSize || y >= worldSize) return null;
  if (record.x === x && record.y === y) return record;
  record.x = x;
  record.y = y;
  return record;
}

export function removeMapEditorDoodads(draft, ids) {
  if (!Array.isArray(draft?.doodads)) return [];
  const removed = new Set((ids || []).map(positiveSafeInteger).filter(Boolean));
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

export function nearestMapEditorDoodad(records, point, radius = 24) {
  const candidates = doodadIdsWithinRadius(records, point, radius);
  let nearest = null;
  let nearestDistance = Infinity;
  for (const id of candidates) {
    const record = records.find((candidate) => candidate.id === id);
    const distance = (record.x - point.x) ** 2 + (record.y - point.y) ** 2;
    if (distance < nearestDistance || (distance === nearestDistance && id < nearest.id)) {
      nearest = record;
      nearestDistance = distance;
    }
  }
  return nearest || null;
}

export function symmetricDoodadPlacements(worldSize, points, symmetry = "none") {
  const extent = Math.trunc(Number(worldSize));
  if (!extent || !Array.isArray(points)) return [];
  const transforms = symmetryTransforms(symmetry);
  const placements = [];
  const seen = new Set();
  for (const point of points) {
    const source = boundedPoint(point, extent);
    if (!source) continue;
    for (const transform of transforms) {
      const next = transformPoint(source, extent, transform);
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

function normalizedDoodadFields(source, worldSize) {
  const typeId = String(source?.typeId || "");
  const x = Number(source?.x);
  const y = Number(source?.y);
  if (!TYPE_IDS.has(typeId) || !Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x >= worldSize || y >= worldSize) return null;
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

function transformPoint(point, size, transform) {
  const max = size - 1;
  if (transform === "horizontal") return { x: point.x, y: max - point.y };
  if (transform === "vertical") return { x: max - point.x, y: point.y };
  if (transform === "rotate90") return { x: max - point.y, y: point.x };
  if (transform === "rotate180") return { x: max - point.x, y: max - point.y };
  if (transform === "rotate270") return { x: point.y, y: max - point.x };
  if (transform === "diagonalMain") return { x: point.y, y: point.x };
  if (transform === "diagonalAnti") return { x: max - point.y, y: max - point.x };
  if (transform === "rotate120" || transform === "rotate240") {
    const centre = max / 2;
    const sine = transform === "rotate120" ? Math.sqrt(3) / 2 : -Math.sqrt(3) / 2;
    return boundedPoint({
      x: Math.round(centre + (point.x - centre) * -0.5 - (point.y - centre) * sine),
      y: Math.round(centre + (point.x - centre) * sine + (point.y - centre) * -0.5),
    }, size);
  }
  return { ...point };
}

function boundedPoint(point, size) {
  const x = Math.round(Number(point?.x));
  const y = Math.round(Number(point?.y));
  return Number.isFinite(x) && Number.isFinite(y) && x >= 0 && y >= 0 && x < size && y < size ? { x, y } : null;
}

function finitePoint(point) {
  const x = Number(point?.x);
  const y = Number(point?.y);
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function positiveSafeInteger(value) {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : 0;
}
