import { ESCAPE_TRAIL_DURATION_MS } from "../config.js";
import { gfxCircle, gfxFill, gfxNoFill, gfxStroke, gfxStrokePaths } from "./native_graphics.js";

const PURPLE = 0xa65cff;
const LILAC = 0xe8caff;

/** A brief, complete teleport trace; presentation time never affects the rescue itself. */
export function drawEscapeTrails(g, trails, now) {
  for (const trail of trails) {
    const age = (now - trail.createdAt) / ESCAPE_TRAIL_DURATION_MS;
    if (age < 0 || age >= 1) continue;
    const fade = (1 - age) ** 1.4;
    const dx = trail.toX - trail.fromX;
    const dy = trail.toY - trail.fromY;
    const distance = Math.hypot(dx, dy);
    const nx = -dy / Math.max(1, distance);
    const ny = dx / Math.max(1, distance);
    const steps = Math.max(12, Math.min(160, Math.ceil(distance / 6)));
    const paths = [[], []];
    for (let i = 0; i <= steps; i += 1) {
      const t = i / steps;
      const envelope = Math.sin(t * Math.PI);
      const wave = Math.sin(t * Math.PI * 12 - age * 10) * 3 * envelope;
      for (let lane = 0; lane < 2; lane += 1) {
        const offset = lane ? -wave : wave;
        paths[lane].push([trail.fromX + dx * t + nx * offset, trail.fromY + dy * t + ny * offset - 7]);
      }
    }
    gfxStrokePaths(g, paths, 5, PURPLE, fade * 0.12);
    gfxStrokePaths(g, paths, 1.1, PURPLE, fade * 0.85);
    gfxStrokePaths(g, [paths[0]], 0.5, LILAC, fade * 0.9);
    for (const [x, y] of [[trail.fromX, trail.fromY], [trail.toX, trail.toY]]) {
      for (let arm = 0; arm < 3; arm += 1) {
        const spiral = [];
        for (let i = 0; i <= 24; i += 1) {
          const t = i / 24;
          const angle = arm * Math.PI * 2 / 3 + t * Math.PI * 2 - age * 9;
          const radius = (3 + t * 17) * (1 + age * 0.4);
          spiral.push([x + Math.cos(angle) * radius, y - 7 + Math.sin(angle) * radius * 0.7]);
        }
        gfxStrokePaths(g, [spiral], 3.5, PURPLE, fade * 0.18);
        gfxStrokePaths(g, [spiral], 1, LILAC, fade * 0.85);
      }
      gfxStroke(g, 0);
      for (let i = 0; i < 16; i += 1) {
        const angle = i * 2.39996 + age * 8 + trail.seed;
        const radius = 5 + ((i * 7) % 19) + age * 12;
        gfxFill(g, i % 3 ? PURPLE : LILAC, fade * 0.85);
        gfxCircle(g, x + Math.cos(angle) * radius, y - 7 + Math.sin(angle) * radius * 0.7, i % 3 ? 1.1 : 1.6);
      }
    }
    gfxNoFill(g);
    gfxStroke(g, 0);
  }
}
