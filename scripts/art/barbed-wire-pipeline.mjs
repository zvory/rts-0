#!/usr/bin/env node
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

// Preserve the exact crop, despill and scale used for the selected in-game candidate.
const assetDir = new URL("../../client/assets/rigs/barbed-wire-tangled/", import.meta.url);
const source = PNG.sync.read(fs.readFileSync(new URL("source-magenta.png", assetDir)));
const size = 256;
const output = new PNG({ width: size, height: size });
let minX = source.width, minY = source.height, maxX = 0, maxY = 0;
for (let y = 0; y < source.height; y++) {
  for (let x = 0; x < source.width; x++) {
    const i = (y * source.width + x) * 4;
    const [r, g, b] = source.data.subarray(i, i + 3);
    const distance = Math.max(255 - r, g, 255 - b);
    const t = Math.max(0, Math.min(1, (distance - 28) / 64));
    const alpha = Math.round(255 * t * t * (3 - 2 * t));
    source.data[i + 3] = alpha;
    if (alpha) {
      const spill = Math.max(0, Math.min(r, b) - g);
      source.data[i] = r - spill;
      source.data[i + 2] = b - spill;
    }
    if (alpha > 128) {
      minX = Math.min(minX, x); maxX = Math.max(maxX, x);
      minY = Math.min(minY, y); maxY = Math.max(maxY, y);
    }
  }
}
if (minX > maxX || minY > maxY) throw new Error("Barbed-wire source has no opaque artwork");
const scale = 240 / Math.max(maxX - minX + 1, maxY - minY + 1);
const width = Math.round((maxX - minX + 1) * scale);
const height = Math.round((maxY - minY + 1) * scale);
for (let y = 0; y < height; y++) {
  for (let x = 0; x < width; x++) {
    const sums = [0, 0, 0, 0];
    for (let sy = 0; sy < 4; sy++) {
      for (let sx = 0; sx < 4; sx++) {
        const px = Math.min(maxX, Math.floor(minX + (x + (sx + 0.5) / 4) / scale));
        const py = Math.min(maxY, Math.floor(minY + (y + (sy + 0.5) / 4) / scale));
        const i = (py * source.width + px) * 4;
        const alpha = source.data[i + 3] / 255;
        for (let c = 0; c < 3; c++) sums[c] += source.data[i + c] * alpha;
        sums[3] += alpha;
      }
    }
    const dest = ((Math.floor((size - height) / 2) + y) * size
      + Math.floor((size - width) / 2) + x) * 4;
    for (let c = 0; c < 3; c++) output.data[dest + c] = sums[3] ? Math.round(sums[c] / sums[3]) : 0;
    output.data[dest + 3] = Math.round(sums[3] * 255 / 16);
  }
}
const destination = new URL("barbed-wire.png", assetDir);
fs.writeFileSync(destination, PNG.sync.write(output));
console.log(`Generated ${fileURLToPath(destination)} (${size}×${size})`);
