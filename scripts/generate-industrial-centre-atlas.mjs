#!/usr/bin/env node
// Deterministic runtime preparation of the selected ImageGen source.
import fs from 'node:fs';
import { PNG } from 'pngjs';
const root = new URL('../client/assets/rigs/industrial-centre-refinery/', import.meta.url);
const source = PNG.sync.read(fs.readFileSync(new URL('source-magenta.png', root)));
const size = 384;
const atlas = new PNG({ width: size * 4, height: size });
// Remove chroma before area downsampling, so transparent edges cannot bleed magenta.
for (let i = 0; i < source.data.length; i += 4) {
  const [r, g, b] = source.data.subarray(i, i + 3);
  if (r > g + 55 && b > g + 55) source.data[i + 3] = 0;
}
for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
  let weight = 0, alpha = 0, red = 0, green = 0, blue = 0;
  const left = x * source.width / size, right = (x + 1) * source.width / size;
  const top = y * source.height / size, bottom = (y + 1) * source.height / size;
  for (let sy = Math.floor(top); sy < Math.ceil(bottom); sy++) {
    for (let sx = Math.floor(left); sx < Math.ceil(right); sx++) {
      const area = (Math.min(right, sx + 1) - Math.max(left, sx))
        * (Math.min(bottom, sy + 1) - Math.max(top, sy));
      const i = (sy * source.width + sx) * 4;
      const a = source.data[i + 3] / 255 * area;
      weight += area; alpha += a;
      red += source.data[i] * a; green += source.data[i + 1] * a; blue += source.data[i + 2] * a;
    }
  }
  if (!alpha) continue;
  const rgb = [red / alpha, green / alpha, blue / alpha];
  const a = Math.round(alpha / weight * 255);
  const put = (column, color, opacity) => {
    const i = (y * atlas.width + column * size + x) * 4;
    atlas.data.set([...color.map(Math.round), opacity], i);
  };
  put(0, rgb, a);
  // Only light neutral painted surfaces receive team tint; iron, soil and fire stay fixed.
  const spread = Math.max(...rgb) - Math.min(...rgb);
  if (Math.min(...rgb) > 135 && spread < 42) put(1, rgb, a);
  put(2, [0, 0, 0], a);
  // Column four remains transparent: retain the depot rig's emblem contract without an emblem.
}
fs.writeFileSync(new URL('industrial-centre-atlas.png', root), PNG.sync.write(atlas));
console.log('industrial-centre atlas: 1536x384, base / team paint / shadow / empty emblem');
