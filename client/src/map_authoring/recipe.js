import { applyMapOperation, terrainCharacter } from "./operations.js";
import { MAP_AUTHORING_SYMMETRY, symmetrySupported } from "./symmetry.js";

export const CURRENT_AUTHORED_MAP_VERSION = 6;
export const MAX_AUTHORED_MAP_DIMENSION_TILES = 256;

export function isMapAuthoringRecipe(value) {
  return !!value && typeof value === "object" && !Array.isArray(value)
    && Array.isArray(value.operations) && !Array.isArray(value.terrain);
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
  }
  return map;
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
