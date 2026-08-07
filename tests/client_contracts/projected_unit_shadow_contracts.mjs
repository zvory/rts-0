import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  hasProjectedUnitShadow,
  UnifiedGpuShadowLayer,
} from "../../client/src/renderer/unified_gpu_shadows.js";

for (const kind of [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.SCOUT_CAR, KIND.TANK]) {
  assert.equal(hasProjectedUnitShadow(kind), true, `${kind} uses the projected GPU shadow path`);
}
assert.equal(hasProjectedUnitShadow(KIND.ARTILLERY), false);

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
layer.uniforms = { uniforms: { uMaxHeight: 40 } };
layer.unitHeightGraphics = { clear() {}, poly() {}, fill() {} };
layer.unitHeightTexture = {};
const entities = [
  { id: 1, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0 },
  { id: 2, kind: KIND.RIFLEMAN, x: 16, y: 16, facing: 0, visionOnly: true },
];

layer.renderer = { render() {} };
assert.equal(layer.update(entities), 2);
assert.equal(layer.mesh.visible, true);
assert.equal(layer.hasShadowFor(entities[0].id), true);
assert.equal(layer.hasShadowFor(entities[1].id), false, "concealment-only units do not cast shadows");

layer.renderer.render = () => { throw new Error("planned texture render failure"); };
assert.throws(() => layer.update(entities), /planned texture render failure/);
assert.equal(layer.mesh.visible, false, "failed uploads leave the projected mesh hidden");
assert.equal(layer.hasShadowFor(entities[0].id), false, "failed uploads retain native shadows");
