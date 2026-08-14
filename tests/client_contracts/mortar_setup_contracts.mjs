import assert from "node:assert/strict";

import { KIND, STATE } from "../../client/src/protocol.js";
import { _deployedWeaponSetupVisual } from "../../client/src/renderer/units.js";

assert.deepEqual(
  _deployedWeaponSetupVisual({ kind: KIND.MORTAR_TEAM, state: STATE.MOVE }),
  { prongFactor: 0, frameProgress: 0, barrel: false },
  "moving mortars use their packed pose without setup state",
);
assert.deepEqual(
  _deployedWeaponSetupVisual({ kind: KIND.MORTAR_TEAM, state: STATE.IDLE }),
  { prongFactor: 1, frameProgress: 1, barrel: true },
  "stationary mortars use their deployed firing pose without setup state",
);
