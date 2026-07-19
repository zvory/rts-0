// Immediate drawing helpers backed exclusively by Pixi v8's path/fill/stroke API.
// They keep paint state per Graphics instance so existing renderers can issue small,
// independent primitives without allocating shared GraphicsContext objects.

const paintByGraphics = new WeakMap();

function paintFor(graphics) {
  let paint = paintByGraphics.get(graphics);
  if (!paint) {
    paint = { fill: null, stroke: null, cursor: null };
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
  paint.cursor = null;
  return graphics;
}

function paintShape(graphics) {
  const paint = paintFor(graphics);
  if (paint.fill) graphics.fill(paint.fill);
  if (paint.stroke) graphics.stroke(paint.stroke);
  return graphics;
}

export function gfxPaint(graphics) {
  return paintShape(graphics);
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

export function gfxMove(graphics, x, y) {
  paintFor(graphics).cursor = { x, y };
  return graphics;
}

export function gfxLine(graphics, x, y) {
  const paint = paintFor(graphics);
  const start = paint.cursor;
  if (start && paint.stroke) {
    graphics.moveTo(start.x, start.y).lineTo(x, y).stroke(paint.stroke);
  }
  paint.cursor = { x, y };
  return graphics;
}
