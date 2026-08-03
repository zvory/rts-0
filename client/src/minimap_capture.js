/** Render the production minimap at an exact square size without gameplay transients. */
export function captureMinimapPng(minimap, { width, height }) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || width !== height) {
    throw new RangeError("Minimap PNG capture requires matching positive integer dimensions.");
  }
  const { canvas, ctx } = minimap;
  const original = { width: canvas.width, height: canvas.height };
  try {
    canvas.width = width;
    canvas.height = height;
    minimap.render(null, { capturePresentation: true });
    return Object.freeze({
      width,
      height,
      rgba: new Uint8ClampedArray(ctx.getImageData(0, 0, width, height).data),
      pngDataUrl: canvas.toDataURL("image/png"),
    });
  } finally {
    canvas.width = original.width;
    canvas.height = original.height;
    minimap.render();
  }
}
