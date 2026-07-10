import assert from "node:assert/strict";

import { TERRAIN } from "../../client/src/protocol.js";
import { Fog } from "../../client/src/fog.js";
import { GameState } from "../../client/src/state.js";
import { LabMapEditorPanel } from "../../client/src/lab_map_editor_panel.js";
import {
  LAB_MAP_HISTORY_LIMIT,
  LAB_MAP_MAX_NATURALS_PER_PLAYER,
  LabMapEditorSession,
  addDraftPlayerNatural,
  moveDraftPlayerNatural,
  moveDraftPlayerStart,
  paintDraftRect,
  protectDraftBaseTerrain,
} from "../../client/src/lab_map_editor_session.js";
import { findFakes, withFakeDocument } from "./fakes.mjs";
import { textWithin } from "./dom_text.mjs";

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
  assert.equal(session.hasUnappliedChanges, false, "the initial Lab map is the current test baseline");

  session.mutate("paint", (draft) => {
    paintDraftRect(draft, { x0: 0, y0: 0, x1: 0, y1: 0 }, TERRAIN.WATER);
    protectDraftBaseTerrain(draft);
  });
  assert.equal(session.hasUnappliedChanges, true, "draft edits do not silently change the current test");
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
  session.markCurrentDraftAsTested();
  assert.equal(session.hasUnappliedChanges, false, "only an explicit test restart accepts the draft baseline");
}

{
  const session = new LabMapEditorSession({ storage: null });
  session.initializeFromStart(startPayload());
  session.mutate("move Player 1", (draft) => {
    const result = moveDraftPlayerStart(draft, 0, { x: 12, y: 11 });
    assert.equal(result.ok, true);
    protectDraftBaseTerrain(draft);
  });
  session.mutate("add Player 1 natural", (draft) => {
    const result = addDraftPlayerNatural(draft, 0, { x: 16, y: 16 });
    assert.equal(result.ok, true);
    protectDraftBaseTerrain(draft);
  });
  const firstNatural = session.playerSlots()[0].naturals[0];
  session.mutate("move Player 1 natural", (draft) => {
    const result = moveDraftPlayerNatural(draft, 0, firstNatural.id, { x: 17, y: 16 });
    assert.equal(result.ok, true);
  });
  const playerOne = session.playerSlots()[0];
  assert.deepEqual(playerOne.start, { id: "main-1", kind: "main", x: 12, y: 11 });
  assert.deepEqual(playerOne.naturals, [{ id: firstNatural.id, kind: "natural", x: 17, y: 16 }]);
  assert.deepEqual(session.mapOverlay(), {
    players: [
      { playerIndex: 0, start: { x: 12, y: 11 }, naturals: [{ x: 17, y: 16 }] },
      { playerIndex: 1, start: { x: 23, y: 23 }, naturals: [] },
    ],
  });
  const overlap = addDraftPlayerNatural(session.draft, 0, { x: 12, y: 11 });
  assert.equal(overlap.ok, false, "a player cannot accidentally place a natural on a start");
  assert.equal(session.playerSlots()[0].naturals.length <= LAB_MAP_MAX_NATURALS_PER_PLAYER, true);
}

await withFakeDocument(async () => {
  const root = document.createElement("div");
  const session = new LabMapEditorSession({ storage: null });
  const armed = [];
  const overlays = [];
  const terrainPreviews = [];
  const appliedDrafts = [];
  const panel = new LabMapEditorPanel({
    root,
    session,
    labClient: {
      exportScenario: async () => ({ ok: false }),
      applyMapDraft: async (draft) => {
        appliedDrafts.push(draft);
        return { ok: true, outcome: { battleReset: true } };
      },
    },
    match: {
      armLabTool(tool, callbacks) {
        armed.push({ tool, callbacks });
        return tool;
      },
    },
    startPayload: startPayload(),
    applyLabMapReset: () => true,
    setLabMapDraftOverlay: (overlay) => {
      overlays.push(overlay);
      return overlay;
    },
    setLabMapDraftTerrainPreview: (draft) => {
      terrainPreviews.push(draft);
      return draft;
    },
  });
  const terrainGroup = findFakes(panel.el, (el) => (
    el.tagName === "FIELDSET" && textWithin(el).includes("Terrain paint")
  ))[0];
  const terrainButtons = findFakes(terrainGroup, (el) => el.tagName === "BUTTON" && !!el.dataset.terrain);
  const terrainIcons = findFakes(terrainGroup, (el) => String(el.className).includes("lab-terrain-icon"));
  assert.equal(terrainButtons.length, 3);
  assert.deepEqual(terrainIcons.map((icon) => icon.dataset.terrain).sort(), ["grass", "stone", "water"]);
  assert.equal(
    findFakes(terrainGroup, (el) => el.tagName === "SELECT").length,
    0,
    "terrain paint does not render a brush-size selector",
  );

  const water = terrainButtons.find((button) => button.dataset.terrain === "water");
  water.listeners.click();
  assert.equal(armed.at(-1).tool.kind, "editMapTerrain");
  assert.equal(armed.at(-1).tool.payload.terrain, TERRAIN.WATER);
  assert.equal(armed.at(-1).tool.paintOnDrag, true);
  assert.equal(armed.at(-1).callbacks.onBoxSelection, undefined);

  panel.paintWorldClick({ x: 4, y: 4, tool: { payload: { terrain: TERRAIN.WATER } } });
  assert.equal(session.materialized().terrain[0], TERRAIN.WATER, "terrain paint changes exactly the clicked tile");
  assert.equal(appliedDrafts.length, 0, "painting changes only the draft, not the current Lab test");
  assert.equal(session.hasUnappliedChanges, true);
  assert.equal(
    terrainPreviews.at(-1)?.terrain?.[0],
    TERRAIN.WATER,
    "painting redraws a local draft terrain preview without mutating the Lab test",
  );
  assert(overlays.some((overlay) => overlay?.players?.length === 2), "the authored player markers are published to the map overlay");

  const playerSetup = findFakes(panel.el, (el) => (
    el.tagName === "FIELDSET" && textWithin(el).includes("Player starts and natural bases")
  ))[0];
  assert(playerSetup, "the editor presents player-owned starts and naturals rather than raw site slots");
  assert.equal(
    findFakes(playerSetup, (el) => el.tagName === "SELECT").length,
    0,
    "base setup does not ask authors to connect anonymous site ids through select boxes",
  );
  const moveStart = findFakes(playerSetup, (el) => el.tagName === "BUTTON" && el.textContent === "Move Player 1 start")[0];
  moveStart.listeners.click();
  assert.equal(armed.at(-1).tool.kind, "editMapPlayerStart");
  armed.at(-1).callbacks.onWorldClick({ x: 12 * 32, y: 11 * 32, tool: armed.at(-1).tool });
  assert.deepEqual(session.playerSlots()[0].start, { id: "main-1", kind: "main", x: 12, y: 11 });
  assert.equal(appliedDrafts.length, 0, "moving a start remains draft-only until the explicit test action");

  await panel.restartTestWithDraft();
  assert.equal(appliedDrafts.length, 1, "Restart test with this draft is the only map-to-test transition");
  assert.equal(session.hasUnappliedChanges, false);
  panel.destroy();
  assert.equal(terrainPreviews.at(-1), null, "closing the editor restores authoritative terrain rendering");
});

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
    const result = addDraftPlayerNatural(draft, 0, { x: 16, y: 16 });
    assert.equal(result.ok, true);
    protectDraftBaseTerrain(draft);
  });
  const materialized = session.materialized();
  assert.deepEqual(materialized.expansionSites, [{ x: 16, y: 16 }]);
  assert.equal(session.exportMap().version, 2);
}

console.log("lab map editor contracts passed");
