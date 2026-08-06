import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import {
  buildMapFromRecipe,
  renderPreviewSvg,
  runAuthoritativeMapTool,
  runCli,
  validateMap,
} from "../scripts/map-author.mjs";
import { expandSymmetricPoints } from "../client/src/map_authoring/symmetry.js";
import { MAP_AUTHORING_LAYER } from "../client/src/map_authoring/layers.js";
import { MapEditorSession, mapEditorRectTiles, MAP_EDITOR_SYMMETRY, symmetricTerrainTiles } from "../client/src/map_editor_session.js";
import { TERRAIN } from "../client/src/protocol.js";

const repoRoot = new URL("../", import.meta.url);
const serverMapSource = fs.readFileSync(new URL("server/crates/sim/src/game/map.rs", repoRoot), "utf8");
const currentServerMapVersion = Number(serverMapSource.match(/CURRENT_MAP_VERSION:\s*u32\s*=\s*(\d+)/)?.[1]);
assert(Number.isSafeInteger(currentServerMapVersion), "test reads the authoritative server map version");

const recipe = {
  name: "CLI Contract",
  width: 32,
  height: 32,
  symmetry: "halfTurn",
  operations: [
    { type: "blob", material: "water", center: [3, 3], radius: [4, 3], roughness: 0.3, seed: 7 },
    // Off-angle roads are intentionally accepted. Their desirability belongs to the author.
    { type: "road", points: [[2, 8], [17, 13]], width: 3 },
    { type: "start", at: [3, 3] },
  ],
};

const map = buildMapFromRecipe(recipe);
assert.equal(map.version, currentServerMapVersion);
assert.equal(map.width, 32);
assert.equal(map.height, 32);
assert.equal(map.startLocations.length, 2);
assert.equal(map.baseSites.length, 2);
assert.deepEqual(map.concealmentTiles, []);
assert.deepEqual(map.noVehicleTiles, []);
assert.deepEqual(map.noBuildingTiles, []);
assert(map.noEntrenchmentTiles.length > 0, "road operations author no-entrenchment tiles");
for (const tile of map.noEntrenchmentTiles) {
  assert("=-|\\/".includes(map.terrain[tile.y][tile.x]), "automatic no-entrenchment tiles follow roads");
}
assert.deepEqual(map.damageReductionTiles, []);
assert.deepEqual(map.slowMovementTiles, []);
for (let y = 0; y < map.height; y += 1) {
  for (let x = 0; x < map.width; x += 1) {
    assert.equal(map.terrain[y][x], map.terrain[map.height - 1 - y][map.width - 1 - x]);
  }
}

const validation = validateMap(map, { symmetry: "halfTurn" });
assert(validation.warnings.some((warning) => warning.includes("protected area")), "blocked base clearance is advisory");
assert(!validation.warnings.some((warning) => warning.includes("unsupported field \"forestSpans\"")),
  "the CLI validator accepts the current schema's compact Forest field");
assert(!validation.warnings.some((warning) => warning.includes("symmetry mismatches")), "generated terrain preserves symmetry");

for (const [forestSpans, expected] of [
  [undefined, "must be an array"],
  [[[1, 2]], "must be [y, xStart, xEnd]"],
  [[[1, 4, 3]], "outside the map or has reversed x bounds"],
  [[[1, 1, 2], [1, 2, 3]], "overlaps another span at (2,1)"],
  [[[map.height, 1, 1]], "outside the map or has reversed x bounds"],
]) {
  const warnings = validateMap({ ...map, forestSpans }).warnings;
  assert(warnings.some((warning) => warning.includes(expected)),
    `the CLI validator rejects malformed Forest spans: ${expected}`);
}

assert(validateMap({ ...map, noBuildingTiles: undefined }).warnings
  .some((warning) => warning.includes("noBuildingTiles must be an array")),
"the CLI validator requires the current schema's no-building layer");

assert(validateMap({ ...map, noEntrenchmentTiles: undefined }).warnings
  .some((warning) => warning.includes("noEntrenchmentTiles must be an array")),
"the CLI validator requires the current schema's no-entrenchment layer");

const protectedRecipe = {
  name: "Advisory protected terrain",
  width: 32,
  height: 32,
  operations: [
    { type: "start", at: [12, 12] },
    { type: "blob", material: "water", center: [12, 12], radius: 1, roughness: 0 },
  ],
};
const protectedMap = buildMapFromRecipe(protectedRecipe);
const importedProtected = new MapEditorSession({ storage: null });
importedProtected.loadAuthoredMap(protectedMap);
assert.deepEqual(
  importedProtected.exportMap(),
  protectedMap,
  "materialized recipe output preserves impassable protected terrain through authored-map import",
);
assert(validateMap(protectedMap).warnings.some((warning) => warning.includes("protected area")));

const operationlessRecipe = { name: "Operationless recipe", width: 16, height: 18 };
assert.deepEqual(buildMapFromRecipe(operationlessRecipe).terrain, Array(18).fill(".".repeat(16)));

const roadBackground = buildMapFromRecipe({ ...operationlessRecipe, background: "road-bare" });
assert.equal(roadBackground.noEntrenchmentTiles.length, roadBackground.width * roadBackground.height,
  "road backgrounds automatically author no-entrenchment on every tile");
const filledRoad = buildMapFromRecipe({
  ...operationlessRecipe,
  operations: [{ type: "fill", material: "road-horizontal" }],
});
assert.equal(filledRoad.noEntrenchmentTiles.length, filledRoad.width * filledRoad.height,
  "road fill operations automatically author no-entrenchment on every tile");

const parityRecipe = {
  name: "Shared operation parity",
  width: 32,
  height: 32,
  symmetry: "halfTurn",
  operations: [{ type: "rect", material: "0", from: [3, 5], to: [5, 7] }],
};
const recipeTerrain = buildMapFromRecipe(parityRecipe).terrain;
const editorSession = new MapEditorSession({ storage: null });
editorSession.initializeBlank({ size: 32, playerCount: 2 });
editorSession.beginTerrainStroke("Parity paint");
const editorTiles = symmetricTerrainTiles(
  editorSession.draft,
  mapEditorRectTiles({ x: 3, y: 5 }, { x: 5, y: 7 }, editorSession.draft),
  TERRAIN.GRAVEL_A,
  MAP_EDITOR_SYMMETRY.HALF_TURN,
);
editorSession.paintTerrainTiles(editorTiles, TERRAIN.GRAVEL_A);
editorSession.commitTerrainStroke();
assert.deepEqual(
  editorSession.exportMap().terrain,
  recipeTerrain,
  "UI tile gestures and recipe shapes materialize through the same symmetry and mutation engine",
);

const radial = buildMapFromRecipe({
  name: "Shared symmetry vocabulary",
  width: 16,
  height: 16,
  symmetry: "radial",
  operations: [{ type: "rect", material: "water", from: [2, 3], to: [2, 3] }],
});
for (const [x, y] of [[2, 3], [12, 2], [13, 12], [3, 13]]) assert.equal(radial.terrain[y][x], "~");
assert(!validateMap(radial, { symmetry: "radial" }).warnings.some((warning) => warning.includes("symmetry")));
for (const symmetry of ["horizontal", "vertical", "halfTurn", "threeWay", "radial", "diagonalMain", "diagonalAnti"]) {
  const generated = buildMapFromRecipe({
    name: `${symmetry} terrain`, width: 32, height: 32, symmetry,
    operations: [{ type: "rect", material: "water", from: [4, 7], to: [6, 9] }],
  });
  assert(!validateMap(generated, { symmetry }).warnings.some((warning) => warning.includes("symmetry")),
    `${symmetry} recipe output passes its shared symmetry check`);
}

const clippedThreeWay = buildMapFromRecipe({
  name: "Clipped three-way terrain",
  width: 32,
  height: 32,
  symmetry: "threeWay",
  operations: [{ type: "rect", material: "water", from: [8, 1], to: [8, 1] }],
});
assert(!validateMap(clippedThreeWay, { symmetry: "threeWay" }).warnings.some((warning) => warning.includes("symmetry")),
  "three-way checks recognize the exact rounded, edge-clipped orbit generated from [8,1]");

const clippedDoodadOrbit = expandSymmetricPoints(
  { width: 512, height: 512 },
  [{ x: 186, y: 0 }],
  "threeWay",
).map((point, index) => ({ id: index + 1, typeId: "tree.oak", ...point }));
const clippedDoodadMap = {
  version: currentServerMapVersion,
  name: "Clipped three-way doodad",
  description: "",
  _design: "symmetry fixture",
  width: 16,
  height: 16,
  terrain: Array(16).fill(".".repeat(16)),
  startLocations: [],
  baseSites: [],
  doodads: clippedDoodadOrbit,
  concealmentTiles: [],
  noVehicleTiles: [],
};
assert(!validateMap(clippedDoodadMap, { symmetry: "threeWay" }).warnings.some((warning) => warning.includes("doodads")),
  "three-way checks recognize the exact rounded, edge-clipped 512 px doodad orbit from {186,0}");

const wrongRoadOrientation = buildMapFromRecipe({
  name: "Wrong road orientation",
  width: 32,
  height: 32,
  symmetry: "horizontal",
  operations: [{ type: "rect", material: "road-diagonal-nw-se", from: [3, 4], to: [3, 4] }],
});
wrongRoadOrientation.terrain[27] = `${".".repeat(3)}\\${".".repeat(28)}`;
assert(validateMap(wrongRoadOrientation, { symmetry: "horizontal" }).warnings
  .some((warning) => warning.includes("terrain") && warning.includes("symmetry")),
"symmetry checks require the correctly transformed marked-road orientation");

const allHorizontalRoad = {
  ...clippedDoodadMap,
  name: "Wrong diagonal road field",
  width: 16,
  height: 16,
  terrain: Array(16).fill("-".repeat(16)),
  doodads: [],
};
assert(validateMap(allHorizontalRoad, { symmetry: "diagonalMain" }).warnings
  .some((warning) => warning.includes("terrain") && warning.includes("symmetry")),
"a dominant marked-road character is still checked when symmetry must rotate its orientation");

const radialLocations = expandSymmetricPoints({ width: 32, height: 32 }, [{ x: 10, y: 11 }], "radial");
const radialDoodads = expandSymmetricPoints({ width: 1024, height: 1024 }, [{ x: 100, y: 120 }], "radial")
  .map((point, index) => ({ id: index + 1, typeId: "tree.oak", ...point }));
const layeredRadial = {
  version: currentServerMapVersion,
  name: "Layered radial",
  description: "",
  _design: "symmetry fixture",
  width: 32,
  height: 32,
  terrain: Array(32).fill(".".repeat(32)),
  startLocations: [...radialLocations],
  baseSites: radialLocations.map((point) => ({ ...point, steelPatches: 4, oilPatches: 1 })),
  doodads: radialDoodads,
  concealmentTiles: [...radialLocations],
  noVehicleTiles: [...radialLocations],
  noBuildingTiles: [...radialLocations],
  noEntrenchmentTiles: [...radialLocations],
};
assert(!validateMap(layeredRadial, { symmetry: "radial" }).warnings.some((warning) => warning.includes("symmetry")));
layeredRadial.terrain[10] = `${".".repeat(9)}#${".".repeat(22)}`;
layeredRadial.baseSites[0].steelPatches = 5;
layeredRadial.startLocations.pop();
layeredRadial.concealmentTiles.pop();
layeredRadial.noVehicleTiles.pop();
layeredRadial.noBuildingTiles.pop();
layeredRadial.noEntrenchmentTiles.pop();
layeredRadial.doodads.pop();
const layeredWarnings = validateMap(layeredRadial, { symmetry: "radial" }).warnings;
for (const layer of ["terrain", "start locations", "base locations", "concealment tiles", "no-vehicle tiles", "no-building tiles", "no-entrenchment tiles", "doodads"]) {
  assert(layeredWarnings.some((warning) => warning.includes(layer)), `${layer} symmetry is checked`);
}

const preview = renderPreviewSvg(map, { tilePixels: 3 });
assert(preview.startsWith("<svg"));
assert(preview.includes(">1</text>"));

const layeredPreviewMap = {
  ...map,
  forestSpans: [[7, 7, 7]],
  concealmentTiles: [{ x: 1, y: 2 }],
  noVehicleTiles: [{ x: 3, y: 4 }],
  noBuildingTiles: [{ x: 5, y: 6 }],
  doodads: [
    { id: 1, typeId: "tree.oak", x: 64, y: 64 },
    { id: 2, typeId: "unit.tank_trap", x: 80, y: 80 },
    { id: 3, typeId: "wildflower.single", x: 96, y: 96, color: "#c58af9" },
  ],
};
const layeredPreview = renderPreviewSvg(layeredPreviewMap);
for (const layer of Object.values(MAP_AUTHORING_LAYER)) {
  assert(layeredPreview.includes(`data-layer="${layer}"`), `default CLI preview includes ${layer}`);
}
const concealmentPreview = renderPreviewSvg(layeredPreviewMap, { layers: MAP_AUTHORING_LAYER.CONCEALMENT });
assert(concealmentPreview.includes(`data-layer="${MAP_AUTHORING_LAYER.CONCEALMENT}"`));
assert(!concealmentPreview.includes(`data-layer="${MAP_AUTHORING_LAYER.BASE}"`));
assert(!concealmentPreview.includes(`data-layer="${MAP_AUTHORING_LAYER.TREES}"`));
const forestPreview = renderPreviewSvg(layeredPreviewMap, { layers: MAP_AUTHORING_LAYER.FOREST });
assert(forestPreview.includes(`data-layer="${MAP_AUTHORING_LAYER.FOREST}"`));
assert(!forestPreview.includes(`data-layer="${MAP_AUTHORING_LAYER.CONCEALMENT}"`),
  "the CLI previews Forest independently from its materialized gameplay effects");
assert.throws(() => renderPreviewSvg(layeredPreviewMap, { layers: "forest-effect" }), /Unsupported map authoring layer/,
  "the CLI rejects unknown combined-layer aliases");

const malformedValidation = validateMap({
  version: currentServerMapVersion,
  width: 20,
  height: 20,
  terrain: {},
  startLocations: [null],
  baseSites: [{ x: 8, y: 8, steelPatches: -1, oilPatches: 10 }],
  doodads: null,
  concealmentTiles: [{ x: "1", y: 2 }],
  unsupportedRootField: true,
});
assert(malformedValidation.warnings.some((warning) => warning.includes("terrain has")));
assert(malformedValidation.warnings.some((warning) => warning.includes("start location 0")));
assert(malformedValidation.warnings.some((warning) => warning.includes("steelPatches")));
assert(malformedValidation.warnings.some((warning) => warning.includes("oilPatches")));
assert(malformedValidation.warnings.some((warning) => warning.includes("doodads must be an array")));
assert(malformedValidation.warnings.some((warning) => warning.includes("concealmentTiles[0]")));
assert(malformedValidation.warnings.some((warning) => warning.includes("unsupportedRootField")));
assert.deepEqual(validateMap(null).warnings, ["map must be a JSON object"]);

const injectedPreview = renderPreviewSvg({
  name: "Preview safety",
  width: 2,
  height: 2,
  terrain: ["..", ".."],
  startLocations: [],
  baseSites: [{ x: '0\" onmouseover=\"alert(1)', y: 0 }],
  concealmentTiles: {},
  noVehicleTiles: null,
  doodads: {},
});
assert(!injectedPreview.includes("onmouseover"), "preview omits non-numeric site coordinates");
assert(injectedPreview.includes('data-layer="concealment"'),
  "preview tolerates advisory-invalid optional layer collections");
const unsupportedDoodadPreview = renderPreviewSvg({
  name: "Unsupported doodad safety",
  width: 2,
  height: 2,
  terrain: ["..", ".."],
  startLocations: [],
  baseSites: [],
  doodads: [{ id: 1, typeId: "unsupported.doodad", x: 16, y: 16 }],
});
assert(!unsupportedDoodadPreview.includes("<path"),
  "preview omits doodads outside the server-backed authoring catalog");
assert.throws(
  () => buildMapFromRecipe({ width: 257, height: 1 }),
  /at most 256 tiles/,
  "recipe dimensions are bounded to the server-supported maximum",
);
assert.throws(
  () => buildMapFromRecipe({ width: 12.5, height: 1 }),
  /positive integers/,
  "recipe dimensions are not silently truncated",
);
assert.throws(() => buildMapFromRecipe({ width: "12", height: 1 }), /positive integers/);
assert.throws(
  () => buildMapFromRecipe({ width: 15, height: 16 }),
  /at least 16 tiles/,
  "recipe dimensions share the editor's minimum",
);
const boundedResources = buildMapFromRecipe({
  width: 16,
  height: 16,
  operations: [{ type: "base", at: [8, 8], steelPatches: 100, oilPatches: -2 }],
});
assert.deepEqual(boundedResources.baseSites, [{ x: 8, y: 8, steelPatches: 36, oilPatches: 0 }],
  "recipe resource counts use the authored-map canonical bounds");
const generatedMapSession = new MapEditorSession({ storage: null });
generatedMapSession.loadAuthoredMap(boundedResources);
assert.deepEqual(generatedMapSession.exportMap(), boundedResources,
  "canonical recipe output is byte-for-byte stable as a materialized map import");
assert.throws(
  () => buildMapFromRecipe({
    width: 32,
    height: 32,
    operations: Array.from({ length: 5 }, (_, index) => ({ type: "start", at: [8 + index * 3, 8] })),
  }),
  /more than 4 start locations/,
  "the shared engine rejects a fifth start before either adapter can silently truncate it",
);
const pathologicalPathRecipe = {
  name: "Compact pathological path",
  width: 256,
  height: 256,
  operations: [{
    type: "road",
    points: Array.from({ length: 10_000 }, (_, index) => index % 2 ? [255, 255] : [0, 0]),
    width: 1,
  }],
};
const complexityStartedAt = performance.now();
assert.throws(
  () => buildMapFromRecipe(pathologicalPathRecipe),
  /points must contain at most 2048 entries/,
  "a compact pathological path is rejected before geometry materialization",
);
assert(performance.now() - complexityStartedAt < 1_000,
  "path complexity rejection stays fast enough for synchronous CLI materialization");
assert.throws(
  () => buildMapFromRecipe({
    width: 16,
    height: 16,
    operations: Array.from({ length: 1_025 }, () => ({ type: "fill", material: "grass" })),
  }),
  /operations must contain at most 1024 entries/,
  "operation count is bounded before the first operation runs",
);
assert.throws(
  () => buildMapFromRecipe({
    width: 16,
    height: 16,
    operations: [{ type: "paintTiles", character: ".", tiles: Array(65_537).fill({ x: 0, y: 0 }) }],
  }),
  /tiles must contain at most 65536 entries/,
  "one explicit tile list cannot exceed a complete maximum-size map",
);
assert.throws(
  () => buildMapFromRecipe({
    width: 16,
    height: 16,
    operations: Array.from({ length: 5 }, () => ({
      type: "overlayTiles",
      tiles: Array(65_536).fill({ x: 0, y: 0 }),
      edit: { concealment: true },
    })),
  }),
  /explicit tiles must total at most 262144 entries/,
  "explicit tile lists also share one recipe-wide bound",
);
assert.throws(
  () => buildMapFromRecipe({
    width: 256,
    height: 256,
    operations: [{
      type: "road",
      points: Array.from({ length: 125 }, (_, index) => [index % 256, index % 256]),
      width: 1,
    }],
  }),
  /estimated work exceeds the 8388608-unit limit/,
  "bounded point arrays still cannot multiply into excessive tile/segment work",
);

{
  const calls = [];
  const stdout = [];
  const status = runAuthoritativeMapTool("check", "maps/example.json", {
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '{"valid":true}\n', stderr: "" };
    },
    stdout: (value) => stdout.push(value),
  });
  assert.equal(status, 0);
  assert.equal(calls[0].command, "cargo");
  assert.deepEqual(calls[0].args.slice(-4, -1), ["authored-map", "--", "check"]);
  assert.equal(path.basename(calls[0].args.at(-1)), "example.json");
  assert.deepEqual(stdout, ['{"valid":true}\n']);
  assert.equal(runCli(["report", "map.json"], {
    runAuthoritativeMapTool(command, file) { calls.push({ command, file }); return 7; },
  }), 7);
  assert.deepEqual(calls.at(-1), { command: "report", file: "map.json" });
}

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-map-author-test-"));
try {
  const recipePath = path.join(tempRoot, "recipe.json");
  const mapPath = path.join(tempRoot, "map.json");
  const previewPath = path.join(tempRoot, "preview.svg");
  fs.writeFileSync(recipePath, JSON.stringify(recipe));

  const build = spawnSync(process.execPath, ["scripts/map-author.mjs", "build", recipePath, "--output", mapPath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(build.status, 0, build.stderr);
  assert(build.stdout.includes("map was not rejected"), "build reports advisory warnings without failing");
  assert(fs.existsSync(mapPath));

  const pathologicalPath = path.join(tempRoot, "pathological-recipe.json");
  fs.writeFileSync(pathologicalPath, JSON.stringify(pathologicalPathRecipe));
  const pathologicalBuild = spawnSync(process.execPath, [
    "scripts/map-author.mjs", "build", pathologicalPath, "--output", mapPath,
  ], { cwd: repoRoot, encoding: "utf8", timeout: 2_000 });
  assert.equal(pathologicalBuild.status, 1);
  assert.match(pathologicalBuild.stderr, /points must contain at most 2048 entries/,
    "the Node CLI exposes the shared complexity error instead of entering geometry work");

  const validate = spawnSync(process.execPath, ["scripts/map-author.mjs", "validate", mapPath, "--symmetry", "halfTurn"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(validate.status, 0, validate.stderr);
  assert(validate.stdout.includes("protected area"));

  const render = spawnSync(process.execPath, [
    "scripts/map-author.mjs", "preview", mapPath, "--output", previewPath, "--layers", "concealment",
  ], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(render.status, 0, render.stderr);
  const previewSvg = fs.readFileSync(previewPath, "utf8");
  assert(previewSvg.startsWith("<svg"));
  assert(previewSvg.includes('data-layer="concealment"') && !previewSvg.includes('data-layer="base"'),
    "CLI --layers forwards the exact preview layer selection");
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

console.log("✅ map_author_cli_contracts.mjs: shared authoring parity, advisory validation, and preview");
