import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "../protocol.js";
import { STATS, TANK_BODY } from "../config.js";
import { DEFAULT_TILE_SIZE } from "./constants.js";
import { buildTankTrapLineSites, tankTrapBuildCommands } from "./tank_trap_line.js";

const POINT_IN_RECT_EPS_PX = 0.001;

export function footprintValidAgainstEntities(
  entities,
  allowedOverlapIds,
  tileX,
  tileY,
  footW,
  footH,
  map,
  policy = placementPolicyForBuilding(null),
) {
  return footprintPlacementBlocker(
    entities,
    allowedOverlapIds,
    tileX,
    tileY,
    footW,
    footH,
    map,
    policy,
  ) == null;
}

export function footprintPlacementBlocker(
  entities,
  allowedOverlapIds,
  tileX,
  tileY,
  footW,
  footH,
  map,
  policy = placementPolicyForBuilding(null),
) {
  return footprintPlacementBlockerFromSource(
    () => entities,
    allowedOverlapIds,
    tileX,
    tileY,
    footW,
    footH,
    map,
    policy,
  );
}

export function createFootprintPlacementBlockerQuery(
  entities,
  allowedOverlapIds,
  map,
  policy = placementPolicyForBuilding(null),
) {
  const nearbyEntities = createPlacementEntityQuery(entities, allowedOverlapIds, map, policy);
  return (tileX, tileY, footW, footH) => footprintPlacementBlockerFromSource(
    (minX, minY, maxX, maxY) => nearbyEntities(minX, minY, maxX, maxY),
    allowedOverlapIds,
    tileX,
    tileY,
    footW,
    footH,
    map,
    policy,
  );
}

function footprintPlacementBlockerFromSource(
  entitySource,
  allowedOverlapIds,
  tileX,
  tileY,
  footW,
  footH,
  map,
  policy,
) {
  if (tileX < 0 || tileY < 0) return "terrain";
  if (tileX + footW > map.width || tileY + footH > map.height) return "terrain";
  for (let ty = tileY; ty < tileY + footH; ty++) {
    for (let tx = tileX; tx < tileX + footW; tx++) {
      const code = map.terrain[ty * map.width + tx];
      if (!PASSABLE[code]) return "terrain";
    }
  }
  const ts = map.tileSize;
  const minX = tileX * ts;
  const minY = tileY * ts;
  const maxX = (tileX + footW) * ts;
  const maxY = (tileY + footH) * ts;
  let contextualOilCenter = false;
  for (const e of entitySource(minX, minY, maxX, maxY)) {
    if (e.shotReveal || e.visionOnly) continue;
    if (allowedOverlapIds?.has(e.id)) continue;
    if (resourceAllowedForPlacement(e, policy, minX, minY, maxX, maxY)) {
      contextualOilCenter = true;
      continue;
    }
    if (!entityBlocksPlacement(e, policy)) continue;
    if (entityIntersectsRect(e, minX, minY, maxX, maxY, ts)) {
      return isBuilding(e.kind) || isResource(e.kind) ? "structure" : "unit";
    }
  }
  if (policy?.resourceOverlap === "oilCenterRequired" && !contextualOilCenter) return "terrain";
  return null;
}

function createPlacementEntityQuery(entities, allowedOverlapIds, map, policy) {
  const tileSize = map?.tileSize;
  if (!(tileSize > 0) || !Number.isFinite(tileSize)) return () => entities;

  const buckets = new Map();
  let order = 0;
  for (const entity of entities) {
    const entityOrder = order++;
    if (entity.shotReveal || entity.visionOnly || allowedOverlapIds?.has(entity.id)) continue;
    if (!entityBlocksPlacement(entity, policy)) continue;
    const bounds = placementEntityBounds(entity, tileSize);
    if (!bounds) continue;
    const minTileX = Math.floor(bounds.minX / tileSize);
    const minTileY = Math.floor(bounds.minY / tileSize);
    const maxTileX = Math.floor(bounds.maxX / tileSize);
    const maxTileY = Math.floor(bounds.maxY / tileSize);
    const entry = { entity, order: entityOrder };
    for (let tileY = minTileY; tileY <= maxTileY; tileY++) {
      for (let tileX = minTileX; tileX <= maxTileX; tileX++) {
        const key = `${tileX},${tileY}`;
        let bucket = buckets.get(key);
        if (!bucket) {
          bucket = [];
          buckets.set(key, bucket);
        }
        bucket.push(entry);
      }
    }
  }

  return (minX, minY, maxX, maxY) => {
    const minTileX = Math.floor(minX / tileSize);
    const minTileY = Math.floor(minY / tileSize);
    const maxTileX = Math.floor(maxX / tileSize);
    const maxTileY = Math.floor(maxY / tileSize);
    if (minTileX === maxTileX && minTileY === maxTileY) {
      return (buckets.get(`${minTileX},${minTileY}`) || []).map((entry) => entry.entity);
    }
    const entries = [];
    const seenOrders = new Set();
    for (let tileY = minTileY; tileY <= maxTileY; tileY++) {
      for (let tileX = minTileX; tileX <= maxTileX; tileX++) {
        for (const entry of buckets.get(`${tileX},${tileY}`) || []) {
          if (seenOrders.has(entry.order)) continue;
          seenOrders.add(entry.order);
          entries.push(entry);
        }
      }
    }
    entries.sort((a, b) => a.order - b.order);
    return entries.map((entry) => entry.entity);
  };
}

function placementEntityBounds(entity, tileSize) {
  const stat = STATS[entity.kind];
  if (!stat || !Number.isFinite(entity.x) || !Number.isFinite(entity.y)) return null;
  if (isBuilding(entity.kind)) {
    const halfW = ((stat.footW ? stat.footW : 1) * tileSize) / 2;
    const halfH = ((stat.footH ? stat.footH : 1) * tileSize) / 2;
    return {
      minX: entity.x - halfW,
      minY: entity.y - halfH,
      maxX: entity.x + halfW,
      maxY: entity.y + halfH,
    };
  }
  if (entity.kind === KIND.ANTI_TANK_GUN) {
    const radius = circularBodyRadius(entity);
    return {
      minX: entity.x - radius,
      minY: entity.y - radius,
      maxX: entity.x + radius,
      maxY: entity.y + radius,
    };
  }
  if (isVehicleBodyKind(entity.kind)) {
    const body = vehicleBody(entity, 0);
    if (!body) return null;
    const c = Math.cos(body.facing);
    const s = Math.sin(body.facing);
    const halfW = Math.abs(c) * body.halfLen + Math.abs(s) * body.halfWidth;
    const halfH = Math.abs(s) * body.halfLen + Math.abs(c) * body.halfWidth;
    return {
      minX: body.x - halfW,
      minY: body.y - halfH,
      maxX: body.x + halfW,
      maxY: body.y + halfH,
    };
  }
  const radius = stat.size ? stat.size : 0;
  return {
    minX: entity.x - radius,
    minY: entity.y - radius,
    maxX: entity.x + radius,
    maxY: entity.y + radius,
  };
}

export function placementPolicyForBuilding(kind) {
  return Object.freeze({
    unitOverlap: kind === KIND.TANK_TRAP ? "infantryAllowed" : "none",
    resourceOverlap: kind === KIND.PUMP_JACK ? "oilCenterRequired" : "none",
  });
}

export function pumpJackBuildIntentForResource(resource, map) {
  if (!resource || resource.kind !== KIND.OIL || resource.remaining === 0 || !map) return null;
  const stat = STATS[KIND.PUMP_JACK];
  if (!stat?.footW || !stat?.footH) return null;
  const tileSize = map.tileSize || DEFAULT_TILE_SIZE;
  if (!(tileSize > 0)) return null;
  const tileX = Math.round(resource.x / tileSize - stat.footW * 0.5);
  const tileY = Math.round(resource.y / tileSize - stat.footH * 0.5);
  if (!Number.isFinite(tileX) || !Number.isFinite(tileY)) return null;
  return { building: KIND.PUMP_JACK, tileX, tileY };
}

export function nearestLiveOilPumpJackSite(entities, world, map) {
  if (!world || !map) return null;
  const tileSize = map.tileSize || DEFAULT_TILE_SIZE;
  if (!(tileSize > 0)) return null;
  const maxDistanceSq = tileSize * tileSize;
  let nearest = null;
  for (const resource of entities || []) {
    const intent = pumpJackBuildIntentForResource(resource, map);
    if (!intent) continue;
    const dx = resource.x - world.x;
    const dy = resource.y - world.y;
    const distanceSq = dx * dx + dy * dy;
    if (distanceSq > maxDistanceSq) continue;
    if (
      nearest &&
      (distanceSq > nearest.distanceSq ||
        (distanceSq === nearest.distanceSq && resource.id >= nearest.resourceId))
    ) continue;
    nearest = { ...intent, resourceId: resource.id, distanceSq };
  }
  return nearest;
}

export function movementBodyClass(kind) {
  return isVehicleBodyKind(kind) ? "vehicleBody" : "infantryLike";
}

function entityBlocksPlacement(e, policy) {
  if (isBuilding(e.kind) || isResource(e.kind)) return true;
  if (!isUnit(e.kind)) return false;
  if (STATS[e.kind]?.blocksGroundPlacement === false) return false;
  if (policy?.unitOverlap === "infantryAllowed") {
    return movementBodyClass(e.kind) === "vehicleBody";
  }
  return true;
}

function resourceAllowedForPlacement(e, policy, minX, minY, maxX, maxY) {
  if (policy?.resourceOverlap !== "oilCenterRequired") return false;
  if (e.kind !== KIND.OIL || e.remaining === 0) return false;
  return pointInsideRect(e.x, e.y, minX, minY, maxX, maxY);
}

function pointInsideRect(x, y, minX, minY, maxX, maxY) {
  return (
    x >= minX - POINT_IN_RECT_EPS_PX &&
    x <= maxX + POINT_IN_RECT_EPS_PX &&
    y >= minY - POINT_IN_RECT_EPS_PX &&
    y <= maxY + POINT_IN_RECT_EPS_PX
  );
}

export function entityIntersectsRect(e, minX, minY, maxX, maxY, tileSize) {
  return entityIntersectsRectWithBodyPolicy(e, minX, minY, maxX, maxY, tileSize, isVehicleBodyKind);
}

export function selectionEntityIntersectsRect(e, minX, minY, maxX, maxY, tileSize) {
  return entityIntersectsRectWithBodyPolicy(e, minX, minY, maxX, maxY, tileSize, hasOrientedSelectionBody);
}

function entityIntersectsRectWithBodyPolicy(e, minX, minY, maxX, maxY, tileSize, orientedBodyKind) {
  const stat = STATS[e.kind];
  if (!stat) return false;
  let halfW;
  let halfH;
  if (isBuilding(e.kind)) {
    halfW = ((stat.footW ? stat.footW : 1) * tileSize) / 2;
    halfH = ((stat.footH ? stat.footH : 1) * tileSize) / 2;
    return e.x + halfW > minX && e.x - halfW < maxX && e.y + halfH > minY && e.y - halfH < maxY;
  } else {
    if (e.kind === KIND.ANTI_TANK_GUN) return bodyCircleIntersectsRect(e, minX, minY, maxX, maxY, 0);
    if (orientedBodyKind(e.kind)) return orientedVehicleIntersectsRect(e, minX, minY, maxX, maxY, 0);
    const radius = stat.size ? stat.size : 0;
    const nearestX = Math.min(Math.max(e.x, minX), maxX);
    const nearestY = Math.min(Math.max(e.y, minY), maxY);
    const dx = e.x - nearestX;
    const dy = e.y - nearestY;
    return dx * dx + dy * dy <= radius * radius;
  }
}

export function _refreshPlacement() {
  const intent = clientIntent(this);
  const place = intent?.placement;
  if (!place) return;
  const map = this.state.map;
  if (!map) return;
  if (!this.mouse) return;

  const world = this._groundAtScreen(this.mouse.x, this.mouse.y);
  if (!world) {
    intent?.updatePlacement?.(place.tileX, place.tileY, false);
    return;
  }
  const stat = STATS[place.building];
  const footW = stat && stat.footW ? stat.footW : 1;
  const footH = stat && stat.footH ? stat.footH : 1;

  // Pump Jacks target resource entities rather than arbitrary ground. Snap to
  // a nearby visible live oil patch so the player need not pixel-hunt it without
  // letting an empty-ground click target a distant or off-screen patch.
  const pumpJackSite = place.building === KIND.PUMP_JACK
    ? nearestLiveOilPumpJackSite(this._selectionEntities(), world, map)
    : null;
  // Other buildings stay centered on the cursor (top-left footprint tile).
  const tileX = pumpJackSite?.tileX ?? Math.floor(world.x / map.tileSize - footW / 2 + 0.5);
  const tileY = pumpJackSite?.tileY ?? Math.floor(world.y / map.tileSize - footH / 2 + 0.5);
  if (place.building === KIND.PUMP_JACK && !pumpJackSite) {
    intent?.updatePlacement?.(tileX, tileY, false);
    return;
  }
  if (this._placementDrag && place.building === KIND.TANK_TRAP) {
    const chosenWorker = this._selectedWorkerIds()[0];
    const allowed = chosenWorker === undefined ? new Set() : new Set([chosenWorker]);
    const blockerAt = createFootprintPlacementBlockerQuery(
      this._selectionEntities(),
      allowed,
      map,
      placementPolicyForBuilding(place.building),
    );
    const lineSites = buildTankTrapLineSites({
      start: this._placementDrag,
      end: { tileX, tileY },
      isValid: (siteX, siteY) => {
        const blockedBy = blockerAt(siteX, siteY, footW, footH);
        return { valid: blockedBy == null, blockedBy };
      },
    });
    const firstValid = lineSites.find((site) => site.valid) || lineSites[0] || { tileX, tileY, valid: false };
    intent?.updatePlacement?.(firstValid.tileX, firstValid.tileY, !!lineSites.some((site) => site.valid), {
      lineSites,
    });
    return;
  }

  const valid = this._footprintValid(tileX, tileY, footW, footH, map, place.building);
  const lineSites = place.building === KIND.TANK_TRAP ? [{ tileX, tileY, valid }] : undefined;
  intent?.updatePlacement?.(tileX, tileY, valid, lineSites ? { lineSites } : {});
}

export function _footprintValid(tileX, tileY, footW, footH, map, buildingKind = null) {
  return inputFootprintPlacementBlocker(this, tileX, tileY, footW, footH, map, buildingKind) == null;
}

function inputFootprintPlacementBlocker(input, tileX, tileY, footW, footH, map, buildingKind = null) {
  const chosenWorker = input._selectedWorkerIds()[0];
  const allowed = chosenWorker === undefined ? new Set() : new Set([chosenWorker]);
  return footprintPlacementBlocker(
    input._selectionEntities(),
    allowed,
    tileX,
    tileY,
    footW,
    footH,
    map,
    placementPolicyForBuilding(buildingKind),
  );
}

export function _confirmPlacement(ev = {}) {
  const intent = clientIntent(this);
  const place = intent?.placement;
  if (!place || !place.valid) return;
  const workers = this._selectedWorkerIds();
  if (workers.length === 0) {
    // No worker to build with; abandon placement rather than send a dead command.
    intent?.endPlacement?.();
    return;
  }
  if (place.building === KIND.TANK_TRAP && Array.isArray(place.lineSites)) {
    this._confirmTankTrapLinePlacement(place, workers, ev);
    return;
  }
  const queued = !!ev.shiftKey;
  this.commandInteraction.issueCommand(cmd.build(workers, place.building, place.tileX, place.tileY, queued));
  if (this.audio) this.audio.play("build_confirm", { category: "ui", priority: 2 });
  // Shift-confirm keeps placement mode active so the player can chain
  // several queued buildings; Shift keyup owns the eventual de-arm.
  if (queued) return;
  intent?.endPlacement?.();
}

export function _beginTankTrapPlacementDrag() {
  const place = clientIntent(this)?.placement;
  if (!place || place.building !== KIND.TANK_TRAP) return false;
  this._placementDrag = { tileX: place.tileX, tileY: place.tileY };
  this._refreshPlacement();
  return true;
}

export function _finishTankTrapPlacementDrag(ev = {}) {
  if (!this._placementDrag) return false;
  this._placementDrag = null;
  this._confirmPlacement(ev);
  return true;
}

export function _cancelPlacementDrag() {
  this._placementDrag = null;
}

export function _confirmTankTrapLinePlacement(place, workers, ev = {}) {
  const commands = tankTrapBuildCommands(workers, place.lineSites, place.building);
  if (commands.length === 0) return false;
  for (const command of commands) this.commandInteraction.issueCommand(command);
  if (this.audio) this.audio.play("build_confirm", { category: "ui", priority: 2 });
  if (!ev.shiftKey) clientIntent(this)?.endPlacement?.();
  return true;
}

// Shared input helpers.

export function pointHitsOrientedVehicle(e, wx, wy, pad) {
  const body = vehicleBody(e, pad);
  if (!body) return false;
  const dx = wx - e.x;
  const dy = wy - e.y;
  const c = Math.cos(body.facing);
  const s = Math.sin(body.facing);
  const forward = dx * c + dy * s;
  const side = -dx * s + dy * c;
  return Math.abs(forward) <= body.halfLen && Math.abs(side) <= body.halfWidth;
}

export function orientedVehicleIntersectsRect(e, minX, minY, maxX, maxY, pad) {
  const body = vehicleBody(e, pad);
  if (!body) return false;
  return orientedBoxIntersectsRect(body, minX, minY, maxX, maxY);
}

export function pointHitsBodyCircle(e, wx, wy, pad) {
  const radius = circularBodyRadius(e) + pad;
  return Math.hypot(wx - e.x, wy - e.y) <= radius;
}

export function bodyCircleIntersectsRect(e, minX, minY, maxX, maxY, pad) {
  const radius = circularBodyRadius(e) + pad;
  const nearestX = Math.min(Math.max(e.x, minX), maxX);
  const nearestY = Math.min(Math.max(e.y, minY), maxY);
  const dx = e.x - nearestX;
  const dy = e.y - nearestY;
  return dx * dx + dy * dy <= radius * radius;
}

export function circularBodyRadius(e) {
  const body = STATS[e.kind]?.body;
  if (body) return body.width * 0.5 + (body.clearance || 0);
  return STATS[e.kind]?.size || 0;
}

export function vehicleBody(e, pad) {
  const stat = STATS[e.kind];
  const body = (stat && stat.body) || TANK_BODY;
  const facing = typeof e.facing === "number" && Number.isFinite(e.facing) ? e.facing : 0;
  if (!Number.isFinite(e.x) || !Number.isFinite(e.y)) return null;
  return {
    x: e.x,
    y: e.y,
    halfLen: body.length * 0.5 + (body.clearance || 0) + pad,
    halfWidth: body.width * 0.5 + (body.clearance || 0) + pad,
    facing,
  };
}

export function isVehicleBodyKind(kind) {
  return kind === KIND.ANTI_TANK_GUN ||
    kind === KIND.MORTAR_TEAM ||
    kind === KIND.ARTILLERY ||
    kind === KIND.TANK ||
    kind === KIND.SCOUT_CAR ||
    kind === KIND.COMMAND_CAR;
}

export function hasOrientedSelectionBody(kind) {
  return kind === KIND.SCOUT_PLANE || isVehicleBodyKind(kind);
}

export function orientedBoxIntersectsRect(body, minX, minY, maxX, maxY) {
  const rectCx = (minX + maxX) * 0.5;
  const rectCy = (minY + maxY) * 0.5;
  const rectHalfW = (maxX - minX) * 0.5;
  const rectHalfH = (maxY - minY) * 0.5;
  const c = Math.cos(body.facing);
  const s = Math.sin(body.facing);
  const axes = [
    [1, 0],
    [0, 1],
    [c, s],
    [-s, c],
  ];
  for (const [ax, ay] of axes) {
    const boxCenter = body.x * ax + body.y * ay;
    const rectCenter = rectCx * ax + rectCy * ay;
    const boxRadius =
      Math.abs(ax * c + ay * s) * body.halfLen +
      Math.abs(ax * -s + ay * c) * body.halfWidth;
    const rectRadius = Math.abs(ax) * rectHalfW + Math.abs(ay) * rectHalfH;
    if (Math.abs(boxCenter - rectCenter) > boxRadius + rectRadius) return false;
  }
  return true;
}

/** True if the event target is an editable text field we must not steal keys from. */
export function isTextEntry(el) {
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || el.isContentEditable === true;
}

/** Command-card hotkeys use the physical KeyboardEvent.code identity. */
export function commandHotkeyCodeFromEvent(ev) {
  return /^Key[A-Z]$/.test(ev?.code || "") ? ev.code : "";
}

function clientIntent(input) {
  return input?.clientIntent || null;
}
