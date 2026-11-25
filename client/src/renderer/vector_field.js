import {gfxFillStrokePath} from "./native_graphics.js";

// Six by three tiles, centered on the first click, with a large solid direction arrow.
export function drawVectorField(g, x, y, direction, tileSize = 32, preview = false) {
  const length = Math.hypot(...direction);
  const dx = length > 0.001 ? direction[0] / length : 1;
  const dy = length > 0.001 ? direction[1] / length : 0;
  const transform = ([along, across]) => [x + dx * along - dy * across, y + dy * along + dx * across];
  const l = tileSize * 3, w = tileSize * 1.5;
  gfxFillStrokePath(g, [[-l,-w],[l,-w],[l,w],[-l,w]].map(transform), {
    fill: {color: 0x39cf68, alpha: preview ? 0.10 : 0.07},
    stroke: {width: 2, color: 0x65f58b, alpha: 0.9},
  });
  gfxFillStrokePath(g, [[-l*0.85,-w*0.26],[l*0.25,-w*0.26],[l*0.25,-w*0.8],
    [l*0.87,0],[l*0.25,w*0.8],[l*0.25,w*0.26],[-l*0.85,w*0.26]].map(transform), {
    fill: {color: 0x40e872, alpha: preview ? 0.48 : 0.62},
    stroke: {width: 1.7, color: 0xa1ffb5, alpha: 0.95},
  });
}
