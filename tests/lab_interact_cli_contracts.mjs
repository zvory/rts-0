import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { execFile, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import { DEFAULT_IDLE_MS, IPC_VERSION, configuredIdleMs, processAlive, runtimePaths, sleep } from "../scripts/lab-interact/runtime.mjs";

const execFileAsync = promisify(execFile);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "scripts/lab-interact/cli.mjs");
const isolatedTmp = fs.mkdtempSync(path.join("/tmp", "rts-li-contracts-"));
const originalTmp = process.env.TMPDIR;
process.env.TMPDIR = isolatedTmp;
process.once("exit", restoreTmp);
const paths = runtimePaths(root, { tmpDir: isolatedTmp });
const baseEnv = {
  ...process.env,
  RTS_LAB_INTERACT_DRIVER_FACTORY_MODULE: "tests/fixtures/lab_interact_fake_driver.mjs",
  RTS_LAB_INTERACT_IDLE_MS: "5000",
};

assert.equal(configuredIdleMs({}), DEFAULT_IDLE_MS, "the production idle default is exactly 30 minutes");
assert.equal(DEFAULT_IDLE_MS, 30 * 60_000, "idle default remains explicit and reviewable");
assert.equal(configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "25" }), 25, "tests may override the idle deadline");
assert.throws(() => configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "0" }), /must be an integer/, "invalid idle overrides are rejected");

shutdown(baseEnv);
fs.rmSync(paths.directory, { recursive: true, force: true });

const initial = call("status");
assert.equal(initial.ok, true, "the first CLI command automatically starts the daemon");
const daemonState = JSON.parse(fs.readFileSync(paths.state, "utf8"));
assert.equal(daemonState.workspaceRoot, fs.realpathSync(root), "runtime is pinned to the real worktree path");
assert.equal(daemonState.idleMs, 5000, "the daemon records its configured idle bound");
assert.ok(processAlive(daemonState.pid), "the daemon stays alive between CLI processes");
assert.equal(fs.statSync(paths.parent).mode & 0o777, 0o700, "runtime parent is private to the current uid");
assert.equal(fs.statSync(paths.directory).mode & 0o777, 0o700, "worktree runtime directory is mode 0700");
assert.equal(fs.statSync(paths.state).mode & 0o777, 0o600, "capability state is mode 0600");
assert.equal(fs.statSync(paths.socket).mode & 0o777, 0o600, "daemon socket is mode 0600");
const interactionBeforeInvalid = JSON.parse(fs.readFileSync(paths.state, "utf8")).lastInteractionAt;
const invalidEnvelope = await rawRequest(paths.socket, {
  protocolVersion: IPC_VERSION,
  daemonId: daemonState.daemonId,
  capability: "0".repeat(64),
  command: "status",
  input: {},
});
assert.equal(invalidEnvelope.error.code, "invalidRequest", "a wrong capability is rejected by the handshake");
assert.equal(JSON.parse(fs.readFileSync(paths.state, "utf8")).lastInteractionAt, interactionBeforeInvalid, "invalid envelopes do not extend idle lifetime");

const opened = call("open");
const sessionId = opened.result.sessionId;
assert.match(sessionId, /^lab_[a-f0-9]{32}$/, "open returns a bounded opaque session id");
const secondOpen = callFailure("open");
assert.equal(secondOpen.error.code, "sessionLimit", "one worktree exposes only one authoritative session");

call("spawn", { sessionId, spawns: [
  { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
  { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
] });
const inspected = call("inspect", { sessionId, refs: ["shooter", "target"], limit: 2 });
assert.deepEqual(inspected.result.entities.map((entity) => entity.alias).sort(), ["shooter", "target"], "aliases persist across CLI invocations");
const screenshot = call("screenshot", {
  sessionId,
  name: "cli-contract",
  presentation: "clean",
  viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  subjects: ["shooter", "target"],
});
assert.equal(screenshot.result.image.mimeType, "image/png", "screenshot returns bounded PNG metadata");
assert.equal("data" in screenshot.result.image, false, "CLI screenshot responses never embed image bytes");
assert.match(screenshot.result.pngPath, /target\/lab-interact\//, "new captures use only the renamed artifact root");

const unexpected = callFailure("inspect", { sessionId, unexpected: true });
assert.equal(unexpected.error.code, "invalidInput", "exact input shapes reject unknown fields");
const unsafeName = callFailure("screenshot", { sessionId, name: "../escape" });
assert.equal(unsafeName.error.code, "invalidInput", "artifact paths cannot be injected through names");
const invalidJson = spawnSync(process.execPath, [cli, "status", "not-json"], { cwd: root, env: baseEnv, encoding: "utf8" });
assert.notEqual(invalidJson.status, 0, "invalid CLI JSON exits nonzero");
assert.equal(invalidJson.stdout, "", "failed commands never write stdout");
assert.equal(JSON.parse(invalidJson.stderr).ok, false, "failed commands write one concise JSON error to stderr");

call("close", { sessionId });
const explicit = call("shutdown");
assert.equal(explicit.result.shuttingDown, true, "shutdown acknowledges immediate teardown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "shutdown removes socket, state, and runtime files");

prepareStaleRuntime();
const recovered = call("status");
assert.equal(recovered.ok, true, "a dead stale runtime is replaced automatically");
assert.notEqual(JSON.parse(fs.readFileSync(paths.state, "utf8")).pid, 99999999, "stale pid state is replaced");
shutdown(baseEnv);
await waitFor(() => !fs.existsSync(paths.directory), 2000, "recovered daemon shuts down cleanly");

const raceEnv = { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "5000" };
const race = await Promise.all([
  execFileAsync(process.execPath, [cli, "status"], { cwd: root, env: raceEnv }),
  execFileAsync(process.execPath, [cli, "status"], { cwd: root, env: raceEnv }),
]);
assert.ok(race.every(({ stdout, stderr }) => JSON.parse(stdout).ok && stderr === ""), "concurrent first commands share one startup race safely");
shutdown(raceEnv);
await waitFor(() => !fs.existsSync(paths.directory), 2000, "race-started daemon cleans up");

call("status");
const winningState = JSON.parse(fs.readFileSync(paths.state, "utf8"));
const duplicate = spawnSync(process.execPath, [path.join(root, "scripts/lab-interact/daemon.mjs"), root], {
  cwd: root, env: baseEnv, encoding: "utf8", timeout: 2000,
});
assert.notEqual(duplicate.status, 0, "a duplicate daemon cannot bind the owned worktree socket");
const afterDuplicate = JSON.parse(fs.readFileSync(paths.state, "utf8"));
assert.equal(afterDuplicate.daemonId, winningState.daemonId, "duplicate startup cannot delete or replace the winner runtime");
assert.equal(call("status").ok, true, "the winning daemon remains reachable after a duplicate startup");
shutdown(baseEnv);
await waitFor(() => !fs.existsSync(paths.directory), 2000, "duplicate-start test cleans up");

const inFlightEnv = { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "80", RTS_LAB_INTERACT_FAKE_DELAY_MS: "250" };
const inFlightSession = call("open", {}, inFlightEnv).result.sessionId;
const delayed = execFileAsync(process.execPath, [cli, "spawn", JSON.stringify({
  sessionId: inFlightSession,
  spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "slow" }],
})], { cwd: root, env: inFlightEnv });
await waitFor(() => JSON.parse(fs.readFileSync(paths.state, "utf8")).activeRequests === 1, 1000, "delayed command becomes active");
await sleep(140);
assert.equal(fs.existsSync(paths.directory), true, "idle expiry never tears down an in-flight command");
const shutdownDuringFlight = call("shutdown", {}, inFlightEnv);
assert.equal(shutdownDuringFlight.result.shuttingDown, true, "shutdown latches while another command is in flight");
const delayedResult = await delayed;
assert.equal(JSON.parse(delayedResult.stdout).ok, true, "admitted in-flight work finishes before shutdown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "latched shutdown runs when the last command finishes");

const idleEnv = { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "80" };
call("status", {}, idleEnv);
const idlePid = JSON.parse(fs.readFileSync(paths.state, "utf8")).pid;
await waitFor(() => !fs.existsSync(paths.directory), 2000, "idle daemon deletes its runtime directory");
await waitFor(() => !processAlive(idlePid), 2000, "idle daemon exits after closing owned resources");

restoreTmp();
console.log("✅ lab_interact_cli_contracts.mjs: CLI, daemon, validation, aliases, races, stale recovery, idle cleanup, and shutdown passed");

function call(command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  assert.equal(result.status, 0, `${command} succeeds: ${result.stderr}`);
  assert.equal(result.stderr, "", `${command} writes no stderr on success`);
  const lines = result.stdout.trim().split("\n");
  assert.equal(lines.length, 1, `${command} writes exactly one JSON value to stdout`);
  const response = JSON.parse(lines[0]);
  assert.equal(response.ok, true, `${command} returns a successful envelope`);
  return response;
}

function callFailure(command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  assert.notEqual(result.status, 0, `${command} fails for rejected input`);
  assert.equal(result.stdout, "", `${command} failure keeps stdout empty`);
  return JSON.parse(result.stderr);
}

function shutdown(env) {
  spawnSync(process.execPath, [cli, "shutdown", "{}"], { cwd: root, env, encoding: "utf8" });
}

function prepareStaleRuntime() {
  fs.mkdirSync(paths.directory, { recursive: true, mode: 0o700 });
  fs.writeFileSync(paths.state, `${JSON.stringify({ pid: 99999999, workspaceRoot: root })}\n`);
  fs.writeFileSync(paths.socket, "stale");
}

async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(20);
  }
  assert.fail(message);
}

function rawRequest(socketPath, request) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    socket.setEncoding("utf8");
    let body = "";
    socket.once("connect", () => socket.end(`${JSON.stringify(request)}\n`));
    socket.on("data", (chunk) => { body += chunk; });
    socket.once("error", reject);
    socket.once("end", () => resolve(JSON.parse(body)));
  });
}

function restoreTmp() {
  if (originalTmp == null) delete process.env.TMPDIR;
  else process.env.TMPDIR = originalTmp;
  fs.rmSync(isolatedTmp, { recursive: true, force: true });
}
