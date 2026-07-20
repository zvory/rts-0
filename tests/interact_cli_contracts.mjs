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
} from "../scripts/interact/runtime.ts";
import { INTERACT_COMMANDS } from "../scripts/interact/command_service.ts";
import { INTERACT_COMMAND_HELP } from "../scripts/interact/command_help.ts";
import { commandDefinition, namespaceCommandKey } from "../scripts/interact/command_registry.ts";
import { InteractTestArtifacts } from "./fixtures/interact_test_artifacts.mjs";

const execFileAsync = promisify(execFile);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "scripts/interact/cli.mjs");
const tailnetPreviewCli = path.join(root, "scripts/tailnet-preview.mjs");
const isolatedTmp = fs.mkdtempSync(path.join("/tmp", "rts-li-contracts-"));
const previewRoot = path.join(isolatedTmp, "durable-previews");
const previewPort = await reserveLoopbackPort();
const testArtifacts = new InteractTestArtifacts(root);
const originalTmp = process.env.TMPDIR;
process.env.TMPDIR = isolatedTmp;
process.once("exit", restoreTmp);
const paths = runtimePaths(root, { tmpDir: isolatedTmp });
let ownedDaemonPid = null;
const baseEnv = {
  ...process.env,
  RTS_INTERACT_DRIVER_FACTORY_MODULE: "tests/fixtures/interact_fake_driver.mjs",
  RTS_INTERACT_IDLE_MS: "5000",
  RTS_INTERACT_FAKE_OPEN_DELAY_MS: "250",
  RTS_INTERACT_TEST_TAILNET_PREVIEW_HOST: "127.0.0.1",
  RTS_INTERACT_TEST_TAILNET_PREVIEW_ROOT: previewRoot,
  RTS_INTERACT_TEST_TAILNET_PREVIEW_PORT: String(previewPort),
};

try {
  assert.equal(configuredIdleMs({}), DEFAULT_IDLE_MS, "the production idle default is exactly 30 minutes");
  assert.equal(DEFAULT_IDLE_MS, 30 * 60_000, "idle default remains explicit and reviewable");
  assert.equal(configuredIdleMs({ RTS_INTERACT_IDLE_MS: "25" }), 25, "tests may override the idle deadline");
  assert.throws(() => configuredIdleMs({ RTS_INTERACT_IDLE_MS: "0" }), /must be an integer/, "invalid idle overrides are rejected");

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
    assert.deepEqual(helpEnvelope.result.namespaces.map(({ name }) => name), ["lab", "game", "dev-scenario"], `${helpCommand} lists every supported namespace`);
    assert.equal(helpEnvelope.result.usage, "node scripts/interact/cli.mjs <lab|game|dev-scenario> <command> [JSON-object]", `${helpCommand} requires a namespace`);
  }
  for (const args of [["lab", "--help"], ["help", "lab"]]) {
    const help = spawnSync(process.execPath, [cli, ...args], {
      cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
    });
    assert.equal(help.status, 0, `${args.join(" ")} succeeds without a Git workspace`);
    const result = JSON.parse(help.stdout).result;
    assert.equal(result.namespace, "lab", `${args.join(" ")} identifies the namespace`);
    assert.ok(result.commands.includes("open"), `${args.join(" ")} lists open`);
    assert.ok(result.commands.includes("shutdown"), `${args.join(" ")} lists shutdown`);
  }
  for (const args of [["game", "--help"], ["help", "game"]]) {
    const help = spawnSync(process.execPath, [cli, ...args], {
      cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
    });
    assert.equal(help.status, 0, `${args.join(" ")} succeeds without a Git workspace`);
    const result = JSON.parse(help.stdout).result;
    assert.equal(result.namespace, "game", `${args.join(" ")} identifies the game namespace`);
    assert.deepEqual(
      result.commands,
      ["open", "close", "status", "inspect", "select", "move", "camera", "screenshot", "record-start", "record-stop", "record-wait", "capture-timelapse", "capture-cancel", "give-up", "shutdown"],
      "game help exposes bounded match observation, player controls, and region-aware media commands",
    );
  }
  for (const args of [["dev-scenario", "--help"], ["help", "dev-scenario"]]) {
    const help = spawnSync(process.execPath, [cli, ...args], {
      cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
    });
    assert.equal(help.status, 0, `${args.join(" ")} succeeds without a Git workspace`);
    const result = JSON.parse(help.stdout).result;
    assert.equal(result.namespace, "dev-scenario", `${args.join(" ")} identifies the dev-scenario namespace`);
    assert.deepEqual(
      result.commands,
      ["open", "close", "status", "inspect", "select", "camera", "screenshot", "record-start", "record-stop", "record-wait", "capture-timelapse", "capture-cancel", "shutdown"],
      "dev-scenario help exposes observation, framing, and media without gameplay mutations",
    );
  }
  const retiredScenarioNamespace = spawnSync(process.execPath, [cli, "scenario", "--help"], {
    cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
  });
  assert.notEqual(retiredScenarioNamespace.status, 0, "the retired scenario namespace is rejected");
  assert.equal(JSON.parse(retiredScenarioNamespace.stderr).error.code, "unknownNamespace", "the namespace rename does not leave a hidden alias");
  for (const inheritedName of ["constructor", "toString", "__proto__"]) {
    const inheritedNamespace = spawnSync(process.execPath, [cli, inheritedName, "--help"], {
      cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
    });
    assert.notEqual(inheritedNamespace.status, 0, `inherited object name ${inheritedName} is not a namespace`);
    assert.equal(JSON.parse(inheritedNamespace.stderr).error.code, "unknownNamespace", `${inheritedName} fails at the namespace boundary`);
    assert.equal(namespaceCommandKey(inheritedName, "open"), null, `${inheritedName} cannot resolve an internal command`);
    assert.equal(commandDefinition(inheritedName), null, `${inheritedName} cannot resolve an inherited registry member`);
  }
  assert.deepEqual(Object.keys(INTERACT_COMMAND_HELP).sort(), [...INTERACT_COMMANDS].sort(), "help descriptor coverage equals the public command catalog");
  assert.deepEqual(
    INTERACT_COMMAND_HELP.screenshot.variants,
    ["presentation=clean hides UI chrome", "presentation=normal retains visible Lab panels and game UI", "response.preview.url is the user-delivery URL; local capture paths are withheld"],
    "screenshot help explains presentation modes and the required Tailnet delivery URL",
  );
  assert.ok(
    INTERACT_COMMAND_HELP.order.variants.some((variant) => variant.includes("adjustProductionRepeat")),
    "order help exposes signed repeat-production adjustments and their multi-building shape",
  );
  assert.ok(
    INTERACT_COMMAND_HELP.open.defaults.includes("map=1v1"),
    "Lab open help advertises the current default 1v1 map",
  );
  for (const command of INTERACT_COMMANDS) {
    for (const args of [["lab", "help", command], ["lab", command, "--help"], ["help", "lab", command]]) {
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
  const unknownHelp = spawnSync(process.execPath, [cli, "lab", "help", "not-a-command"], {
    cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
  });
  assert.notEqual(unknownHelp.status, 0, "unknown per-command help remains a concise failure");
  assert.equal(JSON.parse(unknownHelp.stderr).error.code, "unknownCommand", "unknown help reports unknownCommand without Git or daemon access");
  const overlongNamespaceHelp = spawnSync(process.execPath, [cli, "help", "lab", "open", "{}"], {
    cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
  });
  assert.notEqual(overlongNamespaceHelp.status, 0, "root-qualified help never dispatches a command payload");
  assert.equal(JSON.parse(overlongNamespaceHelp.stderr).error.code, "usage", "overlong root-qualified help fails closed before Git or daemon access");
  const missingNamespace = spawnSync(process.execPath, [cli, "status", "{}"], {
    cwd: path.dirname(root), env: baseEnv, encoding: "utf8",
  });
  assert.notEqual(missingNamespace.status, 0, "bare commands are rejected instead of silently targeting Lab");
  assert.equal(JSON.parse(missingNamespace.stderr).error.code, "unknownNamespace", "bare commands report the required namespace");
  assert.equal(fs.existsSync(paths.directory), false, "help does not start a daemon or create runtime state");

  const failedOpenEnv = { ...baseEnv, RTS_INTERACT_FAKE_OPEN_FAILURE: "1" };
  const failedOpen = callFailure("open", {}, failedOpenEnv);
  assert.equal(failedOpen.error.code, "injectedOpenFailure", "a failed cold open preserves its corrective error");
  assert.equal(call("status", {}, failedOpenEnv).result.opening, false, "a failed cold open clears opening state and leaves the daemon reachable");
  call("shutdown", {}, failedOpenEnv);
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "failed-open daemon shuts down cleanly");

  const coldOpen = call("open");
  assert.match(coldOpen.result.sessionId, /^lab_[a-f0-9]{32}$/, "a cold first open returns one complete JSON envelope");
  assert.equal(coldOpen.result.status.map, "1v1", "a blank Lab open uses the current default 1v1 map");
  assert.equal(call("status").result.daemonCheckout.matches, true, "matching checkout status is explicit");
  const currentCheckoutCommit = readState(paths).checkoutCommit;
  assert.equal(call("status").result.opening, false, "completed cold open clears the opening status");
  call("close", { sessionId: coldOpen.result.sessionId });
  call("shutdown");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "cold-first-open daemon shuts down cleanly");

  const gameOpened = callNamespace("game", "open", { opponent: "ai_turtle" }).result;
  const gameSessionId = gameOpened.sessionId;
  assert.match(gameSessionId, /^game_[a-f0-9]{32}$/, "game open returns a distinct bounded session id");
  assert.equal(gameOpened.kind, "game", "game open identifies the isolated match kind");
  assert.deepEqual(gameOpened.capabilities.orders, ["move"], "game capabilities expose move as the only gameplay order");
  assert.equal(gameOpened.capabilities.selection, true, "game capabilities advertise browser-local selection");
  assert.deepEqual(gameOpened.capabilities.media, ["screenshot", "recording"], "player capabilities omit spectator-only time-lapse capture");
  const gameInspection = callNamespace("game", "inspect", { sessionId: gameSessionId }).result;
  assert.deepEqual(gameInspection.entities.map(({ id }) => id), [100], "game inspect defaults to locally owned fog-filtered entities");
  assert.equal(gameInspection.ui.hudVisible, true, "game inspect returns semantic UI state");
  assert.deepEqual(
    callNamespace("game", "select", { sessionId: gameSessionId, ids: [100] }).result.selection,
    [100],
    "game select targets a visible unit without issuing a gameplay order",
  );
  const moved = callNamespace("game", "move", { sessionId: gameSessionId, units: [100], x: 700, y: 700 }).result;
  assert.equal(moved.result.accepted, true, "game move uses the bounded normal command surface");
  assert.equal(
    callNamespaceFailure("game", "move", { sessionId: gameSessionId, units: [101], x: 700, y: 700 }).error.code,
    "notControllable",
    "game move rejects non-owned units",
  );
  const gameScreenshot = callNamespace("game", "screenshot", { sessionId: gameSessionId, name: "game-ui" }).result;
  assert.equal(gameScreenshot.presentation, "normal", "game screenshots retain the UI by default");
  assert.equal(gameScreenshot.preview.available, true, "game screenshots publish a Tailnet preview");
  const gameRecording = callNamespace("game", "record-start", { sessionId: gameSessionId, name: "game-ui", maxDurationMs: 1000 }).result;
  assert.equal(gameRecording.recorder.presentation, "normal", "game recordings retain the UI by default");
  callNamespace("game", "record-stop", { sessionId: gameSessionId });
  const surrendered = callNamespace("game", "give-up", { sessionId: gameSessionId }).result;
  assert.equal(surrendered.result.phase, "concluded", "game give-up reaches the concluded score state");
  assert.equal(callNamespaceFailure("lab", "open").error.code, "sessionKindMismatch", "an active game session cannot be mistaken for Lab");
  callNamespace("game", "close", { sessionId: gameSessionId });
  const spectatorOpened = callNamespace("game", "open", {
    spectate: ["ai_2_1", "ai_turtle"],
    autoSpectator: true,
  }).result;
  assert.equal(spectatorOpened.capabilities.role, "spectator", "AI-vs-AI open reports the spectator role");
  assert.equal(spectatorOpened.status.autoSpectatorEnabled, true, "AI-vs-AI open proves fight-following mode is active");
  assert.deepEqual(spectatorOpened.capabilities.orders, [], "AI-vs-AI spectators receive no gameplay orders");
  assert.deepEqual(spectatorOpened.capabilities.media, ["screenshot", "recording", "timelapse"], "spectator capabilities advertise time-lapse capture");
  const spectatorInspection = callNamespace("game", "inspect", { sessionId: spectatorOpened.sessionId }).result;
  assert.deepEqual(spectatorInspection.entities.map(({ id }) => id), [100, 101], "spectator inspection defaults to all visible entities");
  assert.deepEqual(
    callNamespace("game", "select", { sessionId: spectatorOpened.sessionId, ids: [101] }).result.selection,
    [101],
    "AI-vs-AI spectators can select a visible AI unit",
  );
  const overview = callNamespace("game", "camera", { sessionId: spectatorOpened.sessionId, camera: { action: "overview" } }).result;
  assert.deepEqual(overview.camera.focus, { x: 1024, y: 1024 }, "overview camera frames the complete map");
  const minimap = callNamespace("game", "screenshot", { sessionId: spectatorOpened.sessionId, name: "minimap", region: "minimap" }).result;
  assert.equal(minimap.region.preset, "minimap", "game screenshots report the resolved capture region");
  const timelapse = callNamespace("game", "capture-timelapse", {
    sessionId: spectatorOpened.sessionId, name: "whole-map", maxDurationMs: 1000,
    sampleEveryMs: 500, fps: 30, speed: 8, region: "viewport",
  }).result;
  assert.equal(timelapse.frameSummary.count, 2, "time-lapse returns a compact sampled-frame summary");
  assert.equal(timelapse.preview.available, true, "time-lapse publishes a Tailnet video preview");
  assert.equal(callNamespaceFailure("game", "move", { sessionId: spectatorOpened.sessionId, units: [100], x: 700, y: 700 }).error.code, "playerSeatRequired", "spectator sessions cannot issue player moves");
  callNamespace("game", "close", { sessionId: spectatorOpened.sessionId });
  callNamespace("game", "shutdown");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "game contract daemon shuts down cleanly");

  const scenarioOpened = callNamespace("dev-scenario", "open", {
    id: "direct_reverse_order", unit: "tank", count: 1,
  }).result;
  assert.match(scenarioOpened.sessionId, /^scenario_[a-f0-9]{32}$/, "scenario open returns a distinct bounded session id");
  assert.equal(scenarioOpened.kind, "scenario", "scenario open identifies the dev scenario kind");
  assert.equal(scenarioOpened.capabilities.role, "observer", "scenario capabilities describe the observation namespace rather than the underlying server seat");
  assert.deepEqual(scenarioOpened.capabilities.orders, [], "scenario sessions expose no gameplay orders");
  assert.equal(scenarioOpened.capabilities.giveUp, false, "scenario sessions expose no surrender mutation");
  assert.equal(scenarioOpened.capabilities.selection, true, "scenario capabilities advertise observation-only selection");
  assert.deepEqual(scenarioOpened.capabilities.media, ["screenshot", "recording", "timelapse"], "scenario sessions advertise every observation capture mode");
  const scenarioScreenshot = callNamespace("dev-scenario", "screenshot", {
    sessionId: scenarioOpened.sessionId, name: "before",
  }).result;
  assert.equal(scenarioScreenshot.presentation, "clean", "scenario screenshots use clean presentation by default");
  assert.equal(scenarioScreenshot.preview.available, true, "scenario screenshots publish a Tailnet preview");
  const scenarioInspection = callNamespace("dev-scenario", "inspect", {
    sessionId: scenarioOpened.sessionId, ids: [100], cameraViewport: true,
  }).result;
  assert.deepEqual(scenarioInspection.entities.map(({ id }) => id), [100], "dev-scenario inspect uses the shared visible-entity path");
  assert.deepEqual(
    callNamespace("dev-scenario", "select", { sessionId: scenarioOpened.sessionId, ids: [100] }).result.selection,
    [100],
    "dev-scenario select shares the observation selection path",
  );
  const scenarioCamera = callNamespace("dev-scenario", "camera", {
    sessionId: scenarioOpened.sessionId, camera: { action: "focus", entities: [100] },
  }).result;
  assert.deepEqual(scenarioCamera.camera.focus, { x: 512, y: 512 }, "dev-scenario camera shares normalized entity focus");
  const scenarioRecording = callNamespace("dev-scenario", "record-start", {
    sessionId: scenarioOpened.sessionId, name: "full-run", maxDurationMs: 1000,
  }).result;
  assert.equal(scenarioRecording.recorder.presentation, "clean", "scenario recordings use clean presentation by default");
  callNamespace("dev-scenario", "record-stop", { sessionId: scenarioOpened.sessionId });
  const scenarioTimelapse = callNamespace("dev-scenario", "capture-timelapse", {
    sessionId: scenarioOpened.sessionId, name: "pathing", maxDurationMs: 1000, sampleEveryMs: 500,
  }).result;
  assert.equal(scenarioTimelapse.preview.available, true, "scenario time-lapses publish a Tailnet video preview");
  assert.equal(
    callNamespaceFailure("game", "move", { sessionId: scenarioOpened.sessionId, units: [100], x: 700, y: 700 }).error.code,
    "sessionKindMismatch",
    "scenario sessions cannot inherit game commands",
  );
  callNamespace("dev-scenario", "close", { sessionId: scenarioOpened.sessionId });
  callNamespace("dev-scenario", "shutdown");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "scenario contract daemon shuts down cleanly");

  const mismatchEnv = { ...baseEnv, RTS_INTERACT_TEST_CHECKOUT_COMMIT: "b".repeat(40) };
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
  assert.equal(protectedMismatch.error.details.recoveryCommand, "node scripts/interact/cli.mjs lab shutdown", "active mismatch reports the explicit recovery command");
  assert.equal(processAlive(matchingState.pid), true, "active mismatch leaves the daemon and scene alive");
  assert.equal(call("shutdown").result.shuttingDown, true, "shutdown remains available across checkout mismatch");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "explicit mismatched shutdown completes");

  const afterExplicitRefresh = call("open").result;
  assert.notEqual(afterExplicitRefresh.sessionId, mismatchSession, "a deliberate refresh creates a new session id");
  assert.equal(callFailure("inspect", { sessionId: mismatchSession }).error.code, "unknownSession", "pre-refresh session ids become stale");
  call("close", { sessionId: afterExplicitRefresh.sessionId });
  call("shutdown");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "current daemon stops before pre-feature fixture startup");
  const preFeatureEnv = { ...baseEnv, RTS_INTERACT_TEST_OMIT_CHECKOUT: "1" };
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

  const invalidConfiguration = spawnSync(process.execPath, [cli, "lab", "status", "{}"], {
    cwd: root,
    env: { ...baseEnv, RTS_INTERACT_IDLE_MS: "0" },
    encoding: "utf8",
  });
  assert.notEqual(invalidConfiguration.status, 0, "invalid daemon configuration fails before spawning a child");
  assert.equal(JSON.parse(invalidConfiguration.stderr).error.code, "invalidConfiguration", "invalid daemon configuration stays actionable");
  assert.equal(fs.existsSync(paths.directory), false, "invalid daemon configuration creates no runtime lease");

  const brokenFactoryStartedAt = Date.now();
  const brokenFactory = spawnSync(process.execPath, [cli, "lab", "status", "{}"], {
    cwd: root,
    env: { ...baseEnv, RTS_INTERACT_DRIVER_FACTORY_MODULE: "tests/fixtures/missing_interact_driver.mjs" },
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
  const daemonProbe = await rawRequest(paths.socket, { protocolVersion: IPC_VERSION, probe: "interact" });
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

  const opening = execFileAsync(process.execPath, [cli, "lab", "open", "{}"], { cwd: root, env: baseEnv });
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
  const firstSessionId = testArtifacts.ownSession(opened.result.sessionId);
  assert.match(firstSessionId, /^lab_[a-f0-9]{32}$/, "open returns a bounded opaque session id");
  const repeatedOpen = call("open");
  assert.equal(repeatedOpen.result.sessionId, firstSessionId, "repeated open idempotently returns the active session");
  call("close", { sessionId: firstSessionId });
  const staleAfterClose = callFailure("inspect", { sessionId: firstSessionId });
  assert.equal(staleAfterClose.error.code, "unknownSession", "a closed session id becomes stale immediately");
  const concurrentOpen = await Promise.all([
    execFileAsync(process.execPath, [cli, "lab", "open", "{}"], { cwd: root, env: baseEnv }),
    execFileAsync(process.execPath, [cli, "lab", "open", "{}"], { cwd: root, env: baseEnv }),
  ]);
  const concurrentIds = concurrentOpen.map(({ stdout, stderr }) => {
    assert.equal(stderr, "", "concurrent open writes no stderr");
    return JSON.parse(stdout).result.sessionId;
  });
  assert.equal(new Set(concurrentIds).size, 1, "concurrent opens coalesce on one driver and session");
  const sessionId = concurrentIds[0];
  testArtifacts.ownSession(sessionId);
  assert.notEqual(sessionId, firstSessionId, "close followed by open creates a fresh session id");

  call("spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
    { owner: 1, kind: "factory", x: 960, y: 1248, alias: "factory_a" },
    { owner: 1, kind: "factory", x: 1248, y: 1248, alias: "factory_b" },
  ] });
  const inspected = call("inspect", { sessionId, refs: ["shooter", "target"], limit: 2 });
  assert.deepEqual(inspected.result.entities.map((entity) => entity.alias).sort(), ["shooter", "target"], "aliases persist across CLI invocations");
  const labSelection = call("select", { sessionId, refs: ["shooter", "target"] });
  assert.deepEqual(labSelection.result.selection, [100, 101], "Lab select resolves aliases and replaces browser-local selection");
  assert.deepEqual(labSelection.result.entities.map((entity) => entity.alias), ["shooter", "target"], "Lab select returns decorated selected entities");
  assert.deepEqual(call("inspect", { sessionId, limit: 4 }).result.selection, [100, 101], "Lab inspect reports browser-local selection ids");
  const drag = call("drag", {
    sessionId,
    button: "left",
    from: { x: 120, y: 140 },
    to: { x: 520, y: 340 },
    steps: 60,
    durationMs: 2000,
    holdKeys: ["attack", "shift"],
  });
  assert.deepEqual(
    {
      button: drag.result.result.button,
      from: drag.result.result.from,
      to: drag.result.result.to,
      steps: drag.result.result.steps,
      durationMs: drag.result.result.durationMs,
      holdKeys: drag.result.result.holdKeys,
    },
    {
      button: "left",
      from: { x: 120, y: 140 },
      to: { x: 520, y: 340 },
      steps: 60,
      durationMs: 2000,
      holdKeys: ["attack", "shift"],
    },
    "Lab drag preserves one bounded viewport gesture through the command service",
  );
  assert.equal(
    callFailure("drag", { sessionId, from: { x: 0, y: 0 }, to: { x: 10, y: 10 }, holdKeys: ["Escape"] }).error.code,
    "invalidInput",
    "Lab drag rejects arbitrary keyboard input",
  );
  assert.equal(
    callFailure("drag", { sessionId, from: { x: 0, y: 0 }, to: { x: 10, y: 10 }, durationMs: 10_001 }).error.code,
    "invalidInput",
    "Lab drag duration remains hard bounded",
  );
  assert.equal(
    callFailure("order", {
      sessionId,
      playerId: 1,
      command: { c: "adjustProductionRepeat", buildings: ["factory_a", "factory_b"], unit: "tank", delta: 0 },
    }).error.code,
    "invalidInput",
    "production repeat adjustments reject deltas outside -1 or 1",
  );
  const repeatAdjustment = call("order", {
    sessionId,
    playerId: 1,
    command: { c: "adjustProductionRepeat", buildings: ["factory_a", "factory_b"], unit: "tank", delta: 1 },
  });
  assert.deepEqual(repeatAdjustment.result.command.buildings, [102, 103], "production repeat orders resolve building aliases");
  assert.equal(repeatAdjustment.result.command.delta, 1, "production repeat orders preserve the signed delta");
  call("remove", { sessionId, refs: ["factory_a", "factory_b"] });
  const screenshot = call("screenshot", {
    sessionId,
    name: "cli-contract",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjects: ["shooter", "target"],
  });
  assert.equal(screenshot.result.image.mimeType, "image/png", "screenshot returns bounded PNG metadata");
  assert.equal("data" in screenshot.result.image, false, "CLI screenshot responses never embed image bytes");
  assert.equal("pngPath" in screenshot.result, false, "CLI withholds raw screenshot paths in favor of the delivery URL");
  assert.equal("manifestPath" in screenshot.result, false, "CLI withholds raw screenshot manifest paths");
  assert.equal(screenshot.result.preview.available, true, "screenshot creates a Tailnet preview");
  assert.match(screenshot.result.preview.url, new RegExp(`^http://127\\.0\\.0\\.1:${previewPort}/p/[A-Za-z0-9_-]{16,64}/cli-contract\\.png$`), "screenshot emits an opaque durable Tailnet preview URL");
  assert.ok(screenshot.result.preview.expiresAt >= Date.now() + 24 * 60 * 60 * 1000 - 1_000, "screenshot preview is retained for at least 24 hours");
  assert.match(screenshot.result.preview.instruction, /Share this Tailnet URL/, "screenshot tells the caller to share the Tailnet preview");
  assert.equal(screenshot.result.preview.url.includes("target/interact/lab"), false, "preview URLs never reveal filesystem paths");
  const screenshotPreview = await fetch(screenshot.result.preview.url);
  assert.equal(screenshotPreview.status, 200, "screenshot Tailnet preview serves the capture");
  assert.equal(screenshotPreview.headers.get("content-type"), "image/png", "screenshot Tailnet preview preserves the image type");
  assert.ok((await screenshotPreview.arrayBuffer()).byteLength > 0, "screenshot Tailnet preview has image bytes");
  assert.equal(callFailure("record-wait", { sessionId }).error.code, "recordingInactive", "a never-started CLI wait fails clearly");

  const recordingStarted = call("record-start", {
    sessionId, name: "cli-contract-motion", maxDurationMs: 5_000,
    viewport: { width: 800, height: 600, deviceScaleFactor: 1 }, scale: 0.5,
  });
  assert.equal(recordingStarted.result.recorder.active, true, "record-start exposes one active session recorder");
  assert.equal(call("status", { sessionId }).result.recorder.active, true, "session status exposes active recorder state");
  assert.equal(callFailure("record-start", { sessionId, name: "duplicate" }).error.code, "recordingActive", "duplicate recording starts are correctable errors");
  const recordingWaitProcess = execFileAsync(process.execPath, [cli, "lab", "record-wait", JSON.stringify({ sessionId })], { cwd: root, env: baseEnv });
  await waitFor(() => readState(paths)?.activeRequests === 1, 1_000, "record-wait is admitted outside the session mutation queue");
  call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1100, y: 1000 } });
  const stepped = call("time", { sessionId, control: { action: "step", ticks: 3 } });
  const recordingStopped = call("record-stop", { sessionId });
  const { stdout: recordingWaitStdout, stderr: recordingWaitStderr } = await recordingWaitProcess;
  assert.equal(recordingWaitStderr, "", "successful record-wait keeps stderr empty");
  assert.deepEqual(JSON.parse(recordingWaitStdout).result, recordingStopped.result, "concurrent record-wait returns the same finalized artifact as record-stop");
  assert.equal(recordingStopped.result.probe.codec, "h264", "record-stop returns bounded H.264 MP4 probe metadata");
  assert.equal("videoPath" in recordingStopped.result, false, "record-stop withholds raw video paths");
  assert.equal("framePaths" in recordingStopped.result, false, "record-stop withholds raw frame paths");
  assert.equal("contactSheetPath" in recordingStopped.result, false, "record-stop withholds raw contact-sheet paths");
  assert.equal(recordingStopped.result.preview.available, true, "record-stop emits a Tailnet video preview");
  assert.equal(recordingStopped.result.preview.mimeType, "video/mp4", "record-stop preview identifies mobile MP4 media");
  assert.equal(recordingStopped.result.preview.poster.available, true, "record-stop emits a Tailnet poster preview");
  assert.equal(recordingStopped.result.frames.count, 2, "record-stop keeps a bounded representative-frame count");
  const recordingRange = await fetch(recordingStopped.result.preview.url, { headers: { Range: "bytes=0-3" } });
  assert.equal(recordingRange.status, 206, "video previews support byte ranges for mobile playback");
  assert.equal(recordingRange.headers.get("content-type"), "video/mp4", "video range response preserves the media type");
  assert.deepEqual(call("record-wait", { sessionId }).result, recordingStopped.result, "completed record-wait remains idempotent");
  assert.equal(callFailure("record-stop", { sessionId }).error.code, "recordingInactive", "duplicate recording stops are correctable errors");
  assert.equal(callFailure("record-start", { sessionId, maxDurationMs: 60_001 }).error.code, "invalidInput", "recording duration remains hard bounded");
  assert.equal(call("record-start", { sessionId, name: "one-minute-bound", maxDurationMs: 60_000 }).result.recorder.maxDurationMs, 60_000, "CLI accepts the exact one-minute recording bound");
  call("record-stop", { sessionId });
  assert.equal(callFailure("record-start", { sessionId, crop: { x: 0, y: 0, width: 1, height: 10 } }).error.code, "invalidInput", "recording crop dimensions remain bounded");

  const fixed = call("capture-fixed", { sessionId, name: "cli-fixed", fps: 60, frameCount: 3 });
  assert.deepEqual(
    { count: fixed.result.frameSummary.count, representatives: fixed.result.frameSummary.representativeFrames },
    { count: 3, representatives: 3 },
    "capture-fixed summarizes frames without exposing local representative PNG paths",
  );
  assert.equal("videoPath" in fixed.result, false, "capture-fixed withholds its raw video path");
  assert.equal(fixed.result.preview.available, true, "capture-fixed emits a Tailnet video preview");
  assert.equal(fixed.result.preview.poster.available, true, "capture-fixed emits a Tailnet contact-sheet preview");
  assert.deepEqual(
    fixed.result.authoritative,
    { startTick: stepped.result.result.snapshotTick, endTick: stepped.result.result.snapshotTick + 1 },
    "capture-fixed reports its authoritative tick range",
  );
  assert.equal("representativeFramePaths" in fixed.result.frameSummary, false, "fixed capture omits raw representative frame paths");
  assert.equal(callFailure("capture-fixed", { sessionId, fps: 61, frameCount: 1 }).error.code, "invalidInput", "fixed capture FPS remains bounded");

  const setupExport = call("export", { sessionId, kind: "setup", name: "Aliased fixture", reproduction: true });
  assert.match(setupExport.result.artifactId, /^artifact_[a-f0-9]{32}$/, "setup export returns an opaque artifact id");
  assert.equal("checkpointPayload" in setupExport.result, false, "setup export never prints checkpoint bytes");
  assert.match(setupExport.result.path, /target\/interact\/lab\/artifacts\//, "portable Lab artifacts stay under the namespaced worktree target root");
  const setupInspect = call("artifact-inspect", { sessionId, artifactId: setupExport.result.artifactId });
  assert.equal(setupInspect.result.kind, "setup", "artifact inspection derives the stored kind");
  assert.equal(setupInspect.result.aliasCount, 2, "artifact inspection reports bounded alias metadata");
  const setupImport = call("import", { sessionId, kind: "setup", artifactId: setupExport.result.artifactId });
  assert.deepEqual(setupImport.result.aliases.stale, { count: 0, details: [], truncated: false }, "setup import reconciles aliases through the authoritative id map");
  assert.equal(setupImport.result.aliases.restored.count, 2, "setup import reports the restored alias total");
  assert.deepEqual(call("inspect", { sessionId, refs: ["shooter", "target"] }).result.entities.map((entity) => entity.id).sort(), [1100, 1101], "remapped aliases resolve after setup import");
  const unsafeImport = callFailure("import", { sessionId, kind: "setup", path: "/etc/passwd" });
  assert.equal(unsafeImport.error.code, "unsafeArtifactPath", "imports reject paths outside target/interact/lab");

  const replayExport = call("export", { sessionId, kind: "replay", name: "Fixture replay" });
  assert.equal(replayExport.result.operationCount, 0, "replay export reports operation count without embedding operations");
  assert.equal(call("import", { sessionId, kind: "replay", artifactId: replayExport.result.artifactId }).result.validation.ok, true, "replay import reports authoritative rebuild validation");

  const unexpected = callFailure("inspect", { sessionId, unexpected: true });
  assert.equal(unexpected.error.code, "invalidInput", "exact input shapes reject unknown fields");
  const unsafeName = callFailure("screenshot", { sessionId, name: "../escape" });
  assert.equal(unsafeName.error.code, "invalidInput", "artifact paths cannot be injected through names");
  const invalidJson = spawnSync(process.execPath, [cli, "lab", "status", "not-json"], { cwd: root, env: baseEnv, encoding: "utf8" });
  assert.notEqual(invalidJson.status, 0, "invalid CLI JSON exits nonzero");
  assert.equal(invalidJson.stdout, "", "failed commands never write stdout");
  assert.equal(JSON.parse(invalidJson.stderr).ok, false, "failed commands write one concise JSON error to stderr");

  call("close", { sessionId });
  const explicit = call("shutdown");
  assert.equal(explicit.result.shuttingDown, true, "shutdown acknowledges immediate teardown");
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "shutdown removes socket, state, and runtime files");
  assert.equal((await fetch(screenshot.result.preview.url)).status, 200, "Lab daemon shutdown does not invalidate an issued preview URL");

  const unavailablePreviewEnv = { ...baseEnv, RTS_INTERACT_TEST_TAILNET_PREVIEW_HOST: "not-a-loopback-host" };
  const unavailableSession = call("open", {}, unavailablePreviewEnv).result.sessionId;
  const unavailablePreview = call("screenshot", { sessionId: unavailableSession, name: "no-tailnet" }, unavailablePreviewEnv).result;
  assert.equal(unavailablePreview.preview.available, false, "a missing Tailnet listener leaves the visual capture successful but undeliverable");
  assert.equal("pngPath" in unavailablePreview, false, "an unavailable preview still withholds the raw local screenshot path");
  assert.match(unavailablePreview.preview.instruction, /Do not share a local file path/, "an unavailable preview prevents fallback local-file delivery");
  call("shutdown", {}, unavailablePreviewEnv);
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "unavailable-preview daemon still shuts down cleanly");
  stopPreviewService();
  await assert.rejects(
    fetch(screenshot.result.preview.url),
    /fetch failed/,
    "explicit preview-service shutdown terminates the detached loopback server",
  );

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

  const raceEnv = { ...baseEnv, RTS_INTERACT_IDLE_MS: "5000", RTS_INTERACT_TEST_STARTUP_DELAY_MS: "250" };
  const firstRace = execFileAsync(process.execPath, [cli, "lab", "status"], { cwd: root, env: raceEnv });
  await waitFor(() => fs.existsSync(paths.lock), 1000, "first startup publishes its ownership lock");
  const secondRace = execFileAsync(process.execPath, [cli, "lab", "status"], { cwd: root, env: raceEnv });
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
  const lateChild = spawn(process.execPath, [path.join(root, "scripts/interact/daemon.ts"), root, lateNonce], {
    cwd: root,
    env: { ...baseEnv, RTS_INTERACT_TEST_PREBIND_DELAY_MS: "250" },
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
  const duplicate = spawnSync(process.execPath, [path.join(root, "scripts/interact/daemon.ts"), root, duplicateNonce], {
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

  const inFlightEnv = { ...baseEnv, RTS_INTERACT_IDLE_MS: "1000", RTS_INTERACT_FAKE_DELAY_MS: "1500" };
  const inFlightSession = call("open", {}, inFlightEnv).result.sessionId;
  const delayed = execFileAsync(process.execPath, [cli, "lab", "spawn", JSON.stringify({
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

  const idleEnv = { ...baseEnv, RTS_INTERACT_IDLE_MS: "80" };
  call("status", {}, idleEnv);
  const idlePid = JSON.parse(fs.readFileSync(paths.state, "utf8")).pid;
  await waitFor(() => !fs.existsSync(paths.directory), 2000, "idle daemon deletes its runtime directory");
  await waitFor(() => !processAlive(idlePid), 2000, "idle daemon exits after closing owned resources");

  console.log("✅ interact_cli_contracts.mjs: CLI, daemon, validation, aliases, races, stale recovery, idle cleanup, and shutdown passed");
} finally {
  shutdown(baseEnv);
  await stopOwnedDaemon();
  stopPreviewService();
  testArtifacts.cleanup();
  testArtifacts.assertClean();
  restoreTmp();
}

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close((error) => error ? reject(error) : resolve(address.port));
    });
  });
}

function stopPreviewService() {
  const result = spawnSync(process.execPath, [tailnetPreviewCli, "--stop", "--root", previewRoot, "--port", String(previewPort)], {
    cwd: root,
    env: baseEnv,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, `preview service stops cleanly: ${result.stderr}`);
}

function call(command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, "lab", command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  trackDaemon();
  assert.equal(result.status, 0, `${command} succeeds: ${result.stderr}`);
  assert.equal(result.stderr, "", `${command} writes no stderr on success`);
  const lines = result.stdout.trim().split("\n");
  assert.equal(lines.length, 1, `${command} writes exactly one JSON value to stdout`);
  const response = JSON.parse(lines[0]);
  assert.equal(response.ok, true, `${command} returns a successful envelope`);
  if (command === "open") testArtifacts.ownSession(response.result.sessionId);
  if (command === "export") testArtifacts.ownPortableArtifact(response.result);
  return response;
}

function callFailure(command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, "lab", command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  trackDaemon();
  assert.notEqual(result.status, 0, `${command} fails for rejected input`);
  assert.equal(result.stdout, "", `${command} failure keeps stdout empty`);
  const lines = result.stderr.trim().split("\n");
  assert.equal(lines.length, 1, `${command} failure writes exactly one JSON value to stderr`);
  return JSON.parse(lines[0]);
}

function callNamespace(namespace, command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, namespace, command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  trackDaemon();
  assert.equal(result.status, 0, `${namespace} ${command} succeeds: ${result.stderr}`);
  assert.equal(result.stderr, "", `${namespace} ${command} writes no stderr on success`);
  const response = JSON.parse(result.stdout);
  assert.equal(response.ok, true, `${namespace} ${command} returns a successful envelope`);
  if (namespace === "game" && command === "open") testArtifacts.ownGameSession(response.result.sessionId);
  if (namespace === "dev-scenario" && command === "open") testArtifacts.ownScenarioSession(response.result.sessionId);
  return response;
}

function callNamespaceFailure(namespace, command, input = {}, env = baseEnv) {
  const result = spawnSync(process.execPath, [cli, namespace, command, JSON.stringify(input)], { cwd: root, env, encoding: "utf8" });
  trackDaemon();
  assert.notEqual(result.status, 0, `${namespace} ${command} fails for rejected input`);
  assert.equal(result.stdout, "", `${namespace} ${command} failure keeps stdout empty`);
  return JSON.parse(result.stderr);
}

function shutdown(env) {
  trackDaemon();
  spawnSync(process.execPath, [cli, "lab", "shutdown", "{}"], { cwd: root, env, encoding: "utf8" });
}

function trackDaemon() {
  const pid = readState(paths)?.pid;
  if (Number.isInteger(pid) && pid > 0) ownedDaemonPid = pid;
}

async function stopOwnedDaemon() {
  if (!ownedDaemonPid) return;
  const deadline = Date.now() + 5_000;
  while (processAlive(ownedDaemonPid) && Date.now() < deadline) await sleep(20);
  if (processAlive(ownedDaemonPid)) {
    try {
      process.kill(ownedDaemonPid, "SIGTERM");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  await waitFor(
    () => !processAlive(ownedDaemonPid),
    2_000,
    "contract cleanup exits the test-owned daemon",
  );
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
