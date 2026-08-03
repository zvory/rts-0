import { COLORS } from "./config.js";
import { minimapTerrainColor } from "./minimap_terrain.js";

const hex = (value) => `#${value.toString(16).padStart(6, "0")}`;

export function minimapMapTransform(map, canvasWidth, canvasHeight) {
  const width = positiveSize(canvasWidth, "minimap width");
  const height = positiveSize(canvasHeight, "minimap height");
  const tileSize = positiveSize(map?.tileSize, "map tile size");
  const mapWidth = positiveSize(map?.width, "map width") * tileSize;
  const mapHeight = positiveSize(map?.height, "map height") * tileSize;
  const scale = Math.min(width / mapWidth, height / mapHeight);
  return Object.freeze({
    scale,
    offX: (width - mapWidth * scale) / 2,
    offY: (height - mapHeight * scale) / 2,
  });
}

export function paintMinimapTerrain(ctx, map, {
  scale,
  offX,
  offY,
} = minimapMapTransform(map, ctx?.canvas?.width, ctx?.canvas?.height)) {
  if (!ctx || !map || !Array.isArray(map.terrain)) {
    throw new TypeError("Minimap terrain painting requires a 2D context and materialized map.");
  }
  if (![scale, offX, offY].every(Number.isFinite) || scale <= 0) {
    throw new RangeError("Minimap terrain transform must be finite and positive.");
  }
  const requiredTiles = map.width * map.height;
  if (map.terrain.length !== requiredTiles) {
    throw new RangeError("Minimap terrain does not match the declared map dimensions.");
  }
  const cellWidth = map.tileSize * scale + 1;
  const cellHeight = map.tileSize * scale + 1;
  for (let ty = 0; ty < map.height; ty += 1) {
    for (let tx = 0; tx < map.width; tx += 1) {
      ctx.fillStyle = hex(minimapTerrainColor(map.terrain[ty * map.width + tx], tx, ty));
      ctx.fillRect(
        offX + tx * map.tileSize * scale,
        offY + ty * map.tileSize * scale,
        cellWidth,
        cellHeight,
      );
    }
  }
  return { scale, offX, offY };
}

export function paintMinimapMap(ctx, map) {
  if (!ctx?.canvas) throw new TypeError("Minimap map painting requires a canvas-backed 2D context.");
  ctx.fillStyle = hex(COLORS.bgVoid);
  ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  return paintMinimapTerrain(ctx, map, minimapMapTransform(map, ctx.canvas.width, ctx.canvas.height));
}

function positiveSize(value, label) {
  const size = Number(value);
  if (!Number.isSafeInteger(size) || size <= 0) throw new RangeError(`${label} must be a positive integer.`);
  return size;
}
