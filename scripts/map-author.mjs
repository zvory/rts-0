#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  AUTHORING_PASSABLE_CHARACTERS as PASSABLE,
  AUTHORING_TERRAIN_CHARACTERS as KNOWN_TERRAIN,
} from "../client/src/map_authoring/operations.js";
import {
  MAP_AUTHORING_LAYER,
  MAP_AUTHORING_LAYER_IDS,
  mapAuthoringDoodadLayer,
  mapAuthoringLayerVisibilityFromSelection,
} from "../client/src/map_authoring/layers.js";
import { forestTilesFromSpans } from "../client/src/map_authoring/forests.js";
import {
  buildMapFromRecipe,
  CURRENT_AUTHORED_MAP_VERSION as CURRENT_MAP_VERSION,
  MAX_AUTHORED_MAP_DIMENSION_TILES as MAX_MAP_DIMENSION_TILES,
} from "../client/src/map_authoring/recipe.js";
import { mapSymmetryWarnings } from "../client/src/map_authoring/symmetry_validation.js";

export { buildMapFromRecipe } from "../client/src/map_authoring/recipe.js";

const SCRIPT_FILE = fileURLToPath(import.meta.url);
const REPO_ROOT = path.resolve(path.dirname(SCRIPT_FILE), "..");
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
  "baseSites", "_design", "doodads", "forestSpans", "concealmentTiles", "noVehicleTiles",
  "noBuildingTiles", "damageReductionTiles", "slowMovementTiles",
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

function locationKey(record) {
  return `${record?.x},${record?.y}`;
}

function isUint32(value) {
  return Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
}

function inBounds(width, height, x, y) {
  return x >= 0 && y >= 0 && x < width && y < height;
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

function validateForestSpans(spans, width, height) {
  if (!Array.isArray(spans)) return ["forestSpans must be an array"];
  const warnings = [];
  const occupied = new Set();
  for (const [index, span] of spans.entries()) {
    if (!Array.isArray(span) || span.length !== 3 || !span.every(isUint32)) {
      warnings.push(`forestSpans[${index}] must be [y, xStart, xEnd] unsigned integers`);
      continue;
    }
    const [y, xStart, xEnd] = span;
    if (y >= height || xStart > xEnd || xEnd >= width) {
      warnings.push(`forestSpans[${index}] is outside the map or has reversed x bounds`);
      continue;
    }
    let overlap = null;
    for (let x = xStart; x <= xEnd; x += 1) {
      const key = `${x},${y}`;
      if (occupied.has(key)) {
        overlap = key;
        break;
      }
    }
    if (overlap) {
      warnings.push(`forestSpans[${index}] overlaps another span at (${overlap})`);
    } else {
      for (let x = xStart; x <= xEnd; x += 1) occupied.add(`${x},${y}`);
    }
  }
  return warnings;
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
  warnings.push(...validateForestSpans(map.forestSpans, width, height));
  for (const field of ["concealmentTiles", "noVehicleTiles", "noBuildingTiles", "damageReductionTiles", "slowMovementTiles"]) {
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
  warnings.push(...mapSymmetryWarnings(map, symmetry));
  const area = Math.max(1, width * height);
  const terrain = terrainCounts(map);
  const passableTiles = Object.entries(terrain).reduce((sum, [character, count]) => sum + (PASSABLE.has(character) ? count : 0), 0);
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
      terrain,
    },
  };
}

function xml(value) {
  return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

export function renderPreviewSvg(map, { tilePixels = 5, layers = "all" } = {}) {
  if (!isUint32(map?.width) || !isUint32(map?.height) || map.width < 1 || map.height < 1
    || map.width > MAX_MAP_DIMENSION_TILES || map.height > MAX_MAP_DIMENSION_TILES) {
    throw new Error(`Preview width and height must be unsigned integers from 1 to ${MAX_MAP_DIMENSION_TILES}`);
  }
  const scale = Math.max(1, integer(tilePixels, 5));
  const width = integer(map.width);
  const height = integer(map.height);
  const pixelWidth = width * scale;
  const pixelHeight = height * scale;
  const visibility = mapAuthoringLayerVisibilityFromSelection(layers);
  const elements = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${pixelWidth}" height="${pixelHeight}" viewBox="0 0 ${width} ${height}">`,
    `<title>${xml(map.name || "Map preview")}</title>`,
  ];
  if (visibility[MAP_AUTHORING_LAYER.BASE]) {
    elements.push(`<g data-layer="${MAP_AUTHORING_LAYER.BASE}">`);
    elements.push(`<rect width="${width}" height="${height}" fill="${SVG_COLORS["."]}"/>`);
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
    elements.push("</g>");
  }
  appendSemanticTileLayer(elements, forestTilesFromSpans(map.forestSpans, { width, height }), {
    id: MAP_AUTHORING_LAYER.FOREST,
    visible: visibility[MAP_AUTHORING_LAYER.FOREST],
    width,
    height,
    fill: "#315f36",
    stroke: "#b3d57a",
  });
  appendSemanticTileLayer(elements, map.concealmentTiles, {
    id: MAP_AUTHORING_LAYER.CONCEALMENT,
    visible: visibility[MAP_AUTHORING_LAYER.CONCEALMENT],
    width,
    height,
    fill: "#2d8c64",
    stroke: "#5ed19a",
  });
  appendSemanticTileLayer(elements, map.noVehicleTiles, {
    id: MAP_AUTHORING_LAYER.NO_VEHICLE,
    visible: visibility[MAP_AUTHORING_LAYER.NO_VEHICLE],
    width,
    height,
    fill: "#d94b45",
    stroke: "#ffaaa5",
  });
  appendSemanticTileLayer(elements, map.noBuildingTiles, {
    id: MAP_AUTHORING_LAYER.NO_BUILDING,
    visible: visibility[MAP_AUTHORING_LAYER.NO_BUILDING],
    width,
    height,
    fill: "#d58a2f",
    stroke: "#ffd293",
  });
  appendSemanticTileLayer(elements, map.damageReductionTiles, {
    id: MAP_AUTHORING_LAYER.DAMAGE_REDUCTION,
    visible: visibility[MAP_AUTHORING_LAYER.DAMAGE_REDUCTION],
    width,
    height,
    fill: "#3e82d7",
    stroke: "#a9ccff",
  });
  appendSemanticTileLayer(elements, map.slowMovementTiles, {
    id: MAP_AUTHORING_LAYER.SLOW_MOVEMENT,
    visible: visibility[MAP_AUTHORING_LAYER.SLOW_MOVEMENT],
    width,
    height,
    fill: "#8b5fc7",
    stroke: "#d9bfff",
  });
  appendDoodadLayers(elements, map.doodads, visibility, width, height);
  const validStarts = Array.isArray(map.startLocations)
    ? map.startLocations.filter((site) => Number.isFinite(site?.x) && Number.isFinite(site?.y))
    : [];
  const starts = new Set(validStarts.map(locationKey));
  const baseSites = Array.isArray(map.baseSites)
    ? map.baseSites.filter((site) => Number.isFinite(site?.x) && Number.isFinite(site?.y))
    : [];
  if (visibility[MAP_AUTHORING_LAYER.BASE]) {
    elements.push(`<g data-layer="${MAP_AUTHORING_LAYER.BASE}" data-part="sites">`);
    for (const [index, site] of baseSites.entries()) {
      const isStart = starts.has(locationKey(site));
      elements.push(`<circle cx="${site.x + 0.5}" cy="${site.y + 0.5}" r="${isStart ? 4.2 : 3.2}" fill="${isStart ? "#f36f38" : "#f2d057"}" stroke="#161b20" stroke-width="0.8"/>`);
      elements.push(`<text x="${site.x + 0.5}" y="${site.y + 1.7}" text-anchor="middle" font-family="ui-monospace, monospace" font-size="3.4" font-weight="700" fill="#161b20">${index + 1}</text>`);
    }
    elements.push("</g>");
  }
  elements.push("</svg>");
  return `${elements.join("\n")}\n`;
}

function appendSemanticTileLayer(elements, records, { id, visible, width, height, fill, stroke }) {
  if (!visible) return;
  elements.push(`<g data-layer="${id}" fill="${fill}" fill-opacity="0.3" stroke="${stroke}" stroke-width="0.08">`);
  for (const tile of Array.isArray(records) ? records : []) {
    if (!Number.isInteger(tile?.x) || !Number.isInteger(tile?.y) || !inBounds(width, height, tile.x, tile.y)) continue;
    elements.push(`<rect x="${tile.x}" y="${tile.y}" width="1" height="1"/>`);
  }
  elements.push("</g>");
}

function appendDoodadLayers(elements, records, visibility, width, height) {
  const grouped = new Map(MAP_AUTHORING_LAYER_IDS.map((id) => [id, []]));
  for (const record of Array.isArray(records) ? records : []) {
    if (!DOODAD_TYPES.has(record?.typeId)
      || !Number.isFinite(record?.x) || !Number.isFinite(record?.y)
      || record.x < 0 || record.y < 0 || record.x >= width * MAP_TILE_SIZE_PX || record.y >= height * MAP_TILE_SIZE_PX) continue;
    grouped.get(mapAuthoringDoodadLayer(record.typeId))?.push(record);
  }
  for (const id of [
    MAP_AUTHORING_LAYER.TREES,
    MAP_AUTHORING_LAYER.GAMEPLAY_DOODADS,
    MAP_AUTHORING_LAYER.DECORATIVE_DOODADS,
  ]) {
    if (!visibility[id]) continue;
    elements.push(`<g data-layer="${id}">`);
    for (const record of grouped.get(id) || []) elements.push(renderDoodadSvg(record, id));
    elements.push("</g>");
  }
}

function renderDoodadSvg(record, layerId) {
  const x = Math.round(record.x / MAP_TILE_SIZE_PX * 1000) / 1000;
  const y = Math.round(record.y / MAP_TILE_SIZE_PX * 1000) / 1000;
  if (layerId === MAP_AUTHORING_LAYER.TREES) {
    return `<circle cx="${x}" cy="${y}" r="1.15" fill="#315f36" stroke="#b3d57a" stroke-width="0.16"/>`;
  }
  if (layerId === MAP_AUTHORING_LAYER.DECORATIVE_DOODADS) {
    const color = /^#[0-9a-f]{6}$/.test(record.color || "") ? record.color : "#e8b84a";
    return `<circle cx="${x}" cy="${y}" r="0.24" fill="${color}" stroke="#fff4d9" stroke-width="0.06"/>`;
  }
  return `<path d="M ${x - 0.45} ${y - 0.45} L ${x + 0.45} ${y + 0.45} M ${x + 0.45} ${y - 0.45} L ${x - 0.45} ${y + 0.45}" stroke="#ded5bd" stroke-width="0.2"/>`;
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
  node scripts/map-author.mjs check <map.json>
  node scripts/map-author.mjs report <map.json>
  node scripts/map-author.mjs preview <map.json> --output <preview.svg> [--tile-pixels 5] [--layers <csv>]

Recipe operations:
  {"type":"blob","material":"water","center":[40,80],"radius":[18,10],"roughness":0.35,"seed":1}
  {"type":"stroke","material":"stone","points":[[40,40],[55,47]],"width":9,"roughness":2,"seed":2}
  {"type":"road","points":[[10,20],[80,20]],"width":5}
  {"type":"base","at":[30,30]}
  {"type":"start","at":[20,20]}

Recipe symmetry supports none, horizontal, vertical, halfTurn, threeWay, radial, diagonalMain,
and diagonalAnti. Quarter-turn, three-way, and diagonal symmetry require a square map. Validation
is advisory and never rejects a readable map.

Preview layers: ${MAP_AUTHORING_LAYER_IDS.join(", ")}. Omit --layers (or use all) to show every
layer; pass a comma-separated subset to isolate authoring layers.
`;
}

export function runAuthoritativeMapTool(command, mapFile, {
  spawnSyncImpl = spawnSync,
  stdout = (value) => process.stdout.write(value),
  stderr = (value) => process.stderr.write(value),
} = {}) {
  const result = spawnSyncImpl("cargo", [
    "run", "--quiet", "--manifest-path", "server/Cargo.toml", "-p", "rts-sim",
    "--bin", "authored-map", "--", command, path.resolve(mapFile),
  ], { cwd: REPO_ROOT, encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.stdout) stdout(result.stdout);
  if (result.stderr) stderr(result.stderr);
  return Number.isInteger(result.status) ? result.status : 1;
}

export function runCli(argv = process.argv.slice(2), dependencies = {}) {
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
  if (command === "check" || command === "report") {
    if (positional.length !== 1) throw new Error(`${command} needs exactly one <map.json>`);
    return (dependencies.runAuthoritativeMapTool || runAuthoritativeMapTool)(command, positional[0]);
  }
  if (command === "preview") {
    if (!positional[0] || !options.output) throw new Error("preview needs <map.json> and --output <preview.svg>");
    if (options.layers === true) throw new Error("preview --layers needs a comma-separated layer list or all");
    const map = readJson(positional[0]);
    const output = writeFile(options.output, renderPreviewSvg(map, {
      tilePixels: options["tile-pixels"],
      layers: options.layers,
    }));
    process.stdout.write(`Wrote ${output}\n`);
    return 0;
  }
  throw new Error(`Unknown command ${JSON.stringify(command)}\n\n${usage()}`);
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_FILE;
if (isMain) {
  try {
    process.exitCode = runCli();
  } catch (error) {
    process.stderr.write(`map-author: ${error.message}\n`);
    process.exitCode = 1;
  }
}
