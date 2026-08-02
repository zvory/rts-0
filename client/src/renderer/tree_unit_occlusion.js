import { STATS } from "../config.js";
import { isUnit } from "../protocol.js";
import {
  gfxCircle,
  gfxNoFill,
  gfxPoly,
  gfxRoundRect,
  gfxStroke,
  gfxStrokeLine,
} from "./native_graphics.js";

const UNIT_OUTLINE_COLOR = 0xffffff;
const UNIT_OUTLINE_ALPHA = 0.96;
const UNIT_OUTLINE_WIDTH_PX = 2.25;

/**
 * Draw a stable white body outline above a canopy that occludes an already-presented unit.
 * Enemy admission remains server/fog-owned: this pass can only inspect the filtered entity list.
 */
export function _drawTreeOccludedUnitOutlines(entities) {
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || entity.visionOnly) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeOutlineRadius(entity))) continue;
    try {
      drawUnitOutline(this._slot("forestUnitOutlines", entity.id), entity);
    } catch (error) {
      this._recordRenderError?.(`forestUnitOutline:${entity.kind || "unknown"}`, error);
      continue;
    }
    count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.doodads.forestUnitOutlines", count);
  return count;
}

/** Draw every authoritative stealth reveal as outline-only intel above fog and canopies. */
export function _drawStealthUnitOutlines(entities) {
  if (!Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind) || !entity.visionOnly) continue;
    try {
      drawUnitOutline(this._slot("stealthUnitOutlines", entity.id), entity);
    } catch (error) {
      this._recordRenderError?.(`stealthUnitOutline:${entity.kind || "unknown"}`, error);
      continue;
    }
    count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.stealthUnitOutlines", count);
  return count;
}

export function unitTreeOutlineRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.5 : 7);
}

function drawUnitOutline(graphics, entity) {
  const bounds = unitOutlineBounds(entity);
  graphics.position.set(finite(entity.x), finite(entity.y));
  graphics.rotation = finite(entity.facing);
  gfxNoFill(graphics);
  gfxStroke(graphics, UNIT_OUTLINE_WIDTH_PX, UNIT_OUTLINE_COLOR, UNIT_OUTLINE_ALPHA);
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
    drawInfantryOutline(graphics, bounds.radius);
  }
}

function drawInfantryOutline(graphics, radius) {
  gfxPoly(graphics, [
    radius * 0.7, 0,
    radius * 0.2, -radius * 0.6,
    -radius * 0.58, -radius * 0.46,
    -radius * 0.76, 0,
    -radius * 0.58, radius * 0.46,
    radius * 0.2, radius * 0.6,
  ]);
  gfxCircle(graphics, radius * 0.72, 0, radius * 0.32);
  gfxStrokeLine(
    graphics,
    -radius * 0.18,
    radius * 0.18,
    radius * 1.82,
    -radius * 0.18,
    UNIT_OUTLINE_WIDTH_PX,
    UNIT_OUTLINE_COLOR,
    UNIT_OUTLINE_ALPHA,
  );
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
