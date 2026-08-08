import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  buildDirectionalHorizonMask,
  hasProjectedUnitShadow,
  MAX_RECEIVER_MARCH_STEPS,
  projectRayToTerrainReceiver,
  projectedProxyPolygon,
  RECEIVER_BISECTION_STEPS,
  shadowSunModel,
  STATIC_SHADOW_SAMPLES_PER_TILE,
  supportsUnifiedGpuShadowPass,
  unitMaskBounds,
  UnifiedGpuShadowLayer,
} from "../../client/src/renderer/unified_gpu_shadows.js";

assert.equal(STATIC_SHADOW_SAMPLES_PER_TILE, 4, "static visibility is map-relative, not viewport-relative");
assert.equal(MAX_RECEIVER_MARCH_STEPS, 1536, "the fixed shader loop covers a maximum 256-tile map diagonal");
assert.equal(RECEIVER_BISECTION_STEPS, 4, "first receiver brackets get four bounded refinements");
assert.equal(
  shadowSunModel({ azimuthDegrees: 90, elevationDegrees: 12 }).slope,
  Math.tan(12 * Math.PI / 180),
  "dynamic projection uses the authored low sun without the former 30-degree clamp",
);
assert.equal(
  supportsUnifiedGpuShadowPass({
    width: 2,
    height: 2,
    tileSize: 32,
    elevation: [0, 0, 0, 0],
    sun: { azimuthDegrees: 0, elevationDegrees: 10, warmth: 0 },
  }),
  false,
  "flat maps do not activate the full-map shadow shader even when they author a low sun",
);
assert.equal(
  supportsUnifiedGpuShadowPass({
    width: 2,
    height: 2,
    tileSize: 32,
    elevation: [0, 1, 0, 0],
    sun: { azimuthDegrees: 0, elevationDegrees: 10, warmth: 0 },
  }),
  true,
  "authored elevation and sun activate the unified terrain and unit shadow pass",
);

const flatStaticMask = buildDirectionalHorizonMask({
  width: 3,
  height: 3,
  tileSize: 32,
  elevation: Array(9).fill(0),
  sun: { azimuthDegrees: 180, elevationDegrees: 12 },
}, 2);
assert.deepEqual([flatStaticMask.width, flatStaticMask.height], [6, 6]);
assert(flatStaticMask.data.every((coverage) => coverage === 0), "flat light-space depth has no blockers");

const ridgeStaticMask = buildDirectionalHorizonMask({
  width: 3,
  height: 3,
  tileSize: 32,
  elevation: [0, 0, 0, 0, 0, 0, 0, 8, 0],
  sun: { azimuthDegrees: 180, elevationDegrees: 12 },
}, 4);
assert(
  ridgeStaticMask.data.slice(0, ridgeStaticMask.width * 8).some((coverage) => coverage > 0),
  "sun-facing ridge depth casts into downstream receivers",
);

const receiverCase = (overrides = {}) => projectRayToTerrainReceiver({
  worldX: 100,
  worldY: 50,
  worldHeight: 20,
  sunDirection: [1, 0],
  sunSlope: Math.tan(12 * Math.PI / 180),
  mapWorldWidth: 300,
  mapWorldHeight: 100,
  tileSize: 32,
  mapMinWorldHeight: 0,
  terrainHeightAt: () => 0,
  ...overrides,
});
const flatReceiver = receiverCase();
assert(flatReceiver.hit && Math.abs(flatReceiver.distance - 93.75) <= 0.5,
  "flat ground resolves the low-sun geometric receiver without oscillation");
const risingReceiver = receiverCase({ terrainHeightAt: (x) => 100 - x });
assert(risingReceiver.hit && Math.abs(risingReceiver.distance - 16.5) <= 0.5,
  "slope=1, height=20, sun=12 degrees finds the first receiver near 16.5px instead of the old ~94px fixed-point result");
const descendingReceiver = receiverCase({
  worldX: 250,
  mapWorldWidth: 500,
  mapMinWorldHeight: -50,
  terrainHeightAt: (x) => -(250 - x) * 0.1,
});
assert(descendingReceiver.hit && descendingReceiver.distance > 170 && descendingReceiver.distance < 180,
  "descending terrain extends the ray monotonically until its later first hit");
const edgeReceiver = receiverCase({ worldX: 40, mapWorldWidth: 100 });
assert(!edgeReceiver.hit && edgeReceiver.reason === "limit" && edgeReceiver.distance === 40 && edgeReceiver.x === 0,
  "a ray with no receiver ends consistently on its last valid map-boundary point");
const lowSunReceiver = receiverCase({
  sunSlope: Math.tan(2 * Math.PI / 180),
  terrainHeightAt: (x) => 100 - x,
});
assert(lowSunReceiver.hit && lowSunReceiver.distance > 19 && lowSunReceiver.distance < 20,
  "very low authored sun remains bounded and finds the first rising-slope receiver");
const originReceiver = receiverCase({ worldHeight: 0 });
assert(originReceiver.hit && originReceiver.distance === 0 && originReceiver.reason === "origin",
  "a box corner already touching terrain resolves at the origin");

const horizontalBarrel = projectedProxyPolygon({
  x: 100,
  y: 100,
  facing: 0,
  center: [0, 0, 20],
  size: [20, 4, 4],
  yaw: 0,
  pitch: 0,
}, new Float32Array([0, -1]), 1);
const barrelXs = horizontalBarrel.filter((_, index) => index % 2 === 0);
const barrelYs = horizontalBarrel.filter((_, index) => index % 2 === 1);
assert.equal(Math.min(...barrelXs), 90);
assert.equal(Math.max(...barrelXs), 110);
assert.equal(Math.min(...barrelYs), 116, "elevated barrel shadow starts at its bottom-height projection");
assert.equal(Math.max(...barrelYs), 124, "elevated barrel shadow ends at its top-height projection");
assert(Math.min(...barrelYs) > 100, "analytic projection leaves lit space beneath an elevated barrel");

for (const kind of [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.SCOUT_CAR, KIND.TANK]) {
  assert.equal(hasProjectedUnitShadow(kind), true, `${kind} uses the projected GPU shadow path`);
}
for (const kind of [KIND.PANZERFAUST, KIND.ANTI_TANK_GUN, KIND.MORTAR_TEAM, KIND.ARTILLERY, KIND.COMMAND_CAR]) {
  assert.equal(hasProjectedUnitShadow(kind), true, `${kind} prototype uses the projected GPU shadow path`);
}
for (const kind of [KIND.SCOUT_PLANE, KIND.EKAT, KIND.WORKER, KIND.GOLEM]) {
  assert.equal(hasProjectedUnitShadow(kind), false, `${kind} remains outside this prototype`);
}

const layer = new UnifiedGpuShadowLayer({ pixi: null });
layer.supported = true;
layer.enabled = true;
layer.map = {
  width: 1,
  height: 1,
  tileSize: 32,
  elevation: [0],
  minElevation: 0,
};
layer.mesh = { visible: true };
layer.uniforms = { uniforms: { uSunDirection: new Float32Array([0, -1]), uSunSlope: 1 } };
layer.unitProjectionSlope = 1;
layer.staticBuildCount = 1;
layer.staticMapBuildCount = 1;
layer.staticBuildDurationMs = 2.5;
layer.staticCacheWidth = 504;
layer.staticCacheHeight = 504;
layer.map.width = 126;
layer.map.height = 126;
layer.unitUniforms = { uniforms: { uMaskOrigin: new Float32Array(2) } };
layer.uniforms.uniforms.uUnitMaskOrigin = new Float32Array(2);
layer.uniforms.uniforms.uUnitMaskWorldSize = new Float32Array(2);
const textureSizes = [];
layer.unitShadowTexture = { resize(width, height) { textureSizes.push([width, height]); } };
let uploadedShapes = [];
layer._writeInstances = (shapes) => { uploadedShapes = shapes; };
layer.unitMesh = {};
const entities = [
  { id: 1, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0 },
  { id: 2, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0, visionOnly: true },
];

layer.renderer = { render() {} };
const gpuLabels = [];
layer.gpuTimer = { measure(label, draw) { gpuLabels.push(label); return draw(); } };
const camera = { x: 100, y: 200, zoom: 1, viewportWidth: 800, viewportHeight: 600 };
assert.equal(layer.update(entities), 0, "detailed unit shadows default off and skip projection work");
assert.equal(layer.hasShadowFor(entities[0].id), false, "disabled projection retains lightweight rig shadows");
layer.setUnitShadowsEnabled(true);
assert.equal(layer.update(entities, camera), 2);
assert.equal(layer.staticBuildCount, 1, "entity and camera updates never rebuild static terrain visibility");
assert.deepEqual(layer.staticTerrainSummary(), {
  buildCount: 1,
  lifetimeBuildCount: 1,
  buildMs: 2.5,
  width: 504,
  height: 504,
  samplesPerTile: 4,
}, "worker diagnostics expose the one-shot cache build independently from frame updates");
assert.equal(layer.mesh.visible, true);
assert.equal(layer.hasShadowFor(entities[0].id), true);
assert.equal(layer.hasShadowFor(entities[1].id), false, "concealment-only units do not cast shadows");
assert.equal(gpuLabels.at(-1), "renderer.unitShadows.mask", "optional GPU diagnostics wrap the exact mask draw");

const prototypePairs = [
  KIND.PANZERFAUST,
  KIND.ANTI_TANK_GUN,
  KIND.MORTAR_TEAM,
  KIND.ARTILLERY,
  KIND.COMMAND_CAR,
].flatMap((kind, index) => [
  { id: 100 + index * 2, kind, x: 16, y: 16, facing: 0 },
  { id: 101 + index * 2, kind, x: 16, y: 16, facing: 0 },
]);
assert.equal(
  layer.update(prototypePairs, camera),
  58,
  "each model box becomes one analytic projected polygon with no barrel tessellation",
);
assert(prototypePairs.every((entity) => layer.hasShadowFor(entity.id)));
assert.equal(uploadedShapes.length, 58, "one typed instance is uploaded per model box");
assert.deepEqual(textureSizes.at(-1), [404, 304], "800x600 viewport uses a half-resolution R8 mask plus a two-texel filter gutter");
assert.deepEqual(
  unitMaskBounds(camera, layer.map),
  { minX: 96, minY: 196, widthPx: 404, heightPx: 304, widthWorld: 808, heightWorld: 608 },
  "mask bounds snap outward exactly and retain a two-texel linear-filter gutter",
);
assert.deepEqual(
  unitMaskBounds({ ...camera, x: -100, y: -200 }, layer.map),
  { minX: 0, minY: 0, widthPx: 352, heightPx: 202, widthWorld: 704, heightWorld: 404 },
  "overscroll is clipped to the exact visible map interval before the filter gutter is added",
);

layer.setUnitShadowsEnabled(false);
assert.equal(layer.update(prototypePairs, camera), 0, "turning detailed unit shadows off bypasses every model instance");
assert(prototypePairs.every((entity) => !layer.hasShadowFor(entity.id)));
layer.setUnitShadowsEnabled(true);

layer.renderer.render = () => { throw new Error("planned texture render failure"); };
assert.throws(() => layer.update(entities, camera), /planned texture render failure/);
assert.equal(layer.mesh.visible, false, "failed uploads leave the projected mesh hidden");
assert.equal(layer.hasShadowFor(entities[0].id), false, "failed uploads retain native shadows");
