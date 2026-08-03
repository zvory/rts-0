import strictAssert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { STATS } from "../../client/src/config.js";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import {
  createBuildingPngRigAtlases,
  createBuildingPngRigDefinitions,
} from "../../client/src/renderer/rigs/building_png.js";
import {
  pngAtlasCanRenderRoute,
  pngAtlasRouteCoverage,
} from "../../client/src/renderer/rigs/png_runtime.js";
import { assert } from "./assertions.mjs";
import { installFakePixi } from "./pixi_fakes.mjs";

const repoRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "../..");
const expectedFootprints = new Map([
  [KIND.RESOURCE_DEPOT, [3, 3]],
  [KIND.BARRACKS, [3, 2]],
  [KIND.TRAINING_CENTRE, [3, 2]],
  [KIND.ENGINEERING_COMPLEX, [3, 3]],
  [KIND.FACTORY, [3, 3]],
  [KIND.STEELWORKS, [3, 3]],
  [KIND.PUMP_JACK, [1, 1]],
]);
const silhouetteShadowKinds = new Set([
  KIND.RESOURCE_DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.ENGINEERING_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
]);
const definitions = createBuildingPngRigDefinitions();
const atlases = createBuildingPngRigAtlases();
strictAssert.equal(definitions.size, expectedFootprints.size);
strictAssert.equal(atlases.size, expectedFootprints.size);
for (const excludedKind of [KIND.DEPOT, KIND.TANK_TRAP]) {
  strictAssert.equal(definitions.has(excludedKind), false);
  strictAssert.equal(atlases.has(excludedKind), false);
}
for (const [kind, footprint] of expectedFootprints) {
  const definition = definitions.get(kind);
  const atlas = atlases.get(kind);
  const bodyRoute = { parts: ["part.base", "part.tint"] };
  strictAssert.ok(definition, `missing building PNG definition for ${kind}`);
  strictAssert.ok(atlas, `missing building PNG atlas for ${kind}`);
  strictAssert.equal(pngAtlasCanRenderRoute(definition, atlas, bodyRoute), true);
  strictAssert.deepEqual(pngAtlasRouteCoverage(definition, atlas, bodyRoute).missingParts, []);
  strictAssert.deepEqual([STATS[kind].footW, STATS[kind].footH], footprint);
  strictAssert.deepEqual(
    [atlas.viewBox.width, atlas.viewBox.height],
    footprint.map((tiles) => tiles * 32),
  );
  const hasSilhouetteShadow = silhouetteShadowKinds.has(kind);
  strictAssert.deepEqual(
    atlas.sprites.map((sprite) => sprite.tintSlot),
    hasSilhouetteShadow ? ["fixed", "team", "fixed"] : ["fixed", "team"],
  );
  if (hasSilhouetteShadow) {
    const shadowRoute = { parts: ["part.shadow"] };
    strictAssert.equal(pngAtlasCanRenderRoute(definition, atlas, shadowRoute), true);
    strictAssert.deepEqual(pngAtlasRouteCoverage(definition, atlas, shadowRoute).missingParts, []);
  }
  const assetPath = atlas.image.split("?", 1)[0].replace(/^\/assets\//, "client/assets/");
  const absoluteAssetPath = path.join(repoRoot, assetPath);
  strictAssert.equal(fs.existsSync(absoluteAssetPath), true, `missing ${assetPath}`);
  strictAssert.deepEqual(readPngDimensions(absoluteAssetPath), {
    width: atlas.grid.width,
    height: atlas.grid.height,
  });
}

const restorePixi = installFakePixi();
let renderer;
try {
  const parent = {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) { view.parentNode = this; },
    removeChild(view) { view.parentNode = null; },
  };
  renderer = await Renderer.create(parent);
  renderer._map = { tileSize: 32 };
  const entity = {
    id: 506,
    owner: 2,
    kind: KIND.BARRACKS,
    x: 160,
    y: 160,
    hp: 100,
    maxHp: 400,
    state: "idle",
    buildProgress: 0.42,
  };
  const colorByOwner = new Map([[2, 0xc85050]]);
  const state = {
    playerId: 99,
    players: [{ id: 2, color: "#c85050" }],
    spectator: true,
  };

  renderer._drawBuilding(entity, colorByOwner, state);
  const fallback = renderer._liveRigPools.buildingRigs.get(entity.id);
  assert(!renderer._iconPool, "building renderer omits legacy abbreviation labels");
  assert(
    typeof fallback?.matches === "function",
    "building uses its SVG rig while the production atlas is unavailable",
  );

  for (const seen of Object.values(renderer._seen)) seen.clear();
  renderer._buildingPngRigAtlasTextures.set(
    KIND.BARRACKS,
    PIXI.Texture.from("barracks-building-atlas-test-texture"),
  );
  renderer._drawBuilding(entity, colorByOwner, state);
  renderer._sweep();

  const body = renderer._liveRigPools.buildingRigs.get(entity.id);
  const shadow = renderer._liveRigPools.buildingPngShadows.get(entity.id);
  assert(
    body !== fallback && typeof body?.matchesPngAtlasRig === "function",
    "loaded building atlas replaces the temporary SVG instance in the shared body pool",
  );
  assert(fallback._destroyed === true, "building atlas promotion destroys the replaced SVG instance");
  assert(body.parts.size === 2, "building PNG body route draws fixed-color and team-tint sprites");
  assert(shadow?.parts.size === 1, "perspective building PNG routes its silhouette shadow separately");
  assert(body.container.alpha === 0.45, "building PNG preserves scaffold transparency");
  assert(shadow.container.alpha === 0.45, "building silhouette shadow preserves scaffold transparency");
  assert(
    body.parts.get("sprite.base")?.display.tint === 0xffffff
      && body.parts.get("sprite.tint")?.display.tint === 0xc85050,
    "building PNG applies the owning player's color only to its team-tint sprite",
  );
  assert(
    renderer._pools.buildingShadows.get(entity.id)?.visible === false,
    "silhouette shadow replaces the temporary footprint shadow after the atlas loads",
  );
} finally {
  renderer?.destroy();
  restorePixi();
}

const readinessRenderer = {
  _assetReadiness: new Map(),
  _missingTextureEntityIds: new Set(),
  _renderFrameCount: 0,
  _renderErrors: new Map(),
  groundDecalDiagnostics: () => ({ assetStatus: "idle" }),
};
Renderer.prototype._trackVisualAsset.call(
  readinessRenderer,
  "building-png:barracks",
  Promise.reject(new Error("building atlas unavailable")),
  { kind: KIND.BARRACKS, source: "buildingPngAtlas", required: false },
);
await Promise.resolve();
await Promise.resolve();
const startupReadiness = Renderer.prototype.startupAssetReadiness.call(readinessRenderer);
strictAssert.equal(startupReadiness.ready, true);
strictAssert.equal(startupReadiness.failedAssets.length, 0);
strictAssert.equal(startupReadiness.fallbackAssets.length, 1);
const captureReadiness = Renderer.prototype.captureReadiness.call(readinessRenderer, {
  subjectKinds: [KIND.BARRACKS],
});
strictAssert.equal(captureReadiness.ready, false);
strictAssert.equal(captureReadiness.failedAssets.length, 1);
Renderer.prototype._trackVisualAsset.call(
  readinessRenderer,
  "live-png:tank",
  Promise.reject(new Error("required unit atlas unavailable")),
  { kind: KIND.TANK, source: "livePngAtlas" },
);
await Promise.resolve();
await Promise.resolve();
const blockedStartup = Renderer.prototype.startupAssetReadiness.call(readinessRenderer);
strictAssert.equal(blockedStartup.ready, false);
strictAssert.equal(blockedStartup.failedAssets.length, 1);
strictAssert.equal(blockedStartup.fallbackAssets.length, 1);

console.log("building_png_renderer_contracts: ok");

function readPngDimensions(filePath) {
  const buffer = Buffer.alloc(24);
  const file = fs.openSync(filePath, "r");
  try {
    strictAssert.equal(fs.readSync(file, buffer, 0, buffer.length, 0), buffer.length);
  } finally {
    fs.closeSync(file);
  }
  strictAssert.equal(buffer.toString("hex", 0, 8), "89504e470d0a1a0a");
  strictAssert.equal(buffer.toString("ascii", 12, 16), "IHDR");
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}
