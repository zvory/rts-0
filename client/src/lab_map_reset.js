export function applyLabMapReset(match, outcome) {
  const map = outcome?.map;
  const players = outcome?.players;
  const battleReset = outcome?.battleReset === true;
  const applied = battleReset
    ? match?.state?.resetForLabMap?.({ map, players, tick: outcome?.tick })
    : match?.state?.updateForLabMap?.({ map, players, tick: outcome?.tick });
  if (!match?.labMetadata || !applied) {
    return false;
  }
  if (battleReset) {
    match.clientIntent?.clearPlannedOrders?.();
    match.fog?.resetMap?.(map.width, map.height, match.state.map.terrain);
  } else {
    match.fog?.updateTerrain?.(match.state.map.terrain);
  }
  match.renderer?.buildStaticMap?.(match.state.map);
  match.applyBounds?.();
  match.lastSnapshotTick = Number.isFinite(outcome?.tick) ? outcome.tick : match.lastSnapshotTick;
  match.roomTimeControls?.noteSnapshotTick?.(match.lastSnapshotTick);
  return true;
}
