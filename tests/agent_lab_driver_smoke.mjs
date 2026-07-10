// Private-server Agent Lab smoke. Run directly; it owns and cleans up its browser and server.
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { AgentLabDriver } from "../scripts/agent-lab/driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let driver;
try {
  driver = await AgentLabDriver.open({ workspaceRoot: root, startupTimeoutMs: 90_000 });
  const status = await driver.status();
  assert.equal(status.ready, true, `bridge must report authoritative readiness: ${status.reason}`);

  const catalog = await driver.catalog();
  assert.ok(catalog.players.length >= 2, "catalog lists authoritative lab players");
  assert.ok(catalog.factions.some((faction) => faction.units.includes("rifleman")), "catalog exposes human-lab spawn kinds");

  await driver.time({ action: "pause" });
  const first = await driver.spawn({ owner: 1, kind: "rifleman", x: 960, y: 960 });
  const second = await driver.spawn({ owner: 2, kind: "rifleman", x: 1248, y: 960 });
  const firstId = first.entity?.id;
  const secondId = second.entity?.id;
  assert.ok(Number.isInteger(firstId) && Number.isInteger(secondId), "paused spawns return observed authoritative entity ids");

  const afterSpawn = await driver.inspect({ ids: [firstId, secondId], limit: 2 });
  assert.equal(afterSpawn.entities.length, 2, "inspection sees both spawned entities from an authoritative snapshot");

  await driver.order({
    playerId: 1,
    command: { c: "move", units: [firstId], x: 1088, y: 1088 },
  });
  await driver.time({ action: "step", ticks: 3 });
  const afterOrder = await driver.inspect({ ids: [firstId], limit: 1 });
  assert.equal(afterOrder.entities[0]?.id, firstId, "normal issueCommandAs order leaves the unit observable");

  const camera = await driver.camera({ action: "focus", entityIds: [firstId, secondId] });
  assert.ok(Number.isFinite(camera.camera.x) && Number.isFinite(camera.camera.zoom), "camera focus returns bounded camera state");
  await driver.reset();

  const diagnostics = driver.diagnostics();
  assert.deepEqual(diagnostics.pageConsoleErrors, [], "Agent Lab page has no console errors");
  assert.deepEqual(diagnostics.pageErrors, [], "Agent Lab page has no frame errors");
  console.log("✅ agent_lab_driver_smoke.mjs: private server, bridge, mutation, order, time, camera, reset, and cleanup passed");
} finally {
  await driver?.close();
}
