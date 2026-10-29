// Renderer-local revisioned textures, separate from the gameplay protocol. State advances only
// when the host dispatches a frame, so superseded pending frames cannot lose texture updates.
export function minimapTexturePacket(frame, state, type, version) {
  const minimap = frame.minimap;
  if (!minimap) return null;
  if (state.minimapSize !== minimap.size) {
    state.minimapSize = minimap.size;
    state.minimapImages = new Map();
  }
  const images = [], transfer = [];
  for (const image of minimap.images || []) {
    if (state.minimapImages.get(image.id) === image.revision) continue;
    validateMinimapImage(image);
    const pixels = new Uint8Array(image.pixels);
    images.push({ ...image, pixels });
    transfer.push(pixels.buffer);
    state.minimapImages.set(image.id, image.revision);
  }
  if (!images.length) return null;
  return { message: { version, type, generation: frame.generation,
    payload: { size: minimap.size, images } }, transfer };
}

export function validateMinimapImage(image) {
  if (!Number.isSafeInteger(image?.id) || image.id < 1 || !Number.isSafeInteger(image.revision) || image.revision < 0
    || !Number.isSafeInteger(image.width) || image.width < 1 || image.width > 8192
    || !Number.isSafeInteger(image.height) || image.height < 1 || image.height > 8192
    || !Number.isSafeInteger(image.fullWidth) || image.fullWidth < image.width || image.fullWidth > 8192
    || !Number.isSafeInteger(image.fullHeight) || image.fullHeight < image.height || image.fullHeight > 8192
    || !Number.isSafeInteger(image.offsetX) || image.offsetX < 0 || image.offsetX + image.width > image.fullWidth
    || !Number.isSafeInteger(image.offsetY) || image.offsetY < 0 || image.offsetY + image.height > image.fullHeight
    || !(image.pixels instanceof Uint8Array) || image.pixels.length !== image.width * image.height * 4) {
    throw new TypeError("Invalid minimap texture snapshot");
  }
}
