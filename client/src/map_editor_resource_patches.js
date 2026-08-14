import { PASSABLE } from "./protocol.js";
import { terrainCharacterAt } from "./map_authoring/terrain_layers.js";

const TILE_SIZE = 32;
const STEEL_BLOCK_DIST_TILES = 4;
const STEEL_FIELD_COLUMNS = 6;
const START_RESOURCE_MIN_DIST_TILES = 3.5;
const START_RESOURCE_MAX_DIST_TILES = 7;
const RESOURCE_NODE_RADIUS = TILE_SIZE / 2;

const OIL_OFFSET_PAIRS = Object.freeze([[6, 2], [5, 4], [3, 4], [3, 2]]);

/**
 * Mirror the server's deterministic base-resource layout for editor-only stand-ins.
 * These records are presentation data; the authored map continues to store only counts.
 */
export function mapEditorResourcePatches(draft) {
  const width = Math.max(0, Math.trunc(Number(draft?.width) || 0));
  const height = Math.max(0, Math.trunc(Number(draft?.height) || 0));
  if (!width || !height) return [];

  const sites = orderedBaseSites(draft);
  const patches = [];
  const occupiedOilTiles = [];
  const steelNodes = [];
  for (const site of sites) {
    const anchorX = (site.x + 0.5) * TILE_SIZE;
    const anchorY = (site.y + 0.5) * TILE_SIZE;
    const baseAngle = Math.atan2(height * TILE_SIZE * 0.5 - anchorY, width * TILE_SIZE * 0.5 - anchorX);
    const steel = steelPatchRecords(site, anchorX, anchorY, baseAngle);
    patches.push(...steel);
    steelNodes.push(...steel);

    const inwardX = Math.cos(baseAngle);
    const inwardY = Math.sin(baseAngle);
    const lateralX = -inwardY;
    const lateralY = inwardX;
    const outwardX = -inwardX;
    const outwardY = -inwardY;
    const blockedPumpJackTiles = resourceBlockedPumpJackTiles(width, height, steelNodes);
    const oilCount = Math.max(0, Math.trunc(Number(site.oilPatches) || 0));
    for (let index = 0; index < oilCount; index += 1) {
      const [outwardTiles, lateralTiles] = oilPatchLocalOffset(index, oilCount);
      const desired = {
        x: anchorX + (outwardTiles * outwardX + lateralTiles * lateralX) * TILE_SIZE,
        y: anchorY + (outwardTiles * outwardY + lateralTiles * lateralY) * TILE_SIZE,
      };
      const tile = nearestOilTile(draft, desired, { x: anchorX, y: anchorY }, occupiedOilTiles, blockedPumpJackTiles);
      occupiedOilTiles.push(tile);
      patches.push({ kind: "oil", x: (tile.x + 0.5) * TILE_SIZE, y: (tile.y + 0.5) * TILE_SIZE });
    }
  }
  return patches;
}

function orderedBaseSites(draft) {
  const bases = Array.isArray(draft?.baseSites) ? draft.baseSites : [];
  const starts = Array.isArray(draft?.startLocations) ? draft.startLocations : [];
  const byLocation = new Map(bases.map((site) => [`${site.x}:${site.y}`, site]));
  const ordered = [];
  const seen = new Set();
  for (const location of [...starts, ...bases]) {
    const key = `${location?.x}:${location?.y}`;
    const site = byLocation.get(key);
    if (!site || seen.has(key)) continue;
    seen.add(key);
    ordered.push(site);
  }
  return ordered;
}

function steelPatchRecords(site, anchorX, anchorY, baseAngle) {
  const patches = Math.max(0, Math.trunc(Number(site.steelPatches) || 0));
  const fieldCounts = [Math.ceil(patches / 2), Math.floor(patches / 2)];
  const inwardX = Math.cos(baseAngle);
  const inwardY = Math.sin(baseAngle);
  const lateralX = -inwardY;
  const lateralY = inwardX;
  const records = [];
  for (let sideIndex = 0; sideIndex < 2; sideIndex += 1) {
    const count = fieldCounts[sideIndex];
    if (!count) continue;
    const side = sideIndex === 0 ? 1 : -1;
    const blockDistance = side * STEEL_BLOCK_DIST_TILES * TILE_SIZE;
    const blockX = anchorX + blockDistance * lateralX;
    const blockY = anchorY + blockDistance * lateralY;
    const rows = Math.ceil(count / STEEL_FIELD_COLUMNS);
    const rowCenter = (rows - 1) / 2;
    const rowSpacing = rows > 1 ? TILE_SIZE / (rows - 1) : TILE_SIZE;
    const columnCenter = (STEEL_FIELD_COLUMNS - 1) / 2;
    for (let index = 0; index < count; index += 1) {
      const column = index % STEEL_FIELD_COLUMNS;
      const row = Math.floor(index / STEEL_FIELD_COLUMNS);
      const inward = (column - columnCenter) * TILE_SIZE;
      const lateral = (row - rowCenter) * rowSpacing;
      records.push({
        kind: "steel",
        x: blockX + inward * inwardX + lateral * lateralX,
        y: blockY + inward * inwardY + lateral * lateralY,
      });
    }
  }
  return records;
}

function nearestOilTile(draft, desired, anchor, occupiedOilTiles, blockedPumpJackTiles) {
  const constrained = nearestTile(draft, desired, (tile) => (
    hasOilGap(tile, occupiedOilTiles)
    && !blockedPumpJackTiles.has(tileKey(tile))
    && distanceTiles(tile, anchor) >= START_RESOURCE_MIN_DIST_TILES
    && distanceTiles(tile, anchor) <= START_RESOURCE_MAX_DIST_TILES
  ));
  return constrained
    || nearestTile(draft, desired, (tile) => hasOilGap(tile, occupiedOilTiles) && !blockedPumpJackTiles.has(tileKey(tile)))
    || nearestTileToWorldPoint(draft, desired);
}

function nearestTile(draft, desired, accepts) {
  let best = null;
  for (let y = 0; y < draft.height; y += 1) {
    for (let x = 0; x < draft.width; x += 1) {
      const tile = { x, y };
      if (!tilePassable(draft, tile) || !accepts(tile)) continue;
      const centerX = (x + 0.5) * TILE_SIZE;
      const centerY = (y + 0.5) * TILE_SIZE;
      const score = (centerX - desired.x) ** 2 + (centerY - desired.y) ** 2;
      if (!best || score < best.score - 0.001
        || (Math.abs(score - best.score) <= 0.001 && (y < best.y || (y === best.y && x < best.x)))) {
        best = { x, y, score };
      }
    }
  }
  return best && { x: best.x, y: best.y };
}

function nearestTileToWorldPoint(draft, desired) {
  return {
    x: clamp(roundTiesAway(desired.x / TILE_SIZE - 0.5), 0, draft.width - 1),
    y: clamp(roundTiesAway(desired.y / TILE_SIZE - 0.5), 0, draft.height - 1),
  };
}

function oilPatchLocalOffset(index, count) {
  const hasCentre = count % 2 === 1;
  if (hasCentre && index === 0) return [6, 0];
  const pairedIndex = Math.max(0, index - (hasCentre ? 1 : 0));
  const pair = OIL_OFFSET_PAIRS[Math.min(Math.floor(pairedIndex / 2), OIL_OFFSET_PAIRS.length - 1)];
  return [pair[0], pairedIndex % 2 === 0 ? -pair[1] : pair[1]];
}

function tilePassable(draft, tile) {
  const value = terrainCharacterAt(draft, tile.x, tile.y);
  if (typeof value === "number") return PASSABLE[value] === true;
  return value !== "#" && value !== "~";
}

function resourceBlockedPumpJackTiles(width, height, steelNodes) {
  const blocked = new Set();
  for (const node of steelNodes) {
    const minX = clamp(Math.floor((node.x - RESOURCE_NODE_RADIUS) / TILE_SIZE) - 1, 0, width - 1);
    const minY = clamp(Math.floor((node.y - RESOURCE_NODE_RADIUS) / TILE_SIZE) - 1, 0, height - 1);
    const maxX = clamp(Math.floor((node.x + RESOURCE_NODE_RADIUS) / TILE_SIZE), 0, width - 1);
    const maxY = clamp(Math.floor((node.y + RESOURCE_NODE_RADIUS) / TILE_SIZE), 0, height - 1);
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        if (circleIntersectsTile(node, x, y)) blocked.add(`${x}:${y}`);
      }
    }
  }
  return blocked;
}

function circleIntersectsTile(circle, tileX, tileY) {
  const minX = tileX * TILE_SIZE;
  const minY = tileY * TILE_SIZE;
  const nearestX = clamp(circle.x, minX, minX + TILE_SIZE);
  const nearestY = clamp(circle.y, minY, minY + TILE_SIZE);
  return (circle.x - nearestX) ** 2 + (circle.y - nearestY) ** 2 <= RESOURCE_NODE_RADIUS ** 2;
}

function hasOilGap(tile, occupied) {
  return occupied.every((other) => Math.abs(tile.x - other.x) > 1 || Math.abs(tile.y - other.y) > 1);
}

function distanceTiles(tile, anchor) {
  const x = (tile.x + 0.5) * TILE_SIZE;
  const y = (tile.y + 0.5) * TILE_SIZE;
  return Math.hypot(x - anchor.x, y - anchor.y) / TILE_SIZE;
}

function tileKey(tile) { return `${tile.x}:${tile.y}`; }
function roundTiesAway(value) { return Math.sign(value) * Math.round(Math.abs(value)); }
function clamp(value, min, max) { return Math.max(min, Math.min(max, value)); }
