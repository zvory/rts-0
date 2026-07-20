import { isUnit } from "./protocol.js";

export function markActionableFiringReveal(entity, map, visibleTiles, playerId) {
  if (!entity || entity.aboveFogReveal || entity.shotReveal || entity.visionOnly || !isUnit(entity.kind)) {
    return entity;
  }
  if (Number(entity.owner) === 0 || Number(entity.owner) === Number(playerId)) return entity;
  const width = Number(map?.width);
  const height = Number(map?.height);
  const tileSize = Number(map?.tileSize);
  if (!Number.isInteger(width) || !Number.isInteger(height) || !(tileSize > 0)) return entity;
  if (visibleTiles.length !== width * height) return entity;
  const tx = Math.floor(Number(entity.x) / tileSize);
  const ty = Math.floor(Number(entity.y) / tileSize);
  if (tx < 0 || ty < 0 || tx >= width || ty >= height) return entity;
  if (visibleTiles[ty * width + tx]) return entity;

  // `shotReveal` belongs to event-backed visual ghosts and intentionally excludes them from
  // interaction. Snapshot-backed reveals remain authoritative entities that merely render above
  // the presentation fog.
  return { ...entity, aboveFogReveal: true };
}
