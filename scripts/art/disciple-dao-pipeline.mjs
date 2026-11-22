#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const rig = path.join(root, "client/assets/rigs/disciple-dao");
const check = process.argv.includes("--check");
const read = name => PNG.sync.read(fs.readFileSync(name));
const blank = (width, height) => new PNG({ width, height });
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

function key(image) {
  for (let i = 0; i < image.data.length; i += 4) {
    let [r, g, b] = image.data.subarray(i, i + 3);
    const dominance = Math.min(r, b) - g;
    const alpha = clamp((105 - dominance) / 70, 0, 1);
    // Unmix the magenta matte before premultiplied supersampling.
    if (alpha > 0 && alpha < 1) {
      r = clamp((r - 255 * (1 - alpha)) / alpha, 0, 255);
      b = clamp((b - 255 * (1 - alpha)) / alpha, 0, 255);
      g = clamp(g / alpha, 0, 255);
      r = g = b = (r + g + b) / 3;
    }
    if (Math.min(r, b) - g > 12) { r = Math.min(r, g + 4); b = Math.min(b, g + 4); }
    image.data.set([r, g, b, Math.round(alpha * 255)], i);
  }
  return image;
}

function crop(image, x, y, width, height) {
  const output = blank(width, height);
  for (let row = 0; row < height; row++) {
    image.data.copy(output.data, row * width * 4,
      ((y + row) * image.width + x) * 4, ((y + row) * image.width + x + width) * 4);
  }
  return output;
}

function hairPivot(image, expectedX, expectedY, radius) {
  let sx = 0, sy = 0, n = 0;
  for (let y = Math.max(0, Math.floor(expectedY - radius)); y < Math.min(image.height, expectedY + radius); y++) {
    for (let x = Math.max(0, Math.floor(expectedX - radius)); x < Math.min(image.width, expectedX + radius); x++) {
      const i = (y * image.width + x) * 4;
      if (image.data[i + 3] > 240 && Math.max(...image.data.subarray(i, i + 3)) < 85) {
        sx += x; sy += y; n++;
      }
    }
  }
  if (!n) throw new Error("missing crown pivot");
  return [sx / n, sy / n];
}

function paint(output, source, scale, left, top, width, height) {
  // 4x4 area samples keep thin strings/feathers without transparent-color halos.
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let r = 0, g = 0, b = 0, alpha = 0;
      for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
        const ix = Math.floor((x + (sx + 0.5) / 4) / scale);
        const iy = Math.floor((y + (sy + 0.5) / 4) / scale);
        if (ix < 0 || iy < 0 || ix >= source.width || iy >= source.height) continue;
        const i = (iy * source.width + ix) * 4;
        const a = source.data[i + 3] / 255;
        r += source.data[i] * a; g += source.data[i + 1] * a; b += source.data[i + 2] * a; alpha += a;
      }
      const ox = left + x, oy = top + y;
      if (!alpha || ox < 0 || oy < 0 || ox >= output.width || oy >= output.height) continue;
      output.data.set([Math.round(r / alpha), Math.round(g / alpha), Math.round(b / alpha), Math.round(alpha * 255 / 16)], (oy * output.width + ox) * 4);
    }
  }
}

function frame(source, scale, pivot) {
  const output = blank(256, 256);
  paint(output, source, scale, Math.round(128 - pivot[0] * scale), Math.round(128 - pivot[1] * scale),
    Math.ceil(source.width * scale), Math.ceil(source.height * scale));
  return output;
}

function gridFrames(name, rows, scale) {
  const image = key(read(path.join(rig, "source", name)));
  const frames = [];
  for (let row = 0; row < rows; row++) for (let col = 0; col < 4; col++) {
    const x = Math.round(col * image.width / 4), y = Math.round(row * image.height / rows);
    const cell = crop(image, x, y, Math.round((col + 1) * image.width / 4) - x, Math.round((row + 1) * image.height / rows) - y);
    // The head leads low in the arms-back running poses; the idle crown sits higher.
    const cy = rows === 2 && row === 0 && col > 0 ? cell.height * 0.60 : cell.height * 0.44;
    const pivot = hairPivot(cell, cell.width * 0.5, cy, cell.width * 0.16);
    frames.push(frame(cell, scale, pivot));
  }
  return frames;
}

function write(name, image) {
  const bytes = PNG.sync.write(image);
  if (check) {
    if (!fs.readFileSync(name).equals(bytes)) throw new Error(`stale generated asset: ${path.relative(root, name)}`);
  } else { fs.mkdirSync(path.dirname(name), { recursive: true }); fs.writeFileSync(name, bytes); }
}

function strip(name, frames) {
  const image = blank(frames.length * 256, 256);
  for (let i = 0; i < frames.length; i++) for (let y = 0; y < 256; y++) {
    frames[i].data.copy(image.data, (y * image.width + i * 256) * 4, y * 256 * 4, (y + 1) * 256 * 4);
  }
  write(path.join(rig, "generated", `${name}-runtime-strip.png`), image);
}

const concepts = key(read(path.join(rig, "source/concepts.png")));
const xs = [125, 352, 574, 792, 1014, 1254];
const crowns = [237, 455, 675, 895, 1126];
const lightness = [];
for (let i = 0; i < 4; i++) {
  const cell = crop(concepts, xs[i], 50, xs[i + 1] - xs[i], 205);
  // Row labels occupy the upper-left margin of the first cell, outside its actor.
  if (i === 0) for (let y = 0; y < 110; y++) for (let x = 0; x < 32; x++) cell.data[(y * cell.width + x) * 4 + 3] = 0;
  lightness.push(frame(cell, 0.70, hairPivot(cell, crowns[i] - xs[i], 83, 30)));
}
strip("lightness", [...lightness, ...gridFrames("lightness-attack.png", 1, 0.34)]);
strip("magic", gridFrames("magic-motion.png", 2, 0.37));
const arrowDir = path.join(root, "client/assets/effects/disciple-arrow");
const arrow = key(read(path.join(arrowDir, "arrow-source.png")));
let minX = arrow.width, minY = arrow.height, maxX = 0, maxY = 0;
for (let y = 0; y < arrow.height; y++) for (let x = 0; x < arrow.width; x++) {
  if (arrow.data[(y * arrow.width + x) * 4 + 3] < 40) continue;
  minX = Math.min(minX, x); minY = Math.min(minY, y); maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
}
const source = crop(arrow, minX, minY, maxX - minX + 1, maxY - minY + 1);
const scale = 128 / source.width;
const output = blank(128, Math.ceil(source.height * scale));
paint(output, source, scale, 0, 0, output.width, output.height);
write(path.join(arrowDir, "arrow.png"), output);
console.log(`Disciple strips and arrow ${check ? "are current" : "generated"}.`);
