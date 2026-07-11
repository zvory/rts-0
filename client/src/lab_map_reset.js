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

/** Keep the renderer and minimap on the same browser-local Lab map draft preview. */
export function previewLabMapDraftTerrain(match, draft) {
  const renderer = match?.renderer;
  const minimap = match?.minimap;
  const liveMap = match?.state?.map;
  if (!liveMap) return false;

  if (!draft) {
    let applied = false;
    if (typeof renderer?.buildStaticMap === "function") {
      try {
        renderer.buildStaticMap(liveMap);
        applied = true;
      } catch {}
    }
    if (typeof minimap?.setMapPreview === "function") {
      try {
        minimap.setMapPreview(null);
        applied = true;
      } catch {}
    }
    return applied;
  }

  const previewMap = {
    ...liveMap,
    width: draft.size,
    height: draft.size,
    terrain: draft.terrain,
    // Resource patches are materialized authoritatively only when the test restarts.
    // Do not leave the running test's old patches on the draft minimap preview.
    resources: [],
  };
  let applied = false;
  if (typeof renderer?.previewStaticTerrain === "function") {
    try {
      renderer.previewStaticTerrain(previewMap);
      applied = true;
    } catch {}
  }
  if (typeof minimap?.setMapPreview === "function") {
    try {
      minimap.setMapPreview(previewMap);
      applied = true;
    } catch {}
  }
  return applied;
}
