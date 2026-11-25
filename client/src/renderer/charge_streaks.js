import { entityBodyRadius } from "../config.js";
import { gfxFill, gfxNoFill, gfxPoly, gfxStroke } from "./native_graphics.js";

// No retained trail: losing Charge or visibility hides the pooled geometry in this frame.
export function drawChargeStreaks(g, entity, now) {
  const radius = entityBodyRadius(entity) || 18;
  g.position.set(entity.x, entity.y);
  g.rotation = Number.isFinite(entity.facing) ? entity.facing : 0;
  gfxStroke(g, 0);
  for (let lane = 0; lane < 7; lane++) {
    const phase = ((now / 230 + entity.id * 0.173 + lane * 0.381) % 1 + 1) % 1;
    const across = (lane - 3) * radius * 0.29;
    const front = -radius * (0.3 + Math.abs(lane - 3) * 0.12) - phase * 17;
    const length = radius * (2.0 + ((lane * 7) % 5) * 0.24);
    const halfWidth = 0.7 + (lane % 3) * 0.3;
    const alpha = Math.sin(phase * Math.PI) * 0.36;
    // Wide faint edge and a narrow luminous core read as translucent speed lines.
    for (const [width, opacity, color] of [
      [halfWidth * 2.6, alpha * 0.22, 0xb7e3ee],
      [halfWidth, alpha, 0xf1f8ee],
    ]) {
      gfxFill(g, color, opacity);
      gfxPoly(g, [front, across - width, front - length, across,
        front, across + width, front + 6, across]);
    }
  }
  gfxNoFill(g);
}
