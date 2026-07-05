import { KIND } from "../../protocol.js";
import {
  applyFrameStripColorAdjustmentToRgba,
  isNeutralFrameStripColorAdjustment,
} from "./frame_strip_color_profile.js";
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
  if (!isNeutralFrameStripColorAdjustment(atlas.runtimeColorAdjustment)) {
    return loadAdjustedPngRigAtlasTexture(pixi, atlas);
  }
  return loadRawPngRigAtlasTexture(pixi, atlas);
}

function loadRawPngRigAtlasTexture(pixi, atlas) {
  if (pixi.Assets?.load) return pixi.Assets.load(atlas.image);
  const texture = pixi.Texture?.from?.(atlas.image) ?? null;
  return Promise.resolve(texture);
}

async function loadAdjustedPngRigAtlasTexture(pixi, atlas) {
  const doc = globalThis.document;
  if (!doc?.createElement || !globalThis.Image || !pixi.Texture?.from) {
    return loadRawPngRigAtlasTexture(pixi, atlas);
  }
  try {
    const image = await loadImage(atlas.image);
    const width = positiveDimension(
      image.naturalWidth,
      positiveDimension(image.width, positiveDimension(atlas.grid?.width, 1)),
    );
    const height = positiveDimension(
      image.naturalHeight,
      positiveDimension(image.height, positiveDimension(atlas.grid?.height, 1)),
    );
    const canvas = doc.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext?.("2d", { willReadFrequently: true });
    if (!ctx) return loadRawPngRigAtlasTexture(pixi, atlas);
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, width, height);
    ctx.drawImage(image, 0, 0, width, height);
    const imageData = ctx.getImageData(0, 0, width, height);
    applyFrameStripColorAdjustmentToRgba(imageData.data, atlas.runtimeColorAdjustment);
    ctx.putImageData(imageData, 0, 0);
    return pixi.Texture.from(canvas);
  } catch (_err) {
    return loadRawPngRigAtlasTexture(pixi, atlas);
  }
}

function loadImage(src) {
  return new Promise((resolve, reject) => {
    const image = new globalThis.Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`failed to load PNG rig atlas image ${src}`));
    image.src = src;
  });
}

function positiveDimension(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}
