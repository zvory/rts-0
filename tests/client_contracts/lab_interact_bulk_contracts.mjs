import assert from "node:assert/strict";

import { LabInteractBridge } from "../../client/src/lab_interact_bridge.js";
import { LabClient } from "../../client/src/lab_client.js";
import { LAB_ROLE } from "../../client/src/protocol.js";

const sent = [];
const handlers = new Map();
const net = {
  on(type, handler) { handlers.set(type, handler); },
  off(type) { handlers.delete(type); },
  lab(requestId, op) { sent.push({ requestId, op }); return true; },
};
const directClient = new LabClient(net, { timeoutMs: 1000 });
const directPromise = directClient.spawnEntities([
  { owner: 1, kind: "rifleman", x: 128, y: 160, completed: true },
  { owner: 2, kind: "rifleman", x: 192, y: 160, completed: true },
]);
assert.equal(sent.length, 1, "LabClient sends one plural spawn request");
handlers.get("labResult")({
  requestId: sent[0].requestId,
  ok: false,
  op: "spawnEntities",
  error: "batch item 1 failed",
  failedIndex: 1,
  details: { attempted: { x: 192, y: 160 }, blockers: [], suggestions: [{ x: 224, y: 160 }] },
});
const directResult = await directPromise;
assert.equal(directResult.failedIndex, 1, "LabClient preserves the failed batch index");
assert.equal(directResult.details.suggestions[0].x, 224, "LabClient preserves structured placement details");
void directClient.applyUpdates([{ operation: "move", entityId: 9, x: 224, y: 160 }]);
assert.equal(sent.at(-1).op.op, "applyUpdates", "LabClient sends plural updates for one item");
void directClient.deleteEntities([9, 10]);
assert.equal(sent.at(-1).op.op, "deleteEntities", "LabClient sends plural deletes once");
directClient.destroy();

const entities = new Map();
const calls = [];
const state = {
  currRecvTime: 1,
  tick: 10,
  players: [],
  map: { name: "Default", width: 64, height: 64, tileSize: 32 },
  entityById: (id) => entities.get(id) || null,
};
const labClient = {
  state: { role: LAB_ROLE.OPERATOR, room: "bulk-contract", godModePlayers: [] },
  async spawnEntities(spawns) {
    calls.push({ op: "spawnEntities", spawns });
    spawns.forEach((spawn, index) => entities.set(index + 1, { id: index + 1, ...spawn }));
    state.currRecvTime += 1;
    return {
      ok: true,
      op: "spawnEntities",
      outcome: { items: spawns.map((_, index) => ({ index, outcome: { entityId: index + 1 } })) },
    };
  },
  async applyUpdates(updates) {
    calls.push({ op: "applyUpdates", updates });
    for (const update of updates) {
      if (update.operation === "move") Object.assign(entities.get(update.entityId), { x: update.x, y: update.y });
    }
    state.currRecvTime += 1;
    return {
      ok: true,
      op: "applyUpdates",
      outcome: { items: updates.map((update, index) => ({ index, outcome: { entityId: update.entityId, x: update.x, y: update.y } })) },
    };
  },
  async deleteEntities(entityIds) {
    calls.push({ op: "deleteEntities", entityIds });
    entityIds.forEach((id) => entities.delete(id));
    state.currRecvTime += 1;
    return {
      ok: true,
      op: "deleteEntities",
      outcome: { items: entityIds.map((entityId, index) => ({ index, outcome: { entityId } })) },
    };
  },
};
const match = {
  state,
  net: { stepRoomTime: () => calls.push({ op: "stepRoomTime" }) },
  capabilities: { roomTime: { available: true } },
  roomTimeControls: { roomTimeState: { currentTick: 10, speed: 0, paused: true } },
};
const bridge = new LabInteractBridge({
  enabled: true,
  windowLike: {},
  sleep: () => Promise.resolve(),
  app: { net: { ws: { readyState: 1 } }, labClient, match },
});

const spawned = await bridge.spawn({
  spawns: [
    { owner: 1, kind: "rifleman", x: 100, y: 100, completed: true },
    { owner: 2, kind: "rifleman", x: 200, y: 100, completed: true },
  ],
});
assert.equal(calls.filter((call) => call.op === "spawnEntities").length, 1, "bridge sends one browser request for a bulk spawn");
assert.equal(calls.filter((call) => call.op === "stepRoomTime").length, 1, "paused bulk spawn advances authoritative time once");
assert.equal(spawned.entities.length, 2, "bridge observes the complete authoritative spawn batch");

await bridge.update({
  updates: [
    { operation: "move", entityId: 1, x: 200, y: 100 },
    { operation: "move", entityId: 2, x: 100, y: 100 },
  ],
});
assert.equal(calls.filter((call) => call.op === "applyUpdates").length, 1, "bridge sends one plural update request");
assert.equal(entities.get(1).x, 200, "bridge waits for the whole observed update batch");

await bridge.remove({ entityIds: [1, 2] });
assert.equal(calls.filter((call) => call.op === "deleteEntities").length, 1, "bridge sends one plural delete request");
assert.equal(entities.size, 0, "bridge observes the complete authoritative delete batch");

labClient.spawnEntities = async () => ({
  ok: false,
  op: "spawnEntities",
  error: "batch item 0 failed",
  failedIndex: 0,
  details: { attempted: { x: 0, y: 0 }, blockers: [{ kind: "boundary", worldSize: 2048 }], suggestions: [{ x: 32, y: 32 }] },
});
const rejected = await bridge.call("spawn", { spawns: [{ owner: 1, kind: "rifleman", x: 0, y: 0 }] });
assert.equal(rejected.error.details.failedIndex, 0, "bridge preserves failedIndex on rejection");
assert.equal(rejected.error.details.suggestions[0].x, 32, "bridge preserves placement suggestions on rejection");

bridge.destroy();
console.log("✅ lab_interact_bulk_contracts.mjs: plural mutation and structured error contracts passed");
