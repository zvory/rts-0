// Deterministic registration only; source art was generated with built-in image_gen.
// Requires the repository's pngjs dependency. No generation API is invoked here.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const root = fileURLToPath(new URL("../client/assets/rigs/rifleman-directional-prototype/", import.meta.url));
const source = PNG.sync.read(readFileSync(`${root}source-atlas.png`));
const size = 128, anchor = { x: 64, y: 112 }, sampleSize = 3, scale = 1 / sampleSize;
const output = new PNG({ width: size * 16, height: size });
const registrations = [];
for (let frame = 0; frame < 16; frame++) {
  const col = frame % 4, row = Math.floor(frame / 4);
  const x0 = Math.round(col * source.width / 4), y0 = Math.round(row * source.height / 4);
  const width = Math.round((col + 1) * source.width / 4) - x0;
  const height = Math.round((row + 1) * source.height / 4) - y0;
  const alphaAt = (x, y) => source.data[((y0 + y) * source.width + x0 + x) * 4 + 3];
  // Remove disconnected alpha specks. They otherwise make a transparent cell's
  // bounding box include its entire width and shift the body when it is trimmed.
  const seen = new Uint8Array(width * height);
  let body = [];
  for (let start = 0; start < seen.length; start++) {
    if (seen[start] || alphaAt(start % width, Math.floor(start / width)) < 64) continue;
    const component = [start];
    seen[start] = 1;
    for (let head = 0; head < component.length; head++) {
      const p = component[head], x = p % width, y = Math.floor(p / width);
      for (const [dx, dy] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
        const nx = x + dx, ny = y + dy, n = ny * width + nx;
        if (nx < 0 || ny < 0 || nx >= width || ny >= height || seen[n] || alphaAt(nx, ny) < 64) continue;
        seen[n] = 1;
        component.push(n);
      }
    }
    if (component.length > body.length) body = component;
  }
  if (body.length < 1000) throw new Error(`Missing body in frame ${frame}`);
  const mask = new Uint8Array(width * height);
  let top = height, bottom = 0;
  for (const p of body) {
    const x = p % width, y = Math.floor(p / width);
    top = Math.min(top, y); bottom = Math.max(bottom, y);
    // Keep antialiased edge pixels adjacent to the solid component.
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
      if (x + dx >= 0 && x + dx < width && y + dy >= 0 && y + dy < height) mask[p + dy * width + dx] = 1;
    }
  }
  // Register the lower boot silhouettes, excluding the rifle and upper body.
  const bootTop = bottom - (bottom - top) * 0.22;
  let sumX = 0, count = 0;
  const soles = [];
  for (let x = 0; x < width; x++) {
    let sole = -1;
    for (let y = Math.ceil(bootTop); y <= bottom; y++) if (mask[y * width + x] && alphaAt(x, y) >= 64) {
      sumX += x; count++; sole = y;
    }
    if (sole >= 0) soles.push(sole);
  }
  const footX = sumX / count;
  // Median lower envelope follows the contact plane instead of the lowest
  // isolated toe pixel; both boots contribute even in diagonal views.
  soles.sort((a, b) => a - b);
  const footY = soles[Math.floor(soles.length / 2)];
  const offsetX = Math.round(anchor.x - footX * scale);
  const offsetY = Math.round(anchor.y - footY * scale);
  registrations.push({ frame, sourceCell: { x: x0, y: y0, width, height }, footX, footY, offsetX, offsetY });
  // Area sampling preserves the generated alpha rather than keying a background.
  for (let y = 0; y < Math.ceil(height * scale); y++) for (let x = 0; x < Math.ceil(width * scale); x++) {
    const dx = x + offsetX, dy = y + offsetY;
    if (dx < 0 || dy < 0 || dx >= size || dy >= size) continue;
    let alpha = 0; const rgb = [0, 0, 0];
    for (let sy = y * sampleSize; sy < Math.min(height, (y + 1) * sampleSize); sy++) for (let sx = x * sampleSize; sx < Math.min(width, (x + 1) * sampleSize); sx++) {
      const index = ((y0 + sy) * source.width + x0 + sx) * 4;
      const a = mask[sy * width + sx] ? source.data[index + 3] : 0;
      alpha += a;
      for (let c = 0; c < 3; c++) rgb[c] += source.data[index + c] * a;
    }
    if (alpha === 0) continue;
    const dst = (dy * output.width + frame * size + dx) * 4;
    for (let c = 0; c < 3; c++) output.data[dst + c] = Math.round(rgb[c] / alpha);
    output.data[dst + 3] = Math.round(alpha / (sampleSize * sampleSize));
  }
}
writeFileSync(`${root}directional-strip.png`, PNG.sync.write(output));
writeFileSync(`${root}registration.json`, `${JSON.stringify({ frameSize: size, anchor, scale, registrations }, null, 2)}\n`);
console.log(`Registered ${registrations.length} directions at (${anchor.x}, ${anchor.y}).`);
