import assert from "node:assert/strict";
import { KIND } from "../../client/src/protocol.js";
import { STATS, WORKER_BUILDABLE, WORKER_BUILD_CARD_SLOTS } from "../../client/src/config.js";
import { barbedWireLineTiles, buildTankTrapLineSites, tankTrapBuildCommands } from "../../client/src/input/tank_trap_line.js";
import { createBuildingRigDefinitions } from "../../client/src/renderer/rigs/building_routing.js";

for (const end of [{ tileX: 5, tileY: 0 }, { tileX: 0, tileY: 5 }, { tileX: 4, tileY: 4 }, { tileX: -3, tileY: 5 }]) {
  const start = { tileX: 0, tileY: 0 };
  const tiles = barbedWireLineTiles(start, end);
  assert.deepEqual(tiles[0], start);
  assert.deepEqual(tiles.at(-1), end);
  assert.equal(new Set(tiles.map((t) => `${t.tileX},${t.tileY}`)).size, tiles.length);
  for (let i = 1; i < tiles.length; i++) {
    assert.equal(Math.abs(tiles[i].tileX - tiles[i - 1].tileX) + Math.abs(tiles[i].tileY - tiles[i - 1].tileY), 1);
  }
  const sites = buildTankTrapLineSites({ start, end, building: KIND.BARBED_WIRE, isValid: () => true });
  const commands = tankTrapBuildCommands([11], sites, KIND.BARBED_WIRE);
  assert.equal(commands.length, tiles.length);
  commands.forEach((cmd, i) => {
    assert.equal(cmd.building, KIND.BARBED_WIRE);
    assert.equal(cmd.tileX, tiles[i].tileX);
    assert.equal(cmd.tileY, tiles[i].tileY);
    assert.equal(!!cmd.queued, i > 0);
  });
}
const blocked = buildTankTrapLineSites({ start: { tileX: 0, tileY: 0 }, end: { tileX: 4, tileY: 0 }, building: KIND.BARBED_WIRE, isValid: (x) => x !== 2 });
assert.deepEqual(blocked.map((s) => s.valid), [true, true, false, false, false]);
assert.equal(tankTrapBuildCommands([11], blocked, KIND.BARBED_WIRE).length, 2);
assert.equal(STATS[KIND.BARBED_WIRE].cost.steel, 4);
assert.equal(STATS[KIND.BARBED_WIRE].buildTicks, 90);
assert.equal(STATS[KIND.BARBED_WIRE].requires, KIND.TRAINING_CENTRE);
assert(WORKER_BUILDABLE.includes(KIND.BARBED_WIRE));
assert(WORKER_BUILD_CARD_SLOTS.includes(KIND.BARBED_WIRE));
assert(createBuildingRigDefinitions().has(KIND.BARBED_WIRE));
console.log("✅ barbed wire line, commands, catalog and SVG contracts passed");
