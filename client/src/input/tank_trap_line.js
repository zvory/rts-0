import { cmd, KIND } from "../protocol.js";

export function tankTrapLineTiles(start, end) {
  const x0 = finiteTile(start?.tileX);
  const y0 = finiteTile(start?.tileY);
  const x1 = finiteTile(end?.tileX);
  const y1 = finiteTile(end?.tileY);
  if (x0 == null || y0 == null || x1 == null || y1 == null) return [];

  const dx = Math.abs(x1 - x0);
  const dy = Math.abs(y1 - y0);
  const sx = Math.sign(x1 - x0);
  const sy = Math.sign(y1 - y0);
  const tiles = [{ tileX: x0, tileY: y0 }];
  let x = x0;
  let y = y0;

  const majorIsX = dx >= dy;
  const diagonalCount = Math.min(dx, dy);
  const straightCount = Math.floor((Math.max(dx, dy) - diagonalCount) / 2);
  const stepCount = diagonalCount + straightCount;
  let diagonalsUsed = 0;

  for (let step = 1; step <= stepCount; step++) {
    const targetDiagonals = Math.round((step * diagonalCount) / stepCount);
    if (diagonalsUsed < targetDiagonals) {
      x += sx;
      y += sy;
      diagonalsUsed++;
    } else {
      if (majorIsX) {
        x += sx * 2;
      } else {
        y += sy * 2;
      }
    }
    tiles.push({ tileX: x, tileY: y });
  }

  return tiles;
}

export function buildTankTrapLineSites({ start, end, isValid, building = KIND.TANK_TRAP }) {
  if (building === KIND.BARBED_WIRE) {
    let blocked = false;
    return barbedWireLineTiles(start, end).map((tile) => {
      const placement = normalizePlacementResult(isValid?.(tile.tileX, tile.tileY) ?? true);
      blocked ||= !placement.valid;
      return { ...tile, valid: !blocked, blockedBy: placement.blockedBy };
    });
  }
  const validFn = typeof isValid === "function" ? isValid : () => true;
  let previousValid = null;
  let gap = null;
  return tankTrapLineTiles(start, end).map((tile) => {
    const placement = normalizePlacementResult(validFn(tile.tileX, tile.tileY));
    const skipped = !placement.valid && skippableTankTrapBlocker(placement.blockedBy);
    let valid = false;
    if (placement.valid) {
      valid = !previousValid || gap === "skipped" || allowedTankTrapStep(previousValid, tile);
      if (valid) {
        previousValid = tile;
        gap = null;
      } else {
        gap = "blocked";
      }
    } else if (skipped && gap !== "blocked") {
      gap = "skipped";
    } else {
      gap = "blocked";
    }
    return { ...tile, valid, skipped, blockedBy: placement.blockedBy };
  });
}

export function validTankTrapLineSites(sites) {
  const validSites = [];
  let gap = null;
  for (const site of Array.isArray(sites) ? sites : []) {
    if (!site?.valid) {
      if (site?.skipped && gap !== "blocked") {
        gap = "skipped";
      } else {
        gap = "blocked";
      }
      continue;
    }
    const tile = { tileX: site.tileX, tileY: site.tileY };
    const previous = validSites[validSites.length - 1];
    if (previous && gap !== "skipped" && !allowedTankTrapStep(previous, tile)) {
      gap = "blocked";
      continue;
    }
    validSites.push(tile);
    gap = null;
  }
  return validSites;
}

export function tankTrapBuildCommands(workerIds, sites, building = KIND.TANK_TRAP) {
  const workers = Array.isArray(workerIds) ? workerIds.filter((id) => Number.isInteger(id)) : [];
  const lineSites = Array.isArray(sites) ? sites : [];
  const blockedIndex = lineSites.findIndex((site) => !site.valid);
  const validSites = building === KIND.BARBED_WIRE
    ? lineSites.slice(0, blockedIndex < 0 ? lineSites.length : blockedIndex)
    : validTankTrapLineSites(sites);
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

function allowedTankTrapStep(a, b) {
  const dx = Math.abs(b.tileX - a.tileX);
  const dy = Math.abs(b.tileY - a.tileY);
  return (dx === 1 && dy === 1) || (dx === 2 && dy === 0) || (dx === 0 && dy === 2);
}

function normalizePlacementResult(result) {
  if (typeof result === "object" && result !== null) {
    return { valid: !!result.valid, blockedBy: result.blockedBy ?? null };
  }
  return { valid: !!result, blockedBy: result ? null : "blocked" };
}

function skippableTankTrapBlocker(blockedBy) {
  return blockedBy === "terrain" || blockedBy === "structure";
}

function finiteTile(value) {
  return Number.isInteger(value) ? value : null;
}


export function barbedWireLineTiles(start, end) {
  const values = [start?.tileX, start?.tileY, end?.tileX, end?.tileY];
  if (!values.every(Number.isInteger)) return [];
  let [x, y, x1, y1] = values;
  const dx = Math.abs(x1 - x), dy = Math.abs(y1 - y);
  const sx = Math.sign(x1 - x), sy = Math.sign(y1 - y);
  let error = dx - dy;
  const tiles = [{ tileX: x, tileY: y }];
  while (x !== x1 || y !== y1) {
    const twice = error * 2;
    if (twice > -dy) {
      error -= dy; x += sx;
      tiles.push({ tileX: x, tileY: y });
    }
    if (twice < dx) {
      error += dx; y += sy;
      tiles.push({ tileX: x, tileY: y });
    }
  }
  return tiles;
}
