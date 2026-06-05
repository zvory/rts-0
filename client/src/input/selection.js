import { isUnit, isBuilding, isResource, KIND, SETUP } from "../protocol.js";
import { STATS } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS } from "./constants.js";
import { entityIntersectsRect, isVehicleBodyKind, pointHitsOrientedVehicle } from "./placement.js";

export function _commitClickSelection(p, additive, ctrl) {
  const world = this._worldAt(p.x, p.y);
  const hit = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ true);
  if (!hit) {
    if (!additive) this.state.clearSelection();
    return;
  }
  if (ctrl && isUnit(hit.kind) && hit.owner === this.state.playerId) {
    const ids = this._closestOwnUnitKindInViewport(hit.kind, hit.x, hit.y, hit);
    if (additive) this.state.addToSelection(ids);
    else this.state.setSelection(ids);
    return;
  }
  if (ctrl && isBuilding(hit.kind) && hit.owner === this.state.playerId) {
    const ids = this._ownBuildingsOfKindInViewport(hit.kind);
    if (additive) this.state.addToSelection(ids);
    else this.state.setSelection(ids);
    return;
  }
  if (additive) {
    if (this.state.selection.has(hit.id)) this.state.removeFromSelection([hit.id]);
    else this.state.addToSelection([hit.id]);
    return;
  }
  else this.state.setSelection([hit.id]);
}

export function _ownBuildingsOfKindInViewport(kind) {
  const el = this.dom;
  const w = el.clientWidth;
  const h = el.clientHeight;
  const topLeft = this.camera.screenToWorld(0, 0);
  const botRight = this.camera.screenToWorld(w, h);
  const minX = Math.min(topLeft.x, botRight.x);
  const maxX = Math.max(topLeft.x, botRight.x);
  const minY = Math.min(topLeft.y, botRight.y);
  const maxY = Math.max(topLeft.y, botRight.y);
  const me = this.state.playerId;
  return this.state
    .entitiesInterpolated(1)
    .filter(
      (e) =>
        e.owner === me &&
        e.kind === kind &&
        e.x >= minX && e.x <= maxX &&
        e.y >= minY && e.y <= maxY,
    )
    .sort((a, b) => a.id - b.id)
    .map((e) => e.id);
}

export function _unitSelectionGroup(e) {
  if (!e) return "";
  if (e.kind !== KIND.AT_TEAM) return e.kind;
  return `${e.kind}:${e.setupState || SETUP.PACKED}`;
}

export function _closestOwnUnitKindInViewport(kind, anchorX, anchorY, anchor = null) {
  const el = this.dom;
  const w = el.clientWidth;
  const h = el.clientHeight;
  const topLeft = this.camera.screenToWorld(0, 0);
  const botRight = this.camera.screenToWorld(w, h);
  const minX = Math.min(topLeft.x, botRight.x);
  const maxX = Math.max(topLeft.x, botRight.x);
  const minY = Math.min(topLeft.y, botRight.y);
  const maxY = Math.max(topLeft.y, botRight.y);
  const me = this.state.playerId;
  const anchorGroup = anchor ? _unitSelectionGroup(anchor) : kind;
  return this.state
    .entitiesInterpolated(1)
    .filter(
      (e) =>
        e.owner === me &&
        (anchor ? _unitSelectionGroup(e) === anchorGroup : e.kind === kind) &&
        e.x >= minX && e.x <= maxX &&
        e.y >= minY && e.y <= maxY,
    )
    .sort((a, b) => {
      const da = Math.hypot(a.x - anchorX, a.y - anchorY);
      const db = Math.hypot(b.x - anchorX, b.y - anchorY);
      return da - db || a.id - b.id;
    })
    .slice(0, 12)
    .map((e) => e.id);
}

export function _commitBoxSelection(drag, additive) {
  const a = this._worldAt(Math.min(drag.x0, drag.x1), Math.min(drag.y0, drag.y1));
  const b = this._worldAt(Math.max(drag.x0, drag.x1), Math.max(drag.y0, drag.y1));
  const minX = Math.min(a.x, b.x);
  const maxX = Math.max(a.x, b.x);
  const minY = Math.min(a.y, b.y);
  const maxY = Math.max(a.y, b.y);

  const entities = this.state.entitiesInterpolated(1);
  const me = this.state.playerId;
  const spectator = !!this.state.spectator;

  const units = [];
  const buildings = [];
  for (const e of entities) {
    if (e.shotReveal) continue;
    if (!spectator && e.owner !== me) continue;
    if (spectator && e.owner === 0) continue;
    if (!this._entityIntersectsRect(e, minX, minY, maxX, maxY)) continue;
    if (isUnit(e.kind)) units.push(e.id);
    else if (isBuilding(e.kind)) buildings.push(e.id);
  }

  const picked = units.length > 0
    ? this._closestIdsToPoint(units, drag.x0, drag.y0)
    : buildings;
  if (picked.length === 0) {
    if (!additive) this.state.clearSelection();
    return;
  }
  if (additive) this.state.addToSelection(picked);
  else this.state.setSelection(picked);
}

export function _closestIdsToPoint(ids, screenX, screenY) {
  const anchor = this._worldAt(screenX, screenY);
  return this.state
    .entitiesInterpolated(1)
    .filter((e) => ids.includes(e.id))
    .sort((a, b) => {
      const da = Math.hypot(a.x - anchor.x, a.y - anchor.y);
      const db = Math.hypot(b.x - anchor.x, b.y - anchor.y);
      return da - db || a.id - b.id;
    })
    .slice(0, 12)
    .map((e) => e.id);
}

export function _entityAtWorld(wx, wy, ownPreferred) {
  const entities = this.state.entitiesInterpolated(1);
  const me = this.state.playerId;
  const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;

  let best = null;
  let bestScore = Infinity; // lower is better (distance, with ownership tiebreak)
  for (const e of entities) {
    if (e.shotReveal) continue;
    if (!this._worldPointHitsEntity(e, wx, wy, tileSize)) continue;
    const dx = wx - e.x;
    const dy = wy - e.y;
    const dist = Math.hypot(dx, dy);
    // Bias toward own entities when requested by subtracting a large bonus.
    const ownBonus = ownPreferred && e.owner === me ? OWN_HIT_BONUS : 0;
    const score = dist - ownBonus;
    if (score < bestScore) {
      bestScore = score;
      best = e;
    }
  }
  return best;
}

export function _resourceAtWorld(wx, wy) {
  const entities = this.state.entitiesInterpolated(1);
  const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;

  let best = null;
  let bestDist = Infinity;
  for (const e of entities) {
    if (e.shotReveal) continue;
    if (!isResource(e.kind) || e.remaining === 0) continue;
    if (!this._worldPointHitsEntity(e, wx, wy, tileSize)) continue;
    const dist = Math.hypot(wx - e.x, wy - e.y);
    if (dist < bestDist || (dist === bestDist && e.id < best.id)) {
      bestDist = dist;
      best = e;
    }
  }
  return best;
}

export function _worldPointHitsEntity(e, wx, wy, tileSize) {
  const stat = STATS[e.kind];
  if (isBuilding(e.kind)) {
    const halfW = ((stat && stat.footW ? stat.footW : 1) * tileSize) / 2;
    const halfH = ((stat && stat.footH ? stat.footH : 1) * tileSize) / 2;
    return (
      wx >= e.x - halfW - HIT_PAD_PX &&
      wx <= e.x + halfW + HIT_PAD_PX &&
      wy >= e.y - halfH - HIT_PAD_PX &&
      wy <= e.y + halfH + HIT_PAD_PX
    );
  }
  if (isVehicleBodyKind(e.kind)) return pointHitsOrientedVehicle(e, wx, wy, HIT_PAD_PX);
  const radius = (stat && stat.size ? stat.size : DEFAULT_HIT_RADIUS) + HIT_PAD_PX;
  return Math.hypot(wx - e.x, wy - e.y) <= radius;
}

export function _entityIntersectsRect(e, minX, minY, maxX, maxY) {
  return entityIntersectsRect(
    e,
    minX,
    minY,
    maxX,
    maxY,
    this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE,
  );
}
