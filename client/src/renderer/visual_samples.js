import { COLORS, ENTRENCHMENT_TRENCH_RADIUS_TILES } from "../config.js";
import { finiteNumber } from "./shared.js";
import { drawOccupiedTrenchLip, drawOccupiedTrenchShadow } from "./trenches.js";

const SAMPLE_ID_RE = /^[A-Za-z0-9_-]{1,64}$/;
const LABEL_MAX_LENGTH = 28;
const MIN_TRENCH_RADIUS_PX = 4;
const MAX_TRENCH_RADIUS_PX = 96;
const DEFAULT_TRENCH_VARIANT = "basin";
const EMPTY_SAMPLES = Object.freeze([]);

const LABEL_STYLE = Object.freeze({
  fontFamily: "monospace",
  fontSize: 12,
  fontWeight: "700",
  fill: 0xf2dfae,
  stroke: 0x16110c,
  strokeThickness: 3,
  align: "center",
});

const TRENCH_VARIANTS = Object.freeze({
  basin: Object.freeze({
    outerScale: 1,
    midScale: 0.82,
    innerScale: 0.54,
    jitter: 0.17,
    rimAlpha: 0.88,
    dirtAlpha: 0.96,
    shadowAlpha: 0.56,
    lightAlpha: 0.28,
    points: 22,
    facets: 4,
  }),
  wide_shadow: Object.freeze({
    outerScale: 1.07,
    midScale: 0.9,
    innerScale: 0.7,
    jitter: 0.14,
    rimAlpha: 0.78,
    dirtAlpha: 0.92,
    shadowAlpha: 0.68,
    lightAlpha: 0.2,
    points: 24,
    facets: 3,
  }),
  hard_rim: Object.freeze({
    outerScale: 1.03,
    midScale: 0.76,
    innerScale: 0.48,
    jitter: 0.11,
    rimAlpha: 0.98,
    dirtAlpha: 0.92,
    shadowAlpha: 0.52,
    lightAlpha: 0.36,
    points: 18,
    facets: 5,
  }),
  broken_earth: Object.freeze({
    outerScale: 1.14,
    midScale: 0.84,
    innerScale: 0.5,
    jitter: 0.31,
    rimAlpha: 0.9,
    dirtAlpha: 0.88,
    shadowAlpha: 0.58,
    lightAlpha: 0.3,
    points: 26,
    facets: 7,
  }),
  compact_dark: Object.freeze({
    outerScale: 0.9,
    midScale: 0.72,
    innerScale: 0.58,
    jitter: 0.18,
    rimAlpha: 0.82,
    dirtAlpha: 0.88,
    shadowAlpha: 0.72,
    lightAlpha: 0.16,
    points: 20,
    facets: 3,
  }),
});

export class VisualSampleLayer {
  constructor({
    sampleLayer,
    labelLayer,
    pixi = globalThis.PIXI,
    recordDiagnostic = null,
    recordError = null,
  } = {}) {
    this.sampleLayer = sampleLayer;
    this.labelLayer = labelLayer;
    this.pixi = pixi;
    this.recordDiagnostic = recordDiagnostic;
    this.recordError = recordError;
    this.samplePool = new Map();
    this.labelPool = new Map();
    this.sourceRef = null;
    this.sourceTileSize = null;
    this.cached = { samples: [], errors: [] };
    this.visibleCount = 0;
    this.errorCount = 0;
    this.totalRendered = 0;
  }

  render(rendererModel, { tileSize = 32, camera = null } = {}) {
    const normalized = this.normalizedFor(rendererModel, tileSize);
    const seen = new Set();
    for (const sample of normalized.samples) {
      seen.add(sample.id);
      if (sample.kind === "trench") this.drawTrench(sample);
      this.drawLabel(sample, camera);
    }
    this.sweepPool(this.samplePool, this.sampleLayer, seen, "sample");
    this.sweepPool(this.labelPool, this.labelLayer, seen, "label");
    this.visibleCount = normalized.samples.length;
    this.errorCount = normalized.errors.length;
    this.totalRendered += normalized.samples.length;
    this.recordDiagnostic?.("renderer.visualSamples.visible", normalized.samples.length);
    this.recordDiagnostic?.("renderer.visualSamples.invalid", normalized.errors.length);
    this.recordDiagnostic?.("renderer.visualSamples.displayObjects", this.displayObjectCount());
    return normalized.samples.length;
  }

  normalizedFor(rendererModel, tileSize) {
    const source = staticSampleSource(rendererModel);
    if (source === this.sourceRef && tileSize === this.sourceTileSize) return this.cached;
    const normalized = normalizeStaticVisualSamples(rendererModel, { tileSize });
    this.sourceRef = source;
    this.sourceTileSize = tileSize;
    this.cached = normalized;
    this.publishErrors(normalized.errors);
    return normalized;
  }

  publishErrors(errors) {
    if (!errors.length) {
      if (globalThis.__rtsVisualSampleErrors) delete globalThis.__rtsVisualSampleErrors;
      return;
    }
    const latest = errors[errors.length - 1];
    globalThis.__rtsVisualSampleErrors = {
      total: errors.length,
      latest,
      errors,
    };
    for (const error of errors) {
      this.recordError?.(
        `visualSample:${error.id || error.index}`,
        new Error(error.message),
      );
    }
  }

  drawTrench(sample) {
    const g = this.slotGraphics(sample.id);
    if (!g) return;
    const variant = TRENCH_VARIANTS[sample.variant] || TRENCH_VARIANTS[DEFAULT_TRENCH_VARIANT];
    const seed = hashString(`${sample.id}:${sample.variant}`);
    const rng = mulberry32(seed);
    g.clear();
    g.visible = true;
    g.alpha = sample.alpha;
    g.position?.set?.(sample.x, sample.y);

    g.beginFill(COLORS.shadow, 0.2);
    g.drawPolygon(irregularPolygon(sample.radius * variant.outerScale, {
      points: variant.points,
      jitter: variant.jitter,
      rng,
      offsetX: sample.radius * 0.08,
      offsetY: sample.radius * 0.14,
    }));
    g.endFill();

    g.beginFill(COLORS.trenchRim, variant.rimAlpha);
    g.drawPolygon(irregularPolygon(sample.radius * variant.outerScale, {
      points: variant.points,
      jitter: variant.jitter,
      rng,
    }));
    g.endFill();

    g.beginFill(COLORS.trenchDirt, variant.dirtAlpha);
    g.drawPolygon(irregularPolygon(sample.radius * variant.midScale, {
      points: Math.max(14, variant.points - 3),
      jitter: variant.jitter * 0.75,
      rng,
      offsetX: sample.radius * -0.02,
      offsetY: sample.radius * 0.02,
    }));
    g.endFill();

    g.beginFill(COLORS.trenchShadow, variant.shadowAlpha);
    g.drawPolygon(irregularPolygon(sample.radius * variant.innerScale, {
      points: Math.max(12, variant.points - 6),
      jitter: variant.jitter,
      rng,
      offsetX: sample.radius * 0.05,
      offsetY: sample.radius * 0.08,
    }));
    g.endFill();

    drawFacets(g, sample.radius, variant, rng);
    if (sample.occupied) {
      drawOccupiedTrenchShadow(g, sample.radius, seed);
      drawOccupiedTrenchLip(g, sample.radius, seed);
    }
  }

  drawLabel(sample, camera) {
    const label = this.slotLabel(sample.id);
    if (!label) return;
    label.text = sample.label;
    label.visible = true;
    label.alpha = 0.95;
    label.position?.set?.(sample.x, sample.y - sample.radius - sample.labelOffsetY);
    label.scale?.set?.(labelScaleForCamera(camera, sample.x, sample.y));
  }

  slotGraphics(id) {
    if (!this.pixi?.Graphics || !this.sampleLayer) return null;
    let g = this.samplePool.get(id);
    if (!g) {
      g = new this.pixi.Graphics();
      this.samplePool.set(id, g);
      this.sampleLayer.addChild?.(g);
    }
    return g;
  }

  slotLabel(id) {
    if (!this.pixi?.Text || !this.labelLayer) return null;
    let text = this.labelPool.get(id);
    if (!text) {
      text = new this.pixi.Text("", LABEL_STYLE);
      text.anchor?.set?.(0.5, 1);
      this.labelPool.set(id, text);
      this.labelLayer.addChild?.(text);
    }
    return text;
  }

  sweepPool(pool, layer, seen, diagnosticName) {
    for (const [id, display] of pool) {
      if (seen.has(id)) continue;
      layer?.removeChild?.(display);
      display.destroy?.();
      pool.delete(id);
      this.recordDiagnostic?.(`renderer.visualSamples.destroyed.${diagnosticName}`);
    }
  }

  displayObjectCount() {
    return this.samplePool.size + this.labelPool.size;
  }

  diagnostics() {
    return {
      visibleSamples: this.visibleCount,
      invalidSamples: this.errorCount,
      totalRendered: this.totalRendered,
      sampleDisplayObjects: this.samplePool.size,
      labelDisplayObjects: this.labelPool.size,
      layerChildCount:
        (this.sampleLayer?.children?.length || 0) +
        (this.labelLayer?.children?.length || 0),
    };
  }

  destroy() {
    for (const display of this.samplePool.values()) {
      if (display.parent?.removeChild) display.parent.removeChild(display);
      display.destroy?.();
    }
    for (const label of this.labelPool.values()) {
      if (label.parent?.removeChild) label.parent.removeChild(label);
      label.destroy?.();
    }
    this.samplePool.clear();
    this.labelPool.clear();
    this.sourceRef = null;
    this.cached = { samples: [], errors: [] };
    this.visibleCount = 0;
    this.errorCount = 0;
  }
}

export function _drawVisualSamples(rendererModel, { state = null, camera = null } = {}) {
  return this._visualSamples?.render(rendererModel, {
    tileSize: (this._map && this._map.tileSize) || state?.map?.tileSize || 32,
    camera,
  }) || 0;
}

export function normalizeStaticVisualSamples(rendererModel, { tileSize = 32 } = {}) {
  const source = staticSampleSource(rendererModel);
  const samples = [];
  const errors = [];
  const seenIds = new Set();
  const normalizedTileSize = finiteNumber(tileSize) && tileSize > 0 ? tileSize : 32;
  for (let index = 0; index < source.length; index += 1) {
    const raw = source[index];
    const normalized = normalizeStaticVisualSample(raw, {
      index,
      tileSize: normalizedTileSize,
      seenIds,
    });
    if (normalized.error) {
      errors.push(normalized.error);
      continue;
    }
    samples.push(normalized.sample);
  }
  return { samples, errors };
}

function normalizeStaticVisualSample(raw, { index, tileSize, seenIds }) {
  const id = typeof raw?.id === "string" ? raw.id.trim() : "";
  if (!id || !SAMPLE_ID_RE.test(id)) return invalidSample(index, id, "invalid-id");
  if (seenIds.has(id)) return invalidSample(index, id, "duplicate-id");
  seenIds.add(id);

  const kind = typeof raw?.kind === "string"
    ? raw.kind.trim()
    : typeof raw?.type === "string"
      ? raw.type.trim()
      : "";
  if (kind !== "trench") return invalidSample(index, id, "unsupported-kind");

  const x = Number(raw?.x);
  const y = Number(raw?.y);
  if (!finiteNumber(x) || !finiteNumber(y)) return invalidSample(index, id, "invalid-position");

  const radiusTiles = finiteNumber(raw?.radiusTiles)
    ? Math.max(0, Number(raw.radiusTiles))
    : ENTRENCHMENT_TRENCH_RADIUS_TILES;
  const rawRadius = finiteNumber(raw?.radius) ? Number(raw.radius) : radiusTiles * tileSize;
  const radius = clamp(rawRadius, MIN_TRENCH_RADIUS_PX, MAX_TRENCH_RADIUS_PX);
  const variant = typeof raw?.variant === "string" && raw.variant.trim()
    ? raw.variant.trim()
    : DEFAULT_TRENCH_VARIANT;
  if (!TRENCH_VARIANTS[variant]) return invalidSample(index, id, "unknown-variant");

  return {
    sample: {
      id,
      kind,
      label: normalizeLabel(raw?.label, id),
      x,
      y,
      radius,
      radiusTiles,
      variant,
      occupied: Boolean(raw?.occupied),
      alpha: clamp(finiteNumber(raw?.alpha) ? Number(raw.alpha) : 1, 0.05, 1),
      labelOffsetY: clamp(finiteNumber(raw?.labelOffsetY) ? Number(raw.labelOffsetY) : 12, 0, 80),
    },
  };
}

function staticSampleSource(rendererModel) {
  if (Array.isArray(rendererModel)) return rendererModel;
  if (Array.isArray(rendererModel?.staticSamples)) return rendererModel.staticSamples;
  return EMPTY_SAMPLES;
}

function invalidSample(index, id, reason) {
  const displayId = id || `index-${index}`;
  return {
    error: {
      index,
      id,
      reason,
      message: `Invalid visual static sample ${displayId}: ${reason}.`,
    },
  };
}

function normalizeLabel(value, fallback) {
  const label = typeof value === "string" && value.trim() ? value.trim() : fallback;
  return label.slice(0, LABEL_MAX_LENGTH);
}

function labelScaleForCamera(camera, x, y) {
  const extent = camera?.projectedExtent?.({ x, y, heightPx: 0 }, 1, 1);
  const scale = finiteNumber(extent?.scaleX) && extent.scaleX > 0 ? extent.scaleX : 1;
  return clamp(1 / scale, 0.5, 1.6);
}

function drawFacets(g, radius, variant, rng) {
  g.lineStyle(2, COLORS.trenchDirtLight, variant.lightAlpha);
  for (let i = 0; i < variant.facets; i += 1) {
    const angle = rng() * Math.PI * 2;
    const inner = radius * (0.38 + rng() * 0.16);
    const outer = radius * (0.74 + rng() * 0.18);
    g.moveTo(Math.cos(angle) * inner, Math.sin(angle) * inner);
    g.lineTo(Math.cos(angle) * outer, Math.sin(angle) * outer);
  }
}

function irregularPolygon(radius, { points, jitter, rng, offsetX = 0, offsetY = 0 }) {
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

function hashString(value) {
  let hash = 2166136261;
  const text = String(value || "");
  for (let i = 0; i < text.length; i += 1) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function mulberry32(seed) {
  let value = seed >>> 0;
  return function next() {
    value += 0x6d2b79f5;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function clamp(value, min, max) {
  if (!finiteNumber(value)) return min;
  if (value < min) return min;
  if (value > max) return max;
  return value;
}
