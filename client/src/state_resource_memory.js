export function clearObservedEmptyResources(map, deltas, visibleTiles) {
  // Live nodes on visible tiles always have a delta. Depleted nodes have already
  // been removed server-side, so a missed death event is repaired by visible absence.
  const { width, height, tileSize } = map;
  if (visibleTiles.length !== width * height) return;
  const liveIds = new Set(deltas.map((delta) => delta?.id));
  for (const node of map.resources) {
    if (node.remaining === 0 || liveIds.has(node.id)) continue;
    const x = Math.floor(node.x / tileSize);
    const y = Math.floor(node.y / tileSize);
    if (x >= 0 && x < width && y >= 0 && y < height && visibleTiles[y * width + x]) {
      node.remaining = 0;
    }
  }
}
