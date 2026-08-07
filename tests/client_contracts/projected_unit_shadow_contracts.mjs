import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  hasProjectedUnitShadow,
  ProjectedUnitShadowLayer,
  projectedUnitShadowHeight,
} from "../../client/src/renderer/projected_unit_shadows.js";

assert.equal(projectedUnitShadowHeight(KIND.RIFLEMAN), 22);
assert.equal(projectedUnitShadowHeight(KIND.MACHINE_GUNNER), 22);
assert.equal(projectedUnitShadowHeight(KIND.SCOUT_CAR), 22);
assert.equal(projectedUnitShadowHeight(KIND.TANK), 33);
assert.equal(projectedUnitShadowHeight(KIND.ARTILLERY), null);

for (const kind of [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.SCOUT_CAR, KIND.TANK]) {
  assert.equal(hasProjectedUnitShadow(kind), true, `${kind} uses the projected GPU shadow path`);
}
assert.equal(hasProjectedUnitShadow(KIND.ARTILLERY), false);

const layer = new ProjectedUnitShadowLayer({ pixi: null });
layer.supported = true;
layer.enabled = true;
layer.map = {
  width: 1,
  height: 1,
  tileSize: 32,
  elevation: [0],
  minElevation: 0,
};
layer.geometry = { instanceCount: 0 };
layer.mesh = { visible: false };
layer.instanceData = new Float32Array(800 * 11);
layer.instanceBuffer = { setDataWithSize() {} };
const entities = Array.from({ length: 402 }, (_, index) => ({
  id: index + 1,
  kind: KIND.RIFLEMAN,
  x: 16,
  y: 16,
  facing: 0,
}));
entities[0].visionOnly = true;
assert.equal(layer.update(entities), 800);
assert.equal(layer.hasShadowFor(entities[0].id), false, "concealment-only units do not cast shadows");
assert.equal(layer.hasShadowFor(entities[401].id), false, "overflow units retain native shadows");
assert.equal(layer.hasShadowFor(entities[400].id), true, "the final whole admitted unit is projected");

layer.instanceBuffer.setDataWithSize = () => { throw new Error("planned upload failure"); };
assert.throws(() => layer.update([entities[1]]), /planned upload failure/);
assert.equal(layer.geometry.instanceCount, 0, "failed uploads hide stale projected geometry");
assert.equal(layer.mesh.visible, false, "failed uploads leave the projected mesh hidden");
assert.equal(layer.hasShadowFor(entities[1].id), false, "failed uploads retain native shadows");
