import { drawTerrainTile } from "./renderer/terrain.js";

export function createMapEditorTerrainPreview(code, documentRef = globalThis.document) {
  const tiles = 3;
  const textureTileSize = 8;
  const canvas = documentRef?.createElement?.("canvas");
  if (!canvas) return null;
  canvas.width = tiles * textureTileSize;
  canvas.height = tiles * textureTileSize;
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) return null;
  context.imageSmoothingEnabled = false;
  const map = { width: tiles, height: tiles, terrain: Array(tiles * tiles).fill(code) };
  for (let y = 0; y < tiles; y += 1) {
    for (let x = 0; x < tiles; x += 1) drawTerrainTile(context, map, x, y, textureTileSize);
  }
  return canvas;
}
