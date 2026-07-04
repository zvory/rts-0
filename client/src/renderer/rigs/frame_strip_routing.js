import { KIND } from "../../protocol.js";
import {
  applyFrameStripColorAdjustmentToRgba,
  frameStripRuntimeColorAdjustment,
  isNeutralFrameStripColorAdjustment,
} from "./frame_strip_color_profile.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "./machine_gunner_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "./rifleman_png_strip.js";

const LIVE_FRAME_STRIPS = Object.freeze([
  [KIND.MACHINE_GUNNER, MACHINE_GUNNER_PNG_FRAME_STRIP],
  [KIND.RIFLEMAN, RIFLEMAN_PNG_FRAME_STRIP],
]);

export function createLiveFrameStrips() {
  const strips = new Map();
  for (const [kind, strip] of LIVE_FRAME_STRIPS) {
    if (strip?.enabled) strips.set(kind, strip);
  }
  return strips;
}

export function liveFrameStripFor(strips, kind) {
  return strips?.get?.(kind) ?? null;
}

export function loadFrameStripTexture(pixi, strip) {
  if (!pixi || !strip?.image) return Promise.resolve(null);
  const adjustment = frameStripRuntimeColorAdjustment(strip);
  if (!isNeutralFrameStripColorAdjustment(adjustment)) {
    return loadAdjustedFrameStripTexture(pixi, strip, adjustment);
  }
  return loadRawFrameStripTexture(pixi, strip);
}

function loadRawFrameStripTexture(pixi, strip) {
  if (pixi.Assets?.load) return pixi.Assets.load(strip.image);
  const texture = pixi.Texture?.from?.(strip.image) ?? null;
  return Promise.resolve(texture);
}

async function loadAdjustedFrameStripTexture(pixi, strip, adjustment) {
  const doc = globalThis.document;
  if (!doc?.createElement || !globalThis.Image || !pixi.Texture?.from) {
    return loadRawFrameStripTexture(pixi, strip);
  }
  const image = await loadImage(strip.image);
  const width = Math.max(1, image.naturalWidth || image.width || strip.frameWidth || 1);
  const height = Math.max(1, image.naturalHeight || image.height || strip.frameHeight || 1);
  const canvas = doc.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext?.("2d", { willReadFrequently: true });
  if (!ctx) return loadRawFrameStripTexture(pixi, strip);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, width, height);
  ctx.drawImage(image, 0, 0, width, height);
  try {
    const imageData = ctx.getImageData(0, 0, width, height);
    applyFrameStripColorAdjustmentToRgba(imageData.data, adjustment);
    ctx.putImageData(imageData, 0, 0);
    return pixi.Texture.from(canvas);
  } catch (_err) {
    return loadRawFrameStripTexture(pixi, strip);
  }
}

function loadImage(src) {
  return new Promise((resolve, reject) => {
    const image = new globalThis.Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`failed to load frame strip image ${src}`));
    image.src = src;
  });
}
