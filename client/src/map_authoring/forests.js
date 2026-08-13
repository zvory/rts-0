import {
  MAP_AUTHORING_SYMMETRY,
  symmetrySupported,
  symmetryTransforms,
  transformPoint,
} from "./symmetry.js";
import { TREE_DOODAD_GEOMETRY, doodadSizeVariation } from "../config.js";

export const FOREST_DOODAD_ID_BASE = 0x8000_0000;

const TREE_TYPES = Object.freeze(["tree.oak", "tree.pine", "tree.spruce", "tree.alder"]);
const TARGET_TREE_DENSITY = 0.23;
const MAX_GENERATED_TREES = 3_500;
const TILE_SIZE = 32;
const INTERIOR_FOLIAGE_COVERAGE = 0.95;
const EDGE_JOIN_TOLERANCE_PX = 6;
const FOLIAGE_SAMPLE_STEP_PX = 8;
const SYMMETRY_PREFERENCE = Object.freeze([
  MAP_AUTHORING_SYMMETRY.RADIAL,
  MAP_AUTHORING_SYMMETRY.QUADRANT_MIRROR,
  MAP_AUTHORING_SYMMETRY.THREE_WAY,
  MAP_AUTHORING_SYMMETRY.HALF_TURN,
  MAP_AUTHORING_SYMMETRY.HORIZONTAL,
  MAP_AUTHORING_SYMMETRY.VERTICAL,
  MAP_AUTHORING_SYMMETRY.DIAGONAL_MAIN,
  MAP_AUTHORING_SYMMETRY.DIAGONAL_ANTI,
  MAP_AUTHORING_SYMMETRY.NONE,
]);

export function normalizeForestSpans(spans, dimensions) {
  return forestSpansFromTiles(forestTilesFromSpans(spans, dimensions), dimensions);
}

export function forestTilesFromSpans(spans, dimensions) {
  const map = normalizeDimensions(dimensions);
  if (!map || !Array.isArray(spans)) return [];
  const tiles = [];
  const seen = new Set();
  for (const span of spans) {
    if (!Array.isArray(span) || span.length !== 3) continue;
    const y = Math.trunc(Number(span[0]));
    const x0 = Math.trunc(Number(span[1]));
    const x1 = Math.trunc(Number(span[2]));
    if (![y, x0, x1].every(Number.isInteger) || y < 0 || y >= map.height || x0 < 0 || x1 < x0 || x1 >= map.width) continue;
    for (let x = x0; x <= x1; x += 1) {
      const key = `${x},${y}`;
      if (!seen.has(key)) {
        seen.add(key);
        tiles.push({ x, y });
      }
    }
  }
  return tiles.sort(tileOrder);
}

export function forestSpansFromTiles(tiles, dimensions) {
  const map = normalizeDimensions(dimensions);
  if (!map || !Array.isArray(tiles)) return [];
  const rows = new Map();
  for (const tile of tiles) {
    const x = Math.trunc(Number(tile?.x));
    const y = Math.trunc(Number(tile?.y));
    if (!Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x >= map.width || y >= map.height) continue;
    const xs = rows.get(y) || new Set();
    xs.add(x);
    rows.set(y, xs);
  }
  const spans = [];
  for (const y of [...rows.keys()].sort((a, b) => a - b)) {
    const xs = [...rows.get(y)].sort((a, b) => a - b);
    for (let index = 0; index < xs.length;) {
      const x0 = xs[index];
      let x1 = x0;
      index += 1;
      while (index < xs.length && xs[index] === x1 + 1) {
        x1 = xs[index];
        index += 1;
      }
      spans.push([y, x0, x1]);
    }
  }
  return spans;
}

export function editForestSpans(spans, tiles, paint, dimensions) {
  const byKey = new Map(forestTilesFromSpans(spans, dimensions).map((tile) => [tileKey(tile), tile]));
  const changed = [];
  for (const candidate of tiles || []) {
    const tile = boundedTile(candidate, dimensions);
    if (!tile) continue;
    const key = tileKey(tile);
    if (paint && !byKey.has(key)) {
      byKey.set(key, tile);
      changed.push(tile);
    } else if (!paint && byKey.delete(key)) changed.push(tile);
  }
  return {
    spans: forestSpansFromTiles([...byKey.values()], dimensions),
    changed,
  };
}

export function generatedForestDoodads(spans, dimensions, { max = MAX_GENERATED_TREES } = {}) {
  const map = normalizeDimensions(dimensions);
  const tiles = forestTilesFromSpans(spans, map);
  if (!map || !tiles.length || max <= 0) return [];
  const tileSet = new Set(tiles.map(tileKey));
  const symmetry = strongestExactSymmetry(tileSet, map);
  const transforms = symmetryTransforms(map, symmetry);
  const limit = Math.min(MAX_GENERATED_TREES, Math.max(0, Math.trunc(max)));
  const generated = [];
  const generatedTiles = new Set();
  const boundary = tiles
    .map((tile) => ({ tile, edges: exposedForestEdges(tile, tileSet) }))
    .filter((entry) => entry.edges.length);
  const boundaryByKey = new Map(boundary.map((entry) => [tileKey(entry.tile), entry]));
  const uncoveredEdgeProbes = new Set(boundary.flatMap(({ tile, edges }) => (
    edges.map((edge) => edgeProbeKey(tile, edge))
  )));
  const visitedBoundary = new Set();

  // First seal the visible perimeter. Edge anchors are derived from foliage bounds: bottom-edge
  // roots deliberately land in the tile below the semantic forest while their canopy bottom sits
  // on the painted boundary; top and side roots are inset by their actual canopy extent.
  for (const { tile } of boundary) {
    if (generated.length >= limit || visitedBoundary.has(tileKey(tile))) continue;
    const orbit = uniqueOrbit(tile, map, transforms)
      .map((candidate) => boundaryByKey.get(tileKey(candidate.tile)))
      .filter(Boolean);
    for (const candidate of orbit) visitedBoundary.add(tileKey(candidate.tile));
    if (!orbit.some(({ tile: candidate, edges }) => (
      edges.some((edge) => uncoveredEdgeProbes.has(edgeProbeKey(candidate, edge)))
    ))) continue;
    const canonicalKey = orbit.map((candidate) => tileKey(candidate.tile)).sort()[0] || tileKey(tile);
    const typeId = bestBoundaryTreeType(orbit, canonicalKey, map, tileSet);
    if (!typeId || generated.length + orbit.length > limit) continue;
    for (const candidate of orbit) {
      const record = edgeTreeRecord(candidate.tile, candidate.edges, typeId, map);
      if (!record) continue;
      generated.push(record);
      generatedTiles.add(tileKey(candidate.tile));
      markCoveredEdgeProbes(uncoveredEdgeProbes, boundaryByKey, record);
    }
  }

  const density = Math.min(TARGET_TREE_DENSITY, Math.max(0, limit - generated.length) / tiles.length);
  const visited = new Set();
  for (const tile of tiles) {
    if (generated.length >= limit || visited.has(tileKey(tile))) continue;
    const orbit = uniqueOrbit(tile, map, transforms).filter((candidate) => tileSet.has(tileKey(candidate.tile)));
    for (const candidate of orbit) visited.add(tileKey(candidate.tile));
    if (orbit.some((candidate) => generatedTiles.has(tileKey(candidate.tile)))) continue;
    const canonicalKey = orbit.map((candidate) => tileKey(candidate.tile)).sort()[0] || tileKey(tile);
    const choice = hashUnit(canonicalKey, 17);
    if (choice >= density || generated.length + orbit.length > limit) continue;
    const preferred = TREE_TYPES[Math.min(TREE_TYPES.length - 1, Math.floor(hashUnit(canonicalKey, 29) * TREE_TYPES.length))];
    const typeId = bestInteriorTreeType(orbit, preferred, canonicalKey, map, tileSet);
    if (!typeId) continue;
    for (const candidate of orbit) {
      const record = interiorTreeRecord(candidate.tile, typeId, canonicalKey, map);
      if (record) generated.push(record);
    }
  }
  return generated.sort((left, right) => left.id - right.id);
}

export function forestTreeFoliageBounds(record) {
  const geometry = TREE_DOODAD_GEOMETRY[record?.typeId];
  if (!geometry) return null;
  const variation = doodadSizeVariation(Number(record.id));
  return {
    left: Number(record.x) + geometry.foliage.left * variation,
    right: Number(record.x) + geometry.foliage.right * variation,
    top: Number(record.y) + geometry.foliage.top * variation,
    bottom: Number(record.y) + geometry.foliage.bottom * variation,
  };
}

export function forestTreeFoliageCoverage(record, spans, dimensions) {
  const map = normalizeDimensions(dimensions);
  const bounds = forestTreeFoliageBounds(record);
  if (!map || !bounds) return 0;
  return foliageCoverage(bounds, new Set(forestTilesFromSpans(spans, map).map(tileKey)), map);
}

export function isGeneratedForestDoodad(record, dimensions) {
  return forestOwnedDoodadIds([record], dimensions).has(Number(record?.id));
}

export function forestOwnedDoodadIds(records, dimensions, { maxDoodads = 4_096 } = {}) {
  const source = Array.isArray(records) ? records : [];
  const candidates = source.filter((record) => isForestDoodadCandidate(record, dimensions));
  if (!candidates.length) return new Set();
  const definiteManualCount = source.length - candidates.length;
  const generated = generatedForestDoodads(dimensions?.forestSpans, dimensions, {
    max: Math.max(0, Math.trunc(maxDoodads) - definiteManualCount),
  });
  const expectedById = new Map(generated.map((record) => [record.id, record]));
  return new Set(candidates.filter((record) => (
    sameGeneratedDoodad(record, expectedById.get(Number(record.id)))
  )).map((record) => Number(record.id)));
}

function isForestDoodadCandidate(record, dimensions) {
  const id = Number(record?.id);
  const map = normalizeDimensions(dimensions);
  return Boolean(
    !map
      ? false
      : TREE_TYPES.includes(record?.typeId)
        && Number.isSafeInteger(id)
        && id > FOREST_DOODAD_ID_BASE
        && id <= FOREST_DOODAD_ID_BASE + map.width * map.height
        && forestSpansContainTile(
          dimensions?.forestSpans,
          (id - FOREST_DOODAD_ID_BASE - 1) % map.width,
          Math.floor((id - FOREST_DOODAD_ID_BASE - 1) / map.width),
        )
  );
}

function sameGeneratedDoodad(record, expected) {
  if (!expected) return false;
  return Number(record.id) === expected.id
    && record.typeId === expected.typeId
    && Number(record.x) === expected.x
    && Number(record.y) === expected.y
    && record.color == null;
}

function bestBoundaryTreeType(orbit, canonicalKey, map, tileSet) {
  const preferredIndex = Math.min(TREE_TYPES.length - 1, Math.floor(hashUnit(canonicalKey, 29) * TREE_TYPES.length));
  let best = null;
  for (let offset = 0; offset < TREE_TYPES.length; offset += 1) {
    const typeId = TREE_TYPES[(preferredIndex + offset) % TREE_TYPES.length];
    const coverages = orbit.map(({ tile, edges }) => {
      const record = edgeTreeRecord(tile, edges, typeId, map);
      return record ? foliageCoverage(forestTreeFoliageBounds(record), tileSet, map) : 0;
    });
    const score = Math.min(...coverages);
    if (!best || score > best.score) best = { typeId, score };
  }
  return best?.typeId || null;
}

function bestInteriorTreeType(orbit, preferred, canonicalKey, map, tileSet) {
  const ordered = [preferred, ...TREE_TYPES.filter((typeId) => typeId !== preferred)];
  let best = null;
  for (const typeId of ordered) {
    const coverages = orbit.map(({ tile }) => {
      const record = interiorTreeRecord(tile, typeId, canonicalKey, map);
      return record ? foliageCoverage(forestTreeFoliageBounds(record), tileSet, map) : 0;
    });
    const score = Math.min(...coverages);
    if (!best || score > best.score) best = { typeId, score };
    if (score >= INTERIOR_FOLIAGE_COVERAGE) return typeId;
  }
  return best?.score >= INTERIOR_FOLIAGE_COVERAGE ? best.typeId : null;
}

function edgeTreeRecord(tile, edges, typeId, map) {
  const id = generatedTreeId(tile, map);
  const geometry = TREE_DOODAD_GEOMETRY[typeId];
  if (!geometry) return null;
  const variation = doodadSizeVariation(id);
  const foliage = scaledFoliage(geometry.foliage, variation);
  const horizontalCenter = (foliage.left + foliage.right) / 2;
  const verticalCenter = (foliage.top + foliage.bottom) / 2;
  let x = (tile.x + 0.5) * TILE_SIZE - horizontalCenter;
  let y = (tile.y + 0.5) * TILE_SIZE - verticalCenter;
  if (edges.includes("left")) x = tile.x * TILE_SIZE - foliage.left;
  else if (edges.includes("right")) x = (tile.x + 1) * TILE_SIZE - foliage.right;
  if (edges.includes("top")) y = tile.y * TILE_SIZE - foliage.top;
  else if (edges.includes("bottom")) y = (tile.y + 1) * TILE_SIZE - foliage.bottom;
  return boundedTreeRecord({ id, typeId, x: Math.round(x), y: Math.round(y) }, map);
}

function interiorTreeRecord(tile, typeId, canonicalKey, map) {
  const id = generatedTreeId(tile, map);
  const geometry = TREE_DOODAD_GEOMETRY[typeId];
  if (!geometry) return null;
  const variation = doodadSizeVariation(id);
  const foliage = scaledFoliage(geometry.foliage, variation);
  const horizontalCenter = (foliage.left + foliage.right) / 2;
  const verticalCenter = (foliage.top + foliage.bottom) / 2;
  const localKey = `${canonicalKey}:${tileKey(tile)}`;
  const jitterX = Math.round((hashUnit(localKey, 41) - 0.5) * 12);
  const jitterY = Math.round((hashUnit(localKey, 53) - 0.5) * 12);
  return boundedTreeRecord({
    id,
    typeId,
    x: Math.round((tile.x + 0.5) * TILE_SIZE - horizontalCenter + jitterX),
    y: Math.round((tile.y + 0.5) * TILE_SIZE - verticalCenter + jitterY),
  }, map);
}

function boundedTreeRecord(record, map) {
  const worldWidth = map.width * TILE_SIZE;
  const worldHeight = map.height * TILE_SIZE;
  if (record.x < 0 || record.y < 0 || record.x >= worldWidth || record.y >= worldHeight) return null;
  return record;
}

function foliageCoverage(bounds, tileSet, map) {
  if (!bounds) return 0;
  const centreX = (bounds.left + bounds.right) / 2;
  const centreY = (bounds.top + bounds.bottom) / 2;
  const radiusX = Math.max(1, (bounds.right - bounds.left) / 2);
  const radiusY = Math.max(1, (bounds.bottom - bounds.top) / 2);
  let inside = 0;
  let sampled = 0;
  for (let y = bounds.top + FOLIAGE_SAMPLE_STEP_PX / 2; y < bounds.bottom; y += FOLIAGE_SAMPLE_STEP_PX) {
    for (let x = bounds.left + FOLIAGE_SAMPLE_STEP_PX / 2; x < bounds.right; x += FOLIAGE_SAMPLE_STEP_PX) {
      const dx = (x - centreX) / radiusX;
      const dy = (y - centreY) / radiusY;
      if (dx * dx + dy * dy > 1) continue;
      sampled += 1;
      const tileX = Math.floor(x / TILE_SIZE);
      const tileY = Math.floor(y / TILE_SIZE);
      if (tileX >= 0 && tileY >= 0 && tileX < map.width && tileY < map.height && tileSet.has(`${tileX},${tileY}`)) inside += 1;
    }
  }
  return sampled ? inside / sampled : 0;
}

function markCoveredEdgeProbes(uncovered, boundaryByKey, record) {
  const bounds = forestTreeFoliageBounds(record);
  if (!bounds) return;
  const minX = Math.floor((bounds.left - EDGE_JOIN_TOLERANCE_PX) / TILE_SIZE) - 1;
  const maxX = Math.floor((bounds.right + EDGE_JOIN_TOLERANCE_PX) / TILE_SIZE) + 1;
  const minY = Math.floor((bounds.top - EDGE_JOIN_TOLERANCE_PX) / TILE_SIZE) - 1;
  const maxY = Math.floor((bounds.bottom + EDGE_JOIN_TOLERANCE_PX) / TILE_SIZE) + 1;
  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      const candidate = boundaryByKey.get(`${x},${y}`);
      if (!candidate) continue;
      for (const edge of candidate.edges) {
        const point = edgeProbePoint(candidate.tile, edge);
        if (
          point.x >= bounds.left - EDGE_JOIN_TOLERANCE_PX
          && point.x <= bounds.right + EDGE_JOIN_TOLERANCE_PX
          && point.y >= bounds.top - EDGE_JOIN_TOLERANCE_PX
          && point.y <= bounds.bottom + EDGE_JOIN_TOLERANCE_PX
        ) uncovered.delete(edgeProbeKey(candidate.tile, edge));
      }
    }
  }
}

function exposedForestEdges(tile, tileSet) {
  const edges = [];
  if (!tileSet.has(`${tile.x},${tile.y - 1}`)) edges.push("top");
  if (!tileSet.has(`${tile.x + 1},${tile.y}`)) edges.push("right");
  if (!tileSet.has(`${tile.x},${tile.y + 1}`)) edges.push("bottom");
  if (!tileSet.has(`${tile.x - 1},${tile.y}`)) edges.push("left");
  return edges;
}

function edgeProbePoint(tile, edge) {
  if (edge === "top") return { x: (tile.x + 0.5) * TILE_SIZE, y: tile.y * TILE_SIZE };
  if (edge === "right") return { x: (tile.x + 1) * TILE_SIZE, y: (tile.y + 0.5) * TILE_SIZE };
  if (edge === "bottom") return { x: (tile.x + 0.5) * TILE_SIZE, y: (tile.y + 1) * TILE_SIZE };
  return { x: tile.x * TILE_SIZE, y: (tile.y + 0.5) * TILE_SIZE };
}

function edgeProbeKey(tile, edge) { return `${tile.x},${tile.y}:${edge}`; }
function generatedTreeId(tile, map) { return FOREST_DOODAD_ID_BASE + tile.y * map.width + tile.x + 1; }
function forestSpansContainTile(spans, x, y) {
  return Array.isArray(spans) && spans.some((span) => (
    Array.isArray(span) && span.length === 3 && Number(span[0]) === y
    && Number(span[1]) <= x && x <= Number(span[2])
  ));
}
function scaledFoliage(foliage, variation) {
  return {
    left: foliage.left * variation,
    right: foliage.right * variation,
    top: foliage.top * variation,
    bottom: foliage.bottom * variation,
  };
}

function strongestExactSymmetry(tileSet, dimensions) {
  for (const symmetry of SYMMETRY_PREFERENCE) {
    if (!symmetrySupported(dimensions, symmetry)) continue;
    const transforms = symmetryTransforms(dimensions, symmetry);
    if (transforms.every((transform) => [...tileSet].every((key) => {
      const transformed = transformPoint(pointFromKey(key), dimensions, transform);
      return transformed && tileSet.has(tileKey(transformed));
    }))) return symmetry;
  }
  return MAP_AUTHORING_SYMMETRY.NONE;
}

function uniqueOrbit(tile, dimensions, transforms) {
  const byKey = new Map();
  for (const transform of transforms) {
    const transformed = transformPoint(tile, dimensions, transform);
    if (transformed) byKey.set(tileKey(transformed), { tile: transformed, transform });
  }
  return [...byKey.values()].sort((left, right) => tileOrder(left.tile, right.tile));
}

function boundedTile(value, dimensions) {
  const map = normalizeDimensions(dimensions);
  const x = Math.trunc(Number(value?.x));
  const y = Math.trunc(Number(value?.y));
  return map && Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < map.width && y < map.height
    ? { x, y }
    : null;
}

function normalizeDimensions(value) {
  const width = Math.trunc(Number(value?.width));
  const height = Math.trunc(Number(value?.height));
  return width > 0 && height > 0 ? { width, height } : null;
}

function hashUnit(key, salt) {
  let value = salt >>> 0;
  for (let index = 0; index < key.length; index += 1) {
    value ^= key.charCodeAt(index);
    value = Math.imul(value, 0x01000193);
  }
  value ^= value >>> 16;
  value = Math.imul(value, 0x21f0aaad);
  value ^= value >>> 15;
  return (value >>> 0) / 0x1_0000_0000;
}

function tileKey(tile) { return `${tile.x},${tile.y}`; }
function pointFromKey(key) { const [x, y] = key.split(",").map(Number); return { x, y }; }
function tileOrder(left, right) { return left.y - right.y || left.x - right.x; }
