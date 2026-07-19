// Immediate shape helpers backed exclusively by Pixi v8's path/fill/stroke API.
// Arbitrary paths are deliberately explicit: callers provide the complete subpaths
// and paint for one operation, so path boundaries and joins cannot depend on hidden
// v7-style cursor state.

const paintByGraphics = new WeakMap();

function paintFor(graphics) {
  let paint = paintByGraphics.get(graphics);
  if (!paint) {
    paint = { fill: null, stroke: null };
    paintByGraphics.set(graphics, paint);
  }
  return paint;
}

export function gfxFill(graphics, color, alpha = 1) {
  paintFor(graphics).fill = { color, alpha };
  return graphics;
}

export function gfxNoFill(graphics) {
  paintFor(graphics).fill = null;
  return graphics;
}

export function gfxStroke(graphics, width = 0, color = 0, alpha = 1) {
  paintFor(graphics).stroke = width > 0 && alpha > 0 ? { width, color, alpha } : null;
  return graphics;
}

export function gfxReset(graphics) {
  const paint = paintFor(graphics);
  paint.fill = null;
  paint.stroke = null;
  return graphics;
}

function paintShape(graphics) {
  const paint = paintFor(graphics);
  if (paint.fill) graphics.fill(paint.fill);
  if (paint.stroke) graphics.stroke(paint.stroke);
  return graphics;
}

export function gfxRect(graphics, x, y, width, height) {
  graphics.rect(x, y, width, height);
  return paintShape(graphics);
}

export function gfxRoundRect(graphics, x, y, width, height, radius) {
  graphics.roundRect(x, y, width, height, radius);
  return paintShape(graphics);
}

export function gfxCircle(graphics, x, y, radius) {
  graphics.circle(x, y, radius);
  return paintShape(graphics);
}

export function gfxEllipse(graphics, x, y, radiusX, radiusY) {
  graphics.ellipse(x, y, radiusX, radiusY);
  return paintShape(graphics);
}

export function gfxPoly(graphics, points) {
  graphics.poly(points);
  return paintShape(graphics);
}

export function gfxStrokeLine(graphics, x1, y1, x2, y2, width, color, alpha = 1) {
  if (!(width > 0) || !(alpha > 0)) return graphics;
  graphics.moveTo(x1, y1).lineTo(x2, y2).stroke({ width, color, alpha });
  return graphics;
}

// `subpaths` is an array of point arrays: [[[x, y], ...], ...]. All subpaths
// share one stroke operation, matching a v7 path containing multiple moveTo calls.
export function gfxStrokePaths(graphics, subpaths, width, color, alpha = 1) {
  if (!(width > 0) || !(alpha > 0)) return graphics;
  let hasPath = false;
  for (const points of subpaths) {
    if (!Array.isArray(points) || points.length < 2) continue;
    graphics.moveTo(points[0][0], points[0][1]);
    for (let i = 1; i < points.length; i += 1) {
      graphics.lineTo(points[i][0], points[i][1]);
    }
    hasPath = true;
  }
  if (hasPath) graphics.stroke({ width, color, alpha });
  return graphics;
}

export function gfxFillStrokePath(graphics, points, {
  fill = null,
  stroke = null,
  close = true,
} = {}) {
  if (!Array.isArray(points) || points.length < 2) return graphics;
  graphics.moveTo(points[0][0], points[0][1]);
  for (let i = 1; i < points.length; i += 1) {
    graphics.lineTo(points[i][0], points[i][1]);
  }
  if (close && typeof graphics.closePath === "function") graphics.closePath();
  if (fill) graphics.fill(fill);
  if (stroke) graphics.stroke(stroke);
  return graphics;
}

export function gfxFillCurrentPath(graphics, color, alpha = 1) {
  graphics.fill({ color, alpha });
  return graphics;
}
