import { transformRoadCharacter } from "./operations.js";
import {
  MAP_AUTHORING_SYMMETRY,
  normalizeDimensions,
  symmetrySupported,
  symmetryTransforms,
  transformPoint,
} from "./symmetry.js";

const TILE_SIZE_PX = 32;

/** Advisory symmetry checks shared by browser and Node authoring surfaces. */
export function mapSymmetryWarnings(map, symmetry = MAP_AUTHORING_SYMMETRY.NONE) {
  if (symmetry === MAP_AUTHORING_SYMMETRY.NONE || symmetry == null) return [];
  if (!Object.values(MAP_AUTHORING_SYMMETRY).includes(symmetry)) {
    return [`unsupported symmetry check ${JSON.stringify(symmetry)}`];
  }
  const dimensions = normalizeDimensions(map);
  if (!dimensions) return [];
  if (!symmetrySupported(dimensions, symmetry)) {
    return [`symmetry ${JSON.stringify(symmetry)} requires a square map`];
  }
  const warnings = [];
  if (rectangularTerrain(map, dimensions)) {
    const mismatches = terrainMismatchCount(map, dimensions, symmetry);
    if (mismatches) warnings.push(`terrain has ${mismatches} ${symmetry} symmetry mismatches`);
  }
  pushLocationWarnings(warnings, "start locations", map.startLocations, dimensions, symmetry);
  pushLocationWarnings(warnings, "base locations", map.baseSites, dimensions, symmetry, baseResourcesEqual);
  pushLocationWarnings(warnings, "stealth tiles", map.stealthTiles, dimensions, symmetry);
  pushLocationWarnings(warnings, "no-vehicle tiles", map.noVehicleTiles, dimensions, symmetry);
  pushDoodadWarnings(warnings, map.doodads, dimensions, symmetry);
  return warnings;
}

function terrainMismatchCount(map, dimensions, symmetry) {
  let mismatches = 0;
  const transforms = symmetryTransforms(dimensions, symmetry).slice(1);
  const background = dominantTerrainCharacter(map.terrain);
  for (let y = 0; y < dimensions.height; y += 1) {
    for (let x = 0; x < dimensions.width; x += 1) {
      const source = map.terrain[y][x];
      // Checking authored features is sufficient: if one symmetric copy is erased to the dominant
      // background, another retained copy still reports the missing partner. This also avoids
      // false positives from the documented square-grid approximation of three-way rotation.
      if (source === background) continue;
      if (transforms.some((transform) => {
        const target = transformPoint({ x, y }, dimensions, transform);
        const expected = transformRoadCharacter(source, transform);
        return target && !matchingTerrainNear(map, target, expected, symmetry);
      })) mismatches += 1;
    }
  }
  return mismatches;
}

function dominantTerrainCharacter(rows) {
  const counts = new Map();
  for (const row of rows) for (const character of row) counts.set(character, (counts.get(character) || 0) + 1);
  let dominant = null;
  let count = -1;
  for (const [character, candidate] of counts) {
    if (candidate > count) {
      dominant = character;
      count = candidate;
    }
  }
  return dominant;
}

function pushLocationWarnings(warnings, label, records, dimensions, symmetry, equal = () => true) {
  if (!Array.isArray(records)) return;
  const valid = records.filter((record) => boundedRecord(record, dimensions));
  const byLocation = new Map(valid.map((record) => [locationKey(record), record]));
  let missing = 0;
  let differing = 0;
  for (const source of valid) {
    for (const transform of symmetryTransforms(dimensions, symmetry).slice(1)) {
      const target = transformPoint(source, dimensions, transform);
      const partner = target && findLocationPartner(valid, byLocation, target, symmetry);
      if (!partner) missing += 1;
      else if (!equal(source, partner)) differing += 1;
    }
  }
  if (missing) warnings.push(`${label} have ${missing} missing ${symmetry} partners`);
  if (differing) warnings.push(`${label} have ${differing} ${symmetry} resource mismatches`);
}

function pushDoodadWarnings(warnings, records, mapDimensions, symmetry) {
  if (!Array.isArray(records)) return;
  const dimensions = {
    width: mapDimensions.width * TILE_SIZE_PX,
    height: mapDimensions.height * TILE_SIZE_PX,
  };
  const valid = records.filter((record) => boundedRecord(record, dimensions));
  const keys = new Set(valid.map(doodadKey));
  let missing = 0;
  for (const source of valid) {
    for (const transform of symmetryTransforms(dimensions, symmetry).slice(1)) {
      let target = transformPoint(source, dimensions, transform);
      if (!target) continue;
      if (source.typeId === "unit.tank_trap") target = snapToTileCentre(target);
      const expected = { ...source, ...target };
      const matched = keys.has(doodadKey(expected)) || (symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY
        && valid.some((candidate) => doodadAttributesEqual(candidate, expected) && nearby(candidate, expected)));
      if (!matched) missing += 1;
    }
  }
  if (missing) warnings.push(`doodads have ${missing} missing ${symmetry} partners`);
}

function rectangularTerrain(map, dimensions) {
  return Array.isArray(map.terrain)
    && map.terrain.length === dimensions.height
    && map.terrain.every((row) => typeof row === "string" && [...row].length === dimensions.width);
}

function boundedRecord(record, dimensions) {
  return record && Number.isInteger(record.x) && Number.isInteger(record.y)
    && record.x >= 0 && record.y >= 0
    && record.x < dimensions.width && record.y < dimensions.height;
}

function baseResourcesEqual(left, right) {
  return left.steelPatches === right.steelPatches && left.oilPatches === right.oilPatches;
}

function matchingTerrainNear(map, target, expected, symmetry) {
  const radius = symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY ? 1 : 0;
  for (let y = target.y - radius; y <= target.y + radius; y += 1) {
    for (let x = target.x - radius; x <= target.x + radius; x += 1) {
      if (terrainEquivalent(map.terrain[y]?.[x], expected)) return true;
    }
  }
  return false;
}

function terrainEquivalent(left, right) {
  return left === right;
}

function findLocationPartner(records, byLocation, target, symmetry) {
  return byLocation.get(locationKey(target)) || (symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY
    ? records.find((candidate) => nearby(candidate, target))
    : null);
}

function nearby(left, right) {
  const dx = left.x - right.x;
  const dy = left.y - right.y;
  return dx * dx + dy * dy <= 2;
}

function doodadAttributesEqual(left, right) {
  return left.typeId === right.typeId && (left.color || "") === (right.color || "");
}

function doodadKey(record) {
  return `${record.typeId}:${record.x},${record.y}:${record.color || ""}`;
}

function locationKey(record) {
  return `${record.x},${record.y}`;
}

function snapToTileCentre(point) {
  return {
    x: Math.floor(point.x / TILE_SIZE_PX) * TILE_SIZE_PX + TILE_SIZE_PX / 2,
    y: Math.floor(point.y / TILE_SIZE_PX) * TILE_SIZE_PX + TILE_SIZE_PX / 2,
  };
}
