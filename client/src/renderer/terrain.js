import { COLORS } from "../config.js";
import { hash2 } from "./shared.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";
import {
  groundTransitionEdges,
  impassableEdgeDirections,
  isImpassableTerrain,
  roadMarkingOrientation,
  terrainColor,
  terrainOverlayColor,
  terrainVariantPalette,
} from "./terrain_palette.js";

const TERRAIN_TEXTURE_DOWNSAMPLE = 4;
const GROUND_TRANSITION_DEPTH = 0.56;
const RELIEF_LIGHT_X = -0.46;
const RELIEF_LIGHT_Y = -0.46;
const RELIEF_LIGHT_Z = 0.76;
const RELIEF_SLOPE_SCALE = 1.55;
const RELIEF_DIRECTIONAL_STRENGTH = 1.35;
const RELIEF_DIRECTIONAL_LIMIT = 0.34;

function colorCss(color, alpha = 1) {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return alpha >= 1 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${alpha})`;
}

function fillImpassableEdge(ctx, map, tx, ty, code, ts) {
  if (!isImpassableTerrain(code)) return;

  const edge = Math.max(1, Math.floor(ts * 0.16));
  const color = code === 2 ? 0x0c2028 : 0x24231f;
  const x = tx * ts;
  const y = ty * ts;
  ctx.fillStyle = colorCss(color, 0.72);
  const edges = impassableEdgeDirections(map, tx, ty, code);
  if (edges.includes("north")) ctx.fillRect(x, y, ts, edge);
  if (edges.includes("south")) ctx.fillRect(x, y + ts - edge, ts, edge);
  if (edges.includes("west")) ctx.fillRect(x, y, edge, ts);
  if (edges.includes("east")) ctx.fillRect(x + ts - edge, y, edge, ts);
}

export function drawTerrainTile(ctx, map, tx, ty, textureTileSize) {
  if (tx < 0 || ty < 0 || tx >= map.width || ty >= map.height) return;
  const code = map.terrain[ty * map.width + tx];
  const x = tx * textureTileSize;
  const y = ty * textureTileSize;
  const color = terrainColor(code, tx, ty);
  ctx.fillStyle = colorCss(color);
  ctx.fillRect(x, y, textureTileSize, textureTileSize);

  const variant = terrainVariantPalette(code);
  if (variant) {
    drawTerrainVariantDetails(ctx, variant, tx, ty, x, y, textureTileSize);
  } else {
    const blocks = textureTileSize >= 4 ? 4 : 2;
    const block = textureTileSize / blocks;
    for (let by = 0; by < blocks; by++) {
      for (let bx = 0; bx < blocks; bx++) {
        const n = hash2(tx * 17 + bx, ty * 17 + by);
        if (n < 0.42) continue;
        const overlay = terrainOverlayColor(code, n);
        ctx.fillStyle = colorCss(overlay, code === 2 ? 0.22 : 0.16);
        ctx.fillRect(x + bx * block, y + by * block, Math.ceil(block), Math.ceil(block));
      }
    }
  }
  drawGroundTransitions(ctx, map, tx, ty, code, textureTileSize);
  // Markings belong to the road surface and must remain legible above edge dither.
  drawRoadMarking(ctx, code, x, y, textureTileSize);
  fillImpassableEdge(ctx, map, tx, ty, code, textureTileSize);
}

function elevationAt(map, tx, ty) {
  const x = Math.max(0, Math.min(map.width - 1, tx));
  const y = Math.max(0, Math.min(map.height - 1, ty));
  return Number(map.elevation?.[y * map.width + x]) || 0;
}

/**
 * Soften tile elevations into one continuous C2 surface, then light that surface in-place. Cubic
 * B-spline sampling avoids visible level bands and never moves the ground plane or its hit targets.
 */
function drawElevationRelief(ctx, map, tilePixels) {
  const levels = map.elevation || [];
  let minHeight = Infinity;
  let maxHeight = -Infinity;
  for (const raw of levels) {
    const height = Number(raw) || 0;
    minHeight = Math.min(minHeight, height);
    maxHeight = Math.max(maxHeight, height);
  }
  if (!Number.isFinite(minHeight) || maxHeight <= minHeight) return;

  const width = map.width * tilePixels;
  const height = map.height * tilePixels;
  const image = ctx.getImageData(0, 0, width, height);
  const pixels = image.data;
  const heightRange = Math.max(1, maxHeight - minHeight);

  for (let py = 0; py < height; py += 1) {
    const sampleY = (py + 0.5) / tilePixels - 0.5;
    for (let px = 0; px < width; px += 1) {
      const sampleX = (px + 0.5) / tilePixels - 0.5;
      const surface = sampleElevationSurface(map, sampleX, sampleY);
      const normalX = -surface.dx * RELIEF_SLOPE_SCALE;
      const normalY = -surface.dy * RELIEF_SLOPE_SCALE;
      const inverseLength = 1 / Math.hypot(normalX, normalY, 1);
      const light = (
        normalX * RELIEF_LIGHT_X
        + normalY * RELIEF_LIGHT_Y
        + RELIEF_LIGHT_Z
      ) * inverseLength;
      const directional = clamp(
        (light - RELIEF_LIGHT_Z) * RELIEF_DIRECTIONAL_STRENGTH,
        -RELIEF_DIRECTIONAL_LIMIT,
        RELIEF_DIRECTIONAL_LIMIT,
      );
      const altitude = clamp((surface.height - minHeight) / heightRange, 0, 1);
      const ambientShape = clamp(-surface.laplacian * 0.045, -0.11, 0.09);
      const brightness = 1 + directional + ambientShape + (altitude - 0.5) * 0.075;
      const tint = altitude * 0.09;
      const cool = (1 - altitude) * 0.03;
      const index = (py * width + px) * 4;
      pixels[index] = clampByte(pixels[index] * brightness * (1 - tint - cool) + 232 * tint + 54 * cool);
      pixels[index + 1] = clampByte(pixels[index + 1] * brightness * (1 - tint - cool) + 211 * tint + 75 * cool);
      pixels[index + 2] = clampByte(pixels[index + 2] * brightness * (1 - tint - cool) + 159 * tint + 84 * cool);
    }
  }
  ctx.putImageData(image, 0, 0);
}

function sampleElevationSurface(map, x, y) {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const xBasis = cubicBasis(x - ix);
  const yBasis = cubicBasis(y - iy);
  let height = 0;
  let dx = 0;
  let dy = 0;
  let dxx = 0;
  let dyy = 0;
  for (let oy = 0; oy < 4; oy += 1) {
    for (let ox = 0; ox < 4; ox += 1) {
      const elevation = elevationAt(map, ix + ox - 1, iy + oy - 1);
      height += elevation * xBasis.weight[ox] * yBasis.weight[oy];
      dx += elevation * xBasis.derivative[ox] * yBasis.weight[oy];
      dy += elevation * xBasis.weight[ox] * yBasis.derivative[oy];
      dxx += elevation * xBasis.second[ox] * yBasis.weight[oy];
      dyy += elevation * xBasis.weight[ox] * yBasis.second[oy];
    }
  }
  return { height, dx, dy, laplacian: dxx + dyy };
}

function cubicBasis(t) {
  const t2 = t * t;
  const t3 = t2 * t;
  return {
    weight: [
      (1 - 3 * t + 3 * t2 - t3) / 6,
      (4 - 6 * t2 + 3 * t3) / 6,
      (1 + 3 * t + 3 * t2 - 3 * t3) / 6,
      t3 / 6,
    ],
    derivative: [
      (-3 + 6 * t - 3 * t2) / 6,
      (-12 * t + 9 * t2) / 6,
      (3 + 6 * t - 9 * t2) / 6,
      t2 / 2,
    ],
    second: [1 - t, -2 + 3 * t, 1 - 3 * t, t],
  };
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function drawTerrainVariantDetails(ctx, variant, tx, ty, x, y, size) {
  const pixel = Math.max(1, Math.floor(size / 8));
  const cells = Math.max(2, Math.floor(size / pixel));
  if (variant.pattern === "mud") {
    drawBrokenMudMarks(ctx, variant, tx, ty, x, y, pixel, cells);
    return;
  }
  if (variant.pattern === "frost") {
    drawFrostDetails(ctx, variant, tx, ty, x, y, pixel, cells);
    return;
  }
  for (let py = 0; py < cells; py += 1) {
    for (let px = 0; px < cells; px += 1) {
      const n = hash2(tx * 31 + px * 7, ty * 37 + py * 11);
      let detail = null;
      if (variant.pattern === "gravel") {
        if (n > 0.83) detail = variant.details[0];
        else if (n < 0.13) detail = variant.details[1];
      } else if (variant.pattern === "dirt") {
        if (n > 0.9) detail = variant.details[0];
        else if (n < 0.08) detail = variant.details[1];
      }
      if (detail == null) continue;
      ctx.fillStyle = colorCss(detail, 0.78);
      ctx.fillRect(x + px * pixel, y + py * pixel, pixel, pixel);
    }
  }
}

function drawFrostDetails(ctx, variant, tx, ty, x, y, pixel, cells) {
  if (hash2(tx * 211 + 127, ty * 223 + 131) < 0.28) {
    const length = 2 + Math.floor(hash2(tx * 227 + 137, ty * 229 + 139) * 3);
    const startX = Math.floor(hash2(tx * 233 + 149, ty * 239 + 151) * Math.max(1, cells - length + 1));
    const startY = Math.floor(hash2(tx * 241 + 157, ty * 251 + 163) * cells);
    ctx.fillStyle = colorCss(variant.details[0], 0.72);
    for (let offset = 0; offset < length; offset += 1) {
      const drift = offset === length - 1 && hash2(tx * 257, ty * 263) > 0.62 ? 1 : 0;
      const py = Math.min(cells - 1, startY + drift);
      ctx.fillRect(x + (startX + offset) * pixel, y + py * pixel, pixel, pixel);
    }
  }
  if (hash2(tx * 269 + 167, ty * 271 + 173) >= 0.1) return;
  const px = Math.floor(hash2(tx * 277 + 179, ty * 281 + 181) * cells);
  const py = Math.floor(hash2(tx * 283 + 191, ty * 293 + 193) * cells);
  ctx.fillStyle = colorCss(variant.details[1], 0.65);
  ctx.fillRect(x + px * pixel, y + py * pixel, pixel, pixel);
}

function drawBrokenMudMarks(ctx, variant, tx, ty, x, y, pixel, cells) {
  const activation = hash2(tx * 103 + 31, ty * 107 + 37);
  if (activation < variant.activity) {
    const length = Math.min(cells, 2 + Math.floor(hash2(tx * 109 + 41, ty * 113 + 43) * 4));
    const startX = Math.floor(hash2(tx * 127 + 47, ty * 131 + 53) * Math.max(1, cells - length + 1));
    const startY = Math.floor(hash2(tx * 137 + 59, ty * 139 + 61) * cells);
    const angled = hash2(tx * 149 + 67, ty * 151 + 71) > 0.8;
    ctx.fillStyle = colorCss(variant.details[0], 0.76);
    for (let offset = 0; offset < length; offset += 1) {
      const py = Math.max(0, Math.min(cells - 1, startY + (angled && offset >= Math.ceil(length / 2) ? 1 : 0)));
      ctx.fillRect(x + (startX + offset) * pixel, y + py * pixel, pixel, pixel);
    }
  }
  if (hash2(tx * 157 + 73, ty * 163 + 79) >= 0.12) return;
  const poolX = Math.floor(hash2(tx * 167 + 83, ty * 173 + 89) * Math.max(1, cells - 2));
  const poolY = Math.floor(hash2(tx * 179 + 97, ty * 181 + 101) * Math.max(1, cells - 1));
  const poolWidth = hash2(tx * 191 + 103, ty * 193 + 107) > 0.55 ? 3 : 2;
  ctx.fillStyle = colorCss(variant.details[0], 0.58);
  ctx.fillRect(x + poolX * pixel, y + poolY * pixel, Math.min(poolWidth, cells - poolX) * pixel, pixel);
  if (hash2(tx * 197 + 109, ty * 199 + 113) > 0.68 && poolY + 1 < cells) {
    ctx.fillRect(x + (poolX + 1) * pixel, y + (poolY + 1) * pixel, pixel, pixel);
  }
}

function drawGroundTransitions(ctx, map, tx, ty, code, size) {
  const edges = groundTransitionEdges(map, tx, ty, code);
  if (!edges.length) return;
  const x = tx * size;
  const y = ty * size;
  for (const edge of edges) {
    drawStochasticTransition(ctx, edge, x, y, size, tx, ty);
  }
}

function drawStochasticTransition(ctx, edge, x, y, size, tx, ty) {
  const depth = Math.min(size, Math.max(1, Math.ceil(size * GROUND_TRANSITION_DEPTH)));
  for (let inward = 0; inward < depth; inward += 1) {
    const progress = depth <= 1 ? 0 : inward / (depth - 1);
    const coverage = 0.92 - progress * 0.76;
    for (let along = 0; along < size; along += 1) {
      if (transitionNoise(edge, tx, ty, along, inward) >= coverage) continue;
      fillTransitionPixel(ctx, edge, x, y, size, along, inward, transitionPixelColor(edge, along, inward));
    }
  }
}

function transitionNoise(edge, tx, ty, along, inward) {
  const directionSeed = edge.direction === "north" ? 11 : edge.direction === "south" ? 23 : edge.direction === "west" ? 37 : 53;
  return hash2(tx * 97 + edge.tx * 31 + along * 7 + directionSeed, ty * 101 + edge.ty * 43 + inward * 13 + edge.code * 17);
}

function transitionPixelColor(edge, along, inward) {
  const n = hash2(edge.tx * 59 + along * 11, edge.ty * 61 + inward * 17 + edge.code * 19);
  return n > 0.78 ? terrainOverlayColor(edge.code, n) : terrainColor(edge.code, edge.tx, edge.ty);
}

function fillTransitionPixel(ctx, edge, x, y, size, along, inward, color) {
  let px = x + along;
  let py = y + inward;
  if (edge.direction === "south") py = y + size - 1 - inward;
  if (edge.direction === "west") {
    px = x + inward;
    py = y + along;
  }
  if (edge.direction === "east") {
    px = x + size - 1 - inward;
    py = y + along;
  }
  ctx.fillStyle = colorCss(color);
  ctx.fillRect(px, py, 1, 1);
}

function drawRoadMarking(ctx, code, x, y, size) {
  const orientation = roadMarkingOrientation(code);
  if (!orientation) return;
  const line = Math.max(1, Math.floor(size * 0.16));
  const middle = Math.floor((size - line) * 0.5);
  ctx.fillStyle = colorCss(COLORS.roadLine);
  if (orientation === "horizontal") {
    ctx.fillRect(x, y + middle, size, line);
  } else if (orientation === "vertical") {
    ctx.fillRect(x + middle, y, line, size);
  } else {
    const steps = Math.max(1, size - line);
    for (let offset = 0; offset <= steps; offset++) {
      const diagonalY = orientation === "diagonalNwSe" ? offset : steps - offset;
      ctx.fillRect(x + offset, y + diagonalY, line, line);
    }
  }
}

export function buildStaticMap(map, {
  preserveMapLayers = false,
} = {}) {
  this._map = {
    width: map.width,
    height: map.height,
    tileSize: map.tileSize,
    terrain: Array.from(map.terrain || []),
    elevation: Array.from(map.elevation || new Uint8Array(map.width * map.height)),
  };
  const ts = map.tileSize;
  const textureTileSize = Math.max(1, Math.round(ts / TERRAIN_TEXTURE_DOWNSAMPLE));
  const reusable = this._terrainCanvas
    && this._terrainCanvas.width === map.width * textureTileSize
    && this._terrainCanvas.height === map.height * textureTileSize
    && this._terrainSprite;
  const canvas = reusable ? this._terrainCanvas : createWorkerSafeCanvas();
  canvas.width = map.width * textureTileSize;
  canvas.height = map.height * textureTileSize;
  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) return;
  ctx.imageSmoothingEnabled = false;
  this._terrainCanvas = canvas;
  this._terrainContext = ctx;
  this._terrainTextureTileSize = textureTileSize;
  for (let ty = 0; ty < map.height; ty++) {
    for (let tx = 0; tx < map.width; tx++) {
      drawTerrainTile(ctx, this._map, tx, ty, textureTileSize);
    }
  }
  drawElevationRelief(ctx, this._map, textureTileSize);

  const layer = this.layers.terrain;
  if (reusable) {
    this._terrainSprite.texture.source.update();
    this._terrainSprite.scale.set(ts / textureTileSize);
  } else {
    if (this._terrainSprite) {
      this._terrainSprite.destroy(true);
      layer.removeChildren();
    }
    const tex = PIXI.Texture.from(canvas);
    this._terrainSprite = new PIXI.Sprite(tex);
    this._terrainSprite.scale.set(ts / textureTileSize);
    layer.addChild(this._terrainSprite);
  }
  if (!preserveMapLayers) {
    this._initGroundDecalsForMap?.(map);
    this._initTrenchesForMap?.(map);
  }
}

/** Replace only the cached terrain pixels for a browser-local map-editor preview. */
export function previewStaticTerrain(map) {
  return buildStaticMap.call(this, map, { preserveMapLayers: true });
}

/** Patch changed terrain tiles plus adjacent edge tiles into the existing canvas/texture. */
export function updateStaticTerrainTiles(changes) {
  const map = this._map;
  const ctx = this._terrainContext;
  const textureTileSize = this._terrainTextureTileSize;
  if (!map || !ctx || !textureTileSize || !this._terrainSprite || !Array.isArray(changes)) return 0;
  const dirty = new Set();
  for (const change of changes) {
    const x = Math.trunc(Number(change?.x));
    const y = Math.trunc(Number(change?.y));
    const code = Number(change?.code);
    if (x < 0 || y < 0 || x >= map.width || y >= map.height || !Number.isInteger(code)) continue;
    map.terrain[y * map.width + x] = code;
    dirty.add(`${x},${y}`);
    dirty.add(`${x - 1},${y}`);
    dirty.add(`${x + 1},${y}`);
    dirty.add(`${x},${y - 1}`);
    dirty.add(`${x},${y + 1}`);
  }
  const hasRelief = map.elevation.some((height) => height !== 0);
  if (hasRelief && dirty.size) {
    // This path is editor-only in practice; repainting avoids stacking the alpha-free relief pass.
    for (let y = 0; y < map.height; y += 1) {
      for (let x = 0; x < map.width; x += 1) drawTerrainTile(ctx, map, x, y, textureTileSize);
    }
    drawElevationRelief(ctx, map, textureTileSize);
  } else {
    for (const key of dirty) {
      const [x, y] = key.split(",").map(Number);
      drawTerrainTile(ctx, map, x, y, textureTileSize);
    }
  }
  if (dirty.size) this._terrainSprite.texture.source.update();
  return dirty.size;
}
