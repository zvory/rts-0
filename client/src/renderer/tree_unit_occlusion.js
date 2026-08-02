import { STATS } from "../config.js";
import { isUnit } from "../protocol.js";
import {
  gfxCircle,
  gfxNoFill,
  gfxReset,
  gfxRoundRect,
  gfxStroke,
} from "./native_graphics.js";

const FOREST_OUTLINE_COLOR = 0xffffff;
const FOREST_OUTLINE_ALPHA = 0.94;
const FOREST_OUTLINE_WIDTH_PX = 2.25;

/**
 * Draw a white body outline above any tree canopy occluding an already-presented unit.
 * Enemy admission remains server/fog-owned: this pass can only inspect the filtered entity list.
 */
export function _drawTreeOccludedUnitOutlines(entities, {
  renderContexts = null,
} = {}) {
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind)) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeOutlineRadius(entity))) continue;
    try {
      drawUnitOutline(this._slot("forestUnitOutlines", entity.id), entity, renderContexts?.get?.(entity.id));
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

function drawUnitOutline(graphics, entity, renderContext) {
  const bounds = unitOutlineBounds(entity);
  graphics.clear();
  gfxReset(graphics);
  graphics.position.set(finite(entity.x), finite(entity.y));
  graphics.rotation = finite(renderContext?.facing, finite(entity.facing));
  gfxNoFill(graphics);
  gfxStroke(graphics, FOREST_OUTLINE_WIDTH_PX, FOREST_OUTLINE_COLOR, FOREST_OUTLINE_ALPHA);
  if (bounds.body) {
    const radius = Math.min(bounds.length, bounds.width) * 0.28;
    gfxRoundRect(
      graphics,
      -bounds.length / 2,
      -bounds.width / 2,
      bounds.length,
      bounds.width,
      radius,
    );
  } else {
    gfxCircle(graphics, 0, 0, bounds.radius);
  }
}

function unitOutlineBounds(entity) {
  const body = STATS[entity?.kind]?.body;
  if (Number.isFinite(body?.length) && Number.isFinite(body?.width)) {
    return {
      body: true,
      length: Math.max(8, body.length),
      width: Math.max(8, body.width),
    };
  }
  const width = Number(entity?.visualBounds?.widthPx);
  return { body: false, radius: Math.max(7, Number.isFinite(width) ? width * 0.5 : 7) };
}

function finite(value, fallback = 0) {
  return Number.isFinite(Number(value)) ? Number(value) : fallback;
}
