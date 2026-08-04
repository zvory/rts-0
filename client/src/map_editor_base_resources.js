const TILE_SIZE = 32;
const STEEL_FIELD_COLUMNS = 6;
const STEEL_BLOCK_DIST_TILES = 4;
const START_RESOURCE_MIN_DIST_TILES = 3.5;
const START_RESOURCE_MAX_DIST_TILES = 7;
const RESOURCE_NODE_RADIUS = TILE_SIZE / 2;

const OIL_OFFSETS = Object.freeze([
  [4, 4], [4, 2], [6, 3], [2, 4], [3, 6],
  [6, 5], [1, 6], [6, 1], [0, 4],
]);

/**
 * Build the resource-node preview shown by the Map Editor. This mirrors the live setup geometry
 * in server/crates/sim/src/game/setup.rs; the focused contract test keeps its constants aligned.
 */
export function mapEditorBaseResourcePreviews(draft) {
  const width = Math.max(0, Math.trunc(Number(draft?.width)) || 0);
  const height = Math.max(0, Math.trunc(Number(draft?.height)) || 0);
  if (!width || !height) return [];
  const sites = Array.isArray(draft?.baseSites) ? draft.baseSites : [];
  const spawnedSteel = [];
  const occupiedOilTiles = new Set();
  const previews = [];
  for (const [baseIndex, site] of sites.entries()) {
    const steel = steelPreviews(site, baseIndex, width, height);
    spawnedSteel.push(...steel);
    previews.push(...steel);
    const blockedPumpJackTiles = steelBlockedPumpJackTiles(spawnedSteel, width, height);
    previews.push(...oilPreviews(
      site,
      baseIndex,
      draft,
      occupiedOilTiles,
      blockedPumpJackTiles,
    ));
  }
  return previews;
}

function steelPreviews(site, baseIndex, width, height) {
  const count = boundedCount(site?.steelPatches, 36);
  const { homeX, homeY, baseAngle } = baseGeometry(site, width, height);
  const perpX = -Math.sin(baseAngle);
  const perpY = Math.cos(baseAngle);
  const fieldCounts = [Math.ceil(count / 2), Math.floor(count / 2)];
  const previews = [];
  for (const [fieldIndex, fieldPatches] of fieldCounts.entries()) {
    if (!fieldPatches) continue;
    const side = fieldIndex === 0 ? 1 : -1;
    const blockDistance = side * STEEL_BLOCK_DIST_TILES * TILE_SIZE;
    const blockX = homeX + blockDistance * Math.cos(baseAngle);
    const blockY = homeY + blockDistance * Math.sin(baseAngle);
    const rows = Math.ceil(fieldPatches / STEEL_FIELD_COLUMNS);
    const rowCenter = (rows - 1) / 2;
    const rowSpacing = rows > 1 ? TILE_SIZE / (rows - 1) : TILE_SIZE;
    const columnCenter = (STEEL_FIELD_COLUMNS - 1) / 2;
    for (let index = 0; index < fieldPatches; index += 1) {
      const column = index % STEEL_FIELD_COLUMNS;
      const row = Math.floor(index / STEEL_FIELD_COLUMNS);
      const offsetX = (column - columnCenter) * TILE_SIZE;
      const offsetY = (row - rowCenter) * rowSpacing;
      previews.push({
        kind: "steel",
        baseIndex,
        x: blockX + offsetX * perpX + offsetY * Math.cos(baseAngle),
        y: blockY + offsetX * perpY + offsetY * Math.sin(baseAngle),
      });
    }
  }
  return previews;
}

function oilPreviews(site, baseIndex, draft, occupied, blocked) {
  const width = Math.max(1, Math.trunc(Number(draft.width)) || 1);
  const height = Math.max(1, Math.trunc(Number(draft.height)) || 1);
  const count = boundedCount(site?.oilPatches, OIL_OFFSETS.length);
  const { homeX, homeY, baseAngle } = baseGeometry(site, width, height);
  const oilAngle = baseAngle + Math.PI / 2;
  const stepX = Math.cos(oilAngle) < 0 ? -1 : 1;
  const stepY = Math.sin(oilAngle) < 0 ? -1 : 1;
  const previews = [];
  for (let index = 0; index < count; index += 1) {
    const [offsetX, offsetY] = OIL_OFFSETS[index];
    const desiredX = clamp(Math.trunc(Number(site?.x)) + offsetX * stepX, 0, width - 1);
    const desiredY = clamp(Math.trunc(Number(site?.y)) + offsetY * stepY, 0, height - 1);
    const tile = nearestOilTile(draft, desiredX, desiredY, homeX, homeY, occupied, blocked);
    occupied.add(tileKey(tile.x, tile.y));
    previews.push({
      kind: "oil",
      baseIndex,
      x: (tile.x + 0.5) * TILE_SIZE,
      y: (tile.y + 0.5) * TILE_SIZE,
    });
  }
  return previews;
}

function nearestOilTile(draft, desiredX, desiredY, homeX, homeY, occupied, blocked) {
  return nearestAcceptedTile(draft, desiredX, desiredY, (x, y) => (
    oilTileAccepted(x, y, occupied, blocked) && resourceDistanceAccepted(x, y, homeX, homeY)
  )) || nearestAcceptedTile(draft, desiredX, desiredY, (x, y) => (
    oilTileAccepted(x, y, occupied, blocked)
  )) || { x: desiredX, y: desiredY };
}

function nearestAcceptedTile(draft, desiredX, desiredY, accepted) {
  let best = null;
  const maxRadius = Math.max(desiredX, draft.width - desiredX - 1, desiredY, draft.height - desiredY - 1);
  const consider = (x, y) => {
    if (x < 0 || y < 0 || x >= draft.width || y >= draft.height) return;
    if (!tilePassable(draft, x, y) || !accepted(x, y)) return;
    const score = (x - desiredX) ** 2 + (y - desiredY) ** 2;
    if (!best || score < best.score || (score === best.score && (y < best.y || (y === best.y && x < best.x)))) {
      best = { x, y, score };
    }
  };
  for (let radius = 0; radius <= maxRadius; radius += 1) {
    if (radius === 0) {
      consider(desiredX, desiredY);
    } else {
      const left = desiredX - radius;
      const right = desiredX + radius;
      const top = desiredY - radius;
      const bottom = desiredY + radius;
      for (let x = left; x <= right; x += 1) {
        consider(x, top);
        consider(x, bottom);
      }
      for (let y = top + 1; y < bottom; y += 1) {
        consider(left, y);
        consider(right, y);
      }
    }
    // Every unvisited tile is at least radius + 1 away on one axis. Strict inequality preserves
    // the server's row/column tie-break when a later ring can match the current squared distance.
    if (best && best.score < (radius + 1) ** 2) break;
  }
  return best;
}

function oilTileAccepted(x, y, occupied, blocked) {
  if (blocked.has(tileKey(x, y))) return false;
  for (let otherY = y - 1; otherY <= y + 1; otherY += 1) {
    for (let otherX = x - 1; otherX <= x + 1; otherX += 1) {
      if (occupied.has(tileKey(otherX, otherY))) return false;
    }
  }
  return true;
}

function resourceDistanceAccepted(x, y, homeX, homeY) {
  const centerX = (x + 0.5) * TILE_SIZE;
  const centerY = (y + 0.5) * TILE_SIZE;
  const distance = Math.hypot(centerX - homeX, centerY - homeY) / TILE_SIZE;
  return distance >= START_RESOURCE_MIN_DIST_TILES && distance <= START_RESOURCE_MAX_DIST_TILES;
}

function steelBlockedPumpJackTiles(steel, width, height) {
  const blocked = new Set();
  for (const node of steel) {
    const minX = clamp(Math.floor((node.x - RESOURCE_NODE_RADIUS) / TILE_SIZE) - 1, 0, width - 1);
    const minY = clamp(Math.floor((node.y - RESOURCE_NODE_RADIUS) / TILE_SIZE) - 1, 0, height - 1);
    const maxX = clamp(Math.floor((node.x + RESOURCE_NODE_RADIUS) / TILE_SIZE), 0, width - 1);
    const maxY = clamp(Math.floor((node.y + RESOURCE_NODE_RADIUS) / TILE_SIZE), 0, height - 1);
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        const nearestX = clamp(node.x, x * TILE_SIZE, (x + 1) * TILE_SIZE);
        const nearestY = clamp(node.y, y * TILE_SIZE, (y + 1) * TILE_SIZE);
        if ((node.x - nearestX) ** 2 + (node.y - nearestY) ** 2 <= RESOURCE_NODE_RADIUS ** 2) {
          blocked.add(tileKey(x, y));
        }
      }
    }
  }
  return blocked;
}

function baseGeometry(site, width, height) {
  const homeX = (Number(site?.x) + 0.5) * TILE_SIZE;
  const homeY = (Number(site?.y) + 0.5) * TILE_SIZE;
  const baseAngle = Math.atan2(height * TILE_SIZE * 0.5 - homeY, width * TILE_SIZE * 0.5 - homeX);
  return { homeX, homeY, baseAngle };
}

function tilePassable(draft, x, y) {
  const terrain = draft?.terrain?.[y]?.[x];
  return terrain !== "#" && terrain !== "~";
}

function boundedCount(value, max) {
  const count = Number(value);
  return Number.isInteger(count) ? clamp(count, 0, max) : 0;
}

function tileKey(x, y) { return `${x},${y}`; }
function clamp(value, min, max) { return Math.max(min, Math.min(max, value)); }
