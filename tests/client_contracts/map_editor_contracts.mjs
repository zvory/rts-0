import assert from "node:assert/strict";
import fs from "node:fs";

import { TERRAIN } from "../../client/src/protocol.js";
import { createMapHandoff, consumeMapHandoff } from "../../client/src/map_editor_handoff.js";
import { mapEditorLaunchConfig } from "../../client/src/map_editor_launch.js";
import {
  addDraftPlayerNatural,
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_NATURAL_CLEARANCE_TILES,
  MapEditorSession,
  authoredMapFromMaterialized,
  materializedMapsEqual,
  moveDraftPlayerNatural,
  moveDraftPlayerStart,
  removeDraftPlayerNatural,
} from "../../client/src/map_editor_session.js";

const repoRoot = new URL("../../", import.meta.url);
const noTerrainMap = JSON.parse(fs.readFileSync(new URL("server/assets/maps/no-terrain.json", repoRoot), "utf8"));
const serverMapSource = fs.readFileSync(new URL("server/crates/sim/src/game/map.rs", repoRoot), "utf8");

{
  const serverMainRadius = Number(serverMapSource.match(/BASE_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  const serverNaturalRadius = Number(serverMapSource.match(/EXPANSION_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  assert.equal(MAP_EDITOR_MAIN_CLEARANCE_TILES, serverMainRadius, "editor start clearance mirrors authored-map validation");
  assert.equal(MAP_EDITOR_NATURAL_CLEARANCE_TILES, serverNaturalRadius, "editor natural clearance mirrors authored-map validation");
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(noTerrainMap);
  session.selectLayout("2p_cross_nw_se");
  const materialized = session.materialized();
  assert.equal(materialized.size, 126);
  assert.equal(materialized.starts.length, 2);
  assert.equal(materialized.expansionSites.length, 2, "legacy singular natural entries normalize into the active layout");
  assert.equal(session.exportMap().layouts[0].slots[0].natural, undefined, "exports use the canonical naturals array");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const layoutId = session.selectedLayoutId;
  session.mutate("Added crossed naturals", (draft) => {
    addDraftPlayerNatural(draft, 0, { x: 20, y: 20 }, layoutId);
    addDraftPlayerNatural(draft, 1, { x: 12, y: 12 }, layoutId);
  });
  const local = session.materialized();
  const reconstructed = new MapEditorSession({ storage: null });
  reconstructed.loadAuthoredMap(authoredMapFromMaterialized({
    ...local,
    description: "Returned from Lab",
  }));
  const returned = reconstructed.materialized();
  assert.notDeepEqual(
    local.expansionSites,
    returned.expansionSites,
    "map-only Lab reconstruction may group natural sites differently",
  );
  assert.equal(
    materializedMapsEqual(local, returned),
    true,
    "global natural-site ordering does not discard richer local layout metadata",
  );
  returned.terrain[0] = TERRAIN.WATER;
  assert.equal(materializedMapsEqual(local, returned), false);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  let notifications = 0;
  const unsubscribe = session.subscribe((snapshot) => {
    if (snapshot.reason) notifications += 1;
  });
  const beforeUndo = session.undoStack.length;
  session.beginTerrainStroke();
  const first = session.paintTerrainTiles([{ x: 0, y: 0 }], TERRAIN.WATER);
  const second = session.paintTerrainTiles([{ x: 1, y: 0 }, { x: 2, y: 0 }], TERRAIN.ROCK);
  assert.equal(notifications, 0, "painting does not serialize/notify the whole map per tile");
  assert.equal(first.length, 1);
  assert.equal(second.length, 2, "each paint step returns only newly dirty tiles");
  assert.equal(session.commitTerrainStroke(), true);
  assert.equal(notifications, 1, "one pointer stroke publishes one state transaction");
  assert.equal(session.undoStack.length, beforeUndo + 1, "one pointer stroke creates one undo state");
  session.undo();
  assert.equal(session.materialized().terrain[0], TERRAIN.GRASS);
  unsubscribe();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const start = session.playerSlots()[0].start;
  session.beginTerrainStroke();
  assert.deepEqual(
    session.paintTerrainTiles([
      { x: start.x + MAP_EDITOR_MAIN_CLEARANCE_TILES, y: start.y },
    ], TERRAIN.WATER),
    [],
    "the full authored start clearance remains protected grass",
  );
  assert.equal(session.commitTerrainStroke(), false);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const layoutId = session.selectedLayoutId;
  session.mutate("Added natural", (draft) => {
    addDraftPlayerNatural(draft, 0, { x: 16, y: 16 }, layoutId);
  });
  const natural = session.playerSlots()[0].naturals[0];
  session.beginTerrainStroke();
  assert.deepEqual(
    session.paintTerrainTiles([
      { x: natural.x + MAP_EDITOR_NATURAL_CLEARANCE_TILES, y: natural.y },
    ], TERRAIN.ROCK),
    [],
    "the full authored natural clearance remains protected grass",
  );
  assert.equal(session.commitTerrainStroke(), false);

  session.mutate("Moved sites from edge", (draft) => {
    moveDraftPlayerStart(draft, 0, { x: 0, y: 0 }, layoutId);
    moveDraftPlayerNatural(draft, 0, natural.id, { x: 31, y: 31 }, layoutId);
  });
  assert.deepEqual(session.playerSlots()[0].start, {
    id: session.playerSlots()[0].start.id,
    kind: "main",
    x: MAP_EDITOR_MAIN_CLEARANCE_TILES,
    y: MAP_EDITOR_MAIN_CLEARANCE_TILES,
  });
  assert.equal(session.playerSlots()[0].naturals[0].x, 31 - MAP_EDITOR_NATURAL_CLEARANCE_TILES);
  assert.equal(session.playerSlots()[0].naturals[0].y, 31 - MAP_EDITOR_NATURAL_CLEARANCE_TILES);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const originalLayoutId = session.selectedLayoutId;
  let addedNatural = null;
  session.mutate("Added shared natural", (draft) => {
    addedNatural = addDraftPlayerNatural(draft, 0, { x: 12, y: 8 }, originalLayoutId);
  });
  assert.equal(addedNatural.ok, true);
  const sharedMainId = session.activeLayout.slots[0].main;
  const sharedNaturalId = addedNatural.id;
  session.addLayout(2);
  const editedLayoutId = session.selectedLayoutId;

  session.mutate("Moved start in one layout", (draft) => {
    moveDraftPlayerStart(draft, 0, { x: 10, y: 10 }, editedLayoutId);
  });
  session.mutate("Moved natural in one layout", (draft) => {
    moveDraftPlayerNatural(draft, 0, sharedNaturalId, { x: 14, y: 10 }, editedLayoutId);
  });
  session.mutate("Removed natural in one layout", (draft) => {
    removeDraftPlayerNatural(draft, 0, session.activeLayout.slots[0].naturals[0], editedLayoutId);
  });

  const original = session.draft.layouts.find((layout) => layout.id === originalLayoutId);
  const edited = session.draft.layouts.find((layout) => layout.id === editedLayoutId);
  const editedMainId = edited.slots[0].main;
  assert.equal(original.slots[0].main, sharedMainId, "moving a start detaches it from other layouts");
  assert.equal(original.slots[0].naturals[0], sharedNaturalId, "moving/removing a natural preserves other layouts");
  assert.notEqual(edited.slots[0].main, sharedMainId);
  assert.deepEqual(edited.slots[0].naturals, []);
  assert(session.draft.sites.some((site) => site.id === sharedNaturalId), "shared natural site remains authored");
  session.removeSelectedLayout();
  assert(
    !session.draft.sites.some((site) => site.id === editedMainId),
    "removing a layout also removes its unreferenced protected sites",
  );
  session.undo();
  assert(session.draft.sites.some((site) => site.id === editedMainId), "layout removal remains fully undoable");
}

{
  const authored = authoredMapFromMaterialized({
    name: "Returned Lab map",
    description: "",
    size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }, { x: 23, y: 23 }],
    expansionSites: [{ x: 12, y: 8 }, { x: 19, y: 23 }],
  });
  assert.equal(authored.layouts[0].slots.length, 2);
  assert.deepEqual(authored.layouts[0].slots.map((slot) => slot.naturals.length), [1, 1]);
}

{
  const values = new Map();
  const storage = {
    setItem: (key, value) => values.set(key, value),
    getItem: (key) => values.get(key) || null,
  };
  const session = new MapEditorSession({ storage });
  session.loadAuthoredMap(noTerrainMap);
  session.selectLayout("2p_cross_nw_se");
  assert.equal(session.saveLocal("roundtrip"), true);
  const restored = new MapEditorSession({ storage });
  assert.equal(restored.loadLocal("roundtrip"), true);
  assert.equal(restored.selectedLayoutId, "2p_cross_nw_se");
  assert.equal(restored.draft.layouts.length, noTerrainMap.layouts.length);
  assert.equal(restored.loadLocal("roundtrip"), true, "an unchanged saved workspace still loads successfully");
}

{
  const unavailableStorage = {
    getItem() { throw new Error("storage disabled"); },
    setItem() { throw new Error("storage disabled"); },
  };
  const session = new MapEditorSession({ storage: unavailableStorage });
  session.initializeBlank({ size: 32, playerCount: 2 });
  assert.equal(session.saveLocal("disabled"), false, "disabled local storage does not abort the editor");
  assert.equal(session.loadLocal("disabled"), false, "disabled local storage reports an unavailable workspace");
}

{
  let posted = null;
  const fetchImpl = async (url, options = {}) => {
    if (url === "/api/map-handoffs" && options.method === "POST") {
      posted = { url, body: JSON.parse(options.body) };
      return { ok: true, status: 200, json: async () => ({ handoffId: "a".repeat(32), expiresInMs: 120000 }) };
    }
    assert.equal(options.method, "POST", "one-use handoffs are consumed with non-prefetchable POST");
    return { ok: true, status: 200, json: async () => ({ destination: "editor", authoredMap: noTerrainMap }) };
  };
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(noTerrainMap);
  const created = await createMapHandoff({
    destination: "editor",
    authoredMap: session.exportMap(),
    materializedMap: session.materialized(),
    selectedLayoutId: session.selectedLayoutId,
    fetchImpl,
  });
  assert.equal(created.handoffId.length, 32);
  assert.equal(posted.url, "/api/map-handoffs");
  assert.equal(posted.body.destination, "editor");
  assert(!posted.url.includes("terrain"), "full map JSON never enters a transition URL");
  assert.equal((await consumeMapHandoff("b".repeat(32), { fetchImpl })).destination, "editor");
}

{
  assert.deepEqual(
    mapEditorLaunchConfig({ pathname: "/map-editor", search: `?handoff=${"c".repeat(32)}&workspace=roundtrip` }),
    { handoffId: "c".repeat(32), workspaceId: "roundtrip", error: "" },
  );
  assert.equal(mapEditorLaunchConfig({ pathname: "/", search: "" }), null);
}

{
  const main = fs.readFileSync(new URL("client/src/main.js", repoRoot), "utf8");
  const rendererTerrain = fs.readFileSync(new URL("client/src/renderer/terrain.js", repoRoot), "utf8");
  const serverMain = fs.readFileSync(new URL("server/src/main.rs", repoRoot), "utf8");
  assert.match(main, /mapEditorLaunchConfig\(\) \? new MapEditorApp\(\) : new App\(\)/);
  const editorApp = fs.readFileSync(new URL("client/src/map_editor_app.js", repoRoot), "utf8");
  assert.match(editorApp, /simulation:\s*false/);
  assert.match(editorApp, /gameplayCommands:\s*false/);
  assert.match(rendererTerrain, /updateStaticTerrainTiles/);
  assert.match(rendererTerrain, /baseTexture\.update\(\)/, "dirty painting updates one persistent texture");
  assert.match(serverMain, /\/map-editor/);
  assert.equal(fs.existsSync(new URL("client/map-editor.html", repoRoot)), false, "legacy standalone editor is retired");
}
