import { RIFLEMAN_EXPERIMENTAL_FRAME_STRIP } from "./rigs/rifleman_experimental_png_strip.js";

export function setExperimentalGraphicsEnabled(enabled) {
  this._experimentalGraphicsEnabled = enabled === true;
  if (enabled) this._experimentalRiflemanFrameStrip();
}

export function _experimentalRiflemanFrameStrip() {
  if (!this._experimentalGraphicsEnabled) return null;
  if (!this._experimentalRiflemanStrip) {
    const strip = RIFLEMAN_EXPERIMENTAL_FRAME_STRIP;
    const texture = this._visualFrameStripTextureFor("rifleman", strip);
    if (texture) this._experimentalRiflemanStrip = { strip, texture };
  }
  return this._experimentalRiflemanStrip || null;
}
