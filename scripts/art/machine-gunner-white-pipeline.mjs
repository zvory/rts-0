#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const generatedDir = path.join(
  repoRoot,
  "client/assets/rigs/machine-gunner-pass-01/generated",
);
const outputPath = path.join(
  repoRoot,
  "client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png",
);

const frameSize = 128;
const padding = 6;
const sheets = {
  carry: readPng(path.join(generatedDir, "machine-gunner-white-carry-alpha.png")),
  deploy: readPng(path.join(generatedDir, "machine-gunner-white-deploy-alpha.png")),
  fire: readPng(path.join(generatedDir, "machine-gunner-white-fire-alpha.png")),
};
const columns = { carry: 6, deploy: 6, fire: 3 };
const frames = [
  ...Array.from({ length: 6 }, (_, frame) => ({ sheet: "carry", frame })),
  { sheet: "carry", frame: 0 },
  ...Array.from({ length: 5 }, (_, index) => ({ sheet: "deploy", frame: index + 1 })),
  ...Array.from({ length: 3 }, (_, frame) => ({ sheet: "fire", frame })),
];
const preparedFrames = frames.map(({ sheet: sheetName, frame }) => {
  const sourceSheet = sheets[sheetName];
  const sourceCell = sheetCell(sourceSheet, columns[sheetName], frame);
  const sheet = isolatedCell(sourceSheet, sourceCell);
  const bounds = alphaBounds(
    sheet,
    { x: 0, y: 0, width: sheet.width, height: sheet.height },
    16,
  );
  return { sheetName, sheet, bounds };
});
const scalesBySheet = Object.fromEntries(Object.keys(sheets).map((sheetName) => {
  const group = preparedFrames.filter((frame) => frame.sheetName === sheetName);
  const maxWidth = Math.max(...group.map((frame) => frame.bounds.width));
  const maxHeight = Math.max(...group.map((frame) => frame.bounds.height));
  return [sheetName, Math.min(
    (frameSize - padding * 2) / maxWidth,
    (frameSize - padding * 2) / maxHeight,
  )];
}));

const output = new PNG({
  width: frameSize * frames.length,
  height: frameSize,
  colorType: 6,
});
const frameBounds = [];

for (let outputFrame = 0; outputFrame < preparedFrames.length; outputFrame += 1) {
  const { sheetName, sheet, bounds } = preparedFrames[outputFrame];
  const scale = scalesBySheet[sheetName];
  const width = Math.max(1, Math.round(bounds.width * scale));
  const height = Math.max(1, Math.round(bounds.height * scale));
  const left = outputFrame * frameSize + Math.floor((frameSize - width) / 2);
  const top = Math.floor((frameSize - height) / 2);

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const sourceX = bounds.x + (x + 0.5) / scale - 0.5;
      const sourceY = bounds.y + (y + 0.5) / scale - 0.5;
      const destination = ((top + y) * output.width + left + x) * 4;
      const rgba = sampleRgba(sheet, sourceX, sourceY);
      for (let channel = 0; channel < 4; channel += 1) {
        output.data[destination + channel] = rgba[channel];
      }
    }
  }
  frameBounds.push(alphaBounds(output, {
    x: outputFrame * frameSize,
    y: 0,
    width: frameSize,
    height: frameSize,
  }, 1));
}

fs.writeFileSync(outputPath, PNG.sync.write(output, { colorType: 6 }));
console.log(JSON.stringify({
  output: path.relative(repoRoot, outputPath),
  frameSize,
  frameCount: frames.length,
  scalesBySheet,
  frameBounds: frameBounds.map(({ x, y, width, height }) => ({
    x: x % frameSize,
    y,
    width,
    height,
  })),
}, null, 2));

function sheetCell(sheet, count, frame) {
  const x = Math.round(frame * sheet.width / count);
  const right = Math.round((frame + 1) * sheet.width / count);
  return { x, y: 0, width: right - x, height: sheet.height };
}

function isolatedCell(sheet, cell) {
  const result = new PNG({ width: cell.width, height: cell.height, colorType: 6 });
  for (let y = 0; y < cell.height; y += 1) {
    const sourceStart = ((cell.y + y) * sheet.width + cell.x) * 4;
    const destinationStart = y * cell.width * 4;
    sheet.data.copy(result.data, destinationStart, sourceStart, sourceStart + cell.width * 4);
  }

  const visited = new Uint8Array(result.width * result.height);
  let largest = [];
  for (let y = 0; y < result.height; y += 1) {
    for (let x = 0; x < result.width; x += 1) {
      const start = y * result.width + x;
      if (visited[start] || result.data[start * 4 + 3] < 16) continue;
      const component = [start];
      visited[start] = 1;
      for (let cursor = 0; cursor < component.length; cursor += 1) {
        const index = component[cursor];
        const currentX = index % result.width;
        const currentY = Math.floor(index / result.width);
        for (const [nextX, nextY] of [
          [currentX - 1, currentY], [currentX + 1, currentY],
          [currentX, currentY - 1], [currentX, currentY + 1],
        ]) {
          if (nextX < 0 || nextX >= result.width || nextY < 0 || nextY >= result.height) continue;
          const next = nextY * result.width + nextX;
          if (visited[next] || result.data[next * 4 + 3] < 16) continue;
          visited[next] = 1;
          component.push(next);
        }
      }
      if (component.length > largest.length) largest = component;
    }
  }

  const keep = new Uint8Array(result.width * result.height);
  for (const index of largest) keep[index] = 1;
  for (let index = 0; index < keep.length; index += 1) {
    if (keep[index]) continue;
    result.data[index * 4] = 0;
    result.data[index * 4 + 1] = 0;
    result.data[index * 4 + 2] = 0;
    result.data[index * 4 + 3] = 0;
  }
  return result;
}

function alphaBounds(png, region, threshold) {
  let minX = region.x + region.width;
  let minY = region.y + region.height;
  let maxX = -1;
  let maxY = -1;
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      if (png.data[(y * png.width + x) * 4 + 3] < threshold) continue;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }
  if (maxX < minX || maxY < minY) throw new Error("Frame has no visible pixels");
  return { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

function sampleRgba(png, x, y) {
  const x0 = clamp(Math.floor(x), 0, png.width - 1);
  const y0 = clamp(Math.floor(y), 0, png.height - 1);
  const x1 = Math.min(png.width - 1, x0 + 1);
  const y1 = Math.min(png.height - 1, y0 + 1);
  const tx = x - Math.floor(x);
  const ty = y - Math.floor(y);
  const samples = [
    [x0, y0, (1 - tx) * (1 - ty)],
    [x1, y0, tx * (1 - ty)],
    [x0, y1, (1 - tx) * ty],
    [x1, y1, tx * ty],
  ];
  let alpha = 0;
  const premultiplied = [0, 0, 0];
  for (const [sampleX, sampleY, weight] of samples) {
    const offset = (sampleY * png.width + sampleX) * 4;
    const weightedAlpha = png.data[offset + 3] * weight;
    alpha += weightedAlpha;
    for (let channel = 0; channel < 3; channel += 1) {
      premultiplied[channel] += png.data[offset + channel] * weightedAlpha;
    }
  }
  const roundedAlpha = Math.round(alpha);
  if (roundedAlpha === 0 || alpha === 0) return [0, 0, 0, 0];
  return [
    Math.round(premultiplied[0] / alpha),
    Math.round(premultiplied[1] / alpha),
    Math.round(premultiplied[2] / alpha),
    roundedAlpha,
  ];
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function readPng(filePath) {
  return PNG.sync.read(fs.readFileSync(filePath), { skipRescale: true });
}
