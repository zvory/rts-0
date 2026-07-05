import {
  applyColorAdjustmentToRgba,
  isNeutralColorAdjustment,
} from "./color_adjustment.js";

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
  const doc = globalThis.document;
  if (!doc?.createElement || !globalThis.Image || !pixi.Texture?.from) {
    return rawLoad();
  }
  try {
    const loadedImage = await loadImage(image, errorLabel);
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
    const canvas = doc.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext?.("2d", { willReadFrequently: true });
    if (!ctx) return rawLoad();
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, width, height);
    ctx.drawImage(loadedImage, 0, 0, width, height);
    const imageData = ctx.getImageData(0, 0, width, height);
    applyColorAdjustmentToRgba(imageData.data, adjustment);
    ctx.putImageData(imageData, 0, 0);
    return pixi.Texture.from(canvas);
  } catch (_err) {
    return rawLoad();
  }
}

function loadImage(src, errorLabel) {
  return new Promise((resolve, reject) => {
    const image = new globalThis.Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`failed to load ${errorLabel} ${src}`));
    image.src = src;
  });
}

function firstPositiveDimension(...values) {
  for (const value of values) {
    if (Number.isFinite(value) && value > 0) return value;
  }
  return 1;
}
