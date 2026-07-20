export function createWorkerSafeCanvas(width = 1, height = 1, {
  OffscreenCanvasCtor = globalThis.OffscreenCanvas,
} = {}) {
  if (typeof OffscreenCanvasCtor !== "function") {
    throw new Error("Pixi rendering requires OffscreenCanvas.");
  }
  return new OffscreenCanvasCtor(positiveDimension(width), positiveDimension(height));
}

export async function fetchImageBitmap(url, {
  fetchFn = globalThis.fetch,
  createImageBitmapFn = globalThis.createImageBitmap,
} = {}) {
  if (typeof fetchFn !== "function" || typeof createImageBitmapFn !== "function") {
    throw new Error("Pixi asset decoding requires fetch and createImageBitmap.");
  }
  const response = await fetchFn(url);
  if (!response?.ok) throw new Error(`failed to fetch renderer asset ${url} (${response?.status ?? "unknown"})`);
  return createImageBitmapFn(await response.blob(), {
    premultiplyAlpha: "premultiply",
    colorSpaceConversion: "none",
  });
}

export async function loadWorkerSafeTexture(pixi, url, options = {}) {
  if (!pixi?.Texture?.from) throw new Error("Pixi Texture.from is unavailable.");
  const bitmap = await fetchImageBitmap(url, options);
  try {
    const texture = pixi.Texture.from(bitmap);
    if (texture?.source) texture.source.scaleMode = "nearest";
    return texture;
  } catch (error) {
    bitmap.close?.();
    throw error;
  }
}

function positiveDimension(value) {
  const number = Math.trunc(Number(value));
  return Number.isFinite(number) && number > 0 ? number : 1;
}
