import { normalizeDimensions } from "./symmetry.js";

export function lineTiles(from, to) {
  const start = finitePoint(from);
  const end = finitePoint(to);
  if (!start || !end) return [];
  const out = [];
  let x = Math.round(start.x);
  let y = Math.round(start.y);
  const targetX = Math.round(end.x);
  const targetY = Math.round(end.y);
  const dx = Math.abs(targetX - x);
  const sx = x < targetX ? 1 : -1;
  const dy = -Math.abs(targetY - y);
  const sy = y < targetY ? 1 : -1;
  let error = dx + dy;
  while (true) {
    out.push({ x, y });
    if (x === targetX && y === targetY) break;
    const twice = error * 2;
    if (twice >= dy) { error += dy; x += sx; }
    if (twice <= dx) { error += dx; y += sy; }
  }
  return out;
}

export function rectTiles(dimensions, from, to) {
  const map = normalizeDimensions(dimensions);
  const start = finitePoint(from);
  const end = finitePoint(to);
  if (!map || !start || !end) return [];
  const tiles = [];
  const minX = Math.max(0, Math.floor(Math.min(start.x, end.x)));
  const maxX = Math.min(map.width - 1, Math.ceil(Math.max(start.x, end.x)));
  const minY = Math.max(0, Math.floor(Math.min(start.y, end.y)));
  const maxY = Math.min(map.height - 1, Math.ceil(Math.max(start.y, end.y)));
  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) tiles.push({ x, y });
  }
  return tiles;
}

export function pathTiles(dimensions, operation) {
  const map = requiredDimensions(dimensions);
  const points = (operation.points || []).map((value, index) => tuplePoint(value, `points[${index}]`));
  if (!points.length) throw new Error("stroke and road operations need at least one point");
  const brushWidth = Math.max(0.5, finiteNumber(operation.width, 1));
  const roughness = Math.max(0, finiteNumber(operation.roughness, 0));
  const radius = brushWidth / 2;
  const seed = integer(operation.seed, 1);
  const padding = Math.ceil(radius + roughness + 2);
  const minX = Math.max(0, Math.floor(Math.min(...points.map(({ x }) => x)) - padding));
  const maxX = Math.min(map.width - 1, Math.ceil(Math.max(...points.map(({ x }) => x)) + padding));
  const minY = Math.max(0, Math.floor(Math.min(...points.map(({ y }) => y)) - padding));
  const maxY = Math.min(map.height - 1, Math.ceil(Math.max(...points.map(({ y }) => y)) + padding));
  const tiles = [];
  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      const { distance, segmentIndex } = distanceToPath(x, y, points);
      const edge = radius + organicNoise(x, y, seed) * roughness;
      if (distance <= edge) tiles.push({ x, y, segmentIndex, distance });
    }
  }
  return { points, tiles, radius };
}

export function blobTiles(dimensions, operation) {
  const map = requiredDimensions(dimensions);
  const center = tuplePoint(operation.center, "center");
  const radiusValue = Array.isArray(operation.radius) ? operation.radius : [operation.radius, operation.radius];
  const radiusX = Math.max(0.5, finiteNumber(radiusValue[0], 1));
  const radiusY = Math.max(0.5, finiteNumber(radiusValue[1], 1));
  const roughness = Math.max(0, Math.min(1, finiteNumber(operation.roughness, 0.25)));
  const seed = integer(operation.seed, 1);
  const padding = Math.ceil(Math.max(radiusX, radiusY) * roughness * 0.35 + 2);
  const tiles = [];
  for (let y = Math.max(0, Math.floor(center.y - radiusY - padding)); y <= Math.min(map.height - 1, Math.ceil(center.y + radiusY + padding)); y += 1) {
    for (let x = Math.max(0, Math.floor(center.x - radiusX - padding)); x <= Math.min(map.width - 1, Math.ceil(center.x + radiusX + padding)); x += 1) {
      const normalizedDistance = Math.hypot((x - center.x) / radiusX, (y - center.y) / radiusY);
      const boundary = 1 + organicNoise(x, y, seed) * roughness * 0.3;
      if (normalizedDistance <= boundary) tiles.push({ x, y });
    }
  }
  return tiles;
}

export function tuplePoint(value, label = "point") {
  if (!Array.isArray(value) || value.length !== 2 || !value.every(Number.isFinite)) {
    throw new Error(`${label} must be [x, y]`);
  }
  return { x: finiteNumber(value[0]), y: finiteNumber(value[1]) };
}

function distanceToPath(x, y, points) {
  if (points.length === 1) return { distance: Math.hypot(x - points[0].x, y - points[0].y), segmentIndex: 0 };
  let distance = Infinity;
  let segmentIndex = 0;
  for (let index = 1; index < points.length; index += 1) {
    const candidate = distanceToSegment(x, y, points[index - 1], points[index]);
    if (candidate < distance) {
      distance = candidate;
      segmentIndex = index - 1;
    }
  }
  return { distance, segmentIndex };
}

function distanceToSegment(px, py, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0
    ? 0
    : Math.max(0, Math.min(1, ((px - start.x) * dx + (py - start.y) * dy) / lengthSquared));
  return Math.hypot(px - (start.x + t * dx), py - (start.y + t * dy));
}

function organicNoise(x, y, seed) {
  return valueNoise(x, y, seed, 11) * 0.65 + valueNoise(x, y, seed + 7919, 4) * 0.35;
}

function valueNoise(x, y, seed, scale) {
  const scaledX = x / scale;
  const scaledY = y / scale;
  const x0 = Math.floor(scaledX);
  const y0 = Math.floor(scaledY);
  const tx = smoothstep(scaledX - x0);
  const ty = smoothstep(scaledY - y0);
  const a = hashUnit(x0, y0, seed);
  const b = hashUnit(x0 + 1, y0, seed);
  const c = hashUnit(x0, y0 + 1, seed);
  const d = hashUnit(x0 + 1, y0 + 1, seed);
  const top = a + (b - a) * tx;
  const bottom = c + (d - c) * tx;
  return (top + (bottom - top) * ty) * 2 - 1;
}

function hashUnit(x, y, seed) {
  let value = Math.imul(x | 0, 0x1f123bb5) ^ Math.imul(y | 0, 0x5f356495) ^ Math.imul(seed | 0, 0x6c8e9cf5);
  value = Math.imul(value ^ (value >>> 15), 0x2c1b3c6d);
  value = Math.imul(value ^ (value >>> 12), 0x297a2d39);
  return ((value ^ (value >>> 15)) >>> 0) / 0xffffffff;
}

function smoothstep(value) {
  return value * value * (3 - 2 * value);
}

function finitePoint(value) {
  const x = Number(value?.x);
  const y = Number(value?.y);
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function finiteNumber(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function integer(value, fallback = 0) {
  return Math.trunc(finiteNumber(value, fallback));
}

function requiredDimensions(value) {
  const map = normalizeDimensions(value);
  if (!map) throw new Error("map dimensions must be positive integers");
  return map;
}
