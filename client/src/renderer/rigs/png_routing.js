import { KIND } from "../../protocol.js";
import { loadColorAdjustedTexture } from "./color_adjusted_texture.js";
import { loadWorkerSafeTexture } from "../raster_primitives.js";
import { ANTI_TANK_GUN_PNG_RIG_ATLAS } from "./anti_tank_gun_png_atlas.js";
import { ARTILLERY_PNG_RIG_ATLAS } from "./artillery_png_atlas.js";
import { MORTAR_TEAM_PNG_RIG_ATLAS } from "./mortar_team_png_atlas.js";
import { SCOUT_CAR_PNG_RIG_ATLAS } from "./scout_car_png_atlas.js";
import { TANK_PNG_RIG_ATLAS } from "./tank_png_atlas.js";

const LIVE_PNG_RIG_ATLASES = Object.freeze([
  [KIND.ANTI_TANK_GUN, ANTI_TANK_GUN_PNG_RIG_ATLAS],
  [KIND.ARTILLERY, ARTILLERY_PNG_RIG_ATLAS],
  [KIND.MORTAR_TEAM, MORTAR_TEAM_PNG_RIG_ATLAS],
  [KIND.SCOUT_CAR, SCOUT_CAR_PNG_RIG_ATLAS],
  [KIND.TANK, TANK_PNG_RIG_ATLAS],
]);

export function createLivePngRigAtlases() {
  const atlases = new Map();
  for (const [kind, atlas] of LIVE_PNG_RIG_ATLASES) {
    if (atlas?.enabled) atlases.set(kind, atlas);
  }
  return atlases;
}

export function livePngRigAtlasFor(atlases, kind) {
  return atlases?.get?.(kind) ?? null;
}

export function loadPngRigAtlasTexture(pixi, atlas) {
  if (!pixi || !atlas?.image) return Promise.resolve(null);
  return loadColorAdjustedTexture(pixi, {
    image: atlas.image,
    adjustment: atlas.runtimeColorAdjustment,
    widthFallbacks: [atlas.grid?.width],
    heightFallbacks: [atlas.grid?.height],
    rawLoad: () => loadRawPngRigAtlasTexture(pixi, atlas),
    errorLabel: "PNG rig atlas image",
  });
}

function loadRawPngRigAtlasTexture(pixi, atlas) {
  return loadWorkerSafeTexture(pixi, atlas.image);
}
