export function compareRgbaBuffers(legacy, rig, thresholds = {}) {
  validateBuffer("legacy", legacy);
  validateBuffer("rig", rig);
  if (legacy.width !== rig.width || legacy.height !== rig.height) {
    return {
      passed: false,
      reason: "dimension-mismatch",
      width: legacy.width,
      height: legacy.height,
      rigWidth: rig.width,
      rigHeight: rig.height,
      alphaWeightedMatchingRatio: 0,
      maxPerPixelRgbaDistance: Infinity,
      opaqueMismatchCount: Infinity,
      mismatchBounds: null,
      largestOpaqueMismatchClusterPx: Infinity,
    };
  }

  const dataA = legacy.data;
  const dataB = rig.data;
  const perChannelTolerance = finite(thresholds.perChannelTolerance, 6);
  const opaqueAlphaThreshold = finite(thresholds.opaqueAlphaThreshold, 128);
  let totalWeight = 0;
  let weightedMatch = 0;
  let maxPerPixelRgbaDistance = 0;
  const opaqueMismatches = new Set();
  const bounds = emptyBounds();

  for (let offset = 0; offset < dataA.length; offset += 4) {
    const dr = Math.abs(dataA[offset] - dataB[offset]);
    const dg = Math.abs(dataA[offset + 1] - dataB[offset + 1]);
    const db = Math.abs(dataA[offset + 2] - dataB[offset + 2]);
    const da = Math.abs(dataA[offset + 3] - dataB[offset + 3]);
    const distance = Math.hypot(dr, dg, db, da);
    maxPerPixelRgbaDistance = Math.max(maxPerPixelRgbaDistance, distance);

    const weight = Math.max(dataA[offset + 3], dataB[offset + 3]) / 255;
    if (weight > 0) {
      totalWeight += weight;
      weightedMatch += weight * Math.max(0, 1 - distance / 510);
    }

    const opaque = Math.max(dataA[offset + 3], dataB[offset + 3]) >= opaqueAlphaThreshold;
    const mismatched = dr > perChannelTolerance
      || dg > perChannelTolerance
      || db > perChannelTolerance
      || da > perChannelTolerance;
    if (opaque && mismatched) {
      const pixel = offset / 4;
      opaqueMismatches.add(pixel);
      includePixel(bounds, pixel % legacy.width, Math.floor(pixel / legacy.width));
    }
  }

  const alphaWeightedMatchingRatio = totalWeight > 0 ? weightedMatch / totalWeight : 1;
  const opaqueMismatchCount = opaqueMismatches.size;
  const largestOpaqueMismatchClusterPx = largestCluster(opaqueMismatches, legacy.width, legacy.height);
  const mismatchBounds = Number.isFinite(bounds.minX)
    ? {
      minX: bounds.minX,
      minY: bounds.minY,
      maxX: bounds.maxX,
      maxY: bounds.maxY,
      width: bounds.maxX - bounds.minX + 1,
      height: bounds.maxY - bounds.minY + 1,
    }
    : null;

  const limits = {
    minAlphaWeightedMatchingRatio: finite(thresholds.minAlphaWeightedMatchingRatio, 0.985),
    maxPerPixelRgbaDistance: finite(thresholds.maxPerPixelRgbaDistance, 96),
    maxOpaqueMismatchCount: finite(thresholds.maxOpaqueMismatchCount, 48),
    maxOpaqueMismatchClusterPx: finite(thresholds.maxOpaqueMismatchClusterPx, 12),
  };
  const passed = alphaWeightedMatchingRatio >= limits.minAlphaWeightedMatchingRatio
    && maxPerPixelRgbaDistance <= limits.maxPerPixelRgbaDistance
    && opaqueMismatchCount <= limits.maxOpaqueMismatchCount
    && largestOpaqueMismatchClusterPx <= limits.maxOpaqueMismatchClusterPx;

  return {
    passed,
    alphaWeightedMatchingRatio: round(alphaWeightedMatchingRatio),
    maxPerPixelRgbaDistance: round(maxPerPixelRgbaDistance),
    opaqueMismatchCount,
    mismatchBounds,
    largestOpaqueMismatchClusterPx,
    thresholds: limits,
  };
}

export function makeDiffRgba(legacy, rig, { perChannelTolerance = 6 } = {}) {
  validateBuffer("legacy", legacy);
  validateBuffer("rig", rig);
  if (legacy.width !== rig.width || legacy.height !== rig.height) {
    throw new Error("cannot create diff for buffers with different dimensions");
  }
  const out = new Uint8Array(legacy.data.length);
  for (let offset = 0; offset < out.length; offset += 4) {
    const dr = Math.abs(legacy.data[offset] - rig.data[offset]);
    const dg = Math.abs(legacy.data[offset + 1] - rig.data[offset + 1]);
    const db = Math.abs(legacy.data[offset + 2] - rig.data[offset + 2]);
    const da = Math.abs(legacy.data[offset + 3] - rig.data[offset + 3]);
    if (dr > perChannelTolerance || dg > perChannelTolerance || db > perChannelTolerance || da > perChannelTolerance) {
      out[offset] = 255;
      out[offset + 1] = Math.min(255, Math.max(dr, dg, db) * 3);
      out[offset + 2] = 0;
      out[offset + 3] = Math.max(160, da);
    }
  }
  return { width: legacy.width, height: legacy.height, data: out };
}

function validateBuffer(name, buffer) {
  if (!buffer || !Number.isInteger(buffer.width) || !Number.isInteger(buffer.height)) {
    throw new Error(`${name} pixel buffer must include integer width and height`);
  }
  if (!buffer.data || buffer.data.length !== buffer.width * buffer.height * 4) {
    throw new Error(`${name} pixel buffer has invalid RGBA length`);
  }
}

function largestCluster(mismatches, width, height) {
  let largest = 0;
  const seen = new Set();
  for (const start of mismatches) {
    if (seen.has(start)) continue;
    let size = 0;
    const stack = [start];
    seen.add(start);
    while (stack.length > 0) {
      const index = stack.pop();
      size += 1;
      const x = index % width;
      const y = Math.floor(index / width);
      for (const [nx, ny] of [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]]) {
        if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
        const next = ny * width + nx;
        if (mismatches.has(next) && !seen.has(next)) {
          seen.add(next);
          stack.push(next);
        }
      }
    }
    largest = Math.max(largest, size);
  }
  return largest;
}

function emptyBounds() {
  return { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
}

function includePixel(bounds, x, y) {
  bounds.minX = Math.min(bounds.minX, x);
  bounds.minY = Math.min(bounds.minY, y);
  bounds.maxX = Math.max(bounds.maxX, x);
  bounds.maxY = Math.max(bounds.maxY, y);
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}

function round(value) {
  if (!Number.isFinite(value)) return value;
  const rounded = Number(value.toFixed(6));
  return Object.is(rounded, -0) ? 0 : rounded;
}
