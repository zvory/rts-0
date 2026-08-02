import { COLORS, TERRAIN_VARIANT_PALETTES } from "./config.js";
import { TERRAIN, isRoadTerrain } from "./protocol.js";

const hash2 = (x, y) => {
  let n = (x * 374761393 + y * 668265263) | 0;
  n = (n ^ (n >>> 13)) | 0;
  n = Math.imul(n, 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
};

export const minimapTerrainStyleSignature = () => [
  COLORS.rock, COLORS.water, COLORS.road, COLORS.roadAlt,
  COLORS.field, COLORS.mud, COLORS.grass, COLORS.grassAlt,
  ...Object.values(TERRAIN_VARIANT_PALETTES).flatMap(({ base, alt }) => [base, alt]),
].join(",");

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
  if (isRoadTerrain(code)) return hash2(tx, ty) > 0.6 ? COLORS.roadAlt : COLORS.road;
  const n = hash2(tx, ty);
  if (n > 0.78) return COLORS.field;
  if (n < 0.18) return COLORS.mud;
  return (tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt;
};
