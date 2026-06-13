import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "../protocol.js";
import { MINING_CC_RANGE_TILES, STATS, TANK_BODY, isProducerBuilding } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS, ZOOM_STEP } from "./constants.js";

export function footprintValidAgainstEntities(
  entities,
  allowedOverlapIds,
  tileX,
  tileY,
  footW,
  footH,
  map,
) {
  if (tileX < 0 || tileY < 0) return false;
  if (tileX + footW > map.width || tileY + footH > map.height) return false;
  for (let ty = tileY; ty < tileY + footH; ty++) {
    for (let tx = tileX; tx < tileX + footW; tx++) {
      const code = map.terrain[ty * map.width + tx];
      if (!PASSABLE[code]) return false;
    }
  }
  const ts = map.tileSize;
  const minX = tileX * ts;
  const minY = tileY * ts;
  const maxX = (tileX + footW) * ts;
  const maxY = (tileY + footH) * ts;
  for (const e of entities) {
    if (e.shotReveal || e.visionOnly) continue;
    if (allowedOverlapIds?.has(e.id)) continue;
    if (entityIntersectsRect(e, minX, minY, maxX, maxY, ts)) return false;
  }
  return true;
}

export function entityIntersectsRect(e, minX, minY, maxX, maxY, tileSize) {
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
    if (isVehicleBodyKind(e.kind)) return orientedVehicleIntersectsRect(e, minX, minY, maxX, maxY, 0);
    const radius = stat.size ? stat.size : 0;
    const nearestX = Math.min(Math.max(e.x, minX), maxX);
    const nearestY = Math.min(Math.max(e.y, minY), maxY);
    const dx = e.x - nearestX;
    const dy = e.y - nearestY;
    return dx * dx + dy * dy <= radius * radius;
  }
}

export function _refreshPlacement() {
  const place = this.state.placement;
  if (!place) return;
  const map = this.state.map;
  if (!map) return;
  if (!this.mouse) return;

  const world = this._worldAt(this.mouse.x, this.mouse.y);
  const stat = STATS[place.building];
  const footW = stat && stat.footW ? stat.footW : 1;
  const footH = stat && stat.footH ? stat.footH : 1;

  // Snap so the footprint is centered on the cursor (top-left tile of the footprint).
  const tileX = Math.floor(world.x / map.tileSize - footW / 2 + 0.5);
  const tileY = Math.floor(world.y / map.tileSize - footH / 2 + 0.5);
  const valid = this._footprintValid(tileX, tileY, footW, footH, map);
  this.state.updatePlacement(tileX, tileY, valid);
}

export function _footprintValid(tileX, tileY, footW, footH, map) {
  const chosenWorker = this._selectedWorkerIds()[0];
  const allowed = chosenWorker === undefined ? new Set() : new Set([chosenWorker]);
  return footprintValidAgainstEntities(
    this.state.entitiesInterpolated(1),
    allowed,
    tileX,
    tileY,
    footW,
    footH,
    map,
  );
}

export function _confirmPlacement(ev = {}) {
  const place = this.state.placement;
  if (!place || !place.valid) return;
  const workers = this._selectedWorkerIds();
  if (workers.length === 0) {
    // No worker to build with; abandon placement rather than send a dead command.
    this.state.endPlacement();
    return;
  }
  const queued = !!ev.shiftKey;
  this._issueCommand(cmd.build(workers, place.building, place.tileX, place.tileY, queued));
  if (this.audio) this.audio.play("build_confirm", { category: "ui", priority: 2 });
  // Shift-confirm keeps placement mode active so the player can chain
  // several queued buildings; Shift keyup owns the eventual de-arm.
  if (queued) return;
  this.state.endPlacement();
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
  return kind === KIND.TANK || kind === KIND.SCOUT_CAR || kind === KIND.COMMAND_CAR;
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

/** Command-card hotkeys are single letter keys, matched against button data-hotkey. */
export function commandHotkeyFromEvent(ev) {
  if (!ev || typeof ev.code !== "string" || !ev.code.startsWith("Key")) return "";
  return ev.code.slice(3).toUpperCase();
}
