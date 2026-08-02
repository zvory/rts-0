import { STATS } from "../config.js";
import { isUnit } from "../protocol.js";
import { gfxReset, gfxStrokePaths } from "./native_graphics.js";
import { liveRigKeyForEntity } from "./rigs/live_routing.js";

const FOREST_REVEAL_ALPHA = 0.58;
const FOREST_HATCH_COLOR = 0xffffff;
const FOREST_HATCH_ALPHA = 0.18;
const FOREST_HATCH_WIDTH_PX = 0.9;
const FOREST_HATCH_SPACING_PX = 7;

/**
 * Re-render an already-presented unit above an occluding tree canopy, then add a light crosshatch.
 * Enemy admission remains server/fog-owned: this pass can only inspect the filtered entity list.
 */
export function _drawTreeOccludedUnitReveals(entities, state, colorByOwner, {
  renderContexts = null,
  visualUnitOverrides = null,
  visualFrameStripOverrides = null,
} = {}) {
  if (!this._doodads || !Array.isArray(entities)) return 0;
  let count = 0;
  for (const entity of entities) {
    if (!isUnit(entity?.kind)) continue;
    if (!this._doodads.occludesUnit(entity, unitTreeRevealRadius(entity))) continue;
    const renderContext = renderContexts?.get?.(entity.id);
    if (!renderContext) continue;
    try {
      this._drawUnit(entity, colorByOwner, state, {
        omitShadow: true,
        omitEffects: true,
        unit: "forestUnitReveals",
        overlay: "forestUnitReveals",
        liveRigUnit: "forestUnitRevealRigs",
        liveRigOverlay: "forestUnitRevealRigOverlays",
        renderContext: { ...renderContext },
        visualOverride: visualUnitOverrides?.get?.(entity.id) || null,
        visualFrameStrip: visualFrameStripOverrides?.get?.(liveRigKeyForEntity(entity)) || null,
        alpha: FOREST_REVEAL_ALPHA,
      });
      drawCrosshatch(this._slot("forestUnitHatches", entity.id), entity, renderContext);
    } catch (error) {
      this._recordRenderError?.(`forestUnitReveal:${entity.kind || "unknown"}`, error);
      continue;
    }
    count += 1;
  }
  this._recordRenderDiagnostic?.("renderer.doodads.forestUnitReveals", count);
  return count;
}

export function unitTreeRevealRadius(entity) {
  const width = Number(entity?.visualBounds?.widthPx);
  return Math.max(7, Number.isFinite(width) ? width * 0.5 : 7);
}

function drawCrosshatch(graphics, entity, renderContext) {
  const bounds = unitRevealBounds(entity);
  gfxReset(graphics.clear());
  graphics.position.set(finite(entity.x), finite(entity.y));
  graphics.rotation = finite(renderContext?.facing, finite(entity.facing));
  const paths = bounds.body
    ? rectangularCrosshatch(bounds.length / 2, bounds.width / 2)
    : circularCrosshatch(bounds.radius);
  gfxStrokePaths(
    graphics,
    paths,
    FOREST_HATCH_WIDTH_PX,
    FOREST_HATCH_COLOR,
    FOREST_HATCH_ALPHA,
  );
}

function unitRevealBounds(entity) {
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

function circularCrosshatch(radius) {
  const paths = [];
  for (const direction of [-1, 1]) {
    for (let offset = -radius; offset <= radius; offset += FOREST_HATCH_SPACING_PX) {
      const extent = Math.sqrt(Math.max(0, radius * radius - offset * offset));
      if (direction > 0) paths.push([[offset - extent, -extent], [offset + extent, extent]]);
      else paths.push([[offset - extent, extent], [offset + extent, -extent]]);
    }
  }
  return paths;
}

function rectangularCrosshatch(halfLength, halfWidth) {
  const paths = [];
  for (const direction of [-1, 1]) {
    for (let offset = -halfLength - halfWidth; offset <= halfLength + halfWidth; offset += FOREST_HATCH_SPACING_PX) {
      const intersections = lineRectIntersections(offset, direction, halfLength, halfWidth);
      if (intersections.length >= 2) paths.push([intersections[0], intersections[1]]);
    }
  }
  return paths;
}

function lineRectIntersections(offset, direction, halfLength, halfWidth) {
  const points = [];
  for (const x of [-halfLength, halfLength]) {
    const y = direction * (x - offset);
    if (y >= -halfWidth && y <= halfWidth) points.push([x, y]);
  }
  for (const y of [-halfWidth, halfWidth]) {
    const x = offset + direction * y;
    if (x >= -halfLength && x <= halfLength) points.push([x, y]);
  }
  return dedupePoints(points);
}

function dedupePoints(points) {
  const seen = new Set();
  return points.filter(([x, y]) => {
    const key = `${x.toFixed(4)},${y.toFixed(4)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function finite(value, fallback = 0) {
  return Number.isFinite(Number(value)) ? Number(value) : fallback;
}
