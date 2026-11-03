import { KIND } from "../../protocol.js";
import { loadColorAdjustedTexture } from "./color_adjusted_texture.js";
import { createWorkerSafeCanvas } from "../raster_primitives.js";
import { loadWorkerSafeTexture } from "../raster_primitives.js";
import { ANTI_TANK_GUN_PNG_RIG_ATLAS } from "./anti_tank_gun_png_atlas.js";
import { ARTILLERY_PNG_RIG_ATLAS } from "./artillery_png_atlas.js";
import { COMMAND_CAR_PNG_RIG_ATLAS } from "./command_car_png_atlas.js";
import { MORTAR_TEAM_PNG_RIG_ATLAS } from "./mortar_team_png_atlas.js";
import { SCOUT_CAR_PNG_RIG_ATLAS } from "./scout_car_png_atlas.js";
import { TANK_PNG_RIG_ATLAS } from "./tank_png_atlas.js";
import { ROCKET_TRUCK_PREVIEW_ATLAS } from "./rocket_truck_preview_atlas.js";

const LIVE_PNG_RIG_ATLASES = Object.freeze([
  [KIND.ROCKET_LAUNCHER, ROCKET_TRUCK_PREVIEW_ATLAS],
  [KIND.ANTI_TANK_GUN, ANTI_TANK_GUN_PNG_RIG_ATLAS],
  [KIND.ARTILLERY, ARTILLERY_PNG_RIG_ATLAS],
  [KIND.COMMAND_CAR, COMMAND_CAR_PNG_RIG_ATLAS],
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

export async function loadPngRigAtlasTexture(pixi, atlas) {
  if (!pixi || !atlas?.image) return Promise.resolve(null);
  const base = await loadColorAdjustedTexture(pixi, {
    image: atlas.image,
    adjustment: atlas.runtimeColorAdjustment,
    widthFallbacks: [atlas.grid?.width],
    heightFallbacks: [atlas.grid?.height],
    rawLoad: () => loadRawPngRigAtlasTexture(pixi, atlas),
    errorLabel: "PNG rig atlas image",
  });
  if (!atlas.attachments?.length) return base;
  try {
    const height = Math.max(atlas.grid.height, ...atlas.attachments.map((part) => part.y + part.source.h));
    const canvas = createWorkerSafeCanvas(atlas.grid.width, height);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("PNG rig attachment canvas is unavailable");
    context.imageSmoothingEnabled = false;
    context.drawImage(base.source.resource, 0, 0);
    for (const attachment of atlas.attachments) {
      const texture = await loadPngRigAtlasTexture(pixi, attachment.atlas);
      try {
        const frame = attachment.source;
        context.drawImage(texture.source.resource, frame.x, frame.y, frame.w, frame.h,
          attachment.x, attachment.y, frame.w, frame.h);
      } finally {
        texture.destroy(true);
      }
    }
    const composed = pixi.Texture.from(canvas);
    composed.rtsRendererOwnedTexture = true;
    composed.source.scaleMode = "nearest";
    return composed;
  } finally {
    base.destroy(true);
  }
}

function loadRawPngRigAtlasTexture(pixi, atlas) {
  return loadWorkerSafeTexture(pixi, atlas.image);
}
