import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "../protocol.js";
import { STATS, TANK_BODY } from "../config.js";
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
  for (const e of entities) {
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

export function placementPolicyForBuilding(kind) {
  return Object.freeze({
    unitOverlap: kind === KIND.TANK_TRAP ? "infantryAllowed" : "none",
    resourceOverlap: kind === KIND.PUMP_JACK ? "oilCenterRequired" : "none",
  });
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

  // Snap so the footprint is centered on the cursor (top-left tile of the footprint).
  const tileX = Math.floor(world.x / map.tileSize - footW / 2 + 0.5);
  const tileY = Math.floor(world.y / map.tileSize - footH / 2 + 0.5);
  if (this._placementDrag && place.building === KIND.TANK_TRAP) {
    const lineSites = buildTankTrapLineSites({
      start: this._placementDrag,
      end: { tileX, tileY },
      isValid: (siteX, siteY) => {
        const blockedBy = inputFootprintPlacementBlocker(this, siteX, siteY, footW, footH, map, place.building);
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
