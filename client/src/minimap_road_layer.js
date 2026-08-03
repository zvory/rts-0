import {
  hasMinimapRoadMarkings,
  minimapRoadMarkingStyleSignature,
  paintMinimapRoadMarkings,
} from "./minimap_terrain.js";

const signatureChanged = (prev, next) => {
  if (!prev) return true;
  for (const [key, value] of Object.entries(next)) {
    if (prev[key] !== value) return true;
  }
  return false;
};

export class MinimapRoadLayer {
  constructor({ createCanvas, onInvalidation, onDiagnostic } = {}) {
    this.createCanvas = createCanvas;
    this.onInvalidation = onInvalidation;
    this.onDiagnostic = onDiagnostic;
    this.canvas = null;
    this.ctx = null;
    this.signature = null;
    this.presenceMap = null;
    this.presenceTerrain = null;
    this.markingsPresent = false;
  }

  draw({ ctx, map, size, scale, offX, offY, presentation }) {
    if (!this._mapHasMarkings(map)) return;
    const layer = this._ensureLayer(size);
    if (!layer) {
      this._paint(ctx, map, scale, offX, offY);
      return;
    }
    const nextSignature = {
      map,
      terrain: map.terrain,
      mapWidth: map.width,
      mapHeight: map.height,
      tileSize: map.tileSize,
      size,
      scale,
      offX,
      offY,
      presentation,
      style: minimapRoadMarkingStyleSignature(),
    };
    if (signatureChanged(this.signature, nextSignature)) {
      this.onInvalidation?.(this.signature, nextSignature);
      this.onDiagnostic?.("minimap.cache.road.miss");
      layer.ctx.clearRect(0, 0, size, size);
      this._paint(layer.ctx, map, scale, offX, offY);
      this.signature = nextSignature;
    } else {
      this.onDiagnostic?.("minimap.cache.road.hit");
    }
    ctx.drawImage(layer.canvas, 0, 0);
  }

  invalidate() {
    this.signature = null;
  }

  destroy() {
    this.canvas = null;
    this.ctx = null;
    this.signature = null;
    this.presenceMap = null;
    this.presenceTerrain = null;
    this.markingsPresent = false;
  }

  _paint(ctx, map, scale, offX, offY) {
    paintMinimapRoadMarkings(ctx, map, scale, (x, y) => ({
      x: offX + x * scale,
      y: offY + y * scale,
    }));
  }

  _mapHasMarkings(map) {
    if (this.presenceMap !== map || this.presenceTerrain !== map.terrain) {
      this.presenceMap = map;
      this.presenceTerrain = map.terrain;
      this.markingsPresent = hasMinimapRoadMarkings(map);
    }
    return this.markingsPresent;
  }

  _ensureLayer(size) {
    if (!this.canvas) this.canvas = this.createCanvas?.() || null;
    if (!this.canvas) return null;
    if (this.canvas.width !== size) this.canvas.width = size;
    if (this.canvas.height !== size) this.canvas.height = size;
    if (!this.ctx) this.ctx = this.canvas.getContext?.("2d") || null;
    return this.ctx ? { canvas: this.canvas, ctx: this.ctx } : null;
  }
}
