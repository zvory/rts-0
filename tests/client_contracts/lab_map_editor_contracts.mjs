import assert from "node:assert/strict";
import fs from "node:fs";

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

function authoredMap(size = 32) {
  return {
    version: 2,
    name: "No Terrain",
    description: "All grass for editor loading tests.",
    _design: "Test fixture.",
    terrain: Array.from({ length: size }, () => ".".repeat(size)),
    sites: [
      { id: "main_nw", kind: "main", x: 8, y: 8 },
      { id: "natural_nw", kind: "natural", x: 12, y: 8 },
      { id: "main_se", kind: "main", x: 23, y: 23 },
      { id: "natural_se", kind: "natural", x: 19, y: 23 },
    ],
    layouts: [
      { id: "solo", playerCount: 1, slots: [{ main: "main_nw", naturals: ["natural_nw"] }] },
      {
        id: "duel",
        playerCount: 2,
        slots: [
          { main: "main_nw", naturals: ["natural_nw"] },
          { main: "main_se", naturals: ["natural_se"] },
        ],
      },
    ],
  };
}

const noTerrainMap = JSON.parse(
  fs.readFileSync(new URL("../../server/assets/maps/no-terrain.json", import.meta.url), "utf8"),
);

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

{
  const session = new LabMapEditorSession({ storage: null });
  session.initializeFromStart(startPayload());
  assert.equal(session.loadAuthoredMap(authoredMap(), { expectedSize: 32, playerCount: 2 }), true);
  assert.equal(session.selectedLayoutId, "duel", "loaded maps select a layout compatible with the live lab");
  assert.equal(session.activeLayout.id, "duel");
  assert.deepEqual(session.materialized().starts, [{ x: 8, y: 8 }, { x: 23, y: 23 }]);
  assert.equal(session.exportMap().layouts.length, 2, "loading preserves every authored layout for export");
  assert.throws(
    () => session.loadAuthoredMap(authoredMap(33), { expectedSize: 32, playerCount: 2 }),
    /This lab uses a 32 × 32 map/,
    "loading rejects a map whose size cannot be applied to the current lab",
  );
}

{
  const session = new LabMapEditorSession({ storage: null });
  assert.equal(session.loadAuthoredMap(noTerrainMap, { expectedSize: 126, playerCount: 2 }), true);
  assert.equal(session.activeLayout.slots.length, 2, "the real No Terrain map selects a two-player layout");
  assert.equal(
    session.exportMap().layouts.length,
    noTerrainMap.layouts.length,
    "the real No Terrain map keeps every authored layout after loading",
  );
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
    fetchImpl: null,
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

await withFakeDocument(async () => {
  const root = document.createElement("div");
  const session = new LabMapEditorSession({ storage: null });
  const requests = [];
  const applied = [];
  const panel = new LabMapEditorPanel({
    root,
    session,
    labClient: {
      exportScenario: async () => ({ ok: false }),
      applyMapDraft: async (draft) => {
        applied.push(draft);
        return { ok: true, outcome: { battleReset: true } };
      },
    },
    match: { armLabTool() {} },
    startPayload: startPayload(),
    applyLabMapReset: () => true,
    fetchImpl: async (url) => {
      requests.push(url);
      if (url === "/maps/catalog") {
        return {
          ok: true,
          json: async () => ({
            maps: [{
              file: "no-terrain.json",
              name: "No Terrain",
              description: "All grass.",
            }],
          }),
        };
      }
      if (url === "/maps/no-terrain.json") {
        return { ok: true, json: async () => authoredMap() };
      }
      return { ok: false, status: 404, json: async () => ({}) };
    },
  });
  await panel.loadMapCatalog();
  const loader = findFakes(panel.el, (el) => (
    el.tagName === "FIELDSET" && textWithin(el).includes("Load map")
  ))[0];
  assert.ok(loader, "the Lab map editor exposes a built-in map loader");
  assert.equal(await panel.loadCatalogMap("no-terrain.json"), true);
  assert.deepEqual(requests, ["/maps/catalog", "/maps/no-terrain.json"]);
  assert.equal(session.draft.name, "No Terrain");
  assert.equal(session.selectedLayoutId, "duel");
  assert.equal(session.exportMap().layouts.length, 2, "catalog loads keep non-active authored layouts");
  assert.equal(applied.length, 0, "loading a built-in map changes only the draft");
  assert.equal(session.hasUnappliedChanges, true);
  await panel.restartTestWithDraft();
  assert.deepEqual(applied[0].starts, [{ x: 8, y: 8 }, { x: 23, y: 23 }]);
  assert.equal(session.hasUnappliedChanges, false);
  panel.destroy();
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
