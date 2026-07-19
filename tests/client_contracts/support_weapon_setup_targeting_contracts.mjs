import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import {
  supportWeaponSetupTargetGroups,
  supportWeaponSetupTargets,
} from "../../client/src/input/support_weapon_setup_targeting.js";

const TILE_SIZE = 32;
const guns = [-10, -5, 0, 5, 10].map((yTiles, index) => ({
  id: index + 1,
  kind: KIND.ANTI_TANK_GUN,
  x: 0,
  y: yTiles * TILE_SIZE,
}));

const close = { x: 12 * TILE_SIZE, y: 0 };
const closeTargets = supportWeaponSetupTargets(guns, close, TILE_SIZE);
assert.ok(
  closeTargets.every((target) => target.x === close.x && target.y === close.y),
  "setup clicks inside 14 tiles remain one literal convergence point",
);
assert.deepEqual(
  supportWeaponSetupTargetGroups(closeTargets),
  [{ units: [1, 2, 3, 4, 5], x: close.x, y: close.y }],
  "literal convergence remains one batched setup command",
);

const parallelTargets = supportWeaponSetupTargets(
  guns,
  { x: 20 * TILE_SIZE, y: 0 },
  TILE_SIZE,
);
assert.ok(
  parallelTargets.every((target) => Math.abs(target.facing) < 0.000001),
  "at 20 tiles every AT gun faces exactly parallel toward the cursor ray",
);
assert.equal(
  supportWeaponSetupTargetGroups(parallelTargets).length,
  guns.length,
  "parallel facing uses individualized authoritative setup rays",
);

const halfFanTargets = supportWeaponSetupTargets(
  guns,
  { x: 22.5 * TILE_SIZE, y: 0 },
  TILE_SIZE,
);
assert.ok(
  Math.abs(halfFanTargets[0].facing + Math.PI / 8) < 0.000001 &&
    Math.abs(halfFanTargets[4].facing - Math.PI / 8) < 0.000001,
  "halfway from 20 to 25 tiles smoothly opens the endpoints to a 45-degree total fan",
);

const fullFanTargets = supportWeaponSetupTargets(
  guns,
  { x: 25 * TILE_SIZE, y: 0 },
  TILE_SIZE,
);
const expectedFacings = [-Math.PI / 4, -Math.PI / 8, 0, Math.PI / 8, Math.PI / 4];
for (let index = 0; index < fullFanTargets.length; index += 1) {
  assert.ok(
    Math.abs(fullFanTargets[index].facing - expectedFacings[index]) < 0.000001,
    `gun ${index + 1} receives its ranked 90-degree fan angle`,
  );
}

const justPastParallel = supportWeaponSetupTargets(
  guns,
  { x: 20.1 * TILE_SIZE, y: 0 },
  TILE_SIZE,
);
assert.ok(
  Math.abs(justPastParallel[0].facing) < 0.002,
  "fanout eases in without snapping immediately beyond 20 tiles",
);

const artillery = { id: 9, kind: KIND.ARTILLERY, x: 0, y: 0 };
const mixedTarget = { x: 25 * TILE_SIZE, y: 0 };
const mixedTargets = supportWeaponSetupTargets([guns[0], artillery], mixedTarget, TILE_SIZE);
assert.deepEqual(
  mixedTargets[1],
  { id: artillery.id, x: mixedTarget.x, y: mixedTarget.y },
  "the AT-specific fan leaves artillery on the literal setup point",
);

console.log("✅ support_weapon_setup_targeting_contracts.mjs: all assertions passed");
