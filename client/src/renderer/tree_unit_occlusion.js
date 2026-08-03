import { isUnit } from "../protocol.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";

/**
 * Duplicate the actual presented unit rig into a filtered surface when a canopy occludes it.
 * Enemy admission remains server/fog-owned: this pass only sees the filtered entity list.
 */
export function _drawTreeOccludedUnitOutlines(entities, state, colorByOwner, options = {}) {
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
    })) count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.doodads.forestUnitOutlines", count);
  return count;
}

/** Draw authoritative stealth reveals from their real animated rig, filtered to its outer edge. */
export function _drawStealthUnitOutlines(entities, state, colorByOwner, options = {}) {
  if (!Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || !entity.visionOnly) continue;
    if (drawActualUnitOutline.call(this, entity, state, colorByOwner, {
      ...options,
      renderContext: null,
      layerName: "stealthUnitOutlines",
      unitPool: "stealthUnitOutlineRigs",
      overlayPool: "stealthUnitOutlineRigOverlays",
      errorPrefix: "stealthUnitOutline",
    })) count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.stealthUnitOutlines", count);
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
    return true;
  } catch (error) {
    this._recordRenderError?.(`${options.errorPrefix}:${entity.kind || "unknown"}`, error);
    return false;
  }
}
