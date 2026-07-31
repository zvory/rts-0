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
const compact = readPng(path.join(generatedDir, "machine-gunner-pass-01-prewhite-strip.png"));
const runtime = readPng(path.join(assetDir, "machine-gunner-pass-01-strip.png"));
const mask = readPng(path.join(generatedDir, "machine-gunner-pass-06-high-resolution-mask.png"));
const colorGuide = readPng(path.join(generatedDir, "machine-gunner-pass-06-high-resolution-color-guide.png"));

assert.equal(compact.width, 960);
assert.equal(compact.height, 64);
assert.equal(runtime.width, 2400);
assert.equal(runtime.height, 160);
assert.equal(mask.width, runtime.width);
assert.equal(mask.height, runtime.height);
assert.equal(colorGuide.width, runtime.width);
assert.equal(colorGuide.height, runtime.height);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameWidth, RIFLEMAN_PNG_FRAME_STRIP.frameWidth);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameWidth, 160);
assert.equal(MACHINE_GUNNER_PNG_FRAME_STRIP.frameHeight, 160);
assertApprox(
  MACHINE_GUNNER_PNG_FRAME_STRIP.frameWidth * MACHINE_GUNNER_PNG_FRAME_STRIP.worldScale,
  64 * 0.84,
  "deployed world-space canvas extent changed",
);
assertApprox(
  MACHINE_GUNNER_PNG_FRAME_STRIP.frameWidth * MACHINE_GUNNER_PNG_FRAME_STRIP.movementWorldScale,
  64 * 0.612,
  "movement world-space canvas extent changed",
);

let visiblePixels = 0;
const visibleByFrame = Array(15).fill(0);
const colorsByFrame = Array.from({ length: 15 }, () => new Set());
for (let y = 0; y < runtime.height; y += 1) {
  for (let x = 0; x < runtime.width; x += 1) {
    const offset = (y * runtime.width + x) * 4;
    const visible = runtime.data[offset + 3] > 0;
    const frame = Math.floor(x / 160);
    assert.equal(mask.data[offset + 3] > 0, visible, `mask visibility differs at ${x},${y}`);
    assert.equal(
      colorGuide.data[offset + 3],
      runtime.data[offset + 3],
      `color-guide alpha differs at ${x},${y}`,
    );
    if (!visible) continue;
    for (let channel = 0; channel < 3; channel += 1) {
      assert.equal(
        runtime.data[offset + channel],
        colorGuide.data[offset + channel],
        `runtime RGB differs from its high-resolution ImageGen guide at ${x},${y}`,
      );
    }
    visiblePixels += 1;
    visibleByFrame[frame] += 1;
    colorsByFrame[frame].add(
      `${runtime.data[offset]},${runtime.data[offset + 1]},${runtime.data[offset + 2]}`,
    );
  }
}

assert.equal(visiblePixels, 100768, "high-resolution visible coverage stays intentional");
assert.equal(visibleByFrame.every((count) => count >= 4000), true);
assert.equal(
  colorsByFrame.every((colors) => colors.size >= 48),
  true,
  "every high-resolution frame preserves a broad shaded color range",
);

for (let frame = 0; frame < 15; frame += 1) {
  const compactBounds = alphaBounds(compact, frame * 64, 0, 64, 64);
  const expected = scaleBounds(compactBounds, 2.5);
  const actual = alphaBounds(runtime, frame * 160, 0, 160, 160);
  if (frame === 7 || frame === 8) {
    assert.equal(actual.x >= expected.x, true);
    assert.equal(actual.y >= expected.y, true);
    assert.equal(actual.x + actual.width, expected.x + expected.width);
    assert.equal(actual.y + actual.height, expected.y + expected.height);
    assert.equal(actual.width >= expected.width * 0.75, true);
    assert.equal(actual.height >= expected.height * 0.75, true);
  } else {
    assert.deepEqual(actual, expected, `frame ${frame} world-space bounds changed`);
  }
}

console.log("machine gunner high-resolution white-strip contracts passed");

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
  return {
    x: minX - regionX,
    y: minY - regionY,
    width: maxX - minX + 1,
    height: maxY - minY + 1,
  };
}

function scaleBounds(bounds, scale) {
  const x = Math.round(bounds.x * scale);
  const y = Math.round(bounds.y * scale);
  const right = Math.round((bounds.x + bounds.width) * scale);
  const bottom = Math.round((bounds.y + bounds.height) * scale);
  return { x, y, width: right - x, height: bottom - y };
}

function assertApprox(actual, expected, message) {
  assert.equal(Math.abs(actual - expected) < 1e-9, true, `${message}: ${actual} != ${expected}`);
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
