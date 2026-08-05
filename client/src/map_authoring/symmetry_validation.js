import { transformRoadCharacter } from "./operations.js";
import { forestTilesFromSpans } from "./forests.js";
import {
  MAP_AUTHORING_SYMMETRY,
  expandSymmetricPoints,
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
  pushLocationWarnings(warnings, "forest tiles", forestTilesFromSpans(map.forestSpans, dimensions), dimensions, symmetry);
  pushLocationWarnings(warnings, "concealment tiles", map.concealmentTiles, dimensions, symmetry);
  pushLocationWarnings(warnings, "no-vehicle tiles", map.noVehicleTiles, dimensions, symmetry);
  pushLocationWarnings(warnings, "damage-reduction tiles", map.damageReductionTiles, dimensions, symmetry);
  pushLocationWarnings(warnings, "slow-movement tiles", map.slowMovementTiles, dimensions, symmetry);
  pushDoodadWarnings(warnings, map.doodads, dimensions, symmetry);
  return warnings;
}

function terrainMismatchCount(map, dimensions, symmetry) {
  const transforms = symmetryTransforms(dimensions, symmetry).slice(1);
  const background = dominantTerrainCharacter(map.terrain);
  const backgroundIsInvariant = transforms.every((transform) => (
    transformRoadCharacter(background, transform) === background
  ));
  if (symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY) {
    return generatedTerrainMismatchCount(map, dimensions, background, backgroundIsInvariant);
  }
  let mismatches = 0;
  for (let y = 0; y < dimensions.height; y += 1) {
    for (let x = 0; x < dimensions.width; x += 1) {
      const source = map.terrain[y][x];
      // Checking authored features is sufficient: if one symmetric copy is erased to the dominant
      // background, another retained copy still reports the missing partner. This also avoids
      // false positives from the documented square-grid approximation of three-way rotation.
      if (backgroundIsInvariant && source === background) continue;
      if (transforms.some((transform) => {
        const target = transformPoint({ x, y }, dimensions, transform);
        const expected = transformRoadCharacter(source, transform);
        return target && map.terrain[target.y]?.[target.x] !== expected;
      })) mismatches += 1;
    }
  }
  return mismatches;
}

function generatedTerrainMismatchCount(map, dimensions, background, backgroundIsInvariant) {
  const records = [];
  for (let y = 0; y < dimensions.height; y += 1) {
    for (let x = 0; x < dimensions.width; x += 1) {
      const character = map.terrain[y][x];
      if (!backgroundIsInvariant || character !== background) records.push({ x, y, character });
    }
  }
  const covered = new Set();
  for (const seed of records) {
    const orbit = expandSymmetricPoints(dimensions, [seed], MAP_AUTHORING_SYMMETRY.THREE_WAY, {
      decorate: (point, transform) => ({
        ...point,
        character: transformRoadCharacter(seed.character, transform),
      }),
    });
    if (orbit.every((expected) => map.terrain[expected.y]?.[expected.x] === expected.character)) {
      for (const expected of orbit) covered.add(locationKey(expected));
    }
  }
  return records.reduce((count, record) => count + (covered.has(locationKey(record)) ? 0 : 1), 0);
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
  if (symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY) {
    pushGeneratedLocationWarnings(warnings, label, valid, dimensions, equal);
    return;
  }
  const byLocation = new Map(valid.map((record) => [locationKey(record), record]));
  let missing = 0;
  let differing = 0;
  for (const source of valid) {
    for (const transform of symmetryTransforms(dimensions, symmetry).slice(1)) {
      const target = transformPoint(source, dimensions, transform);
      const partner = target && byLocation.get(locationKey(target));
      if (!partner) missing += 1;
      else if (!equal(source, partner)) differing += 1;
    }
  }
  if (missing) warnings.push(`${label} have ${missing} missing ${symmetry} partners`);
  if (differing) warnings.push(`${label} have ${differing} ${symmetry} resource mismatches`);
}

function pushGeneratedLocationWarnings(warnings, label, records, dimensions, equal) {
  const byLocation = new Map(records.map((record) => [locationKey(record), record]));
  const geometryCovered = new Set();
  const attributesCovered = new Set();
  for (const seed of records) {
    const orbit = expandSymmetricPoints(dimensions, [seed], MAP_AUTHORING_SYMMETRY.THREE_WAY);
    const partners = orbit.map((expected) => byLocation.get(locationKey(expected)));
    if (partners.every(Boolean)) {
      for (const expected of orbit) geometryCovered.add(locationKey(expected));
      if (partners.every((partner) => equal(seed, partner))) {
        for (const expected of orbit) attributesCovered.add(locationKey(expected));
      }
    }
  }
  const missing = records.filter((record) => !geometryCovered.has(locationKey(record))).length;
  const differing = records.filter((record) => (
    geometryCovered.has(locationKey(record)) && !attributesCovered.has(locationKey(record))
  )).length;
  if (missing) warnings.push(`${label} have ${missing} missing threeWay partners`);
  if (differing) warnings.push(`${label} have ${differing} threeWay resource mismatches`);
}

function pushDoodadWarnings(warnings, records, mapDimensions, symmetry) {
  if (!Array.isArray(records)) return;
  const dimensions = {
    width: mapDimensions.width * TILE_SIZE_PX,
    height: mapDimensions.height * TILE_SIZE_PX,
  };
  const valid = records.filter((record) => boundedRecord(record, dimensions));
  if (symmetry === MAP_AUTHORING_SYMMETRY.THREE_WAY) {
    const covered = generatedDoodadCoverage(valid, dimensions);
    const missing = valid.filter((record) => !covered.has(doodadKey(record))).length;
    if (missing) warnings.push(`doodads have ${missing} missing ${symmetry} partners`);
    return;
  }
  const keys = new Set(valid.map(doodadKey));
  let missing = 0;
  for (const source of valid) {
    for (const transform of symmetryTransforms(dimensions, symmetry).slice(1)) {
      let target = transformPoint(source, dimensions, transform);
      if (!target) continue;
      if (source.typeId === "unit.tank_trap") target = snapToTileCentre(target);
      const expected = { ...source, ...target };
      const matched = keys.has(doodadKey(expected));
      if (!matched) missing += 1;
    }
  }
  if (missing) warnings.push(`doodads have ${missing} missing ${symmetry} partners`);
}

function generatedDoodadCoverage(records, dimensions) {
  const keys = new Set(records.map(doodadKey));
  const covered = new Set();
  for (const seed of records) {
    const orbit = expandSymmetricPoints(dimensions, [seed], MAP_AUTHORING_SYMMETRY.THREE_WAY, {
      decorate: (point) => {
        const transformed = seed.typeId === "unit.tank_trap" ? snapToTileCentre(point) : point;
        return { ...seed, ...transformed };
      },
    });
    if (orbit.every((expected) => keys.has(doodadKey(expected)))) {
      for (const expected of orbit) covered.add(doodadKey(expected));
    }
  }
  return covered;
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
