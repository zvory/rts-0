import assert from "node:assert/strict";
import fs from "node:fs";

{
  const savedRaf = globalThis.requestAnimationFrame;
  const scheduled = [];
  globalThis.requestAnimationFrame = (callback) => {
    scheduled.push(callback);
    return scheduled.length;
  };
  try {
    const presentations = [];
    let cameraUpdates = 0;
    let paintCode = TERRAIN.WATER;
    const viewport = {
      destroyed: false,
      presentationStopped: false,
      presentationInFlight: null,
      lastFrameAt: 0,
      keys: {},
      camera: {
        x: 10,
        y: 20,
        zoom: 2,
        update() { cameraUpdates += 1; this.x += 1; },
      },
      presentationFrameId: 0,
      terrainRevision: 0,
      overlayRevision: 0,
      pendingTerrainUpdate: null,
      pendingOverlay: null,
      symmetry: MAP_EDITOR_SYMMETRY.NONE,
      selectedBaseIndex: null,
      tool: { kind: "terrain", terrain: TERRAIN.WATER },
      session: {
        draft: { terrain: ["....", "....", "....", "...."] },
        paintTerrainTiles() {
          return [
            { x: 1, y: 1, code: paintCode },
            { x: 2, y: 2, code: paintCode },
          ];
        },
        mapOverlay() { return { starts: [], bases: [] }; },
      },
      siteRecord: MapEditorViewport.prototype.siteRecord,
      paintPreviewRecord: () => null,
      onStatus(message, error) { this.status = { message, error }; },
      presentation: {
        present(record) {
          let resolve;
          const settled = new Promise((done) => { resolve = done; });
          presentations.push({ record, resolve });
          return settled;
        },
      },
      tick: MapEditorViewport.prototype.tick,
      submitPresentation: MapEditorViewport.prototype.submitPresentation,
      settlePresentation: MapEditorViewport.prototype.settlePresentation,
      stopPresentation: MapEditorViewport.prototype.stopPresentation,
    };
    MapEditorViewport.prototype.paintTiles.call(viewport, [{ x: 1, y: 1 }]);
    MapEditorViewport.prototype.drawOverlay.call(viewport);
    viewport.tick(16);
    assert.equal(presentations.length, 1, "the Map Editor submits one detached presentation record");
    assert.equal(presentations[0].record.version, 1);
    assert.equal(scheduled.length, 1, "the Map Editor keeps its camera/input RAF running during presentation");

    for (let i = 0; i < 20; i += 1) {
      paintCode = i % 2 === 0 ? TERRAIN.MUD : TERRAIN.WATER;
      MapEditorViewport.prototype.paintTiles.call(viewport, [{ x: 1, y: 1 }]);
    }
    MapEditorViewport.prototype.drawOverlay.call(viewport);
    scheduled.shift()(32);
    assert.equal(cameraUpdates, 2, "camera state advances while the worker presentation remains deferred");
    assert.equal(presentations.length, 1, "backpressure permits only one editor worker submission in flight");
    assert.equal(viewport.pendingTerrainUpdate.changes.length, 2,
      "repainting while the worker is slow remains bounded to unique changed tiles");
    presentations[0].resolve({ status: "presented", frameId: 1 });
    await Promise.resolve();
    assert.equal(viewport.pendingTerrainUpdate.revision, 21,
      "terrain painted during an in-flight frame remains pending after the older frame presents");
    assert.equal(viewport.pendingOverlay.revision, 2,
      "a newer editor overlay remains pending after the older frame presents");

    scheduled.shift()(48);
    assert.equal(presentations.length, 2);
    assert.equal(presentations[1].record.camera.x, 13,
      "the first post-acknowledgment submission carries the latest accumulated camera state");
    assert.equal(presentations[1].record.terrainUpdate.revision, 21,
      "the next acknowledged editor frame carries every accumulated terrain change");
    presentations[1].resolve({ status: "presented", frameId: 2 });
    await Promise.resolve();
    assert.equal(viewport.pendingTerrainUpdate, null, "presented terrain changes clear after exact revision acknowledgment");
    assert.equal(viewport.pendingOverlay, null, "presented overlay state clears after exact revision acknowledgment");
    assert.equal(viewport.status, undefined, "serialized editor presentation completes without a false failure status");
  } finally {
    globalThis.requestAnimationFrame = savedRaf;
  }
}
import { TERRAIN } from "../../client/src/protocol.js";
import { createMapHandoff } from "../../client/src/map_editor_handoff.js";
import { mapEditorLaunchConfig } from "../../client/src/map_editor_launch.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import {
  canonicalDoodadColor,
  createDoodadSprayStroke,
  createMapEditorDoodads,
  extendDoodadSprayStroke,
  MAP_EDITOR_DOODAD_CATALOG,
  MAP_EDITOR_DOODAD_TYPES,
  MAP_EDITOR_MAX_DOODADS,
  normalizeMapEditorDoodads,
  symmetricDoodadPlacements,
} from "../../client/src/map_editor_doodads.js";
import { createMapEditorPresentation } from "../../client/src/map_editor_presentation.js";
import {
  mapEditorSymmetryGuideCentre,
  mapEditorSymmetryGuideLines,
  MapEditorViewport,
} from "../../client/src/map_editor_viewport.js";
import {
  addSymmetricDraftLocations,
  authoredMapFromMaterialized,
  MAP_EDITOR_BASE_SITE_CLEARANCE_TILES,
  MAP_EDITOR_DEFAULT_SIZE,
  MAP_EDITOR_DEFAULT_OIL_PATCHES,
  MAP_EDITOR_DEFAULT_STEEL_PATCHES,
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_MAX_BASE_SITES,
  MAP_EDITOR_MAX_OIL_PATCHES,
  MAP_EDITOR_MAX_STEEL_PATCHES,
  MAP_EDITOR_MAX_SIZE,
  MAP_EDITOR_MIN_SIZE,
  MAP_EDITOR_SYMMETRY,
  MapEditorSession,
  mapEditorRectTiles,
  materializedMapsEqual,
  moveSymmetricDraftLocation,
  removeDraftLocation,
  symmetricMapTiles,
  symmetricTerrainTiles,
} from "../../client/src/map_editor_session.js";

const baseLocations = (sites) => sites.map(({ x, y }) => ({ x, y }));

{
  const viewport = { keys: { up: false, down: false, left: false, right: false } };
  let prevented = 0;
  const dvorakQwertyWPosition = {
    code: "KeyW",
    key: ",",
    target: null,
    preventDefault() { prevented += 1; },
  };
  MapEditorViewport.prototype.handleKey.call(viewport, dvorakQwertyWPosition, true);
  assert.equal(viewport.keys.up, true, "Map Editor pan uses the physical W position on Dvorak");
  MapEditorViewport.prototype.handleKey.call(viewport, dvorakQwertyWPosition, false);
  assert.equal(viewport.keys.up, false, "Map Editor releases physical-position pan keys");
  assert.equal(prevented, 2);
}

{
  const savedCancel = globalThis.cancelAnimationFrame;
  const savedWindow = globalThis.window;
  const cancelled = [];
  globalThis.cancelAnimationFrame = (frame) => cancelled.push(frame);
  globalThis.window = { removeEventListener() {} };
  try {
    const viewport = {
      destroyed: false,
      frame: 73,
      unsubscribe() {},
      presentation: {
        canvas: { removeEventListener() {} },
        destroyed: 0,
        destroy() { this.destroyed += 1; },
      },
    };
    MapEditorViewport.prototype.destroy.call(viewport);
    MapEditorViewport.prototype.destroy.call(viewport);
    assert.deepEqual(cancelled, [73], "Map Editor teardown cancels its one owned RAF exactly once");
    assert.equal(viewport.presentation.destroyed, 1, "Map Editor teardown destroys presentation idempotently");
  } finally {
    globalThis.cancelAnimationFrame = savedCancel;
    globalThis.window = savedWindow;
  }
}

const repoRoot = new URL("../../", import.meta.url);
const oneVOneNoTerrainMap = JSON.parse(fs.readFileSync(new URL("server/assets/maps/1v1-no-terrain.json", repoRoot), "utf8"));
const serverMapSource = fs.readFileSync(new URL("server/crates/sim/src/game/map.rs", repoRoot), "utf8");
const serverProtocolSource = fs.readFileSync(new URL("server/crates/protocol/src/lab_scenario.rs", repoRoot), "utf8");
const mainSource = fs.readFileSync(new URL("client/src/main.js", repoRoot), "utf8");

assert(
  /try\s*\{\s*await app\.start\(\);\s*\}\s*catch \(error\)\s*\{[\s\S]*showRendererBootstrapError\(error\)/.test(mainSource),
  "application startup bounds Map Editor worker bootstrap rejection with the renderer error UI",
);

{
  const serverMainRadius = Number(serverMapSource.match(/BASE_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  const serverBaseRadius = Number(serverMapSource.match(/BASE_SITE_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  assert.equal(MAP_EDITOR_MAIN_CLEARANCE_TILES, serverMainRadius);
  assert.equal(MAP_EDITOR_BASE_SITE_CLEARANCE_TILES, serverBaseRadius);
  const serverMaxSteel = Number(serverProtocolSource.match(/MAX_STEEL_PATCHES_PER_BASE:\s*u32\s*=\s*(\d+)/)?.[1]);
  const serverMaxOil = Number(serverProtocolSource.match(/MAX_OIL_PATCHES_PER_BASE:\s*u32\s*=\s*(\d+)/)?.[1]);
  assert.equal(MAP_EDITOR_MAX_STEEL_PATCHES, serverMaxSteel);
  assert.equal(MAP_EDITOR_MAX_OIL_PATCHES, serverMaxOil);
}

{
  const panel = {
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return { maps: [
          { file: "schone-tage.json", name: "Schone Tage", description: "" },
          { file: "schone-tage(5).json", name: "Schone Tage", description: "" },
          { file: "../secret.json", name: "Secret", description: "" },
        ] };
      },
    }),
    selectedMapFile: "",
    catalog: [],
    catalogSkipped: [],
    render() { this.renders = (this.renders || 0) + 1; },
  };
  await MapEditorPanel.prototype.loadCatalog.call(panel);
  assert.deepEqual(panel.catalog.map((entry) => entry.file), ["schone-tage.json", "schone-tage(5).json"]);
  assert.deepEqual(panel.catalog.map((entry) => entry.label), [
    "Schone Tage — schone-tage.json",
    "Schone Tage — schone-tage(5).json",
  ], "duplicate map names are distinguishable by filename");
  assert.deepEqual(panel.catalogSkipped, ["../secret.json"], "unsupported map filenames are tracked for visible warnings");
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(oneVOneNoTerrainMap);
  const materialized = session.materialized();
  assert.equal(session.exportMap().version, 5);
  assert.equal(session.exportMap().layouts, undefined, "flat map data has no layout matrix");
  assert.equal(materialized.starts.length, 2);
  assert.equal(materialized.baseSites.length, 4, "every authored base is materialized without choosing a player layout");
  assert(materialized.baseSites.some((site) => site.x === 25 && site.y === 25), "start locations are permanent base sites");
  assert(materialized.baseSites.every((site) => site.steelPatches === 12 && site.oilPatches === 3),
    "bundled maps materialize per-base resource counts");
  assert.deepEqual(
    session.mapOverlay().bases.map((site) => site.index),
    [2, 3],
    "neutral base controls retain their backing authored base indices",
  );
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeFromScenario({
    name: "Checkpoint", map: { data: {
      size: 32, terrain: Array(32 * 32).fill(TERRAIN.GRASS), starts: [{ x: 8, y: 8 }],
      expansionSites: [{ x: 20, y: 20 }],
    } },
  });
  assert.equal(session.materialized().baseSites.length, 2, "checkpoint scenario expansion sites migrate into flat base sites");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 1 });
  let result;
  assert.equal(session.mutate("Removed final start", (draft) => {
    result = removeDraftLocation(draft, { kind: "start", locationIndex: 0 });
  }), true);
  assert.equal(result.ok, true);
  assert.equal(session.draft.startLocations.length, 0, "an editor draft may temporarily have no start locations");
  assert.deepEqual(session.draft.baseSites, [{
    x: 8, y: 8, steelPatches: MAP_EDITOR_DEFAULT_STEEL_PATCHES, oilPatches: MAP_EDITOR_DEFAULT_OIL_PATCHES,
  }], "removing a start keeps its resource site as a neutral base");
  assert.deepEqual(session.materialized().starts, [], "zero-start editor drafts remain materializable");

  assert.equal(session.mutate("Rebuilt radial starts", (draft) => {
    result = addSymmetricDraftLocations(draft, {
      kind: "start", tile: { x: 8, y: 8 }, symmetry: MAP_EDITOR_SYMMETRY.RADIAL,
    });
  }), true);
  assert.equal(result.count, 4);
  assert.deepEqual(session.draft.startLocations, [
    { x: 8, y: 8 }, { x: 23, y: 8 }, { x: 23, y: 23 }, { x: 8, y: 23 },
  ]);
  assert.deepEqual(baseLocations(session.draft.baseSites), session.draft.startLocations,
    "symmetric start placement reuses an existing base and creates only the missing resource sites");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 1 });
  const viewport = {
    tool: { kind: "start", locationIndex: 0, add: false },
    armTool(tool) { this.tool = tool; },
  };
  const statuses = [];
  const panel = {
    session,
    viewport,
    setStatus(message, error) { statuses.push({ message, error }); },
  };
  MapEditorPanel.prototype.removeLocation.call(panel, "start", 0);
  assert.equal(viewport.tool, null,
    "removing the final start clears its now-invalid armed move tool before the author clicks the map again");
  assert.deepEqual(statuses, [{ message: "Map location removed.", error: false }]);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 1 });
  let result;
  assert.equal(session.mutate("Completed radial starts", (draft) => {
    result = addSymmetricDraftLocations(draft, {
      kind: "start", tile: { x: 8, y: 8 }, symmetry: MAP_EDITOR_SYMMETRY.RADIAL,
    });
  }), true);
  assert.equal(result.count, 3, "symmetric placement fills only the missing counterparts of an existing start");
  assert.equal(session.draft.startLocations.length, 4);
  assert.equal(session.draft.baseSites.length, 4);
}

{
  const draft = authoredMapFromMaterialized({
    name: "Moved symmetric base", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 12 }, { x: 8, y: 19 }],
  });
  const result = moveSymmetricDraftLocation(draft, {
    kind: "base", locationIndex: 1, tile: { x: 10, y: 12 }, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  });
  assert.deepEqual(result, { ok: true, count: 2 });
  assert.deepEqual(baseLocations(draft.baseSites), [{ x: 8, y: 8 }, { x: 10, y: 12 }, { x: 10, y: 19 }],
    "a symmetric base move relocates its existing matching neutral base");
}

{
  const draft = authoredMapFromMaterialized({
    name: "Moved unpaired base", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 12 }],
  });
  const result = moveSymmetricDraftLocation(draft, {
    kind: "base", locationIndex: 1, tile: { x: 10, y: 12 }, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  });
  assert.deepEqual(result, { ok: true, count: 1 });
  assert.deepEqual(baseLocations(draft.baseSites), [{ x: 8, y: 8 }, { x: 10, y: 12 }],
    "a symmetric base move leaves a missing counterpart absent");
}

{
  const draft = authoredMapFromMaterialized({
    name: "Moved symmetric start bases", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }, { x: 8, y: 23 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 23 }],
  });
  const result = moveSymmetricDraftLocation(draft, {
    kind: "base", locationIndex: 0, tile: { x: 10, y: 8 }, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  });
  assert.deepEqual(result, { ok: true, count: 2 });
  assert.deepEqual(baseLocations(draft.baseSites), [{ x: 10, y: 8 }, { x: 10, y: 23 }]);
  assert.deepEqual(draft.startLocations, baseLocations(draft.baseSites),
    "moving symmetric start-backed bases keeps their start locations attached");
}

{
  const draft = authoredMapFromMaterialized({
    name: "Unmoved symmetric base", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 12 }, { x: 8, y: 19 }],
  });
  const before = structuredClone(draft);
  const result = moveSymmetricDraftLocation(draft, {
    kind: "base", locationIndex: 1, tile: { x: 8, y: 12 }, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  });
  assert.deepEqual(result, { ok: true, count: 0 });
  assert.deepEqual(draft, before, "an unchanged base move never removes its symmetric counterpart");
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(authoredMapFromMaterialized({
    name: "Reselected symmetric base", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 12 }, { x: 8, y: 19 }, { x: 14, y: 14 }],
  }));
  const viewport = {
    session,
    tool: { kind: "base", locationIndex: 2, add: false, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL },
    selectedBaseIndex: 2,
    setSelectedBase(index) { this.selectedBaseIndex = index; },
    onStatus() {},
  };
  MapEditorViewport.prototype.applySiteTool.call(viewport, { x: 10, y: 19 });
  assert.equal(viewport.selectedBaseIndex, 2,
    "moving an earlier symmetric base keeps the selected base on its stable backing index");
}

{
  const viewport = {
    selectedBaseIndex: null,
    redraws: 0,
    drawOverlay() { this.redraws += 1; },
  };
  MapEditorViewport.prototype.setSelectedBase.call(viewport, 7);
  MapEditorViewport.prototype.setSelectedBase.call(viewport, 7);
  MapEditorViewport.prototype.setSelectedBase.call(viewport, null);
  assert.equal(viewport.redraws, 2, "base selection redraws the editor overlay only when it changes");

  const site = MapEditorViewport.prototype.siteRecord.call({}, { x: 10, y: 12 }, 0xf4c542, 7, "B1", true);
  assert.deepEqual(site, { x: 336, y: 400, color: 0xf4c542, radius: 7, label: "B1", selected: true },
    "the selected base becomes a detached presentation marker");

  const recordViewport = {
    session: {
      draft: { terrain: Array(16) },
      mapOverlay: () => ({ starts: [], bases: [] }),
    },
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    overlayRevision: 0,
    selectedBaseIndex: null,
    siteRecord: MapEditorViewport.prototype.siteRecord,
    paintPreviewRecord: () => null,
  };
  MapEditorViewport.prototype.drawOverlay.call(recordViewport);
  assert.equal(recordViewport.pendingOverlay.revision, 1);
  assert(Array.isArray(recordViewport.pendingOverlay.gridPaths),
    "Map Editor grid lines cross as detached paths for the Pixi owner");
}

{
  const legacy = {
    version: 2,
    name: "Legacy",
    description: "",
    _design: "",
    terrain: ["................................", ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32), ".".repeat(32)],
    sites: [{ id: "main", kind: "main", x: 8, y: 8 }, { id: "natural", kind: "natural", x: 22, y: 22 }],
    layouts: [{ id: "2p", playerCount: 2, slots: [{ main: "main", naturals: ["natural"] }, { main: "natural", naturals: [] }] }],
  };
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(legacy);
  assert.equal(session.exportMap().version, 5, "local v2 maps migrate into current flat map data");
  assert.equal(session.exportMap().layouts, undefined);
}

{
  const legacyWorkspace = {
    version: 2,
    name: "Saved legacy map",
    terrain: Array.from({ length: 32 }, () => ".".repeat(32)),
    sites: [
      { id: "main", kind: "main", x: 8, y: 8 },
      { id: "natural", kind: "natural", x: 22, y: 22 },
    ],
    layouts: [{ id: "one", playerCount: 1, slots: [{ main: "main", naturals: ["natural"] }] }],
  };
  const values = new Map([
    ["rts.mapEditor.legacy-workspace.v2", JSON.stringify({ schemaVersion: 2, draft: legacyWorkspace })],
  ]);
  const storage = {
    getItem(key) { return values.get(key) || null; },
    setItem(key, value) { values.set(key, value); },
  };
  const session = new MapEditorSession({ storage });
  assert.equal(session.loadLocal("legacy-workspace"), true, "v5 sessions recover saved v2 workspaces");
  assert.equal(session.exportMap().version, 5);
  assert.equal(session.materialized().baseSites.length, 2);
}

{
  const v4Draft = {
    version: 4,
    name: "Saved v4 map",
    description: "",
    terrain: Array.from({ length: 16 }, () => ".".repeat(16)),
    startLocations: [{ x: 7, y: 7 }],
    baseSites: [{ x: 7, y: 7, steelPatches: 12, oilPatches: 3 }],
  };
  const values = new Map([
    ["rts.map-editor.v4.v4-workspace", JSON.stringify({ schemaVersion: 4, draft: v4Draft })],
  ]);
  const storage = {
    getItem(key) { return values.get(key) || null; },
    setItem(key, value) { values.set(key, value); },
  };
  const session = new MapEditorSession({ storage });
  assert.equal(session.loadLocal("v4-workspace"), true);
  assert.equal(session.exportMap().version, 5);
  assert.deepEqual(session.exportMap().doodads, [], "v4 local maps migrate to an empty v5 doodad layer");
  assert.equal(session.saveLocal("v4-workspace"), true);
  assert(values.has("rts.map-editor.v5.v4-workspace"), "new local saves use the v5 namespace");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  session.beginTerrainStroke();
  const start = session.draft.startLocations[0];
  assert.deepEqual(session.paintTerrainTiles([{ x: start.x + MAP_EDITOR_MAIN_CLEARANCE_TILES, y: start.y }], TERRAIN.WATER), []);
  assert.equal(session.commitTerrainStroke(), false);
  const base = session.draft.baseSites[0];
  assert.equal(base.x, start.x);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 1 });
  const panel = {
    session,
    setStatus(message) { this.status = message; },
  };
  MapEditorPanel.prototype.updateBasePatchCount.call(panel, 0, "steelPatches", 36);
  MapEditorPanel.prototype.updateBasePatchCount.call(panel, 0, "oilPatches", 9);
  assert.deepEqual(
    {
      steelPatches: session.draft.baseSites[0].steelPatches,
      oilPatches: session.draft.baseSites[0].oilPatches,
    },
    { steelPatches: 36, oilPatches: 9 },
    "Map Editor controls update the authoritative per-base patch counts",
  );
  MapEditorPanel.prototype.updateBasePatchCount.call(panel, 0, "steelPatches", 100);
  MapEditorPanel.prototype.updateBasePatchCount.call(panel, 0, "oilPatches", -4);
  assert.deepEqual(
    {
      steelPatches: session.draft.baseSites[0].steelPatches,
      oilPatches: session.draft.baseSites[0].oilPatches,
    },
    { steelPatches: 36, oilPatches: 0 },
    "Map Editor controls clamp Steel to 36 and Oil to the zero minimum",
  );
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const start = session.draft.startLocations[0];
  const roadTiles = [
    { x: start.x - MAP_EDITOR_MAIN_CLEARANCE_TILES, y: start.y, code: TERRAIN.ROAD_BARE },
    { x: start.x + MAP_EDITOR_MAIN_CLEARANCE_TILES, y: start.y, code: TERRAIN.ROAD_HORIZONTAL },
    { x: start.x, y: start.y + MAP_EDITOR_MAIN_CLEARANCE_TILES, code: TERRAIN.ROAD_VERTICAL },
    { x: start.x + 6, y: start.y + 6, code: TERRAIN.ROAD_DIAGONAL_NW_SE },
    { x: start.x + 6, y: start.y - 6, code: TERRAIN.ROAD_DIAGONAL_NE_SW },
  ];
  session.beginTerrainStroke();
  for (const tile of roadTiles) {
    assert.deepEqual(session.paintTerrainTiles([tile], tile.code), [tile]);
  }
  assert.equal(session.commitTerrainStroke(), true);
  for (const tile of roadTiles) {
    assert.equal(session.materialized().terrain[tile.y * 32 + tile.x], tile.code);
  }
  session.beginTerrainStroke();
  for (const tile of roadTiles) {
    assert.deepEqual(
      session.paintTerrainTiles([tile], TERRAIN.GRASS),
      [{ x: tile.x, y: tile.y, code: TERRAIN.GRASS }],
    );
  }
  assert.equal(session.commitTerrainStroke(), true);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 4 });
  for (const start of session.draft.startLocations) {
    assert(start.x >= MAP_EDITOR_MAIN_CLEARANCE_TILES && start.x < 16 - MAP_EDITOR_MAIN_CLEARANCE_TILES);
    assert(start.y >= MAP_EDITOR_MAIN_CLEARANCE_TILES && start.y < 16 - MAP_EDITOR_MAIN_CLEARANCE_TILES);
  }
}

{
  const session = new MapEditorSession({ storage: null });
  const viewport = { armed: "unchanged", armTool(tool) { this.armed = tool; } };
  const statuses = [];
  const panel = {
    session, viewport, blankMapSize: "48", selectedStartIndex: 3, selectedBaseIndex: 4,
    setStatus(message, error = false) { statuses.push({ message, error }); },
  };
  assert.equal(MapEditorPanel.prototype.newBlankMap.call(panel), true);
  assert.equal(session.draft.terrain.length, 48);
  assert.equal(viewport.armed, null);
  assert.deepEqual(statuses.pop(), { message: "Created a blank 48 × 48 two-player map.", error: false });

  const before = session.materialized();
  panel.blankMapSize = String(MAP_EDITOR_MAX_SIZE + 1);
  assert.equal(MapEditorPanel.prototype.newBlankMap.call(panel), false);
  assert.deepEqual(session.materialized(), before, "invalid custom sizes preserve the current draft");
  assert.deepEqual(statuses.pop(), {
    message: `Blank map size must be a whole number from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`,
    error: true,
  });

  const defaults = new MapEditorSession({ storage: null });
  defaults.initializeBlank();
  assert.equal(defaults.draft.terrain.length, MAP_EDITOR_DEFAULT_SIZE);
}

{
  const panel = {
    blankMapSize: "126", observedMapSize: null, renders: 0,
    render() { this.renders += 1; },
  };
  const snapshot = (size, detail = {}) => ({
    draft: { terrain: Array.from({ length: size }, () => ".".repeat(size)) },
    ...detail,
  });
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96));
  assert.equal(panel.blankMapSize, "96", "the blank-size field starts from the active map size");
  panel.blankMapSize = "72";
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96, { reason: "changed" }));
  assert.equal(panel.blankMapSize, "72", "ordinary map edits preserve an in-progress custom size");
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96, { reason: "loaded" }));
  assert.equal(panel.blankMapSize, "96", "loading a same-sized map restores its inferred size");
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(48, { reason: "undo" }));
  assert.equal(panel.blankMapSize, "48", "size-changing history updates the inferred size");
  assert.equal(panel.renders, 4);
}

{
  const starts = [{ x: 8, y: 8 }, { x: 117, y: 117 }, { x: 117, y: 8 }, { x: 8, y: 117 }];
  const baseSites = Array.from({ length: 32 }, (_, index) => ({ x: 20 + index, y: 20 }));
  const draft = authoredMapFromMaterialized({
    name: "Capped bases", description: "", size: 126,
    terrain: Array(126 * 126).fill(TERRAIN.GRASS), starts, baseSites,
  });
  assert.equal(draft.baseSites.length, MAP_EDITOR_MAX_BASE_SITES);
  for (const start of starts) assert(draft.baseSites.some((site) => site.x === start.x && site.y === start.y));
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 126, playerCount: 1 });
  let result;
  for (let x = 20; x < 51; x++) {
    assert.equal(session.mutate("Added base", (draft) => {
      result = addSymmetricDraftLocations(draft, { kind: "base", tile: { x, y: 40 } });
    }), true);
  }
  assert.equal(session.draft.baseSites.length, MAP_EDITOR_MAX_BASE_SITES);
  const before = session.materialized();
  assert.equal(session.mutate("Cannot add start beyond base capacity", (draft) => {
    result = addSymmetricDraftLocations(draft, { kind: "start", tile: { x: 80, y: 80 } });
  }), false);
  assert.match(result.error, /at most 32 base sites/);
  assert.deepEqual(session.materialized(), before, "adding a start must not discard an existing base site");
}

{
  assert.deepEqual(symmetricMapTiles(8, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.HORIZONTAL), [{ x: 1, y: 2 }, { x: 1, y: 5 }]);
  assert.deepEqual(symmetricMapTiles(8, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.HALF_TURN), [{ x: 1, y: 2 }, { x: 6, y: 5 }]);
  assert.deepEqual(symmetricMapTiles(8, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.THREE_WAY), [{ x: 1, y: 2 }, { x: 6, y: 2 }, { x: 3, y: 6 }]);
  assert.deepEqual(
    symmetricMapTiles(8, [{ x: 0, y: 0 }], MAP_EDITOR_SYMMETRY.THREE_WAY),
    [{ x: 0, y: 0 }],
    "three-way copies beyond the square map are omitted",
  );
  assert.deepEqual(symmetricMapTiles(8, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.RADIAL), [{ x: 1, y: 2 }, { x: 5, y: 1 }, { x: 6, y: 5 }, { x: 2, y: 6 }]);
  const threeWayGuides = mapEditorSymmetryGuideLines(8, MAP_EDITOR_SYMMETRY.THREE_WAY);
  assert.equal(threeWayGuides.length, 3);
  assert.deepEqual(threeWayGuides[0], { x0: 128, y0: 128, x1: 128, y1: 0 });
  assert(Math.abs(threeWayGuides[1].x1 - 256) < 1e-9 && Math.abs(threeWayGuides[1].y1 - 201.9008344562721) < 1e-9);
  assert(Math.abs(threeWayGuides[2].x1) < 1e-9 && Math.abs(threeWayGuides[2].y1 - 201.9008344562721) < 1e-9);
  assert.deepEqual(mapEditorSymmetryGuideLines(8, MAP_EDITOR_SYMMETRY.RADIAL), [
    { x0: 0, y0: 128, x1: 256, y1: 128 }, { x0: 128, y0: 0, x1: 128, y1: 256 },
  ]);
  assert.deepEqual(mapEditorSymmetryGuideCentre(8, MAP_EDITOR_SYMMETRY.HALF_TURN), { x: 128, y: 128 });
  assert.deepEqual(mapEditorRectTiles({ x: 1, y: 1 }, { x: 2, y: 3 }, 8), [
    { x: 1, y: 1 }, { x: 2, y: 1 }, { x: 1, y: 2 }, { x: 2, y: 2 }, { x: 1, y: 3 }, { x: 2, y: 3 },
  ]);
}

{
  assert.deepEqual(
    symmetricTerrainTiles(8, [{ x: 1, y: 2 }], TERRAIN.ROAD_HORIZONTAL, MAP_EDITOR_SYMMETRY.THREE_WAY),
    [
      { x: 1, y: 2, paintTerrainCode: TERRAIN.ROAD_HORIZONTAL },
      { x: 6, y: 2, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NE_SW },
      { x: 3, y: 6, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NW_SE },
    ],
    "three-way symmetry snaps marked roads to the nearest square-grid direction",
  );
  assert.deepEqual(
    symmetricTerrainTiles(8, [{ x: 1, y: 2 }], TERRAIN.ROAD_DIAGONAL_NW_SE, MAP_EDITOR_SYMMETRY.HORIZONTAL),
    [
      { x: 1, y: 2, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NW_SE },
      { x: 1, y: 5, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NE_SW },
    ],
    "reflected diagonal roads swap their marking orientation",
  );
  assert.deepEqual(
    symmetricTerrainTiles(8, [{ x: 1, y: 2 }], TERRAIN.ROAD_HORIZONTAL, MAP_EDITOR_SYMMETRY.RADIAL),
    [
      { x: 1, y: 2, paintTerrainCode: TERRAIN.ROAD_HORIZONTAL },
      { x: 5, y: 1, paintTerrainCode: TERRAIN.ROAD_VERTICAL },
      { x: 6, y: 5, paintTerrainCode: TERRAIN.ROAD_HORIZONTAL },
      { x: 2, y: 6, paintTerrainCode: TERRAIN.ROAD_VERTICAL },
    ],
    "quarter-turn symmetry rotates horizontal road markings vertically",
  );

  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const painted = symmetricTerrainTiles(
    32,
    [{ x: 10, y: 12 }],
    TERRAIN.ROAD_DIAGONAL_NW_SE,
    MAP_EDITOR_SYMMETRY.HORIZONTAL,
  );
  session.beginTerrainStroke();
  assert.deepEqual(session.paintTerrainTiles(painted, TERRAIN.ROAD_DIAGONAL_NW_SE), [
    { x: 10, y: 12, code: TERRAIN.ROAD_DIAGONAL_NW_SE },
    { x: 10, y: 19, code: TERRAIN.ROAD_DIAGONAL_NE_SW },
  ]);
  assert.equal(session.commitTerrainStroke(), true);
  const map = session.materialized();
  assert.equal(map.terrain[12 * 32 + 10], TERRAIN.ROAD_DIAGONAL_NW_SE);
  assert.equal(map.terrain[19 * 32 + 10], TERRAIN.ROAD_DIAGONAL_NE_SW);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 126, playerCount: 2 });
  let result;
  assert.equal(session.mutate("Added radial bases", (draft) => {
    result = addSymmetricDraftLocations(draft, { kind: "base", tile: { x: 45, y: 45 }, symmetry: MAP_EDITOR_SYMMETRY.RADIAL });
  }), true);
  assert.equal(result.count, 4);
  assert.equal(session.draft.baseSites.length, 6, "base sites are not capped per player");
  assert.equal(session.mutate("Moved radial starts", (draft) => {
    result = moveSymmetricDraftLocation(draft, { kind: "start", locationIndex: 0, tile: { x: 40, y: 46 }, symmetry: MAP_EDITOR_SYMMETRY.RADIAL });
  }), true);
  assert.equal(result.count, 2, "symmetry moves existing matching start locations only");
  assert.equal(session.mutate("Cannot remove start base", (draft) => {
    result = removeDraftLocation(draft, { kind: "base", locationIndex: 0 });
  }), false);
  assert.match(result.error, /Remove the matching start/);
}

{
  const draft = authoredMapFromMaterialized({
    name: "Three-player map", description: "", size: 126,
    terrain: Array(126 * 126).fill(TERRAIN.GRASS), starts: [], baseSites: [],
  });
  const result = addSymmetricDraftLocations(draft, {
    kind: "start", tile: { x: 47, y: 7 }, symmetry: MAP_EDITOR_SYMMETRY.THREE_WAY,
  });
  assert.deepEqual(result, { ok: true, count: 3 });
  assert.deepEqual(draft.startLocations, [{ x: 47, y: 7 }, { x: 118, y: 77 }, { x: 22, y: 104 }]);
  assert.deepEqual(baseLocations(draft.baseSites), draft.startLocations,
    "three-way placement creates a matching base for each 1v1v1 start");

  const movedFromRotatedCopy = moveSymmetricDraftLocation(draft, {
    kind: "start", locationIndex: 1, tile: { x: 117, y: 77 },
    symmetry: MAP_EDITOR_SYMMETRY.THREE_WAY,
  });
  assert.deepEqual(movedFromRotatedCopy, { ok: true, count: 3 });
  assert.deepEqual(draft.startLocations, [{ x: 48, y: 8 }, { x: 117, y: 77 }, { x: 23, y: 102 }],
    "a rotated copy with square-grid rounding drift still moves the complete location group");
  assert.deepEqual(baseLocations(draft.baseSites), draft.startLocations,
    "moving a rounded three-way copy keeps its matching start bases coupled");

  const edgeDraft = authoredMapFromMaterialized({
    name: "Edge placement", description: "", size: 126,
    terrain: Array(126 * 126).fill(TERRAIN.GRASS), starts: [], baseSites: [],
  });
  const edgeBefore = structuredClone(edgeDraft);
  const invalidAdd = addSymmetricDraftLocations(edgeDraft, {
    kind: "start", tile: { x: 22, y: 7 }, symmetry: MAP_EDITOR_SYMMETRY.THREE_WAY,
  });
  assert.equal(invalidAdd.ok, false);
  assert.match(invalidAdd.error, /edge clearance/);
  assert.deepEqual(edgeDraft, edgeBefore, "three-way placement rejects clipped or edge-unsafe copies atomically");

  const before = structuredClone(draft);
  const invalidMove = moveSymmetricDraftLocation(draft, {
    kind: "start", locationIndex: 0, tile: { x: 22, y: 7 }, symmetry: MAP_EDITOR_SYMMETRY.THREE_WAY,
  });
  assert.equal(invalidMove.ok, false);
  assert.match(invalidMove.error, /edge clearance/);
  assert.deepEqual(draft, before, "three-way relocation rejects clipped or edge-unsafe copies atomically");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 126, playerCount: 2 });
  let result;
  assert.equal(session.mutate("Moved half-turn starts", (draft) => {
    result = moveSymmetricDraftLocation(draft, {
      kind: "start", locationIndex: 0, tile: { x: 40, y: 46 }, symmetry: MAP_EDITOR_SYMMETRY.HALF_TURN,
    });
  }), true);
  assert.equal(result.count, 2, "half-turn moves the opposing start and its matching base site");
  assert.deepEqual(session.draft.startLocations, [{ x: 40, y: 46 }, { x: 85, y: 79 }]);
  assert.deepEqual(baseLocations(session.draft.baseSites), session.draft.startLocations);
}

{
  const draft = authoredMapFromMaterialized({
    name: "Swap", description: "", size: 32, terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }, { x: 8, y: 23 }],
    baseSites: [{ x: 8, y: 8 }, { x: 8, y: 23 }],
  });
  const result = moveSymmetricDraftLocation(draft, {
    kind: "start", locationIndex: 0, tile: { x: 8, y: 23 }, symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  });
  assert.equal(result.ok, true);
  assert.deepEqual(draft.startLocations, [{ x: 8, y: 23 }, { x: 8, y: 8 }], "symmetric base swaps stay atomic");
  assert.deepEqual(baseLocations(draft.baseSites), draft.startLocations);
}

{
  const draft = authoredMapFromMaterialized({
    name: "Round trip", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }, { x: 23, y: 23 }],
    baseSites: [{ x: 8, y: 8 }, { x: 23, y: 23 }, { x: 16, y: 16 }],
  });
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(draft);
  const rebuilt = new MapEditorSession({ storage: null });
  rebuilt.loadAuthoredMap(authoredMapFromMaterialized({ ...session.materialized(), description: "" }));
  assert.equal(materializedMapsEqual(session.materialized(), rebuilt.materialized()), true);
}

{
  const request = [];
  await createMapHandoff({
    destination: "lab", authoredMap: { version: 5 }, materializedMap: { starts: [], baseSites: [], doodads: [] },
    fetchImpl: async (_url, init) => {
      request.push(JSON.parse(init.body));
      return { ok: true, json: async () => ({ handoffId: "0123456789abcdef0123456789abcdef" }) };
    },
  });
  assert.equal(request[0].selectedLayoutId, undefined, "handoffs carry flat map data only");
}

{
  assert.equal(mapEditorLaunchConfig({ search: "?workspace=map-1", pathname: "/map-editor" }).workspaceId, "map-1");
  assert.equal(MAP_EDITOR_MAX_BASE_SITES, 32);
  assert.equal(MAP_EDITOR_MAX_STEEL_PATCHES, 36);
  assert.equal(MAP_EDITOR_MAX_OIL_PATCHES, 9);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  session.beginTerrainStroke();
  session.paintTerrainTiles([{ x: 0, y: 0 }], TERRAIN.WATER);
  const statuses = [];
  const viewport = {
    paintPointerId: 7, panPointerId: null, tool: { kind: "terrain", shape: "box" },
    paintStartTile: { x: 4, y: 4 }, lastPaintTile: { x: 12, y: 12 }, session,
    eventTile() { throw new Error("cancelled paint must not resolve a release tile"); },
    paintBox() { throw new Error("cancelled paint must not fill a box"); },
    drawOverlay() {}, onStatus: (message, error) => statuses.push({ message, error }),
  };
  MapEditorViewport.prototype.handlePointerUp.call(viewport, {
    type: "pointercancel", pointerId: 7, currentTarget: { releasePointerCapture() {} },
  });
  assert.equal(session.materialized().terrain[0], TERRAIN.GRASS);
  assert.deepEqual(statuses, [{ message: "Terrain paint cancelled.", error: false }]);
  assert.equal(viewport.paintStartTile, null, "pointer cancellation clears the pending box preview");
}

{
  assert.equal(MAP_EDITOR_MAX_DOODADS, 4096, "the editor mirrors the authoritative doodad cap");
  assert.deepEqual(
    MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "tree").map((entry) => entry.typeId),
    [
      "tree.oak", "tree.pine", "tree.birch", "tree.spruce", "tree.aspen", "tree.alder",
      "tree.oak.topdown", "tree.pine.topdown", "tree.birch.topdown", "tree.spruce.topdown",
      "tree.aspen.topdown", "tree.alder.topdown",
    ],
    "the tree palette exposes six inert species in both 3/4 and top-down perspectives",
  );
  assert.equal(canonicalDoodadColor(" #AbC "), "#aabbcc");
  assert.equal(canonicalDoodadColor("not-a-color"), null);
  const doodads = normalizeMapEditorDoodads([
    { id: 7, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 12, y: 20, color: "#ffffff" },
    { id: 7, typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, x: 30, y: 40, color: "#F0A" },
    { id: 0, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 50, y: 60 },
    { id: 9, typeId: "tree.unknown", x: 1, y: 1 },
    { id: 10, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_BIRCH, x: 128, y: 0 },
  ], 128);
  assert.deepEqual(doodads, [
    { typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, x: 30, y: 40, color: "#ff00aa", id: 1 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 50, y: 60, id: 2 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 12, y: 20, id: 7 },
  ], "normalization repairs ids deterministically, canonicalizes flowers, strips tree color, and rejects invalid records");
}

{
  const once = createDoodadSprayStroke({ x: 100, y: 100 }, { radius: 24, density: 4, seed: 19 });
  const oncePlacements = [...once.placements, ...extendDoodadSprayStroke(once.stroke, { x: 180, y: 100 })];
  const partitioned = createDoodadSprayStroke({ x: 100, y: 100 }, { radius: 24, density: 4, seed: 19 });
  const partitionedPlacements = [...partitioned.placements];
  for (const x of [110, 123, 141, 160, 180]) {
    partitionedPlacements.push(...extendDoodadSprayStroke(partitioned.stroke, { x, y: 100 }));
  }
  assert.deepEqual(partitionedPlacements, oncePlacements,
    "fixed-distance spray output is independent of pointer-event subdivision");
  assert.deepEqual(symmetricDoodadPlacements(320, [{ x: 17, y: 31 }], MAP_EDITOR_SYMMETRY.HALF_TURN), [
    { x: 17, y: 31 }, { x: 302, y: 288 },
  ], "doodad symmetry operates on sub-tile world pixels");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  assert.equal(session.exportMap().version, 5);
  assert.deepEqual(session.materialized().doodads, []);
  session.beginDoodadStroke("Sprayed flowers");
  const added = session.placeDoodads([{ x: 100, y: 120 }, { x: 140, y: 150 }], {
    typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_CLUSTER,
    color: "#C58AF9",
  });
  assert.deepEqual(added.map((record) => record.id), [1, 2], "new doodads receive stable smallest-free numeric ids");
  assert.equal(session.commitDoodadStroke(), true);
  assert.equal(session.undoStack.length, 1, "a complete doodad stroke creates one history entry");
  assert.deepEqual(session.materialized().doodads.map((record) => record.color), ["#c58af9", "#c58af9"]);
  session.undo();
  assert.deepEqual(session.materialized().doodads, []);
  session.redo();
  assert.deepEqual(session.materialized().doodads.map((record) => record.id), [1, 2]);

  session.beginDoodadStroke("Cancelled tree");
  session.placeDoodads([{ x: 200, y: 220 }], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_BIRCH });
  session.cancelDoodadStroke();
  assert.deepEqual(session.materialized().doodads.map((record) => record.id), [1, 2],
    "pointer cancellation can restore the doodad draft atomically");
}

{
  const draft = authoredMapFromMaterialized({
    name: "Doodad round trip", description: "", size: 16,
    terrain: Array(16 * 16).fill(TERRAIN.GRASS), starts: [], baseSites: [],
    doodads: [{ id: 4, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 111, y: 222 }],
  });
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(draft);
  assert.deepEqual(session.materialized().doodads, draft.doodads);
  const other = structuredClone(session.materialized());
  assert.equal(materializedMapsEqual(session.materialized(), other), true);
  other.doodads[0].x += 1;
  assert.equal(materializedMapsEqual(session.materialized(), other), false,
    "handoff equality includes static doodads");
}

{
  const record = createMapEditorPresentation({
    frameId: 1,
    camera: { x: 0, y: 0, zoom: 1 },
    visualTimeMs: 1200,
    doodadUpdate: {
      kind: "replace", revision: 1,
      doodads: [{ id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 30, y: 40 }],
    },
    overlay: {
      doodadSelection: { id: 1, x: 30, y: 40 },
      doodadBrushPreview: { x: 40, y: 50, radius: 48, mode: "erase", typeId: null, color: null },
    },
  });
  assert.equal(structuredClone(record).doodadUpdate.doodads[0].id, 1,
    "the revisioned doodad replacement and editor overlay remain structured-cloneable");
  assert.throws(() => createMapEditorPresentation({
    frameId: 1, camera: { x: 0, y: 0, zoom: 1 },
    doodadUpdate: { kind: "patch", revision: 1, upserts: [], removedIds: [2, 2] },
  }), /duplicate removed ids/);
}

{
  const draft = authoredMapFromMaterialized({
    name: "Cap", description: "", size: 16,
    terrain: Array(16 * 16).fill(TERRAIN.GRASS), starts: [], baseSites: [], doodads: [],
  });
  draft.doodads = Array.from({ length: MAP_EDITOR_MAX_DOODADS - 1 }, (_, index) => ({
    id: index + 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: index % 512, y: Math.floor(index / 512),
  }));
  const added = createMapEditorDoodads(draft, [{ x: 1, y: 1 }, { x: 2, y: 2 }], {
    typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE,
  });
  assert.equal(added.length, 1);
  assert.equal(draft.doodads.length, MAP_EDITOR_MAX_DOODADS, "authoring stops exactly at the exported cap");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Sprayed flowers");
  session.placeDoodads([{ x: 50, y: 60 }], {
    typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE,
    color: "#ef739d",
  });
  const statuses = [];
  const viewport = {
    doodadPointerId: 17,
    doodadPointerMode: "spray",
    doodadDragOffset: null,
    doodadSprayStroke: {},
    doodadLastWorld: { x: 50, y: 60 },
    paintPointerId: null,
    panPointerId: null,
    session,
    drawOverlay() {},
    onStatus(message, error) { statuses.push({ message, error }); },
  };
  MapEditorViewport.prototype.handlePointerUp.call(viewport, {
    type: "pointercancel", pointerId: 17, currentTarget: { releasePointerCapture() {} },
  });
  assert.deepEqual(session.materialized().doodads, [], "pointercancel rolls back an in-progress doodad stroke");
  assert.equal(session.undoStack.length, 0, "a cancelled doodad stroke never enters history");
  assert.deepEqual(statuses, [{ message: "Doodad edit cancelled.", error: false }]);
}

{
  const viewport = {
    doodadRevision: 3,
    pendingDoodadUpdate: {
      kind: "patch", revision: 3,
      upserts: [{ id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 10, y: 20 }],
      removedIds: [2],
    },
  };
  MapEditorViewport.prototype.queueDoodadPatch.call(viewport, {
    upserts: [{ id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER_TOPDOWN, x: 30, y: 40 }],
    removedIds: [1],
  });
  assert.deepEqual(viewport.pendingDoodadUpdate, {
    kind: "patch", revision: 4,
    upserts: [{ id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER_TOPDOWN, x: 30, y: 40 }],
    removedIds: [1],
  }, "pending doodad patches coalesce by stable id without resurrecting removals");
}
