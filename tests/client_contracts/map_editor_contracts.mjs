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
        draft: { width: 4, height: 4, terrain: ["....", "....", "....", "...."] },
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
    assert.equal(presentations[0].record.version, 2);
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
import { authoritativeAnalysisSummary, MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { defaultMapAuthoringLayerVisibility } from "../../client/src/map_authoring/layers.js";
import {
  canonicalDoodadColor,
  createDoodadSprayStroke,
  createMapEditorDoodads,
  doodadIdsWithinRect,
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
  mapEditorSymmetrySupported,
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
  session.initializeBlank({ size: 16, playerCount: 1 });
  const statuses = [];
  const panel = {
    session,
    viewport: { armTool(tool) { this.tool = tool; } },
    selectedStartIndex: 4,
    selectedBaseIndex: 7,
    loadMapData: MapEditorPanel.prototype.loadMapData,
    setStatus(message, error = false) { statuses.push({ message, error }); },
  };
  await MapEditorPanel.prototype.loadJsonFile.call(panel, {
    name: "local-map.json",
    size: 1024,
    async text() { return JSON.stringify(oneVOneNoTerrainMap); },
  });
  assert.equal(session.exportMap().name, oneVOneNoTerrainMap.name, "local JSON loads through the authored-map validator");
  assert.equal(session.hasUnsavedChanges, false, "a freshly imported file is the editor's saved baseline");
  assert.equal(panel.selectedStartIndex, 0);
  assert.equal(panel.selectedBaseIndex, 0);
  assert.equal(panel.viewport.tool, null);
  assert.deepEqual(statuses.pop(), { message: "Loaded local-map.json.", error: false });

  const recipe = {
    name: "UI Recipe",
    width: 32,
    height: 32,
    operations: [],
  };
  await MapEditorPanel.prototype.loadJsonFile.call(panel, {
    name: "recipe.json",
    size: 2048,
    async text() { return JSON.stringify(recipe); },
  });
  assert.equal(session.exportMap().name, oneVOneNoTerrainMap.name,
    "recipe input does not replace the current human-authored map");
  assert.deepEqual(statuses.pop(), {
    message: "Could not load recipe.json: Map JSON needs a terrain array.",
    error: true,
  });

  await MapEditorPanel.prototype.loadJsonFile.call(panel, {
    name: "broken.json",
    size: 12,
    async text() { return "{not json"; },
  });
  assert.match(statuses.at(-1).message, /^Could not load broken\.json:/);
  assert.equal(statuses.at(-1).error, true, "invalid local JSON produces a visible error");

  await MapEditorPanel.prototype.loadJsonFile.call(panel, {
    name: "huge.json",
    size: 2 * 1024 * 1024 + 1,
    async text() { throw new Error("oversized files must not be read"); },
  });
  assert.deepEqual(statuses.at(-1), {
    message: "Could not load huge.json: Map JSON files must be 2 MB or smaller.",
    error: true,
  });

  session.mutate("Changed imported map", (draft) => { draft.description = "changed"; });
  assert.equal(session.hasUnsavedChanges, true);
  const savedDocument = globalThis.document;
  const savedCreateObjectURL = URL.createObjectURL;
  const savedRevokeObjectURL = URL.revokeObjectURL;
  const anchor = { click() {}, remove() {} };
  globalThis.document = {
    body: { appendChild(node) { assert.equal(node, anchor); } },
    createElement(tag) { assert.equal(tag, "a"); return anchor; },
  };
  URL.createObjectURL = () => "blob:map-export";
  URL.revokeObjectURL = (url) => { assert.equal(url, "blob:map-export"); };
  try {
    MapEditorPanel.prototype.exportJson.call(panel);
  } finally {
    globalThis.document = savedDocument;
    URL.createObjectURL = savedCreateObjectURL;
    URL.revokeObjectURL = savedRevokeObjectURL;
  }
  assert.equal(session.hasUnsavedChanges, false, "exporting the current map clears the unsaved-change warning");
  assert.match(statuses.at(-1).message, /^Exported .+\.json\.$/);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 24, playerCount: 2 });
  const requests = [];
  const statuses = [];
  const panel = {
    session,
    fetchImpl: async (url, options) => {
      requests.push({ url, options });
      if (url.endsWith("/report")) {
        return {
          ok: true,
          async json() {
            return {
              valid: true,
              analyzedRouteCount: 2,
              unanalyzedRouteCount: 1,
              routes: [
                { analyzed: true, reachable: true },
                { analyzed: true, reachable: false },
                { analyzed: false, reachable: false, failureReason: "analysisBudgetExhausted" },
              ],
            };
          },
        };
      }
      return {
        ok: true,
        async json() { return { valid: true, baseSites: [{}, {}], startLocations: [{}, {}] }; },
      };
    },
    analysisPending: false,
    analysisKind: null,
    analysisResult: null,
    setStatus(message, error = false) { statuses.push({ message, error }); },
    render() { this.renders = (this.renders || 0) + 1; },
  };
  const check = await MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "check");
  assert.equal(check.valid, true);
  assert.equal(requests[0].url, "/api/map-authoring/check");
  assert.equal(requests[0].options.method, "POST");
  assert.deepEqual(JSON.parse(requests[0].options.body), session.exportMap());
  assert.deepEqual(statuses.at(-1), { message: "Authoritative check passed: 2 bases, 2 starts.", error: false });

  const report = await MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "report");
  assert.equal(report.unanalyzedRouteCount, 1);
  assert.deepEqual(statuses.at(-1), {
    message: "Route report: 2 analyzed, 1 unreachable; 1 unanalyzed/truncated.",
    error: false,
  }, "unanalyzed rows are called out and excluded from unreachable routes");
  assert.equal(authoritativeAnalysisSummary("report", report), statuses.at(-1).message);
  assert.equal(
    authoritativeAnalysisSummary("report", { valid: true, routes: [] }),
    "Route report: unknown analyzed, 0 unreachable; unknown unanalyzed/truncated.",
  );
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(oneVOneNoTerrainMap);
  const materialized = session.materialized();
  assert.equal(session.exportMap().version, 6);
  assert.deepEqual({ width: materialized.width, height: materialized.height }, { width: 126, height: 126 });
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
  assert.equal(session.initializeFromStart({
    map: { width: 12, height: 8, terrain: Array(12 * 8).fill(TERRAIN.GRASS) },
    players: [{ startTileX: 3, startTileY: 2 }, { startTileX: 9, startTileY: 6 }],
  }, { name: "Wide authoritative map" }), true);
  assert.deepEqual({ width: session.draft.width, height: session.draft.height }, { width: 12, height: 8 },
    "authoritative rectangular maps import without requiring equal axes");
  const bounds = [];
  const viewport = {
    session,
    terrainRevision: 0,
    root: { clientWidth: 960, clientHeight: 640 },
    camera: {
      worldW: 0,
      setBounds(...args) { bounds.push(args); },
      setZoom(value) { this.zoom = value; },
      centerOn(x, y) { this.centre = { x, y }; },
    },
  };
  MapEditorViewport.prototype.rebuildTerrain.call(viewport);
  assert.deepEqual(bounds, [[384, 256, 960, 640]], "editor camera bounds preserve rectangular world extents");
  assert.deepEqual(viewport.camera.centre, { x: 192, y: 128 });
  assert.deepEqual({
    width: viewport.pendingTerrainUpdate.width,
    height: viewport.pendingTerrainUpdate.height,
    cells: viewport.pendingTerrainUpdate.terrain.length,
  }, { width: 12, height: 8, cells: 96 }, "worker terrain replacement carries both axes explicitly");
}

{
  const views = [];
  const viewport = {
    camera: {
      worldW: 3200,
      worldH: 1600,
      viewW: 800,
      viewH: 600,
      zoom: 1,
      setView(view) { views.push(view); this.zoom = view.zoom; },
      setZoom(zoom) { this.zoom = Math.max(0.05, Math.min(4, zoom)); },
    },
    frameMap: MapEditorViewport.prototype.frameMap,
    zoomPercent: MapEditorViewport.prototype.zoomPercent,
  };
  assert.equal(MapEditorViewport.prototype.fitToScreen.call(viewport), true);
  assert.deepEqual(views.at(-1), { centerX: 1600, centerY: 800, zoom: 0.25 },
    "Fit to screen centers the map and keeps its full rectangular extent visible");
  assert.equal(MapEditorViewport.prototype.fillScreen.call(viewport), true);
  assert.deepEqual(views.at(-1), { centerX: 1600, centerY: 800, zoom: 0.375 },
    "Fill screen centers the map and covers the viewport");
  assert.equal(MapEditorViewport.prototype.setZoomPercent.call(viewport, 175), 175,
    "the direct percentage control sets camera scale");
  assert.equal(MapEditorViewport.prototype.zoomIn.call(viewport), 219,
    "the plus control zooms in around the viewport centre");
  assert.equal(MapEditorViewport.prototype.zoomOut.call(viewport), 175,
    "the minus control reverses one zoom step");
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
      draft: { width: 16, height: 16, terrain: Array(16) },
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
  assert.equal(session.exportMap().version, 6, "local v2 maps migrate into current flat map data");
  assert.equal(session.exportMap().layouts, undefined);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const overlap = [{ x: 14, y: 14 }, { x: 15, y: 14 }];
  session.beginOverlayStroke("Painted stealth");
  assert.deepEqual(session.paintOverlayTiles(overlap, { stealth: true }), overlap);
  assert.equal(session.commitOverlayStroke(), true);
  session.beginOverlayStroke("Excluded vehicles");
  assert.deepEqual(session.paintOverlayTiles(overlap, { noVehicle: true }), overlap);
  assert.equal(session.commitOverlayStroke(), true);
  assert.deepEqual(session.materialized().stealthTiles, overlap);
  assert.deepEqual(session.materialized().noVehicleTiles, overlap,
    "independent authoring tools may intentionally overlap their sparse semantic layers");

  session.beginOverlayStroke("Made long grass");
  assert.deepEqual(session.paintOverlayTiles([overlap[1]], { noVehicle: false }), [overlap[1]]);
  assert.equal(session.commitOverlayStroke(), true);
  assert.deepEqual(session.materialized().stealthTiles, overlap,
    "removing vehicle exclusion leaves independent stealth cover intact");
  assert.deepEqual(session.materialized().noVehicleTiles, [overlap[0]]);
  assert.deepEqual(session.exportMap().stealthTiles, overlap,
    "authored exports retain sparse coordinate pairs rather than a full tile layer");
  assert.equal(session.undo(), true);
  assert.deepEqual(session.materialized().noVehicleTiles, overlap,
    "overlay strokes participate in the editor's normal undo history");
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
  session.initializeBlank({ size: 32, playerCount: 2 });
  const start = session.draft.startLocations[0];
  const variants = [
    TERRAIN.GRAVEL_A,
    TERRAIN.GRAVEL_B,
    TERRAIN.GRAVEL_C,
    TERRAIN.DIRT_A,
    TERRAIN.DIRT_B,
    TERRAIN.DIRT_C,
    TERRAIN.MUD_A,
    TERRAIN.MUD_B,
    TERRAIN.MUD_C,
    TERRAIN.FROSTED_GROUND,
  ];
  const chars = "0123456789";
  session.beginTerrainStroke();
  for (const [index, code] of variants.entries()) {
    const tile = { x: start.x - 7 + index, y: start.y };
    assert.deepEqual(session.paintTerrainTiles([tile], code), [{ ...tile, code }]);
  }
  assert.equal(session.commitTerrainStroke(), true);
  const materialized = session.materialized();
  for (const [index, code] of variants.entries()) {
    const x = start.x - 7 + index;
    assert.equal(materialized.terrain[start.y * 32 + x], code, `open terrain ${code} survives materialization`);
    assert.equal(session.exportMap().terrain[start.y][x], chars[index], `open terrain ${code} keeps its authored character`);
  }
  const rebuilt = new MapEditorSession({ storage: null });
  rebuilt.loadAuthoredMap(session.exportMap());
  assert.deepEqual(rebuilt.materialized().terrain, materialized.terrain, "visual open terrains survive editor export/import");
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
    session, viewport, blankMapWidth: "64", blankMapHeight: "48", selectedStartIndex: 3, selectedBaseIndex: 4,
    requestedDimensions: MapEditorPanel.prototype.requestedDimensions,
    setStatus(message, error = false) { statuses.push({ message, error }); },
  };
  assert.equal(MapEditorPanel.prototype.newBlankMap.call(panel), true);
  assert.deepEqual({ width: session.draft.width, height: session.draft.height }, { width: 64, height: 48 });
  assert.equal(viewport.armed, null);
  assert.deepEqual(statuses.pop(), { message: "Created a blank 64 × 48 two-player map.", error: false });

  const before = session.materialized();
  panel.blankMapWidth = String(MAP_EDITOR_MAX_SIZE + 1);
  assert.equal(MapEditorPanel.prototype.newBlankMap.call(panel), false);
  assert.deepEqual(session.materialized(), before, "invalid custom sizes preserve the current draft");
  assert.deepEqual(statuses.pop(), {
    message: `Map width and height must be whole numbers from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`,
    error: true,
  });

  const defaults = new MapEditorSession({ storage: null });
  defaults.initializeBlank();
  assert.deepEqual({ width: defaults.draft.width, height: defaults.draft.height }, {
    width: MAP_EDITOR_DEFAULT_SIZE, height: MAP_EDITOR_DEFAULT_SIZE,
  });
}

{
  const panel = {
    blankMapWidth: "126", blankMapHeight: "126", observedMapDimensions: null,
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    viewport: { setSymmetry() {}, tool: null },
    renders: 0,
    render() { this.renders += 1; },
  };
  const snapshot = (width, height, detail = {}) => ({
    draft: { width, height, terrain: Array.from({ length: height }, () => ".".repeat(width)) },
    ...detail,
  });
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96, 64));
  assert.deepEqual([panel.blankMapWidth, panel.blankMapHeight], ["96", "64"], "dimension fields start from the active map");
  panel.blankMapWidth = "72";
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96, 64, { reason: "changed" }));
  assert.equal(panel.blankMapWidth, "72", "ordinary map edits preserve in-progress dimensions");
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(96, 64, { reason: "loaded" }));
  assert.deepEqual([panel.blankMapWidth, panel.blankMapHeight], ["96", "64"], "loading restores both dimensions");
  MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot(48, 32, { reason: "undo" }));
  assert.deepEqual([panel.blankMapWidth, panel.blankMapHeight], ["48", "32"], "dimension-changing history updates both fields");
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
  assert.deepEqual(symmetricMapTiles({ width: 12, height: 8 }, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.HALF_TURN), [
    { x: 1, y: 2 }, { x: 10, y: 5 },
  ], "half-turn symmetry uses each rectangular axis independently");
  assert.deepEqual(symmetricMapTiles({ width: 12, height: 8 }, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.RADIAL), [
    { x: 1, y: 2 },
  ], "quarter-turn symmetry is unavailable when it would change the map shape");
  assert.equal(mapEditorSymmetrySupported({ width: 12, height: 8 }, MAP_EDITOR_SYMMETRY.RADIAL), false);
  assert.deepEqual(mapEditorSymmetryGuideLines({ width: 12, height: 8 }, MAP_EDITOR_SYMMETRY.HORIZONTAL), [
    { x0: 0, y0: 128, x1: 384, y1: 128 },
  ]);
  assert.deepEqual(mapEditorSymmetryGuideCentre({ width: 12, height: 8 }, MAP_EDITOR_SYMMETRY.HALF_TURN), { x: 192, y: 128 });
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
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ width: 32, height: 24, playerCount: 2 });
  session.beginDoodadStroke("Placed tree before resize");
  session.placeDoodads([{ x: 320, y: 320 }], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  assert.equal(session.commitDoodadStroke(), true);
  const before = session.materialized();
  const resized = session.resize({ width: 64, height: 24 });
  assert.deepEqual(resized, { ok: true, count: 1 });
  const map = session.materialized();
  assert.deepEqual({ width: map.width, height: map.height, cells: map.terrain.length }, {
    width: 64, height: 24, cells: 64 * 24,
  });
  assert.deepEqual(map.starts, before.starts.map((start) => ({ x: start.x + 16, y: start.y })),
    "centred horizontal expansion shifts authored locations with the preserved terrain");
  assert.deepEqual(map.doodads.map(({ x, y }) => ({ x, y })), [{ x: 832, y: 320 }],
    "centred rectangular resize shifts doodads with the preserved terrain");
  assert(map.terrain.slice(0, 16).every((code) => code === TERRAIN.GRASS),
    "new side tiles are filled with grass rather than stretching the source terrain");
  assert.equal(session.undo(), true);
  assert.deepEqual(session.materialized(), before, "rectangular resize is one undoable editor operation");
}

{
  const draft = authoredMapFromMaterialized({
    name: "Round trip", description: "", width: 48, height: 32,
    terrain: Array(48 * 32).fill(TERRAIN.GRASS),
    starts: [{ x: 8, y: 8 }, { x: 39, y: 23 }],
    baseSites: [{ x: 8, y: 8 }, { x: 39, y: 23 }, { x: 24, y: 16 }],
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
    destination: "lab",
    authoredMap: { version: 6 },
    materializedMap: { width: 32, height: 16, starts: [], baseSites: [], doodads: [] },
    fetchImpl: async (_url, init) => {
      request.push(JSON.parse(init.body));
      return { ok: true, json: async () => ({ handoffId: "0123456789abcdef0123456789abcdef" }) };
    },
  });
  assert.equal(request[0].selectedLayoutId, undefined, "handoffs carry flat map data only");
}

{
  assert.deepEqual(mapEditorLaunchConfig({ search: "", pathname: "/map-editor" }), {
    handoffId: "",
    error: "",
  });
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
      "tree.oak", "tree.pine", "tree.spruce", "tree.alder",
    ],
    "the tree palette exposes four visual species sharing one authoritative tree/trunk semantic",
  );
  assert.deepEqual(
    MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "neutral-unit").map((entry) => entry.typeId),
    ["unit.tank_trap"],
    "the doodad palette exposes authored neutral Tank Traps",
  );
  assert.equal(canonicalDoodadColor(" #AbC "), "#aabbcc");
  assert.equal(canonicalDoodadColor("not-a-color"), null);
  const doodads = normalizeMapEditorDoodads([
    { id: 7, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 12, y: 20, color: "#ffffff" },
    { id: 7, typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, x: 30, y: 40, color: "#F0A" },
    { id: 0, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 50, y: 60 },
    { id: Number.MAX_SAFE_INTEGER, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE, x: 70, y: 80 },
    { id: 9, typeId: "tree.unknown", x: 1, y: 1 },
    { id: 10, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, x: 128, y: 0 },
    { id: 11, typeId: MAP_EDITOR_DOODAD_TYPES.TANK_TRAP, x: 95, y: 97, color: "#ffffff" },
  ], 128);
  assert.deepEqual(doodads, [
    { typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, x: 30, y: 40, color: "#ff00aa", id: 1 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 50, y: 60, id: 2 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE, x: 70, y: 80, id: 3 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 12, y: 20, id: 7 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TANK_TRAP, x: 80, y: 112, id: 11 },
  ], "normalization repairs duplicate, missing, and non-u32 ids, canonicalizes flowers, strips tree color, and rejects invalid records");

  assert.deepEqual(normalizeMapEditorDoodads([
    { id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 1535, y: 1023 },
    { id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 1536, y: 10 },
    { id: 3, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, x: 10, y: 1024 },
  ], { width: 1536, height: 1024 }), [
    { id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 1535, y: 1023 },
  ], "normalization treats rectangular world width and height as one explicit contract");
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
  assert.deepEqual(symmetricDoodadPlacements(
    { width: 640, height: 320 },
    [{ x: 17, y: 31 }],
    MAP_EDITOR_SYMMETRY.HALF_TURN,
  ), [
    { x: 17, y: 31 }, { x: 622, y: 288 },
  ], "rectangular doodad symmetry reflects each independent world extent");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  assert.equal(session.exportMap().version, 6);
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
  session.placeDoodads([{ x: 200, y: 220 }], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER });
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
      doodadSelections: [{ id: 1, x: 30, y: 40 }],
      doodadSelectionBox: { x: 20, y: 20, width: 30, height: 40 },
      doodadBrushPreview: { x: 40, y: 50, radius: 48, mode: "erase", typeId: null, color: null },
    },
  });
  assert.equal(structuredClone(record).doodadUpdate.doodads[0].id, 1,
    "the revisioned doodad replacement and editor overlay remain structured-cloneable");
  assert.equal(record.version, 2);
  assert.deepEqual(record.layerVisibility, defaultMapAuthoringLayerVisibility(),
    "Map Editor presentation v2 carries complete layer visibility to the Pixi owner");
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
    doodadSelectStart: null,
    doodadSelectEnd: null,
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
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed selection fixtures");
  session.placeDoodads([
    { x: 40, y: 60 },
    { x: 100, y: 120 },
    { x: 180, y: 200 },
  ], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  session.commitDoodadStroke();
  const statuses = [];
  const viewport = {
    session,
    selectedDoodadIds: new Set(),
    doodadSelectStart: { x: 150, y: 150 },
    doodadSelectEnd: { x: 20, y: 40 },
    onStatus(message, error) { statuses.push({ message, error }); },
    drawOverlay() {},
  };
  const selected = MapEditorViewport.prototype.finishDoodadBoxSelection.call(viewport);
  assert.deepEqual(selected, [1, 2], "remove mode selects every doodad inside a drag box in either direction");
  assert.equal(session.doodadStroke, null, "box selection never starts a move/edit transaction");
  const changed = MapEditorViewport.prototype.deleteSelectedDoodads.call(viewport);
  assert.equal(changed, true);
  assert.deepEqual(session.draft.doodads.map((record) => record.id), [3],
    "delete selection removes the whole box-selected group without moving surviving doodads");
  assert.deepEqual(statuses, [
    { message: "2 doodads selected for removal.", error: false },
    { message: "2 doodads deleted.", error: false },
  ]);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed fixtures");
  session.placeDoodads([
    { x: 40, y: 60 },
    { x: 100, y: 120 },
  ], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  session.commitDoodadStroke();
  session.beginDoodadStroke("Active spray");
  session.placeDoodads([{ x: 180, y: 200 }], {
    typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE,
    color: "#ef739d",
  });
  const statuses = [];
  const viewport = {
    session,
    selectedDoodadIds: new Set([1, 2]),
    onStatus(message, error) { statuses.push({ message, error }); },
    drawOverlay() {},
  };
  assert.equal(MapEditorViewport.prototype.deleteSelectedDoodads.call(viewport), false,
    "selection deletion refuses to merge into another active doodad edit");
  assert.deepEqual(session.draft.doodads.map((record) => record.id), [1, 2, 3]);
  assert(session.doodadStroke, "the active edit remains open for its pointer lifecycle to finish");
  assert.deepEqual(statuses, [{
    message: "Finish the current doodad edit before deleting the selection.", error: true,
  }]);
  session.cancelDoodadStroke();
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
    upserts: [{ id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, x: 30, y: 40 }],
    removedIds: [1],
  });
  assert.deepEqual(viewport.pendingDoodadUpdate, {
    kind: "patch", revision: 4,
    upserts: [{ id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, x: 30, y: 40 }],
    removedIds: [1],
  }, "pending doodad patches coalesce by stable id without resurrecting removals");
}

{
  const calls = [];
  const viewport = {
    selectedDoodadIds: new Set(),
    rebuildTerrain() { calls.push("terrain"); },
    rebuildDoodads() { calls.push("doodads"); },
    queueDoodadPatch(update) { calls.push(["patch", update]); },
    drawOverlay() { calls.push("overlay"); },
  };
  const patch = { upserts: [{ id: 1 }], removedIds: [] };
  MapEditorViewport.prototype.applySessionSnapshot.call(viewport, {
    reason: "doodadStroke",
    draft: { doodads: [{ id: 1 }] },
    doodadPatch: patch,
  });
  assert.deepEqual(calls, [["patch", patch], "overlay"],
    "a doodad-only commit patches vegetation without rebuilding the static terrain texture");
}

{
  const calls = [];
  const selectedDoodadIds = new Set([1]);
  const viewport = {
    selectedDoodadIds,
    rebuildTerrain() { calls.push("terrain"); },
    rebuildDoodads() { calls.push("doodads"); },
    drawOverlay() { calls.push("overlay"); },
  };
  MapEditorViewport.prototype.applySessionSnapshot.call(viewport, {
    reason: "loaded",
    draft: { doodads: [{ id: 1 }] },
  });
  assert.equal(viewport.selectedDoodadIds.size, 0,
    "loading a different map clears selection even when it reuses a selected doodad id");
  assert.deepEqual(calls, ["terrain", "doodads", "overlay"]);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed tree");
  const updates = [];
  const viewport = {
    session,
    tool: { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, color: null, symmetry: MAP_EDITOR_SYMMETRY.NONE },
    queueDoodadPatch(update) { updates.push(structuredClone(update)); },
  };
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 64, y: 96 }]);
  assert.deepEqual(updates, [{
    upserts: [{ id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 64, y: 96 }],
  }], "an active doodad stroke streams its compact visual patch before pointer-up");
  assert.equal(session.undoStack.length, 0, "live stroke feedback does not split the undo transaction");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ width: 64, height: 24, playerCount: 1 });
  session.beginDoodadStroke("Placed trees on wide map");
  const added = session.placeDoodads([
    { x: 1800, y: 500 },
    { x: 200, y: 800 },
  ], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  assert.deepEqual(added.map(({ x, y }) => ({ x, y })), [{ x: 1800, y: 500 }],
    "rectangular doodad placement uses width and height independently");
  assert.deepEqual(doodadIdsWithinRect(session.draft.doodads, { x: 100, y: 400 }, { x: 1900, y: 700 }), [added[0].id],
    "box removal selects doodads across the full wide-map axis without moving them");

  const updates = [];
  const viewport = {
    session,
    tool: { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, color: null, symmetry: MAP_EDITOR_SYMMETRY.HALF_TURN },
    queueDoodadPatch(update) { updates.push(structuredClone(update)); },
  };
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 100, y: 200 }]);
  assert.deepEqual(updates[0].upserts.map(({ x, y }) => ({ x, y })), [
    { x: 100, y: 200 }, { x: 1947, y: 567 },
  ], "rectangular half-turn doodad symmetry reflects around each world axis independently");
  session.cancelDoodadStroke();
}
