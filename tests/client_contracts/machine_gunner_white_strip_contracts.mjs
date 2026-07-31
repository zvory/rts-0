import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetDir = path.join(repoRoot, "client/assets/rigs/machine-gunner-pass-01");
const generatedDir = path.join(assetDir, "generated");
const before = readPng(path.join(generatedDir, "machine-gunner-pass-01-prewhite-strip.png"));
const after = readPng(path.join(assetDir, "machine-gunner-pass-01-strip.png"));
const mask = readPng(path.join(generatedDir, "machine-gunner-pass-05-white-transfer-mask.png"));
const colorGuide = readPng(path.join(generatedDir, "machine-gunner-pass-05-white-color-guide.png"));

assert.equal(before.width, 960);
assert.equal(before.height, 64);
assert.equal(after.width, before.width);
assert.equal(after.height, before.height);
assert.equal(mask.width, before.width);
assert.equal(mask.height, before.height);
assert.equal(colorGuide.width, before.width);
assert.equal(colorGuide.height, before.height);

let changedPixels = 0;
const changedByFrame = Array(15).fill(0);
const colorsByFrame = Array.from({ length: 15 }, () => new Set());
for (let y = 0; y < before.height; y += 1) {
  for (let x = 0; x < before.width; x += 1) {
    const offset = (y * before.width + x) * 4;
    const masked = mask.data[offset + 3] > 0;
    const frame = Math.floor(x / 64);
    const visible = before.data[offset + 3] > 0;

    assert.equal(after.data[offset + 3], before.data[offset + 3], `alpha changed at ${x},${y}`);
    if (!masked) {
      for (let channel = 0; channel < 3; channel += 1) {
        assert.equal(
          after.data[offset + channel],
          before.data[offset + channel],
          `untransferred RGB changed at ${x},${y}`,
        );
      }
    }
    if (visible) {
      assert.equal(
        colorGuide.data[offset + 3],
        255,
        `visible runtime pixel lacks a whole-frame ImageGen color guide at ${x},${y}`,
      );
      for (let channel = 0; channel < 3; channel += 1) {
        assert.equal(
          after.data[offset + channel],
          colorGuide.data[offset + channel],
          `runtime RGB does not match its whole-frame ImageGen color guide at ${x},${y}`,
        );
      }
    }
    if (masked) {
      assert.equal(colorGuide.data[offset + 3], 255, `masked pixel lacks a color guide at ${x},${y}`);
      for (let channel = 0; channel < 3; channel += 1) {
        assert.equal(
          after.data[offset + channel],
          colorGuide.data[offset + channel],
          `transferred RGB does not match the registered ImageGen color guide at ${x},${y}`,
        );
      }
      assert.notDeepEqual(
        [...after.data.subarray(offset, offset + 3)],
        [...before.data.subarray(offset, offset + 3)],
        `approved recolor pixel is unchanged at ${x},${y}`,
      );
      changedPixels += 1;
      changedByFrame[frame] += 1;
      colorsByFrame[frame].add(
        `${after.data[offset]},${after.data[offset + 1]},${after.data[offset + 2]}`,
      );
    }
  }
}

assert.equal(changedPixels, 15442, "whole-frame ImageGen color-transfer coverage stays intentional");
assert.equal(changedByFrame.every((count) => count >= 15), true);
assert.equal(
  colorsByFrame.every((colors) => colors.size >= 24),
  true,
  "every frame preserves a shaded color range instead of flat recolor blocks",
);
console.log("machine gunner white-strip contracts passed");

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
