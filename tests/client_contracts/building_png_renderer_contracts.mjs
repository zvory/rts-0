import strictAssert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { HARVEST_TICKS, STATS, TICK_HZ } from "../../client/src/config.js";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import {
  createBuildingPngRigAtlases,
  createBuildingPngRigDefinitions,
} from "../../client/src/renderer/rigs/building_png.js";
import {
  createRigRenderContext,
  sampleRigAnimation,
} from "../../client/src/renderer/rigs/animation.js";
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
  [KIND.STEEL_MINE, [1, 1]],
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
const emblemKinds = new Set([
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
  strictAssert.ok(definition, `missing building PNG definition for ${kind}`);
  strictAssert.ok(atlas, `missing building PNG atlas for ${kind}`);
  strictAssert.deepEqual([STATS[kind].footW, STATS[kind].footH], footprint);
  strictAssert.deepEqual(
    [atlas.viewBox.width, atlas.viewBox.height],
    footprint.map((tiles) => tiles * 32),
  );
  const hasSilhouetteShadow = silhouetteShadowKinds.has(kind);
  const hasEmblem = emblemKinds.has(kind);
  const componentBodyParts = kind === KIND.STEEL_MINE
    ? ["part.pickaxe"]
    : kind === KIND.PUMP_JACK
      ? ["part.frame", "part.beam"]
      : null;
  const expectedBodyParts = componentBodyParts
    ?? ["part.base", "part.tint", ...(hasEmblem ? ["part.emblem"] : [])];
  const expectedShadowParts = hasSilhouetteShadow ? ["part.shadow"] : [];
  strictAssert.deepEqual(atlas.routes.body, expectedBodyParts);
  strictAssert.deepEqual(atlas.routes.shadow, expectedShadowParts);
  strictAssert.equal(Object.isFrozen(atlas.routes), true);
  strictAssert.equal(Object.isFrozen(atlas.routes.body), true);
  strictAssert.equal(Object.isFrozen(atlas.routes.shadow), true);
  strictAssert.equal(pngAtlasCanRenderRoute(definition, atlas, { parts: atlas.routes.body }), true);
  strictAssert.deepEqual(
    pngAtlasRouteCoverage(definition, atlas, { parts: atlas.routes.body }).missingParts,
    [],
  );
  strictAssert.deepEqual(
    atlas.sprites.map((sprite) => sprite.id),
    componentBodyParts
      ? componentBodyParts.map((part) => `sprite.${part.slice("part.".length)}`)
      : [
        "sprite.base",
        "sprite.tint",
        ...(hasSilhouetteShadow ? ["sprite.shadow"] : []),
        ...(hasEmblem ? ["sprite.emblem"] : []),
      ],
  );
  strictAssert.deepEqual(
    atlas.sprites.map((sprite) => sprite.tintSlot),
    componentBodyParts
      ? componentBodyParts.map(() => "fixed")
      : [
        "fixed",
        "team",
        ...(hasSilhouetteShadow ? ["fixed"] : []),
        ...(hasEmblem ? ["team"] : []),
      ],
  );
  strictAssert.deepEqual(
    new Set(atlas.sprites.flatMap((sprite) => sprite.sourceParts)),
    new Set(definition.parts.map((part) => part.id)),
  );
  strictAssert.equal(atlas.grid.columns, atlas.sprites.length);
  for (const [column, sprite] of atlas.sprites.entries()) {
    strictAssert.equal(sprite.frame.x, column * sprite.frame.w);
    strictAssert.ok(sprite.frame.x >= 0 && sprite.frame.y >= 0, `${kind}.${sprite.id} frame starts in grid`);
    strictAssert.ok(
      sprite.frame.x + sprite.frame.w <= atlas.grid.width
        && sprite.frame.y + sprite.frame.h <= atlas.grid.height,
      `${kind}.${sprite.id} frame fits grid`,
    );
    const expectedVisualScale = kind === KIND.RESOURCE_DEPOT ? 1.1 : 1;
    strictAssert.equal(
      sprite.frame.w / sprite.frame.pixelsPerUnitX,
      footprint[0] * 32 * expectedVisualScale,
    );
    strictAssert.equal(
      sprite.frame.h / sprite.frame.pixelsPerUnitY,
      footprint[1] * 32 * expectedVisualScale,
    );
  }
  strictAssert.equal(definition.parts.some((part) => part.id === "part.emblem"), hasEmblem);
  if (hasEmblem) {
    strictAssert.equal(definition.parts.find((part) => part.id === "part.emblem")?.tintSlot, "team");
    strictAssert.equal(atlas.sprites.find((sprite) => sprite.id === "sprite.emblem")?.tintSlot, "team");
  }
  if (hasSilhouetteShadow) {
    const shadowRoute = { parts: atlas.routes.shadow };
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

const extractorCycleMs = (HARVEST_TICKS / TICK_HZ) * 1000;
const steelDefinition = definitions.get(KIND.STEEL_MINE);
const activeSteel = { id: 901, kind: KIND.STEEL_MINE, extractorActive: true };
const woundSteel = sampleRigAnimation(
  steelDefinition,
  activeSteel,
  createRigRenderContext(activeSteel, { now: extractorCycleMs * 0.45 }),
);
const impactSteel = sampleRigAnimation(
  steelDefinition,
  activeSteel,
  createRigRenderContext(activeSteel, { now: extractorCycleMs * 0.70 }),
);
strictAssert.ok(
  woundSteel.parts["part.pickaxe"].transform.rotation > 0.7,
  "steel extractor winds the pickaxe back before the strike",
);
strictAssert.ok(
  Math.abs(impactSteel.parts["part.pickaxe"].transform.rotation) < 0.001,
  "steel extractor reaches the ore at the payout beat",
);
const inactiveSteel = { ...activeSteel, extractorActive: false };
const inactiveSample = sampleRigAnimation(
  steelDefinition,
  inactiveSteel,
  createRigRenderContext(inactiveSteel, { now: extractorCycleMs * 0.70 }),
);
strictAssert.ok(inactiveSample.parts["part.pickaxe"].transform.rotation > 0.7);

const pumpDefinition = definitions.get(KIND.PUMP_JACK);
const activePump = { id: 902, kind: KIND.PUMP_JACK, extractorActive: true };
const pumpTop = sampleRigAnimation(
  pumpDefinition,
  activePump,
  createRigRenderContext(activePump, { now: extractorCycleMs * 3 * 0.25 }),
);
const pumpBottom = sampleRigAnimation(
  pumpDefinition,
  activePump,
  createRigRenderContext(activePump, { now: extractorCycleMs * 3 * 0.75 }),
);
strictAssert.ok(pumpTop.parts["part.beam"].transform.rotation > 0.15);
strictAssert.ok(pumpBottom.parts["part.beam"].transform.rotation < -0.15);
const pumpAfterOneHarvest = sampleRigAnimation(
  pumpDefinition,
  activePump,
  createRigRenderContext(activePump, { now: extractorCycleMs }),
);
strictAssert.ok(
  pumpAfterOneHarvest.parts["part.beam"].transform.rotation > 0.13,
  "pump jack advances only one third of its motion cycle per harvest",
);
const inactivePump = { ...activePump, extractorActive: false };
const inactivePumpSample = sampleRigAnimation(
  pumpDefinition,
  inactivePump,
  createRigRenderContext(inactivePump, { now: extractorCycleMs * 3 * 0.25 }),
);
strictAssert.equal(inactivePumpSample.parts["part.beam"].transform.rotation, 0);

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
    state: "train",
    buildProgress: 0.42,
    prodProgress: 0.25,
    prodQueue: 6,
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
  assert(body.parts.size === 3, "emblem building PNG body route draws base, team tint, and emblem sprites");
  assert(shadow?.parts.size === 1, "perspective building PNG routes its silhouette shadow separately");
  assert(body.container.alpha === 0.45, "building PNG preserves scaffold transparency");
  assert(shadow.container.alpha === 0.45, "building silhouette shadow preserves scaffold transparency");
  assert(
    body.parts.get("sprite.base")?.display.tint === 0xffffff
      && body.parts.get("sprite.tint")?.display.tint === 0xc85050
      && body.parts.get("sprite.emblem")?.display.tint === 0xc85050,
    "building PNG applies the owning player's color to both the paint and white-keyed emblem sprites",
  );
  assert(
    renderer._pools.buildingShadows.get(entity.id)?.visible === false,
    "silhouette shadow replaces the temporary footprint shadow after the atlas loads",
  );
  const queueLabel = renderer._queueLabelPool.get(entity.id);
  assert(
    queueLabel?.text === "+5" && queueLabel.visible,
    "a production queue exposes the count waiting behind its active item",
  );
  assert(
    queueLabel.parent === renderer.layers.buildingOverlays,
    "queue labels stay above producer bodies when PNG art replaces an SVG fallback",
  );
  assert(
    renderer.world.children.indexOf(renderer.layers.buildingOverlays) >
      renderer.world.children.indexOf(renderer.layers.buildings),
    "the queue-label layer renders above the building-body layer",
  );

  const steelMine = {
    ...entity,
    id: 507,
    kind: KIND.STEEL_MINE,
    state: "idle",
    buildProgress: 1,
    prodProgress: 0,
    prodQueue: 0,
  };
  renderer._drawBuilding(steelMine, colorByOwner, state);
  assert(
    renderer._pools.buildingShadows.get(steelMine.id)?.visible === true,
    "extractor SVG fallback keeps its footprint shadow while the component atlas is unavailable",
  );
  for (const seen of Object.values(renderer._seen)) seen.clear();
  renderer._buildingPngRigAtlasTextures.set(
    KIND.STEEL_MINE,
    PIXI.Texture.from("steel-mine-building-atlas-test-texture"),
  );
  renderer._drawBuilding(steelMine, colorByOwner, state);
  renderer._sweep();
  assert(
    renderer._pools.buildingShadows.get(steelMine.id)?.visible === false,
    "loaded extractor component art suppresses the temporary fallback shadow",
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
