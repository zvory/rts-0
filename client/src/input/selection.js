import { isBuilding, isResource, isUnit, KIND, SETUP } from "../protocol.js";
import {
  groundCoverageForScreenRect,
  pickSelectionProxy,
  proxyIntersectsViewport,
  selectionProxiesInScreenRect,
} from "./selection_projection.js";

export function publishSelectionScene(scene) {
  if (!scene || scene.version !== 1 || !Array.isArray(scene.proxies) || !scene.projection) return false;
  this.selectionScene = scene;
  return true;
}

export function _groundAtScreen(sx, sy) {
  const groundAtScreen = this.selectionScene?.projection?.groundAtScreen;
  if (typeof groundAtScreen !== "function") return null;
  let point;
  try {
    point = groundAtScreen({ x: sx, y: sy });
  } catch {
    return null;
  }
  return finiteClampedGroundPoint(this, point);
}

export function _entityAtScreen(
  screen,
  ownPreferred = false,
  eligible = () => true,
  preference = () => 0,
) {
  const proxy = pickSelectionProxy(this.selectionScene, screen, {
    eligible: (candidate) => eligible(entityForProxy(candidate), candidate),
    preference: (candidate) => {
      const ownPreference = ownPreferred && ownOwner(this.state, candidate.owner) ? 1 : 0;
      return ownPreference + preference(entityForProxy(candidate), candidate);
    },
  });
  return entityForProxy(proxy);
}

export function _resourceAtScreen(screen) {
  return this._entityAtScreen(screen, false, (entity) => isResource(entity?.kind) && entity.remaining !== 0);
}

export function _selectionEntityById(id) {
  const proxy = (this.selectionScene?.proxies || []).find((candidate) => candidate.id === id);
  return entityForProxy(proxy);
}

export function _selectionEntities() {
  return (this.selectionScene?.proxies || []).map(entityForProxy).filter(Boolean);
}

export function _commitClickSelection(p, additive, ctrl) {
  const hit = this._entityAtScreen(p, true, (entity) => (
    entity?.kind !== KIND.SCOUT_PLANE || scoutPlaneInspectable(this.state)
  ), (entity) => (
    ownOwner(this.state, entity?.owner) &&
    isBuilding(entity?.kind) &&
    Number.isFinite(entity?.buildProgress)
      ? 1
      : 0
  ));
  if (!hit) {
    if (!additive) clearSelection(this);
    return;
  }
  if (ctrl && isUnit(hit.kind) && ownOwner(this.state, hit.owner)) {
    const ids = this._closestOwnUnitKindInViewport(hit);
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
  setSelection(this, [hit.id]);
}

export function _ownBuildingsOfKindInViewport(kind) {
  return (this.selectionScene?.proxies || [])
    .filter((proxy) => ownOwner(this.state, proxy.owner) && proxy.kind === kind)
    .filter((proxy) => proxyIntersectsViewport(this.selectionScene, proxy))
    .sort((a, b) => a.id - b.id)
    .map((proxy) => proxy.id);
}

export function _unitSelectionGroup(entity) {
  if (!entity) return "";
  if (entity.kind !== KIND.ANTI_TANK_GUN) return entity.kind;
  return `${entity.kind}:${entity.setupState || SETUP.PACKED}`;
}

export function _closestOwnUnitKindInViewport(anchorOrKind, anchorX, anchorY, explicitAnchor = null) {
  const anchor = explicitAnchor || (typeof anchorOrKind === "object" ? anchorOrKind : null);
  const kind = anchor?.kind || anchorOrKind;
  const anchorGroup = anchor ? _unitSelectionGroup(anchor) : kind;
  const anchorProxy = anchor
    ? (this.selectionScene?.proxies || []).find((proxy) => proxy.id === anchor.id)
    : null;
  const projectedAnchor = this.selectionScene?.projection?.project?.({
    x: anchor?.x ?? anchorX,
    y: anchor?.y ?? anchorY,
    heightPx: anchorProxy?.anchor?.heightPx || 0,
  });
  return (this.selectionScene?.proxies || [])
    .filter((proxy) => ownOwner(this.state, proxy.owner) && (
      anchor ? _unitSelectionGroup(entityForProxy(proxy)) === anchorGroup : proxy.kind === kind
    ))
    .filter((proxy) => proxyIntersectsViewport(this.selectionScene, proxy))
    .map((proxy) => {
      const projected = this.selectionScene.projection.project(proxy.anchor);
      return {
        id: proxy.id,
        distance: Number.isFinite(projectedAnchor?.x) && Number.isFinite(projectedAnchor?.y)
          ? Math.hypot(projected.x - projectedAnchor.x, projected.y - projectedAnchor.y)
          : 0,
      };
    })
    .sort((a, b) => a.distance - b.distance || a.id - b.id)
    .map(({ id }) => id);
}

export function _commitBoxSelection(drag, additive) {
  const proxies = _selectableProxiesInDragRect.call(this, drag);
  const units = proxies.filter((proxy) => isUnit(proxy.kind)).map((proxy) => proxy.id);
  const buildings = proxies.filter((proxy) => isBuilding(proxy.kind)).map((proxy) => proxy.id);
  const picked = units.length > 0 ? units : buildings;
  if (picked.length === 0) {
    if (!additive) clearSelection(this);
    return;
  }
  if (additive) addToSelection(this, picked);
  else setSelection(this, picked);
}

export function _selectableProxiesInDragRect(drag, options = {}) {
  const spectator = !!this.state?.spectator;
  return selectionProxiesInScreenRect(this.selectionScene, drag, {
    anchor: { x: drag.x0, y: drag.y0 },
    eligible: (proxy) => {
      const entity = entityForProxy(proxy);
      if (options.unitsOnly && !isUnit(proxy.kind)) return false;
      if (options.buildingsOnly && !isBuilding(proxy.kind)) return false;
      return selectableEntity(this.state, entity, spectator);
    },
  });
}

export function _selectableEntityIdsInDragRect(drag, options = {}) {
  const ids = _selectableProxiesInDragRect.call(this, drag, options).map((proxy) => proxy.id);
  return options.sortByAnchor === false ? ids.slice().sort((a, b) => a - b) : ids;
}

export function _dragGroundCoverage(drag) {
  return groundCoverageForScreenRect(this.selectionScene, drag);
}

export function _visibleSelectionIds(ids) {
  const presented = new Set((this.selectionScene?.proxies || []).map((proxy) => proxy.id));
  const out = [];
  const seen = new Set();
  for (const id of ids || []) {
    if (!presented.has(id) || seen.has(id)) continue;
    out.push(id);
    seen.add(id);
  }
  return out;
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
  if (!entity || entity.shotReveal || entity.visionOnly) return false;
  if (state?.controlPolicy?.kind === "lab") return state.controlPolicy.canSelectEntity(entity, state);
  if (entity.kind === KIND.SCOUT_PLANE) return scoutPlaneInspectable(state);
  if (!spectator) return ownOwner(state, entity.owner);
  return entity.owner !== 0;
}

function scoutPlaneInspectable(state) {
  return state?.controlPolicy?.kind === "lab" || !!state?.spectator;
}

function finiteClampedGroundPoint(input, point) {
  if (!Number.isFinite(point?.x) || !Number.isFinite(point?.y)) return null;
  let x = point.x;
  let y = point.y;
  const map = input.state?.map;
  if (map) {
    const maxX = Number(map.width) * Number(map.tileSize);
    const maxY = Number(map.height) * Number(map.tileSize);
    if (Number.isFinite(maxX) && maxX > 0) x = Math.max(0, Math.min(maxX - 1, x));
    if (Number.isFinite(maxY) && maxY > 0) y = Math.max(0, Math.min(maxY - 1, y));
  }
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function entityForProxy(proxy) {
  if (!proxy) return null;
  return proxy.interaction || {
    id: proxy.id,
    kind: proxy.kind,
    owner: proxy.owner,
    facing: proxy.facing,
    setupState: proxy.setupState,
    x: proxy.anchor?.x,
    y: proxy.anchor?.y,
  };
}

function closeCommandCardMenu(input) {
  input?.clientIntent?.closeCommandCardMenu?.();
}

function setSelection(input, ids) {
  closeCommandCardMenu(input);
  input.state.setSelection(ids, { entityById: (id) => input._selectionEntityById(id) });
  reconcileLocalPlannedOrders(input);
}

function addToSelection(input, ids) {
  closeCommandCardMenu(input);
  input.state.addToSelection(ids, { entityById: (id) => input._selectionEntityById(id) });
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
