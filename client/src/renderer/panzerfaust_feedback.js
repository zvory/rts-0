import { gfxNoFill, gfxCircle, gfxPoly, gfxStrokeLine, gfxStrokePaths, gfxFill, gfxStroke } from "./native_graphics.js";
import {
  clamp01,
  drawFreeRotatedRect,
  hash2,
  rendererVisualNow,
  smoothstep01,
} from "./shared.js";

export function _drawPanzerfaustShots(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.livePanzerfaustShots !== "function") return;
  const now = rendererVisualNow(this);
  const shots = state.livePanzerfaustShots(now);
  if (!shots.length) return;

  for (const shot of shots) {
    const duration = Math.max(1, shot.durationMs || 1);
    const age = now - shot.createdAt;
    const t = clamp01(age / duration);
    const dx = shot.toX - shot.fromX;
    const dy = shot.toY - shot.fromY;
    const len = Math.hypot(dx, dy);
    const angle = len > 0.001 ? Math.atan2(dy, dx) : 0;
    const ux = Math.cos(angle);
    const uy = Math.sin(angle);
    const x = shot.fromX + dx * t;
    const y = shot.fromY + dy * t;
    const travelFade = 1 - smoothstep01(Math.max(0, t - 0.78) / 0.22);
    const launchFade = 1 - clamp01(age / 180);

    if (launchFade > 0) {
      gfxStroke(g, 0, 0x000000, 0);
      gfxFill(g, 0xfff1a8, 0.86 * launchFade);
      gfxPoly(g, [
        shot.fromX + ux * 3 - uy * 3.2,
        shot.fromY + uy * 3 + ux * 3.2,
        shot.fromX + ux * 20,
        shot.fromY + uy * 20,
        shot.fromX + ux * 4 + uy * 5.2,
        shot.fromY + uy * 4 - ux * 5.2,
      ]);
      gfxNoFill(g);
      gfxFill(g, 0x6f5c45, 0.22 * launchFade);
      gfxCircle(g, shot.fromX - ux * 2, shot.fromY - uy * 2, 7);
      gfxNoFill(g);
    }

    if (age <= duration + 80) {
      const tail = Math.min(34, Math.max(14, len * 0.22));
      gfxStrokeLine(g, x - ux * tail, y - uy * tail, x, y,
        3.2, 0x1d1812, 0.56 * travelFade);
      gfxStrokeLine(g, x - ux * tail * 0.72, y - uy * tail * 0.72, x, y,
        1.7, 0xffd65a, 0.86 * travelFade);
      gfxStroke(g, 0, 0x000000, 0);
      gfxFill(g, 0x19130d, 0.98 * travelFade);
      drawFreeRotatedRect(g, x, y, 9.5, 3.2, angle);
      gfxNoFill(g);
      gfxFill(g, 0xd8d0b0, 0.72 * travelFade);
      drawFreeRotatedRect(g, x + ux * 3.2, y + uy * 3.2, 3.2, 2.2, angle);
      gfxNoFill(g);
    }
  }
}

export function _drawPanzerfaustImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.livePanzerfaustImpacts !== "function") return;
  const now = rendererVisualNow(this);
  const impacts = state.livePanzerfaustImpacts(now);
  if (!impacts.length) return;

  for (const impact of impacts) {
    const age = now - impact.createdAt;
    const t = clamp01(age / 720);
    const flashFade = 1 - smoothstep01(Math.max(0, t - 0.18) / 0.32);
    const dustFade = 1 - smoothstep01(Math.max(0, t - 0.32) / 0.68);
    const radius = 11 + t * 12;
    drawJaggedRing(g, impact.x, impact.y, radius * 0.72, 10, impact.seed + 3, 0.72, 1.18,
      3, 0xfff2d0, 0.92 * flashFade);
    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0xffb22e, 0.26 * flashFade);
    drawJaggedBlob(g, impact.x, impact.y, radius * 1.25, 13, impact.seed + 11, 0.62, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0x5f5141, 0.28 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, radius * 1.85, 17, impact.seed + 23, 0.58, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0x201912, 0.18 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, radius * 0.9, 9, impact.seed + 31, 0.7, 1.0);
    gfxNoFill(g);
  }
}

function drawJaggedBlob(g, cx, cy, radius, points, seed, minScale, maxScale) {
  const poly = [];
  for (let i = 0; i < points; i += 1) {
    const a = (i / points) * Math.PI * 2;
    const n = hash2(seed + i * 17, seed - i * 31);
    const r = radius * (minScale + (maxScale - minScale) * n);
    poly.push(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  gfxPoly(g, poly);
}

function drawJaggedRing(g, cx, cy, radius, points, seed, minScale, maxScale, width, color, alpha) {
  const path = [];
  for (let i = 0; i <= points; i += 1) {
    const j = i % points;
    const a = (j / points) * Math.PI * 2;
    const n = hash2(seed + j * 19, seed + j * 7);
    const r = radius * (minScale + (maxScale - minScale) * n);
    const x = cx + Math.cos(a) * r;
    const y = cy + Math.sin(a) * r;
    path.push([x, y]);
  }
  gfxStrokePaths(g, [path], width, color, alpha);
}
