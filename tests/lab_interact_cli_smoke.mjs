// Live CLI smoke. Standalone runs own a private Rust server; browser CI reuses its loopback server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { processAlive, runtimePaths, sleep } from "../scripts/lab-interact/runtime.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "scripts/lab-interact/cli.mjs");
const isolatedTmp = fs.mkdtempSync(path.join("/tmp", "rts-li-smoke-"));
const env = { ...process.env, TMPDIR: isolatedTmp };
const paths = runtimePaths(root, { tmpDir: isolatedTmp });
let sessionId = null;
let daemonPid = null;

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

function spawnBulkUsingPlacementSuggestions(sessionId, spawns) {
  const corrected = spawns.map((spawn) => ({ ...spawn }));
  let previousFailedIndex = -1;
  for (let attempt = 0; attempt <= corrected.length; attempt += 1) {
    const result = invoke("spawn", { sessionId, spawns: corrected });
    if (result.status === 0) {
      const response = JSON.parse(result.stdout);
      assert.equal(response.ok, true, "corrected bulk spawn returns success");
      assert.equal(response.result.results.length, corrected.length, "one successful bulk spawn returns every authored unit");
      assert.equal(new Set(response.result.results.map((entry) => entry.id)).size, corrected.length, "bulk spawn returns one distinct entity id per input");
      return response.result;
    }

    assert.equal(result.stdout, "", "rejected atomic bulk spawn keeps stdout empty");
    const error = JSON.parse(result.stderr).error;
    assert.equal(error.code, "labRejected", "terrain-blocked bulk spawn is an authoritative Lab rejection");
    const failedIndex = error.details?.failedIndex;
    assert.ok(
      Number.isInteger(failedIndex) && failedIndex > previousFailedIndex && failedIndex < corrected.length,
      "each authoritative correction advances to a later batch input",
    );
    assert.deepEqual(
      error.details.attempted,
      { x: corrected[failedIndex].x, y: corrected[failedIndex].y },
      "placement diagnostics identify the attempted batch position",
    );
    const suggestion = error.details.suggestions?.[0];
    assert.ok(Number.isFinite(suggestion?.x) && Number.isFinite(suggestion?.y), "blocked placement supplies a legal authoritative suggestion");
    corrected[failedIndex] = { ...corrected[failedIndex], x: suggestion.x, y: suggestion.y };
    previousFailedIndex = failedIndex;
  }
  assert.fail("authoritative placement suggestions did not produce a legal bulk arrangement");
}

try {
  assert.equal(call("shutdown").alreadyStopped, true, "isolated smoke starts without touching any developer daemon");
  const opened = call("open", { viewport: { width: 1000, height: 700, deviceScaleFactor: 1 } });
  sessionId = opened.sessionId;
  daemonPid = JSON.parse(fs.readFileSync(paths.state, "utf8")).pid;
  assert.equal(opened.workspace.root, fs.realpathSync(root), "CLI daemon serves the selected worktree");
  assert.equal(call("open").sessionId, sessionId, "live repeated open returns the active session idempotently");
  const catalog = call("catalog", { sessionId, categories: ["units", "players", "commands"] });
  assert.ok(catalog.categories.units.includes("rifleman"), "catalog exposes the normal lab unit catalog");
  call("time", { sessionId, control: { action: "pause" } });
  call("spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
  ] });
  const largeSceneAliases = Array.from({ length: 36 }, (_, index) => `scene_${index}`);
  spawnBulkUsingPlacementSuggestions(sessionId, largeSceneAliases.map((alias, index) => ({
    owner: index % 2 + 1,
    kind: "rifleman",
    x: 480 + (index % 6) * 80,
    y: 1200 + Math.floor(index / 6) * 80,
    alias,
  })));
  const authoredSubjects = ["shooter", "target", ...largeSceneAliases];
  call("camera", { sessionId, camera: { action: "focus", refs: authoredSubjects, padding: 64 } });
  const screenshot = call("screenshot", {
    sessionId,
    name: "cli-smoke",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjects: authoredSubjects,
  });
  assert.equal(screenshot.image.mimeType, "image/png", "screenshot identifies PNG metadata without embedding bytes");
  assert.ok(fs.statSync(screenshot.pngPath).size > 4096, "CLI writes a nontrivial PNG artifact");
  const manifest = JSON.parse(fs.readFileSync(screenshot.manifestPath, "utf8"));
  assert.deepEqual(manifest.errors.render, [], "capture manifest reports no render errors");
  assert.deepEqual(
    { count: manifest.subjects.count, detailed: manifest.subjects.details.length, truncated: manifest.subjects.truncated },
    { count: authoredSubjects.length, detailed: 24, truncated: true },
    "large-scene canary checks every focused subject while bounding detailed manifest rows",
  );
  assert.equal(screenshot.readiness.subjects.count, authoredSubjects.length, "readiness covers the full authored subject set");
  assert.deepEqual(screenshot.readiness.missingTextureSubjectIds, [], "large-scene readiness rejects texture fallback for every subject");
  const recordingStarted = call("record-start", {
    sessionId, name: "cli-smoke-motion", maxDurationMs: 5_000,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  assert.equal(recordingStarted.recorder.active, true, "live CLI starts one persistent-page recorder");
  call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1088, y: 1088 } });
  call("time", { sessionId, control: { action: "resume", speed: 1 } });
  await sleep(1_200);
  call("time", { sessionId, control: { action: "pause" } });
  const recording = call("record-stop", { sessionId });
  assert.equal(recording.probe.codec, "h264", "live recording probes as H.264");
  assert.deepEqual({ width: recording.probe.width, height: recording.probe.height }, { width: 1000, height: 700 }, "live MP4 records the clean viewport crop");
  assert.ok(recording.probe.durationSeconds > 0 && recording.probe.durationSeconds <= 6, "live MP4 duration stays within the requested bound");
  assert.ok(fs.existsSync(recording.videoPath) && fs.existsSync(recording.contactSheetPath), "live recording writes MP4 and contact sheet artifacts");
  assert.equal(fs.existsSync(recording.videoPath.replace(/\.mp4$/, ".webm")), false, "live recording removes its temporary WebM");
  const recordingManifest = JSON.parse(fs.readFileSync(recording.manifestPath, "utf8"));
  assert.ok(recordingManifest.authoritative.endTick >= recordingManifest.authoritative.startTick, "recording manifest tracks authoritative tick bounds");
  assert.ok(recordingManifest.operations.some((entry) => entry.command === "order"), "recording manifest records accepted scene operations");
  assert.deepEqual(recordingManifest.errors.page, [], "recording manifest reports zero page errors");
  assert.equal(call("inspect", { sessionId, refs: ["shooter", "target"], limit: 2 }).entities.length, 2, "inspection returns authoritative spawned entities");

  const closedSessionId = sessionId;
  call("close", { sessionId });
  sessionId = null;
  const reopened = call("open");
  sessionId = reopened.sessionId;
  assert.notEqual(sessionId, closedSessionId, "close followed by live open creates a fresh session id");
  assert.equal(callFailure("inspect", { sessionId: closedSessionId }).code, "unknownSession", "closed live session ids are rejected as stale");
  call("time", { sessionId, control: { action: "pause" } });
  call("spawn", { sessionId, spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "resetSubject" }] });
  const reset = call("reset", { sessionId });
  assert.ok(reset.result, "fresh live sessions reset through the authoritative bridge");
  call("close", { sessionId });
  sessionId = null;
  assert.equal(call("shutdown").shuttingDown, true, "live smoke explicitly requests daemon teardown");
  await waitFor(() => !fs.existsSync(paths.directory), 5_000, "explicit shutdown removes the isolated runtime");
  await waitFor(() => !processAlive(daemonPid), 5_000, "explicit shutdown exits the child daemon");
} finally {
  if (sessionId) invoke("close", { sessionId });
  invoke("shutdown");
  if (daemonPid) await waitFor(() => !processAlive(daemonPid), 5_000, "smoke cleanup exits the daemon");
  fs.rmSync(isolatedTmp, { recursive: true, force: true });
}

console.log("✅ lab_interact_cli_smoke.mjs: live CLI lifecycle, recording/contact sheet, capture, reset, and cleanup passed");

async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(25);
  }
  assert.fail(message);
}
