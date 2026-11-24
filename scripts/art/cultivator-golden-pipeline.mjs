#!/usr/bin/env node
// Pack generated black-matte VFX below the original, byte-identical character row.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const dir = path.join(root, 'client/assets/rigs/cultivator-golden');
const specs = JSON.parse(fs.readFileSync(path.join(dir, 'source/specs.json')));
const check = process.argv.includes('--check');
fs.mkdirSync(path.join(dir, 'generated'), { recursive: true });
for (const [name, baseFile, w, h] of specs) {
  const base = PNG.sync.read(fs.readFileSync(path.join(root, 'client/assets/rigs', baseFile)));
  const art = PNG.sync.read(fs.readFileSync(path.join(dir, 'source', `${name}-effects.png`)));
  const out = new PNG({ width: base.width, height: h * 2 });
  PNG.bitblt(base, out, 0, 0, base.width, h, 0, 0);
  for (let f = 0; f < 4; f++) for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    let r = 0, g = 0, b = 0;
    // Area samples retain thin glow cores while reducing the generated source.
    for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
      const ax = Math.min(art.width - 1, Math.floor((f % 2 + (x + (sx + .5) / 4) / w) * art.width / 2));
      const ay = Math.min(art.height - 1, Math.floor((Math.floor(f / 2) + (y + (sy + .5) / 4) / h) * art.height / 2));
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
console.log(`Cultivator golden atlases ${check ? 'verified' : 'packed'} (${specs.length}).`);
