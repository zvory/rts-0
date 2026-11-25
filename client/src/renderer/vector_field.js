import {VECTOR_FIELD_ADEPT_SIZE_TILES, VECTOR_FIELD_MASTER_SIZE_TILES,
  VECTOR_FIELD_SPEED_PX_PER_TICK, TICK_HZ} from "../config.js";
import {gfxFillStrokePath, gfxStrokeLine, gfxStroke, gfxFill, gfxCircle, gfxNoFill} from "./native_graphics.js";
import {hash2} from "./shared.js";

// Shared geometry and flow for the staged preview and the authoritative field.
export function drawVectorField(g, x, y, direction, tileSize = 32, options = {}) {
  const {preview = false, sizeTiles = VECTOR_FIELD_ADEPT_SIZE_TILES,
    now = 0, particles = false, seed = 0} = options;
  const size = sizeTiles === VECTOR_FIELD_MASTER_SIZE_TILES ? sizeTiles : VECTOR_FIELD_ADEPT_SIZE_TILES;
  const length = Math.hypot(...direction);
  const dx = length > 0.001 ? direction[0] / length : 1;
  const dy = length > 0.001 ? direction[1] / length : 0;
  const transform = ([along, across]) => [x + dx * along - dy * across, y + dy * along + dx * across];
  const half = tileSize * size / 2;
  gfxFillStrokePath(g, [[-half,-half],[half,-half],[half,half],[-half,half]].map(transform), {
    fill: {color: 0x39cf68, alpha: preview ? 0.10 : 0.07},
    stroke: {width: 2, color: 0x65f58b, alpha: 0.9},
  });
  gfxFillStrokePath(g, [[-half*0.85,-half*0.26],[half*0.25,-half*0.26],[half*0.25,-half*0.8],
    [half*0.87,0],[half*0.25,half*0.8],[half*0.25,half*0.26],[-half*0.85,half*0.26]].map(transform), {
    fill: {color: 0x40e872, alpha: preview ? 0.24 : 0.32},
    stroke: {width: 1.7, color: 0xa1ffb5, alpha: 0.95},
  });
  if (!particles) return;
  const span = half * 2;
  const travel = now * VECTOR_FIELD_SPEED_PX_PER_TICK * TICK_HZ / 1000;
  for (let i = 0; i < size * size * 2; i++) {
    const phase = ((travel + hash2(seed + 1, i) * span) % span + span) % span;
    const across = (hash2(seed + 73, i + 11) - 0.5) * (span - 6);
    const alpha = Math.min(1, phase / 12, (span - phase) / 12) * 0.85;
    const head = transform([phase - half, across]);
    const tail = transform([Math.max(-half, phase - half - 7), across]);
    gfxStrokeLine(g, ...tail, ...head, 1.2, 0x86ffac, alpha * 0.6);
    gfxStroke(g, 0);
    gfxFill(g, 0xa4ffb9, alpha);
    gfxCircle(g, ...head, 1.6);
    gfxNoFill(g);
  }
}

export function drawVectorFieldPreview(g, preview, tileSize, now) {
  const {x,y} = preview.vectorCenter;
  drawVectorField(g,x,y,preview.vectorDirection,tileSize,
    {preview:true,sizeTiles:preview.vectorFieldSize,now,particles:true});
}
