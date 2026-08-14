import assert from "node:assert/strict";
import fs from "node:fs";

import {
  boundedMapEditorElevationLevel,
  selectMapEditorElevationLevel,
} from "../../client/src/map_editor_elevation_controls.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { mapEditorContentLabel } from "../../client/src/map_editor_panel_workflow.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";
import { MAP_EDITOR_DOODAD_TYPES } from "../../client/src/map_editor_doodads.js";
import { TERRAIN } from "../../client/src/protocol.js";
import {
  MAP_EDITOR_MAX_BASE_SITES,
  MAP_EDITOR_MAX_START_LOCATIONS,
} from "../../client/src/map_editor_session.js";

const shellStyles = fs.readFileSync(new URL("../../client/map_editor_shell.css", import.meta.url), "utf8");
const elevationSource = fs.readFileSync(new URL("../../client/src/map_editor_elevation_controls.js", import.meta.url), "utf8");
const panelSource = fs.readFileSync(new URL("../../client/src/map_editor_panel.js", import.meta.url), "utf8");
const terrainControlsSource = fs.readFileSync(new URL("../../client/src/map_editor_terrain_controls.js", import.meta.url), "utf8");
const workflowSource = fs.readFileSync(new URL("../../client/src/map_editor_panel_workflow.js", import.meta.url), "utf8");

for (const label of ["Textures", "Elevation", "Objects", "Zones", "Locations"]) {
  assert.match(workflowSource, new RegExp(`\\["[a-z]+", "${label}"\\]`), `${label} remains a first-class palette category`);
}
assert.match(shellStyles, /\.map-editor-tools-window \.map-editor-panel-body\s*\{[^}]*grid-template-rows:\s*auto minmax\(0, 1fr\) auto/s,
  "palette tabs and current-tool context stay pinned around the one scrolling content region");
assert.match(shellStyles, /\.map-editor-tool-rail\s*\{[^}]*position:\s*absolute/s,
  "editing operations live in a separate map-space rail");
assert.match(elevationSource, /range\.addEventListener\("input", \(\) => \{ number\.value = range\.value; \}\)/,
  "dragging the elevation slider synchronizes its numeric field without replacing the active range input through a panel render");
assert.match(elevationSource, /range\.addEventListener\("change", \(\) => select\(range\.value\)\)/,
  "releasing the elevation slider applies the synchronized paint level");
assert.match(terrainControlsSource, /button\("Erase", \(\) => panel\.selectSemanticFeatureErase\(\)/,
  "the semantic feature palette exposes a dedicated Erase tile");
assert.match(terrainControlsSource, /eraseIcon\.className = "map-editor-erase-icon"/,
  "the semantic Erase tile carries a recognizable eraser icon");
assert.match(terrainControlsSource, /featurePalette\.insertBefore\(eraseControl, featurePalette\.children\[2\]/,
  "Erase stays immediately after Stone and Water instead of falling below the road variants");

{
  const operations = [];
  const panel = {
    terrainContent: "material",
    selectedTerrain: TERRAIN.WATER,
    selectedTerrainLayer: "feature",
    lastOperation: { terrain: "erase" },
    selectOperation(operation) { operations.push(operation); this.lastOperation.terrain = operation; },
  };
  MapEditorPanel.prototype.selectTerrainMaterial.call(panel, TERRAIN.ROCK, "feature");
  assert.equal(panel.selectedTerrain, TERRAIN.ROCK);
  assert.deepEqual(operations, ["brush"],
    "choosing a semantic material exits the Erase tile and resumes painting");
  MapEditorPanel.prototype.selectSemanticFeatureErase.call(panel);
  assert.equal(panel.selectedTerrainLayer, "feature");
  assert.deepEqual(operations, ["brush", "erase"],
    "the semantic Erase tile arms the same operation as the Apply rail");
}

{
  const panel = {
    activeCategory: "terrain",
    terrainContent: "material",
    selectedTerrainLayer: "feature",
    lastOperation: { terrain: "erase" },
    viewport: { tool: { kind: "terrain", terrain: TERRAIN.GRASS, eraseFeature: true } },
  };
  assert.equal(MapEditorPanel.prototype.semanticFeatureEraseSelected.call(panel), true,
    "Apply > Erase selects the semantic Erase tile through shared active-operation state");
}

{
  const panel = {
    activeCategory: "objects",
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE,
  };
  assert.deepEqual(
    [...MapEditorPanel.prototype.availableOperations.call(panel)],
    ["place", "spray", "erase"],
    "wildflower content admits place, spray, and erase",
  );
  panel.selectedDoodadType = MAP_EDITOR_DOODAD_TYPES.TANK_TRAP;
  assert.deepEqual(
    [...MapEditorPanel.prototype.availableOperations.call(panel)],
    ["place", "erase"],
    "non-sprayable gameplay objects visibly disable spray without losing placement or erase",
  );
}

{
  const panel = {
    activeCategory: "objects",
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.TANK_TRAP,
    viewport: { tool: { kind: "doodad", mode: "erase" } },
    lastOperation: { objects: "erase" },
  };
  assert.equal(mapEditorContentLabel(panel, (value) => value, (value) => value), "All objects",
    "object erase truthfully describes its global radius instead of implying type-filtered deletion");
  panel.activeCategory = "terrain";
  panel.terrainContent = "material";
  panel.selectedTerrain = TERRAIN.WATER;
  panel.selectedTerrainLayer = "ground";
  panel.viewport.tool = { kind: "terrain", terrain: TERRAIN.GRASS, shape: "brush" };
  panel.lastOperation = { terrain: "erase" };
  assert.equal(mapEditorContentLabel(panel, (value) => value, (value) => value), "Ground to grass",
    "ground erase describes the applied grass result instead of the retained material selection");
  panel.selectedTerrainLayer = "feature";
  panel.viewport.tool = { kind: "terrain", terrain: TERRAIN.GRASS, eraseFeature: true, shape: "brush" };
  assert.equal(mapEditorContentLabel(panel, (value) => value, (value) => value), "Remove terrain features",
    "feature erase describes removing semantic terrain instead of repainting ground");
  panel.activeCategory = "elevation";
  panel.terrainContent = "elevation";
  panel.selectedElevation = 6;
  panel.viewport.tool = { kind: "elevation", level: 6, shape: "brush" };
  panel.lastOperation = { terrain: "brush" };
  assert.equal(mapEditorContentLabel(panel, (value) => value, (value) => value), "Elevation level 6",
    "elevation paint describes the selected height instead of the stale terrain material");
  panel.viewport.tool = { kind: "elevation", level: 0, shape: "brush" };
  panel.lastOperation.terrain = "erase";
  assert.equal(mapEditorContentLabel(panel, (value) => value, (value) => value), "Elevation level 0",
    "elevation erase describes the resulting height instead of terrain-to-grass");
}

{
  assert.equal(boundedMapEditorElevationLevel(-4), 0);
  assert.equal(boundedMapEditorElevationLevel(6.9), 6);
  assert.equal(boundedMapEditorElevationLevel(99), 9);
  const operations = [];
  const panel = {
    terrainContent: "elevation",
    selectedElevation: 0,
    lastOperation: { terrain: "erase" },
    selectOperation(operation) { operations.push(operation); this.lastOperation.terrain = operation; },
  };
  selectMapEditorElevationLevel(panel, 5);
  assert.equal(panel.selectedElevation, 5);
  assert.deepEqual(operations, ["brush"],
    "choosing a positive elevation exits level-zero erase mode and arms the chosen height");
  panel.lastOperation.terrain = "box";
  selectMapEditorElevationLevel(panel, 7);
  assert.deepEqual(operations, ["brush", "box"], "positive elevation changes preserve box paint mode");
}

{
  const calls = [];
  const panel = {
    activeCategory: "terrain",
    terrainContent: "material",
    lastOperation: { terrain: "box" },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    selectOperation(operation) { calls.push(operation); },
  };
  MapEditorPanel.prototype.selectCategory.call(panel, "elevation");
  assert.equal(panel.activeCategory, "elevation");
  assert.equal(panel.terrainContent, "elevation");
  assert.deepEqual(calls, ["box"], "Elevation is an independent palette category with remembered paint shape");
  MapEditorPanel.prototype.selectCategory.call(panel, "terrain");
  assert.equal(panel.terrainContent, "material", "Textures do not retain the elevation tool mode");
}

{
  const calls = [];
  const statuses = [];
  const panel = {
    activeCategory: "terrain",
    terrainContent: "material",
    selectedTerrain: TERRAIN.WATER,
    paintShape: "box",
    lastOperation: { terrain: "box" },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    operationHelp: MapEditorPanel.prototype.operationHelp,
    armTerrain(terrain) { calls.push({ terrain, shape: this.paintShape }); },
    setStatus(message) { statuses.push(message); },
  };
  assert.equal(MapEditorPanel.prototype.selectOperation.call(panel, "erase"), true);
  assert.deepEqual(calls, [{ terrain: TERRAIN.GRASS, shape: "brush" }],
    "terrain erase uses the grass material while preserving the separately selected water content");
  assert.equal(panel.selectedTerrain, TERRAIN.WATER);
  assert.equal(panel.lastOperation.terrain, "erase");
  assert.deepEqual(statuses, ["Remove editable content under the eraser."],
    "the persistent status dock describes the newly armed terrain operation");
}

{
  const armed = [];
  const panel = {
    selectedTerrain: TERRAIN.ROCK,
    paintShape: "brush",
    terrainBrushWidth: 7,
    symmetry: "none",
    viewport: { armTool(tool) { armed.push(tool); } },
  };
  MapEditorPanel.prototype.armTerrain.call(panel);
  assert.deepEqual(armed, [{
    kind: "terrain",
    terrain: TERRAIN.ROCK,
    shape: "brush",
    width: 7,
    symmetry: "none",
  }], "terrain tools carry the configured brush width into viewport input");
}

{
  const strokes = [];
  const viewport = {
    tool: { kind: "terrain", width: 5 },
    session: { draft: { width: 20, height: 20 } },
    paintTiles(tiles) { strokes.push(tiles); },
  };
  MapEditorViewport.prototype.paintLine.call(viewport, { x: 10, y: 10 }, { x: 10, y: 10 });
  assert.equal(strokes[0].length, 21, "a five-tile terrain brush paints a five-tile-wide circular footprint");
  assert(strokes[0].some(({ x, y }) => x === 8 && y === 10));
  assert(strokes[0].some(({ x, y }) => x === 12 && y === 10));

  viewport.tool.width = 1;
  MapEditorViewport.prototype.paintLine.call(viewport, { x: 10, y: 10 }, { x: 10, y: 10 });
  assert.deepEqual(strokes[1], [{ x: 10, y: 10 }], "the default one-tile brush preserves precise terrain painting");
}

{
  const statuses = [];
  const panel = {
    activeCategory: "objects",
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
    lastOperation: { objects: "place" },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    operationHelp: MapEditorPanel.prototype.operationHelp,
    armDoodad() {},
    setStatus(message) { statuses.push(message); },
  };
  assert.equal(MapEditorPanel.prototype.selectOperation.call(panel, "erase"), true);
  assert.deepEqual(statuses, ["Remove editable content under the eraser."],
    "switching the object rail to Erase cannot leave a stale Placing instruction");
}

{
  const calls = [];
  const panel = {
    activeCategory: "terrain",
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE,
    lastOperation: { objects: "spray" },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    selectOperation(operation) { calls.push(operation); },
  };
  MapEditorPanel.prototype.selectCategory.call(panel, "objects");
  assert.equal(panel.activeCategory, "objects");
  assert.deepEqual(calls, ["spray"],
    "switching categories immediately arms the remembered compatible operation instead of merely relabeling the old tool");
}

{
  const calls = [];
  const panel = {
    activeCategory: "zones",
    selectedOverlayEffects: new Set(["concealment"]),
    paintShape: "brush",
    lastOperation: { zones: "brush" },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    armSelectedOverlays(mode) { calls.push({ mode, shape: this.paintShape }); },
  };
  assert.equal(MapEditorPanel.prototype.selectOperation.call(panel, "box"), true);
  assert.deepEqual(calls, [{ mode: "paint", shape: "box" }],
    "zone content can switch to box application without changing the selected zone effects");
}

{
  let cleared = 0;
  let rendered = 0;
  const panel = {
    activeCategory: "terrain",
    selectedOverlayEffects: new Set(),
    lastOperation: { zones: "brush" },
    viewport: { armTool(tool) { assert.equal(tool, null); cleared += 1; } },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    render() { rendered += 1; },
  };
  MapEditorPanel.prototype.selectCategory.call(panel, "zones");
  assert.deepEqual([...panel.availableOperations()], [], "zone operations disable when no effect is selected");
  assert.equal(cleared, 1, "entering an empty Zones palette clears the previously armed category tool");
  assert.equal(rendered, 1);
}

{
  const fullBases = Array.from({ length: MAP_EDITOR_MAX_BASE_SITES }, (_, index) => ({ x: index, y: 0 }));
  const panel = {
    activeCategory: "locations",
    session: {
      draft: { startLocations: [{}], baseSites: fullBases },
      mapOverlay() { return { bases: fullBases.slice(1) }; },
    },
  };
  panel.locationContent = "base";
  assert.deepEqual([...MapEditorPanel.prototype.availableOperations.call(panel)], ["move", "remove"],
    "neutral-base Add disables at the total authored base-site cap");
  panel.locationContent = "start";
  assert(MapEditorPanel.prototype.availableOperations.call(panel).has("add"),
    "start Add remains available at the base-site cap so an existing neutral base can be promoted");
}

{
  const fullStarts = Array.from({ length: MAP_EDITOR_MAX_START_LOCATIONS }, (_, index) => ({ x: index, y: 0 }));
  const panel = {
    activeCategory: "locations",
    locationContent: "start",
    viewport: {
      tool: { kind: "start", add: true },
      armTool(tool) { this.tool = tool; },
    },
    session: {
      draft: { startLocations: fullStarts, baseSites: fullStarts },
      mapOverlay() { return { bases: [] }; },
    },
  };
  assert.equal(MapEditorPanel.prototype.reconcileOperationAvailability.call(panel), false,
    "a persistent location Add tool is disarmed when a session update consumes the last slot");
  assert.equal(panel.viewport.tool, null,
    "the viewport cannot keep applying an operation that the rail has disabled");
}

{
  const panel = {
    activeCategory: "objects",
    viewport: { tool: null },
    lastOperation: { objects: "spray" },
  };
  assert.equal(MapEditorPanel.prototype.activeOperation.call(panel), null,
    "a reset or completed destructive action never presents a stale remembered operation as armed");
}

{
  const removed = [];
  const panel = {
    activeCategory: "locations",
    locationContent: "base",
    selectedBaseIndex: 0,
    lastOperation: { locations: "move" },
    session: {
      draft: { startLocations: [], baseSites: [{}] },
      mapOverlay() { return { bases: [{ index: 3 }] }; },
    },
    availableOperations: MapEditorPanel.prototype.availableOperations,
    removeLocation(kind, index) { removed.push({ kind, index }); },
  };
  assert.equal(MapEditorPanel.prototype.selectOperation.call(panel, "remove"), true);
  assert.deepEqual(removed, [{ kind: "base", index: 3 }],
    "location removal resolves the selected authored base index through the reorganized rail");
}

console.log("✅ map_editor_panel_workflow_contracts.mjs: palette/operation workflow contracts passed");
