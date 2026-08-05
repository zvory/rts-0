import { loadWorkerSafeTexture } from "./raster_primitives.js";
import {
  DOODAD_MANIFEST,
  DOODAD_TYPE_IDS,
  MAX_DOODADS,
  doodadManifestEntry,
} from "./doodad_manifest.js";
import { gfxEllipse, gfxFill } from "./native_graphics.js";
import { applyWorldYDepth } from "./world_y_depth.js";
import { doodadSizeVariation } from "../config.js";

const COLOR_RE = /^#[0-9a-fA-F]{6}$/;
const MIN_CULL_MARGIN_CSS_PX = 140;
const CANOPY_BUCKET_PX = 128;
const MAX_SIZE_VARIATION = 1.08;
const MAX_DOODAD_WIDTH_PX = Math.max(...Object.values(DOODAD_MANIFEST).map((entry) => entry.widthPx))
  * MAX_SIZE_VARIATION;
const MAX_DOODAD_HEIGHT_PX = Math.max(...Object.values(DOODAD_MANIFEST).map((entry) => entry.heightPx))
  * MAX_SIZE_VARIATION;

/** Worker-owned static vegetation sprites shared by live matches and the Map Editor. */
export class DoodadLayer {
  constructor({
    pixi,
    understoryLayer,
    canopyLayer,
    trackAsset = (_id, promise) => promise,
    loadTexture = loadWorkerSafeTexture,
  }) {
    if (!pixi?.Sprite || !pixi?.Graphics || !understoryLayer || !canopyLayer) {
      throw new TypeError("DoodadLayer requires Pixi sprite/graphics support and two world layers.");
    }
    this.pixi = pixi;
    this.understoryLayer = understoryLayer;
    this.canopyLayer = canopyLayer;
    this.canopyLayer.sortableChildren = true;
    this.trackAsset = trackAsset;
    this.loadTexture = loadTexture;
    this.textures = new Map();
    this.instances = new Map();
    this.records = new Map();
    this.canopyBuckets = new Map();
    this.assetIds = new Set();
    this.loadPromises = [];
    this.destroyed = false;
    this._loadTextures();
  }

  _loadTextures() {
    for (const typeId of DOODAD_TYPE_IDS) {
      const entry = DOODAD_MANIFEST[typeId];
      const assetId = `doodad:${typeId}`;
      this.assetIds.add(assetId);
      const load = Promise.resolve(this.loadTexture(this.pixi, entry.image)).then((texture) => {
        if (!texture) throw new Error(`Doodad texture ${entry.image} did not decode.`);
        texture.rtsRendererOwnedTexture = true;
        if (this.destroyed) {
          destroyTexture(texture);
          return null;
        }
        this.textures.set(typeId, texture);
        return texture;
      });
      this.loadPromises.push(this.trackAsset(assetId, load, {
        kind: typeId,
        source: "doodad",
      }));
    }
  }

  async ready() {
    await Promise.all(this.loadPromises);
    if (this.destroyed) throw new Error("Doodad assets finished after renderer teardown.");
    const missing = DOODAD_TYPE_IDS.filter((typeId) => !this.textures.has(typeId));
    if (missing.length) throw new Error(`Doodad assets failed: ${missing.join(", ")}`);
    return true;
  }

  replace(records) {
    if (this.destroyed) return 0;
    const normalized = normalizeDoodads(records);
    const nextIds = new Set(normalized.map((record) => record.id));
    for (const id of this.instances.keys()) {
      if (!nextIds.has(id)) this._remove(id);
    }
    for (const record of normalized) this._upsert(record);
    this.records = new Map(normalized.map((record) => [record.id, record]));
    this._rebuildCanopyIndex();
    return normalized.length;
  }

  patch({ upserts = [], removedIds = [] } = {}) {
    if (this.destroyed) return 0;
    for (const id of removedIds || []) {
      if (!Number.isSafeInteger(id) || id <= 0) continue;
      this.records.delete(id);
      this._remove(id);
    }
    for (const record of normalizeDoodads(upserts, { max: MAX_DOODADS })) {
      if (!this.records.has(record.id) && this.records.size >= MAX_DOODADS) break;
      this.records.set(record.id, record);
      this._upsert(record);
    }
    this._rebuildCanopyIndex();
    return this.records.size;
  }

  update(visualTimeMs, camera = null) {
    if (this.destroyed) return 0;
    const now = Number.isFinite(visualTimeMs) && visualTimeMs >= 0 ? visualTimeMs : 0;
    const projectedExtent = camera?.projectedExtent?.(
      { x: 0, y: 0, heightPx: 0 },
      MAX_DOODAD_WIDTH_PX,
      MAX_DOODAD_HEIGHT_PX,
    );
    const cullMargin = Math.max(
      MIN_CULL_MARGIN_CSS_PX,
      finiteNonNegative(projectedExtent?.width),
      finiteNonNegative(projectedExtent?.height),
    );
    let visible = 0;
    for (const instance of this.instances.values()) {
      const inView = typeof camera?.containsProjected !== "function"
        || camera.containsProjected({
          x: instance.record.x,
          y: instance.record.y,
          heightPx: 0,
        }, cullMargin);
      instance.display.visible = inView;
      if (instance.shadow) instance.shadow.visible = inView;
      if (!inView) continue;
      visible += 1;
      const sway = Math.sin(now * instance.manifest.windRate + instance.windPhase)
        * instance.manifest.windAmplitude;
      instance.display.rotation = sway;
    }
    return visible;
  }

  /** Presentation-only overlap query against nearby visible canopies. */
  occludesUnit(entity, radiusPx = 0) {
    const x = Number(entity?.x);
    const y = Number(entity?.y);
    const radius = Math.max(0, Number(radiusPx) || 0);
    if (!Number.isFinite(x) || !Number.isFinite(y)) return false;
    const candidates = new Set();
    for (let bucketY = bucketAt(y - radius); bucketY <= bucketAt(y + radius); bucketY += 1) {
      for (let bucketX = bucketAt(x - radius); bucketX <= bucketAt(x + radius); bucketX += 1) {
        for (const id of this.canopyBuckets.get(bucketKey(bucketX, bucketY)) || []) candidates.add(id);
      }
    }
    for (const id of candidates) {
      const instance = this.instances.get(id);
      if (!instance || instance.display.visible === false || instance.record.y <= y) continue;
      const bounds = canopyBounds(instance);
      if (
        x + radius >= bounds.left
        && x - radius <= bounds.right
        && y + radius >= bounds.top
        && y - radius <= bounds.bottom
      ) return true;
    }
    return false;
  }

  _upsert(record) {
    const existing = this.instances.get(record.id);
    if (existing && existing.record.typeId === record.typeId) {
      existing.record = record;
      positionInstance(existing);
      return;
    }
    if (existing) this._remove(record.id);
    const manifest = doodadManifestEntry(record.typeId);
    const texture = this.textures.get(record.typeId);
    if (!manifest || !texture) return;
    const display = new this.pixi.Sprite(texture);
    display.anchor?.set?.(0.5, manifest.anchorY);
    display.width = manifest.widthPx * doodadSizeVariation(record.id);
    display.height = manifest.heightPx * doodadSizeVariation(record.id);
    display.tint = manifest.tintable && record.color ? colorNumber(record.color) : 0xffffff;
    const parent = manifest.layer === "canopy" ? this.canopyLayer : this.understoryLayer;
    parent.addChild(display);
    let shadow = null;
    if (manifest.shadow) {
      shadow = new this.pixi.Graphics();
      gfxEllipse(
        gfxFill(shadow, 0x11170f, 0.28),
        0,
        0,
        manifest.shadow.radiusX,
        manifest.shadow.radiusY,
      );
      this.understoryLayer.addChild(shadow);
    }
    const instance = {
      record,
      manifest,
      display,
      shadow,
      windPhase: stableNoise(record.id, 17) * Math.PI * 2,
    };
    positionInstance(instance);
    this.instances.set(record.id, instance);
  }

  _rebuildCanopyIndex() {
    this.canopyBuckets.clear();
    for (const instance of this.instances.values()) {
      if (instance.manifest.layer !== "canopy") continue;
      const bounds = canopyBounds(instance);
      for (let bucketY = bucketAt(bounds.top); bucketY <= bucketAt(bounds.bottom); bucketY += 1) {
        for (let bucketX = bucketAt(bounds.left); bucketX <= bucketAt(bounds.right); bucketX += 1) {
          const key = bucketKey(bucketX, bucketY);
          const ids = this.canopyBuckets.get(key) || [];
          ids.push(instance.record.id);
          this.canopyBuckets.set(key, ids);
        }
      }
    }
  }

  _remove(id) {
    const instance = this.instances.get(id);
    if (!instance) return;
    instance.display.parent?.removeChild?.(instance.display);
    instance.display.destroy?.({ texture: false, textureSource: false });
    if (instance.shadow) {
      instance.shadow.parent?.removeChild?.(instance.shadow);
      instance.shadow.destroy?.();
    }
    this.instances.delete(id);
  }

  diagnostics() {
    return Object.freeze({
      records: this.records.size,
      instances: this.instances.size,
      textures: this.textures.size,
      understoryChildren: this.understoryLayer?.children?.length || 0,
      canopyChildren: [...this.instances.values()].filter((instance) => instance.manifest.layer === "canopy").length,
    });
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    for (const id of [...this.instances.keys()]) this._remove(id);
    this.records.clear();
    this.canopyBuckets.clear();
    for (const texture of this.textures.values()) destroyTexture(texture);
    this.textures.clear();
    this.loadPromises = [];
  }
}

function normalizeDoodads(records, { max = MAX_DOODADS } = {}) {
  if (!Array.isArray(records)) return [];
  const result = [];
  const seen = new Set();
  for (const value of records) {
    if (result.length >= max) break;
    const id = Number(value?.id);
    const typeId = typeof value?.typeId === "string" ? value.typeId : "";
    const x = Number(value?.x);
    const y = Number(value?.y);
    if (!Number.isSafeInteger(id) || id <= 0 || seen.has(id)) continue;
    if (!doodadManifestEntry(typeId) || !Number.isFinite(x) || !Number.isFinite(y)) continue;
    const record = { id, typeId, x, y };
    if (COLOR_RE.test(value?.color || "") && doodadManifestEntry(typeId)?.tintable) {
      record.color = value.color.toLowerCase();
    }
    seen.add(id);
    result.push(Object.freeze(record));
  }
  return result;
}

function positionInstance(instance) {
  const { record, manifest, display, shadow } = instance;
  display.position?.set?.(record.x, record.y);
  applyWorldYDepth(display, record);
  display.tint = manifest.tintable && record.color ? colorNumber(record.color) : 0xffffff;
  if (shadow) shadow.position?.set?.(record.x, record.y + manifest.shadow.offsetY);
}

function canopyBounds(instance) {
  const variation = doodadSizeVariation(instance.record.id);
  const width = instance.manifest.widthPx * variation;
  const height = instance.manifest.heightPx * variation;
  return {
    left: instance.record.x - width / 2,
    right: instance.record.x + width / 2,
    top: instance.record.y - height * instance.manifest.anchorY,
    bottom: instance.record.y + height * (1 - instance.manifest.anchorY),
  };
}

function bucketAt(value) {
  return Math.floor(value / CANOPY_BUCKET_PX);
}

function bucketKey(x, y) {
  return `${x}:${y}`;
}

function colorNumber(color) {
  return Number.parseInt(color.slice(1), 16);
}

function stableNoise(id, salt) {
  let value = (Math.imul(id | 0, 0x45d9f3b) ^ salt) >>> 0;
  value = Math.imul(value ^ (value >>> 16), 0x45d9f3b) >>> 0;
  value ^= value >>> 16;
  return value / 0xffffffff;
}

function finiteNonNegative(value) {
  return Number.isFinite(value) && value >= 0 ? value : 0;
}

function destroyTexture(texture) {
  if (!texture || texture.destroyed) return;
  texture.destroy?.(true);
  texture.source?.destroy?.();
}
