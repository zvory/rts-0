#!/usr/bin/env node
// Preserve original body cells, optionally append authored attack poses, then pack VFX.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dir = path.join(root, 'client/assets/rigs/cultivator-golden');
const specs = JSON.parse(fs.readFileSync(path.join(dir, 'source/specs.json')));
const check = process.argv.includes('--check');
fs.mkdirSync(path.join(dir, 'generated'), { recursive: true });
for (const [name, baseFile, w, h, , options = {}] of specs) {
  const base = PNG.sync.read(fs.readFileSync(path.join(root, 'client/assets/rigs', baseFile)));
  const bodyFrames = options.attackBody ? loadAttackBodies(base, w, h, options) : [];
  if (process.argv.includes('--prepare-reference')) {
    if (bodyFrames.length) {
      const ref = new PNG({ width: w * 3, height: h * 2 });
      bodyFrames.forEach((frame, i) => PNG.bitblt(frame, ref, 0, 0, w, h, i % 3 * w, Math.floor(i / 3) * h));
      fs.writeFileSync(path.join(dir, 'source', `${name}-attack-reference-v2.png`), PNG.sync.write(ref));
    }
    continue;
  }
  const art = PNG.sync.read(fs.readFileSync(path.join(dir, 'source', `${name}-effects.png`)));
  const out = new PNG({ width: base.width + bodyFrames.length * w, height: h * 2 });
  PNG.bitblt(base, out, 0, 0, base.width, h, 0, 0);
  bodyFrames.forEach((frame, i) => PNG.bitblt(frame, out, 0, 0, w, h, base.width + i * w, 0));
  const count = options.effectFrames || 4, columns = options.effectColumns || 2;
  for (let f = 0; f < count; f++) for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    let r = 0, g = 0, b = 0;
    // Area samples retain thin glow cores while reducing the generated source.
    for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
      const ax = Math.min(art.width - 1, Math.floor((f % columns + (x + (sx + .5) / 4) / w) * art.width / columns));
      const ay = Math.min(art.height - 1, Math.floor((Math.floor(f / columns) + (y + (sy + .5) / 4) / h) * art.height / Math.ceil(count / columns)));
      const i = (ay * art.width + ax) * 4;
      r += art.data[i]; g += art.data[i + 1]; b += art.data[i + 2];
    }
    r /= 16; g /= 16; b /= 16;
    // Black matte -> straight alpha. This preserves the luminous RGB against
    // black without carrying an opaque black rectangle into the game.
    const a = Math.max(r, g, b);
    if (a < 2 || x === 0 || y === 0 || x === w - 1 || y === h - 1) continue;
    const i = ((y + h) * out.width + f * w + x) * 4;
    out.data.set([Math.round(r * 255 / a), Math.round(g * 255 / a), Math.round(b * 255 / a), Math.round(a)], i);
  }
  const dest = path.join(dir, 'generated', path.basename(baseFile));
  const bytes = PNG.sync.write(out);
  if (check) {
    if (!fs.existsSync(dest) || !fs.readFileSync(dest).equals(bytes)) throw Error(`Stale golden atlas: ${name}`);
  } else fs.writeFileSync(dest, bytes);
}
console.log(`Cultivator golden atlases ${process.argv.includes('--prepare-reference') ? 'references prepared' : check ? 'verified' : 'packed'} (${specs.length}).`);

// Imagegen uses a magenta matte for grayscale character poses, preserving black hair.
function loadAttackBodies(base, w, h, options) {
  const source = PNG.sync.read(fs.readFileSync(path.join(dir, 'source', options.attackBody)));
  const frames = [];
  const target = hairPivot(base, 5 * w, w, h);
  for (let f = 0; f < options.bodyFrames; f++) {
    const frame = new PNG({ width: w, height: h });
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      let r = 0, g = 0, b = 0, alpha = 0;
      for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
        const ax = Math.min(source.width - 1, Math.floor((f % 3 + (x + (sx + .5) / 4) / w) * source.width / 3));
        const ay = Math.min(source.height - 1, Math.floor((Math.floor(f / 3) + (y + (sy + .5) / 4) / h) * source.height / 2));
        const i = (ay * source.width + ax) * 4;
        const [sr, sg, sb] = source.data.subarray(i, i + 3);
        const a = Math.max(0, Math.min(1, (100 - (Math.min(sr, sb) - sg)) / 70));
        if (!a) continue;
        r += Math.max(0, Math.min(255 * a, sr - 255 * (1 - a)));
        g += Math.max(0, Math.min(255 * a, sg));
        b += Math.max(0, Math.min(255 * a, sb - 255 * (1 - a)));
        alpha += a;
      }
      if (alpha) frame.data.set([Math.round(r / alpha), Math.round(g / alpha), Math.round(b / alpha), Math.round(alpha / 16 * 255)], (y * w + x) * 4);
    }
    const pivot = hairPivot(frame, 0, w, h);
    const dx = Math.round(target[0] - pivot[0]), dy = Math.round(target[1] - pivot[1]);
    if (Math.abs(dx) > 25 || Math.abs(dy) > 25) throw Error(`Novice pose ${f} drifted from its torso anchor`);
    const aligned = new PNG({ width: w, height: h });
    for (let y = 1; y < h - 1; y++) for (let x = 1; x < w - 1; x++) {
      const ix = x - dx, iy = y - dy;
      if (ix < 0 || iy < 0 || ix >= w || iy >= h) continue;
      const i = (iy * w + ix) * 4;
      aligned.data.set(frame.data.subarray(i, i + 4), (y * w + x) * 4);
    }
    frames.push(aligned);
  }
  return frames;
}

function hairPivot(image, left, w, h) {
  let sx = 0, sy = 0, count = 0;
  for (let y = Math.floor(h * .25); y < h * .52; y++) for (let x = Math.floor(w * .12); x < w * .39; x++) {
    const i = (y * image.width + left + x) * 4;
    if (image.data[i + 3] > 230 && Math.max(...image.data.subarray(i, i + 3)) < 85) { sx += x; sy += y; count++; }
  }
  if (!count) throw Error('Novice attack pose is missing its head anchor');
  return [sx / count, sy / count];
}
