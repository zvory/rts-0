import { cmd, KIND } from "../protocol.js";

export function tankTrapLineTiles(start, end) {
  const x0 = finiteTile(start?.tileX);
  const y0 = finiteTile(start?.tileY);
  const x1 = finiteTile(end?.tileX);
  const y1 = finiteTile(end?.tileY);
  if (x0 == null || y0 == null || x1 == null || y1 == null) return [];

  const fullLine = bresenhamTiles(x0, y0, x1, y1);
  if (fullLine.length <= 1) return fullLine;

  const dx = Math.abs(x1 - x0);
  const dy = Math.abs(y1 - y0);
  if (dx === dy) return fullLine;

  const cadenceAxis = dx >= dy ? "tileX" : "tileY";
  const cadenced = fullLine.filter((tile) =>
    Math.abs(tile[cadenceAxis] - (cadenceAxis === "tileX" ? x0 : y0)) % 2 === 0
  );
  return bridgeKnightMoves(cadenced);
}

export function buildTankTrapLineSites({ start, end, isValid }) {
  const validFn = typeof isValid === "function" ? isValid : () => true;
  return tankTrapLineTiles(start, end).map((tile) => ({
    ...tile,
    valid: !!validFn(tile.tileX, tile.tileY),
  }));
}

export function validTankTrapLineSites(sites) {
  return (Array.isArray(sites) ? sites : [])
    .filter((site) => site?.valid)
    .map((site) => ({ tileX: site.tileX, tileY: site.tileY }));
}

export function tankTrapBuildCommands(workerIds, sites, building = KIND.TANK_TRAP) {
  const workers = Array.isArray(workerIds) ? workerIds.filter((id) => Number.isInteger(id)) : [];
  const validSites = validTankTrapLineSites(sites);
  if (workers.length === 0 || validSites.length === 0) return [];

  const commands = [];
  const immediateCount = Math.min(workers.length, validSites.length);
  for (let i = 0; i < immediateCount; i++) {
    const site = validSites[i];
    commands.push(cmd.build([workers[i]], building, site.tileX, site.tileY, false));
  }
  for (let i = immediateCount; i < validSites.length; i++) {
    const site = validSites[i];
    commands.push(cmd.build(workers, building, site.tileX, site.tileY, true));
  }
  return commands;
}

function bresenhamTiles(x0, y0, x1, y1) {
  const tiles = [];
  const dx = Math.abs(x1 - x0);
  const dy = Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx - dy;
  let x = x0;
  let y = y0;
  while (true) {
    tiles.push({ tileX: x, tileY: y, x, y });
    if (x === x1 && y === y1) break;
    const e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      x += sx;
    }
    if (e2 < dx) {
      err += dx;
      y += sy;
    }
  }
  return tiles.map(({ tileX, tileY }) => ({ tileX, tileY }));
}

function bridgeKnightMoves(tiles) {
  const bridged = [];
  for (const tile of tiles) {
    const prev = bridged[bridged.length - 1];
    if (prev && knightMove(prev, tile)) {
      bridged.push(diagonalBridge(prev, tile));
    }
    if (!sameTile(bridged[bridged.length - 1], tile)) bridged.push(tile);
  }
  return bridged;
}

function diagonalBridge(a, b) {
  return {
    tileX: a.tileX + Math.sign(b.tileX - a.tileX),
    tileY: a.tileY + Math.sign(b.tileY - a.tileY),
  };
}

function knightMove(a, b) {
  const dx = Math.abs(b.tileX - a.tileX);
  const dy = Math.abs(b.tileY - a.tileY);
  return (dx === 2 && dy === 1) || (dx === 1 && dy === 2);
}

function sameTile(a, b) {
  return a?.tileX === b?.tileX && a?.tileY === b?.tileY;
}

function finiteTile(value) {
  return Number.isInteger(value) ? value : null;
}
