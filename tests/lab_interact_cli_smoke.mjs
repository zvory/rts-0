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
  call("camera", { sessionId, camera: { action: "focus", refs: ["shooter", "target"], padding: 64 } });
  const screenshot = call("screenshot", {
    sessionId,
    name: "cli-smoke",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjects: ["shooter", "target"],
  });
  assert.equal(screenshot.image.mimeType, "image/png", "screenshot identifies PNG metadata without embedding bytes");
  assert.ok(fs.statSync(screenshot.pngPath).size > 4096, "CLI writes a nontrivial PNG artifact");
  const manifest = JSON.parse(fs.readFileSync(screenshot.manifestPath, "utf8"));
  assert.deepEqual(manifest.errors.render, [], "capture manifest reports no render errors");
  call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1088, y: 1088 } });
  call("time", { sessionId, control: { action: "step", ticks: 3 } });
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

console.log("✅ lab_interact_cli_smoke.mjs: live CLI lifecycle, session reuse/reopen, capture, reset, and cleanup passed");

async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(25);
  }
  assert.fail(message);
}
