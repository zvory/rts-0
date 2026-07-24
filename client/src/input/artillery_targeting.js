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
    if (bounds && !pointInsideBounds(bounds, rawX, rawY)) continue;
    const firingPosition = artilleryFiringPosition(
      context.originX,
      context.originY,
      rawX,
      rawY,
      minRangePx,
      maxRangePx,
      bounds,
    );
    if (!firingPosition) continue;
    const facing = Math.atan2(rawY - firingPosition.y, rawX - firingPosition.x);
    if (!Number.isFinite(facing)) continue;
    const inRange = !firingPosition.needsMove;
    const insideCurrentCone = carrier.setupState === SETUP.DEPLOYED &&
      inRange &&
      Number.isFinite(currentFacing) &&
      Math.abs(angleDelta(currentFacing, facing)) <= ARTILLERY_FIELD_OF_FIRE_RAD * 0.5 + 0.001;
    locks.push({
      id: carrier.id,
      kind: carrier.kind,
      originX: firingPosition.x,
      originY: firingPosition.y,
      moveFromX: context.originX,
      moveFromY: context.originY,
      x: rawX,
      y: rawY,
      rawX,
      rawY,
      facing,
      currentFacing,
      insideCurrentCone,
      needsMove: firingPosition.needsMove,
      needsRedeploy: inRange && !insideCurrentCone,
      rangePx: maxRangePx,
      minRangePx,
    });
  }
  return locks;
}

function artilleryTargetContext(entity, originX, originY, currentFacing, queued) {
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

function artilleryFiringPosition(originX, originY, rawX, rawY, minRangePx, maxRangePx, bounds) {
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
  const distance = Math.hypot(dx, dy);
  if (!Number.isFinite(distance)) return null;
  if (distance >= minRangePx && distance <= maxRangePx) {
    return { x: originX, y: originY, needsMove: false };
  }
  let preferredDirX;
  let preferredDirY;
  if (distance > Number.EPSILON) {
    preferredDirX = (originX - rawX) / distance;
    preferredDirY = (originY - rawY) / distance;
  } else {
    const centerX = bounds ? bounds.maxX * 0.5 : rawX + maxRangePx;
    const centerY = bounds ? bounds.maxY * 0.5 : rawY;
    const centerDistance = Math.hypot(centerX - rawX, centerY - rawY);
    preferredDirX = centerDistance > Number.EPSILON ? (centerX - rawX) / centerDistance : 1;
    preferredDirY = centerDistance > Number.EPSILON ? (centerY - rawY) / centerDistance : 0;
  }
  const margin = minRangePx * 0.075;
  const stagingDistance = distance < minRangePx
    ? Math.min(maxRangePx, minRangePx + margin)
    : Math.max(minRangePx, maxRangePx - margin);
  const centerDirection = bounds
    ? [bounds.maxX * 0.5 - rawX, bounds.maxY * 0.5 - rawY]
    : [preferredDirX, preferredDirY];
  const candidateDirections = [
    [preferredDirX, preferredDirY],
    centerDirection,
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
    [1, 1],
    [1, -1],
    [-1, 1],
    [-1, -1],
  ];
  for (const [dirX, dirY] of candidateDirections) {
    const dirLength = Math.hypot(dirX, dirY);
    if (!Number.isFinite(dirLength) || dirLength <= Number.EPSILON) continue;
    const x = clampToBounds(
      rawX + dirX / dirLength * stagingDistance,
      bounds?.maxX,
    );
    const y = clampToBounds(
      rawY + dirY / dirLength * stagingDistance,
      bounds?.maxY,
    );
    const targetDistance = Math.hypot(x - rawX, y - rawY);
    if (targetDistance >= minRangePx && targetDistance <= maxRangePx) {
      return { x, y, needsMove: true };
    }
  }
  return null;
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

function clampToBounds(value, max) {
  if (!Number.isFinite(max)) return value;
  return Math.max(0, Math.min(max, value));
}

function firstFinite(...values) {
  return values.find((value) => Number.isFinite(value)) ?? null;
}

function numeric(value, fallback = null) {
  return Number.isFinite(value) ? value : fallback;
}
