import { gfxCircle, gfxFill, gfxStroke, gfxStrokeLine, gfxStrokePaths } from "./native_graphics.js";

/** A ground-space heading arrow, anchored at the selected tanks' arithmetic mean. */
export function drawTankPointPreview(g, preview) {
  if (!preview) return;
  const { originX: x, originY: y, mouseX, mouseY } = preview;
  if (![x, y, mouseX, mouseY].every(Number.isFinite)) return;
  const color = 0x8fffe0;
  gfxFill(g, color, 0.9);
  gfxStroke(g, 2, 0x102c28);
  gfxCircle(g, x, y, 5);
  const distance = Math.hypot(mouseX - x, mouseY - y);
  if (distance < 1) return;
  const ux = (mouseX - x) / distance;
  const uy = (mouseY - y) / distance;
  const head = Math.min(15, distance * 0.4);
  const wings = [[
    [mouseX - ux * head - uy * head * 0.5, mouseY - uy * head + ux * head * 0.5],
    [mouseX, mouseY],
    [mouseX - ux * head + uy * head * 0.5, mouseY - uy * head - ux * head * 0.5],
  ]];
  for (const [width, strokeColor] of [[5, 0x102c28], [2.5, color]]) {
    gfxStrokeLine(g, x, y, mouseX, mouseY, width, strokeColor, 0.95);
    gfxStrokePaths(g, wings, width, strokeColor, 0.95);
  }
}
