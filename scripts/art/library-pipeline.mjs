#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetDir = path.join(root, "client/assets/rigs/technique-library");
const source = PNG.sync.read(fs.readFileSync(path.join(assetDir, "library-source.png")));
const size = 384;
const output = new PNG({ width: size * 2, height: size });
const clamp = (value) => Math.max(0, Math.min(1, value));
let painted = 0;
let visible = 0;

for (let y = 0; y < size; y++) {
  for (let x = 0; x < size; x++) {
    const sums = [0, 0, 0, 0];
    for (let sy = 0; sy < 4; sy++) {
      for (let sx = 0; sx < 4; sx++) {
        const ix = Math.min(source.width - 1, Math.floor((x + (sx + 0.5) / 4) * source.width / size));
        const iy = Math.min(source.height - 1, Math.floor((y + (sy + 0.5) / 4) * source.height / size));
        const offset = (iy * source.width + ix) * 4;
        let [r, g, b] = source.data.subarray(offset, offset + 3);
        const dominance = Math.min(r, b) - g;
        const alpha = 1 - clamp((dominance - 35) / 50);
        // Remove magenta spill before premultiplied supersampling.
        if (alpha > 0 && alpha < 1) {
          r = Math.max(0, (r - 255 * (1 - alpha)) / alpha);
          b = Math.max(0, (b - 255 * (1 - alpha)) / alpha);
          g = Math.min(255, g / alpha);
        }
        sums[0] += r * alpha;
        sums[1] += g * alpha;
        sums[2] += b * alpha;
        sums[3] += alpha;
      }
    }
    const alpha = sums[3] / 16;
    if (alpha === 0) continue;
    const rgb = sums.slice(0, 3).map((value) => Math.round(value / sums[3]));
    const offset = (y * output.width + x) * 4;
    output.data.set([...rgb, Math.round(alpha * 255)], offset);
    visible++;
    const saturation = (Math.max(...rgb) - Math.min(...rgb)) / Math.max(1, ...rgb);
    // Roof/finial mask; the scrolls, stone and timber stay fixed.
    const roofBottom = size * (0.325 + Math.min(0.105, Math.abs(x / size - 0.5) * 0.42));
    const weight = y < roofBottom
      ? clamp((0.23 - saturation) / 0.07) * clamp((Math.min(...rgb) - 115) / 45)
      : 0;
    if (weight > 0) {
      const maskOffset = (y * output.width + x + size) * 4;
      output.data.set([...rgb, Math.round(alpha * weight * 255)], maskOffset);
      painted++;
    }
  }
}

const outputPath = path.join(assetDir, "library-atlas.png");
const bytes = PNG.sync.write(output);
if (process.argv.includes("--check")) {
  if (!fs.readFileSync(outputPath).equals(bytes)) throw new Error("Technique Library atlas is stale");
} else {
  fs.writeFileSync(outputPath, bytes);
}
console.log(JSON.stringify({ frame: size, columns: 2, visible, painted }));
