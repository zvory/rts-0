import assert from "node:assert/strict";

import { InteractBridge } from "../../client/src/interact_bridge.js";
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
  map: { name: "Chokes", width: 64, height: 64, tileSize: 32 },
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
      if (update.operation === "move" && entities.has(update.entityId)) {
        Object.assign(entities.get(update.entityId), { x: update.x, y: update.y });
      }
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
  async request(message) {
    calls.push({ op: message.op });
    return { ok: true, op: message.op, outcome: { accepted: true } };
  },
};
const match = {
  state,
  net: { stepRoomTime: () => { calls.push({ op: "stepRoomTime" }); state.currRecvTime += 1; } },
  capabilities: { roomTime: { available: true } },
  roomTimeControls: { roomTimeState: { currentTick: 10, speed: 0, paused: true } },
};
const bridge = new InteractBridge({
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
assert.equal(calls.filter((call) => call.op === "stepRoomTime").length, 0, "paused bulk spawn observes server fanout without advancing combat");
assert.equal(spawned.entities.length, 2, "bridge observes the complete authoritative spawn batch");

await bridge.order({ playerId: 1, command: { c: "move", units: [1], x: 120, y: 120 } });
assert.equal(calls.filter((call) => call.op === "stepRoomTime").length, 1, "paused orders retain one authoritative consumption tick");

labClient.spawnEntities = async (spawns) => {
  calls.push({ op: "spawnEntities", spawns });
  entities.set(3, { id: 3, ...spawns[0] });
  state.currRecvTime += 1;
  return {
    ok: true,
    op: "spawnEntities",
    outcome: { items: spawns.map((_, index) => ({ index, outcome: { entityId: index + 3 } })) },
  };
};
const fogFilteredSpawn = await bridge.spawn({
  spawns: [
    { owner: 1, kind: "rifleman", x: 300, y: 100, completed: true },
    { owner: 2, kind: "anti_tank_gun", x: 900, y: 900, completed: true },
  ],
});
assert.equal(fogFilteredSpawn.entities.length, 2, "fog-filtered spawn projections preserve batch result positions");
assert.equal(fogFilteredSpawn.entities[0].id, 3, "visible spawn result remains projected at its input index");
assert.equal(fogFilteredSpawn.entities[1], null, "authoritative hidden spawn result remains a positional null instead of timing out");
entities.delete(3);

await bridge.update({
  updates: [
    { operation: "move", entityId: 99, x: 800, y: 800 },
  ],
});
assert.equal(
  calls.filter((call) => call.op === "applyUpdates").length,
  1,
  "authoritative updates hidden by the active fog projection do not time out",
);

await bridge.update({
  updates: [
    { operation: "move", entityId: 1, x: 200, y: 100 },
    { operation: "move", entityId: 2, x: 100, y: 100 },
  ],
});
assert.equal(calls.filter((call) => call.op === "applyUpdates").length, 2, "bridge sends one browser request per plural update");
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
console.log("✅ interact_bulk_contracts.mjs: plural mutation and structured error contracts passed");
