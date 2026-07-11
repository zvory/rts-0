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

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const calls = { spawn: 0, update: 0, remove: 0, inspectSizes: [] };
const service = new LabInteractService({
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
const sessionId = opened.sessionId;

const spawns = Array.from({ length: 100 }, (_, index) => ({
  owner: index % 2 + 1,
  kind: "rifleman",
  x: 100 + index * 32,
  y: 100,
  alias: `unit_${index}`,
}));
const spawned = await service.execute("spawn", { sessionId, spawns });
assert.equal(calls.spawn, 1, "command service dispatches a bulk spawn once");
assert.equal(spawned.results.length, 100, "command service preserves bulk spawn result ordering");

const updated = await service.execute("update", {
  sessionId,
  updates: spawned.results.map((entry, index) => ({
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
  update: { operation: "move", entity: spawned.results[0].id, x: 300, y: 300 },
});
assert.equal(calls.update, 2, "legacy singular update normalizes to one plural driver request");

await service.execute("remove", { sessionId, refs: spawned.results.map((entry) => entry.id) });
assert.equal(calls.remove, 1, "command service dispatches a bulk remove once");

const largeSpawn = await service.execute("spawn", {
  sessionId,
  spawns: Array.from({ length: 400 }, (_, index) => ({
    owner: index % 2 + 1,
    kind: "rifleman",
    x: 100 + index * 32,
    y: 300,
  })),
});
const largeIds = largeSpawn.results.map((entry) => entry.id);
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
assert.equal(calls.remove, 2, "a 400-item remove still dispatches one mutation request");
assert.ok(Math.max(...calls.inspectSizes) <= LAB_INTERACT_LIMITS.maxInspectRefs,
  "bulk reference resolution respects the bridge inspect bound");

assert.equal(LAB_INTERACT_LIMITS.maxMutationBatch, 400, "mutation batch authority is 400");
assert.doesNotThrow(() => validateCommandInput("spawn", {
  sessionId,
  spawns: Array.from({ length: 400 }, () => ({ owner: 1, kind: "rifleman", x: 1, y: 1 })),
}));
assert.throws(() => validateCommandInput("spawn", {
  sessionId,
  spawns: Array.from({ length: 401 }, () => ({ owner: 1, kind: "rifleman", x: 1, y: 1 })),
}), /1-400/);

const normalized = normalizeError(new LabInteractDriverError("labRejected", "blocked", {
  failedIndex: 3,
  attempted: { x: 10, y: 20 },
  suggestions: [{ x: 42, y: 20 }],
}));
assert.equal(normalized.details.failedIndex, 3, "driver failures retain the failed batch index");
assert.equal(normalized.details.suggestions[0].x, 42, "driver failures retain placement suggestions");

await service.shutdown("test");
console.log("✅ lab_interact_bulk_contracts.mjs: bulk command service contracts passed");
