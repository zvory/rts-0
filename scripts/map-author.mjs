#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const TERRAIN = Object.freeze({
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

const PASSABLE = new Set([".", "=", "-", "|", "\\", "/", ..."0123456789"]);
const KNOWN_TERRAIN = new Set([...Object.values(TERRAIN), ..."0123456789"]);
const DEFAULT_STEEL_PATCHES = 12;
const DEFAULT_OIL_PATCHES = 3;
const CURRENT_MAP_VERSION = 6;
const MAX_MAP_DIMENSION_TILES = 256;
const MAX_STEEL_PATCHES_PER_BASE = 36;
const MAX_OIL_PATCHES_PER_BASE = 9;
const MAX_MAP_DOODADS = 4_096;
const MAP_TILE_SIZE_PX = 32;
const DOODAD_TYPES = new Set([
  "tree.oak",
  "tree.pine",
  "tree.spruce",
  "tree.alder",
  "wildflower.single",
  "wildflower.cluster",
  "unit.tank_trap",
]);
const MAP_FIELDS = new Set([
  "version", "name", "description", "width", "height", "terrain", "startLocations",
  "baseSites", "_design", "doodads", "stealthTiles", "noVehicleTiles",
]);
const START_FIELDS = new Set(["x", "y"]);
const BASE_FIELDS = new Set(["x", "y", "steelPatches", "oilPatches"]);
const DOODAD_FIELDS = new Set(["id", "typeId", "x", "y", "color"]);
const SVG_COLORS = Object.freeze({
  ".": "#668b4b",
  "#": "#77756f",
  "~": "#326b80",
  "=": "#aa9368",
  "-": "#aa9368",
  "|": "#aa9368",
  "\\": "#aa9368",
  "/": "#aa9368",
});

function number(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function integer(value, fallback = 0) {
  return Math.trunc(number(value, fallback));
}

function point(value, label = "point") {
  if (!Array.isArray(value) || value.length !== 2 || !value.every(Number.isFinite)) {
    throw new Error(`${label} must be [x, y]`);
  }
  return { x: number(value[0]), y: number(value[1]) };
}

function terrainCharacter(material) {
  const key = String(material || "").toLowerCase();
  const character = TERRAIN[key] || (key.length === 1 && KNOWN_TERRAIN.has(key) ? key : null);
  if (!character) throw new Error(`Unknown terrain material ${JSON.stringify(material)}`);
  return character;
}

function locationKey(record) {
  return `${record?.x},${record?.y}`;
}

function isUint32(value) {
  return Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
}

function inBounds(width, height, x, y) {
  return x >= 0 && y >= 0 && x < width && y < height;
}

function halfTurn(width, height, tile) {
  return { ...tile, x: width - 1 - tile.x, y: height - 1 - tile.y };
}

function expandSymmetry(width, height, records, symmetry) {
  if (symmetry === "none" || symmetry == null) return records;
  if (symmetry !== "halfTurn") throw new Error(`Unsupported symmetry ${JSON.stringify(symmetry)}`);
  const expanded = [];
  const seen = new Set();
  for (const record of records) {
    for (const candidate of [record, halfTurn(width, height, record)]) {
      const key = locationKey(candidate);
      if (seen.has(key)) continue;
      seen.add(key);
      expanded.push(candidate);
    }
  }
  return expanded;
}

function hashUnit(x, y, seed) {
  let value = Math.imul(x | 0, 0x1f123bb5) ^ Math.imul(y | 0, 0x5f356495) ^ Math.imul(seed | 0, 0x6c8e9cf5);
  value = Math.imul(value ^ (value >>> 15), 0x2c1b3c6d);
  value = Math.imul(value ^ (value >>> 12), 0x297a2d39);
  return ((value ^ (value >>> 15)) >>> 0) / 0xffffffff;
}

function smoothstep(value) {
  return value * value * (3 - 2 * value);
}

function valueNoise(x, y, seed, scale) {
  const scaledX = x / scale;
  const scaledY = y / scale;
  const x0 = Math.floor(scaledX);
  const y0 = Math.floor(scaledY);
  const tx = smoothstep(scaledX - x0);
  const ty = smoothstep(scaledY - y0);
  const a = hashUnit(x0, y0, seed);
  const b = hashUnit(x0 + 1, y0, seed);
  const c = hashUnit(x0, y0 + 1, seed);
  const d = hashUnit(x0 + 1, y0 + 1, seed);
  const top = a + (b - a) * tx;
  const bottom = c + (d - c) * tx;
  return (top + (bottom - top) * ty) * 2 - 1;
}

function organicNoise(x, y, seed) {
  return valueNoise(x, y, seed, 11) * 0.65 + valueNoise(x, y, seed + 7919, 4) * 0.35;
}

function distanceToSegment(px, py, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0
    ? 0
    : Math.max(0, Math.min(1, ((px - start.x) * dx + (py - start.y) * dy) / lengthSquared));
  const x = start.x + t * dx;
  const y = start.y + t * dy;
  return Math.hypot(px - x, py - y);
}

function distanceToPath(x, y, points) {
  if (points.length === 1) return Math.hypot(x - points[0].x, y - points[0].y);
  let distance = Infinity;
  let segmentIndex = 0;
  for (let index = 1; index < points.length; index += 1) {
    const candidate = distanceToSegment(x, y, points[index - 1], points[index]);
    if (candidate < distance) {
      distance = candidate;
      segmentIndex = index - 1;
    }
  }
  return { distance, segmentIndex };
}

function pathTiles(width, height, operation) {
  const points = (operation.points || []).map((value, index) => point(value, `points[${index}]`));
  if (!points.length) throw new Error("stroke and road operations need at least one point");
  const brushWidth = Math.max(0.5, number(operation.width, 1));
  const roughness = Math.max(0, number(operation.roughness, 0));
  const radius = brushWidth / 2;
  const seed = integer(operation.seed, 1);
  const padding = Math.ceil(radius + roughness + 2);
  const minX = Math.max(0, Math.floor(Math.min(...points.map(({ x }) => x)) - padding));
  const maxX = Math.min(width - 1, Math.ceil(Math.max(...points.map(({ x }) => x)) + padding));
  const minY = Math.max(0, Math.floor(Math.min(...points.map(({ y }) => y)) - padding));
  const maxY = Math.min(height - 1, Math.ceil(Math.max(...points.map(({ y }) => y)) + padding));
  const tiles = [];
  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      const result = distanceToPath(x, y, points);
      const distance = typeof result === "number" ? result : result.distance;
      const edge = radius + organicNoise(x, y, seed) * roughness;
      if (distance <= edge) tiles.push({ x, y, segmentIndex: result.segmentIndex || 0, distance });
    }
  }
  return { points, tiles, radius };
}

function blobTiles(width, height, operation) {
  const center = point(operation.center, "center");
  const radiusValue = Array.isArray(operation.radius) ? operation.radius : [operation.radius, operation.radius];
  const radiusX = Math.max(0.5, number(radiusValue[0], 1));
  const radiusY = Math.max(0.5, number(radiusValue[1], 1));
  const roughness = Math.max(0, Math.min(1, number(operation.roughness, 0.25)));
  const seed = integer(operation.seed, 1);
  const padding = Math.ceil(Math.max(radiusX, radiusY) * roughness * 0.35 + 2);
  const tiles = [];
  for (let y = Math.max(0, Math.floor(center.y - radiusY - padding)); y <= Math.min(height - 1, Math.ceil(center.y + radiusY + padding)); y += 1) {
    for (let x = Math.max(0, Math.floor(center.x - radiusX - padding)); x <= Math.min(width - 1, Math.ceil(center.x + radiusX + padding)); x += 1) {
      const normalizedDistance = Math.hypot((x - center.x) / radiusX, (y - center.y) / radiusY);
      const boundary = 1 + organicNoise(x, y, seed) * roughness * 0.3;
      if (normalizedDistance <= boundary) tiles.push({ x, y });
    }
  }
  return tiles;
}

function rectTiles(width, height, operation) {
  const from = point(operation.from, "from");
  const to = point(operation.to, "to");
  const tiles = [];
  for (let y = Math.max(0, Math.floor(Math.min(from.y, to.y))); y <= Math.min(height - 1, Math.ceil(Math.max(from.y, to.y))); y += 1) {
    for (let x = Math.max(0, Math.floor(Math.min(from.x, to.x))); x <= Math.min(width - 1, Math.ceil(Math.max(from.x, to.x))); x += 1) tiles.push({ x, y });
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

function paint(grid, tiles, character) {
  for (const tile of tiles) {
    if (grid[tile.y]?.[tile.x] !== undefined) grid[tile.y][tile.x] = character;
  }
}

function addLocation(collection, record) {
  if (!collection.some((candidate) => candidate.x === record.x && candidate.y === record.y)) collection.push(record);
}

function applyOperation(map, grid, operation, defaultSymmetry) {
  const type = String(operation.type || "");
  const symmetry = operation.symmetry ?? defaultSymmetry;
  if (type === "fill") {
    const character = terrainCharacter(operation.material);
    for (const row of grid) row.fill(character);
    return;
  }
  if (type === "rect" || type === "blob" || type === "stroke") {
    const character = terrainCharacter(operation.material);
    const source = type === "rect"
      ? rectTiles(map.width, map.height, operation)
      : type === "blob"
        ? blobTiles(map.width, map.height, operation)
        : pathTiles(map.width, map.height, operation).tiles;
    paint(grid, expandSymmetry(map.width, map.height, source, symmetry), character);
    return;
  }
  if (type === "road") {
    const { points, tiles, radius } = pathTiles(map.width, map.height, { ...operation, roughness: operation.roughness ?? 0 });
    const decorated = tiles.map((tile) => {
      const start = points[tile.segmentIndex] || points[0];
      const end = points[tile.segmentIndex + 1] || start;
      return { ...tile, character: tile.distance <= Math.max(0.35, radius * 0.16) ? roadCenterCharacter(start, end) : "=" };
    });
    const expanded = expandSymmetry(map.width, map.height, decorated, symmetry);
    for (const tile of expanded) paint(grid, [tile], tile.character);
    return;
  }
  if (type === "base" || type === "start") {
    const at = point(operation.at, "at");
    const source = [{ x: integer(at.x), y: integer(at.y) }];
    const locations = expandSymmetry(map.width, map.height, source, symmetry);
    for (const location of locations) {
      const site = {
        ...location,
        steelPatches: integer(operation.steelPatches, DEFAULT_STEEL_PATCHES),
        oilPatches: integer(operation.oilPatches, DEFAULT_OIL_PATCHES),
      };
      addLocation(map.baseSites, site);
      if (type === "start" || operation.start === true) addLocation(map.startLocations, { ...location });
    }
    return;
  }
  throw new Error(`Unknown operation type ${JSON.stringify(type)}`);
}

export function buildMapFromRecipe(recipe) {
  if (!recipe || typeof recipe !== "object") throw new Error("Recipe must be a JSON object");
  const width = number(recipe.width);
  const height = number(recipe.height);
  if (!isUint32(width) || !isUint32(height) || width <= 0 || height <= 0) {
    throw new Error("Recipe width and height must be positive integers");
  }
  if (width > MAX_MAP_DIMENSION_TILES || height > MAX_MAP_DIMENSION_TILES) {
    throw new Error(`Recipe width and height must each be at most ${MAX_MAP_DIMENSION_TILES} tiles`);
  }
  const background = terrainCharacter(recipe.background || "grass");
  const grid = Array.from({ length: height }, () => Array(width).fill(background));
  const map = {
    version: CURRENT_MAP_VERSION,
    name: String(recipe.name || "Untitled map"),
    description: String(recipe.description || ""),
    width,
    height,
    terrain: [],
    startLocations: [],
    baseSites: [],
    _design: String(recipe.design || `Generated by scripts/map-author.mjs with ${recipe.symmetry || "no"} symmetry.`),
    doodads: [],
    stealthTiles: [],
    noVehicleTiles: [],
  };
  for (const operation of recipe.operations || []) applyOperation(map, grid, operation, recipe.symmetry || "none");
  map.terrain = grid.map((row) => row.join(""));
  return map;
}

function connectedRegions(map) {
  const width = integer(map.width);
  const height = integer(map.height);
  if (width <= 0 || height <= 0 || width > MAX_MAP_DIMENSION_TILES || height > MAX_MAP_DIMENSION_TILES) return [];
  if (!Array.isArray(map.terrain) || map.terrain.length !== height) return [];
  const visited = new Uint8Array(Math.max(0, width * height));
  const regions = [];
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const startIndex = y * width + x;
      if (visited[startIndex] || !PASSABLE.has(map.terrain[y]?.[x])) continue;
      visited[startIndex] = 1;
      const queue = [startIndex];
      let cursor = 0;
      let count = 0;
      while (cursor < queue.length) {
        const index = queue[cursor++];
        count += 1;
        const tx = index % width;
        const ty = Math.floor(index / width);
        for (const [nx, ny] of [[tx - 1, ty], [tx + 1, ty], [tx, ty - 1], [tx, ty + 1]]) {
          if (!inBounds(width, height, nx, ny)) continue;
          const neighborIndex = ny * width + nx;
          if (visited[neighborIndex] || !PASSABLE.has(map.terrain[ny]?.[nx])) continue;
          visited[neighborIndex] = 1;
          queue.push(neighborIndex);
        }
      }
      regions.push(count);
    }
  }
  return regions.sort((left, right) => right - left);
}

function terrainCounts(map) {
  const counts = new Map();
  const rows = Array.isArray(map?.terrain) ? map.terrain : [];
  for (const row of rows) {
    if (typeof row !== "string") continue;
    for (const character of row) counts.set(character, (counts.get(character) || 0) + 1);
  }
  return Object.fromEntries([...counts.entries()].sort(([left], [right]) => left.localeCompare(right)));
}

function blockedClearance(map, site, radius) {
  for (let y = site.y - radius; y <= site.y + radius; y += 1) {
    for (let x = site.x - radius; x <= site.x + radius; x += 1) {
      if (!inBounds(map.width, map.height, x, y)) return { x, y, reason: "map edge" };
      if (!PASSABLE.has(map.terrain[y]?.[x])) return { x, y, reason: "impassable terrain" };
    }
  }
  return null;
}

export function validateMap(map, { symmetry = "none" } = {}) {
  const warnings = [];
  const width = integer(map?.width);
  const height = integer(map?.height);
  if (!map || typeof map !== "object" || Array.isArray(map)) {
    return {
      warnings: ["map must be a JSON object"],
      summary: { width: 0, height: 0, starts: 0, bases: 0, regions: 0, largestRegionTiles: 0, passablePercent: 0, terrain: {} },
    };
  }
  if (map?.version !== CURRENT_MAP_VERSION) {
    warnings.push(`schema version is ${JSON.stringify(map?.version)}; the current server expects ${CURRENT_MAP_VERSION}`);
  }
  if (!isUint32(map.width) || !isUint32(map.height) || width <= 0 || height <= 0) {
    warnings.push("width and height must be positive unsigned integers");
  } else if (width > MAX_MAP_DIMENSION_TILES || height > MAX_MAP_DIMENSION_TILES) {
    warnings.push(`width and height must each be at most ${MAX_MAP_DIMENSION_TILES} tiles`);
  }
  for (const [field, value] of [["name", map.name], ["description", map.description], ["_design", map._design]]) {
    if (typeof value !== "string") warnings.push(`${field} must be a string`);
  }
  for (const field of Object.keys(map)) {
    if (!MAP_FIELDS.has(field)) warnings.push(`map has unsupported field ${JSON.stringify(field)}`);
  }
  if (!Array.isArray(map?.terrain) || map.terrain.length !== height) warnings.push(`terrain has ${map?.terrain?.length ?? 0} rows; height is ${height}`);
  const terrainRows = Array.isArray(map.terrain) ? map.terrain : [];
  for (let y = 0; y < terrainRows.length; y += 1) {
    const row = terrainRows[y];
    if (typeof row !== "string" || [...row].length !== width) warnings.push(`terrain row ${y} does not contain ${width} tiles`);
    for (const character of typeof row === "string" ? row : "") {
      if (!KNOWN_TERRAIN.has(character)) warnings.push(`terrain row ${y} contains unknown character ${JSON.stringify(character)}`);
    }
  }
  const starts = Array.isArray(map.startLocations) ? map.startLocations : [];
  const bases = Array.isArray(map.baseSites) ? map.baseSites : [];
  if (!Array.isArray(map.startLocations)) warnings.push("startLocations must be an array");
  if (!Array.isArray(map.baseSites)) warnings.push("baseSites must be an array");
  if (starts.length < 1 || starts.length > 4) warnings.push(`map has ${starts.length} start locations; the current server accepts 1 to 4`);
  if (bases.length < 1 || bases.length > 32) warnings.push(`map has ${bases.length} base sites; the current server accepts 1 to 32`);
  const validStarts = starts.filter((location) => location && isUint32(location.x) && isUint32(location.y));
  const validBases = bases.filter((location) => location && isUint32(location.x) && isUint32(location.y));
  const baseKeys = new Set(validBases.map(locationKey));
  for (const start of validStarts) if (!baseKeys.has(locationKey(start))) warnings.push(`start (${start.x},${start.y}) is not also a base site`);
  for (const [kind, locations] of [["start", starts], ["base", bases]]) {
    const seen = new Set();
    for (const [index, location] of locations.entries()) {
      if (!location || typeof location !== "object" || !isUint32(location.x) || !isUint32(location.y)) {
        warnings.push(`${kind} location ${index} must have unsigned integer x and y`);
        continue;
      }
      const key = locationKey(location);
      if (seen.has(key)) warnings.push(`${kind} location (${key}) is duplicated`);
      seen.add(key);
      if (!inBounds(width, height, location.x, location.y)) warnings.push(`${kind} location (${key}) is outside the map`);
      const allowedFields = kind === "base" ? BASE_FIELDS : START_FIELDS;
      for (const field of Object.keys(location)) {
        if (!allowedFields.has(field)) warnings.push(`${kind} location ${index} has unsupported field ${JSON.stringify(field)}`);
      }
    }
  }
  for (const [index, site] of bases.entries()) {
    if (!site || typeof site !== "object") continue;
    for (const [field, maximum] of [["steelPatches", MAX_STEEL_PATCHES_PER_BASE], ["oilPatches", MAX_OIL_PATCHES_PER_BASE]]) {
      if (!isUint32(site[field]) || site[field] > maximum) {
        warnings.push(`base site ${index} ${field} must be an unsigned integer from 0 to ${maximum}`);
      }
    }
  }
  const startKeys = new Set(validStarts.map(locationKey));
  const terrainIsRectangular = terrainRows.length === height && terrainRows.every((row) => typeof row === "string" && [...row].length === width);
  for (const site of validBases) {
    if (!terrainIsRectangular || !inBounds(width, height, site.x, site.y)) continue;
    const blocked = blockedClearance(map, site, startKeys.has(locationKey(site)) ? 7 : 4);
    if (blocked) warnings.push(`base (${site.x},${site.y}) has ${blocked.reason} in its protected area at (${blocked.x},${blocked.y})`);
  }
  for (const field of ["stealthTiles", "noVehicleTiles"]) {
    const locations = map[field] === undefined ? [] : map[field];
    if (!Array.isArray(locations)) {
      warnings.push(`${field} must be an array`);
      continue;
    }
    const seen = new Set();
    for (const [index, location] of locations.entries()) {
      if (!location || typeof location !== "object" || !isUint32(location.x) || !isUint32(location.y)) {
        warnings.push(`${field}[${index}] must have unsigned integer x and y`);
        continue;
      }
      const key = locationKey(location);
      if (seen.has(key)) warnings.push(`${field}[${index}] duplicates (${key})`);
      seen.add(key);
      if (!inBounds(width, height, location.x, location.y)) warnings.push(`${field}[${index}] is outside the map at (${key})`);
      for (const locationField of Object.keys(location)) {
        if (!START_FIELDS.has(locationField)) warnings.push(`${field}[${index}] has unsupported field ${JSON.stringify(locationField)}`);
      }
    }
  }
  const doodads = map.doodads === undefined ? [] : map.doodads;
  if (!Array.isArray(doodads)) {
    warnings.push("doodads must be an array");
  } else {
    if (doodads.length > MAX_MAP_DOODADS) warnings.push(`doodads must contain at most ${MAX_MAP_DOODADS} entries`);
    const ids = new Set();
    for (const [index, doodad] of doodads.entries()) {
      if (!doodad || typeof doodad !== "object") {
        warnings.push(`doodads[${index}] must be an object`);
        continue;
      }
      for (const field of Object.keys(doodad)) {
        if (!DOODAD_FIELDS.has(field)) warnings.push(`doodads[${index}] has unsupported field ${JSON.stringify(field)}`);
      }
      if (!isUint32(doodad.id) || doodad.id === 0 || ids.has(doodad.id)) warnings.push(`doodads[${index}].id must be a unique nonzero unsigned integer`);
      ids.add(doodad.id);
      if (!DOODAD_TYPES.has(doodad.typeId)) warnings.push(`doodads[${index}].typeId is not in the server catalog`);
      if (!isUint32(doodad.x) || !isUint32(doodad.y)) {
        warnings.push(`doodads[${index}] must have unsigned integer x and y`);
      } else if (doodad.x >= width * MAP_TILE_SIZE_PX || doodad.y >= height * MAP_TILE_SIZE_PX) {
        warnings.push(`doodads[${index}] is outside the map at (${doodad.x},${doodad.y})`);
      }
      if (doodad.typeId === "unit.tank_trap" && (doodad.x % MAP_TILE_SIZE_PX !== MAP_TILE_SIZE_PX / 2 || doodad.y % MAP_TILE_SIZE_PX !== MAP_TILE_SIZE_PX / 2)) {
        warnings.push(`doodads[${index}] tank trap must be centered on a map tile`);
      }
      if (doodad.color != null) {
        const isFlower = doodad.typeId === "wildflower.single" || doodad.typeId === "wildflower.cluster";
        if (!isFlower) warnings.push(`doodads[${index}].color is only allowed for wildflowers`);
        else if (typeof doodad.color !== "string" || !/^#[0-9a-f]{6}$/.test(doodad.color)) warnings.push(`doodads[${index}].color must use canonical lowercase #rrggbb`);
      }
    }
  }
  const regions = connectedRegions(map);
  if (regions.length > 1) warnings.push(`passable terrain has ${regions.length} disconnected regions (${regions.slice(0, 5).join(", ")} tiles${regions.length > 5 ? ", …" : ""})`);
  if (symmetry === "halfTurn") {
    if (width > 0 && height > 0 && width <= MAX_MAP_DIMENSION_TILES && height <= MAX_MAP_DIMENSION_TILES && terrainIsRectangular) {
      let terrainMismatches = 0;
      for (let y = 0; y < height; y += 1) {
        for (let x = 0; x < width; x += 1) {
          if (map.terrain[y]?.[x] !== map.terrain[height - 1 - y]?.[width - 1 - x]) terrainMismatches += 1;
        }
      }
      if (terrainMismatches) warnings.push(`terrain has ${Math.ceil(terrainMismatches / 2)} half-turn symmetry mismatches`);
      for (const [kind, locations] of [["start", validStarts], ["base", validBases]]) {
        const keys = new Set(locations.map(locationKey));
        const missing = locations.filter((location) => !keys.has(`${width - 1 - location.x},${height - 1 - location.y}`));
        if (missing.length) warnings.push(`${kind} locations have ${missing.length} missing half-turn partners`);
      }
    }
  } else if (symmetry !== "none") {
    warnings.push(`symmetry check ${JSON.stringify(symmetry)} is not implemented`);
  }
  const area = Math.max(1, width * height);
  const passableTiles = Object.entries(terrainCounts(map)).reduce((sum, [character, count]) => sum + (PASSABLE.has(character) ? count : 0), 0);
  return {
    warnings,
    summary: {
      width,
      height,
      starts: starts.length,
      bases: bases.length,
      regions: regions.length,
      largestRegionTiles: regions[0] || 0,
      passablePercent: Math.round(passableTiles / area * 1000) / 10,
      terrain: terrainCounts(map),
    },
  };
}

function xml(value) {
  return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

export function renderPreviewSvg(map, { tilePixels = 5 } = {}) {
  if (!isUint32(map?.width) || !isUint32(map?.height) || map.width < 1 || map.height < 1
    || map.width > MAX_MAP_DIMENSION_TILES || map.height > MAX_MAP_DIMENSION_TILES) {
    throw new Error(`Preview width and height must be unsigned integers from 1 to ${MAX_MAP_DIMENSION_TILES}`);
  }
  const scale = Math.max(1, integer(tilePixels, 5));
  const width = integer(map.width);
  const height = integer(map.height);
  const pixelWidth = width * scale;
  const pixelHeight = height * scale;
  const elements = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${pixelWidth}" height="${pixelHeight}" viewBox="0 0 ${width} ${height}">`,
    `<title>${xml(map.name || "Map preview")}</title>`,
    `<rect width="${width}" height="${height}" fill="${SVG_COLORS["."]}"/>`,
  ];
  for (let y = 0; y < height; y += 1) {
    const row = map.terrain?.[y] || "";
    let startX = 0;
    while (startX < width) {
      const character = row[startX];
      let endX = startX + 1;
      while (endX < width && row[endX] === character) endX += 1;
      if (character !== ".") {
        elements.push(`<rect x="${startX}" y="${y}" width="${endX - startX}" height="1" fill="${SVG_COLORS[character] || "#ff00ff"}"/>`);
      }
      startX = endX;
    }
  }
  const validStarts = Array.isArray(map.startLocations)
    ? map.startLocations.filter((site) => Number.isFinite(site?.x) && Number.isFinite(site?.y))
    : [];
  const starts = new Set(validStarts.map(locationKey));
  const baseSites = Array.isArray(map.baseSites)
    ? map.baseSites.filter((site) => Number.isFinite(site?.x) && Number.isFinite(site?.y))
    : [];
  for (const [index, site] of baseSites.entries()) {
    const isStart = starts.has(locationKey(site));
    elements.push(`<circle cx="${site.x + 0.5}" cy="${site.y + 0.5}" r="${isStart ? 4.2 : 3.2}" fill="${isStart ? "#f36f38" : "#f2d057"}" stroke="#161b20" stroke-width="0.8"/>`);
    elements.push(`<text x="${site.x + 0.5}" y="${site.y + 1.7}" text-anchor="middle" font-family="ui-monospace, monospace" font-size="3.4" font-weight="700" fill="#161b20">${index + 1}</text>`);
  }
  elements.push("</svg>");
  return `${elements.join("\n")}\n`;
}

function parseOptions(args) {
  const positional = [];
  const options = {};
  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (!value.startsWith("--")) {
      positional.push(value);
      continue;
    }
    const key = value.slice(2);
    const next = args[index + 1];
    if (next == null || next.startsWith("--")) options[key] = true;
    else {
      options[key] = next;
      index += 1;
    }
  }
  return { positional, options };
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(path.resolve(file), "utf8"));
}

function writeFile(file, contents) {
  const resolved = path.resolve(file);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  fs.writeFileSync(resolved, contents);
  return resolved;
}

function printValidation(result) {
  process.stdout.write(`${JSON.stringify(result.summary, null, 2)}\n`);
  if (!result.warnings.length) {
    process.stdout.write("No advisory warnings.\n");
    return;
  }
  process.stdout.write(`Advisory warnings (${result.warnings.length}; map was not rejected):\n`);
  for (const warning of result.warnings) process.stdout.write(`- ${warning}\n`);
}

function usage() {
  return `Usage:
  node scripts/map-author.mjs build <recipe.json> --output <map.json>
  node scripts/map-author.mjs validate <map.json> [--symmetry halfTurn]
  node scripts/map-author.mjs preview <map.json> --output <preview.svg> [--tile-pixels 5]

Recipe operations:
  {"type":"blob","material":"water","center":[40,80],"radius":[18,10],"roughness":0.35,"seed":1}
  {"type":"stroke","material":"stone","points":[[40,40],[55,47]],"width":9,"roughness":2,"seed":2}
  {"type":"road","points":[[10,20],[80,20]],"width":5}
  {"type":"base","at":[30,30]}
  {"type":"start","at":[20,20]}

Supported symmetry is "none" or "halfTurn". Validation is advisory and never rejects a readable map.
`;
}

export function runCli(argv = process.argv.slice(2)) {
  const [command, ...rest] = argv;
  const { positional, options } = parseOptions(rest);
  if (!command || command === "help" || options.help) {
    process.stdout.write(usage());
    return 0;
  }
  if (command === "build") {
    if (!positional[0] || !options.output) throw new Error("build needs <recipe.json> and --output <map.json>");
    const recipe = readJson(positional[0]);
    const map = buildMapFromRecipe(recipe);
    const output = writeFile(options.output, `${JSON.stringify(map, null, 2)}\n`);
    process.stdout.write(`Wrote ${output}\n`);
    printValidation(validateMap(map, { symmetry: recipe.symmetry || "none" }));
    return 0;
  }
  if (command === "validate") {
    if (!positional[0]) throw new Error("validate needs <map.json>");
    printValidation(validateMap(readJson(positional[0]), { symmetry: options.symmetry || "none" }));
    return 0;
  }
  if (command === "preview") {
    if (!positional[0] || !options.output) throw new Error("preview needs <map.json> and --output <preview.svg>");
    const map = readJson(positional[0]);
    const output = writeFile(options.output, renderPreviewSvg(map, { tilePixels: options["tile-pixels"] }));
    process.stdout.write(`Wrote ${output}\n`);
    return 0;
  }
  throw new Error(`Unknown command ${JSON.stringify(command)}\n\n${usage()}`);
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  try {
    process.exitCode = runCli();
  } catch (error) {
    process.stderr.write(`map-author: ${error.message}\n`);
    process.exitCode = 1;
  }
}
