export const NEUTRAL_COLOR_ADJUSTMENT = Object.freeze({
  brightness: 100,
  saturation: 100,
  hue: 100,
});

const EPSILON = 0.001;

export function normalizeColorAdjustment(value, fallback = NEUTRAL_COLOR_ADJUSTMENT) {
  const source = value && typeof value === "object" ? value : fallback;
  return {
    brightness: positivePercent(source.brightness, fallback.brightness),
    saturation: positivePercent(source.saturation, fallback.saturation),
    hue: positivePercent(source.hue, fallback.hue),
  };
}

export function isNeutralColorAdjustment(adjustment) {
  const normalized = normalizeColorAdjustment(adjustment);
  return (
    nearly(normalized.brightness, 100) &&
    nearly(normalized.saturation, 100) &&
    nearly(normalized.hue, 100)
  );
}

export function applyColorAdjustmentToRgba(data, adjustment) {
  const normalized = normalizeColorAdjustment(adjustment);
  if (!data || isNeutralColorAdjustment(normalized)) return data;

  const brightness = normalized.brightness / 100;
  const saturation = normalized.saturation / 100;
  const hueDegrees = (normalized.hue - 100) * 1.8;
  const rotateHue = Math.abs(hueDegrees) > EPSILON;

  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;
    let r = data[i];
    let g = data[i + 1];
    let b = data[i + 2];
    if (rotateHue) [r, g, b] = hueRotateRgb(r, g, b, hueDegrees);
    if (!nearly(saturation, 1)) {
      const luma = luminance(r, g, b);
      r = luma + (r - luma) * saturation;
      g = luma + (g - luma) * saturation;
      b = luma + (b - luma) * saturation;
    }
    if (!nearly(brightness, 1)) {
      r *= brightness;
      g *= brightness;
      b *= brightness;
    }
    data[i] = clampByte(r);
    data[i + 1] = clampByte(g);
    data[i + 2] = clampByte(b);
  }
  return data;
}

function hueRotateRgb(r, g, b, degrees) {
  const angle = (degrees * Math.PI) / 180;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  return [
    r * (0.213 + cos * 0.787 - sin * 0.213) +
      g * (0.715 - cos * 0.715 - sin * 0.715) +
      b * (0.072 - cos * 0.072 + sin * 0.928),
    r * (0.213 - cos * 0.213 + sin * 0.143) +
      g * (0.715 + cos * 0.285 + sin * 0.140) +
      b * (0.072 - cos * 0.072 - sin * 0.283),
    r * (0.213 - cos * 0.213 - sin * 0.787) +
      g * (0.715 - cos * 0.715 + sin * 0.715) +
      b * (0.072 + cos * 0.928 + sin * 0.072),
  ];
}

function luminance(r, g, b) {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function positivePercent(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function nearly(a, b) {
  return Math.abs(a - b) <= EPSILON;
}
