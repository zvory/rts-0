import { isUnit } from "../protocol.js";
import { _tintFor } from "./entities.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";
import { createUnitOutlineFilter, FOREST_UNIT_FILL_ALPHA } from "./unit_outline_filter.js";

const FOREST_OUTLINE_POOL_NAMES = Object.freeze([
  "forestUnitOutlineRigs",
  "forestUnitOutlineRigOverlays",
]);

/**
 * Duplicate the actual presented unit rig when the authoritative concealment mask obscures it.
 * Enemy admission remains server/fog-owned: this pass only sees the filtered entity list.
 */
export function _drawConcealmentTileUnitOutlines(entities, state, colorByOwner, options = {}) {
  // Outline rig instances are retained while their ordinary unit remains alive. Hide their
  // filtered parents up front, then reactivate only the units still occluded this frame.
  if (this._forestUnitOutlineGroups) {
    for (const entry of this._forestUnitOutlineGroups.values()) entry.group.visible = false;
  }
  if (!Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || entity.visionOnly) continue;
    if (!concealmentTilesOccludeUnit(this._map, entity, unitConcealmentOutlineRadius(entity))) continue;
    const renderContext = options.renderContexts?.get?.(entity.id);
    if (!renderContext) continue;
    if (drawActualUnitOutline.call(this, entity, state, colorByOwner, {
      ...options,
      renderContext,
      layerName: "forestUnitOutlines",
      unitPool: "forestUnitOutlineRigs",
      overlayPool: "forestUnitOutlineRigOverlays",
      errorPrefix: "forestUnitOutline",
      teamFill: true,
    })) count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.concealmentTileUnitOutlines", count);
  return count;
}

/** Draw authoritative concealment reveals from their real animated rig, filtered to its outer edge. */
export function _drawConcealmentUnitOutlines(entities, state, colorByOwner, options = {}) {
  if (!Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || !entity.visionOnly) continue;
    if (drawActualUnitOutline.call(this, entity, state, colorByOwner, {
      ...options,
      renderContext: null,
      layerName: "concealmentUnitOutlines",
      unitPool: "concealmentUnitOutlineRigs",
      overlayPool: "concealmentUnitOutlineRigOverlays",
      errorPrefix: "concealmentUnitOutline",
    })) count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.concealmentUnitOutlines", count);
  return count;
}

export function unitConcealmentOutlineRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.5 : 7);
}

/**
 * Match the readability outline to authored concealment rather than decorative canopy pixels.
 * The unit's ground point counts as occupying a tile; tiles immediately south of that point count
 * as foreground occluders when they overlap the unit's presentation footprint.
 */
export function concealmentTilesOccludeUnit(map, entity, radiusPx = 0) {
  const x = Number(entity?.x);
  const y = Number(entity?.y);
  const tileSize = Number(map?.tileSize);
  const radius = Math.max(0, Number(radiusPx) || 0);
  if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(tileSize) || tileSize <= 0) {
    return false;
  }

  const concealmentTiles = map?.concealmentTiles;
  if (!Array.isArray(concealmentTiles) || concealmentTiles.length === 0) return false;
  const concealmentTileKeys = map?._concealmentTileKeys;
  const hasConcealment = concealmentTileKeys instanceof Set
    ? (tileX, tileY) => concealmentTileKeys.has(`${tileX},${tileY}`)
    : (tileX, tileY) => concealmentTiles.some((tile) => tile?.x === tileX && tile?.y === tileY);
  const unitTileX = Math.floor(x / tileSize);
  const unitTileY = Math.floor(y / tileSize);
  const minTileX = Math.floor((x - radius) / tileSize);
  const maxTileX = Math.floor((x + radius) / tileSize);
  const maxTileY = Math.floor((y + radius) / tileSize);

  if (hasConcealment(unitTileX, unitTileY)) return true;
  for (let tileY = unitTileY + 1; tileY <= maxTileY; tileY += 1) {
    for (let tileX = minTileX; tileX <= maxTileX; tileX += 1) {
      if (!hasConcealment(tileX, tileY)) continue;
      const left = tileX * tileSize;
      const right = left + tileSize;
      const top = tileY * tileSize;
      if (x + radius >= left && x - radius <= right && y + radius >= top) return true;
    }
  }
  return false;
}

function drawActualUnitOutline(entity, state, colorByOwner, options) {
  try {
    this._drawUnit(entity, colorByOwner, state, {
      omitShadow: true,
      omitEffects: true,
      unit: options.layerName,
      overlay: options.layerName,
      liveRigUnit: options.unitPool,
      liveRigOverlay: options.overlayPool,
      ...(options.renderContext ? { renderContext: { ...options.renderContext } } : {}),
      visualOverride: options.visualUnitOverrides?.get?.(entity.id) || null,
      visualFrameStrip: options.visualFrameStripOverrides?.get?.(liveRigKeyForEntity(entity)) || null,
    });
    if (options.teamFill) this._attachForestUnitOutline?.(entity, colorByOwner);
    return true;
  } catch (error) {
    this._recordRenderError?.(`${options.errorPrefix}:${entity.kind || "unknown"}`, error);
    return false;
  }
}

/** Group a unit's body and overlay before filtering so only the combined silhouette gets an edge. */
export function _attachForestUnitOutline(entity, colorByOwner) {
  if (entity?.id == null || !this.layers?.forestUnitOutlines) return null;
  const teamColor = _tintFor(entity.owner, colorByOwner);
  let entry = this._forestUnitOutlineGroups?.get?.(entity.id);
  if (!entry || entry.teamColor !== teamColor) {
    if (entry) this._destroyForestUnitOutlineGroup(entity.id);
    const group = new PIXI.Container();
    const filter = createUnitOutlineFilter(PIXI, {
      fillColor: teamColor,
      fillAlpha: FOREST_UNIT_FILL_ALPHA,
    });
    group.filters = [filter];
    this.layers.forestUnitOutlines.addChild(group);
    entry = { group, filter, teamColor };
    this._forestUnitOutlineGroups.set(entity.id, entry);
  }
  entry.group.visible = true;
  entry.group.zIndex = Number.isFinite(entity.y) ? entity.y : 0;
  for (const poolName of FOREST_OUTLINE_POOL_NAMES) {
    const container = this._liveRigPools?.[poolName]?.get?.(entity.id)?.container;
    if (!container || container.parent === entry.group) continue;
    container.parent?.removeChild?.(container);
    entry.group.addChild(container);
  }
  return entry;
}

export function _destroyForestUnitOutlineGroup(id) {
  const entry = this._forestUnitOutlineGroups?.get?.(id);
  if (!entry) return false;
  entry.group.filters = null;
  entry.filter.destroy?.();
  entry.group.parent?.removeChild?.(entry.group);
  entry.group.destroy?.();
  this._forestUnitOutlineGroups.delete(id);
  return true;
}
