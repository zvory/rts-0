import assert from "node:assert/strict";

export function assertAtlasSpriteUsesWorldScale(definition, atlas, spriteId) {
  const worldScale = atlas.grid?.normalization?.worldScale;
  assert.equal(typeof worldScale, "number");
  const sprite = atlas.sprites.find((candidate) => candidate.id === spriteId);
  assert.ok(sprite, `${spriteId} should exist`);
  const visibleBounds = sprite.frame?.visibleBounds;
  assert.ok(visibleBounds, `${spriteId} should have normalized visible bounds`);
  const sourceBounds = unionPartBounds(definition, sprite.sourceParts);
  const expectedPixelsPerUnitX = (visibleBounds.w / Math.max(1, sourceBounds.maxX - sourceBounds.minX)) / worldScale;
  const expectedPixelsPerUnitY = (visibleBounds.h / Math.max(1, sourceBounds.maxY - sourceBounds.minY)) / worldScale;
  assertAlmostEqual(sprite.frame.pixelsPerUnitX, expectedPixelsPerUnitX, `${spriteId} pixelsPerUnitX`);
  assertAlmostEqual(sprite.frame.pixelsPerUnitY, expectedPixelsPerUnitY, `${spriteId} pixelsPerUnitY`);
}

function unionPartBounds(definition, partIds) {
  const bounds = partIds
    .map((partId) => definition.parts.find((part) => part.id === partId))
    .filter(Boolean)
    .map(partBounds);
  assert.ok(bounds.length > 0, "sprite should reference at least one source part");
  return {
    minX: Math.min(...bounds.map((bound) => bound.minX)),
    minY: Math.min(...bounds.map((bound) => bound.minY)),
    maxX: Math.max(...bounds.map((bound) => bound.maxX)),
    maxY: Math.max(...bounds.map((bound) => bound.maxY)),
  };
}

function partBounds(part) {
  const geometry = part?.geometry || {};
  const points = [];
  if (geometry.type === "rect") {
    points.push([geometry.x, geometry.y], [geometry.x + geometry.width, geometry.y + geometry.height]);
  } else if (geometry.type === "line") {
    points.push([geometry.from.x, geometry.from.y], [geometry.to.x, geometry.to.y]);
  } else if (geometry.type === "polygon" || geometry.type === "polyline") {
    for (const point of geometry.points || []) points.push([point.x, point.y]);
  } else if (geometry.type === "circle") {
    points.push([geometry.cx - geometry.r, geometry.cy - geometry.r], [geometry.cx + geometry.r, geometry.cy + geometry.r]);
  } else if (geometry.type === "ellipse") {
    points.push([geometry.cx - geometry.rx, geometry.cy - geometry.ry], [geometry.cx + geometry.rx, geometry.cy + geometry.ry]);
  }
  assert.ok(points.length > 0, `${part?.id || "part"} should have measurable geometry`);
  const strokePad = Math.max(part?.paint?.strokeWidth || 0, geometry.strokeWidth || 0, 1) * 0.5 + 0.5;
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  return {
    minX: Math.min(...xs) - strokePad,
    minY: Math.min(...ys) - strokePad,
    maxX: Math.max(...xs) + strokePad,
    maxY: Math.max(...ys) + strokePad,
  };
}

function assertAlmostEqual(actual, expected, label, epsilon = 0.000001) {
  assert.ok(
    Math.abs(actual - expected) <= epsilon,
    `${label}: expected ${expected}, got ${actual}`,
  );
}
