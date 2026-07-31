import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/machine_gunner_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetDir = path.join(repoRoot, "client/assets/rigs/machine-gunner-pass-01");
const generatedDir = path.join(assetDir, "generated");
const runtime = readPng(path.join(assetDir, "machine-gunner-pass-01-strip.png"));

assert.deepEqual([runtime.width, runtime.height], [1920, 128]);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameCount, 15);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameWidth, 128);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameHeight, 128);
assert.equal(
  MACHINE_GUNNER_PNG_FRAME_STRIP.worldScale,
  MACHINE_GUNNER_PNG_FRAME_STRIP.movementWorldScale,
);
const expectedFrameBounds = [
  { x: 6, y: 23, width: 115, height: 81 },
  { x: 6, y: 22, width: 116, height: 84 },
  { x: 6, y: 23, width: 116, height: 81 },
  { x: 6, y: 22, width: 115, height: 83 },
  { x: 7, y: 24, width: 113, height: 80 },
  { x: 7, y: 23, width: 113, height: 82 },
  { x: 6, y: 23, width: 115, height: 81 },
  { x: 25, y: 24, width: 78, height: 79 },
  { x: 29, y: 17, width: 70, height: 93 },
  { x: 38, y: 6, width: 52, height: 115 },
  { x: 38, y: 6, width: 52, height: 116 },
  { x: 39, y: 6, width: 50, height: 115 },
  { x: 39, y: 6, width: 49, height: 116 },
  { x: 41, y: 8, width: 46, height: 111 },
  { x: 39, y: 6, width: 49, height: 116 },
];
assert.deepEqual(MACHINE_GUNNER_PNG_FRAME_STRIP.iconVisibleBounds, {
  x: expectedFrameBounds[0].x,
  y: expectedFrameBounds[0].y,
  w: expectedFrameBounds[0].width,
  h: expectedFrameBounds[0].height,
});
assert.equal(
  MACHINE_GUNNER_PNG_FRAME_STRIP.iconVisibleBounds.h
    * MACHINE_GUNNER_PNG_FRAME_STRIP.worldScale
    <= RIFLEMAN_PNG_FRAME_STRIP.iconVisibleBounds.h
      * RIFLEMAN_PNG_FRAME_STRIP.worldScale * 1.35,
  true,
  "packed MG is disproportionately taller than Rifleman",
);

for (const [pose, columns] of [["carry", 6], ["deploy", 6], ["fire", 3]]) {
  const source = readPng(path.join(generatedDir, `machine-gunner-white-${pose}-source.png`));
  const alpha = readPng(path.join(generatedDir, `machine-gunner-white-${pose}-alpha.png`));
  assert.deepEqual([alpha.width, alpha.height], [source.width, source.height]);
  for (let frame = 0; frame < columns; frame += 1) {
    const left = Math.round(frame * alpha.width / columns);
    const right = Math.round((frame + 1) * alpha.width / columns);
    alphaBounds(alpha, left, 0, right - left, alpha.height);
  }

  let magentaBorderPixels = 0;
  let borderPixels = 0;
  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      if (x >= 8 && x < source.width - 8 && y >= 8 && y < source.height - 8) continue;
      const offset = (y * source.width + x) * 4;
      borderPixels += 1;
      if (source.data[offset] > 220 && source.data[offset + 1] < 50 && source.data[offset + 2] > 190) {
        magentaBorderPixels += 1;
      }
    }
  }
  assert.equal(magentaBorderPixels / borderPixels > 0.98, true, `${pose} source lacks a clean magenta border`);
}

for (let frame = 0; frame < 15; frame += 1) {
  const bounds = alphaBounds(runtime, frame * 128, 0, 128, 128);
  assert.deepEqual(bounds, expectedFrameBounds[frame], `frame ${frame} packing changed`);
  assert.equal(bounds.x >= 5 && bounds.y >= 5, true, `frame ${frame} escaped its padded cell`);
  assert.equal(bounds.x + bounds.width <= 123, true, `frame ${frame} escaped its padded cell`);
  assert.equal(bounds.y + bounds.height <= 123, true, `frame ${frame} escaped its padded cell`);

  const colors = new Set();
  for (let y = 0; y < 128; y += 1) {
    for (let x = 0; x < 128; x += 1) {
      const offset = (y * runtime.width + frame * 128 + x) * 4;
      if (runtime.data[offset + 3] === 0) continue;
      assert.equal(
        runtime.data[offset] > 200 && runtime.data[offset + 1] < 70 && runtime.data[offset + 2] > 180,
        false,
        `visible magenta survived in frame ${frame}`,
      );
      colors.add(`${runtime.data[offset]},${runtime.data[offset + 1]},${runtime.data[offset + 2]}`);
    }
  }
  assert.equal(colors.size > 48, true, `frame ${frame} lost shaded color detail`);
}

console.log("machine gunner white-chroma strip contracts passed");

function alphaBounds(png, regionX, regionY, width, height) {
  let minX = regionX + width;
  let minY = regionY + height;
  let maxX = -1;
  let maxY = -1;
  for (let y = regionY; y < regionY + height; y += 1) {
    for (let x = regionX; x < regionX + width; x += 1) {
      if (png.data[(y * png.width + x) * 4 + 3] === 0) continue;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }
  assert.equal(maxX >= minX && maxY >= minY, true, "frame has no visible alpha");
  return { x: minX - regionX, y: minY - regionY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
