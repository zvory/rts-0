import {
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
} from "../config.js";
import { ABILITY, KIND, ORDER_STAGE, SETUP } from "../protocol.js";
import { DEFAULT_TILE_SIZE } from "./constants.js";

export function isArtilleryFireAbility(ability) {
  return ability === ABILITY.POINT_FIRE || ability === ABILITY.BLANKET_FIRE;
}

export function buildArtilleryTargetLocks({
  ability,
  carriers,
  rawX,
  rawY,
  map = null,
  tileSize = DEFAULT_TILE_SIZE,
  definition = null,
} = {}) {
  if (!isArtilleryFireAbility(ability) || !Array.isArray(carriers)) return [];
  if (!Number.isFinite(rawX) || !Number.isFinite(rawY)) return [];
  const ts = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : DEFAULT_TILE_SIZE;
  const minRangePx = numeric(definition?.minRangeTiles, ARTILLERY_MIN_RANGE_TILES) * ts;
  const maxRangePx = numeric(definition?.rangeTiles, ARTILLERY_MAX_RANGE_TILES) * ts;
  const bounds = worldBounds(map, ts);
  const locks = [];
  for (const carrier of carriers) {
    if (carrier?.kind !== KIND.ARTILLERY) continue;
    const originX = numeric(carrier.x);
    const originY = numeric(carrier.y);
    if (!Number.isFinite(originX) || !Number.isFinite(originY)) continue;
    const currentFacing = currentArtilleryFieldFacing(carrier, originX, originY);
    const bodyFacing = numeric(carrier.facing, numeric(carrier.weaponFacing, currentFacing));
    const locked = lockArtilleryFireTarget({
      bounds,
      originX,
      originY,
      rawX,
      rawY,
      setupFacing: currentFacing,
      bodyFacing,
      minRangePx,
      maxRangePx,
    });
    if (!locked) continue;
    const insideCurrentCone = carrier.setupState === SETUP.DEPLOYED &&
      Number.isFinite(currentFacing) &&
      Math.abs(angleDelta(currentFacing, locked.facing)) <= ARTILLERY_FIELD_OF_FIRE_RAD * 0.5 + 0.001;
    locks.push({
      id: carrier.id,
      kind: carrier.kind,
      originX,
      originY,
      x: locked.x,
      y: locked.y,
      rawX,
      rawY,
      facing: locked.facing,
      currentFacing,
      insideCurrentCone,
      needsRedeploy: !insideCurrentCone,
      rangePx: maxRangePx,
      minRangePx,
    });
  }
  return locks;
}

function lockArtilleryFireTarget({
  bounds,
  originX,
  originY,
  rawX,
  rawY,
  setupFacing,
  bodyFacing,
  minRangePx,
  maxRangePx,
}) {
  if (
    !Number.isFinite(originX) ||
    !Number.isFinite(originY) ||
    !Number.isFinite(rawX) ||
    !Number.isFinite(rawY) ||
    !Number.isFinite(minRangePx) ||
    !Number.isFinite(maxRangePx) ||
    minRangePx < 0 ||
    maxRangePx < minRangePx
  ) {
    return null;
  }
  const dx = rawX - originX;
  const dy = rawY - originY;
  const hasClickDirection = Math.abs(dx) > Number.EPSILON || Math.abs(dy) > Number.EPSILON;
  const distance = Math.hypot(dx, dy);
  const facing = hasClickDirection
    ? Math.atan2(dy, dx)
    : firstFinite(setupFacing, bodyFacing);
  if (!Number.isFinite(facing)) return null;
  const dirX = Math.cos(facing);
  const dirY = Math.sin(facing);
  if (!Number.isFinite(dirX) || !Number.isFinite(dirY)) return null;
  const exitDistance = bounds ? rayMapExitDistance(bounds, originX, originY, dirX, dirY) : Infinity;
  if (!Number.isFinite(exitDistance) && exitDistance !== Infinity) return null;
  const maxValid = Math.min(maxRangePx, exitDistance);
  if (maxValid < minRangePx) return null;
  const desired = Number.isFinite(distance)
    ? clamp(distance, minRangePx, maxRangePx)
    : maxRangePx;
  const lockedDistance = Math.max(minRangePx, Math.min(desired, maxValid));
  const x = originX + dirX * lockedDistance;
  const y = originY + dirY * lockedDistance;
  if (bounds && !pointInsideBounds(bounds, x, y)) return null;
  return { x, y, facing };
}

function worldBounds(map, tileSize) {
  const widthTiles = numeric(map?.width, numeric(map?.size));
  const heightTiles = numeric(map?.height, widthTiles);
  if (!(widthTiles > 0) || !(heightTiles > 0)) return null;
  return {
    maxX: Math.max(0, widthTiles * tileSize - 1),
    maxY: Math.max(0, heightTiles * tileSize - 1),
  };
}

function rayMapExitDistance(bounds, originX, originY, dirX, dirY) {
  let enter = 0;
  let exit = Infinity;
  for (const [origin, dir, max] of [[originX, dirX, bounds.maxX], [originY, dirY, bounds.maxY]]) {
    if (Math.abs(dir) <= Number.EPSILON) {
      if (origin < 0 || origin > max) return null;
      continue;
    }
    let near = (0 - origin) / dir;
    let far = (max - origin) / dir;
    if (near > far) [near, far] = [far, near];
    enter = Math.max(enter, near);
    exit = Math.min(exit, far);
  }
  if (!Number.isFinite(exit) || exit < enter || exit < 0) return null;
  return Math.max(0, exit);
}

function pointInsideBounds(bounds, x, y) {
  return Number.isFinite(x) &&
    Number.isFinite(y) &&
    x >= 0 &&
    y >= 0 &&
    x <= bounds.maxX &&
    y <= bounds.maxY;
}

function currentArtilleryFieldFacing(entity, originX, originY) {
  return firstFinite(
    entity?.setupFacing,
    plannedSetupFacing(entity, originX, originY),
    entity?.weaponFacing,
    entity?.facing,
  );
}

function plannedSetupFacing(entity, originX, originY) {
  if (!Array.isArray(entity?.orderPlan)) return null;
  let facing = null;
  for (const marker of entity.orderPlan) {
    if (
      marker?.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      const next = Math.atan2(marker.y - originY, marker.x - originX);
      if (Number.isFinite(next)) facing = next;
    }
  }
  return facing;
}

function angleDelta(a, b) {
  let d = (a - b) % (Math.PI * 2);
  if (d < -Math.PI) d += Math.PI * 2;
  if (d > Math.PI) d -= Math.PI * 2;
  return d;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function firstFinite(...values) {
  return values.find((value) => Number.isFinite(value)) ?? null;
}

function numeric(value, fallback = null) {
  return Number.isFinite(value) ? value : fallback;
}
