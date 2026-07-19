import { COLORS } from "../config.js";
import {
  gfxCircle,
  gfxFill,
  gfxNoFill,
  gfxStroke,
  gfxStrokePaths,
} from "./native_graphics.js";

export function drawFormationMovePreview(g, preview) {
  const points = Array.isArray(preview?.points)
    ? preview.points.filter(finitePoint)
    : [];
  if (points.length < 2) return;
  const color = COLORS.selectOwn;
  // A dark outside stroke keeps the freehand path readable over bright terrain,
  // selection ranges, and fog; the saturated inner stroke shows the exact path.
  const path = points.map(({ x, y }) => [x, y]);
  gfxStrokePaths(g, [path], 7, 0x071018, 0.72);
  gfxStrokePaths(g, [path], 3, color, 1);

  gfxStroke(g, 2, 0x071018, 0.8);
  gfxFill(g, color, 1);
  gfxCircle(g, points[0].x, points[0].y, 4);
  gfxCircle(g, points[points.length - 1].x, points[points.length - 1].y, 4);
  gfxNoFill(g);

  for (const slot of Array.isArray(preview?.slots) ? preview.slots : []) {
    if (!finitePoint(slot)) continue;
    const radius = Math.max(6, Number.isFinite(slot.radius) ? slot.radius : 10);
    gfxStroke(g, 2, color, 0.78);
    gfxFill(g, color, 0.1);
    gfxCircle(g, slot.x, slot.y, radius);
    gfxNoFill(g);
  }
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}
