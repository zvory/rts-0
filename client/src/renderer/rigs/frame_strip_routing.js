import { KIND } from "../../protocol.js";
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
  if (pixi.Assets?.load) return pixi.Assets.load(strip.image);
  const texture = pixi.Texture?.from?.(strip.image) ?? null;
  return Promise.resolve(texture);
}
