import { COLORS } from "../config.js";
import { hash2 } from "./shared.js";
import {
  isImpassableAt,
  isImpassableTerrain,
  roadEdgeDirections,
  roadMarkingOrientation,
  terrainColor,
  terrainOverlayColor,
} from "./terrain_palette.js";

const TERRAIN_TEXTURE_DOWNSAMPLE = 4;

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
  if (!isImpassableAt(map, tx, ty - 1)) ctx.fillRect(x, y, ts, edge);
  if (!isImpassableAt(map, tx, ty + 1)) ctx.fillRect(x, y + ts - edge, ts, edge);
  if (!isImpassableAt(map, tx - 1, ty)) ctx.fillRect(x, y, edge, ts);
  if (!isImpassableAt(map, tx + 1, ty)) ctx.fillRect(x + ts - edge, y, edge, ts);
}

function drawTerrainTile(ctx, map, tx, ty, textureTileSize) {
  if (tx < 0 || ty < 0 || tx >= map.width || ty >= map.height) return;
  const code = map.terrain[ty * map.width + tx];
  const x = tx * textureTileSize;
  const y = ty * textureTileSize;
  const color = terrainColor(code, tx, ty);
  ctx.fillStyle = colorCss(color);
  ctx.fillRect(x, y, textureTileSize, textureTileSize);

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
  drawRoadShoulder(ctx, map, tx, ty, code, textureTileSize);
  drawRoadMarking(ctx, code, x, y, textureTileSize);
  fillImpassableEdge(ctx, map, tx, ty, code, textureTileSize);
}

function drawRoadShoulder(ctx, map, tx, ty, code, size) {
  const edges = roadEdgeDirections(map, tx, ty, code);
  if (!edges.length) return;

  const x = tx * size;
  const y = ty * size;
  const edgeWidth = Math.max(1, Math.floor(size * 0.14));
  const chipSize = Math.max(1, Math.floor(size * 0.1));
  for (const edge of edges) {
    ctx.fillStyle = colorCss(COLORS.roadShoulderDark, 0.94);
    if (edge === "north") ctx.fillRect(x, y, size, edgeWidth);
    if (edge === "south") ctx.fillRect(x, y + size - edgeWidth, size, edgeWidth);
    if (edge === "west") ctx.fillRect(x, y, edgeWidth, size);
    if (edge === "east") ctx.fillRect(x + size - edgeWidth, y, edgeWidth, size);

    for (let offset = 0; offset < size; offset += chipSize) {
      const horizontal = edge === "north" || edge === "south";
      const sampleX = horizontal ? tx * 41 + offset : tx * 41 + (edge === "east" ? 17 : 5);
      const sampleY = horizontal ? ty * 43 + (edge === "south" ? 19 : 7) : ty * 43 + offset;
      const n = hash2(sampleX, sampleY);
      if (n < 0.48) continue;
      const depth = n > 0.82 ? edgeWidth + chipSize : edgeWidth;
      ctx.fillStyle = colorCss(COLORS.roadShoulder, n > 0.82 ? 0.88 : 0.68);
      if (edge === "north") ctx.fillRect(x + offset, y, chipSize, depth);
      if (edge === "south") ctx.fillRect(x + offset, y + size - depth, chipSize, depth);
      if (edge === "west") ctx.fillRect(x, y + offset, depth, chipSize);
      if (edge === "east") ctx.fillRect(x + size - depth, y + offset, depth, chipSize);
    }
  }
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

/** Create a small DOM canvas using the same tile painter as the game map. */
export function createTerrainPreviewCanvas(code) {
  const tiles = 3;
  const textureTileSize = 8;
  const canvas = document.createElement("canvas");
  canvas.width = tiles * textureTileSize;
  canvas.height = tiles * textureTileSize;
  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) return null;
  ctx.imageSmoothingEnabled = false;
  const map = {
    width: tiles,
    height: tiles,
    terrain: Array(tiles * tiles).fill(code),
  };
  for (let ty = 0; ty < tiles; ty++) {
    for (let tx = 0; tx < tiles; tx++) {
      drawTerrainTile(ctx, map, tx, ty, textureTileSize);
    }
  }
  return canvas;
}

export function buildStaticMap(map, { preserveMapLayers = false } = {}) {
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
  const canvas = reusable ? this._terrainCanvas : document.createElement("canvas");
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

  const layer = this.layers.terrain;
  if (reusable) {
    this._terrainSprite.texture.baseTexture.update();
    this._terrainSprite.scale.set(ts / textureTileSize);
  } else {
    if (this._terrainSprite) {
      this._terrainSprite.destroy(true);
      layer.removeChildren();
    }
    const tex = PIXI.Texture.from(canvas, { scaleMode: PIXI.SCALE_MODES.NEAREST });
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
    drawTerrainTile(ctx, map, x, y, textureTileSize);
  }
  if (dirty.size) this._terrainSprite.texture.baseTexture.update();
  return dirty.size;
}
