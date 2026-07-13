// Live CLI canary. Standalone runs own a private Rust server; browser CI reuses its loopback server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { processAlive, runtimePaths, sleep } from "../scripts/lab-interact/runtime.mjs";
import { LabInteractTestArtifacts } from "./fixtures/lab_interact_test_artifacts.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "scripts/lab-interact/cli.mjs");
const isolatedTmp = fs.mkdtempSync(path.join("/tmp", "rts-li-smoke-"));
const env = {
  ...process.env,
  TMPDIR: isolatedTmp,
  RTS_LAB_INTERACT_TEST_TAILNET_PREVIEW_HOST: "127.0.0.1",
};
const renderer = env.RTS_LAB_INTERACT_RENDERER || "pixi";
assert.ok(["pixi", "babylon"].includes(renderer), "Lab Interact smoke renderer must be pixi or babylon");
const recordingDurationMs = Number(env.RTS_LAB_INTERACT_RECORDING_CANARY_MS || 5_000);
assert.ok(
  Number.isInteger(recordingDurationMs) && recordingDurationMs >= 2_000 && recordingDurationMs <= 60_000,
  "recording canary duration must stay between 2 and 60 seconds",
);

const paths = runtimePaths(root, { tmpDir: isolatedTmp });
const testArtifacts = new LabInteractTestArtifacts(root);
let sessionId = null;
let daemonPid = null;

try {
  assert.equal(call("shutdown").alreadyStopped, true, "isolated smoke starts from a stopped daemon");
  const opened = call("open", {
    renderer,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  sessionId = testArtifacts.ownSession(opened.sessionId);
  daemonPid = JSON.parse(fs.readFileSync(paths.state, "utf8")).pid;
  assert.equal(opened.workspace.root, fs.realpathSync(root), "CLI daemon serves the selected worktree");

  const status = call("status", { sessionId });
  assert.equal(status.status.ready, true, `authoritative Lab is ready: ${status.status.reason}`);
  const catalog = call("catalog", { sessionId, categories: ["units", "players", "commands"] });
  assert.ok(catalog.categories.units.includes("rifleman"), "catalog exposes the normal Lab unit catalog");
  assert.ok(catalog.categories.players.length >= 2, "catalog exposes two authoritative players");
  assert.ok(catalog.categories.commands.includes("move"), "catalog exposes the normal move command");
  const paused = call("time", { sessionId, control: { action: "pause" } });
  assert.equal(paused.result.roomTime.paused, true, "time pauses authoritative simulation");

  call("spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
  ] });
  const beforeUpdate = entity("shooter");
  call("update", {
    sessionId,
    updates: [{ operation: "move", entity: "shooter", x: 1024, y: 960 }],
  });
  const afterUpdate = entity("shooter");
  assert.notDeepEqual(
    { x: afterUpdate.x, y: afterUpdate.y },
    { x: beforeUpdate.x, y: beforeUpdate.y },
    "update changes the authoritative shooter position",
  );
  assert.deepEqual({ x: afterUpdate.x, y: afterUpdate.y }, { x: 1024, y: 960 }, "inspect observes the requested update");

  const ordered = call("order", {
    sessionId,
    playerId: 1,
    command: { c: "move", units: ["shooter"], x: 1024, y: 1152 },
  });
  const orderOutcome = ordered.result?.result?.outcome;
  assert.equal(orderOutcome?.accepted, true, "authoritative server accepts the move order");
  const stepped = call("time", { sessionId, control: { action: "step", ticks: 30 } });
  assert.ok(stepped.result.snapshotTick > orderOutcome.queuedAtTick, "time steps beyond command admission");
  const afterOrder = entity("shooter");
  assert.ok(afterOrder.y > afterUpdate.y, "authoritative ticks make observable progress on the move order");

  const focused = call("camera", {
    sessionId,
    camera: { action: "focus", refs: ["shooter", "target"], padding: 64 },
  });
  assert.ok(Number.isFinite(focused.camera.focus?.x), "camera focus returns a bounded semantic camera");
  const screenshot = call("screenshot", {
    sessionId,
    name: renderer === "babylon" ? "babylon-kernel" : "cli-smoke",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjects: ["shooter", "target"],
  });
  assert.deepEqual(
    { mimeType: screenshot.image.mimeType, width: screenshot.image.width, height: screenshot.image.height },
    { mimeType: "image/png", width: 1000, height: 700 },
    "screenshot reports the requested PNG dimensions",
  );
  assert.deepEqual(screenshot.readiness.missingTextureSubjectIds, [], "selected subjects use real textures");
  assert.equal(screenshot.readiness.subjects.count, 2, "readiness covers both authored subjects");
  const captureManifestPath = labArtifactPath(sessionId, "captures", ".json");
  const manifest = JSON.parse(fs.readFileSync(captureManifestPath, "utf8"));
  assert.deepEqual(manifest.errors.page, [], "capture manifest reports no page errors");
  assert.deepEqual(manifest.errors.frame, [], "capture manifest reports no frame errors");
  assert.deepEqual(manifest.errors.render, [], "capture manifest reports no render errors");

  assert.equal(screenshot.preview.available, true, "CLI returns a Tailnet screenshot preview");
  const screenshotPreview = await fetch(screenshot.preview.url);
  assert.equal(screenshotPreview.status, 200, "Tailnet screenshot preview is fetchable");
  assert.equal(screenshotPreview.headers.get("content-type"), "image/png", "preview preserves PNG media type");
  const screenshotBytes = Buffer.from(await screenshotPreview.arrayBuffer());
  assert.ok(screenshotBytes.length > 4096, "preview serves a nontrivial PNG artifact");
  assert.deepEqual(screenshotBytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), "preview has a PNG signature");
  assert.deepEqual(
    { width: screenshotBytes.readUInt32BE(16), height: screenshotBytes.readUInt32BE(20) },
    { width: 1000, height: 700 },
    "preview PNG bytes retain the requested dimensions",
  );

  const setupExport = testArtifacts.ownPortableArtifact(call("export", {
    sessionId,
    kind: "setup",
    name: "Two entity canary",
    reproduction: true,
  }));
  assert.equal(setupExport.aliasCount, 2, "setup export includes both aliases");
  call("remove", { sessionId, refs: ["target"] });
  assert.equal(call("inspect", { sessionId, refs: ["shooter"], limit: 1 }).entities.length, 1, "remove keeps the shooter observable");
  assert.equal(callFailure("inspect", { sessionId, refs: ["target"] }).code, "unknownAlias", "removed aliases are rejected");
  const setupImport = call("import", { sessionId, kind: "setup", artifactId: setupExport.artifactId });
  assert.equal(setupImport.validation.ok, true, "setup import performs authoritative validation");
  assert.equal(setupImport.aliases.restored.count, 2, "setup import restores both aliases");
  const restored = call("inspect", { sessionId, refs: ["shooter", "target"], limit: 2 }).entities;
  assert.deepEqual(restored.map((entry) => entry.alias).sort(), ["shooter", "target"], "setup import restores the scene aliases");

  const recordingStarted = call("record-start", {
    sessionId,
    name: "cli-smoke-motion",
    maxDurationMs: recordingDurationMs,
    viewport: { width: 1200, height: 800, deviceScaleFactor: 1 },
  });
  assert.equal(recordingStarted.recorder.active, true, "live CLI starts one persistent-page recorder");
  call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1152, y: 1152 } });
  const recordingWait = invokeAsync("record-wait", { sessionId });
  await sleep(250);
  call("camera", { sessionId, camera: { action: "focus", refs: ["shooter", "target"], padding: 48 } });
  const recordingWaitResult = await recordingWait;
  assert.equal(recordingWaitResult.status, 0, `record-wait succeeds while camera remains interactive: ${recordingWaitResult.stderr}`);
  const recording = JSON.parse(recordingWaitResult.stdout).result;
  assert.equal(recording.probe.codec, "h264", "live recording probes as H.264");
  assert.deepEqual({ width: recording.probe.width, height: recording.probe.height }, { width: 1200, height: 800 }, "recording retains its clean viewport");
  assert.equal(recording.preview.available, true, "live recording returns a Tailnet MP4 preview");
  assert.equal(recording.preview.poster.available, true, "live recording returns a contact-sheet preview");
  const recordingPreview = await fetch(recording.preview.url, { headers: { Range: "bytes=0-1023" } });
  assert.equal(recordingPreview.status, 206, "Tailnet MP4 preview supports byte ranges");
  assert.equal(recordingPreview.headers.get("content-type"), "video/mp4", "recording preview preserves MP4 media type");
  const recordingDirectory = labArtifactPath(sessionId, "recordings", "", "cli-smoke-motion-");
  const videoPath = path.join(recordingDirectory, "cli-smoke-motion.mp4");
  assert.ok(fs.statSync(videoPath).size > 0, "live recording retains nonempty H.264 media internally");
  const recordingManifest = JSON.parse(fs.readFileSync(path.join(recordingDirectory, "cli-smoke-motion.json"), "utf8"));
  assert.ok(recordingManifest.operations.some((entry) => entry.command === "order"), "recording manifest includes the accepted order");
  assert.ok(recordingManifest.operations.some((entry) => entry.command === "camera"), "recording wait leaves camera interaction available");
  assert.deepEqual(recordingManifest.errors.page, [], "recording manifest reports no page errors");

  const reset = call("reset", { sessionId });
  assert.ok(reset.result, "live session resets through the authoritative bridge");
  const closedSessionId = sessionId;
  call("close", { sessionId });
  sessionId = null;
  assert.equal(callFailure("inspect", { sessionId: closedSessionId }).code, "unknownSession", "closed session ids are rejected as stale");
  assert.equal(call("shutdown").shuttingDown, true, "live smoke explicitly requests daemon teardown");
  await waitFor(() => !fs.existsSync(paths.directory), 5_000, "explicit shutdown removes the isolated runtime");
  await waitFor(() => !processAlive(daemonPid), 5_000, "explicit shutdown exits the child daemon");
} finally {
  if (sessionId) invoke("close", { sessionId });
  invoke("shutdown");
  if (daemonPid) await waitFor(() => !processAlive(daemonPid), 5_000, "smoke cleanup exits the daemon");
  testArtifacts.cleanup();
  testArtifacts.assertClean();
  fs.rmSync(isolatedTmp, { recursive: true, force: true });
}

console.log("✅ lab_interact_cli_smoke.mjs: semantic scene, setup round trip, PNG preview, H.264 recording, and teardown passed");

function invoke(command, input = {}) {
  return spawnSync(process.execPath, [cli, command, JSON.stringify(input)], {
    cwd: root,
    env,
    encoding: "utf8",
    maxBuffer: 2 * 1024 * 1024,
  });
}

function call(command, input = {}) {
  const result = invoke(command, input);
  assert.equal(result.status, 0, `${command} succeeds: ${result.stderr}`);
  const response = JSON.parse(result.stdout);
  assert.equal(response.ok, true, `${command} returns success`);
  return response.result;
}

function callFailure(command, input = {}) {
  const result = invoke(command, input);
  assert.notEqual(result.status, 0, `${command} rejects stale or invalid input`);
  assert.equal(result.stdout, "", `${command} failure keeps stdout empty`);
  return JSON.parse(result.stderr).error;
}

function invokeAsync(command, input = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cli, command, JSON.stringify(input)], {
      cwd: root,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("error", reject);
    child.once("close", (status, signal) => resolve({ status, signal, stdout, stderr }));
  });
}

function entity(ref) {
  const inspected = call("inspect", { sessionId, refs: [ref], limit: 1 });
  assert.equal(inspected.entities.length, 1, `${ref} remains observable`);
  return inspected.entities[0];
}

function labArtifactPath(activeSessionId, category, suffix, directoryPrefix = "") {
  const directory = path.join(root, "target", "lab-interact", activeSessionId, category);
  const names = fs.readdirSync(directory).filter((name) => name.startsWith(directoryPrefix) && name.endsWith(suffix));
  assert.equal(names.length, 1, `one internal Lab ${category} artifact matches ${directoryPrefix || suffix}`);
  return path.join(directory, names[0]);
}

async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(25);
  }
  assert.fail(message);
}
