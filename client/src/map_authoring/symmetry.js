export const MAP_AUTHORING_SYMMETRY = Object.freeze({
  NONE: "none",
  HORIZONTAL: "horizontal",
  VERTICAL: "vertical",
  HALF_TURN: "halfTurn",
  THREE_WAY: "threeWay",
  RADIAL: "radial",
  QUADRANT_MIRROR: "quadrantMirror",
  DIAGONAL_MAIN: "diagonalMain",
  DIAGONAL_ANTI: "diagonalAnti",
});

const SIN_120 = Math.sqrt(3) / 2;
const TRANSFORMS = Object.freeze({
  [MAP_AUTHORING_SYMMETRY.NONE]: ["identity"],
  [MAP_AUTHORING_SYMMETRY.HORIZONTAL]: ["identity", "horizontal"],
  [MAP_AUTHORING_SYMMETRY.VERTICAL]: ["identity", "vertical"],
  [MAP_AUTHORING_SYMMETRY.HALF_TURN]: ["identity", "rotate180"],
  [MAP_AUTHORING_SYMMETRY.THREE_WAY]: ["identity", "rotate120", "rotate240"],
  [MAP_AUTHORING_SYMMETRY.RADIAL]: ["identity", "rotate90", "rotate180", "rotate270"],
  [MAP_AUTHORING_SYMMETRY.QUADRANT_MIRROR]: ["identity", "horizontal", "vertical", "rotate180"],
  [MAP_AUTHORING_SYMMETRY.DIAGONAL_MAIN]: ["identity", "diagonalMain"],
  [MAP_AUTHORING_SYMMETRY.DIAGONAL_ANTI]: ["identity", "diagonalAnti"],
});

export function normalizeDimensions(value) {
  if (typeof value === "number") {
    const size = Math.trunc(Number(value));
    return size > 0 ? { width: size, height: size } : null;
  }
  const width = Math.trunc(Number(value?.width));
  const height = Math.trunc(Number(value?.height));
  return width > 0 && height > 0 ? { width, height } : null;
}

export function symmetrySupported(dimensions, symmetry) {
  const map = normalizeDimensions(dimensions);
  if (!map) return false;
  const normalized = normalizeSymmetry(symmetry);
  return map.width === map.height || ![
    MAP_AUTHORING_SYMMETRY.THREE_WAY,
    MAP_AUTHORING_SYMMETRY.RADIAL,
    MAP_AUTHORING_SYMMETRY.DIAGONAL_MAIN,
    MAP_AUTHORING_SYMMETRY.DIAGONAL_ANTI,
  ].includes(normalized);
}

export function symmetryTransforms(dimensions, symmetry) {
  const normalized = normalizeSymmetry(symmetry);
  return symmetrySupported(dimensions, normalized)
    ? TRANSFORMS[normalized]
    : TRANSFORMS[MAP_AUTHORING_SYMMETRY.NONE];
}

export function boundedPoint(point, dimensions) {
  const map = normalizeDimensions(dimensions);
  if (!map) return null;
  const x = Math.round(Number(point?.x));
  const y = Math.round(Number(point?.y));
  return Number.isFinite(x) && Number.isFinite(y)
    && x >= 0 && y >= 0 && x < map.width && y < map.height
    ? { x, y }
    : null;
}

export function transformPoint(point, dimensions, transform) {
  const map = normalizeDimensions(dimensions);
  const source = boundedPoint(point, map);
  if (!map || !source) return null;
  const maxX = map.width - 1;
  const maxY = map.height - 1;
  if (transform === "horizontal") return { x: source.x, y: maxY - source.y };
  if (transform === "vertical") return { x: maxX - source.x, y: source.y };
  if (transform === "rotate90") return map.width === map.height ? { x: maxX - source.y, y: source.x } : null;
  if (transform === "rotate180") return { x: maxX - source.x, y: maxY - source.y };
  if (transform === "rotate270") return map.width === map.height ? { x: source.y, y: maxY - source.x } : null;
  if (transform === "diagonalMain") return map.width === map.height ? { x: source.y, y: source.x } : null;
  if (transform === "diagonalAnti") return map.width === map.height ? { x: maxX - source.y, y: maxY - source.x } : null;
  if (transform === "rotate120" || transform === "rotate240") {
    if (map.width !== map.height) return null;
    const centreX = maxX / 2;
    const centreY = maxY / 2;
    const sine = transform === "rotate120" ? SIN_120 : -SIN_120;
    return boundedPoint({
      x: centreX + (source.x - centreX) * -0.5 - (source.y - centreY) * sine,
      y: centreY + (source.x - centreX) * sine + (source.y - centreY) * -0.5,
    }, map);
  }
  return source;
}

export function expandSymmetricPoints(dimensions, points, symmetry = MAP_AUTHORING_SYMMETRY.NONE, {
  decorate = (point) => point,
} = {}) {
  const map = normalizeDimensions(dimensions);
  if (!map || !Array.isArray(points)) return [];
  const expanded = [];
  const seen = new Set();
  for (const point of points) {
    const source = boundedPoint(point, map);
    if (!source) continue;
    for (const transform of symmetryTransforms(map, symmetry)) {
      const transformed = transformPoint(source, map, transform);
      if (!transformed) continue;
      const key = `${transformed.x},${transformed.y}`;
      if (seen.has(key)) continue;
      seen.add(key);
      expanded.push(decorate({ ...point, ...transformed }, transform));
    }
  }
  return expanded;
}

function normalizeSymmetry(value) {
  return Object.values(MAP_AUTHORING_SYMMETRY).includes(value)
    ? value
    : MAP_AUTHORING_SYMMETRY.NONE;
}
