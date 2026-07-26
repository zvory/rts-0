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
const mask = readPng(path.join(generatedDir, "machine-gunner-pass-02-white-recolor-mask.png"));

assert.equal(before.width, 960);
assert.equal(before.height, 64);
assert.equal(after.width, before.width);
assert.equal(after.height, before.height);
assert.equal(mask.width, before.width);
assert.equal(mask.height, before.height);

let changedPixels = 0;
const changedByFrame = Array(15).fill(0);
for (let y = 0; y < before.height; y += 1) {
  for (let x = 0; x < before.width; x += 1) {
    const offset = (y * before.width + x) * 4;
    const masked = mask.data[offset + 3] > 0;
    const frame = Math.floor(x / 64);
    const frameX = x % 64;

    assert.equal(after.data[offset + 3], before.data[offset + 3], `alpha changed at ${x},${y}`);
    if (!masked || protectedWeaponPixel(frame, frameX, y)) {
      for (let channel = 0; channel < 3; channel += 1) {
        assert.equal(
          after.data[offset + channel],
          before.data[offset + channel],
          `protected RGB changed at ${x},${y}`,
        );
      }
    }
    if (protectedWeaponPixel(frame, frameX, y)) {
      assert.equal(masked, false, `approved recolor mask overlaps weapon at ${x},${y}`);
    }
    if (masked) {
      const expectedWhite = Math.max(
        180,
        Math.min(
          242,
          Math.round(
            165
              + luma(
                before.data[offset],
                before.data[offset + 1],
                before.data[offset + 2],
              ) * 0.45,
          ),
        ),
      );
      for (let channel = 0; channel < 3; channel += 1) {
        assert.equal(
          after.data[offset + channel],
          expectedWhite,
          `masked RGB does not match the white-clothing transform at ${x},${y}`,
        );
      }
      assert.notDeepEqual(
        [...after.data.subarray(offset, offset + 3)],
        [...before.data.subarray(offset, offset + 3)],
        `approved recolor pixel is unchanged at ${x},${y}`,
      );
      changedPixels += 1;
      changedByFrame[frame] += 1;
    }
  }
}

assert.equal(changedPixels, 860, "approved clothing/backpack recolor coverage stays intentional");
assert.equal(changedByFrame.every((count) => count >= 15), true);
console.log("machine gunner white-strip contracts passed");

function protectedWeaponPixel(frameIndex, x, y) {
  if (frameIndex <= 6) return y >= 32 && y <= 38;
  if (frameIndex === 7) return distanceToSegment(x, y, 16, 48, 47, 20) <= 3.5;
  if (frameIndex === 8) return distanceToSegment(x, y, 23, 53, 43, 18) <= 3.5;
  return x >= 28 && x <= 36 && y >= 22;
}

function distanceToSegment(x, y, x1, y1, x2, y2) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0
    ? 0
    : Math.max(0, Math.min(1, ((x - x1) * dx + (y - y1) * dy) / lengthSquared));
  return Math.hypot(x - (x1 + t * dx), y - (y1 + t * dy));
}

function luma(r, g, b) {
  return r * 0.2126 + g * 0.7152 + b * 0.0722;
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
