import { gfxNoFill, gfxCircle, gfxFillStrokePath, gfxStrokeLine, gfxFill, gfxStroke } from "./native_graphics.js";
import { hash2 } from "./shared.js";

export const MAGIC_ANCHOR_COLOR = 0xc7d07a;
const PARTICLE_COLOR = 0xe7f0a8;
const PARTICLE_SPOKES = 18;

export function drawMagicAnchor(g, object, radius) {
  const coreRadius = Math.max(8, Math.min(18, radius * 0.16));
  gfxStroke(g, 1.8, MAGIC_ANCHOR_COLOR, 0.6);
  gfxFill(g, MAGIC_ANCHOR_COLOR, 0.055);
  gfxCircle(g, object.x, object.y, radius);
  gfxNoFill(g);
  gfxStroke(g, 1.2, MAGIC_ANCHOR_COLOR, 0.28);
  gfxCircle(g, object.x, object.y, radius * 0.66);
  drawMagicAnchorParticles(g, object, radius, coreRadius);
  gfxStroke(g, 2.2, MAGIC_ANCHOR_COLOR, 0.88);
  gfxFill(g, MAGIC_ANCHOR_COLOR, 0.18);
  gfxFillStrokePath(g, [
    [object.x, object.y - coreRadius],
    [object.x + coreRadius * 0.8, object.y],
    [object.x, object.y + coreRadius],
    [object.x - coreRadius * 0.8, object.y],
  ], {
    fill: { color: MAGIC_ANCHOR_COLOR, alpha: 0.18 },
    stroke: { width: 2.2, color: MAGIC_ANCHOR_COLOR, alpha: 0.88 },
  });
  gfxNoFill(g);
  gfxStroke(g, 1.3, 0x11110f, 0.42);
  gfxCircle(g, object.x, object.y, coreRadius * 0.48);
}

function drawMagicAnchorParticles(g, object, radius, coreRadius) {
  const phase = -((object.expiresIn || 0) * 0.09);
  for (let spoke = 0; spoke < PARTICLE_SPOKES; spoke += 1) {
    const angle =
      (spoke / PARTICLE_SPOKES) * Math.PI * 2 +
      (hash2(object.id || 0, spoke) - 0.5) * 0.42;
    for (let lane = 0; lane < 4; lane += 1) {
      const seed = hash2(spoke + 17, lane + (object.id || 0));
      const progress = (phase + seed + lane * 0.23) % 1;
      const inward = progress < 0 ? progress + 1 : progress;
      const distance = coreRadius + (radius - coreRadius) * (1 - inward);
      const density = 1 - distance / radius;
      const tangent = (seed - 0.5) * radius * 0.045 * (1 - density);
      const px = object.x + Math.cos(angle) * distance + Math.cos(angle + Math.PI / 2) * tangent;
      const py = object.y + Math.sin(angle) * distance + Math.sin(angle + Math.PI / 2) * tangent;
      const size = 1.2 + density * 3.2;
      const alpha = 0.16 + density * 0.42;
      gfxFill(g, PARTICLE_COLOR, alpha);
      gfxCircle(g, px, py, size);
      gfxNoFill(g);
      if (density > 0.52) {
        gfxStroke(g, 1.1 + density * 1.2, PARTICLE_COLOR, alpha * 0.55);
        gfxStrokeLine(
          g,
          px + Math.cos(angle) * size * 2.8,
          py + Math.sin(angle) * size * 2.8,
          px - Math.cos(angle) * size * 2.0,
          py - Math.sin(angle) * size * 2.0,
          1.1 + density * 1.2,
          PARTICLE_COLOR,
          alpha * 0.55,
        );
      }
    }
  }
}
