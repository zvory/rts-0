import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  hasProjectedUnitShadow,
  projectedProxyPolygon,
  PROJECTED_UNIT_SHADOW_MIN_ELEVATION_DEGREES,
  PROJECTED_SHADOW_MARCH_STEP_WORLD,
  PROJECTED_SHADOW_MAX_MARCH_STEPS,
  UnifiedGpuShadowLayer,
} from "../../client/src/renderer/unified_gpu_shadows.js";

assert.equal(PROJECTED_SHADOW_MARCH_STEP_WORLD, 8, "terrain retains its coarse ray-march step");
assert.equal(PROJECTED_UNIT_SHADOW_MIN_ELEVATION_DEGREES, 30, "unit silhouettes remain readable under low authored sun");
assert.equal(
  PROJECTED_SHADOW_MARCH_STEP_WORLD * PROJECTED_SHADOW_MAX_MARCH_STEPS,
  768,
  "terrain preserves the previous maximum shadow reach without sampling unit geometry",
);

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
const projectedPolygons = [];
const fills = [];
layer.unitShadowGraphics = {
  clear() { projectedPolygons.length = 0; fills.length = 0; },
  poly(points) { projectedPolygons.push(points); },
  fill(options) { fills.push(options); },
};
layer.unitShadowTexture = {};
const entities = [
  { id: 1, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0 },
  { id: 2, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0, visionOnly: true },
];

layer.renderer = { render() {} };
assert.equal(layer.update(entities), 0, "detailed unit shadows default off and skip projection work");
assert.equal(layer.hasShadowFor(entities[0].id), false, "disabled projection retains lightweight rig shadows");
layer.setUnitShadowsEnabled(true);
assert.equal(layer.update(entities), 2);
assert.equal(layer.mesh.visible, true);
assert.equal(layer.hasShadowFor(entities[0].id), true);
assert.equal(layer.hasShadowFor(entities[1].id), false, "concealment-only units do not cast shadows");

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
  layer.update(prototypePairs),
  58,
  "each model box becomes one analytic projected polygon with no barrel tessellation",
);
assert(prototypePairs.every((entity) => layer.hasShadowFor(entity.id)));
assert.equal(projectedPolygons.length, 58);
assert(projectedPolygons.every((points) => points.length >= 6 && points.length <= 12));
assert(fills.every(({ color, alpha }) => color === 0xffffff && alpha === 1), "mask stores opaque coverage");

layer.setUnitShadowsEnabled(false);
assert.equal(layer.update(prototypePairs), 0, "turning detailed unit shadows off bypasses every model polygon");
assert(prototypePairs.every((entity) => !layer.hasShadowFor(entity.id)));
layer.setUnitShadowsEnabled(true);

layer.renderer.render = () => { throw new Error("planned texture render failure"); };
assert.throws(() => layer.update(entities), /planned texture render failure/);
assert.equal(layer.mesh.visible, false, "failed uploads leave the projected mesh hidden");
assert.equal(layer.hasShadowFor(entities[0].id), false, "failed uploads retain native shadows");
