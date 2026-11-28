#!/usr/bin/env node
// Align authored team-paint poses, preserve original body/VFX rows, and pack fixed/paint layers.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
import puppeteer from 'puppeteer-core';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const directory = path.join(root, 'client/assets/rigs/cultivator-team-color');
const specs = JSON.parse(fs.readFileSync(path.join(directory, 'specs.json')));
const check = process.argv.includes('--check');
const browser = await puppeteer.launch({ executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
try {
  const page = await browser.newPage();
  for (const [name, file, w, h, count] of specs) {
    const original = PNG.sync.read(fs.readFileSync(path.join(root, 'client/assets/rigs/cultivator-golden/generated', file)));
    const encode = filename => fs.readFileSync(path.join(directory, 'source', filename)).toString('base64');
    const output = await page.evaluate(async ({ animation, idle, original, w, h, count, name }) => {
      const load = async data => { const image = new Image(); image.src = `data:image/png;base64,${data}`; await image.decode(); return image; };
      const [sheet, approved, old] = await Promise.all([load(animation), load(idle), load(original)]);
      const canvas = (width, height) => Object.assign(document.createElement('canvas'), { width, height });
      const bounds = source => {
        const data = source.getContext('2d').getImageData(0, 0, source.width, source.height).data;
        let x0 = source.width, y0 = source.height, x1 = 0, y1 = 0;
        for (let y = 0; y < source.height; y++) for (let x = 0; x < source.width; x++) {
          if (data[(y * source.width + x) * 4 + 3] < 64) continue;
          x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
        }
        if (x1 <= x0 || y1 <= y0) throw Error(`Empty authored pose: ${name}`);
        return { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
      };
      const aligned = canvas(w * count, h), rows = Math.ceil(count / 4);
      const alignedCtx = aligned.getContext('2d');
      for (let frame = 0; frame < count; frame++) {
        const source = frame === 0 ? approved : sheet;
        const sw = frame === 0 ? source.width : sheet.width / 4, sh = frame === 0 ? source.height : sheet.height / rows;
        const cell = canvas(Math.round(sw), Math.round(sh));
        cell.getContext('2d').drawImage(source, frame === 0 ? 0 : frame % 4 * sw, frame === 0 ? 0 : Math.floor(frame / 4) * sh, sw, sh, 0, 0, cell.width, cell.height);
        const oldCell = canvas(w, h); oldCell.getContext('2d').drawImage(old, frame * w, 0, w, h, 0, 0, w, h);
        const from = bounds(cell), to = bounds(oldCell);
        const pose = canvas(w, h); pose.getContext('2d').drawImage(cell, from.x, from.y, from.w, from.h, to.x, to.y, to.w, to.h);
        // Preserve the Novice's calibrated head anchor/radius across its smaller authored attack poses.
        if (name === 'novice') {
          const head = (image) => {
            const cx = frame >= 6 ? 40 : 58, cy = frame >= 6 ? 69 : 72, radius = frame >= 6 ? 17 : 24;
            const d = image.getContext('2d').getImageData(0, 0, w, h).data, points = [];
            for (let y = cy - radius; y <= cy + radius; y++) for (let x = cx - radius; x <= cx + radius; x++) {
              const i = (y * w + x) * 4;
              if (d[i + 3] > 230 && Math.max(d[i], d[i + 1], d[i + 2]) < 105) points.push([x, y]);
            }
            if (!points.length) throw Error(`Missing Novice head in frame ${frame}`);
            const x = points.reduce((n, p) => n + p[0], 0) / points.length, y = points.reduce((n, p) => n + p[1], 0) / points.length;
            return { x, y, radius: Math.sqrt(points.reduce((n, p) => n + (p[0] - x) ** 2 + (p[1] - y) ** 2, 0) / points.length) };
          };
          const target = head(oldCell);
          for (let pass = 0; pass < 3; pass++) {
            const sourceHead = head(pose), scale = target.radius / sourceHead.radius;
            const scratch = canvas(w, h); scratch.getContext('2d').drawImage(pose, target.x - sourceHead.x * scale, target.y - sourceHead.y * scale, w * scale, h * scale);
            pose.getContext('2d').clearRect(0, 0, w, h); pose.getContext('2d').drawImage(scratch, 0, 0);
          }
        }
        alignedCtx.drawImage(pose, frame * w, 0);
      }
      return aligned.toDataURL();
    }, { animation: encode(`${name}-animation.png`), idle: encode(`${name}-idle.png`), original: PNG.sync.write(original).toString('base64'), w, h, count, name });
    const authored = PNG.sync.read(Buffer.from(output.split(',')[1], 'base64'));
    const packed = new PNG({ width: original.width, height: h * 4 });
    PNG.bitblt(original, packed, 0, 0, original.width, h * 2, 0, 0);
    for (let y = 0; y < h; y++) for (let x = 0; x < authored.width; x++) {
      const i = (y * authored.width + x) * 4, r = authored.data[i] / 255, g = authored.data[i + 1] / 255, b = authored.data[i + 2] / 255;
      const alpha = authored.data[i + 3] < 8 ? 0 : authored.data[i + 3] / 255;
      const smooth = (lo, hi, v) => { const t = Math.max(0, Math.min(1, (v - lo) / (hi - lo))); return t * t * (3 - 2 * t); };
      const chroma = Math.max(0, Math.min(r, b) - g);
      const purity = 1 - smooth(.35, .75, Math.abs(r - b) / Math.max(.01, Math.max(r, b) - g));
      const magic = name.startsWith('magic') ? 1 - smooth(.08, .18, g / Math.max(.01, r, b)) : 1;
      const mask = purity * smooth(.025, .09, chroma) * magic;
      const paintAlpha = alpha * mask;
      const baseAlpha = paintAlpha < 1 ? (alpha - paintAlpha) / (1 - paintAlpha) : 0;
      const base = [r - (r - g) * mask, g, b - (b - g) * mask];
      const baseIndex = ((y + h * 2) * packed.width + x) * 4, paintIndex = ((y + h * 3) * packed.width + x) * 4;
      packed.data.set([...base.map(v => Math.round(v * 255)), Math.round(baseAlpha * 255)], baseIndex);
      const light = Math.round(Math.max(r, g, b) * 255);
      packed.data.set([light, light, light, Math.round(paintAlpha * 255)], paintIndex);
    }
    const bytes = PNG.sync.write(packed), destination = path.join(directory, 'generated', file);
    if (check) {
      if (!fs.existsSync(destination) || !fs.readFileSync(destination).equals(bytes)) throw Error(`Stale team-color atlas: ${name}`);
    } else fs.writeFileSync(destination, bytes);
    console.log(`${check ? 'Verified' : 'Packed'} ${name}: ${count} body frames, original golden effects preserved`);
  }
} finally { await browser.close(); }
