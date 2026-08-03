import { COLORS, TERRAIN_VARIANT_PALETTES } from "./config.js";
import { TERRAIN, isRoadTerrain } from "./protocol.js";

const hash2 = (x, y) => {
  let n = (x * 374761393 + y * 668265263) | 0;
  n = (n ^ (n >>> 13)) | 0;
  n = Math.imul(n, 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
};

export const minimapTerrainStyleSignature = () => [
  COLORS.rock, COLORS.water, COLORS.minimapRoad, COLORS.minimapRoadLine,
  COLORS.field, COLORS.mud, COLORS.grass, COLORS.grassAlt,
  ...Object.values(TERRAIN_VARIANT_PALETTES).flatMap(({ base, alt }) => [base, alt]),
].join(",");

export const minimapRoadMarkingStyleSignature = () => COLORS.minimapRoadLine;

export const hasMinimapRoadMarkings = (map) => map.terrain.some((code) =>
  code !== TERRAIN.ROAD_BARE && isRoadTerrain(code));

export function paintMinimapRoadMarkings(ctx, map, scale, worldToCanvas) {
  const cell = map.tileSize * scale;
  ctx.save();
  ctx.fillStyle = `#${COLORS.minimapRoadLine.toString(16).padStart(6, "0")}`;
  for (let ty = 0; ty < map.height; ty++) {
    for (let tx = 0; tx < map.width; tx++) {
      const code = map.terrain[ty * map.width + tx];
      if (code === TERRAIN.ROAD_BARE || !isRoadTerrain(code)) continue;
      const p = worldToCanvas(tx * map.tileSize, ty * map.tileSize);
      ctx.beginPath();
      ctx.arc(p.x + cell / 2, p.y + cell / 2, Math.max(0.42, cell * 0.22), 0, Math.PI * 2);
      ctx.fill();
    }
  }
  ctx.restore();
}

// Per-terrain fill matching the production world palette.
export const minimapTerrainColor = (code, tx, ty) => {
  const variant = TERRAIN_VARIANT_PALETTES[code];
  if (variant) {
    const frost = variant.pattern === "frost";
    return hash2(frost ? Math.floor(tx / 4) : tx, frost ? Math.floor(ty / 4) : ty)
      > (frost ? 0.5 : 0.54) ? variant.alt : variant.base;
  }
  if (code === TERRAIN.ROCK) return COLORS.rock;
  if (code === TERRAIN.WATER) return COLORS.water;
  if (isRoadTerrain(code)) return COLORS.minimapRoad;
  const n = hash2(tx, ty);
  if (n > 0.78) return COLORS.field;
  if (n < 0.18) return COLORS.mud;
  return (tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt;
};
