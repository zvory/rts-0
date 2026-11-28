// Local prototype asset preparation. The four source atlases are image_gen output.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const root = fileURLToPath(new URL("../client/assets/rigs/rifleman-experimental/", import.meta.url));
const size = 128, columns = 16, anchor = { x: 64, y: 112 };
const atlas = new PNG({ width: columns * size, height: 9 * size });
const registrations = [];
for (const mode of ["run", "shoot"]) for (let group = 0; group < 4; group++) {
  const source = PNG.sync.read(readFileSync(`${root}${mode}-source-${group}.png`));
  for (let row = 0; row < 4; row++) {
    let headingScale;
    const poseCount = mode === "run" ? 5 : 4;
    for (let pose = 0; pose < poseCount; pose++) {
      const x0 = Math.round(pose * source.width / poseCount), y0 = Math.round(row * source.height / 4);
      const width = Math.round((pose + 1) * source.width / poseCount) - x0;
      const height = Math.round((row + 1) * source.height / 4) - y0;
      const index = (x, y) => ((y0 + y) * source.width + x0 + x) * 4;
      const visible = (x, y) => {
        const i = index(x, y), r = source.data[i], g = source.data[i + 1], b = source.data[i + 2];
        const fringe = (r > 180 && g < 100 && b < 110) || (r > 200 && g > 180 && b < 90);
        return source.data[i + 3] >= 64 && !fringe;
      };
      const seen = new Uint8Array(width * height);
      let body = [];
      for (let start = 0; start < seen.length; start++) {
        if (seen[start] || !visible(start % width, Math.floor(start / width))) continue;
        const component = [start]; seen[start] = 1;
        for (let head = 0; head < component.length; head++) {
          const p = component[head], x = p % width, y = Math.floor(p / width);
          for (const [dx, dy] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
            const nx = x + dx, ny = y + dy, n = ny * width + nx;
            if (nx < 0 || ny < 0 || nx >= width || ny >= height || seen[n] || !visible(nx, ny)) continue;
            seen[n] = 1; component.push(n);
          }
        }
        if (component.length > body.length) body = component;
      }
      if (body.length < 1000) throw new Error(`Missing sprite ${group}/${row}/${pose}`);
      const mask = new Uint8Array(width * height);
      let bottom = 0, helmetTop = height;
      for (const p of body) {
        const x = p % width, y = Math.floor(p / width), i = index(x, y);
        mask[p] = 1; bottom = Math.max(bottom, y);
        const rgb = [source.data[i], source.data[i + 1], source.data[i + 2]];
        if (Math.min(...rgb) > 110 && Math.max(...rgb) - Math.min(...rgb) < 55) helmetTop = Math.min(helmetTop, y);
      }
      let sumX = 0, count = 0;
      const helmetEnd = helmetTop + (bottom - helmetTop) * 0.18;
      for (const p of body) {
        const x = p % width, y = Math.floor(p / width), i = index(x, y);
        if (y <= helmetEnd && Math.min(source.data[i], source.data[i + 1], source.data[i + 2]) > 110) {
          sumX += x; count++;
        }
      }
      const centerX = sumX / count;
      if (pose === 0) headingScale = 88 / (bottom - helmetTop);
      const scale = headingScale;
      const offsetX = anchor.x - centerX * scale, offsetY = anchor.y - bottom * scale;
      const direction = group * 4 + row;
      const frame = mode === "shoot" ? 80 + direction * 4 + pose
        : pose === 0 ? direction : 16 + direction * 4 + pose - 1;
      // Premultiplied bilinear sampling keeps transparent edges clean.
      for (let dy = 0; dy < size; dy++) for (let dx = 0; dx < size; dx++) {
        const sx = (dx - offsetX) / scale, sy = (dy - offsetY) / scale;
        const bx = Math.floor(sx), by = Math.floor(sy), fx = sx - bx, fy = sy - by;
        let a = 0; const rgb = [0, 0, 0];
        for (let yy = 0; yy < 2; yy++) for (let xx = 0; xx < 2; xx++) {
          const x = bx + xx, y = by + yy;
          if (x < 0 || y < 0 || x >= width || y >= height || !mask[y * width + x]) continue;
          const i = index(x, y), weight = (xx ? fx : 1 - fx) * (yy ? fy : 1 - fy);
          const alpha = source.data[i + 3] * weight;
          a += alpha;
          for (let c = 0; c < 3; c++) rgb[c] += source.data[i + c] * alpha;
        }
        if (a <= 0) continue;
        const dst = (((Math.floor(frame / columns) * size + dy) * atlas.width) + (frame % columns) * size + dx) * 4;
        for (let c = 0; c < 3; c++) atlas.data[dst + c] = Math.round(rgb[c] / a);
        atlas.data[dst + 3] = Math.round(a);
      }
      registrations.push({ mode, direction, pose, frame, centerX, bottom, scale, offsetX, offsetY });
    }
  }
}
const fixed = new PNG({ width: atlas.width, height: atlas.height });
const paint = new PNG({ width: atlas.width, height: atlas.height });
const smooth = (lo, hi, value) => { const t = Math.max(0, Math.min(1, (value - lo) / (hi - lo))); return t * t * (3 - 2 * t); };
for (let i = 0; i < atlas.data.length; i += 4) {
  const rgb = [atlas.data[i], atlas.data[i + 1], atlas.data[i + 2]].map(v => v / 255);
  const low = Math.min(...rgb), high = Math.max(...rgb);
  const saturation = high > 0 ? (high - low) / high : 0;
  // Extract only light, nearly neutral material. Skin, leather and wood are
  // chromatic; boots, outlines and the barrel remain below the light threshold.
  const localY = Math.floor(i / 4 / atlas.width) % size;
  const mask = localY >= 96 ? 0 : smooth(0.28, 0.52, low) * (1 - smooth(0.12, 0.34, saturation));
  const alpha = atlas.data[i + 3] / 255, paintAlpha = alpha * mask;
  const baseAlpha = paintAlpha < 1 ? (alpha - paintAlpha) / (1 - paintAlpha) : 0;
  fixed.data.set([atlas.data[i], atlas.data[i + 1], atlas.data[i + 2], Math.round(baseAlpha * 255)], i);
  paint.data.set([atlas.data[i], atlas.data[i + 1], atlas.data[i + 2], Math.round(paintAlpha * 255)], i);
}
const outputs = {
  'neutral-atlas.png': PNG.sync.write(atlas), 'fixed-atlas.png': PNG.sync.write(fixed),
  'paint-atlas.png': PNG.sync.write(paint),
  'registration.json': Buffer.from(JSON.stringify({ anchor, size, columns, registrations }, null, 2) + '\n'),
};
for (const [name, bytes] of Object.entries(outputs)) {
  if (process.argv.includes('--check')) {
    if (!readFileSync(root + name).equals(bytes)) throw new Error('Stale experimental asset: ' + name);
  } else writeFileSync(root + name, bytes);
}
console.log(`Prepared ${registrations.length} sprites with a shaded white-material team-paint mask.`);
