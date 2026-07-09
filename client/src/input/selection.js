import { isUnit, isBuilding, isResource, KIND, SETUP } from "../protocol.js";
import { STATS } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS } from "./constants.js";
import {
  hasOrientedSelectionBody,
  pointHitsOrientedVehicle,
  selectionEntityIntersectsRect,
} from "./placement.js";

export function _commitClickSelection(p, additive, ctrl) {
  const world = this._worldAt(p.x, p.y);
  const hit = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ true);
  if (!hit || hit.kind === KIND.SCOUT_PLANE) {
    if (!additive) clearSelection(this);
    return;
  }
  if (ctrl && isUnit(hit.kind) && ownOwner(this.state, hit.owner)) {
    const ids = this._closestOwnUnitKindInViewport(hit.kind, hit.x, hit.y, hit);
    if (additive) addToSelection(this, ids);
    else setSelection(this, ids);
    return;
  }
  if (ctrl && isBuilding(hit.kind) && ownOwner(this.state, hit.owner)) {
    const ids = this._ownBuildingsOfKindInViewport(hit.kind);
    if (additive) addToSelection(this, ids);
    else setSelection(this, ids);
    return;
  }
  if (additive) {
    if (this.state.selection.has(hit.id)) removeFromSelection(this, [hit.id]);
    else addToSelection(this, [hit.id]);
    return;
  }
  else setSelection(this, [hit.id]);
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
  return this.state
    .entitiesInterpolated(1)
    .filter(
      (e) =>
        ownOwner(this.state, e.owner) &&
        e.kind === kind &&
        e.x >= minX && e.x <= maxX &&
        e.y >= minY && e.y <= maxY,
    )
    .sort((a, b) => a.id - b.id)
    .map((e) => e.id);
}

export function _unitSelectionGroup(e) {
  if (!e) return "";
  if (e.kind !== KIND.ANTI_TANK_GUN) return e.kind;
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
  const anchorGroup = anchor ? _unitSelectionGroup(anchor) : kind;
  return this.state
    .entitiesInterpolated(1)
    .filter(
      (e) =>
        ownOwner(this.state, e.owner) &&
        (anchor ? _unitSelectionGroup(e) === anchorGroup : e.kind === kind) &&
        e.x >= minX && e.x <= maxX &&
        e.y >= minY && e.y <= maxY,
    )
    .sort((a, b) => {
      const da = Math.hypot(a.x - anchorX, a.y - anchorY);
      const db = Math.hypot(b.x - anchorX, b.y - anchorY);
      return da - db || a.id - b.id;
    })
    .map((e) => e.id);
}

export function _commitBoxSelection(drag, additive) {
  const entities = _selectableEntitiesInDragRect.call(this, drag);
  const units = [];
  const buildings = [];
  for (const e of entities) {
    if (isUnit(e.kind)) units.push(e.id);
    else if (isBuilding(e.kind)) buildings.push(e.id);
  }

  const picked = units.length > 0
    ? this._closestIdsToPoint(units, drag.x0, drag.y0)
    : buildings;
  if (picked.length === 0) {
    if (!additive) clearSelection(this);
    return;
  }
  if (additive) addToSelection(this, picked);
  else setSelection(this, picked);
}

export function _dragWorldRect(drag) {
  const a = this._worldAt(Math.min(drag.x0, drag.x1), Math.min(drag.y0, drag.y1));
  const b = this._worldAt(Math.max(drag.x0, drag.x1), Math.max(drag.y0, drag.y1));
  return {
    minX: Math.min(a.x, b.x),
    maxX: Math.max(a.x, b.x),
    minY: Math.min(a.y, b.y),
    maxY: Math.max(a.y, b.y),
  };
}

export function _selectableEntitiesInDragRect(drag, options = {}) {
  const { minX, minY, maxX, maxY } = _dragWorldRect.call(this, drag);
  const entities = this.state.entitiesInterpolated(1);
  const spectator = !!this.state.spectator;
  const unitsOnly = !!options.unitsOnly;
  const buildingsOnly = !!options.buildingsOnly;

  return entities.filter((e) => {
    if (e.shotReveal || e.visionOnly) return false;
    if (unitsOnly && !isUnit(e.kind)) return false;
    if (buildingsOnly && !isBuilding(e.kind)) return false;
    if (!selectableEntity(this.state, e, spectator)) return false;
    return this._entityIntersectsRect(e, minX, minY, maxX, maxY);
  });
}

export function _selectableEntityIdsInDragRect(drag, options = {}) {
  const ids = _selectableEntitiesInDragRect.call(this, drag, options).map((entity) => entity.id);
  return options.sortByAnchor === false ? ids : this._closestIdsToPoint(ids, drag.x0, drag.y0);
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
    .map((e) => e.id);
}

export function _entityAtWorld(wx, wy, ownPreferred) {
  const entities = this.state.entitiesInterpolated(1);
  const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;

  let best = null;
  let bestScore = Infinity; // lower is better (distance, with ownership tiebreak)
  for (const e of entities) {
    if (e.shotReveal || e.visionOnly) continue;
    if (!this._worldPointHitsEntity(e, wx, wy, tileSize)) continue;
    const dx = wx - e.x;
    const dy = wy - e.y;
    const dist = Math.hypot(dx, dy);
    // Bias toward own entities when requested by subtracting a large bonus.
    const ownBonus = ownPreferred && ownOwner(this.state, e.owner) ? OWN_HIT_BONUS : 0;
    const score = dist - ownBonus;
    if (score < bestScore) {
      bestScore = score;
      best = e;
    }
  }
  return best;
}

function ownOwner(state, owner) {
  if (state?.controlPolicy?.kind === "lab") {
    return state.controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

export function selectableEntity(state, entity, spectator) {
  if (entity?.kind === KIND.SCOUT_PLANE) return false;
  if (state?.controlPolicy?.kind === "lab") return state.controlPolicy.canSelectEntity(entity, state);
  if (!spectator) return ownOwner(state, entity.owner);
  return entity.owner !== 0;
}

function closeCommandCardMenu(input) {
  input?.clientIntent?.closeCommandCardMenu?.();
}

function setSelection(input, ids) {
  closeCommandCardMenu(input);
  input.state.setSelection(ids);
  reconcileLocalPlannedOrders(input);
}

function addToSelection(input, ids) {
  closeCommandCardMenu(input);
  input.state.addToSelection(ids);
  reconcileLocalPlannedOrders(input);
}

function removeFromSelection(input, ids) {
  closeCommandCardMenu(input);
  input.state.removeFromSelection(ids);
  reconcileLocalPlannedOrders(input);
}

function clearSelection(input) {
  closeCommandCardMenu(input);
  input.state.clearSelection();
  reconcileLocalPlannedOrders(input);
}

function reconcileLocalPlannedOrders(input) {
  input?.clientIntent?.clearPlannedOrdersOutsideSelection?.(input.state?.selection || []);
}

export function _resourceAtWorld(wx, wy) {
  const entities = this.state.entitiesInterpolated(1);
  const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;

  let best = null;
  let bestDist = Infinity;
  for (const e of entities) {
    if (e.shotReveal || e.visionOnly) continue;
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
  if (e.kind === KIND.ANTI_TANK_GUN) return pointHitsOrientedVehicle(e, wx, wy, 0);
  if (hasOrientedSelectionBody(e.kind)) return pointHitsOrientedVehicle(e, wx, wy, HIT_PAD_PX);
  const radius = (stat && stat.size ? stat.size : DEFAULT_HIT_RADIUS) + HIT_PAD_PX;
  return Math.hypot(wx - e.x, wy - e.y) <= radius;
}

export function _entityIntersectsRect(e, minX, minY, maxX, maxY) {
  return selectionEntityIntersectsRect(
    e,
    minX,
    minY,
    maxX,
    maxY,
    this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE,
  );
}
