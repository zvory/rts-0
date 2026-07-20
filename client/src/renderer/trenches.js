import { gfxNoFill, gfxEllipse, gfxPoly, gfxStrokePaths, gfxFill, gfxStroke } from "./native_graphics.js";
import { COLORS, ENTRENCHMENT_TRENCH_RADIUS_TILES } from "../config.js";
import { finiteNumber } from "./shared.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

export const TRENCH_DECAL_TEXTURE_WORLD_SCALE = 4;

const TRENCH_MIN_RADIUS_PX = 4;
const TRENCH_POLYGON_POINTS = 22;
const TRENCH_FACET_COUNT = 5;
const OCCUPIED_LIP_POINTS = 12;

export class TrenchDecalLayer {
  constructor({
    layer,
    pixi = globalThis.PIXI,
    createCanvas = createWorkerSafeCanvas,
    downsample = TRENCH_DECAL_TEXTURE_WORLD_SCALE,
    recordDiagnostic = null,
  } = {}) {
    this.layer = layer;
    this.pixi = pixi;
    this.createCanvas = createCanvas;
    this.downsample = downsample;
    this.recordDiagnostic = recordDiagnostic;
    this.canvas = null;
    this.ctx = null;
    this.texture = null;
    this.sprite = null;
    this.snapshotSignature = null;
    this.sourceRef = null;
    this.sourceTileSize = null;
    this.visibleCount = 0;
    this.totalStamped = 0;
    this.textureUpdateCount = 0;
  }

  resetForMap(map) {
    this.destroy();
    this.snapshotSignature = null;
    this.sourceRef = null;
    this.sourceTileSize = null;
    this.visibleCount = 0;
    this.totalStamped = 0;
    this.textureUpdateCount = 0;

    if (!this.pixi?.Texture || !this.pixi?.Sprite || !this.layer) return false;
    const tileSize = Number.isFinite(map?.tileSize) ? map.tileSize : 32;
    const worldWidth = Math.max(1, (map?.width || 1) * tileSize);
    const worldHeight = Math.max(1, (map?.height || 1) * tileSize);
    this.canvas = this.createCanvas();
    this.canvas.width = Math.max(1, Math.ceil(worldWidth / this.downsample));
    this.canvas.height = Math.max(1, Math.ceil(worldHeight / this.downsample));
    this.ctx = this.canvas.getContext("2d", { alpha: true });
    if (!this.ctx) {
      this.canvas = null;
      return false;
    }
    this.ctx.imageSmoothingEnabled = false;
    this.texture = this.pixi.Texture.from(this.canvas);
    this.sprite = new this.pixi.Sprite(this.texture);
    this.sprite.scale.set(this.downsample);
    this.layer.addChild(this.sprite);
    this.recordDiagnostic?.("renderer.trenches.displayObjects", this.displayObjectCount());
    return true;
  }

  drawSnapshot(trenches, { tileSize = 32 } = {}) {
    if (!this.ctx || !this.canvas) return 0;
    if (trenches === this.sourceRef && tileSize === this.sourceTileSize) {
      this.recordDiagnostic?.("renderer.trenches.visible", this.visibleCount);
      return this.visibleCount;
    }

    const normalized = normalizedTrenches(trenches, tileSize);
    const signature = trenchSnapshotSignature(normalized);
    this.sourceRef = trenches;
    this.sourceTileSize = tileSize;
    this.visibleCount = normalized.length;
    this.recordDiagnostic?.("renderer.trenches.visible", normalized.length);
    if (signature === this.snapshotSignature) return normalized.length;

    this.ctx.clearRect(0, 0, this.canvas.width || 0, this.canvas.height || 0);
    let stamped = 0;
    for (const trench of normalized) {
      if (stampTrenchDecal(this.ctx, trench, this.downsample)) stamped += 1;
    }
    updateTexture(this.texture);
    this.snapshotSignature = signature;
    this.totalStamped = stamped;
    this.textureUpdateCount += 1;
    this.recordDiagnostic?.("renderer.trenches.stamped", stamped);
    this.recordDiagnostic?.("renderer.trenches.textureUpdates", 1);
    return normalized.length;
  }

  displayObjectCount() {
    return Array.isArray(this.layer?.children) ? this.layer.children.length : 0;
  }

  diagnostics() {
    return {
      visibleTrenches: this.visibleCount,
      totalStamped: this.totalStamped,
      textureUpdateCount: this.textureUpdateCount,
      textureWidth: this.canvas?.width || 0,
      textureHeight: this.canvas?.height || 0,
      downsample: this.downsample,
      layerChildCount: this.displayObjectCount(),
    };
  }

  destroy() {
    this.snapshotSignature = null;
    this.sourceRef = null;
    this.sourceTileSize = null;
    this.visibleCount = 0;
    this.totalStamped = 0;
    if (this.sprite) {
      if (this.sprite.parent && typeof this.sprite.parent.removeChild === "function") {
        this.sprite.parent.removeChild(this.sprite);
      }
      this.sprite.destroy?.({ texture: true, textureSource: true });
      this.sprite = null;
    } else if (this.texture) {
      this.texture.destroy?.(true);
      this.texture.source?.destroy?.();
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

export function _initTrenchesForMap(map) {
  this._trenchDecals?.resetForMap(map);
}

export function _drawTrenches(state) {
  const layer = this._trenchDecals;
  if (!layer) return 0;

  const tileSize = (this._map && this._map.tileSize) || state?.map?.tileSize || 32;
  if (!layer.ctx && state?.map) {
    layer.resetForMap(state.map);
  }
  return layer.drawSnapshot(state?.trenches, { tileSize });
}

export function _drawOccupiedTrenches(entities, state) {
  const tileSize = (this._map && this._map.tileSize) || state?.map?.tileSize || 32;
  const trenches = normalizedTrenches(state?.trenches, tileSize);
  const trenchById = new Map(trenches.map((trench) => [trench.id, trench]));
  let drawn = 0;

  for (const entity of entities || []) {
    const trenchId = occupiedTrenchId(entity);
    if (trenchId == null) continue;
    const trench = trenchById.get(trenchId);
    if (!trench) continue;
    const seed = occupiedTrenchSeed(entity, trench);

    const shadow = this._slot?.("trenchOccupantShadows", entity.id);
    if (shadow) {
      shadow.position?.set?.(trench.x, trench.y);
      drawOccupiedTrenchShadow(shadow, trench.radius, seed);
    }

    const lip = this._slot?.("trenchOccupantLips", entity.id);
    if (lip) {
      lip.position?.set?.(trench.x, trench.y);
      drawOccupiedTrenchLip(lip, trench.radius, seed);
    }
    drawn += 1;
  }

  this._recordRenderDiagnostic?.("renderer.trenchOccupants.visible", drawn);
  return drawn;
}

export function normalizedTrenches(trenches, tileSize = 32) {
  if (!Array.isArray(trenches)) return [];
  const fallbackRadius = ENTRENCHMENT_TRENCH_RADIUS_TILES * tileSize;
  const out = [];
  for (const trench of trenches) {
    if (!finiteNumber(trench?.x) || !finiteNumber(trench?.y)) continue;
    const radiusTiles = finiteNumber(trench.radiusTiles)
      ? Math.max(0, trench.radiusTiles)
      : ENTRENCHMENT_TRENCH_RADIUS_TILES;
    const radius = Math.max(TRENCH_MIN_RADIUS_PX, radiusTiles * tileSize || fallbackRadius);
    out.push({
      id: Number.isFinite(Number(trench.id)) ? Number(trench.id) : out.length + 1,
      x: trench.x,
      y: trench.y,
      radius,
    });
  }
  out.sort((a, b) => a.id - b.id);
  return out;
}

export function drawOccupiedTrenchShadow(g, radius, seed = 1) {
  if (!g || !finiteNumber(radius) || radius <= 0) return;
  const rng = mulberry32(seed >>> 0);
  const r = Math.max(TRENCH_MIN_RADIUS_PX, radius);

  gfxFill(g, COLORS.trenchShadow, 0.5);
  gfxPoly(g, irregularLocalPolygon(r * 0.76, {
    points: 18,
    jitter: 0.18,
    rng,
    offsetX: r * 0.03,
    offsetY: r * 0.08,
  }));
  gfxNoFill(g);

  gfxFill(g, COLORS.shadow, 0.2);
  gfxEllipse(g, 0, r * 0.16, r * 0.68, r * 0.34);
  gfxNoFill(g);

  gfxStroke(g, Math.max(2, r * 0.22), COLORS.trenchRim, 0.6);
  gfxPoly(g, irregularLocalPolygon(r * 0.96, {
    points: OCCUPIED_LIP_POINTS + 6,
    jitter: 0.1,
    rng,
    offsetY: r * 0.02,
  }));

  gfxStroke(g, Math.max(1.5, r * 0.13), COLORS.trenchDirt, 0.54);
  gfxPoly(g, irregularLocalPolygon(r * 0.86, {
    points: OCCUPIED_LIP_POINTS + 4,
    jitter: 0.08,
    rng,
    offsetY: r * 0.025,
  }));
}

export function drawOccupiedTrenchLip(g, radius, seed = 1) {
  if (!g || !finiteNumber(radius) || radius <= 0) return;
  const rng = mulberry32((seed ^ 0x9e3779b9) >>> 0);
  const r = Math.max(TRENCH_MIN_RADIUS_PX, radius);

  gfxFill(g, COLORS.trenchRim, 0.94);
  gfxPoly(g, arcBandPolygon(r * 1.1, r * 0.5, Math.PI * 0.06, Math.PI * 0.94, {
    points: OCCUPIED_LIP_POINTS,
    jitter: 0.12,
    rng,
    offsetY: r * 0.08,
  }));
  gfxNoFill(g);

  gfxFill(g, COLORS.trenchDirt, 0.92);
  gfxPoly(g, arcBandPolygon(r * 0.98, r * 0.58, Math.PI * 0.11, Math.PI * 0.89, {
    points: OCCUPIED_LIP_POINTS - 2,
    jitter: 0.1,
    rng,
    offsetY: r * 0.09,
  }));
  gfxNoFill(g);

  gfxFill(g, COLORS.trenchShadow, 0.34);
  gfxPoly(g, arcBandPolygon(r * 0.7, r * 0.5, Math.PI * 0.12, Math.PI * 0.88, {
    points: OCCUPIED_LIP_POINTS - 3,
    jitter: 0.08,
    rng,
    offsetY: r * 0.06,
  }));
  gfxNoFill(g);

  const facetPaths = [];
  for (let i = 0; i < 4; i += 1) {
    const angle = Math.PI * (0.18 + rng() * 0.64);
    const inner = r * (0.6 + rng() * 0.08);
    const outer = r * (0.96 + rng() * 0.1);
    facetPaths.push([
      [Math.cos(angle) * inner, Math.sin(angle) * inner + r * 0.08],
      [Math.cos(angle) * outer, Math.sin(angle) * outer + r * 0.08],
    ]);
  }
  gfxStrokePaths(g, facetPaths, 2, COLORS.trenchDirtLight, 0.42);
}

export function stampTrenchDecal(ctx, trench, downsample = TRENCH_DECAL_TEXTURE_WORLD_SCALE) {
  if (!ctx || !trench || !Number.isFinite(trench.x) || !Number.isFinite(trench.y)) return false;
  const rng = mulberry32(trenchSeed(trench));
  const x = trench.x / downsample;
  const y = trench.y / downsample;
  const r = Math.max(TRENCH_MIN_RADIUS_PX, trench.radius) / downsample;

  drawIrregularDisc(ctx, x, y, r, {
    fillStyle: colorCss(COLORS.trenchDirt, 1),
    jitter: 0.16,
    points: TRENCH_POLYGON_POINTS,
    rng,
  });

  drawIrregularDisc(ctx, x + r * 0.04, y + r * 0.06, r * 0.66, {
    fillStyle: colorCss(COLORS.trenchShadow, 0.58),
    jitter: 0.18,
    points: TRENCH_POLYGON_POINTS - 4,
    rng,
  });

  drawArcBand(ctx, x, y, r * 0.82, r * 0.45, Math.PI * 0.03, Math.PI * 0.93, {
    fillStyle: colorCss(COLORS.trenchShadow, 0.48),
    rng,
  });

  for (let i = 0; i < TRENCH_FACET_COUNT; i += 1) {
    drawDirtFacet(ctx, x, y, r, rng, i);
  }
  return true;
}

function occupiedTrenchId(entity) {
  const id = Number(entity?.occupiedTrenchId);
  return Number.isInteger(id) && id > 0 ? id : null;
}

function occupiedTrenchSeed(entity, trench) {
  let seed = trenchSeed(trench);
  seed = fnvStep(seed, Number(entity?.id) | 0);
  return seed || 1;
}

function irregularLocalPolygon(radius, {
  points,
  jitter,
  rng,
  offsetX = 0,
  offsetY = 0,
}) {
  const out = [];
  const start = rng() * Math.PI * 2;
  for (let i = 0; i < points; i += 1) {
    const angle = start + (Math.PI * 2 * i) / points;
    const localRadius = radius * (1 + (rng() - 0.5) * jitter);
    out.push(
      offsetX + Math.cos(angle) * localRadius,
      offsetY + Math.sin(angle) * localRadius,
    );
  }
  return out;
}

function arcBandPolygon(outerRadius, innerRadius, startAngle, endAngle, {
  points,
  jitter,
  rng,
  offsetY = 0,
}) {
  const out = [];
  for (let i = 0; i <= points; i += 1) {
    const t = i / points;
    const angle = startAngle + (endAngle - startAngle) * t;
    const radius = outerRadius * (1 + (rng() - 0.5) * jitter);
    out.push(Math.cos(angle) * radius, Math.sin(angle) * radius + offsetY);
  }
  for (let i = points; i >= 0; i -= 1) {
    const t = i / points;
    const angle = startAngle + (endAngle - startAngle) * t;
    const radius = innerRadius * (1 + (rng() - 0.5) * jitter);
    out.push(Math.cos(angle) * radius, Math.sin(angle) * radius + offsetY);
  }
  return out;
}

function trenchSnapshotSignature(trenches) {
  let hash = 2166136261;
  for (const trench of trenches) {
    hash = fnvStep(hash, trench.id | 0);
    hash = fnvStep(hash, Math.round(trench.x * 10));
    hash = fnvStep(hash, Math.round(trench.y * 10));
    hash = fnvStep(hash, Math.round(trench.radius * 10));
  }
  return `${trenches.length}:${hash >>> 0}`;
}

function drawIrregularDisc(ctx, x, y, radius, {
  fillStyle,
  jitter,
  points,
  rng,
}) {
  ctx.fillStyle = fillStyle;
  ctx.beginPath();
  const offset = rng() * Math.PI * 2;
  for (let i = 0; i < points; i += 1) {
    const angle = offset + (Math.PI * 2 * i) / points;
    const localRadius = radius * (1 + (rng() - 0.5) * jitter);
    const px = x + Math.cos(angle) * localRadius;
    const py = y + Math.sin(angle) * localRadius;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.closePath();
  ctx.fill();
}

function drawArcBand(ctx, x, y, outerRadius, innerRadius, startAngle, endAngle, {
  fillStyle,
  rng,
}) {
  const steps = 9;
  ctx.fillStyle = fillStyle;
  ctx.beginPath();
  for (let i = 0; i <= steps; i += 1) {
    const t = i / steps;
    const angle = startAngle + (endAngle - startAngle) * t;
    const radius = outerRadius * (1 + (rng() - 0.5) * 0.08);
    const px = x + Math.cos(angle) * radius;
    const py = y + Math.sin(angle) * radius;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  for (let i = steps; i >= 0; i -= 1) {
    const t = i / steps;
    const angle = startAngle + (endAngle - startAngle) * t;
    const radius = innerRadius * (1 + (rng() - 0.5) * 0.12);
    ctx.lineTo(x + Math.cos(angle) * radius, y + Math.sin(angle) * radius);
  }
  ctx.closePath();
  ctx.fill();
}

function drawDirtFacet(ctx, x, y, radius, rng, index) {
  const angle = -Math.PI * 0.95 + rng() * Math.PI * 1.35;
  const dist = radius * (0.22 + rng() * 0.46);
  const cx = x + Math.cos(angle) * dist;
  const cy = y + Math.sin(angle) * dist;
  const length = radius * (0.22 + rng() * 0.2);
  const width = radius * (0.05 + rng() * 0.07);
  const rotation = angle + (rng() - 0.5) * 0.9;
  const c = Math.cos(rotation);
  const s = Math.sin(rotation);
  const points = [
    [-length * 0.55, -width],
    [length * 0.45, -width * (0.7 + rng() * 0.5)],
    [length * 0.55, width * (0.6 + rng() * 0.6)],
    [-length * 0.45, width],
  ];
  ctx.fillStyle = colorCss(COLORS.trenchDirtLight, index % 2 === 0 ? 0.42 : 0.28);
  ctx.beginPath();
  points.forEach(([px, py], pointIndex) => {
    const x2 = cx + px * c - py * s;
    const y2 = cy + px * s + py * c;
    if (pointIndex === 0) ctx.moveTo(x2, y2);
    else ctx.lineTo(x2, y2);
  });
  ctx.closePath();
  ctx.fill();
}

function trenchSeed(trench) {
  let seed = 0x811c9dc5;
  seed = fnvStep(seed, trench.id | 0);
  seed = fnvStep(seed, Math.round(trench.x * 10));
  seed = fnvStep(seed, Math.round(trench.y * 10));
  seed = fnvStep(seed, Math.round(trench.radius * 10));
  return seed || 1;
}

function fnvStep(seed, value) {
  let hash = seed >>> 0;
  hash ^= value >>> 0;
  return Math.imul(hash, 16777619) >>> 0;
}

function mulberry32(seed) {
  let a = seed >>> 0;
  return function next() {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function colorCss(color, alpha = 1) {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return alpha >= 1 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${alpha})`;
}

function updateTexture(texture) {
  if (typeof texture?.update === "function") texture.update();
  else texture?.source?.update?.();
}
