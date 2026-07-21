import {
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_FIRE_RADIUS_TILES,
  ARTILLERY_MIN_RANGE_TILES,
} from "../config.js";
import { ABILITY, KIND, ORDER_STAGE, SETUP, UPGRADE } from "../protocol.js";
import { DEFAULT_TILE_SIZE } from "./constants.js";

export function isArtilleryFireAbility(ability) {
  return ability === ABILITY.POINT_FIRE || ability === ABILITY.BLANKET_FIRE;
}

export function artilleryMinFireRadiusTiles(upgrades = []) {
  return Array.isArray(upgrades) && upgrades.includes(UPGRADE.BALLISTIC_TABLES)
    ? ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES
    : ARTILLERY_MIN_FIRE_RADIUS_TILES;
}

export function artilleryFireRadiusTiles(
  center,
  target,
  tileSize = DEFAULT_TILE_SIZE,
  minRadiusTiles = ARTILLERY_MIN_FIRE_RADIUS_TILES,
) {
  const ts = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : DEFAULT_TILE_SIZE;
  const minimum = Number.isFinite(minRadiusTiles)
    ? Math.max(ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES, minRadiusTiles)
    : ARTILLERY_MIN_FIRE_RADIUS_TILES;
  if (!center || !target) return minimum;
  const radius = Math.hypot(target.x - center.x, target.y - center.y) / ts;
  if (!Number.isFinite(radius)) return minimum;
  return Math.max(
    minimum,
    Math.min(ARTILLERY_BLANKET_RADIUS_TILES, radius),
  );
}

export function buildArtilleryTargetLocks({
  ability,
  carriers,
  rawX,
  rawY,
  map = null,
  tileSize = DEFAULT_TILE_SIZE,
  definition = null,
  queued = false,
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
    const currentFacing = currentArtilleryFieldFacing(carrier);
    const context = artilleryTargetContext(carrier, originX, originY, currentFacing, queued);
    if (!context) continue;
    const locked = lockArtilleryFireTarget({
      bounds,
      originX: context.originX,
      originY: context.originY,
      rawX,
      rawY,
      setupFacing: context.setupFacing,
      bodyFacing: context.bodyFacing,
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
      originX: context.originX,
      originY: context.originY,
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

function artilleryTargetContext(entity, originX, originY, currentFacing, queued) {
  if (!queued && activeMovementOrderPlan(entity)) return null;
  const bodyFacing = numeric(entity?.facing, numeric(entity?.weaponFacing, currentFacing));
  const context = {
    originX,
    originY,
    setupFacing: currentFacing,
    bodyFacing,
  };
  if (!queued) return context;
  return queuedArtilleryTargetContext(entity, context);
}

function activeMovementOrderPlan(entity) {
  const first = Array.isArray(entity?.orderPlan) ? entity.orderPlan[0] : null;
  return first?.kind === ORDER_STAGE.MOVE || first?.kind === ORDER_STAGE.ATTACK_MOVE;
}

function queuedArtilleryTargetContext(entity, context) {
  if (!Array.isArray(entity?.orderPlan)) return context;
  const next = { ...context };
  for (const marker of entity.orderPlan) {
    if (
      (marker?.kind === ORDER_STAGE.MOVE || marker?.kind === ORDER_STAGE.ATTACK_MOVE) &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      next.originX = marker.x;
      next.originY = marker.y;
      next.setupFacing = null;
      continue;
    }
    if (
      marker?.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      const facing = Math.atan2(marker.y - next.originY, marker.x - next.originX);
      if (Number.isFinite(facing)) next.setupFacing = facing;
      continue;
    }
    if (marker?.kind === ORDER_STAGE.POINT_FIRE || marker?.kind === ORDER_STAGE.BLANKET_FIRE) {
      return null;
    }
  }
  return next;
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

function currentArtilleryFieldFacing(entity) {
  return firstFinite(
    entity?.setupFacing,
    entity?.weaponFacing,
    entity?.facing,
  );
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
