// Shared authored-map limits. Keep these aligned with the authoritative Rust loader; both
// browser and Node recipe adapters consume this module instead of applying their own caps.
export const AUTHORED_MAP_MIN_DIMENSION_TILES = 16;
export const AUTHORED_MAP_MAX_DIMENSION_TILES = 256;
export const AUTHORED_MAP_MAX_START_LOCATIONS = 4;
export const AUTHORED_MAP_MAX_BASE_SITES = 32;
export const AUTHORED_MAP_MAX_STEEL_PATCHES = 36;
export const AUTHORED_MAP_MAX_OIL_PATCHES = 9;

export function boundedAuthoredPatchCount(value, max, fallback) {
  const count = Number(value);
  return Number.isInteger(count) ? Math.max(0, Math.min(max, count)) : fallback;
}
