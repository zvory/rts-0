import { STATS } from "../config.js";

export const FORMATION_LINE_MAX_POINTS = 64;

const SAMPLE_STEP_PX = 8;
const MIN_SLOT_SPACING_PX = 24;
const RANK_GAP_PX = 8;

export function appendFormationLinePoint(points, point, { force = false } = {}) {
  if (!Array.isArray(points) || !finitePoint(point)) return points;
  if (points.length === 0) {
    points.push(copyPoint(point));
    return points;
  }
  const last = points[points.length - 1];
  if (!force && Math.hypot(point.x - last.x, point.y - last.y) < SAMPLE_STEP_PX) return points;
  if (points.length >= FORMATION_LINE_MAX_POINTS) points[points.length - 1] = copyPoint(point);
  else points.push(copyPoint(point));
  return points;
}

/** Build provisional, body-aware slots along and beside a freehand polyline. */
export function buildFormationLinePreview(points, entities = []) {
  const line = normalizeLine(points);
  const units = (Array.isArray(entities) ? entities : [])
    .filter((entity) => Number.isInteger(entity?.id))
    .slice()
    .sort((a, b) => a.id - b.id);
  if (line.length < 2 || units.length === 0) return { points: line, slots: [] };

  const radii = units.map(unitRadius);
  const maxRadius = Math.max(...radii, MIN_SLOT_SPACING_PX / 2);
  const spacing = Math.max(MIN_SLOT_SPACING_PX, maxRadius * 2 + RANK_GAP_PX);
  const metrics = lineMetrics(line);
  const columns = metrics.length >= spacing * (units.length - 1)
    ? units.length
    : Math.max(1, Math.min(units.length, Math.floor(metrics.length / spacing) + 1));
  const rankCount = Math.ceil(units.length / columns);
  const slots = [];
  let unitIndex = 0;

  for (let rank = 0; rank < rankCount; rank += 1) {
    const count = Math.min(columns, units.length - unitIndex);
    const rankOffset = (rank - (rankCount - 1) / 2) * spacing;
    for (let column = 0; column < count; column += 1) {
      const distance = count === 1 ? metrics.length / 2 : metrics.length * column / (count - 1);
      const sample = sampleLine(metrics, distance);
      slots.push({
        unitId: units[unitIndex].id,
        x: sample.x + sample.nx * rankOffset,
        y: sample.y + sample.ny * rankOffset,
        radius: radii[unitIndex],
      });
      unitIndex += 1;
    }
  }
  return { points: line, slots };
}

function normalizeLine(points) {
  const out = [];
  for (const point of Array.isArray(points) ? points : []) {
    if (!finitePoint(point)) continue;
    const last = out[out.length - 1];
    if (last && Math.hypot(point.x - last.x, point.y - last.y) < 0.01) continue;
    out.push(copyPoint(point));
    if (out.length >= FORMATION_LINE_MAX_POINTS) break;
  }
  return out;
}

function lineMetrics(points) {
  const segments = [];
  let length = 0;
  for (let i = 1; i < points.length; i += 1) {
    const from = points[i - 1];
    const to = points[i];
    const dx = to.x - from.x;
    const dy = to.y - from.y;
    const segmentLength = Math.hypot(dx, dy);
    if (segmentLength <= 0) continue;
    segments.push({ from, to, dx, dy, length: segmentLength, start: length });
    length += segmentLength;
  }
  return { segments, length };
}

function sampleLine(metrics, distance) {
  const target = Math.max(0, Math.min(metrics.length, distance));
  const segment = metrics.segments.find((candidate) => target <= candidate.start + candidate.length) ||
    metrics.segments[metrics.segments.length - 1];
  if (!segment) return { x: 0, y: 0, nx: 0, ny: 1 };
  const t = Math.max(0, Math.min(1, (target - segment.start) / segment.length));
  return {
    x: segment.from.x + segment.dx * t,
    y: segment.from.y + segment.dy * t,
    nx: -segment.dy / segment.length,
    ny: segment.dx / segment.length,
  };
}

function unitRadius(entity) {
  const size = Number(STATS[entity?.kind]?.size);
  return Number.isFinite(size) && size > 0 ? size : 10;
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}

function copyPoint(point) {
  return { x: point.x, y: point.y };
}
