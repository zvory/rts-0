import assert from "node:assert/strict";
import fs from "node:fs";

import { TERRAIN } from "../../client/src/protocol.js";
import { Fog } from "../../client/src/fog.js";
import { GameState } from "../../client/src/state.js";
import { LabMapEditorPanel } from "../../client/src/lab_map_editor_panel.js";
import {
  LAB_MAP_HISTORY_LIMIT,
  LabMapEditorSession,
  paintDraftRect,
  placeDraftSite,
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

withFakeDocument(() => {
  const root = document.createElement("div");
  const session = new LabMapEditorSession({ storage: null });
  const armed = [];
  const panel = new LabMapEditorPanel({
    root,
    session,
    labClient: {
      exportScenario: async () => ({ ok: false }),
      applyMapDraft: async () => ({ ok: true, outcome: { battleReset: false } }),
    },
    match: {
      armLabTool(tool, callbacks) {
        armed.push({ tool, callbacks });
        return tool;
      },
    },
    startPayload: startPayload(),
    applyLabMapReset: () => true,
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
  panel.destroy();
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
  assert.deepEqual(applied[0].starts, [{ x: 8, y: 8 }, { x: 23, y: 23 }]);
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
    const id = placeDraftSite(draft, { kind: "natural", x: 16, y: 16 });
    draft.layouts[0].slots[0].naturals = [id];
    protectDraftBaseTerrain(draft);
  });
  const materialized = session.materialized();
  assert.deepEqual(materialized.expansionSites, [{ x: 16, y: 16 }]);
  assert.equal(session.exportMap().version, 2);
}

console.log("lab map editor contracts passed");
