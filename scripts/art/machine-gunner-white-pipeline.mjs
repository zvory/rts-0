#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetDir = path.join(repoRoot, "client/assets/rigs/machine-gunner-pass-01");
const generatedDir = path.join(assetDir, "generated");

const baselinePath = path.join(generatedDir, "machine-gunner-pass-01-prewhite-strip.png");
const outputPath = path.join(assetDir, "machine-gunner-pass-01-strip.png");
const maskPath = path.join(generatedDir, "machine-gunner-pass-05-white-transfer-mask.png");
const colorGuidePath = path.join(generatedDir, "machine-gunner-pass-05-white-color-guide.png");

const sheets = {
  carry: {
    original: readPng(path.join(generatedDir, "machine-gunner-pass-01-carry-alpha.png")),
    generated: readPng(path.join(generatedDir, "machine-gunner-pass-03-white-carry-imagegen.png")),
    columns: 6,
  },
  deploy: {
    original: readPng(path.join(generatedDir, "machine-gunner-pass-01-deploy-alpha.png")),
    generated: readPng(path.join(generatedDir, "machine-gunner-pass-03-white-deploy-imagegen.png")),
    columns: 6,
  },
  fire: {
    original: readPng(path.join(generatedDir, "machine-gunner-pass-01-fire-recoil-alpha.png")),
    generated: readPng(path.join(generatedDir, "machine-gunner-pass-03-white-fire-imagegen.png")),
    columns: 3,
  },
};

const frameSources = [
  ...Array.from({ length: 6 }, (_, frame) => ({ sheet: "carry", frame })),
  { sheet: "carry", frame: 0 },
  ...Array.from({ length: 5 }, (_, index) => ({ sheet: "deploy", frame: index + 1 })),
  ...Array.from({ length: 3 }, (_, frame) => ({ sheet: "fire", frame })),
];

const baseline = readPng(baselinePath);
if (baseline.width !== 960 || baseline.height !== 64) {
  throw new Error(`Unexpected baseline strip size ${baseline.width}x${baseline.height}`);
}

const output = clonePng(baseline);
const mask = new PNG({ width: baseline.width, height: baseline.height, colorType: 6 });
const colorGuide = new PNG({ width: baseline.width, height: baseline.height, colorType: 6 });

for (let frameIndex = 0; frameIndex < frameSources.length; frameIndex += 1) {
  const source = frameSources[frameIndex];
  const sheet = sheets[source.sheet];
  const runtimeBounds = alphaBounds(baseline, {
    x: frameIndex * 64,
    y: 0,
    width: 64,
    height: 64,
  });
  const sourceCellWidth = sheet.original.width / sheet.columns;
  const sourceCell = {
    x: source.frame * sourceCellWidth,
    y: 0,
    width: sourceCellWidth,
    height: sheet.original.height,
  };
  const sourceBounds = alphaBounds(sheet.original, sourceCell);
  for (let y = runtimeBounds.y; y < runtimeBounds.y + runtimeBounds.height; y += 1) {
    for (let x = runtimeBounds.x; x < runtimeBounds.x + runtimeBounds.width; x += 1) {
      const runtimeX = frameIndex * 64 + x;
      const runtimeOffset = pixelOffset(baseline, runtimeX, y);
      if (baseline.data[runtimeOffset + 3] === 0) continue;

      const u = runtimeBounds.width <= 1 ? 0.5 : (x - runtimeBounds.x) / (runtimeBounds.width - 1);
      const v = runtimeBounds.height <= 1 ? 0.5 : (y - runtimeBounds.y) / (runtimeBounds.height - 1);
      const sourceX = sourceCell.x + sourceBounds.x + u * Math.max(0, sourceBounds.width - 1);
      const sourceY = sourceCell.y + sourceBounds.y + v * Math.max(0, sourceBounds.height - 1);
      const generatedColor = generatedColorAt(sheet, sourceX, sourceY);

      colorGuide.data[runtimeOffset] = generatedColor.r;
      colorGuide.data[runtimeOffset + 1] = generatedColor.g;
      colorGuide.data[runtimeOffset + 2] = generatedColor.b;
      colorGuide.data[runtimeOffset + 3] = 255;
    }
  }
}

let changedPixels = 0;
const changedByFrame = Array(frameSources.length).fill(0);
for (let y = 0; y < baseline.height; y += 1) {
  for (let x = 0; x < baseline.width; x += 1) {
    const offset = pixelOffset(baseline, x, y);
    if (colorGuide.data[offset + 3] === 0) continue;
    if (
      baseline.data[offset] === colorGuide.data[offset]
      && baseline.data[offset + 1] === colorGuide.data[offset + 1]
      && baseline.data[offset + 2] === colorGuide.data[offset + 2]
    ) {
      colorGuide.data[offset + 3] = 0;
      continue;
    }
    output.data[offset] = colorGuide.data[offset];
    output.data[offset + 1] = colorGuide.data[offset + 1];
    output.data[offset + 2] = colorGuide.data[offset + 2];
    mask.data[offset] = 255;
    mask.data[offset + 1] = 255;
    mask.data[offset + 2] = 255;
    mask.data[offset + 3] = 255;
    changedPixels += 1;
    changedByFrame[Math.floor(x / 64)] += 1;
  }
}

assertAlphaUnchanged(baseline, output);
assertOnlyMaskedRgbChanged(baseline, output, mask);
if (changedPixels < 700) {
  throw new Error(`Recolor mask is unexpectedly small (${changedPixels} changed pixels)`);
}
if (changedByFrame.some((count) => count < 15)) {
  throw new Error(`At least one frame has too little recolor coverage: ${changedByFrame.join(", ")}`);
}

fs.writeFileSync(outputPath, PNG.sync.write(output, { colorType: 6 }));
fs.writeFileSync(maskPath, PNG.sync.write(mask, { colorType: 6 }));
fs.writeFileSync(colorGuidePath, PNG.sync.write(colorGuide, { colorType: 6 }));
console.log(JSON.stringify({
  output: path.relative(repoRoot, outputPath),
  mask: path.relative(repoRoot, maskPath),
  colorGuide: path.relative(repoRoot, colorGuidePath),
  changedPixels,
  changedByFrame,
  alphaBytesChanged: 0,
  unmaskedRgbBytesChanged: 0,
}));

function generatedColorAt(sheet, sourceX, sourceY) {
  let r = 0;
  let g = 0;
  let b = 0;
  let samples = 0;
  for (const [dx, dy] of [
    [-4, -4], [0, -4], [4, -4],
    [-4, 0], [0, 0], [4, 0],
    [-4, 4], [0, 4], [4, 4],
  ]) {
    const sample = generatedColorSample(sheet, sourceX + dx, sourceY + dy);
    r += sample.r;
    g += sample.g;
    b += sample.b;
    samples += 1;
  }
  return {
    r: Math.round(r / samples),
    g: Math.round(g / samples),
    b: Math.round(b / samples),
  };
}

function generatedColorSample(sheet, sourceX, sourceY) {
  const generatedX = Math.round(sourceX * (sheet.generated.width - 1) / (sheet.original.width - 1));
  const generatedY = Math.round(sourceY * (sheet.generated.height - 1) / (sheet.original.height - 1));
  const generatedOffset = pixelOffset(sheet.generated, generatedX, generatedY);
  return {
    r: sheet.generated.data[generatedOffset],
    g: sheet.generated.data[generatedOffset + 1],
    b: sheet.generated.data[generatedOffset + 2],
  };
}

function alphaBounds(png, region) {
  let minX = region.x + region.width;
  let minY = region.y + region.height;
  let maxX = -1;
  let maxY = -1;
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      if (png.data[pixelOffset(png, x, y) + 3] === 0) continue;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }
  if (maxX < minX || maxY < minY) throw new Error("No visible alpha found in frame");
  return {
    x: minX - region.x,
    y: minY - region.y,
    width: maxX - minX + 1,
    height: maxY - minY + 1,
  };
}

function assertAlphaUnchanged(before, after) {
  for (let offset = 3; offset < before.data.length; offset += 4) {
    if (before.data[offset] !== after.data[offset]) {
      throw new Error(`Alpha changed at pixel index ${(offset - 3) / 4}`);
    }
  }
}

function assertOnlyMaskedRgbChanged(before, after, approvedMask) {
  for (let offset = 0; offset < before.data.length; offset += 4) {
    const isMasked = approvedMask.data[offset + 3] > 0;
    for (let channel = 0; channel < 3; channel += 1) {
      if (!isMasked && before.data[offset + channel] !== after.data[offset + channel]) {
        throw new Error(`Unmasked RGB changed at pixel index ${offset / 4}`);
      }
    }
  }
}

function luma(r, g, b) {
  return r * 0.2126 + g * 0.7152 + b * 0.0722;
}

function pixelOffset(png, x, y) {
  const boundedX = Math.max(0, Math.min(png.width - 1, x));
  const boundedY = Math.max(0, Math.min(png.height - 1, y));
  return (boundedY * png.width + boundedX) * 4;
}

function clonePng(source) {
  const clone = new PNG({ width: source.width, height: source.height, colorType: 6 });
  source.data.copy(clone.data);
  return clone;
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
