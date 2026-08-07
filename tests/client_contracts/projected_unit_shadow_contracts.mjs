import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  hasProjectedUnitShadow,
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
