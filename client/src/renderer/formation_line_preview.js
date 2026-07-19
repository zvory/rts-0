import { COLORS } from "../config.js";

export function drawFormationMovePreview(g, preview) {
  const points = Array.isArray(preview?.points)
    ? preview.points.filter(finitePoint)
    : [];
  if (points.length < 2) return;
  const color = COLORS.selectOwn;
  // A dark outside stroke keeps the freehand path readable over bright terrain,
  // selection ranges, and fog; the saturated inner stroke shows the exact path.
  g.lineStyle(7, 0x071018, 0.72);
  drawLine(g, points);
  g.lineStyle(3, color, 1);
  drawLine(g, points);

  g.lineStyle(2, 0x071018, 0.8);
  g.beginFill(color, 1);
  g.drawCircle(points[0].x, points[0].y, 4);
  g.drawCircle(points[points.length - 1].x, points[points.length - 1].y, 4);
  g.endFill();

  for (const slot of Array.isArray(preview?.slots) ? preview.slots : []) {
    if (!finitePoint(slot)) continue;
    const radius = Math.max(6, Number.isFinite(slot.radius) ? slot.radius : 10);
    g.lineStyle(2, color, 0.78);
    g.beginFill(color, 0.1);
    g.drawCircle(slot.x, slot.y, radius);
    g.endFill();
  }
}

function drawLine(g, points) {
  g.moveTo(points[0].x, points[0].y);
  for (let i = 1; i < points.length; i += 1) g.lineTo(points[i].x, points[i].y);
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}
