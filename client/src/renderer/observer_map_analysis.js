import { finiteNumber } from "./shared.js";

const LABEL_MAX_LENGTH = 36;
const LABEL_STYLE = Object.freeze({
  fontFamily: "monospace",
  fontSize: 12,
  fontWeight: "800",
  fill: 0xe7dfc5,
  stroke: 0x0f1115,
  strokeThickness: 3,
  align: "center",
});
const DEFAULT_COMPONENT_FILL = 0x3da5d9;
const DEFAULT_MARKER_COLOR = 0xe7dfc5;

export function _drawObserverMapAnalysisOverlay(model, { camera = null } = {}) {
  const gfx = this._observerMapAnalysisGfx;
  const labelLayer = this._observerMapAnalysisLabels;
  if (!gfx || !labelLayer) return 0;

  gfx.clear();
  const analysis = normalizeMapAnalysis(model?.analysis);
  if (!analysis) {
    sweepLabels(this._observerMapAnalysisLabelPool, labelLayer, new Set());
    return 0;
  }

  const visibleLayers = normalizeVisibleLayers(model?.visibleLayers);
  const seenLabels = new Set();
  let primitiveCount = 0;
  const showLabels = visibleLayers.labels !== false;
  const lineWidth = lineWidthForCamera(camera);

  for (const layer of analysis.layers) {
    const visible = hasOwn(visibleLayers, layer.id)
      ? visibleLayers[layer.id] === true
      : layer.defaultVisible !== false;
    if (!visible) continue;

    for (const primitive of layer.primitives) {
      if (primitive.kind === "tileRect") {
        drawTileRect(gfx, primitive, analysis.tileSize, lineWidth, layer.id);
        primitiveCount += 1;
        if (showLabels && primitive.label) {
          const x = (primitive.tileX + primitive.tileW * 0.5) * analysis.tileSize;
          const y = (primitive.tileY + primitive.tileH * 0.5) * analysis.tileSize;
          drawLabel(this, `${layer.id}:${primitive.id}`, primitive.label, x, y, primitive.stroke, camera, seenLabels);
        }
      } else if (primitive.kind === "marker") {
        drawMarker(gfx, primitive, lineWidth);
        primitiveCount += 1;
        if (showLabels && primitive.label) {
          drawLabel(
            this,
            `${layer.id}:${primitive.id}`,
            primitive.label,
            primitive.x,
            primitive.y - primitive.radius - 6,
            primitive.color,
            camera,
            seenLabels,
          );
        }
      }
    }
  }

  sweepLabels(this._observerMapAnalysisLabelPool, labelLayer, seenLabels);
  this._recordRenderDiagnostic?.("renderer.observerMapAnalysis.primitives", primitiveCount);
  this._recordRenderDiagnostic?.("renderer.observerMapAnalysis.labels", seenLabels.size);
  return primitiveCount;
}

function drawTileRect(g, primitive, tileSize, lineWidth, layerId) {
  const x = primitive.tileX * tileSize;
  const y = primitive.tileY * tileSize;
  const w = primitive.tileW * tileSize;
  const h = primitive.tileH * tileSize;
  const fill = hexToInt(primitive.fill, DEFAULT_COMPONENT_FILL);
  const stroke = hexToInt(primitive.stroke, fill);
  const strokeAlpha = layerId === "regions" ? 0.16 : 0.72;
  const strokeWidth = layerId === "regions" ? Math.max(0.5, lineWidth * 0.45) : lineWidth;
  g.lineStyle(strokeWidth, stroke, strokeAlpha);
  g.beginFill(fill, clamp(primitive.alpha, 0.04, 0.32));
  g.drawRect(x, y, w, h);
  g.endFill();
}

function drawMarker(g, primitive, lineWidth) {
  const color = hexToInt(primitive.color, DEFAULT_MARKER_COLOR);
  const radius = clamp(primitive.radius, 4, 96);
  const stroke = Math.max(lineWidth, 2);
  g.lineStyle(stroke, 0x0f1115, 0.9);
  g.beginFill(color, 0.82);
  if (primitive.shape === "diamond") {
    g.drawPolygon([
      primitive.x, primitive.y - radius,
      primitive.x + radius, primitive.y,
      primitive.x, primitive.y + radius,
      primitive.x - radius, primitive.y,
    ]);
  } else if (primitive.shape === "square") {
    g.drawRect(primitive.x - radius, primitive.y - radius, radius * 2, radius * 2);
  } else {
    g.drawCircle(primitive.x, primitive.y, radius);
  }
  g.endFill();
  g.lineStyle(Math.max(1, lineWidth), color, 0.95);
  g.drawCircle(primitive.x, primitive.y, radius + 3);
}

function drawLabel(renderer, id, text, x, y, color, camera, seen) {
  const pool = renderer._observerMapAnalysisLabelPool;
  const layer = renderer._observerMapAnalysisLabels;
  const PixiText = globalThis.PIXI?.Text;
  if (!pool || !layer || !PixiText) return;
  let label = pool.get(id);
  if (!label) {
    label = new PixiText("", LABEL_STYLE);
    label.anchor?.set?.(0.5, 1);
    pool.set(id, label);
    layer.addChild(label);
    renderer._recordRenderDiagnostic?.("renderer.observerMapAnalysis.label.created");
  }
  label.text = String(text || "").slice(0, LABEL_MAX_LENGTH);
  label.style.fill = hexToInt(color, DEFAULT_MARKER_COLOR);
  label.visible = true;
  label.alpha = 0.96;
  label.position?.set?.(x, y);
  label.scale?.set?.(labelScaleForCamera(camera));
  seen.add(id);
}

function sweepLabels(pool, layer, seen) {
  if (!pool) return;
  for (const [id, label] of pool) {
    if (seen.has(id)) continue;
    layer?.removeChild?.(label);
    label.destroy?.();
    pool.delete(id);
  }
}

function normalizeMapAnalysis(value) {
  if (!value || typeof value !== "object") return null;
  const tileSize = finitePositive(value.tileSize, 32);
  const layers = Array.isArray(value.layers)
    ? value.layers.map(normalizeLayer).filter(Boolean)
    : [];
  return layers.length
    ? {
      mapWidth: finitePositive(value.mapWidth, 0),
      mapHeight: finitePositive(value.mapHeight, 0),
      tileSize,
      layers,
    }
    : null;
}

function normalizeLayer(value) {
  const id = safeId(value?.id);
  if (!id) return null;
  const primitives = Array.isArray(value.primitives)
    ? value.primitives.map(normalizePrimitive).filter(Boolean)
    : [];
  return {
    id,
    label: String(value?.label || id).slice(0, LABEL_MAX_LENGTH),
    defaultVisible: value?.defaultVisible !== false,
    primitives,
  };
}

function normalizePrimitive(value) {
  if (!value || typeof value !== "object") return null;
  if (value.kind === "tileRect") {
    const tileX = finitePositive(value.tileX, 0);
    const tileY = finitePositive(value.tileY, 0);
    const tileW = finitePositive(value.tileW, 0);
    const tileH = finitePositive(value.tileH, 0);
    if (tileW <= 0 || tileH <= 0) return null;
    return {
      kind: "tileRect",
      id: safeId(value.id) || `tile:${tileX}:${tileY}:${tileW}:${tileH}`,
      tileX,
      tileY,
      tileW,
      tileH,
      fill: safeHex(value.fill, "#3da5d9"),
      stroke: safeHex(value.stroke, safeHex(value.fill, "#3da5d9")),
      alpha: clamp(value.alpha, 0.04, 0.32),
      label: labelText(value.label),
    };
  }
  if (value.kind === "marker") {
    const x = finiteNumber(value.x) ? value.x : null;
    const y = finiteNumber(value.y) ? value.y : null;
    if (x == null || y == null) return null;
    return {
      kind: "marker",
      id: safeId(value.id) || `marker:${Math.round(x)}:${Math.round(y)}`,
      x,
      y,
      radius: clamp(value.radius, 4, 96),
      shape: ["diamond", "square", "circle"].includes(value.shape) ? value.shape : "circle",
      color: safeHex(value.color, "#e7dfc5"),
      label: labelText(value.label),
    };
  }
  return null;
}

function normalizeVisibleLayers(value) {
  const out = {};
  if (!value || typeof value !== "object") return out;
  for (const [key, visible] of Object.entries(value)) {
    const id = safeId(key);
    if (id) out[id] = visible === true;
  }
  return out;
}

function hasOwn(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function finitePositive(value, fallback) {
  const number = Math.trunc(Number(value));
  return Number.isFinite(number) && number >= 0 ? number : fallback;
}

function safeId(value) {
  const id = String(value || "").trim();
  return /^[A-Za-z0-9:_-]{1,64}$/.test(id) ? id : "";
}

function labelText(value) {
  const text = String(value || "").trim();
  return text ? text.slice(0, LABEL_MAX_LENGTH) : null;
}

function safeHex(value, fallback) {
  const text = String(value || "").trim();
  return /^#[0-9a-fA-F]{6}$/.test(text) ? text : fallback;
}

function hexToInt(value, fallback) {
  const text = safeHex(value, "");
  return text ? Number.parseInt(text.slice(1), 16) : fallback;
}

function clamp(value, min, max) {
  const number = Number(value);
  if (!Number.isFinite(number)) return min;
  return Math.min(max, Math.max(min, number));
}

function lineWidthForCamera(camera) {
  const zoom = finiteNumber(camera?.zoom) && camera.zoom > 0 ? camera.zoom : 1;
  return clamp(2 / zoom, 0.75, 3);
}

function labelScaleForCamera(camera) {
  const zoom = finiteNumber(camera?.zoom) && camera.zoom > 0 ? camera.zoom : 1;
  return clamp(1 / zoom, 0.5, 1.5);
}
