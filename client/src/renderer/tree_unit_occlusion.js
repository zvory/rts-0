import { isUnit } from "../protocol.js";
import { _tintFor } from "./entities.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";
import { createUnitOutlineFilter, FOREST_UNIT_FILL_ALPHA } from "./unit_outline_filter.js";

const FOREST_OUTLINE_POOL_NAMES = Object.freeze([
  "forestUnitOutlineRigs",
  "forestUnitOutlineRigOverlays",
]);

/**
 * Duplicate the actual presented unit rig into a filtered surface when a canopy occludes it.
 * Enemy admission remains server/fog-owned: this pass only sees the filtered entity list.
 */
export function _drawTreeOccludedUnitOutlines(entities, state, colorByOwner, options = {}) {
  // Outline rig instances are retained while their ordinary unit remains alive. Hide their
  // filtered parents up front, then reactivate only the units still occluded this frame.
  if (this._forestUnitOutlineGroups) {
    for (const entry of this._forestUnitOutlineGroups.values()) entry.group.visible = false;
  }
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || entity.visionOnly) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeOutlineRadius(entity))) continue;
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
  this._recordRenderDiagnostic?.("renderer.doodads.forestUnitOutlines", count);
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

export function unitTreeOutlineRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.5 : 7);
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
