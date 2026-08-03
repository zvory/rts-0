import {
  NEUTRAL_COLOR_ADJUSTMENT,
  applyColorAdjustmentToRgba,
  isNeutralColorAdjustment,
  normalizeColorAdjustment,
  relativeColorAdjustment,
} from "./color_adjustment.js";

export const NEUTRAL_FRAME_STRIP_COLOR_ADJUSTMENT = NEUTRAL_COLOR_ADJUSTMENT;

export const FRAME_STRIP_TARGET_COLOR_ADJUSTMENT = Object.freeze({
  brightness: 170,
  saturation: 118,
  hue: 100,
});

export function frameStripRuntimeColorAdjustment(strip, target = FRAME_STRIP_TARGET_COLOR_ADJUSTMENT) {
  const baked = normalizeFrameStripColorAdjustment(strip?.bakedColorAdjustment);
  const desired = normalizeFrameStripColorAdjustment(strip?.targetColorAdjustment ?? target, target);
  return relativeColorAdjustment(desired, baked);
}

export function normalizeFrameStripColorAdjustment(value, fallback = NEUTRAL_FRAME_STRIP_COLOR_ADJUSTMENT) {
  return normalizeColorAdjustment(value, fallback);
}

export function isNeutralFrameStripColorAdjustment(adjustment) {
  return isNeutralColorAdjustment(adjustment);
}

export function applyFrameStripColorAdjustmentToRgba(data, adjustment) {
  return applyColorAdjustmentToRgba(data, adjustment);
}
