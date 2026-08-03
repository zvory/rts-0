// Shared authored-map limits. Keep these aligned with the authoritative Rust loader; both
// browser and Node recipe adapters consume this module instead of applying their own caps.
export const AUTHORED_MAP_MIN_DIMENSION_TILES = 16;
export const AUTHORED_MAP_MAX_DIMENSION_TILES = 256;
export const AUTHORED_MAP_MAX_START_LOCATIONS = 4;
export const AUTHORED_MAP_MAX_BASE_SITES = 32;
export const AUTHORED_MAP_MAX_STEEL_PATCHES = 36;
export const AUTHORED_MAP_MAX_OIL_PATCHES = 9;

// Recipe inputs are synchronous in both the browser and Node. These caps are deliberately
// independent of the JSON byte limit: a compact polyline can otherwise turn one operation into
// billions of point-to-segment checks and freeze either adapter before a map exists.
export const MAP_AUTHORING_RECIPE_MAX_OPERATIONS = 1_024;
export const MAP_AUTHORING_RECIPE_MAX_PATH_POINTS = 2_048;
export const MAP_AUTHORING_RECIPE_MAX_EXPLICIT_TILES_PER_OPERATION = 65_536;
export const MAP_AUTHORING_RECIPE_MAX_TOTAL_EXPLICIT_TILES = 262_144;
export const MAP_AUTHORING_RECIPE_MAX_WORK_UNITS = 8_388_608;

export function boundedAuthoredPatchCount(value, max, fallback) {
  const count = Number(value);
  return Number.isInteger(count) ? Math.max(0, Math.min(max, count)) : fallback;
}
