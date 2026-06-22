import { STATS } from "../config.js";

export const GROUND_DECAL_TEXTURE_WORLD_SCALE = 4;

const DECAL_CLASS_INFANTRY = "infantry";
const DECAL_CLASS_SCORCH = "scorch";
const NEUTRAL_COLOR = "#9aa0a8";

export class GroundDecalLayer {
  constructor({
    layer,
    pixi = globalThis.PIXI,
    getDocument = () => (typeof document !== "undefined" ? document : null),
    downsample = GROUND_DECAL_TEXTURE_WORLD_SCALE,
    recordDiagnostic = null,
  } = {}) {
    this.layer = layer;
    this.pixi = pixi;
    this.getDocument = getDocument;
    this.downsample = downsample;
    this.recordDiagnostic = recordDiagnostic;
    this.canvas = null;
    this.ctx = null;
    this.texture = null;
    this.sprite = null;
    this.totalStamped = 0;
    this.textureUpdateCount = 0;
  }

  resetForMap(map) {
    this.destroy();
    this.totalStamped = 0;
    this.textureUpdateCount = 0;
    const doc = this.getDocument?.();
    if (!doc || !this.pixi?.Texture || !this.pixi?.Sprite || !this.layer) return false;
    const tileSize = Number.isFinite(map?.tileSize) ? map.tileSize : 32;
    const worldWidth = Math.max(1, (map?.width || 1) * tileSize);
    const worldHeight = Math.max(1, (map?.height || 1) * tileSize);
    this.canvas = doc.createElement("canvas");
    this.canvas.width = Math.max(1, Math.ceil(worldWidth / this.downsample));
    this.canvas.height = Math.max(1, Math.ceil(worldHeight / this.downsample));
    this.ctx = this.canvas.getContext("2d", { alpha: true });
    if (!this.ctx) {
      this.canvas = null;
      return false;
    }
    this.ctx.imageSmoothingEnabled = false;
    this.texture = this.pixi.Texture.from(this.canvas, {
      scaleMode: this.pixi.SCALE_MODES?.NEAREST,
    });
    this.sprite = new this.pixi.Sprite(this.texture);
    this.sprite.scale.set(this.downsample);
    this.layer.addChild(this.sprite);
    this.recordDiagnostic?.("renderer.groundDecals.displayObjects", this.displayObjectCount());
    return true;
  }

  stampBatch(decals, { onError = null } = {}) {
    if (!this.ctx || !Array.isArray(decals) || decals.length === 0) return 0;
    this.recordDiagnostic?.("renderer.groundDecals.pending", decals.length);
    let stamped = 0;
    for (const decal of decals) {
      try {
        if (stampGroundDecal(this.ctx, decal, this.downsample)) stamped += 1;
        else this.recordDiagnostic?.("renderer.groundDecals.skipped", 1);
      } catch (err) {
        this.recordDiagnostic?.("renderer.groundDecals.skipped", 1);
        onError?.("groundDecal", err);
      }
    }
    if (stamped > 0) {
      updateTexture(this.texture);
      this.totalStamped += stamped;
      this.textureUpdateCount += 1;
      this.recordDiagnostic?.("renderer.groundDecals.stamped", stamped);
      this.recordDiagnostic?.("renderer.groundDecals.textureUpdates", 1);
    }
    return stamped;
  }

  displayObjectCount() {
    return Array.isArray(this.layer?.children) ? this.layer.children.length : 0;
  }

  destroy() {
    if (this.sprite) {
      if (this.sprite.parent && typeof this.sprite.parent.removeChild === "function") {
        this.sprite.parent.removeChild(this.sprite);
      }
      this.sprite.destroy?.({ texture: true, baseTexture: true });
      this.sprite = null;
    } else if (this.texture) {
      this.texture.destroy?.(true);
      this.texture.baseTexture?.destroy?.();
    }
    this.texture = null;
    if (this.ctx && this.canvas) {
      this.ctx.clearRect?.(0, 0, this.canvas.width || 0, this.canvas.height || 0);
    }
    if (this.canvas) {
      this.canvas.width = 0;
      this.canvas.height = 0;
    }
    this.ctx = null;
    this.canvas = null;
  }
}

export function _initGroundDecalsForMap(map) {
  this._groundDecals?.resetForMap(map);
}

export function _drawGroundDecals(state) {
  if (!this._groundDecals || typeof state?.consumePendingGroundDecals !== "function") return 0;
  if (!this._groundDecals.ctx) return 0;
  const decals = state.consumePendingGroundDecals();
  return this._groundDecals.stampBatch(decals, {
    onError: (label, err) => this._recordRenderError?.(label, err),
  });
}

export function stampGroundDecal(ctx, decal, downsample = GROUND_DECAL_TEXTURE_WORLD_SCALE) {
  if (!ctx || !decal || !Number.isFinite(decal.x) || !Number.isFinite(decal.y)) return false;
  if (decal.decalClass !== DECAL_CLASS_INFANTRY && decal.decalClass !== DECAL_CLASS_SCORCH) {
    return false;
  }
  const rng = mulberry32(decal.seed || decal.id || 1);
  const x = decal.x / downsample;
  const y = decal.y / downsample;
  const facing = Number.isFinite(decal.facing) ? decal.facing : seededAngle(rng);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(facing);
  const variantScale = 0.92 + rng() * 0.18;
  ctx.scale(variantScale, variantScale);
  if (decal.decalClass === DECAL_CLASS_INFANTRY) {
    stampInfantry(ctx, decal, rng, downsample);
  } else {
    stampScorch(ctx, decal, rng, downsample);
  }
  ctx.restore();
  return true;
}

function stampInfantry(ctx, decal, rng, downsample) {
  const color = normalizeColor(decal.color);
  const blobs = 3 + (decal.variant % 3);
  for (let i = 0; i < blobs; i += 1) {
    const angle = seededAngle(rng);
    const dist = (1.5 + rng() * 7) / downsample;
    const rx = (5 + rng() * 7) / downsample;
    const ry = (3 + rng() * 5) / downsample;
    const x = Math.cos(angle) * dist;
    const y = Math.sin(angle) * dist * 0.7;
    drawEllipse(ctx, x, y, rx, ry, seededAngle(rng), rgba(color, 0.28 + rng() * 0.18));
  }
  drawEllipse(ctx, 0, 0, 6 / downsample, 3 / downsample, seededAngle(rng), rgba(0x120908, 0.22));
}

function stampScorch(ctx, decal, rng, downsample) {
  const color = normalizeColor(decal.color);
  const stat = STATS[decal.kind] || {};
  const body = stat.body || {};
  const length = Math.max(22, body.length || (stat.size || 16) * 2.4) / downsample;
  const width = Math.max(12, body.width || (stat.size || 16) * 1.25) / downsample;
  const char = 0.8 + rng() * 0.16;
  ctx.fillStyle = rgba(0x070706, 0.45);
  irregularHullPath(ctx, length * char, width * (0.9 + rng() * 0.18), rng);
  ctx.fill();
  ctx.fillStyle = rgba(0x201812, 0.36);
  irregularHullPath(ctx, length * 0.76, width * 0.7, rng);
  ctx.fill();

  const chips = 2 + (decal.variant % 3);
  ctx.fillStyle = rgba(color, 0.34);
  for (let i = 0; i < chips; i += 1) {
    const px = (-length * 0.35) + rng() * length * 0.7;
    const py = (-width * 0.32) + rng() * width * 0.64;
    const chipW = Math.max(1, (3 + rng() * 7) / downsample);
    const chipH = Math.max(1, (2 + rng() * 4) / downsample);
    ctx.save();
    ctx.translate(px, py);
    ctx.rotate((rng() - 0.5) * 0.8);
    ctx.fillRect(-chipW / 2, -chipH / 2, chipW, chipH);
    ctx.restore();
  }
}

function irregularHullPath(ctx, length, width, rng) {
  const halfL = length / 2;
  const halfW = width / 2;
  const jitterL = length * 0.08;
  const jitterW = width * 0.12;
  ctx.beginPath();
  ctx.moveTo(-halfL + (rng() - 0.5) * jitterL, -halfW + (rng() - 0.5) * jitterW);
  ctx.lineTo(halfL + (rng() - 0.5) * jitterL, -halfW * 0.85 + (rng() - 0.5) * jitterW);
  ctx.lineTo(halfL * 0.95 + (rng() - 0.5) * jitterL, halfW + (rng() - 0.5) * jitterW);
  ctx.lineTo(-halfL * 0.9 + (rng() - 0.5) * jitterL, halfW * 0.9 + (rng() - 0.5) * jitterW);
  ctx.closePath();
}

function drawEllipse(ctx, x, y, rx, ry, rotation, fillStyle) {
  ctx.fillStyle = fillStyle;
  ctx.beginPath();
  if (typeof ctx.ellipse === "function") {
    ctx.ellipse(x, y, rx, ry, rotation, 0, Math.PI * 2);
  } else {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(rotation);
    ctx.scale(rx, ry);
    ctx.arc(0, 0, 1, 0, Math.PI * 2);
    ctx.restore();
  }
  ctx.fill();
}

function updateTexture(texture) {
  if (typeof texture?.update === "function") texture.update();
  else texture?.baseTexture?.update?.();
}

function normalizeColor(color) {
  if (typeof color === "number" && Number.isFinite(color)) return color >>> 0;
  const match = /^#?([0-9a-fA-F]{6})$/.exec(String(color || NEUTRAL_COLOR));
  return match ? Number.parseInt(match[1], 16) : Number.parseInt(NEUTRAL_COLOR.slice(1), 16);
}

function rgba(color, alpha) {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return `rgba(${r},${g},${b},${alpha})`;
}

function seededAngle(rng) {
  return (rng() * 2 - 1) * Math.PI;
}

function mulberry32(seed) {
  let value = seed >>> 0;
  return () => {
    value = (value + 0x6d2b79f5) >>> 0;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
