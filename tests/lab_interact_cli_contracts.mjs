import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { execFile, spawn, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import {
  DEFAULT_IDLE_MS, IPC_VERSION, STARTUP_GRACE_MS, configuredIdleMs, prepareRuntime,
  processAlive, readStartupLock, reclaimStaleStartupLock, removeOwnedStartupLock,
  runtimePaths, sleep, startupLockStale,
} from "../scripts/lab-interact/runtime.mjs";

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
  RTS_LAB_INTERACT_FAKE_OPEN_DELAY_MS: "75",
};

assert.equal(configuredIdleMs({}), DEFAULT_IDLE_MS, "the production idle default is exactly 30 minutes");
assert.equal(DEFAULT_IDLE_MS, 30 * 60_000, "idle default remains explicit and reviewable");
assert.equal(configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "25" }), 25, "tests may override the idle deadline");
assert.throws(() => configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "0" }), /must be an integer/, "invalid idle overrides are rejected");

shutdown(baseEnv);
fs.rmSync(paths.directory, { recursive: true, force: true });

const invalidConfiguration = spawnSync(process.execPath, [cli, "status", "{}"], {
  cwd: root,
  env: { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "0" },
  encoding: "utf8",
});
assert.notEqual(invalidConfiguration.status, 0, "invalid daemon configuration fails before spawning a child");
assert.equal(JSON.parse(invalidConfiguration.stderr).error.code, "invalidConfiguration", "invalid daemon configuration stays actionable");
assert.equal(fs.existsSync(paths.directory), false, "invalid daemon configuration creates no runtime lease");

const brokenFactoryStartedAt = Date.now();
const brokenFactory = spawnSync(process.execPath, [cli, "status", "{}"], {
  cwd: root,
  env: { ...baseEnv, RTS_LAB_INTERACT_DRIVER_FACTORY_MODULE: "tests/fixtures/missing_lab_interact_driver.mjs" },
  encoding: "utf8",
});
assert.notEqual(brokenFactory.status, 0, "a pre-listen daemon startup failure reaches the CLI");
assert.equal(JSON.parse(brokenFactory.stderr).error.code, "ERR_MODULE_NOT_FOUND", "the CLI preserves the corrective startup error");
assert.ok(Date.now() - brokenFactoryStartedAt < 5_000, "a failed child is reported without waiting for the startup timeout");
assert.equal(fs.existsSync(paths.lock), false, "a failed child releases its claimed startup lease");

prepareRuntime(paths);
const freshDeadLockAt = Date.now();
fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: "d".repeat(32), role: "cli", pid: 99999999, createdAt: freshDeadLockAt })}\n`, { mode: 0o600 });
assert.equal(startupLockStale(paths, freshDeadLockAt), false, "a dead initiating CLI retains a bounded grace for its child to claim the lock");
assert.equal(startupLockStale(paths, freshDeadLockAt + STARTUP_GRACE_MS + 1), true, "dead-owner startup locks become reclaimable after the startup grace");
fs.writeFileSync(paths.lock, "{}\n", { mode: 0o600 });
const staleTime = new Date(Date.now() - STARTUP_GRACE_MS - 1_000);
fs.utimesSync(paths.lock, staleTime, staleTime);
assert.equal(reclaimStaleStartupLock(paths), true, "parseable malformed startup locks recover after the file-mtime grace");
assert.equal(fs.existsSync(paths.lock), false, "malformed lock recovery removes only the verified stale lock");
fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: "f".repeat(32), role: "cli", pid: 99999999, createdAt: Date.now() + 86_400_000 })}\n`, { mode: 0o600 });
fs.utimesSync(paths.lock, staleTime, staleTime);
assert.equal(reclaimStaleStartupLock(paths), true, "future-dated lock records fall back to bounded file-mtime recovery");
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
const firstSessionId = opened.result.sessionId;
assert.match(firstSessionId, /^lab_[a-f0-9]{32}$/, "open returns a bounded opaque session id");
const repeatedOpen = call("open");
assert.equal(repeatedOpen.result.sessionId, firstSessionId, "repeated open idempotently returns the active session");
call("close", { sessionId: firstSessionId });
const staleAfterClose = callFailure("inspect", { sessionId: firstSessionId });
assert.equal(staleAfterClose.error.code, "unknownSession", "a closed session id becomes stale immediately");
const concurrentOpen = await Promise.all([
  execFileAsync(process.execPath, [cli, "open", "{}"], { cwd: root, env: baseEnv }),
  execFileAsync(process.execPath, [cli, "open", "{}"], { cwd: root, env: baseEnv }),
]);
const concurrentIds = concurrentOpen.map(({ stdout, stderr }) => {
  assert.equal(stderr, "", "concurrent open writes no stderr");
  return JSON.parse(stdout).result.sessionId;
});
assert.equal(new Set(concurrentIds).size, 1, "concurrent opens coalesce on one driver and session");
const sessionId = concurrentIds[0];
assert.notEqual(sessionId, firstSessionId, "close followed by open creates a fresh session id");

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
const alreadyStopped = call("shutdown");
assert.equal(alreadyStopped.result.alreadyStopped, true, "shutdown reports alreadyStopped only after proving stale runtime has no live owner");
assert.equal(fs.existsSync(paths.directory), false, "alreadyStopped cleanup removes stale socket, state, lock, and runtime directory");

prepareStaleRuntime();
const recovered = call("status");
assert.equal(recovered.ok, true, "a dead stale runtime is replaced automatically");
assert.notEqual(JSON.parse(fs.readFileSync(paths.state, "utf8")).pid, 99999999, "stale pid state is replaced");
shutdown(baseEnv);
await waitFor(() => !fs.existsSync(paths.directory), 2000, "recovered daemon shuts down cleanly");

const raceEnv = { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "5000", RTS_LAB_INTERACT_TEST_STARTUP_DELAY_MS: "250" };
const firstRace = execFileAsync(process.execPath, [cli, "status"], { cwd: root, env: raceEnv });
await waitFor(() => fs.existsSync(paths.lock), 1000, "first startup publishes its ownership lock");
const secondRace = execFileAsync(process.execPath, [cli, "status"], { cwd: root, env: raceEnv });
await waitFor(() => fs.existsSync(paths.socket), 1000, "forced startup race reaches a listening socket");
assert.equal(fs.existsSync(paths.state), false, "startup state is deliberately delayed while the ownership lock remains held");
const race = await Promise.all([firstRace, secondRace]);
assert.ok(race.every(({ stdout, stderr }) => JSON.parse(stdout).ok && stderr === ""), "concurrent first commands share one startup race safely");
assert.equal(fs.existsSync(paths.lock), false, "daemon releases startup lock only after publishing state");
shutdown(raceEnv);
await waitFor(() => !fs.existsSync(paths.directory), 2000, "race-started daemon cleans up");

prepareRuntime(paths);
const lateNonce = "a".repeat(32);
fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: lateNonce, role: "cli", pid: process.pid, createdAt: Date.now() })}\n`, { mode: 0o600 });
const lateChild = spawn(process.execPath, [path.join(root, "scripts/lab-interact/daemon.mjs"), root, lateNonce], {
  cwd: root,
  env: { ...baseEnv, RTS_LAB_INTERACT_TEST_PREBIND_DELAY_MS: "250" },
  stdio: ["ignore", "pipe", "pipe"],
});
await waitFor(() => readStartupLock(paths)?.role === "daemon", 1000, "spawned daemon claims the initiating CLI nonce");
const replacementNonce = "b".repeat(32);
fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: replacementNonce, role: "cli", pid: process.pid, createdAt: Date.now() })}\n`, { mode: 0o600 });
const lateExit = await childExit(lateChild);
assert.notEqual(lateExit.code, 0, "a late child aborts when its claimed startup nonce is replaced");
assert.equal(fs.existsSync(paths.socket), false, "a late nonce-losing child aborts before socket bind");
assert.equal(fs.existsSync(paths.state), false, "a late nonce-losing child never publishes daemon state");
assert.equal(readStartupLock(paths)?.nonce, replacementNonce, "a late child never removes the replacement startup lock");
fs.rmSync(paths.directory, { recursive: true, force: true });

call("status");
const winningState = JSON.parse(fs.readFileSync(paths.state, "utf8"));
const duplicateNonce = "c".repeat(32);
fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: duplicateNonce, role: "cli", pid: process.pid, createdAt: Date.now() })}\n`, { mode: 0o600 });
const duplicate = spawnSync(process.execPath, [path.join(root, "scripts/lab-interact/daemon.mjs"), root, duplicateNonce], {
  cwd: root, env: baseEnv, encoding: "utf8", timeout: 2000,
});
assert.notEqual(duplicate.status, 0, "a duplicate daemon cannot bind the owned worktree socket");
const afterDuplicate = JSON.parse(fs.readFileSync(paths.state, "utf8"));
assert.equal(afterDuplicate.daemonId, winningState.daemonId, "duplicate startup cannot delete or replace the winner runtime");
assert.equal(fs.existsSync(paths.socket), true, "duplicate bind failure cannot unlink the winner socket");
const duplicateLock = readStartupLock(paths);
assert.equal(duplicateLock?.nonce, duplicateNonce, "duplicate bind loser leaves only its own claimed startup lock");
assert.equal(removeOwnedStartupLock(paths, duplicateNonce, duplicateLock.pid, "daemon"), true, "test cleanup removes the duplicate loser's lock by nonce ownership");
assert.equal(call("status").ok, true, "the winning daemon remains reachable after a duplicate startup");

const savedStateText = fs.readFileSync(paths.state, "utf8");
fs.writeFileSync(paths.state, "{corrupt\n");
const corruptState = callFailure("status");
assert.equal(corruptState.error.code, "daemonStateUnavailable", "corrupt state cannot trigger replacement of a live daemon");
assert.equal(fs.existsSync(paths.socket), true, "corrupt-state recovery never unlinks a live socket");
assert.equal(processAlive(winningState.pid), true, "corrupt-state recovery leaves the live owner running");
assert.equal(callFailure("shutdown").error.code, "daemonIdentity", "shutdown never reports a live corrupt-state owner as already stopped");
fs.writeFileSync(paths.state, savedStateText, { mode: 0o600 });
assert.equal(call("status").ok, true, "restoring authenticated state reconnects to the same daemon");

fs.rmSync(paths.state);
const missingState = callFailure("status");
assert.equal(missingState.error.code, "daemonStateUnavailable", "missing state cannot trigger replacement of a live daemon");
assert.equal(fs.existsSync(paths.socket), true, "missing-state recovery never unlinks a live socket");
fs.writeFileSync(paths.state, savedStateText, { mode: 0o600 });

const wrongCapability = { ...JSON.parse(savedStateText), capability: "0".repeat(64) };
fs.writeFileSync(paths.state, `${JSON.stringify(wrongCapability)}\n`, { mode: 0o600 });
const refusedShutdown = callFailure("shutdown");
assert.equal(refusedShutdown.error.code, "invalidRequest", "shutdown does not report alreadyStopped when a live daemon rejects its handshake");
assert.equal(processAlive(winningState.pid), true, "failed authenticated shutdown leaves the live daemon intact");
fs.writeFileSync(paths.state, savedStateText, { mode: 0o600 });
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
  fs.writeFileSync(paths.lock, `${JSON.stringify({ nonce: "e".repeat(32), role: "cli", pid: 99999999, createdAt: Date.now() - STARTUP_GRACE_MS - 1 })}\n`);
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

function childExit(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
}

function restoreTmp() {
  if (originalTmp == null) delete process.env.TMPDIR;
  else process.env.TMPDIR = originalTmp;
  fs.rmSync(isolatedTmp, { recursive: true, force: true });
}
