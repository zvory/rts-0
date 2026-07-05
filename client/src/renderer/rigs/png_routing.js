import { KIND } from "../../protocol.js";
import { loadColorAdjustedTexture } from "./color_adjusted_texture.js";
import { TANK_PNG_RIG_ATLAS } from "./tank_png_atlas.js";

const LIVE_PNG_RIG_ATLASES = Object.freeze([
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
  if (pixi.Assets?.load) return pixi.Assets.load(atlas.image);
  const texture = pixi.Texture?.from?.(atlas.image) ?? null;
  return Promise.resolve(texture);
}
