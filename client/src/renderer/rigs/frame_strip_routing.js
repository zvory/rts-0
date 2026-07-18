import { KIND } from "../../protocol.js";
import { loadColorAdjustedTexture } from "./color_adjusted_texture.js";
import { frameStripRuntimeColorAdjustment } from "./frame_strip_color_profile.js";
import { LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "./machine_gunner_png_strip.js";
import { RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP } from "./rifleman_panzerfaust_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "./rifleman_png_strip.js";
import { SCOUT_PLANE_PNG_FRAME_STRIP } from "./scout_plane_png_strip.js";

const LIVE_FRAME_STRIPS = Object.freeze([
  [KIND.MACHINE_GUNNER, MACHINE_GUNNER_PNG_FRAME_STRIP],
  [LOADED_RIFLEMAN_RIG_KEY, RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP],
  [KIND.RIFLEMAN, RIFLEMAN_PNG_FRAME_STRIP],
  [KIND.SCOUT_PLANE, SCOUT_PLANE_PNG_FRAME_STRIP],
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
  const frameWidth = positiveDimension(strip.frameWidth, 1);
  const frameHeight = positiveDimension(strip.frameHeight, 1);
  const frameCount = Math.max(1, Math.trunc(positiveDimension(strip.frameCount, 1)));
  return loadColorAdjustedTexture(pixi, {
    image: strip.image,
    adjustment,
    widthFallbacks: [frameWidth * frameCount],
    heightFallbacks: [frameHeight],
    rawLoad: () => loadRawFrameStripTexture(pixi, strip),
    errorLabel: "frame strip image",
  });
}

function loadRawFrameStripTexture(pixi, strip) {
  if (pixi.Assets?.load) return pixi.Assets.load(strip.image);
  const texture = pixi.Texture?.from?.(strip.image) ?? null;
  return Promise.resolve(texture);
}

function positiveDimension(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}
