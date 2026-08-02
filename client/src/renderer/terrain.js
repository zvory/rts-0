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
export const TERRAIN_BLEND_PRESETS = Object.freeze({
  "hard-chips": Object.freeze({ shape: "hard-chips", depth: 0.34, feather: false }),
  "hard-chips-wide": Object.freeze({ shape: "hard-chips", depth: 0.68, feather: false }),
  dither: Object.freeze({ shape: "dither", depth: 0.34, feather: false }),
  organic: Object.freeze({ shape: "organic", depth: 0.38, feather: false }),
  "organic-wide": Object.freeze({ shape: "organic", depth: 0.68, feather: false }),
  "soft-ramp": Object.freeze({ shape: "ramp", depth: 0.68, feather: true }),
  "soft-organic": Object.freeze({ shape: "organic", depth: 0.68, feather: true }),
});
export const TERRAIN_BLEND_MODES = Object.freeze(Object.keys(TERRAIN_BLEND_PRESETS));
export const DEFAULT_TERRAIN_BLEND_MODE = "hard-chips";

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

export function drawTerrainTile(ctx, map, tx, ty, textureTileSize, {
  terrainBlendMode = DEFAULT_TERRAIN_BLEND_MODE,
} = {}) {
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
  drawRoadMarking(ctx, code, x, y, textureTileSize);
  drawGroundTransitions(ctx, map, tx, ty, code, textureTileSize, terrainBlendMode);
  fillImpassableEdge(ctx, map, tx, ty, code, textureTileSize);
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

function drawGroundTransitions(ctx, map, tx, ty, code, size, mode) {
  const edges = groundTransitionEdges(map, tx, ty, code);
  if (!edges.length) return;
  const preset = TERRAIN_BLEND_PRESETS[mode] || TERRAIN_BLEND_PRESETS[DEFAULT_TERRAIN_BLEND_MODE];
  const x = tx * size;
  const y = ty * size;
  for (const edge of edges) {
    if (preset.shape === "dither") drawDitherTransition(ctx, edge, x, y, size, tx, ty, preset);
    else if (preset.shape === "organic") drawOrganicTransition(ctx, edge, x, y, size, tx, ty, preset);
    else if (preset.shape === "ramp") drawRampTransition(ctx, edge, x, y, size, preset);
    else drawHardChipTransition(ctx, edge, x, y, size, tx, ty, preset);
  }
}

function transitionDepth(size, preset) {
  return Math.min(size, Math.max(1, Math.ceil(size * preset.depth)));
}

function transitionAlpha(inward, maxDepth, feather) {
  if (!feather) return 1;
  if (maxDepth <= 1) return 0.82;
  return 0.86 - (inward / (maxDepth - 1)) * 0.68;
}

function drawHardChipTransition(ctx, edge, x, y, size, tx, ty, preset) {
  const maxDepth = transitionDepth(size, preset);
  for (let along = 0; along < size; along += 1) {
    const n = transitionNoise(edge, tx, ty, along, 0);
    if (n < 0.18) continue;
    const middleDepth = Math.max(2, Math.ceil(maxDepth * 0.67));
    const depth = Math.min(maxDepth, n > 0.92 ? maxDepth : n > 0.68 ? middleDepth : 1);
    for (let inward = 0; inward < depth; inward += 1) {
      fillTransitionPixel(ctx, edge, x, y, size, along, inward, transitionPixelColor(edge, along, inward));
    }
  }
}

function drawDitherTransition(ctx, edge, x, y, size, tx, ty, preset) {
  const probabilities = [0.76, 0.4, 0.14];
  const depth = Math.min(probabilities.length, transitionDepth(size, preset));
  for (let inward = 0; inward < depth; inward += 1) {
    for (let along = 0; along < size; along += 1) {
      const ordered = ((along & 1) * 2 + (inward & 1)) / 4;
      const jitter = transitionNoise(edge, tx, ty, along, inward) * 0.34;
      if (ordered * 0.66 + jitter >= probabilities[inward]) continue;
      fillTransitionPixel(ctx, edge, x, y, size, along, inward, transitionPixelColor(edge, along, inward));
    }
  }
}

function drawOrganicTransition(ctx, edge, x, y, size, tx, ty, preset) {
  const maxDepth = transitionDepth(size, preset);
  const raw = Array.from({ length: size }, (_, along) =>
    transitionNoise(edge, tx, ty, along, 1) * maxDepth);
  for (let along = 0; along < size; along += 1) {
    const before = raw[Math.max(0, along - 1)];
    const after = raw[Math.min(size - 1, along + 1)];
    const depth = Math.max(1, Math.min(maxDepth, Math.round(before * 0.25 + raw[along] * 0.5 + after * 0.25)));
    for (let inward = 0; inward < depth; inward += 1) {
      fillTransitionPixel(
        ctx,
        edge,
        x,
        y,
        size,
        along,
        inward,
        transitionPixelColor(edge, along, inward),
        transitionAlpha(inward, maxDepth, preset.feather),
      );
    }
    if (depth < maxDepth && transitionNoise(edge, tx, ty, along, 7) > 0.82) {
      fillTransitionPixel(
        ctx,
        edge,
        x,
        y,
        size,
        along,
        depth,
        transitionPixelColor(edge, along, depth),
        transitionAlpha(depth, maxDepth, preset.feather),
      );
    }
  }
}

function drawRampTransition(ctx, edge, x, y, size, preset) {
  const maxDepth = transitionDepth(size, preset);
  for (let inward = 0; inward < maxDepth; inward += 1) {
    const alpha = transitionAlpha(inward, maxDepth, true);
    for (let along = 0; along < size; along += 1) {
      fillTransitionPixel(
        ctx,
        edge,
        x,
        y,
        size,
        along,
        inward,
        transitionPixelColor(edge, along, inward),
        alpha,
      );
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

function fillTransitionPixel(ctx, edge, x, y, size, along, inward, color, alpha = 1) {
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
  ctx.fillStyle = colorCss(color, alpha);
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
  terrainBlendMode = DEFAULT_TERRAIN_BLEND_MODE,
} = {}) {
  this._map = {
    width: map.width,
    height: map.height,
    tileSize: map.tileSize,
    terrain: Array.from(map.terrain || []),
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
  this._terrainBlendMode = TERRAIN_BLEND_MODES.includes(terrainBlendMode)
    ? terrainBlendMode
    : DEFAULT_TERRAIN_BLEND_MODE;

  for (let ty = 0; ty < map.height; ty++) {
    for (let tx = 0; tx < map.width; tx++) {
      drawTerrainTile(ctx, this._map, tx, ty, textureTileSize, {
        terrainBlendMode: this._terrainBlendMode,
      });
    }
  }

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
  for (const key of dirty) {
    const [x, y] = key.split(",").map(Number);
    drawTerrainTile(ctx, map, x, y, textureTileSize, {
      terrainBlendMode: this._terrainBlendMode,
    });
  }
  if (dirty.size) this._terrainSprite.texture.source.update();
  return dirty.size;
}
