import assert from "node:assert/strict";
import path from "node:path";

import {
  LAB_INTERACT_LIMITS,
  LabInteractService,
  normalizeError,
  validateCommandInput,
} from "../scripts/lab-interact/command_service.mjs";
import { LabInteractDriverError } from "../scripts/lab-interact/driver.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";
import { LabInteractTestArtifacts } from "./fixtures/lab_interact_test_artifacts.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const testArtifacts = new LabInteractTestArtifacts(root);
const calls = { spawn: 0, update: 0, remove: 0, inspectSizes: [] };
let service;

try {
service = new LabInteractService({
  workspaceRoot: root,
  driverFactory: async (options) => {
    const driver = await openLabInteractDriver(options);
    for (const method of ["spawn", "update", "remove"]) {
      const original = driver[method].bind(driver);
      driver[method] = async (...args) => {
        calls[method] += 1;
        return original(...args);
      };
    }
    const inspect = driver.inspect.bind(driver);
    driver.inspect = async (query) => {
      calls.inspectSizes.push(query?.ids?.length || 0);
      return inspect(query);
    };
    return driver;
  },
});
const opened = await service.open({});
const sessionId = testArtifacts.ownSession(opened.sessionId);

const repeatProducers = await service.execute("spawn", {
  sessionId,
  spawns: [
    { owner: 1, kind: "barracks", x: 100, y: 100, alias: "repeat_a" },
    { owner: 1, kind: "barracks", x: 200, y: 100, alias: "repeat_b" },
  ],
});
const repeat = await service.execute("order", {
  sessionId,
  playerId: 1,
  command: {
    c: "adjustProductionRepeat",
    buildings: ["repeat_a", "repeat_b"],
    unit: "rifleman",
    delta: 1,
  },
});
assert.deepEqual(
  repeat.command,
  {
    c: "adjustProductionRepeat",
    buildings: repeatProducers.spawned.details.map((entry) => entry.id),
    unit: "rifleman",
    delta: 1,
  },
  "repeat-production orders resolve every producer alias into one authoritative command",
);
assert.deepEqual(
  repeat.resolved.buildings.map(({ alias, id }) => ({ alias, id })),
  repeatProducers.spawned.details.map(({ alias, id }) => ({ alias, id })),
  "repeat-production results retain producer alias resolution evidence",
);
await service.execute("remove", { sessionId, refs: ["repeat_a", "repeat_b"] });
calls.spawn = 0;
calls.remove = 0;
calls.inspectSizes.length = 0;

const spawns = Array.from({ length: 100 }, (_, index) => ({
  owner: index % 2 + 1,
  kind: "rifleman",
  x: 100 + index * 32,
  y: 100,
  alias: `unit_${index}`,
}));
const spawned = await service.execute("spawn", { sessionId, spawns });
assert.equal(calls.spawn, 1, "command service dispatches a bulk spawn once");
assert.deepEqual(
  { count: spawned.spawned.count, detailed: spawned.spawned.details.length, truncated: spawned.spawned.truncated },
  { count: 100, detailed: LAB_INTERACT_LIMITS.maxResponseDetails, truncated: true },
  "default bulk spawn output reports the full count with bounded ordered detail",
);
assert.deepEqual(
  spawned.spawned.details.map((entry) => entry.alias),
  spawns.slice(0, LAB_INTERACT_LIMITS.maxResponseDetails).map((entry) => entry.alias),
  "default bulk spawn detail preserves input ordering",
);
assert.equal("results" in spawned, false, "default bulk spawn omits decorated entity rows");
assert.equal("result" in spawned, false, "default bulk spawn omits the duplicate raw outcome");
assert.ok(Number.isInteger(spawned.snapshotTick), "default bulk spawn retains the authoritative snapshot tick");

const updated = await service.execute("update", {
  sessionId,
  updates: spawns.map((entry, index) => ({
    operation: "move",
    entity: entry.alias,
    x: 200 + index * 32,
    y: 200,
  })),
});
assert.equal(calls.update, 1, "command service dispatches a bulk update once");
assert.equal(updated.result.result.op, "applyUpdates", "command service uses the plural update operation");

await service.execute("update", {
  sessionId,
  update: { operation: "move", entity: spawns[0].alias, x: 300, y: 300 },
});
assert.equal(calls.update, 2, "legacy singular update normalizes to one plural driver request");

await service.execute("remove", { sessionId, refs: spawns.map((entry) => entry.alias) });
assert.equal(calls.remove, 1, "command service dispatches a bulk remove once");

const detailedSpawn = await service.execute("spawn", {
  sessionId,
  details: true,
  spawns: [
    { owner: 1, kind: "rifleman", x: 100, y: 250, alias: "detailed_0" },
    { owner: 2, kind: "rifleman", x: 132, y: 250, alias: "detailed_1" },
  ],
});
assert.equal(detailedSpawn.results.length, 2, "details=true returns every decorated spawn row");
assert.equal(detailedSpawn.result.outcome.items.length, 2, "details=true preserves the raw authoritative outcome");
await service.execute("remove", { sessionId, refs: ["detailed_0", "detailed_1"] });

const largeSpawn = await service.execute("spawn", {
  sessionId,
  spawns: Array.from({ length: 400 }, (_, index) => ({
    owner: index % 2 + 1,
    kind: "rifleman",
    x: 100 + index * 32,
    y: 300,
    alias: `large_${index}`,
  })),
});
assert.deepEqual(
  { count: largeSpawn.spawned.count, detailed: largeSpawn.spawned.details.length, truncated: largeSpawn.spawned.truncated },
  { count: 400, detailed: LAB_INTERACT_LIMITS.maxResponseDetails, truncated: true },
  "the maximum spawn batch still has a bounded default response",
);
assert.ok(JSON.stringify(largeSpawn).length < 2_000, "the maximum default spawn response stays compact");
const largeAliases = Array.from({ length: 400 }, (_, index) => `large_${index}`);
const largeInspection = await service.execute("inspect", { sessionId, refs: largeAliases, limit: 400 });
assert.equal(largeInspection.entities.length, 400, "inspection accepts and returns the full 400-reference operational bound");
assert.deepEqual(
  largeInspection.cameraViewport,
  { widthCssPx: 1440, heightCssPx: 900 },
  "inspection preserves the semantic camera viewport returned by the bridge",
);
assert.deepEqual(
  largeInspection.cameraWorldBounds,
  { minX: 0, minY: 0, maxX: 2048, maxY: 2048 },
  "inspection preserves semantic camera ground bounds returned by the bridge",
);
const largeIds = largeInspection.entities.map((entry) => entry.id);
const focusedCamera = await service.execute("camera", { sessionId, camera: { action: "focus", refs: largeAliases } });
assert.deepEqual(
  focusedCamera.cameraViewport,
  { widthCssPx: 1440, heightCssPx: 900 },
  "camera focus preserves the semantic camera viewport returned by the bridge",
);
assert.deepEqual(
  focusedCamera.cameraWorldBounds,
  { minX: 0, minY: 0, maxX: 2048, maxY: 2048 },
  "camera focus preserves semantic camera ground bounds returned by the bridge",
);
const largeCapture = await service.execute("screenshot", {
  sessionId, name: "large-contract", subjects: largeAliases,
});
assert.deepEqual(
  { count: largeCapture.readiness.subjects.count, detailed: largeCapture.readiness.subjects.details.length, truncated: largeCapture.readiness.subjects.truncated },
  { count: 400, detailed: 24, truncated: true },
  "a 400-subject capture checks every subject while returning a bounded detailed summary",
);
await service.execute("update", {
  sessionId,
  updates: largeIds.map((id, index) => ({
    operation: "move",
    entity: id,
    x: 200 + index * 32,
    y: 400,
  })),
});
await service.execute("remove", { sessionId, refs: largeIds });
assert.equal(calls.update, 3, "a 400-item update still dispatches one mutation request");
assert.equal(calls.remove, 3, "a 400-item remove still dispatches one mutation request");
assert.ok(Math.max(...calls.inspectSizes) <= LAB_INTERACT_LIMITS.maxInspectRefs,
  "bulk reference resolution respects the bridge inspect bound");

assert.equal(LAB_INTERACT_LIMITS.maxMutationBatch, 400, "mutation batch authority is 400");
assert.equal(LAB_INTERACT_LIMITS.maxAliases, 400, "large scenes may retain 400 operational aliases");
assert.equal(LAB_INTERACT_LIMITS.maxInspectResults, 400, "large-scene inspection may return 400 entities");
assert.doesNotThrow(() => validateCommandInput("camera", { sessionId, camera: { action: "focus", refs: Array.from({ length: 400 }, (_, index) => index + 1) } }));
assert.doesNotThrow(() => validateCommandInput("screenshot", { sessionId, subjects: Array.from({ length: 400 }, (_, index) => index + 1) }));
for (const [command, input] of [
  ["inspect", { sessionId, refs: Array.from({ length: 401 }, (_, index) => index + 1) }],
  ["camera", { sessionId, camera: { action: "focus", refs: Array.from({ length: 401 }, (_, index) => index + 1) } }],
  ["screenshot", { sessionId, subjects: Array.from({ length: 401 }, (_, index) => index + 1) }],
]) {
  assert.throws(() => validateCommandInput(command, input), /0-400|1-400/, `${command} rejects 401 references before the bridge`);
}
assert.doesNotThrow(() => validateCommandInput("spawn", {
  sessionId,
  details: true,
  spawns: Array.from({ length: 400 }, () => ({ owner: 1, kind: "rifleman", x: 1, y: 1 })),
}));
assert.throws(() => validateCommandInput("spawn", {
  sessionId,
  details: "yes",
  spawns: [{ owner: 1, kind: "rifleman", x: 1, y: 1 }],
}), /boolean/);
assert.throws(() => validateCommandInput("spawn", {
  sessionId,
  spawns: Array.from({ length: 401 }, () => ({ owner: 1, kind: "rifleman", x: 1, y: 1 })),
}), /1-400/);
assert.doesNotThrow(() => validateCommandInput("order", {
  sessionId,
  playerId: 1,
  command: {
    c: "adjustProductionRepeat",
    buildings: Array.from({ length: 100 }, (_, index) => index + 1),
    unit: "rifleman",
    delta: -1,
  },
}));
assert.throws(() => validateCommandInput("order", {
  sessionId,
  playerId: 1,
  command: {
    c: "adjustProductionRepeat",
    buildings: Array.from({ length: 101 }, (_, index) => index + 1),
    unit: "rifleman",
    delta: 1,
  },
}), /1-100/, "repeat production retains the bounded command-reference limit");

const normalized = normalizeError(new LabInteractDriverError("labRejected", "blocked", {
  failedIndex: 3,
  attempted: { x: 10, y: 20 },
  suggestions: [{ x: 42, y: 20 }],
}));
assert.equal(normalized.details.failedIndex, 3, "driver failures retain the failed batch index");
assert.equal(normalized.details.suggestions[0].x, 42, "driver failures retain placement suggestions");

console.log("✅ lab_interact_bulk_contracts.mjs: bulk command service contracts passed");
} finally {
  await service?.shutdown("test");
  testArtifacts.cleanup();
  testArtifacts.assertClean();
}
