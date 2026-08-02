import { gfxNoFill, gfxRect, gfxFill } from "./native_graphics.js";
import { COLORS, TERRAIN_VARIANT_PALETTES } from "../config.js";
import { PASSABLE, TERRAIN, isRoadTerrain } from "../protocol.js";
import { hash2 } from "./shared.js";

/** Base color for a terrain tile code. Codes match server terrain constants. */
export function terrainColor(code, tx, ty) {
  const variant = terrainVariantPalette(code);
  if (variant) {
    if (variant.pattern === "frost") {
      return hash2(Math.floor(tx / 4), Math.floor(ty / 4)) > 0.5 ? variant.alt : variant.base;
    }
    return hash2(tx, ty) > 0.54 ? variant.alt : variant.base;
  }
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
  const variant = terrainVariantPalette(code);
  if (variant) return n > 0.74 ? variant.details[0] : variant.details[1];
  if (code === TERRAIN.ROCK) return n > 0.74 ? 0x8a8777 : 0x4f4c43;
  if (code === TERRAIN.WATER) return n > 0.74 ? 0x527482 : 0x1d3d48;
  if (isRoadTerrain(code)) return n > 0.74 ? 0x4b4c46 : 0x242522;
  return n > 0.74 ? 0x817555 : 0x343127;
}

export function terrainVariantPalette(code) {
  return TERRAIN_VARIANT_PALETTES[code] || null;
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

const CARDINAL_NEIGHBORS = Object.freeze([
  Object.freeze({ direction: "north", dx: 0, dy: -1 }),
  Object.freeze({ direction: "south", dx: 0, dy: 1 }),
  Object.freeze({ direction: "west", dx: -1, dy: 0 }),
  Object.freeze({ direction: "east", dx: 1, dy: 0 }),
]);

/** Stable visual identity for terrain transitions. Road markings share one surface. */
export function terrainMaterial(code) {
  if (isImpassableTerrain(code)) return null;
  return isRoadTerrain(code) ? "road" : `terrain:${code}`;
}

/**
 * Higher-ranked passable material creeps into lower-ranked material. Keeping this
 * directional makes each shared edge one texture band wide instead of painting both tiles.
 */
export function terrainBlendRank(code) {
  if (isRoadTerrain(code)) return 0;
  if (code === TERRAIN.GRASS) return 1;
  if (code >= TERRAIN.GRAVEL_A && code <= TERRAIN.FROSTED_GROUND) return code - 6;
  return -1;
}

export function groundTransitionEdges(map, tx, ty, code) {
  const material = terrainMaterial(code);
  const rank = terrainBlendRank(code);
  if (material == null || rank < 0) return [];
  const edges = [];
  for (const neighbor of CARDINAL_NEIGHBORS) {
    const nx = tx + neighbor.dx;
    const ny = ty + neighbor.dy;
    if (nx < 0 || ny < 0 || nx >= map.width || ny >= map.height) continue;
    const neighborCode = map.terrain[ny * map.width + nx];
    const neighborMaterial = terrainMaterial(neighborCode);
    if (
      neighborMaterial == null ||
      neighborMaterial === material ||
      terrainBlendRank(neighborCode) <= rank
    ) continue;
    edges.push({ ...neighbor, code: neighborCode, tx: nx, ty: ny });
  }
  return edges;
}

/**
 * Blocker perimeter edges. Unlike blockers get one shared separator owned by the
 * higher terrain code (water today), preventing a double-width rock/water seam.
 */
export function impassableEdgeDirections(map, tx, ty, code) {
  if (!isImpassableTerrain(code)) return [];
  const edges = [];
  for (const neighbor of CARDINAL_NEIGHBORS) {
    const nx = tx + neighbor.dx;
    const ny = ty + neighbor.dy;
    if (nx < 0 || ny < 0 || nx >= map.width || ny >= map.height) {
      edges.push(neighbor.direction);
      continue;
    }
    const neighborCode = map.terrain[ny * map.width + nx];
    if (!isImpassableTerrain(neighborCode) || neighborCode < code) {
      edges.push(neighbor.direction);
    }
  }
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
  const edges = impassableEdgeDirections(map, tx, ty, code);
  if (edges.includes("north")) gfxRect(g, x, y, ts, edge);
  if (edges.includes("south")) gfxRect(g, x, y + ts - edge, ts, edge);
  if (edges.includes("west")) gfxRect(g, x, y, edge, ts);
  if (edges.includes("east")) gfxRect(g, x + ts - edge, y, edge, ts);
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
