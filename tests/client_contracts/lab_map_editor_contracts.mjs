import assert from "node:assert/strict";

import { TERRAIN } from "../../client/src/protocol.js";
import { Fog } from "../../client/src/fog.js";
import { GameState } from "../../client/src/state.js";
import {
  LAB_MAP_HISTORY_LIMIT,
  LabMapEditorSession,
  paintDraftRect,
  placeDraftSite,
  protectDraftBaseTerrain,
} from "../../client/src/lab_map_editor_session.js";

function startPayload() {
  const size = 32;
  return {
    map: {
      width: size,
      height: size,
      tileSize: 32,
      terrain: Array(size * size).fill(TERRAIN.GRASS),
    },
    players: [
      { id: 1, startTileX: 8, startTileY: 8 },
      { id: 2, startTileX: 23, startTileY: 23 },
    ],
  };
}

{
  const session = new LabMapEditorSession({ storage: null });
  assert.equal(session.initializeFromStart(startPayload(), { name: "POC" }), true);
  assert.equal(session.materialized().starts.length, 2);

  session.mutate("paint", (draft) => {
    paintDraftRect(draft, { x0: 0, y0: 0, x1: 0, y1: 0 }, TERRAIN.WATER);
    protectDraftBaseTerrain(draft);
  });
  assert.equal(session.materialized().terrain[0], TERRAIN.WATER);
  assert.equal(session.undo(), true);
  assert.equal(session.materialized().terrain[0], TERRAIN.GRASS);
  assert.equal(session.redo(), true);
  assert.equal(session.materialized().terrain[0], TERRAIN.WATER);

  session.mutate("near base", (draft) => {
    paintDraftRect(draft, { x0: 10, y0: 8, x1: 10, y1: 8 }, TERRAIN.ROCK);
    protectDraftBaseTerrain(draft);
  });
  assert.equal(session.materialized().terrain[8 * 32 + 10], TERRAIN.GRASS);
  session.mutate("beyond starting units", (draft) => {
    paintDraftRect(draft, { x0: 12, y0: 8, x1: 12, y1: 8 }, TERRAIN.ROCK);
    protectDraftBaseTerrain(draft);
  });
  assert.equal(session.materialized().terrain[8 * 32 + 12], TERRAIN.ROCK);
  const exported = session.exportMap();
  assert.equal(exported.terrain[8][12], "#");
  assert.equal(exported.name, session.draft.name);
}

{
  const initial = startPayload();
  const state = new GameState({
    playerId: 1,
    spectator: true,
    map: initial.map,
    players: initial.players,
    diagnostics: {},
  });
  state.selection.add(12);
  state.controlGroups[0] = [12];
  const nextMap = {
    ...initial.map,
    terrain: Array(initial.map.terrain.length).fill(TERRAIN.WATER),
    resources: [{ id: 50, kind: "oil", x: 320, y: 320 }],
  };
  assert.equal(state.resetForLabMap({
    map: nextMap,
    players: [
      { id: 1, startTileX: 10, startTileY: 10 },
      { id: 2, startTileX: 21, startTileY: 21 },
    ],
    tick: 0,
  }), true);
  assert.equal(state.map.terrain[0], TERRAIN.WATER);
  assert.equal(state.resourceById.get(50).kind, "oil");
  assert.equal(state.players[0].startTileX, 10);
  assert.equal(state.selection.size, 0);
  assert.deepEqual(state.controlGroups[0], []);

  const fog = new Fog(32, 32, initial.map.terrain);
  fog.visibleGrid[0] = 1;
  fog.resetMap(32, 32, nextMap.terrain);
  assert.equal(fog.visibleGrid[0], 0);
  assert.equal(fog.terrain, nextMap.terrain);

  state.selection.add(77);
  const terrainOnlyMap = { ...nextMap, terrain: nextMap.terrain.slice() };
  terrainOnlyMap.terrain[5] = TERRAIN.ROCK;
  assert.equal(state.updateForLabMap({
    map: terrainOnlyMap,
    players: state.players,
    tick: 12,
  }), true);
  assert.equal(state.selection.has(77), true);
  fog.visibleGrid[0] = 1;
  fog.updateTerrain(terrainOnlyMap.terrain);
  assert.equal(fog.visibleGrid[0], 1);
  assert.equal(fog.terrain[5], TERRAIN.ROCK);
}

{
  const session = new LabMapEditorSession({ storage: null });
  session.initializeFromStart(startPayload());
  for (let index = 0; index < LAB_MAP_HISTORY_LIMIT + 5; index++) {
    session.mutate(`rename ${index}`, (draft) => { draft.name = `Map ${index}`; });
  }
  assert.equal(session.undoStack.length, LAB_MAP_HISTORY_LIMIT);
}

{
  const session = new LabMapEditorSession({ storage: null });
  session.initializeFromStart(startPayload());
  session.mutate("natural", (draft) => {
    const id = placeDraftSite(draft, { kind: "natural", x: 16, y: 16 });
    draft.layouts[0].slots[0].naturals = [id];
    protectDraftBaseTerrain(draft);
  });
  const materialized = session.materialized();
  assert.deepEqual(materialized.expansionSites, [{ x: 16, y: 16 }]);
  assert.equal(session.exportMap().version, 2);
}

console.log("lab map editor contracts passed");
