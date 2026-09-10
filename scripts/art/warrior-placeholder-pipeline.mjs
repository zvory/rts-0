#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetDir = path.join(repoRoot, "client/assets/rigs/warrior-placeholder-pass-01/generated");
const sourcePath = path.join(assetDir, "warrior-source.png");
const outputPath = path.join(assetDir, "warrior-runtime-strip.png");
const frameSize = 160;
const frameCount = 6;
const gutter = 5;
const source = PNG.sync.read(fs.readFileSync(sourcePath), { skipRescale: true });
const keyedSource = chromaKey(source);
const figures = groupFigures(connectedComponents(keyedSource), frameCount);
if (figures.length !== frameCount) throw new Error(`expected ${frameCount} complete Warrior figures`);

const output = new PNG({ width: frameSize * frameCount, height: frameSize, colorType: 6 });
const frameStats = [];
for (let frame = 0; frame < frameCount; frame += 1) {
  const isolated = extractComponent(keyedSource, figures[frame], 3);
  const repaired = repairInteriorTransparency(isolated);
  despillEdges(repaired);
  const scale = Math.min(
    (frameSize - gutter * 2) / repaired.width,
    84 / repaired.height,
  );
  const resampled = resizeRgba(
    repaired,
    Math.max(1, Math.round(repaired.width * scale)),
    Math.max(1, Math.round(repaired.height * scale)),
  );
  repairInteriorTransparency(resampled);
  despillEdges(resampled);
  const left = frame * frameSize + Math.floor((frameSize - resampled.width) / 2);
  const top = Math.floor((frameSize - resampled.height) / 2);
  blit(output, left, top, resampled);
  frameStats.push({
    frame,
    ...alphaBounds(output, frame * frameSize, 0, frameSize, frameSize),
  });
}

fs.writeFileSync(outputPath, PNG.sync.write(output, { colorType: 6 }));
console.log(JSON.stringify({
  source: path.relative(repoRoot, sourcePath),
  output: path.relative(repoRoot, outputPath),
  frameSize,
  frameCount,
  gutter,
  frameStats,
}, null, 2));

function chromaKey(image) {
  const result = new PNG({ width: image.width, height: image.height, colorType: 6 });
  for (let index = 0; index < image.width * image.height; index += 1) {
    const offset = index * 4;
    const red = image.data[offset];
    const green = image.data[offset + 1];
    const blue = image.data[offset + 2];
    const distance = Math.max(Math.abs(red - 255), green, Math.abs(blue - 255));
    const alpha = distance <= 28 ? 0 : distance >= 92
      ? 255
      : Math.round(255 * smoothstep((distance - 28) / 64));
    writePixel(result, index, red, green, blue, alpha);
  }
  return result;
}

function smoothstep(value) {
  const clamped = Math.max(0, Math.min(1, value));
  return clamped * clamped * (3 - 2 * clamped);
}

function connectedComponents(image) {
  const visited = new Uint8Array(image.width * image.height);
  const components = [];
  for (let y = 0; y < image.height; y += 1) {
    for (let x = 0; x < image.width; x += 1) {
      const start = y * image.width + x;
      if (visited[start] || alphaAt(image, start) < 16) continue;
      const indices = [start];
      let minX = x;
      let maxX = x;
      let minY = y;
      let maxY = y;
      visited[start] = 1;
      for (let cursor = 0; cursor < indices.length; cursor += 1) {
        const index = indices[cursor];
        const currentX = index % image.width;
        const currentY = Math.floor(index / image.width);
        minX = Math.min(minX, currentX);
        maxX = Math.max(maxX, currentX);
        minY = Math.min(minY, currentY);
        maxY = Math.max(maxY, currentY);
        forEachNeighbor(index, image.width, image.height, (next) => {
          if (visited[next] || alphaAt(image, next) < 16) return;
          visited[next] = 1;
          indices.push(next);
        });
      }
      components.push({ indices, minX, maxX, minY, maxY });
    }
  }
  return components;
}

function groupFigures(components, count) {
  const primaries = components
    .sort((left, right) => right.indices.length - left.indices.length)
    .slice(0, count)
    .map((component) => ({ ...component, indices: [...component.indices] }));
  const primarySet = new Set(primaries.map((component) => component.indices[0]));
  for (const component of components) {
    if (primarySet.has(component.indices[0]) || component.indices.length < 20) continue;
    let nearest = null;
    let nearestDistance = Number.POSITIVE_INFINITY;
    for (const primary of primaries) {
      const distance = rectangleDistance(component, primary);
      if (distance < nearestDistance) {
        nearest = primary;
        nearestDistance = distance;
      }
    }
    if (!nearest || nearestDistance > 72) continue;
    nearest.indices.push(...component.indices);
    nearest.minX = Math.min(nearest.minX, component.minX);
    nearest.maxX = Math.max(nearest.maxX, component.maxX);
    nearest.minY = Math.min(nearest.minY, component.minY);
    nearest.maxY = Math.max(nearest.maxY, component.maxY);
  }
  return primaries.sort((left, right) => left.minX - right.minX);
}

function rectangleDistance(left, right) {
  const dx = Math.max(0, left.minX - right.maxX, right.minX - left.maxX);
  const dy = Math.max(0, left.minY - right.maxY, right.minY - left.maxY);
  return Math.hypot(dx, dy);
}

function extractComponent(image, component, padding) {
  const left = Math.max(0, component.minX - padding);
  const top = Math.max(0, component.minY - padding);
  const right = Math.min(image.width - 1, component.maxX + padding);
  const bottom = Math.min(image.height - 1, component.maxY + padding);
  const result = new PNG({ width: right - left + 1, height: bottom - top + 1, colorType: 6 });
  for (const sourceIndex of component.indices) {
    const sourceX = sourceIndex % image.width;
    const sourceY = Math.floor(sourceIndex / image.width);
    const destinationIndex = (sourceY - top) * result.width + sourceX - left;
    const sourceOffset = sourceIndex * 4;
    writePixel(
      result,
      destinationIndex,
      image.data[sourceOffset],
      image.data[sourceOffset + 1],
      image.data[sourceOffset + 2],
      image.data[sourceOffset + 3],
    );
  }
  return result;
}

function repairInteriorTransparency(image) {
  const pixelCount = image.width * image.height;
  const visible = new Uint8Array(pixelCount);
  for (let index = 0; index < pixelCount; index += 1) {
    if (alphaAt(image, index) >= 16) visible[index] = 1;
  }

  const exterior = new Uint8Array(pixelCount);
  const queue = [];
  for (let x = 0; x < image.width; x += 1) {
    enqueueExterior(x, visible, exterior, queue);
    enqueueExterior((image.height - 1) * image.width + x, visible, exterior, queue);
  }
  for (let y = 1; y < image.height - 1; y += 1) {
    enqueueExterior(y * image.width, visible, exterior, queue);
    enqueueExterior(y * image.width + image.width - 1, visible, exterior, queue);
  }
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    forEachNeighbor(queue[cursor], image.width, image.height, (next) => {
      enqueueExterior(next, visible, exterior, queue);
    });
  }

  const nearest = new Int32Array(pixelCount);
  nearest.fill(-1);
  const colorQueue = [];
  for (let index = 0; index < pixelCount; index += 1) {
    if (!visible[index]) continue;
    nearest[index] = index;
    colorQueue.push(index);
  }
  for (let cursor = 0; cursor < colorQueue.length; cursor += 1) {
    const index = colorQueue[cursor];
    forEachNeighbor(index, image.width, image.height, (next) => {
      if (nearest[next] >= 0 || exterior[next]) return;
      nearest[next] = nearest[index];
      colorQueue.push(next);
    });
  }

  for (let index = 0; index < pixelCount; index += 1) {
    if (exterior[index]) continue;
    if (!visible[index]) {
      const sourceIndex = nearest[index];
      if (sourceIndex >= 0) {
        const sourceOffset = sourceIndex * 4;
        writePixel(
          image,
          index,
          image.data[sourceOffset],
          image.data[sourceOffset + 1],
          image.data[sourceOffset + 2],
          255,
        );
      }
      continue;
    }
    if (alphaAt(image, index) < 255 && !touchesExterior(index, exterior, image.width, image.height)) {
      image.data[index * 4 + 3] = 255;
    }
  }
  return image;
}

function despillEdges(image) {
  const pixelCount = image.width * image.height;
  const nearest = new Int32Array(pixelCount);
  nearest.fill(-1);
  const queue = [];
  for (let index = 0; index < pixelCount; index += 1) {
    const offset = index * 4;
    const alpha = image.data[offset + 3];
    const magentaDominance = Math.min(image.data[offset], image.data[offset + 2])
      - image.data[offset + 1];
    if (alpha < 245 || magentaDominance >= 20) continue;
    nearest[index] = index;
    queue.push(index);
  }
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const index = queue[cursor];
    forEachNeighbor(index, image.width, image.height, (next) => {
      if (nearest[next] >= 0 || alphaAt(image, next) === 0) return;
      nearest[next] = nearest[index];
      queue.push(next);
    });
  }
  for (let index = 0; index < pixelCount; index += 1) {
    const offset = index * 4;
    const alpha = image.data[offset + 3];
    if (alpha === 0) continue;
    const magentaDominance = Math.min(image.data[offset], image.data[offset + 2])
      - image.data[offset + 1];
    if (alpha >= 245 && magentaDominance < 20) continue;
    const sourceIndex = nearest[index];
    if (sourceIndex < 0) continue;
    const sourceOffset = sourceIndex * 4;
    image.data[offset] = image.data[sourceOffset];
    image.data[offset + 1] = image.data[sourceOffset + 1];
    image.data[offset + 2] = image.data[sourceOffset + 2];
  }
}

function enqueueExterior(index, visible, exterior, queue) {
  if (visible[index] || exterior[index]) return;
  exterior[index] = 1;
  queue.push(index);
}

function touchesExterior(index, exterior, width, height) {
  let touches = false;
  forEachNeighbor(index, width, height, (next) => {
    if (exterior[next]) touches = true;
  });
  return touches;
}

function resizeRgba(sourceImage, width, height) {
  const result = new PNG({ width, height, colorType: 6 });
  const scaleX = width / sourceImage.width;
  const scaleY = height / sourceImage.height;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const rgba = sampleRgba(sourceImage, (x + 0.5) / scaleX - 0.5, (y + 0.5) / scaleY - 0.5);
      writePixel(result, y * width + x, ...rgba);
    }
  }
  return result;
}

function sampleRgba(image, sourceX, sourceY) {
  const x0 = Math.max(0, Math.min(image.width - 1, Math.floor(sourceX)));
  const y0 = Math.max(0, Math.min(image.height - 1, Math.floor(sourceY)));
  const x1 = Math.max(0, Math.min(image.width - 1, x0 + 1));
  const y1 = Math.max(0, Math.min(image.height - 1, y0 + 1));
  const tx = sourceX - Math.floor(sourceX);
  const ty = sourceY - Math.floor(sourceY);
  const samples = [
    [x0, y0, (1 - tx) * (1 - ty)],
    [x1, y0, tx * (1 - ty)],
    [x0, y1, (1 - tx) * ty],
    [x1, y1, tx * ty],
  ];
  let alpha = 0;
  const premultiplied = [0, 0, 0];
  for (const [x, y, weight] of samples) {
    const offset = (y * image.width + x) * 4;
    const weightedAlpha = image.data[offset + 3] * weight;
    alpha += weightedAlpha;
    for (let channel = 0; channel < 3; channel += 1) {
      premultiplied[channel] += image.data[offset + channel] * weightedAlpha;
    }
  }
  if (alpha <= 0) return [0, 0, 0, 0];
  return [
    Math.round(premultiplied[0] / alpha),
    Math.round(premultiplied[1] / alpha),
    Math.round(premultiplied[2] / alpha),
    Math.round(alpha),
  ];
}

function blit(destination, left, top, sourceImage) {
  for (let y = 0; y < sourceImage.height; y += 1) {
    const destinationStart = ((top + y) * destination.width + left) * 4;
    const sourceStart = y * sourceImage.width * 4;
    sourceImage.data.copy(
      destination.data,
      destinationStart,
      sourceStart,
      sourceStart + sourceImage.width * 4,
    );
  }
}

function alphaBounds(image, x, y, width, height) {
  let minX = x + width;
  let minY = y + height;
  let maxX = -1;
  let maxY = -1;
  for (let py = y; py < y + height; py += 1) {
    for (let px = x; px < x + width; px += 1) {
      if (image.data[(py * image.width + px) * 4 + 3] < 1) continue;
      minX = Math.min(minX, px);
      minY = Math.min(minY, py);
      maxX = Math.max(maxX, px);
      maxY = Math.max(maxY, py);
    }
  }
  return maxX < 0 ? { x: 0, y: 0, width: 0, height: 0 } : {
    x: minX - x,
    y: minY - y,
    width: maxX - minX + 1,
    height: maxY - minY + 1,
  };
}

function forEachNeighbor(index, width, height, visit) {
  const x = index % width;
  const y = Math.floor(index / width);
  if (x > 0) visit(index - 1);
  if (x + 1 < width) visit(index + 1);
  if (y > 0) visit(index - width);
  if (y + 1 < height) visit(index + width);
}

function alphaAt(image, index) {
  return image.data[index * 4 + 3];
}

function writePixel(image, index, red, green, blue, alpha) {
  const offset = index * 4;
  image.data[offset] = red;
  image.data[offset + 1] = green;
  image.data[offset + 2] = blue;
  image.data[offset + 3] = alpha;
}
