import {
  applyColorAdjustmentToRgba,
  isNeutralColorAdjustment,
} from "./color_adjustment.js";
import { createWorkerSafeCanvas, fetchImageBitmap } from "../raster_primitives.js";

export function loadColorAdjustedTexture(pixi, {
  image,
  adjustment,
  widthFallbacks = [],
  heightFallbacks = [],
  rawLoad,
  errorLabel = "image",
} = {}) {
  const loadRaw = typeof rawLoad === "function" ? rawLoad : () => Promise.resolve(null);
  if (!pixi || !image || isNeutralColorAdjustment(adjustment)) return loadRaw();
  return loadAdjustedTexture(pixi, {
    image,
    adjustment,
    widthFallbacks,
    heightFallbacks,
    rawLoad: loadRaw,
    errorLabel,
  });
}

async function loadAdjustedTexture(pixi, {
  image,
  adjustment,
  widthFallbacks,
  heightFallbacks,
  rawLoad,
  errorLabel,
}) {
  if (typeof globalThis.fetch !== "function" || typeof globalThis.createImageBitmap !== "function" || !pixi.Texture?.from) {
    return rawLoad();
  }
  let loadedImage = null;
  try {
    loadedImage = await loadImage(image, errorLabel);
    const width = firstPositiveDimension(
      loadedImage.naturalWidth,
      loadedImage.width,
      ...widthFallbacks,
      1,
    );
    const height = firstPositiveDimension(
      loadedImage.naturalHeight,
      loadedImage.height,
      ...heightFallbacks,
      1,
    );
    const textureCanvas = createWorkerSafeCanvas(width, height);
    const textureContext = textureCanvas.getContext?.("2d", { willReadFrequently: true });
    if (!textureContext) return rawLoad();
    textureContext.imageSmoothingEnabled = false;
    textureContext.clearRect(0, 0, width, height);
    textureContext.drawImage(loadedImage, 0, 0, width, height);
    const imageData = textureContext.getImageData(0, 0, width, height);
    applyColorAdjustmentToRgba(imageData.data, adjustment);
    textureContext.putImageData(imageData, 0, 0);
    const texture = pixi.Texture.from(textureCanvas);
    try {
      if (texture && typeof texture === "object") texture.rtsRendererOwnedTexture = true;
    } catch (_err) {
      // The adjusted texture is still usable if this implementation disallows custom properties.
    }
    return texture;
  } catch (_err) {
    return rawLoad();
  } finally {
    loadedImage?.close?.();
  }
}

function loadImage(src, errorLabel) {
  return fetchImageBitmap(src).catch((error) => {
    throw new Error(`failed to load ${errorLabel} ${src}: ${error?.message || error}`);
  });
}

function firstPositiveDimension(...values) {
  for (const value of values) {
    if (Number.isFinite(value) && value > 0) return value;
  }
  return 1;
}
