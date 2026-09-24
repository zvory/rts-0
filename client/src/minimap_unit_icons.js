// Shared live/export unit presentation. Images are supplied by the host; no unit-dot fallback.
const SIZES = Object.freeze({
  worker: 9, rifleman: 9, panzerfaust: 10, machine_gunner: 12, tank: 26,
  scout_car: 18, command_car: 20, anti_tank_gun: 22, rocket_launcher: 25,
  scout_plane: 26, artillery: 26,
});

export function unitPngFacing(entity) {
  return (Number.isFinite(entity.facing) ? entity.facing : 0)
    - (entity.kind === "machine_gunner" ? Math.PI / 2 : 0);
}

export function unitPngSize(kind, canvasSize = 480) {
  return (SIZES[kind] || 18) * 1.35 * 1.5 * canvasSize / 480;
}

export class MinimapUnitIcons {
  constructor({ loadImage, createCanvas }) {
    this.loadImage = loadImage;
    this.createCanvas = createCanvas;
    this.entries = new Map();
    this.controller = new AbortController();
  }

  request(kind, color) {
    const key = `${kind}:${color}`;
    if (this.entries.has(key)) return this.entries.get(key);
    const entry = { key, image: null, mask: null, error: null };
    this.entries.set(key, entry);
    entry.promise = Promise.resolve().then(() => {
      if (this.controller.signal.aborted) return null;
      if (!this.loadImage) throw Error("Minimap unit image loader is unavailable");
      return this.loadImage(kind, color, { signal: this.controller.signal });
    }).then(image => {
      if (this.controller.signal.aborted) { image?.close?.(); return; }
      if (!image?.width || !image?.height) throw Error(`Empty minimap portrait: ${kind}`);
      const mask = this.createCanvas();
      mask.width = image.width; mask.height = image.height;
      const context = mask.getContext("2d");
      context.drawImage(image, 0, 0);
      context.globalCompositeOperation = "source-in";
      context.fillStyle = "#ffffff";
      context.fillRect(0, 0, image.width, image.height);
      entry.image = image; entry.mask = mask;
    }).catch(error => {
      if (!this.controller.signal.aborted) entry.error = error;
    });
    return entry;
  }

  async prepare(requests) {
    const entries = requests.map(({ kind, color }) => this.request(kind, color));
    await Promise.all(entries.map(entry => entry.promise));
    const failed = entries.find(entry => entry.error);
    if (failed) throw failed.error;
  }

  readiness() {
    const entries = [...this.entries.values()];
    return {
      ready: entries.every(entry => entry.image && !entry.error),
      pendingAssets: entries.filter(entry => !entry.image && !entry.error).map(entry => ({ id: entry.key, status: "pending" })),
      failedAssets: entries.filter(entry => entry.error).map(entry => ({ id: entry.key, status: "failed", message: entry.error.message })),
    };
  }

  _sprite(entry, kind, canvasSize, flash) {
    if (entry.size !== canvasSize) {
      for (const sprite of entry.sprites || []) {
        if (sprite) { sprite.width = 0; sprite.height = 0; }
      }
      entry.size = canvasSize; entry.sprites = [];
    }
    const index = flash ? 1 : 0;
    if (entry.sprites[index]) return entry.sprites[index];
    const { image, mask } = entry;
    const scale = unitPngSize(kind, canvasSize) / Math.max(image.width, image.height);
    const width = image.width * scale, height = image.height * scale;
    const outline = canvasSize / 480;
    // Oversample the small cached portrait so rotation remains smooth at any device pixel ratio.
    const sprite = this.createCanvas();
    sprite.width = Math.ceil((width + outline * 2 + 2) * 4);
    sprite.height = Math.ceil((height + outline * 2 + 2) * 4);
    const ctx = sprite.getContext("2d");
    const cx = sprite.width / 2, cy = sprite.height / 2;
    for (let i = 0; i < 16; i++) {
      const angle = i * Math.PI / 8;
      ctx.drawImage(mask, cx - width * 2 + Math.cos(angle) * outline * 4,
        cy - height * 2 + Math.sin(angle) * outline * 4, width * 4, height * 4);
    }
    ctx.drawImage(flash ? mask : image, cx - width * 2, cy - height * 2, width * 4, height * 4);
    entry.sprites[index] = sprite;
    return sprite;
  }

  draw(context, entity, color, point, canvasSize, flash = false) {
    const entry = this.request(entity.kind, color);
    if (!entry.image) return;
    const sprite = this._sprite(entry, entity.kind, canvasSize, flash);
    context.save();
    context.translate(point.x, point.y);
    context.rotate(unitPngFacing(entity));
    context.drawImage(sprite, -sprite.width / 8, -sprite.height / 8, sprite.width / 4, sprite.height / 4);
    context.restore();
  }

  destroy() {
    this.controller.abort();
    for (const entry of this.entries.values()) {
      entry.image?.close?.();
      if (entry.mask) { entry.mask.width = 0; entry.mask.height = 0; }
      for (const sprite of entry.sprites || []) {
        if (sprite) { sprite.width = 0; sprite.height = 0; }
      }
      entry.image = null; entry.mask = null;
    }
    this.entries.clear();
  }
}
