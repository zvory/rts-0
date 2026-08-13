import { DOODAD_TYPE, DOODAD_TYPE_IDS } from "./config.js";
import { expandSymmetricPoints } from "./map_authoring/symmetry.js";

export const MAP_EDITOR_MAX_DOODADS = 4096;
export const MAP_EDITOR_MAX_SPRAY_DENSITY = 256;
export const MAP_EDITOR_DEFAULT_FLOWER_COLOR = "#e8b84a";
export const MAP_EDITOR_DOODAD_TYPES = DOODAD_TYPE;
export const MAP_EDITOR_DOODAD_CATALOG = Object.freeze([
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, label: "Single flowers", kind: "wildflower" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_CLUSTER, label: "Flower cluster", kind: "wildflower" }),
  Object.freeze({ typeId: MAP_EDITOR_DOODAD_TYPES.TANK_TRAP, label: "Tank Trap", kind: "neutral-unit" }),
]);

const TYPE_IDS = new Set(DOODAD_TYPE_IDS);
const MAX_U32 = 0xffff_ffff;

export function isMapEditorDoodadType(typeId) {
  return TYPE_IDS.has(typeId);
}

export function isWildflowerDoodadType(typeId) {
  return typeof typeId === "string" && typeId.startsWith("wildflower.") && TYPE_IDS.has(typeId);
}

export function isTreeDoodadType(typeId) {
  return typeof typeId === "string" && typeId.startsWith("tree.") && TYPE_IDS.has(typeId);
}

export function isTankTrapDoodadType(typeId) {
  return typeId === MAP_EDITOR_DOODAD_TYPES.TANK_TRAP;
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
  return firstAvailableDoodadId(used);
}

export function createMapEditorDoodads(draft, placements, {
  typeId,
  color = null,
  max = MAP_EDITOR_MAX_DOODADS,
} = {}) {
  if (!Array.isArray(draft?.doodads) || !Array.isArray(placements) || !isMapEditorDoodadType(typeId)) return [];
  return createMapEditorDoodadRecords(
    draft,
    placements.map((placement) => ({ ...placement, typeId, color })),
    { max },
  );
}

export function createMapEditorDoodadRecords(draft, placements, {
  max = MAP_EDITOR_MAX_DOODADS,
} = {}) {
  if (!Array.isArray(draft?.doodads) || !Array.isArray(placements)) return [];
  const dimensions = draftWorldDimensions(draft);
  const available = Math.max(0, Math.min(MAP_EDITOR_MAX_DOODADS, max) - draft.doodads.length);
  if (!dimensions || !available) return [];
  const used = new Set(draft.doodads.map((record) => record.id));
  let nextId = firstAvailableDoodadId(used);
  const added = [];
  for (const placement of placements) {
    if (added.length >= available) break;
    const fields = normalizedDoodadFields(placement, dimensions);
    if (!fields) continue;
    const id = nextId;
    if (!id) break;
    used.add(id);
    nextId = firstAvailableDoodadId(used, id + 1);
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

export function symmetricDoodadPlacements(worldDimensions, points, symmetry = "none") {
  const dimensions = normalizedWorldDimensions(worldDimensions);
  return dimensions
    ? expandSymmetricPoints(dimensions, points, symmetry, { decorate: ({ x, y }) => ({ x, y }) })
    : [];
}

/** Stateful fixed-distance path sampler. Segment subdivision does not change its output. */
export function createDoodadSprayStroke(point, { radius = 48, density = 4, seed = 1 } = {}) {
  const start = finitePoint(point);
  if (!start) return null;
  const normalizedRadius = Math.max(4, Math.min(256, Number(radius) || 48));
  const normalizedDensity = Math.max(1, Math.min(MAP_EDITOR_MAX_SPRAY_DENSITY, Math.trunc(Number(density)) || 4));
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

export function doodadTypeFromSelection(typeIds, seed = 1) {
  const choices = [...new Set(typeIds || [])].filter(isMapEditorDoodadType);
  if (!choices.length) return null;
  const index = Math.floor(hashUnit(positiveSafeInteger(seed) || 1, choices.length) * choices.length);
  return choices[Math.min(choices.length - 1, index)];
}

function normalizedDoodadFields(source, dimensions) {
  const typeId = String(source?.typeId || "");
  const sourceX = Number(source?.x);
  const sourceY = Number(source?.y);
  const x = isTankTrapDoodadType(typeId) && Number.isInteger(sourceX) ? tileCenter(sourceX) : sourceX;
  const y = isTankTrapDoodadType(typeId) && Number.isInteger(sourceY) ? tileCenter(sourceY) : sourceY;
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

function tileCenter(worldCoordinate) {
  return Math.floor(worldCoordinate / 32) * 32 + 16;
}

function spraySpacing(radius, density) {
  return Math.max(1, radius / (density * 0.75));
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

function firstAvailableDoodadId(used, start = 1) {
  for (let id = start; id <= MAX_U32; id += 1) if (!used.has(id)) return id;
  return 0;
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
