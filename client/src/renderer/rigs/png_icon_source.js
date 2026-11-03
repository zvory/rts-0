// SVG/canvas icons read original image files rather than the worker's packed GPU texture.
export function spriteRasterSource(atlas, sprite) {
  const frame = sprite.frame;
  const attachment = atlas.attachments?.find((part) => frame.x >= part.x && frame.y >= part.y
    && frame.x + frame.w <= part.x + part.source.w && frame.y + frame.h <= part.y + part.source.h);
  if (!attachment) return { image: atlas.image, sheetWidth: atlas.grid.width, sheetHeight: atlas.grid.height, frame };
  return {
    image: attachment.atlas.image,
    sheetWidth: attachment.atlas.grid.width,
    sheetHeight: attachment.atlas.grid.height,
    frame: { ...frame, x: frame.x - attachment.x + attachment.source.x,
      y: frame.y - attachment.y + attachment.source.y },
  };
}
