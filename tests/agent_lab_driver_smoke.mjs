// Private-server Agent Lab smoke. Run directly; it owns and cleans up its browser and server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { AgentLabDriver } from "../scripts/agent-lab/driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let driver;
try {
  driver = await AgentLabDriver.open({
    workspaceRoot: root,
    startupTimeoutMs: 90_000,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  const status = await driver.status();
  assert.equal(status.ready, true, `bridge must report authoritative readiness: ${status.reason}`);

  const catalog = await driver.catalog();
  assert.ok(catalog.players.length >= 2, "catalog lists authoritative lab players");
  assert.ok(catalog.factions.some((faction) => faction.units.includes("rifleman")), "catalog exposes human-lab spawn kinds");

  await driver.time({ action: "pause" });
  const first = await driver.spawn({ owner: 1, kind: "tank", x: 960, y: 960 });
  const second = await driver.spawn({ owner: 2, kind: "rifleman", x: 1248, y: 960 });
  const firstId = first.entity?.id;
  const secondId = second.entity?.id;
  assert.ok(Number.isInteger(firstId) && Number.isInteger(secondId), "paused spawns return observed authoritative entity ids");

  const afterSpawn = await driver.inspect({ ids: [firstId, secondId], limit: 2 });
  assert.equal(afterSpawn.entities.length, 2, "inspection sees both spawned entities from an authoritative snapshot");
  await driver.camera({ action: "focus", entityIds: [firstId] });
  const capture = await driver.screenshot({
    sessionId: `lab_${"a".repeat(32)}`,
    name: "driver-smoke",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjectIds: [firstId],
    subjectSummaries: [afterSpawn.entities.find((entity) => entity.id === firstId)],
    request: { tool: "agent_lab_driver_smoke" },
  });
  assert.equal(capture.image.mimeType, "image/png", "capture returns PNG image content");
  assert.ok(capture.image.bytes > 4096, "capture contains nontrivial rendered PNG bytes");
  assert.deepEqual({ width: capture.image.width, height: capture.image.height }, { width: 1000, height: 700 }, "capture matches the requested viewport dimensions");
  assert.ok(fs.existsSync(capture.pngPath) && fs.existsSync(capture.manifestPath), "capture writes bounded PNG and manifest artifacts");
  const manifest = JSON.parse(fs.readFileSync(capture.manifestPath, "utf8"));
  assert.equal(manifest.authoritative.tick, capture.readiness.snapshotTick, "manifest records authoritative capture tick");
  assert.deepEqual(manifest.errors.page, [], "manifest records no uncaught page errors");
  assert.deepEqual(manifest.errors.frame, [], "manifest records no frame-loop errors");
  assert.deepEqual(manifest.errors.render, [], "manifest records no render errors");

  const order = await driver.order({
    playerId: 2,
    command: { c: "move", units: [secondId], x: 1376, y: 960 },
  });
  assert.equal(order.result?.op, "issueCommandAs", "normal issueCommandAs returns the accepted command receipt");
  await driver.time({ action: "step", ticks: 3 });
  const afterOrder = await driver.inspect({ ids: [secondId], limit: 1 });
  assert.equal(afterOrder.entities[0]?.id, secondId, "normal issueCommandAs order leaves the unit observable");

  const camera = await driver.camera({ action: "focus", entityIds: [firstId, secondId] });
  assert.ok(Number.isFinite(camera.camera.x) && Number.isFinite(camera.camera.zoom), "camera focus returns bounded camera state");
  const twoEntityCapture = await driver.screenshot({
    sessionId: `lab_${"a".repeat(32)}`,
    name: "driver-smoke-pair",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjectIds: [firstId, secondId],
    subjectSummaries: afterSpawn.entities,
    request: { tool: "agent_lab_driver_smoke", fixture: "two-entity" },
  });
  assert.ok(fs.statSync(twoEntityCapture.pngPath).size > 4096, "second clean capture frames two selected authoritative entities");
  assert.deepEqual(twoEntityCapture.readiness.missingTextureSubjectIds, [], "second clean capture has no selected-subject texture fallback");

  const diagnostics = driver.diagnostics();
  assert.deepEqual(diagnostics.pageConsoleErrors, [], "Agent Lab page has no console errors");
  assert.deepEqual(diagnostics.pageErrors, [], "Agent Lab page has no frame errors");
  console.log("✅ agent_lab_driver_smoke.mjs: private server, bridge, mutation, order, time, camera, two captures, and cleanup passed");
} finally {
  await driver?.close();
}
