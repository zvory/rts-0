import { applyMapOperation, terrainCharacter } from "./operations.js";
import {
  AUTHORED_MAP_MAX_BASE_SITES,
  AUTHORED_MAP_MAX_DIMENSION_TILES,
  AUTHORED_MAP_MAX_START_LOCATIONS,
  AUTHORED_MAP_MIN_DIMENSION_TILES,
  MAP_AUTHORING_RECIPE_MAX_EXPLICIT_TILES_PER_OPERATION,
  MAP_AUTHORING_RECIPE_MAX_OPERATIONS,
  MAP_AUTHORING_RECIPE_MAX_PATH_POINTS,
  MAP_AUTHORING_RECIPE_MAX_TOTAL_EXPLICIT_TILES,
  MAP_AUTHORING_RECIPE_MAX_WORK_UNITS,
} from "./limits.js";
import { MAP_AUTHORING_SYMMETRY, symmetrySupported } from "./symmetry.js";

export const CURRENT_AUTHORED_MAP_VERSION = 6;
export const MAX_AUTHORED_MAP_DIMENSION_TILES = AUTHORED_MAP_MAX_DIMENSION_TILES;

export function isMapAuthoringRecipe(value) {
  return !!value && typeof value === "object" && !Array.isArray(value)
    && !Array.isArray(value.terrain)
    && Object.hasOwn(value, "width")
    && Object.hasOwn(value, "height")
    && (value.operations === undefined || Array.isArray(value.operations));
}

export function buildMapFromRecipe(recipe) {
  if (!recipe || typeof recipe !== "object" || Array.isArray(recipe)) throw new Error("Recipe must be a JSON object");
  const width = recipe.width;
  const height = recipe.height;
  if (!isUint32(width) || !isUint32(height) || width <= 0 || height <= 0) {
    throw new Error("Recipe width and height must be positive integers");
  }
  if (width > MAX_AUTHORED_MAP_DIMENSION_TILES || height > MAX_AUTHORED_MAP_DIMENSION_TILES) {
    throw new Error(`Recipe width and height must each be at most ${MAX_AUTHORED_MAP_DIMENSION_TILES} tiles`);
  }
  if (width < AUTHORED_MAP_MIN_DIMENSION_TILES || height < AUTHORED_MAP_MIN_DIMENSION_TILES) {
    throw new Error(`Recipe width and height must each be at least ${AUTHORED_MAP_MIN_DIMENSION_TILES} tiles`);
  }
  if (recipe.operations !== undefined && !Array.isArray(recipe.operations)) {
    throw new Error("Recipe operations must be an array when provided");
  }
  validateRecipeComplexity(recipe.operations || [], { width, height });
  const defaultSymmetry = recipe.symmetry || MAP_AUTHORING_SYMMETRY.NONE;
  validateRecipeSymmetry(defaultSymmetry, { width, height });
  const background = terrainCharacter(recipe.background || "grass");
  const map = {
    version: CURRENT_AUTHORED_MAP_VERSION,
    name: String(recipe.name || "Untitled map"),
    description: String(recipe.description || ""),
    width,
    height,
    terrain: Array.from({ length: height }, () => background.repeat(width)),
    startLocations: [],
    baseSites: [],
    _design: String(recipe.design || `Generated from a shared map-authoring recipe with ${recipe.symmetry || "no"} symmetry.`),
    doodads: [],
    stealthTiles: [],
    noVehicleTiles: [],
  };
  for (const operation of recipe.operations || []) {
    validateRecipeSymmetry(operation.symmetry ?? defaultSymmetry, map);
    applyMapOperation(map, operation, { defaultSymmetry });
    validateRecipeLocationCounts(map);
  }
  return map;
}

/** Reject recipes whose synchronous materialization cost exceeds the shared UI/CLI budget. */
export function validateRecipeComplexity(operations, dimensions) {
  if (operations.length > MAP_AUTHORING_RECIPE_MAX_OPERATIONS) {
    throw new Error(`Recipe operations must contain at most ${MAP_AUTHORING_RECIPE_MAX_OPERATIONS} entries`);
  }
  const mapArea = dimensions.width * dimensions.height;
  let explicitTileCount = 0;
  let workUnits = 0;
  for (let index = 0; index < operations.length; index += 1) {
    const operation = operations[index];
    if (!operation || typeof operation !== "object" || Array.isArray(operation)) {
      throw new Error(`Recipe operation ${index} must be a JSON object`);
    }
    const type = String(operation.type || "");
    if (type === "stroke" || type === "road") {
      if (!Array.isArray(operation.points)) {
        throw new Error(`Recipe operation ${index} points must be an array`);
      }
      if (operation.points.length > MAP_AUTHORING_RECIPE_MAX_PATH_POINTS) {
        throw new Error(
          `Recipe operation ${index} points must contain at most ${MAP_AUTHORING_RECIPE_MAX_PATH_POINTS} entries`,
        );
      }
      // pathTiles tests every candidate tile against every segment. Charging the full map is a
      // conservative, input-only bound that is cheap to compute before geometry materialization.
      workUnits += mapArea * (Math.max(1, operation.points.length) + 4);
    } else if (type === "fill") {
      workUnits += mapArea;
    } else if (type === "rect" || type === "blob") {
      workUnits += mapArea * 5;
    } else if (type === "paintTiles" || type === "overlayTiles") {
      const count = Array.isArray(operation.tiles) ? operation.tiles.length : 0;
      if (count > MAP_AUTHORING_RECIPE_MAX_EXPLICIT_TILES_PER_OPERATION) {
        throw new Error(
          `Recipe operation ${index} tiles must contain at most ${MAP_AUTHORING_RECIPE_MAX_EXPLICIT_TILES_PER_OPERATION} entries`,
        );
      }
      explicitTileCount += count;
      if (explicitTileCount > MAP_AUTHORING_RECIPE_MAX_TOTAL_EXPLICIT_TILES) {
        throw new Error(
          `Recipe explicit tiles must total at most ${MAP_AUTHORING_RECIPE_MAX_TOTAL_EXPLICIT_TILES} entries`,
        );
      }
      // The largest supported symmetry orbit has four copies.
      workUnits += count * 5;
    } else {
      workUnits += 4;
    }
    if (workUnits > MAP_AUTHORING_RECIPE_MAX_WORK_UNITS) {
      throw new Error(`Recipe estimated work exceeds the ${MAP_AUTHORING_RECIPE_MAX_WORK_UNITS}-unit limit`);
    }
  }
}

function validateRecipeLocationCounts(map) {
  if (map.startLocations.length > AUTHORED_MAP_MAX_START_LOCATIONS) {
    throw new Error(`Recipe generates more than ${AUTHORED_MAP_MAX_START_LOCATIONS} start locations`);
  }
  if (map.baseSites.length > AUTHORED_MAP_MAX_BASE_SITES) {
    throw new Error(`Recipe generates more than ${AUTHORED_MAP_MAX_BASE_SITES} base sites`);
  }
}

function validateRecipeSymmetry(symmetry, dimensions) {
  if (!Object.values(MAP_AUTHORING_SYMMETRY).includes(symmetry)) {
    throw new Error(`Unsupported symmetry ${JSON.stringify(symmetry)}`);
  }
  if (!symmetrySupported(dimensions, symmetry)) {
    throw new Error(`Symmetry ${JSON.stringify(symmetry)} requires a square map`);
  }
}

function isUint32(value) {
  return Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
}
