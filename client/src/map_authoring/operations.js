import { blobTiles, pathTiles, rectTiles, tuplePoint } from "./geometry.js";
import {
  AUTHORED_MAP_MAX_OIL_PATCHES,
  AUTHORED_MAP_MAX_STEEL_PATCHES,
  boundedAuthoredPatchCount,
} from "./limits.js";
import { expandSymmetricPoints, MAP_AUTHORING_SYMMETRY, normalizeDimensions } from "./symmetry.js";

export const AUTHORING_TERRAIN = Object.freeze({
  grass: ".",
  stone: "#",
  rock: "#",
  water: "~",
  road: "=",
  "road-bare": "=",
  "road-horizontal": "-",
  "road-vertical": "|",
  "road-diagonal-nw-se": "\\",
  "road-diagonal-ne-sw": "/",
});

export const AUTHORING_TERRAIN_CHARACTERS = new Set([...Object.values(AUTHORING_TERRAIN), ..."0123456789"]);
export const AUTHORING_PASSABLE_CHARACTERS = new Set([".", "=", "-", "|", "\\", "/", ..."0123456789"]);

const ROAD_ANGLES = new Map([["-", 0], ["\\", 45], ["|", 90], ["/", 135]]);
export const AUTHORING_ROAD_CHARACTERS = new Set(["=", ...ROAD_ANGLES.keys()]);

export function terrainCharacter(material) {
  const key = String(material || "").toLowerCase();
  const character = AUTHORING_TERRAIN[key]
    || (key.length === 1 && AUTHORING_TERRAIN_CHARACTERS.has(key) ? key : null);
  if (!character) throw new Error(`Unknown terrain material ${JSON.stringify(material)}`);
  return character;
}

/** Apply one serializable authoring operation to a map draft in place. */
export function applyMapOperation(draft, operation, {
  defaultSymmetry = MAP_AUTHORING_SYMMETRY.NONE,
  protectedTerrain = () => false,
  defaultSteelPatches = 12,
  defaultOilPatches = 3,
} = {}) {
  const dimensions = normalizeDimensions(draft);
  if (!dimensions || !Array.isArray(draft?.terrain)) throw new Error("Map draft needs valid dimensions and terrain rows");
  const type = String(operation?.type || "");
  const symmetry = operation.symmetry ?? defaultSymmetry;
  if (type === "fill") {
    const character = terrainCharacter(operation.material ?? operation.character);
    return paintTerrain(draft, allTiles(dimensions, character), () => false);
  }
  if (["rect", "blob", "stroke"].includes(type)) {
    const character = terrainCharacter(operation.material ?? operation.character);
    const source = type === "rect"
      ? rectTiles(dimensions, tuplePoint(operation.from, "from"), tuplePoint(operation.to, "to"))
      : type === "blob"
        ? blobTiles(dimensions, operation)
        : pathTiles(dimensions, operation).tiles;
    return paintTerrain(draft, expandTerrain(dimensions, source, character, symmetry), protectedTerrain);
  }
  if (type === "road") {
    const { points, tiles, radius } = pathTiles(dimensions, { ...operation, roughness: operation.roughness ?? 0 });
    const decorated = tiles.map((tile) => {
      const start = points[tile.segmentIndex] || points[0];
      const end = points[tile.segmentIndex + 1] || start;
      return { ...tile, character: tile.distance <= Math.max(0.35, radius * 0.16) ? roadCenterCharacter(start, end) : "=" };
    });
    return paintTerrain(draft, expandSymmetricPoints(dimensions, decorated, symmetry, {
      decorate: (tile, transform) => ({ ...tile, character: transformRoadCharacter(tile.character, transform) }),
    }), protectedTerrain);
  }
  if (type === "paintTiles") {
    const character = operation.character == null ? null : terrainCharacter(operation.character);
    const source = Array.isArray(operation.tiles) ? operation.tiles : [];
    const tiles = operation.expandSymmetry === false
      ? source
      : expandSymmetricPoints(dimensions, source, symmetry, {
        decorate: (tile, transform) => ({
          ...tile,
          character: transformRoadCharacter(terrainCharacter(tile.character ?? character), transform),
        }),
      });
    return paintTerrain(draft, tiles.map((tile) => ({ ...tile, character: terrainCharacter(tile.character ?? character) })), protectedTerrain);
  }
  if (type === "overlayTiles") {
    const source = Array.isArray(operation.tiles) ? operation.tiles : [];
    const tiles = expandSymmetricPoints(
      dimensions,
      source,
      operation.expandSymmetry === false ? MAP_AUTHORING_SYMMETRY.NONE : symmetry,
    );
    return paintOverlays(draft, tiles, operation.edit || {});
  }
  if (type === "base" || type === "start") {
    const at = tuplePoint(operation.at, "at");
    const locations = expandSymmetricPoints(dimensions, [{ x: Math.trunc(at.x), y: Math.trunc(at.y) }], symmetry);
    for (const location of locations) {
      addLocation(draft.baseSites, {
        x: location.x,
        y: location.y,
        steelPatches: boundedAuthoredPatchCount(
          operation.steelPatches,
          AUTHORED_MAP_MAX_STEEL_PATCHES,
          defaultSteelPatches,
        ),
        oilPatches: boundedAuthoredPatchCount(
          operation.oilPatches,
          AUTHORED_MAP_MAX_OIL_PATCHES,
          defaultOilPatches,
        ),
      });
      if (type === "start" || operation.start === true) addLocation(draft.startLocations, { x: location.x, y: location.y });
    }
    return { locationPatch: locations };
  }
  throw new Error(`Unknown operation type ${JSON.stringify(type)}`);
}

export function transformRoadCharacter(character, transform) {
  const angle = ROAD_ANGLES.get(character);
  if (angle === undefined) return character;
  let degrees = 0;
  if (transform === "rotate90") degrees = 90;
  else if (transform === "rotate120") degrees = 120;
  else if (transform === "rotate180") degrees = 180;
  else if (transform === "rotate240") degrees = 240;
  else if (transform === "rotate270") degrees = 270;
  else if (transform === "horizontal") return character === "\\" ? "/" : character === "/" ? "\\" : character;
  else if (transform === "vertical") return character === "\\" ? "/" : character === "/" ? "\\" : character;
  else if (transform === "diagonalMain" || transform === "diagonalAnti") {
    return character === "-" ? "|" : character === "|" ? "-" : character;
  } else return character;
  const target = (angle + degrees) % 180;
  let closest = character;
  let closestDistance = Infinity;
  for (const [candidate, candidateAngle] of ROAD_ANGLES) {
    const raw = Math.abs(candidateAngle - target);
    const distance = Math.min(raw, 180 - raw);
    if (distance < closestDistance) {
      closest = candidate;
      closestDistance = distance;
    }
  }
  return closest;
}

function expandTerrain(dimensions, source, character, symmetry) {
  return expandSymmetricPoints(dimensions, source, symmetry, {
    decorate: (tile, transform) => ({ ...tile, character: transformRoadCharacter(character, transform) }),
  });
}

function paintTerrain(draft, tiles, protectedTerrain) {
  const dimensions = normalizeDimensions(draft);
  const byRow = new Map();
  const terrainPatch = [];
  const noEntrenchment = new Map((draft.noEntrenchmentTiles || []).map((tile) => [locationKey(tile), tile]));
  for (const candidate of tiles) {
    const point = expandSymmetricPoints(dimensions, [candidate], MAP_AUTHORING_SYMMETRY.NONE)[0];
    const character = terrainCharacter(candidate.character);
    if (!point || (!AUTHORING_PASSABLE_CHARACTERS.has(character) && protectedTerrain(point, draft))) continue;
    const row = byRow.get(point.y) || [...draft.terrain[point.y]];
    const key = locationKey(point);
    const previous = row[point.x];
    const hadNoEntrenchment = noEntrenchment.has(key);
    if (AUTHORING_ROAD_CHARACTERS.has(character)) noEntrenchment.set(key, { x: point.x, y: point.y });
    else if (AUTHORING_ROAD_CHARACTERS.has(previous)) noEntrenchment.delete(key);
    const overlayChanged = hadNoEntrenchment !== noEntrenchment.has(key);
    if (previous === character && !overlayChanged) continue;
    row[point.x] = character;
    byRow.set(point.y, row);
    terrainPatch.push({ x: point.x, y: point.y, character });
  }
  for (const [y, row] of byRow) draft.terrain[y] = row.join("");
  draft.noEntrenchmentTiles = [...noEntrenchment.values()];
  return { terrainPatch };
}

function paintOverlays(draft, tiles, edit) {
  const concealment = new Map((draft.concealmentTiles || []).map((tile) => [locationKey(tile), tile]));
  const noVehicle = new Map((draft.noVehicleTiles || []).map((tile) => [locationKey(tile), tile]));
  const noBuilding = new Map((draft.noBuildingTiles || []).map((tile) => [locationKey(tile), tile]));
  const noEntrenchment = new Map((draft.noEntrenchmentTiles || []).map((tile) => [locationKey(tile), tile]));
  const damageReduction = new Map((draft.damageReductionTiles || []).map((tile) => [locationKey(tile), tile]));
  const slowMovement = new Map((draft.slowMovementTiles || []).map((tile) => [locationKey(tile), tile]));
  const overlayPatch = [];
  for (const tile of tiles) {
    const key = locationKey(tile);
    const before = `${concealment.has(key)}:${noVehicle.has(key)}:${noBuilding.has(key)}:${noEntrenchment.has(key)}:${damageReduction.has(key)}:${slowMovement.has(key)}`;
    applyOverlayEdit(concealment, key, tile, edit.concealment);
    applyOverlayEdit(noVehicle, key, tile, edit.noVehicle);
    applyOverlayEdit(noBuilding, key, tile, edit.noBuilding);
    applyOverlayEdit(noEntrenchment, key, tile, edit.noEntrenchment);
    if (AUTHORING_ROAD_CHARACTERS.has(draft.terrain[tile.y]?.[tile.x])) {
      noEntrenchment.set(key, { x: tile.x, y: tile.y });
    }
    applyOverlayEdit(damageReduction, key, tile, edit.damageReduction);
    applyOverlayEdit(slowMovement, key, tile, edit.slowMovement);
    if (before !== `${concealment.has(key)}:${noVehicle.has(key)}:${noBuilding.has(key)}:${noEntrenchment.has(key)}:${damageReduction.has(key)}:${slowMovement.has(key)}`) {
      overlayPatch.push({ x: tile.x, y: tile.y });
    }
  }
  draft.concealmentTiles = [...concealment.values()];
  draft.noVehicleTiles = [...noVehicle.values()];
  draft.noBuildingTiles = [...noBuilding.values()];
  draft.noEntrenchmentTiles = [...noEntrenchment.values()];
  draft.damageReductionTiles = [...damageReduction.values()];
  draft.slowMovementTiles = [...slowMovement.values()];
  return { overlayPatch };
}

function applyOverlayEdit(collection, key, tile, value) {
  if (value === true) collection.set(key, { x: tile.x, y: tile.y });
  else if (value === false) collection.delete(key);
}

function allTiles(dimensions, character) {
  const tiles = [];
  for (let y = 0; y < dimensions.height; y += 1) {
    for (let x = 0; x < dimensions.width; x += 1) tiles.push({ x, y, character });
  }
  return tiles;
}

function roadCenterCharacter(start, end) {
  const angle = Math.atan2(end.y - start.y, end.x - start.x) * 180 / Math.PI;
  const normalized = ((angle % 180) + 180) % 180;
  if (normalized < 22.5 || normalized >= 157.5) return "-";
  if (normalized < 67.5) return "\\";
  if (normalized < 112.5) return "|";
  return "/";
}

function addLocation(collection, record) {
  if (!collection.some((candidate) => candidate.x === record.x && candidate.y === record.y)) collection.push(record);
}

function locationKey(record) {
  return `${record?.x},${record?.y}`;
}
