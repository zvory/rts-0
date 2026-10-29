import { SETUP } from "./protocol.js";

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
  constructor({ loadImage, createCanvas, animation = null }) {
    this.animation = animation;
    this.loadImage = loadImage;
    this.createCanvas = createCanvas;
    this.entries = new Map();
    this.primedAnimations = new Set();
    this.controller = new AbortController();
  }

  request(kind, color, part = null) {
    const key = `${kind}:${color}:${part || "portrait"}`;
    if (this.entries.has(key)) return this.entries.get(key);
    const entry = { key, part, image: null, mask: null, error: null };
    this.entries.set(key, entry);
    entry.promise = Promise.resolve().then(() => {
      if (this.controller.signal.aborted) return null;
      if (!this.loadImage) throw Error("Minimap unit image loader is unavailable");
      return this.loadImage(kind, color, { signal: this.controller.signal, minimapPart: part });
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
    const entries = requests.flatMap(({ kind, color }) => this._parts(kind).map(part => this.request(kind, color, part)));
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
    // Articulated layers reserve a shared square for rotations; compensate for its padding.
    const scale = unitPngSize(kind, canvasSize) * (entry.part && !entry.part.includes("frame:") ? 1.8 : 1)
      / Math.max(image.width, image.height);
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

  beginFrame(entities, state, now) { this.animation?.beginFrame(entities, state, now); }

  _parts(kind) {
    const parts = this.animation?.parts(kind);
    if (parts) return parts;
    if (kind === "tank") return ["hull", "turret"];
    if (kind === "anti_tank_gun") return ["packed-base", "packed-barrel", "deployed-base", "deployed-barrel"];
    return [null];
  }

  draw(context, entity, color, point, canvasSize, flash = false) {
    const animated = this.animation?.layers(entity);
    if (animated) {
      // Live matches do not call prepare(). Load the bounded frame set on first sight,
      // so a later shot or setup transition does not start decoding a missing marker.
      const key = `${entity.kind}:${color}`;
      if (!this.primedAnimations.has(key)) {
        this.primedAnimations.add(key);
        for (const part of this._parts(entity.kind)) this.request(entity.kind, color, part);
      }
      const size = unitPngSize(entity.kind, canvasSize);
      for (const layer of animated) this._drawLayer(context, entity, color,
        { x: point.x + layer.x * size, y: point.y + layer.y * size },
        canvasSize, flash, layer.part, layer.rotation, layer);
      return;
    }
    const facing = unitPngFacing(entity);
    const weaponFacing = Number.isFinite(entity.weaponFacing) ? entity.weaponFacing : facing;
    let layers = [[null, facing]];
    if (entity.kind === "tank") layers = [["hull", facing], ["turret", weaponFacing]];
    if (entity.kind === "anti_tank_gun") {
      const deployed = [SETUP.DEPLOYED, SETUP.TEARING_DOWN].includes(entity.setupState);
      const pose = deployed ? "deployed" : "packed";
      const baseFacing = deployed && Number.isFinite(entity.setupFacing) ? entity.setupFacing : facing;
      layers = [[`${pose}-base`, baseFacing], [`${pose}-barrel`, deployed ? weaponFacing : facing]];
    }
    for (const [part, rotation] of layers) {
      this._drawLayer(context, entity, color, point, canvasSize, flash, part, rotation);
    }
  }

  _drawLayer(context, entity, color, point, canvasSize, flash, part, rotation, transform = null) {
    const entry = this.request(entity.kind, color, part);
    if (!entry.image) return;
    const sprite = this._sprite(entry, entity.kind, canvasSize, flash);
    context.save();
    context.translate(point.x, point.y);
    context.rotate(rotation);
    if (transform) {
      context.scale(transform.scaleX, transform.scaleY);
      context.globalAlpha *= transform.alpha;
    }
    context.drawImage(sprite, -sprite.width / 8, -sprite.height / 8, sprite.width / 4, sprite.height / 4);
    context.restore();
  }

  destroy() {
    this.animation?.destroy();
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
    this.primedAnimations.clear();
  }
}
