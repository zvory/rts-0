import { COLORS, ENTRENCHMENT_TRENCH_RADIUS_TILES } from "../config.js";
import { finiteNumber } from "./shared.js";

const TRENCH_CONNECTION_GAP_TILES = 0.55;
const TRENCH_MIN_RADIUS_PX = 4;

export function _drawTrenches(state) {
  const g = this._trenchGfx;
  if (!g) return 0;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.trenches");
  g.clear();

  const tileSize = (this._map && this._map.tileSize) || state?.map?.tileSize || 32;
  const trenches = normalizedTrenches(state?.trenches, tileSize);
  if (trenches.length === 0) {
    this._recordRenderDiagnostic?.("renderer.trenches.visible", 0);
    return 0;
  }

  drawTrenchConnections(g, trenches, tileSize);
  drawTrenchFootprints(g, trenches);
  this._recordRenderDiagnostic?.("renderer.trenches.visible", trenches.length);
  return trenches.length;
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

function drawTrenchConnections(g, trenches, tileSize) {
  if (trenches.length < 2) return;
  const maxRadius = trenches.reduce((max, trench) => Math.max(max, trench.radius), 0);
  const cellSize = Math.max(1, maxRadius * 2 + tileSize * TRENCH_CONNECTION_GAP_TILES);
  const buckets = new Map();

  for (const trench of trenches) {
    const bx = Math.floor(trench.x / cellSize);
    const by = Math.floor(trench.y / cellSize);
    const key = `${bx},${by}`;
    let bucket = buckets.get(key);
    if (!bucket) {
      bucket = [];
      buckets.set(key, bucket);
    }
    bucket.push(trench);
  }

  const pairs = [];
  for (const trench of trenches) {
    const bx = Math.floor(trench.x / cellSize);
    const by = Math.floor(trench.y / cellSize);
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -1; dx <= 1; dx += 1) {
        const bucket = buckets.get(`${bx + dx},${by + dy}`);
        if (!bucket) continue;
        for (const other of bucket) {
          if (other.id <= trench.id) continue;
          const allowed = trench.radius + other.radius + tileSize * TRENCH_CONNECTION_GAP_TILES;
          if (distanceSquared(trench, other) <= allowed * allowed) pairs.push([trench, other]);
        }
      }
    }
  }

  for (const [a, b] of pairs) {
    const width = Math.max(8, Math.min(a.radius, b.radius) * 1.45);
    g.lineStyle(width + 5, COLORS.trenchShadow, 0.34);
    g.moveTo(a.x, a.y);
    g.lineTo(b.x, b.y);
    g.lineStyle(width, COLORS.trenchDirt, 0.74);
    g.moveTo(a.x, a.y);
    g.lineTo(b.x, b.y);
    g.lineStyle(Math.max(2, width * 0.18), COLORS.trenchRim, 0.22);
    g.moveTo(a.x, a.y);
    g.lineTo(b.x, b.y);
  }
}

function drawTrenchFootprints(g, trenches) {
  for (const trench of trenches) {
    const r = trench.radius;
    g.beginFill(COLORS.trenchShadow, 0.36);
    g.drawEllipse(trench.x + r * 0.08, trench.y + r * 0.12, r * 1.18, r * 0.82);
    g.endFill();

    g.beginFill(COLORS.trenchDirt, 0.82);
    g.drawEllipse(trench.x, trench.y, r, r * 0.66);
    g.endFill();

    g.lineStyle(Math.max(2, r * 0.14), COLORS.trenchRim, 0.48);
    g.drawEllipse(trench.x, trench.y, r * 0.96, r * 0.62);

    g.lineStyle(2, COLORS.trenchDirtLight, 0.28);
    g.moveTo(trench.x - r * 0.42, trench.y - r * 0.08);
    g.lineTo(trench.x + r * 0.36, trench.y - r * 0.2);
    g.moveTo(trench.x - r * 0.34, trench.y + r * 0.18);
    g.lineTo(trench.x + r * 0.42, trench.y + r * 0.08);
  }
}

function distanceSquared(a, b) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return dx * dx + dy * dy;
}
