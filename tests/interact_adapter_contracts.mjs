import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";

import { InteractService } from "../scripts/interact/command_service.ts";
import { InteractDriver, InteractDriverError } from "../scripts/interact/driver.ts";
import {
  PrivateServer, SERVER_BUILD_TIMEOUT_MS, privateLoopbackUrl,
} from "../scripts/interact/private_server.ts";
import { ProcessRunner } from "../scripts/interact/process_runner.ts";

const holdOpen = "process.on('SIGTERM',()=>{}); setInterval(()=>{},1000)";

await processRunnerContracts();
await dependencyPreflightContracts();
await coldOpenContracts();
await responsiveServiceContracts();

console.log("✅ interact_adapter_contracts.mjs: bounded processes, abort/reaping, and responsive lifecycle lanes passed");

async function processRunnerContracts() {
  const runner = new ProcessRunner({ maxOutputBytes: 128, termGraceMs: 30 });
  const shellMarker = path.join(os.tmpdir(), `interact-shell-${process.pid}-${Date.now()}`);
  const literalArg = `$(touch ${shellMarker})`;
  const result = await runner.run(process.execPath, [
    "-e",
    "process.stdout.write('a'.repeat(1024)); process.stderr.write('b'.repeat(1024)); console.log(process.argv[1])",
    literalArg,
  ], { timeoutMs: 1_000 });
  assert.equal(result.status, 0, "finite processes return deterministic exit status");
  assert.equal(result.stdoutTruncated, true, "stdout is bounded with explicit truncation metadata");
  assert.equal(result.stderrTruncated, true, "stderr is bounded with explicit truncation metadata");
  assert.ok(Buffer.byteLength(result.stdout) <= 128 && Buffer.byteLength(result.stderr) <= 128, "captured output never exceeds its byte cap");
  assert.ok(result.stdout.includes(literalArg), "argv metacharacters reach the child literally without a shell");
  assert.equal(fs.existsSync(shellMarker), false, "direct argv execution cannot invoke shell substitutions");

  let timeoutPid = null;
  const timeoutStarted = Date.now();
  await assert.rejects(
    runner.run(process.execPath, ["-e", holdOpen], {
      timeoutMs: 200,
      onSpawn: (child) => { timeoutPid = child.pid; },
    }),
    (error) => error?.code === "processTimeout" && error?.result?.signal === "SIGKILL",
    "timeouts send TERM and use bounded KILL fallback when the child refuses to exit",
  );
  assert.ok(Date.now() - timeoutStarted < 1_000, "timeout fallback remains bounded");
  assert.equal(processExists(timeoutPid), false, "timed-out children are reaped before rejection");

  const abortController = new AbortController();
  let abortPid = null;
  const aborted = runner.run(process.execPath, ["-e", "process.on('SIGTERM',()=>process.exit(0)); setInterval(()=>{},1000)"], {
    timeoutMs: 5_000,
    signal: abortController.signal,
    onSpawn: (child) => { abortPid = child.pid; },
  });
  abortController.abort();
  await assert.rejects(
    aborted,
    (error) => error?.code === "processAborted" && error?.result?.terminatedBy === "abort",
    "AbortSignal terminates and projects a stable abort error",
  );
  assert.equal(processExists(abortPid), false, "aborted children are reaped before rejection");
}

async function dependencyPreflightContracts() {
  let serverStarted = false;
  const driver = new InteractDriver({
    workspaceRoot: process.cwd(),
    map: "dependency-preflight",
    puppeteerLoader: async () => {
      throw new InteractDriverError(
        "puppeteerUnavailable",
        "puppeteer-core is not installed from the repository package lock; run npm ci at the repository root.",
      );
    },
    privateServerFactory: async () => {
      serverStarted = true;
      throw new Error("private server should not start before dependency preflight");
    },
  });
  try {
    await assert.rejects(driver.open(), (error) => error?.code === "puppeteerUnavailable", "missing Puppeteer reports its actionable dependency error");
    assert.equal(serverStarted, false, "missing browser tooling is detected before the private Rust build starts");
  } finally {
    if (driver.sessionDir) fs.rmSync(driver.sessionDir, { recursive: true, force: true });
  }
}

async function coldOpenContracts() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-private-server-"));
  const sessionDir = path.join(root, "session");
  fs.mkdirSync(sessionDir);
  const controller = new AbortController();
  const realRunner = new ProcessRunner({ termGraceMs: 30 });
  let cargoPid = null;
  const processRunner = {
    runOrThrow(_command, _args, options) {
      return realRunner.runOrThrow(process.execPath, ["-e", holdOpen], {
        ...options,
        onSpawn: (child) => { cargoPid = child.pid; },
      });
    },
  };
  const opening = PrivateServer.open({
    workspace: { root, head: "a".repeat(40) },
    sessionDir,
    startupTimeoutMs: 5_000,
    artifactCapability: "b".repeat(64),
    signal: controller.signal,
    processRunner,
    allocatePrivatePort: async () => 12345,
  });
  await waitFor(() => cargoPid != null, 500, "fake Cargo child starts");
  controller.abort();
  await assert.rejects(opening, (error) => error?.code === "sessionClosed", "private-server startup maps Cargo abort to session closure");
  assert.equal(processExists(cargoPid), false, "private-server abort reaps the held Cargo child");

  const timeoutRunner = new ProcessRunner({ termGraceMs: 30 });
  let buildPid = null;
  const buildStartedAt = Date.now();
  const timedOutBuild = PrivateServer.open({
    workspace: { root, head: "a".repeat(40) },
    sessionDir,
    startupTimeoutMs: 10,
    buildTimeoutMs: 80,
    artifactCapability: "b".repeat(64),
    processRunner: {
      runOrThrow(_command, _args, options) {
        return timeoutRunner.runOrThrow(process.execPath, ["-e", holdOpen], {
          ...options,
          onSpawn: (child) => { buildPid = child.pid; },
        });
      },
    },
    allocatePrivatePort: async () => 12345,
  });
  await assert.rejects(timedOutBuild, (error) => {
    assert.equal(error?.code, "serverBuildTimeout", "a bounded cold-build timeout is distinct from a compiler failure");
    assert.equal(error?.details?.timedOut, true, "build diagnostics identify the timeout");
    assert.equal(error?.details?.processCode, "processTimeout", "build diagnostics retain the process failure class");
    assert.equal(error?.details?.signal, "SIGKILL", "build diagnostics retain the final child signal");
    assert.ok(fs.existsSync(error?.details?.buildLog), "build diagnostics provide a readable log path");
    assert.match(error.message, /cold-build deadline/, "the timeout message does not claim a compiler error");
    return true;
  });
  assert.ok(Date.now() - buildStartedAt >= 80, "Cargo gets its independent build deadline rather than the shorter readiness timeout");
  assert.equal(processExists(buildPid), false, "timed-out Cargo children are reaped before the failure returns");
  assert.ok(SERVER_BUILD_TIMEOUT_MS > 60_000, "production cold builds receive multi-minute headroom");

  const failedBuild = PrivateServer.open({
    workspace: { root, head: "a".repeat(40) }, sessionDir, startupTimeoutMs: 10,
    artifactCapability: "b".repeat(64), allocatePrivatePort: async () => 12345,
    processRunner: {
      runOrThrow(_command, _args, options) {
        return realRunner.runOrThrow(process.execPath, ["-e", "process.stderr.write('compiler fixture failure'); process.exit(2)"], options);
      },
    },
  });
  await assert.rejects(failedBuild, (error) => {
    assert.equal(error?.code, "serverBuild", "a nonzero compiler exit remains a serverBuild failure");
    assert.equal(error?.details?.exitCode, 2, "compiler diagnostics retain the nonzero exit code");
    assert.match(fs.readFileSync(error?.details?.buildLog, "utf8"), /compiler fixture failure/, "the build log retains compiler stderr");
    return true;
  });
  assert.equal(privateLoopbackUrl("http://localhost:8080"), "http://localhost:8080/", "private-server reuse normalizes loopback URLs");
  assert.throws(() => privateLoopbackUrl("http://192.0.2.1:8080"), (error) => error?.code === "invalidServerUrl", "private-server reuse rejects non-loopback URLs");

  const serverOwner = new PrivateServer({
    workspace: { root, head: "a".repeat(40) },
    sessionDir,
    startupTimeoutMs: 5_000,
    artifactCapability: "b".repeat(64),
    serverTermGraceMs: 30,
  });
  serverOwner.child = spawn(process.execPath, ["-e", "process.on('SIGTERM',()=>{}); console.log('ready'); setInterval(()=>{},1000)"], {
    shell: false,
    stdio: ["ignore", "pipe", "ignore"],
  });
  const serverPid = serverOwner.child.pid;
  await once(serverOwner.child.stdout, "data");
  await serverOwner.close();
  assert.equal(processExists(serverPid), false, "private-server teardown KILLs and reaps a Rust child that ignores TERM");
  fs.rmSync(root, { recursive: true, force: true });
}

async function responsiveServiceContracts() {
  const openRunner = new ProcessRunner({ termGraceMs: 30 });
  let openPid = null;
  const service = new InteractService({
    workspaceRoot: process.cwd(),
    driverFactory: async ({ signal }) => {
      await openRunner.run(process.execPath, ["-e", holdOpen], {
        timeoutMs: 5_000,
        signal,
        onSpawn: (child) => { openPid = child.pid; },
      });
      throw new Error("held open unexpectedly completed");
    },
  });
  const opening = service.execute("open", {});
  void opening.catch(() => {});
  await waitFor(() => openPid != null, 500, "application-owned cold open starts");
  const statusStarted = Date.now();
  assert.equal((await service.execute("status", {})).opening, true, "status observes an in-progress cold open");
  assert.ok(Date.now() - statusStarted < 200, "status does not wait behind cold startup");
  const shutdownStarted = Date.now();
  await service.shutdown("contract");
  assert.ok(Date.now() - shutdownStarted < 1_000, "shutdown aborts cold startup instead of waiting for its ordinary timeout");
  await assert.rejects(opening, (error) => error?.code === "processAborted", "cold-open caller receives the aborted child result");
  assert.equal(processExists(openPid), false, "shutdown reaps the cold-open child");

  const mediaController = new AbortController();
  const mediaRunner = new ProcessRunner({ termGraceMs: 30 });
  let fixedCancelled = false;
  let mediaPid = null;
  const driver = {
    workspace: { root: process.cwd() },
    async status() { return { ready: true, snapshotTick: 1 }; },
    async catalog() { return { players: [] }; },
    recordingStatus() { return { active: mediaPid != null }; },
    fixedCaptureStatus() { return fixedCancelled ? { active: false } : { active: this.fixedActive === true }; },
    async recordStart() {
      await mediaRunner.run(process.execPath, ["-e", holdOpen], {
        timeoutMs: 5_000,
        signal: mediaController.signal,
        onSpawn: (child) => { mediaPid = child.pid; },
      });
    },
    async captureFixed() {
      this.fixedActive = true;
      while (!fixedCancelled) await new Promise((resolve) => setTimeout(resolve, 5));
      this.fixedActive = false;
      throw Object.assign(new Error("cancelled"), { code: "captureCancelled" });
    },
    cancelFixedCapture() { fixedCancelled = true; return { cancelling: true }; },
    async close() { mediaController.abort(); fixedCancelled = true; },
  };
  const mediaService = new InteractService({ workspaceRoot: process.cwd(), driverFactory: async () => driver });
  const opened = await mediaService.execute("open", {});
  const recording = mediaService.execute("record-start", { sessionId: opened.sessionId, name: "held" });
  void recording.catch(() => {});
  await waitFor(() => mediaPid != null, 500, "finite media stage starts");
  const mediaStatusStarted = Date.now();
  assert.equal((await mediaService.execute("status", { sessionId: opened.sessionId })).status.ready, true, "status remains available during a finite media child");
  assert.ok(Date.now() - mediaStatusStarted < 200, "media status does not wait behind the held process");
  mediaController.abort();
  await assert.rejects(recording, (error) => error?.code === "processAborted");

  const capture = mediaService.execute("capture-fixed", { sessionId: opened.sessionId, name: "held", fps: 30, frameCount: 1 });
  void capture.catch(() => {});
  await waitFor(() => driver.fixedActive === true, 500, "fixed capture starts");
  const cancellation = await mediaService.execute("capture-cancel", { sessionId: opened.sessionId });
  assert.equal(cancellation.cancelling, true, "capture cancellation reaches active fixed capture outside the semantic FIFO");
  await assert.rejects(capture, (error) => error?.code === "captureCancelled");
  await mediaService.shutdown("contract");
}

function processExists(pid) {
  if (!Number.isInteger(pid)) return false;
  try { process.kill(pid, 0); return true; } catch { return false; }
}

async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  assert.fail(message);
}
