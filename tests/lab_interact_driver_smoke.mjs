// Private-server Lab Interact smoke. Run directly; it owns and cleans up its browser and server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { LabInteractDriver } from "../scripts/lab-interact/driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let driver;
try {
  driver = await LabInteractDriver.open({
    workspaceRoot: root,
    startupTimeoutMs: 90_000,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  const status = await driver.status();
  assert.equal(status.ready, true, `bridge must report authoritative readiness: ${status.reason}`);

  const catalog = await driver.catalog();
  assert.ok(catalog.players.length >= 2, "catalog lists authoritative lab players");
  assert.ok(catalog.factions.some((faction) => faction.units.includes("rifleman")), "catalog exposes human-lab spawn kinds");

  const paused = await driver.time({ action: "pause" });
  const timingStart = paused.snapshotTick;
  await driver.time({ action: "resume", speed: 1 });
  await new Promise((resolve) => setTimeout(resolve, 900));
  const timingEnd = (await driver.time({ action: "pause" })).snapshotTick;
  const elapsedTicks = timingEnd - timingStart;
  assert.ok(
    elapsedTicks >= 10 && elapsedTicks <= 80,
    `speed 1 keeps the private Lab server near its production tick rate; observed ${elapsedTicks} ticks in 900ms`,
  );
  const spawned = await driver.spawn([
    { owner: 1, kind: "tank", x: 960, y: 960 },
    { owner: 2, kind: "rifleman", x: 1248, y: 960 },
  ]);
  const firstId = spawned.entities?.[0]?.id;
  const secondId = spawned.entities?.[1]?.id;
  assert.ok(Number.isInteger(firstId) && Number.isInteger(secondId), "one paused bulk spawn returns all observed authoritative entity ids");

  await driver.update([
    { operation: "move", entityId: firstId, x: 1248, y: 960 },
    { operation: "move", entityId: secondId, x: 960, y: 960 },
  ]);
  await driver.update([
    { operation: "move", entityId: firstId, x: 960, y: 960 },
    { operation: "move", entityId: secondId, x: 1248, y: 960 },
  ]);

  let placementError;
  try {
    await driver.spawn([{ owner: 1, kind: "rifleman", x: 960, y: 960 }]);
  } catch (error) {
    placementError = error;
  }
  assert.equal(placementError?.details?.failedIndex, 0, "blocked bulk spawn reports its failed input index");
  assert.ok(placementError?.details?.suggestions?.length > 0, "blocked bulk spawn returns authoritative legal suggestions");
  const suggestion = placementError.details.suggestions[0];
  const corrected = await driver.spawn([{ owner: 1, kind: "rifleman", x: suggestion.x, y: suggestion.y }]);
  const correctedId = corrected.entities?.[0]?.id;
  assert.ok(Number.isInteger(correctedId), "the first returned placement suggestion succeeds without coordinate guessing");

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
    request: { tool: "lab_interact_driver_smoke" },
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
  assert.deepEqual(
    order.result?.outcome,
    { accepted: true, playerId: 2 },
    "normal issueCommandAs identifies the authoritative player that accepted the command",
  );
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
    request: { tool: "lab_interact_driver_smoke", fixture: "two-entity" },
  });
  assert.ok(fs.statSync(twoEntityCapture.pngPath).size > 4096, "second clean capture frames two selected authoritative entities");
  assert.deepEqual(twoEntityCapture.readiness.missingTextureSubjectIds, [], "second clean capture has no selected-subject texture fallback");

  const diagnostics = driver.diagnostics();
  assert.deepEqual(diagnostics.pageConsoleErrors, [], "Lab Interact page has no console errors");
  assert.deepEqual(diagnostics.pageErrors, [], "Lab Interact page has no frame errors");
  await driver.remove([firstId, secondId, correctedId]);
  console.log("✅ lab_interact_driver_smoke.mjs: private server, bridge, mutation, order, time, camera, two captures, and cleanup passed");
} finally {
  await driver?.close();
}
