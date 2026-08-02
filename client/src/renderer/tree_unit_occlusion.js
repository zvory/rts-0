import { isUnit } from "../protocol.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";

const ALLIED_CANOPY_REVEAL_ALPHA = 0.28;

/**
 * Re-render the actual friendly unit body at low alpha above a canopy that occludes it.
 * This is presentation-only and never admits an enemy or a fog-filtered entity.
 */
export function _drawTreeOccludedAllies(entities, state, colorByOwner, {
  renderContexts = null,
  visualUnitOverrides = null,
  visualFrameStripOverrides = null,
} = {}) {
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || !isFriendly(entity, state)) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeRevealRadius(entity))) continue;
    const renderContext = renderContexts?.get?.(entity.id);
    if (!renderContext) continue;
    try {
      this._drawUnit(entity, colorByOwner, state, {
        omitShadow: true,
        omitEffects: true,
        unit: "alliedTreeReveals",
        overlay: "alliedTreeReveals",
        liveRigUnit: "alliedTreeRevealRigs",
        liveRigOverlay: "alliedTreeRevealRigOverlays",
        renderContext: { ...renderContext },
        visualOverride: visualUnitOverrides?.get?.(entity.id) || null,
        visualFrameStrip: visualFrameStripOverrides?.get?.(liveRigKeyForEntity(entity)) || null,
        alpha: ALLIED_CANOPY_REVEAL_ALPHA,
      });
    } catch (error) {
      this._recordRenderError?.(`alliedTreeReveal:${entity.kind || "unknown"}`, error);
      continue;
    }
    count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.doodads.alliedTreeReveals", count);
  return count;
}

export function unitTreeRevealRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.36 : 7);
}

function isFriendly(entity, state) {
  const owner = Number(entity?.owner);
  if (!Number.isInteger(owner) || owner <= 0) return false;
  return state?.isOwnOwner?.(owner) === true || state?.isAllyOwner?.(owner) === true;
}
