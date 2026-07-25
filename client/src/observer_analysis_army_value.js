import { STATS } from "./config.js";
import { isUnit, KIND } from "./protocol.js";

export function calculateViewportArmyValue({
  entities = [],
  cameraBounds = null,
  players = [],
  stats = STATS,
} = {}) {
  const rowsByOwner = new Map();
  for (const player of players || []) {
    const id = Number(player?.id);
    if (!Number.isFinite(id) || id === 0) continue;
    rowsByOwner.set(id, {
      owner: id,
      name: player?.name || `Player ${id}`,
      color: player?.color || "#e7dfc5",
      steel: 0,
      oil: 0,
    });
  }

  if (!cameraBounds || !Array.isArray(entities)) return [...rowsByOwner.values()];
  const bounds = normalizeBounds(cameraBounds);
  if (!bounds) return [...rowsByOwner.values()];

  for (const entity of entities) {
    if (
      !entity
      || entity.shotReveal
      || !isUnit(entity.kind)
      || entity.kind === KIND.WORKER
      || entity.kind === KIND.GOLEM
    ) continue;
    const owner = Number(entity.owner);
    if (!Number.isFinite(owner) || owner === 0) continue;
    const x = Number(entity.x);
    const y = Number(entity.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;

    const stat = stats?.[entity.kind] || {};
    const radius = Math.max(0, Number(stat.size) || 0);
    if (!circleIntersectsBounds(x, y, radius, bounds)) continue;

    const row = rowsByOwner.get(owner) || {
      owner,
      name: `Player ${owner}`,
      color: "#e7dfc5",
      steel: 0,
      oil: 0,
    };
    const cost = stat.cost || {};
    row.steel += Math.max(0, Number(cost.steel) || 0);
    row.oil += Math.max(0, Number(cost.oil) || 0);
    rowsByOwner.set(owner, row);
  }

  return [...rowsByOwner.values()].sort((a, b) => a.owner - b.owner);
}

function normalizeBounds(bounds) {
  const left = Number(bounds.left ?? bounds.x);
  const top = Number(bounds.top ?? bounds.y);
  const width = Number(bounds.width ?? bounds.w);
  const height = Number(bounds.height ?? bounds.h);
  if (![left, top, width, height].every(Number.isFinite) || width <= 0 || height <= 0) return null;
  return {
    left,
    top,
    right: left + width,
    bottom: top + height,
  };
}

function circleIntersectsBounds(x, y, radius, bounds) {
  return (
    x + radius >= bounds.left
    && x - radius <= bounds.right
    && y + radius >= bounds.top
    && y - radius <= bounds.bottom
  );
}
