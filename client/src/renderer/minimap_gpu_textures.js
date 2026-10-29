// Immutable icon images share atlas pages so interleaved unit colors/parts remain batchable.
// Frequently changing map/fog layers have independent sources: a fog update never reuploads icons.
export class MinimapGpuTextures {
  constructor(pixi) { this.pixi = pixi; this.images = new Map(); this.pages = []; this.size = 0; }

  update({ images, size }) {
    if (this.size && size !== this.size) this.clear();
    this.size = size;
    const dirty = new Set();
    for (const image of images) {
      let record = this.images.get(image.id);
      if (record?.revision === image.revision) continue;
      if (!record) {
        const slot = image.atlas && image.width + 4 <= 2048 && image.height + 4 <= 2048
          ? this._slot(image.width, image.height) : this._standalone(image.width, image.height);
        const texture = slot.page
          ? new this.pixi.Texture({ source: slot.texture.source,
            frame: new this.pixi.Rectangle(slot.x, slot.y, image.width, image.height) })
          : slot.texture;
        record = { ...slot, texture, revision: -1, width: image.width, height: image.height,
          fullWidth: image.fullWidth, fullHeight: image.fullHeight,
          offsetX: image.offsetX || 0, offsetY: image.offsetY || 0 };
        this.images.set(image.id, record);
      }
      if (!record.page) record.texture.source.scaleMode = image.smooth === false ? "nearest" : "linear";
      record.context.putImageData(new ImageData(new Uint8ClampedArray(image.pixels.buffer,
        image.pixels.byteOffset, image.pixels.byteLength), image.width, image.height), record.x, record.y);
      record.revision = image.revision;
      dirty.add(record.texture.source);
    }
    for (const source of dirty) source.update();
  }

  _standalone(width, height) {
    const canvas = new OffscreenCanvas(width, height);
    const texture = this.pixi.Texture.from(canvas);
    texture.source.style.scaleMode = "linear";
    return { canvas, context: canvas.getContext("2d"), texture, x: 0, y: 0 };
  }

  _slot(width, height) {
    for (const page of this.pages) {
      if (page.x + width + 2 > 2048) { page.x = 2; page.y += page.rowHeight; page.rowHeight = 0; }
      if (page.y + height + 2 > 2048) continue;
      const slot = { ...page, page, x: page.x, y: page.y };
      page.x += width + 4; page.rowHeight = Math.max(page.rowHeight, height + 4);
      return slot;
    }
    const page = { ...this._standalone(2048, 2048), x: 2, y: 2, rowHeight: 0 };
    this.pages.push(page);
    return this._slot(width, height);
  }

  get(id) {
    const image = this.images.get(id);
    if (!image) throw new Error(`Minimap image ${id} was not retained before its frame`);
    return image;
  }

  clear() {
    for (const record of this.images.values()) if (!record.page) record.texture.destroy(true);
    for (const page of this.pages) page.texture.destroy(true);
    for (const record of this.images.values()) if (record.page) record.texture.destroy(false);
    this.images.clear(); this.pages.length = 0; this.size = 0;
  }
}
