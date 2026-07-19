import { gfxNoFill, gfxRect, gfxFill } from "./native_graphics.js";
import { COLORS } from "../config.js";
import { PASSABLE, TERRAIN, isRoadTerrain } from "../protocol.js";
import { hash2 } from "./shared.js";

/** Base color for a terrain tile code. Codes match server terrain constants. */
export function terrainColor(code, tx, ty) {
  if (code === TERRAIN.ROCK) return COLORS.rock;
  if (code === TERRAIN.WATER) return COLORS.water;
  if (isRoadTerrain(code)) return hash2(tx, ty) > 0.6 ? COLORS.roadAlt : COLORS.road;
  const n = hash2(tx, ty);
  if (n > 0.78) return COLORS.field;
  if (n < 0.18) return COLORS.mud;
  return (tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt;
}

/** Muted overlay tint for blocky terrain texture. */
export function terrainOverlayColor(code, n) {
  if (code === TERRAIN.ROCK) return n > 0.74 ? 0x8a8777 : 0x4f4c43;
  if (code === TERRAIN.WATER) return n > 0.74 ? 0x527482 : 0x1d3d48;
  if (isRoadTerrain(code)) return n > 0.74 ? 0x4b4c46 : 0x242522;
  return n > 0.74 ? 0x817555 : 0x343127;
}

/** Exposed road sides, including map boundaries, for the cached terrain shoulder pass. */
export function roadEdgeDirections(map, tx, ty, code) {
  if (!isRoadTerrain(code)) return [];
  const roadAt = (x, y) => x >= 0
    && y >= 0
    && x < map.width
    && y < map.height
    && isRoadTerrain(map.terrain[y * map.width + x]);
  const edges = [];
  if (!roadAt(tx, ty - 1)) edges.push("north");
  if (!roadAt(tx, ty + 1)) edges.push("south");
  if (!roadAt(tx - 1, ty)) edges.push("west");
  if (!roadAt(tx + 1, ty)) edges.push("east");
  return edges;
}

/** Draw dark perimeter strips only where impassable terrain borders passable ground. */
export function drawImpassableEdge(g, map, tx, ty, code, ts) {
  if (!isImpassableTerrain(code)) return;

  const edge = Math.max(3, Math.floor(ts * 0.16));
  const color = code === TERRAIN.WATER ? 0x0c2028 : 0x24231f;
  const x = tx * ts;
  const y = ty * ts;

  gfxFill(g, color, 0.72);
  if (!isImpassableAt(map, tx, ty - 1)) gfxRect(g, x, y, ts, edge);
  if (!isImpassableAt(map, tx, ty + 1)) gfxRect(g, x, y + ts - edge, ts, edge);
  if (!isImpassableAt(map, tx - 1, ty)) gfxRect(g, x, y, edge, ts);
  if (!isImpassableAt(map, tx + 1, ty)) gfxRect(g, x + ts - edge, y, edge, ts);
  gfxNoFill(g);
}

export function isImpassableAt(map, tx, ty) {
  if (tx < 0 || ty < 0 || tx >= map.width || ty >= map.height) return false;
  return isImpassableTerrain(map.terrain[ty * map.width + tx]);
}

export function isImpassableTerrain(code) {
  return PASSABLE[code] !== true;
}

export function roadMarkingOrientation(code) {
  if (code === TERRAIN.ROAD_HORIZONTAL) return "horizontal";
  if (code === TERRAIN.ROAD_VERTICAL) return "vertical";
  if (code === TERRAIN.ROAD_DIAGONAL_NW_SE) return "diagonalNwSe";
  if (code === TERRAIN.ROAD_DIAGONAL_NE_SW) return "diagonalNeSw";
  return null;
}
