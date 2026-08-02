import { isUnit } from "../protocol.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";

/**
 * Draw already-presented units into the forest-outline filter surface when a canopy occludes them.
 * The layer-level filter merges every rig part's alpha on the GPU and emits only the outer edge.
 * Enemy admission remains server/fog-owned: this pass can only inspect the filtered entity list.
 */
export function _drawTreeOccludedUnitOutlines(entities, state, colorByOwner, {
  renderContexts = null,
  visualUnitOverrides = null,
  visualFrameStripOverrides = null,
} = {}) {
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind)) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeOutlineRadius(entity))) continue;
    const renderContext = renderContexts?.get?.(entity.id);
    if (!renderContext) continue;
    try {
      this._drawUnit(entity, colorByOwner, state, {
        omitShadow: true,
        omitEffects: true,
        unit: "forestUnitOutlines",
        overlay: "forestUnitOutlines",
        liveRigUnit: "forestUnitOutlineRigs",
        liveRigOverlay: "forestUnitOutlineRigOverlays",
        renderContext: { ...renderContext },
        visualOverride: visualUnitOverrides?.get?.(entity.id) || null,
        visualFrameStrip: visualFrameStripOverrides?.get?.(liveRigKeyForEntity(entity)) || null,
      });
    } catch (error) {
      this._recordRenderError?.(`forestUnitOutline:${entity.kind || "unknown"}`, error);
      continue;
    }
    count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.doodads.forestUnitOutlines", count);
  return count;
}

export function unitTreeOutlineRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.5 : 7);
}
