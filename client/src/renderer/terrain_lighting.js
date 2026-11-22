/** Encode the cached terrain's lighting multiplier. Neutral 128 preserves the selected flat-map art. */
export function terrainLightingPixels(before, after, width, height) {
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let i = 0; i < pixels.length; i += 4) {
    for (let channel = 0; channel < 3; channel++) {
      pixels[i + channel] = Math.min(255, Math.round(128 * after[i + channel] / Math.max(1, before[i + channel])));
    }
    pixels[i + 3] = 255;
  }
  return { width, height, pixels };
}
