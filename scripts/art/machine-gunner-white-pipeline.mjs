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
const maskPath = path.join(generatedDir, "machine-gunner-pass-06-high-resolution-mask.png");
const colorGuidePath = path.join(generatedDir, "machine-gunner-pass-06-high-resolution-color-guide.png");
const sourceFrameSize = 64;
const runtimeFrameSize = 160;
const runtimeResolutionScale = runtimeFrameSize / sourceFrameSize;

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

const output = new PNG({
  width: runtimeFrameSize * frameSources.length,
  height: runtimeFrameSize,
  colorType: 6,
});
const mask = new PNG({ width: output.width, height: output.height, colorType: 6 });
const colorGuide = new PNG({ width: output.width, height: output.height, colorType: 6 });

for (let frameIndex = 0; frameIndex < frameSources.length; frameIndex += 1) {
  const source = frameSources[frameIndex];
  const sheet = sheets[source.sheet];
  const compactBounds = alphaBounds(baseline, {
    x: frameIndex * sourceFrameSize,
    y: 0,
    width: sourceFrameSize,
    height: sourceFrameSize,
  });
  const runtimeBounds = scaleBounds(compactBounds, runtimeResolutionScale);
  const sourceCellWidth = sheet.original.width / sheet.columns;
  const sourceCell = {
    x: source.frame * sourceCellWidth,
    y: 0,
    width: sourceCellWidth,
    height: sheet.original.height,
  };
  const sourceBounds = alphaBounds(sheet.original, sourceCell);
  const sampleRadius = Math.max(
    0.5,
    Math.min(
      3,
      Math.max(
        sourceBounds.width / runtimeBounds.width,
        sourceBounds.height / runtimeBounds.height,
      ) / 2,
    ),
  );
  for (let y = runtimeBounds.y; y < runtimeBounds.y + runtimeBounds.height; y += 1) {
    for (let x = runtimeBounds.x; x < runtimeBounds.x + runtimeBounds.width; x += 1) {
      const u = runtimeBounds.width <= 1 ? 0.5 : (x - runtimeBounds.x) / (runtimeBounds.width - 1);
      const v = runtimeBounds.height <= 1 ? 0.5 : (y - runtimeBounds.y) / (runtimeBounds.height - 1);
      const sourceX = sourceCell.x + sourceBounds.x + u * Math.max(0, sourceBounds.width - 1);
      const sourceY = sourceCell.y + sourceBounds.y + v * Math.max(0, sourceBounds.height - 1);
      const generatedPixel = generatedPixelAt(sheet, sourceX, sourceY, sampleRadius, sourceCell);
      if (generatedPixel.a === 0) continue;
      const runtimeX = frameIndex * runtimeFrameSize + x;
      const runtimeOffset = pixelOffset(output, runtimeX, y);

      output.data[runtimeOffset] = generatedPixel.r;
      output.data[runtimeOffset + 1] = generatedPixel.g;
      output.data[runtimeOffset + 2] = generatedPixel.b;
      output.data[runtimeOffset + 3] = generatedPixel.a;
      colorGuide.data[runtimeOffset] = generatedPixel.r;
      colorGuide.data[runtimeOffset + 1] = generatedPixel.g;
      colorGuide.data[runtimeOffset + 2] = generatedPixel.b;
      colorGuide.data[runtimeOffset + 3] = generatedPixel.a;
      mask.data[runtimeOffset] = 255;
      mask.data[runtimeOffset + 1] = 255;
      mask.data[runtimeOffset + 2] = 255;
      mask.data[runtimeOffset + 3] = 255;
    }
  }
}

removeDetachedAlphaComponents(output, mask, colorGuide);

let changedPixels = 0;
const changedByFrame = Array(frameSources.length).fill(0);
for (let y = 0; y < output.height; y += 1) {
  for (let x = 0; x < output.width; x += 1) {
    const offset = pixelOffset(output, x, y);
    if (output.data[offset + 3] === 0) continue;
    changedPixels += 1;
    changedByFrame[Math.floor(x / runtimeFrameSize)] += 1;
  }
}

assertScaledFrameBounds(baseline, output);
if (changedPixels < 50000) {
  throw new Error(`High-resolution strip has unexpectedly little visible coverage (${changedPixels} pixels)`);
}
if (changedByFrame.some((count) => count < 500)) {
  throw new Error(`At least one frame has too little high-resolution coverage: ${changedByFrame.join(", ")}`);
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
  runtimeFrameSize,
  runtimeResolutionScale,
}));

function generatedPixelAt(sheet, sourceX, sourceY, radius, sourceCell) {
  let alphaTotal = 0;
  let weightedR = 0;
  let weightedG = 0;
  let weightedB = 0;
  const offsets = [-radius, 0, radius];
  const sampleCount = offsets.length * offsets.length;
  for (const dy of offsets) {
    for (const dx of offsets) {
      const originalX = clamp(sourceX + dx, sourceCell.x, sourceCell.x + sourceCell.width - 1);
      const originalY = clamp(sourceY + dy, sourceCell.y, sourceCell.y + sourceCell.height - 1);
      const alpha = sampleChannel(sheet.original, originalX, originalY, 3);
      if (alpha <= 0) continue;
      const generatedX = originalX * (sheet.generated.width - 1) / (sheet.original.width - 1);
      const generatedY = originalY * (sheet.generated.height - 1) / (sheet.original.height - 1);
      weightedR += sampleChannel(sheet.generated, generatedX, generatedY, 0) * alpha;
      weightedG += sampleChannel(sheet.generated, generatedX, generatedY, 1) * alpha;
      weightedB += sampleChannel(sheet.generated, generatedX, generatedY, 2) * alpha;
      alphaTotal += alpha;
    }
  }
  const alpha = Math.round(alphaTotal / sampleCount);
  if (alpha === 0 || alphaTotal === 0) return { r: 0, g: 0, b: 0, a: 0 };
  return {
    r: Math.round(weightedR / alphaTotal),
    g: Math.round(weightedG / alphaTotal),
    b: Math.round(weightedB / alphaTotal),
    a: alpha,
  };
}

function sampleChannel(png, x, y, channel) {
  const x0 = Math.floor(clamp(x, 0, png.width - 1));
  const y0 = Math.floor(clamp(y, 0, png.height - 1));
  const x1 = Math.min(png.width - 1, x0 + 1);
  const y1 = Math.min(png.height - 1, y0 + 1);
  const tx = x - x0;
  const ty = y - y0;
  const top = png.data[pixelOffset(png, x0, y0) + channel] * (1 - tx)
    + png.data[pixelOffset(png, x1, y0) + channel] * tx;
  const bottom = png.data[pixelOffset(png, x0, y1) + channel] * (1 - tx)
    + png.data[pixelOffset(png, x1, y1) + channel] * tx;
  return top * (1 - ty) + bottom * ty;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function scaleBounds(bounds, scale) {
  const x = Math.round(bounds.x * scale);
  const y = Math.round(bounds.y * scale);
  const right = Math.round((bounds.x + bounds.width) * scale);
  const bottom = Math.round((bounds.y + bounds.height) * scale);
  return { x, y, width: right - x, height: bottom - y };
}

function removeDetachedAlphaComponents(...images) {
  const primary = images[0];
  for (let frame = 0; frame < frameSources.length; frame += 1) {
    // The accepted setup sheet has two known detached left-edge crop fragments in these frames.
    if (frame !== 7 && frame !== 8) continue;
    const visited = new Uint8Array(runtimeFrameSize * runtimeFrameSize);
    const components = [];
    for (let y = 0; y < runtimeFrameSize; y += 1) {
      for (let x = 0; x < runtimeFrameSize; x += 1) {
        const localIndex = y * runtimeFrameSize + x;
        const globalX = frame * runtimeFrameSize + x;
        if (visited[localIndex] || primary.data[pixelOffset(primary, globalX, y) + 3] === 0) continue;
        const component = [];
        const queue = [localIndex];
        visited[localIndex] = 1;
        for (let cursor = 0; cursor < queue.length; cursor += 1) {
          const current = queue[cursor];
          const currentX = current % runtimeFrameSize;
          const currentY = Math.floor(current / runtimeFrameSize);
          component.push(current);
          for (const [nextX, nextY] of [
            [currentX - 1, currentY],
            [currentX + 1, currentY],
            [currentX, currentY - 1],
            [currentX, currentY + 1],
          ]) {
            if (
              nextX < 0
              || nextX >= runtimeFrameSize
              || nextY < 0
              || nextY >= runtimeFrameSize
            ) continue;
            const next = nextY * runtimeFrameSize + nextX;
            const nextGlobalX = frame * runtimeFrameSize + nextX;
            if (visited[next] || primary.data[pixelOffset(primary, nextGlobalX, nextY) + 3] === 0) continue;
            visited[next] = 1;
            queue.push(next);
          }
        }
        components.push(component);
      }
    }
    components.sort((a, b) => b.length - a.length);
    for (const component of components.slice(1)) {
      for (const localIndex of component) {
        const x = frame * runtimeFrameSize + (localIndex % runtimeFrameSize);
        const y = Math.floor(localIndex / runtimeFrameSize);
        for (const image of images) {
          const offset = pixelOffset(image, x, y);
          image.data.fill(0, offset, offset + 4);
        }
      }
    }
  }
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

function assertScaledFrameBounds(before, after) {
  for (let frame = 0; frame < frameSources.length; frame += 1) {
    const compact = alphaBounds(before, {
      x: frame * sourceFrameSize,
      y: 0,
      width: sourceFrameSize,
      height: sourceFrameSize,
    });
    const expected = scaleBounds(compact, runtimeResolutionScale);
    const actual = alphaBounds(after, {
      x: frame * runtimeFrameSize,
      y: 0,
      width: runtimeFrameSize,
      height: runtimeFrameSize,
    });
    const expectedRight = expected.x + expected.width;
    const expectedBottom = expected.y + expected.height;
    const actualRight = actual.x + actual.width;
    const actualBottom = actual.y + actual.height;
    const setupCropIsValid = (frame === 7 || frame === 8)
      && actual.x >= expected.x
      && actual.y >= expected.y
      && actualRight === expectedRight
      && actualBottom === expectedBottom
      && actual.width >= expected.width * 0.75
      && actual.height >= expected.height * 0.75;
    if (!setupCropIsValid && JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`Frame ${frame} bounds changed: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    }
  }
}

function pixelOffset(png, x, y) {
  const boundedX = Math.max(0, Math.min(png.width - 1, x));
  const boundedY = Math.max(0, Math.min(png.height - 1, y));
  return (boundedY * png.width + boundedX) * 4;
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
