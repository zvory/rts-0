import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { execFile, spawn, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import {
  DEFAULT_IDLE_MS, IPC_VERSION, STARTUP_GRACE_MS, configuredIdleMs, prepareRuntime,
  processAlive, readStartupLock, readState, reclaimStaleStartupLock,
  runtimePaths, sleep, startupLockStale,
} from "../scripts/lab-interact/runtime.mjs";
import { LAB_INTERACT_COMMANDS } from "../scripts/lab-interact/command_service.mjs";
import { LAB_INTERACT_COMMAND_HELP } from "../scripts/lab-interact/command_help.mjs";

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
  RTS_LAB_INTERACT_FAKE_OPEN_DELAY_MS: "250",
};

assert.equal(configuredIdleMs({}), DEFAULT_IDLE_MS, "the production idle default is exactly 30 minutes");
assert.equal(DEFAULT_IDLE_MS, 30 * 60_000, "idle default remains explicit and reviewable");
assert.equal(configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "25" }), 25, "tests may override the idle deadline");
assert.throws(() => configuredIdleMs({ RTS_LAB_INTERACT_IDLE_MS: "0" }), /must be an integer/, "invalid idle overrides are rejected");

shutdown(baseEnv);
fs.rmSync(paths.directory, { recursive: true, force: true });

for (const helpCommand of ["--help", "-h", "help"]) {
  const help = spawnSync(process.execPath, [cli, helpCommand], {
    cwd: path.dirname(root),
    env: baseEnv,
    encoding: "utf8",
  });
  assert.equal(help.status, 0, `${helpCommand} succeeds without a Git workspace`);
  assert.equal(help.stderr, "", `${helpCommand} keeps machine-readable help on stdout`);
  const helpEnvelope = JSON.parse(help.stdout);
  assert.equal(helpEnvelope.ok, true, `${helpCommand} returns a successful envelope`);
  assert.ok(helpEnvelope.result.commands.includes("open"), `${helpCommand} lists open`);
  assert.ok(helpEnvelope.result.commands.includes("shutdown"), `${helpCommand} lists shutdown`);
}
assert.deepEqual(Object.keys(LAB_INTERACT_COMMAND_HELP).sort(), [...LAB_INTERACT_COMMANDS].sort(), "help descriptor coverage equals the public command catalog");
for (const command of LAB_INTERACT_COMMANDS) {
  for (const args of [["help", command], [command, "--help"]]) {
    const help = spawnSync(process.execPath, [cli, ...args], {
      cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
    });
    assert.equal(help.status, 0, `${args.join(" ")} succeeds outside a Git checkout`);
    const descriptor = JSON.parse(help.stdout).result;
    assert.equal(descriptor.command, command, `${command} help identifies its command`);
    assert.equal(typeof descriptor.summary, "string", `${command} help has a summary`);
    assert.equal(typeof descriptor.acceptedShape, "string", `${command} help has an exact accepted shape`);
    assert.ok(Array.isArray(descriptor.variants), `${command} help lists variants`);
    assert.ok(Array.isArray(descriptor.defaults), `${command} help lists defaults`);
    assert.ok(Array.isArray(descriptor.bounds), `${command} help lists bounds`);
    assert.ok(descriptor.example && typeof descriptor.example === "object", `${command} help has one JSON example`);
  }
}
const unknownHelp = spawnSync(process.execPath, [cli, "help", "not-a-command"], {
  cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
});
assert.notEqual(unknownHelp.status, 0, "unknown per-command help remains a concise failure");
assert.equal(JSON.parse(unknownHelp.stderr).error.code, "unknownCommand", "unknown help reports unknownCommand without Git or daemon access");
assert.equal(fs.existsSync(paths.directory), false, "help does not start a daemon or create runtime state");

const coldOpen = call("open");
assert.match(coldOpen.result.sessionId, /^lab_[a-f0-9]{32}$/, "a cold first open returns one complete JSON envelope");
assert.equal(call("status").result.daemonCheckout.matches, true, "matching checkout status is explicit");
const currentCheckoutCommit = readState(paths).checkoutCommit;
assert.equal(call("status").result.opening, false, "completed cold open clears the opening status");
call("close", { sessionId: coldOpen.result.sessionId });
call("shutdown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "cold-first-open daemon shuts down cleanly");

const mismatchEnv = { ...baseEnv, RTS_LAB_INTERACT_TEST_CHECKOUT_COMMIT: "b".repeat(40) };
const mismatchSession = call("open", {}, mismatchEnv).result.sessionId;
const matchingState = readState(paths);
assert.match(matchingState.checkoutCommit, /^[a-f0-9]{40}$/, "daemon state publishes its startup checkout commit");
const activeMismatchStatus = call("status");
assert.deepEqual(
  activeMismatchStatus.result.daemonCheckout,
  { daemonCommit: "b".repeat(40), checkoutCommit: currentCheckoutCommit, matches: false },
  "status remains available and reports both checkout commits across mismatch",
);
const protectedMismatch = callFailure("inspect", { sessionId: mismatchSession });
assert.equal(protectedMismatch.error.code, "daemonCheckoutMismatch", "an active mismatched scene is protected from automatic refresh");
assert.equal(protectedMismatch.error.details.recoveryCommand, "node scripts/lab-interact/cli.mjs shutdown", "active mismatch reports the explicit recovery command");
assert.equal(processAlive(matchingState.pid), true, "active mismatch leaves the daemon and scene alive");
assert.equal(call("shutdown").result.shuttingDown, true, "shutdown remains available across checkout mismatch");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "explicit mismatched shutdown completes");

const afterExplicitRefresh = call("open").result;
assert.notEqual(afterExplicitRefresh.sessionId, mismatchSession, "a deliberate refresh creates a new session id");
assert.equal(callFailure("inspect", { sessionId: mismatchSession }).error.code, "unknownSession", "pre-refresh session ids become stale");
call("close", { sessionId: afterExplicitRefresh.sessionId });
call("shutdown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "current daemon stops before pre-feature fixture startup");
const preFeatureEnv = { ...baseEnv, RTS_LAB_INTERACT_TEST_OMIT_CHECKOUT: "1" };
call("status", {}, preFeatureEnv);
const preFeatureState = readState(paths);
assert.deepEqual(
  call("status").result.daemonCheckout,
  { daemonCommit: null, checkoutCommit: currentCheckoutCommit, matches: false },
  "a pre-feature daemon missing checkout metadata remains inspectable as a mismatch",
);
assert.equal(callFailure("open").error.code, "daemonCheckoutMismatch", "metadata-missing daemons are preserved instead of assuming they implement atomic refresh");
assert.equal(processAlive(preFeatureState.pid), true, "metadata-missing mismatch leaves the old daemon running");
call("shutdown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "metadata-missing daemon remains explicitly shut down-able");

call("status", {}, mismatchEnv);
const idleMismatchState = readState(paths);
const idleRefreshed = call("open").result;
const idleRefreshedState = readState(paths);
assert.notEqual(idleRefreshedState.pid, idleMismatchState.pid, "an idle known-mismatch daemon refreshes automatically");
assert.equal(idleRefreshedState.checkoutCommit, currentCheckoutCommit, "idle refresh publishes the current checkout commit");
call("close", { sessionId: idleRefreshed.sessionId });
call("shutdown");
await waitFor(() => !fs.existsSync(paths.directory), 2000, "freshness contract daemon shuts down cleanly");

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
assert.equal(initial.result.opening, false, "idle status reports no session opening in progress");
assert.equal(initial.result.closing, false, "idle status reports no session closing in progress");
const daemonState = JSON.parse(fs.readFileSync(paths.state, "utf8"));
assert.equal(daemonState.workspaceRoot, fs.realpathSync(root), "runtime is pinned to the real worktree path");
assert.equal(daemonState.idleMs, 5000, "the daemon records its configured idle bound");
assert.ok(processAlive(daemonState.pid), "the daemon stays alive between CLI processes");
const daemonProbe = await rawRequest(paths.socket, { protocolVersion: IPC_VERSION, probe: "lab-interact" });
assert.equal(daemonProbe.probe.checkoutCommit, daemonState.checkoutCommit, "compatible probes publish optional checkout metadata without changing IPC v1 identity");
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

const opening = execFileAsync(process.execPath, [cli, "open", "{}"], { cwd: root, env: baseEnv });
await waitFor(
  async () => (await rawRequest(paths.socket, {
    protocolVersion: IPC_VERSION,
    daemonId: daemonState.daemonId,
    capability: daemonState.capability,
    command: "status",
    input: {},
  })).result?.opening === true,
  1000,
  "status exposes an in-flight session open",
);
const { stdout: openingStdout, stderr: openingStderr } = await opening;
assert.equal(openingStderr, "", "opening a session writes no stderr");
const opened = JSON.parse(openingStdout);
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

const recordingStarted = call("record-start", {
  sessionId, name: "cli-contract-motion", maxDurationMs: 5_000,
  viewport: { width: 800, height: 600, deviceScaleFactor: 1 }, scale: 0.5,
});
assert.equal(recordingStarted.result.recorder.active, true, "record-start exposes one active session recorder");
assert.equal(call("status", { sessionId }).result.recorder.active, true, "session status exposes active recorder state");
assert.equal(callFailure("record-start", { sessionId, name: "duplicate" }).error.code, "recordingActive", "duplicate recording starts are correctable errors");
call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1100, y: 1000 } });
const stepped = call("time", { sessionId, control: { action: "step", ticks: 3 } });
const recordingStopped = call("record-stop", { sessionId });
assert.equal(recordingStopped.result.probe.codec, "h264", "record-stop returns bounded H.264 MP4 probe metadata");
assert.match(recordingStopped.result.videoPath, /\.mp4$/, "record-stop returns a mobile MP4 path");
assert.equal(recordingStopped.result.framePaths.length, 2, "record-stop returns representative frame paths");
assert.match(recordingStopped.result.contactSheetPath, /target\/lab-interact\//, "contact sheets stay beneath the artifact root");
assert.equal(callFailure("record-stop", { sessionId }).error.code, "recordingInactive", "duplicate recording stops are correctable errors");
assert.equal(callFailure("record-start", { sessionId, maxDurationMs: 30_001 }).error.code, "invalidInput", "recording duration remains hard bounded");
assert.equal(callFailure("record-start", { sessionId, crop: { x: 0, y: 0, width: 1, height: 10 } }).error.code, "invalidInput", "recording crop dimensions remain bounded");

const fixed = call("capture-fixed", { sessionId, name: "cli-fixed", fps: 60, frameCount: 3 });
assert.equal(fixed.result.framePaths.length, 3, "capture-fixed returns one confined PNG path per requested frame");
assert.match(fixed.result.videoPath, /\.mp4$/, "capture-fixed returns a mobile MP4 path");
assert.deepEqual(
  fixed.result.authoritative,
  { startTick: stepped.result.result.snapshotTick, endTick: stepped.result.result.snapshotTick + 1 },
  "capture-fixed reports its authoritative tick range",
);
assert.match(fixed.result.videoPath, /target\/lab-interact\//, "fixed video remains beneath the artifact root");
assert.equal(callFailure("capture-fixed", { sessionId, fps: 61, frameCount: 1 }).error.code, "invalidInput", "fixed capture FPS remains bounded");

const setupExport = call("export", { sessionId, kind: "setup", name: "Aliased fixture", reproduction: true });
assert.match(setupExport.result.artifactId, /^artifact_[a-f0-9]{32}$/, "setup export returns an opaque artifact id");
assert.equal("checkpointPayload" in setupExport.result, false, "setup export never prints checkpoint bytes");
assert.match(setupExport.result.path, /target\/lab-interact\/artifacts\//, "portable artifacts stay under the worktree target root");
const setupInspect = call("artifact-inspect", { sessionId, artifactId: setupExport.result.artifactId });
assert.equal(setupInspect.result.kind, "setup", "artifact inspection derives the stored kind");
assert.equal(setupInspect.result.aliasCount, 2, "artifact inspection reports bounded alias metadata");
const setupImport = call("import", { sessionId, kind: "setup", artifactId: setupExport.result.artifactId });
assert.deepEqual(setupImport.result.aliases.stale, [], "setup import reconciles aliases through the authoritative id map");
assert.deepEqual(call("inspect", { sessionId, refs: ["shooter", "target"] }).result.entities.map((entity) => entity.id).sort(), [1100, 1101], "remapped aliases resolve after setup import");
const unsafeImport = callFailure("import", { sessionId, kind: "setup", path: "/etc/passwd" });
assert.equal(unsafeImport.error.code, "unsafeArtifactPath", "imports reject paths outside target/lab-interact");

const replayExport = call("export", { sessionId, kind: "replay", name: "Fixture replay" });
assert.equal(replayExport.result.operationCount, 0, "replay export reports operation count without embedding operations");
assert.equal(call("import", { sessionId, kind: "replay", artifactId: replayExport.result.artifactId }).result.validation.ok, true, "replay import reports authoritative rebuild validation");

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
assert.equal(readStartupLock(paths), null, "duplicate bind loser removes its own claimed startup lock");
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

const inFlightEnv = { ...baseEnv, RTS_LAB_INTERACT_IDLE_MS: "1000", RTS_LAB_INTERACT_FAKE_DELAY_MS: "1500" };
const inFlightSession = call("open", {}, inFlightEnv).result.sessionId;
const delayed = execFileAsync(process.execPath, [cli, "spawn", JSON.stringify({
  sessionId: inFlightSession,
  spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "slow" }],
})], { cwd: root, env: inFlightEnv });
await waitFor(() => readState(paths)?.activeRequests === 1, 2000, "delayed command becomes active");
await sleep(1100);
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
    if (await predicate()) return;
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
