import {
  hash2,
  isImpassableAt,
  isImpassableTerrain,
  terrainColor,
  terrainOverlayColor,
} from "./shared.js";

const TERRAIN_TEXTURE_DOWNSAMPLE = 8;

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

export function buildStaticMap(map, { preserveMapLayers = false } = {}) {
  this._map = { width: map.width, height: map.height, tileSize: map.tileSize, terrain: map.terrain };
  const ts = map.tileSize;
  const textureTileSize = Math.max(1, Math.round(ts / TERRAIN_TEXTURE_DOWNSAMPLE));
  const canvas = document.createElement("canvas");
  canvas.width = map.width * textureTileSize;
  canvas.height = map.height * textureTileSize;
  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) return;
  ctx.imageSmoothingEnabled = false;

  for (let ty = 0; ty < map.height; ty++) {
    for (let tx = 0; tx < map.width; tx++) {
      const code = map.terrain[ty * map.width + tx];
      const x = tx * textureTileSize;
      const y = ty * textureTileSize;
      const color = terrainColor(code, tx, ty);
      ctx.fillStyle = colorCss(color);
      ctx.fillRect(x, y, textureTileSize, textureTileSize);

      // Coarse texture blocks keep the ground readable while selling the
      // low-resolution PS1 look. No symbols or national markings are used.
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

      fillImpassableEdge(ctx, map, tx, ty, code, textureTileSize);
    }
  }

  const layer = this.layers.terrain;
  if (this._terrainSprite) {
    this._terrainSprite.destroy(true);
    layer.removeChildren();
  }
  const tex = PIXI.Texture.from(canvas, { scaleMode: PIXI.SCALE_MODES.NEAREST });
  this._terrainSprite = new PIXI.Sprite(tex);
  const scale = ts / textureTileSize;
  this._terrainSprite.scale.set(scale);
  layer.addChild(this._terrainSprite);
  if (!preserveMapLayers) {
    this._initGroundDecalsForMap?.(map);
    this._initTrenchesForMap?.(map);
  }
}

/** Replace only the cached terrain pixels for a browser-local map-editor preview. */
export function previewStaticTerrain(map) {
  return buildStaticMap.call(this, map, { preserveMapLayers: true });
}
