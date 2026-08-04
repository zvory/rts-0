import { COLORS } from "./config.js";

const TREE_TYPE_PREFIX = "tree.";

const hex = (value) => `#${value.toString(16).padStart(6, "0")}`;

const signatureChanged = (prev, next) => {
  if (!prev) return true;
  for (const [key, value] of Object.entries(next)) {
    if (prev[key] !== value) return true;
  }
  return false;
};

const forestStyleSignature = () => [
  COLORS.minimapForestCanopy,
  COLORS.minimapForestTrunk,
].join(",");

export class MinimapForestLayer {
  constructor({ createCanvas, onInvalidation, onDiagnostic } = {}) {
    this.createCanvas = createCanvas;
    this.onInvalidation = onInvalidation;
    this.onDiagnostic = onDiagnostic;
    this.canvas = null;
    this.ctx = null;
    this.signature = null;
    this.doodads = null;
    this.trees = [];
  }

  draw({ ctx, map, size, scale, offX, offY, presentation }) {
    const trees = this._treesForMap(map);
    if (trees.length === 0) return;
    const layer = this._ensureLayer(size);
    if (!layer) {
      this._paint(ctx, trees, map.tileSize, scale, offX, offY);
      return;
    }
    const nextSignature = {
      map,
      doodads: map.doodads,
      mapWidth: map.width,
      mapHeight: map.height,
      tileSize: map.tileSize,
      size,
      scale,
      offX,
      offY,
      presentation,
      style: forestStyleSignature(),
    };
    if (signatureChanged(this.signature, nextSignature)) {
      this.onInvalidation?.(this.signature, nextSignature);
      this.onDiagnostic?.("minimap.cache.forest.miss");
      layer.ctx.clearRect(0, 0, size, size);
      this._paint(layer.ctx, trees, map.tileSize, scale, offX, offY);
      this.signature = nextSignature;
    } else {
      this.onDiagnostic?.("minimap.cache.forest.hit");
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
    this.doodads = null;
    this.trees = [];
  }

  _paint(ctx, trees, tileSize, scale, offX, offY) {
    const radius = Math.max(1.2, tileSize * scale * 0.18);
    const trunkWidth = Math.max(0.65, radius * 0.45);
    const trunkHeight = Math.max(0.8, radius * 0.72);

    ctx.save();
    ctx.fillStyle = hex(COLORS.minimapForestTrunk);
    for (const tree of trees) {
      const x = offX + tree.x * scale;
      const y = offY + tree.y * scale;
      ctx.fillRect(x - trunkWidth / 2, y + radius * 0.35, trunkWidth, trunkHeight);
    }

    ctx.fillStyle = hex(COLORS.minimapForestCanopy);
    ctx.beginPath();
    for (const tree of trees) {
      const x = offX + tree.x * scale;
      const y = offY + tree.y * scale;
      ctx.moveTo(x, y - radius * 1.25);
      ctx.lineTo(x + radius, y + radius * 0.65);
      ctx.lineTo(x - radius, y + radius * 0.65);
      ctx.closePath();
    }
    ctx.fill();
    ctx.restore();
  }

  _ensureLayer(size) {
    if (!this.canvas) this.canvas = this.createCanvas?.() || null;
    if (!this.canvas) return null;
    if (this.canvas.width !== size) this.canvas.width = size;
    if (this.canvas.height !== size) this.canvas.height = size;
    if (!this.ctx) this.ctx = this.canvas.getContext?.("2d") || null;
    return this.ctx ? { canvas: this.canvas, ctx: this.ctx } : null;
  }

  _treesForMap(map) {
    const doodads = map.doodads || [];
    if (this.doodads !== doodads) {
      this.doodads = doodads;
      this.trees = doodads.filter((record) => record.typeId?.startsWith(TREE_TYPE_PREFIX));
    }
    return this.trees;
  }
}
