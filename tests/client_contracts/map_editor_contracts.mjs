import assert from "node:assert/strict";
import fs from "node:fs";

import { installFakePixi } from "./pixi_fakes.mjs";

{
  const savedRaf = globalThis.requestAnimationFrame;
  let scheduled = 0;
  globalThis.requestAnimationFrame = () => ++scheduled;
  try {
    const viewport = {
      destroyed: false,
      lastFrameAt: 0,
      keys: {},
      camera: {
        x: 10,
        y: 20,
        zoom: 2,
        update() {},
      },
      renderer: {
        world: {
          position: { set() {} },
          scale: { set() {} },
        },
        presents: 0,
        present() { this.presents += 1; },
      },
      tick: MapEditorViewport.prototype.tick,
    };
    viewport.tick(16);
    assert.equal(viewport.renderer.presents, 1, "each Map Editor RAF explicitly presents once");
    assert.equal(scheduled, 1, "Map Editor schedules one successor RAF");
  } finally {
    globalThis.requestAnimationFrame = savedRaf;
  }
}
import { TERRAIN } from "../../client/src/protocol.js";
import { createMapHandoff } from "../../client/src/map_editor_handoff.js";
import { mapEditorLaunchConfig } from "../../client/src/map_editor_launch.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
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
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_MAX_BASE_SITES,
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
      renderer: {
        app: { view: { removeEventListener() {} } },
        destroyed: 0,
        destroy() { this.destroyed += 1; },
      },
      labels: [],
      overlay: { destroy() {} },
    };
    MapEditorViewport.prototype.destroy.call(viewport);
    MapEditorViewport.prototype.destroy.call(viewport);
    assert.deepEqual(cancelled, [73], "Map Editor teardown cancels its one owned RAF exactly once");
    assert.equal(viewport.renderer.destroyed, 1, "Map Editor teardown destroys Pixi idempotently");
  } finally {
    globalThis.cancelAnimationFrame = savedCancel;
    globalThis.window = savedWindow;
  }
}

const repoRoot = new URL("../../", import.meta.url);
const noTerrainMap = JSON.parse(fs.readFileSync(new URL("server/assets/maps/no-terrain.json", repoRoot), "utf8"));
const serverMapSource = fs.readFileSync(new URL("server/crates/sim/src/game/map.rs", repoRoot), "utf8");

{
  const serverMainRadius = Number(serverMapSource.match(/BASE_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  const serverBaseRadius = Number(serverMapSource.match(/BASE_SITE_PROTECTION_RADIUS_TILES:\s*i32\s*=\s*(\d+)/)?.[1]);
  assert.equal(MAP_EDITOR_MAIN_CLEARANCE_TILES, serverMainRadius);
  assert.equal(MAP_EDITOR_BASE_SITE_CLEARANCE_TILES, serverBaseRadius);
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(noTerrainMap);
  const materialized = session.materialized();
  assert.equal(session.exportMap().version, 3);
  assert.equal(session.exportMap().layouts, undefined, "flat map data has no layout matrix");
  assert.equal(materialized.starts.length, 4);
  assert.equal(materialized.baseSites.length, 8, "every authored base is materialized without choosing a player layout");
  assert(materialized.baseSites.some((site) => site.x === 25 && site.y === 25), "start locations are permanent base sites");
  assert.deepEqual(
    session.mapOverlay().bases.map((site) => site.index),
    [4, 5, 6, 7],
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
  assert.deepEqual(session.draft.baseSites, [{ x: 8, y: 8 }], "removing a start keeps its resource site as a neutral base");
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
  assert.deepEqual(session.draft.baseSites, session.draft.startLocations,
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
  assert.deepEqual(result, { ok: true, count: 1, removed: 1 });
  assert.deepEqual(draft.baseSites, [{ x: 8, y: 8 }, { x: 10, y: 12 }],
    "a symmetric base move removes its matching neutral base instead of relocating it");
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
  assert.equal(viewport.selectedBaseIndex, 1,
    "removing an earlier symmetric base keeps the moved base selected by its new backing index");
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

  const restorePixi = installFakePixi();
  try {
    const overlay = {
      calls: [],
      lineStyle(...args) { this.calls.push(["lineStyle", ...args]); return this; },
      drawCircle(...args) { this.calls.push(["drawCircle", ...args]); return this; },
      beginFill(...args) { this.calls.push(["beginFill", ...args]); return this; },
      endFill(...args) { this.calls.push(["endFill", ...args]); return this; },
    };
    const feedback = new PIXI.Container();
    const siteViewport = { overlay, labels: [], renderer: { layers: { feedback } } };
    MapEditorViewport.prototype.drawSite.call(siteViewport, { x: 10, y: 12 }, 0xf4c542, 7, "B1", true);
    assert(overlay.calls.some((call) => call[0] === "drawCircle" && call[3] === 13),
      "the selected base gets a larger map highlight ring");
  } finally {
    restorePixi();
  }
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
  assert.equal(session.exportMap().version, 3, "local v2 maps migrate into flat map data");
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
  assert.equal(session.loadLocal("legacy-workspace"), true, "v3 sessions recover saved v2 workspaces");
  assert.equal(session.exportMap().version, 3);
  assert.equal(session.materialized().baseSites.length, 2);
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
    kind: "start", tile: { x: 63, y: 30 }, symmetry: MAP_EDITOR_SYMMETRY.THREE_WAY,
  });
  assert.deepEqual(result, { ok: true, count: 3 });
  assert.deepEqual(draft.startLocations, [{ x: 63, y: 30 }, { x: 90, y: 79 }, { x: 34, y: 78 }]);
  assert.deepEqual(draft.baseSites, draft.startLocations,
    "three-way placement creates a matching base for each 1v1v1 start");

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
  assert.deepEqual(session.draft.baseSites, session.draft.startLocations);
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
  assert.deepEqual(draft.baseSites, draft.startLocations);
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
    destination: "lab", authoredMap: { version: 3 }, materializedMap: { starts: [], baseSites: [] },
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
