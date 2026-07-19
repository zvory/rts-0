import { COLORS } from "../config.js";

export function drawFormationMovePreview(g, preview) {
  const points = Array.isArray(preview?.points)
    ? preview.points.filter(finitePoint)
    : [];
  if (points.length < 2) return;
  const color = COLORS.selectOwn;
  g.lineStyle(4, color, 0.3);
  drawLine(g, points);
  g.lineStyle(2, color, 0.95);
  drawLine(g, points);

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
