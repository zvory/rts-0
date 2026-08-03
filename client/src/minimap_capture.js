/** Exact-size PNG export lifecycle shared by UI and automation through Minimap.capturePng. */
export function captureMinimapPng(canvas, ctx, { width, height }, lifecycle) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || width !== height) {
    throw new RangeError("Minimap PNG capture requires matching positive integer dimensions.");
  }
  const original = {
    width: canvas.width,
    height: canvas.height,
    presentation: lifecycle.presentation(),
  };
  try {
    lifecycle.presentation(true);
    canvas.width = width;
    canvas.height = height;
    lifecycle.render();
    return Object.freeze({
      width,
      height,
      rgba: new Uint8ClampedArray(ctx.getImageData(0, 0, width, height).data),
      pngDataUrl: canvas.toDataURL("image/png"),
    });
  } finally {
    canvas.width = original.width;
    canvas.height = original.height;
    lifecycle.presentation(original.presentation);
    lifecycle.render();
  }
}
