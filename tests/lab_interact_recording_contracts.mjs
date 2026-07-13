import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { EventEmitter } from "node:events";
import { fileURLToPath } from "node:url";

import {
  LAB_INTERACT_LIMITS, LabInteractService, validateCommandInput,
} from "../scripts/lab-interact/command_service.mjs";
import {
  checkMediaCapabilities, createPngMp4Encoder, finalizeMp4Artifacts, LabInteractRecordingError,
  mediaAuxiliaryTimeoutMs, mediaStageTimeoutMs, recordingStopTimeoutMs, RECORDING_LIMITS,
  representativeFrameIndices,
} from "../scripts/lab-interact/recording.mjs";
import {
  RECORDING_REQUEST_TIMEOUT_MS, REQUEST_TIMEOUT_MS,
} from "../scripts/lab-interact/runtime.mjs";
import { requestTimeoutMs } from "../scripts/lab-interact/command_registry.mjs";
import { DRIVER_STATES, LabInteractDriver } from "../scripts/lab-interact/driver.mjs";
import { LAB_INTERACT_SUMMARY_LIMITS } from "../scripts/lab-interact/manifest_summary.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";
import { LabInteractTestArtifacts } from "./fixtures/lab_interact_test_artifacts.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const testArtifacts = new LabInteractTestArtifacts(root);
const fixtureSessionId = () => testArtifacts.createSessionId();
let service;
let shutdownService;

try {
  service = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
  const opened = await service.execute("open", {});
  const sessionId = testArtifacts.ownSession(opened.sessionId);

  assert.equal(RECORDING_LIMITS.defaultDurationMs, 10_000, "recordings default to a short review clip");
  assert.equal(RECORDING_LIMITS.maxDurationMs, 60_000, "recordings support a hard one-minute duration ceiling");
  assert.equal(RECORDING_LIMITS.maxBytes, 64 * 1024 * 1024, "recordings retain a hard 64 MiB file ceiling");
  assert.equal(LAB_INTERACT_LIMITS.maxRecordingOperations, 200, "recording action manifests remain bounded");
  assert.equal(RECORDING_LIMITS.maxOperations, LAB_INTERACT_LIMITS.maxRecordingOperations, "service and driver share the operation bound");
  assert.deepEqual(
    [...representativeFrameIndices(15)],
    [0, 3, 6, 8, 11, 14],
    "representative sampling is evenly spaced and includes both recording endpoints",
  );

  assert.throws(
    () => validateCommandInput("record-start", { sessionId, maxDurationMs: 60_001 }),
    (error) => error?.code === "invalidInput",
    "record-start rejects durations over the hard bound",
  );
  assert.doesNotThrow(
    () => validateCommandInput("record-start", { sessionId, maxDurationMs: 60_000, resumeSpeed: 1 }),
    "record-start accepts exactly one minute with atomic authoritative resume",
  );
  assert.throws(
    () => validateCommandInput("record-start", { sessionId, resumeSpeed: 0 }),
    (error) => error?.code === "invalidInput",
    "record-start rejects an invalid atomic resume speed",
  );
  assert.equal(recordingStopTimeoutMs(60_000), RECORDING_LIMITS.maxStopTimeoutMs, "one-minute recorder flush gets capped duration-derived headroom");
  assert.equal(mediaStageTimeoutMs(60_000), RECORDING_LIMITS.maxMediaStageTimeoutMs, "one-minute FFmpeg stages get capped duration-derived headroom");
  assert.equal(mediaAuxiliaryTimeoutMs(60_000), RECORDING_LIMITS.maxMediaAuxiliaryTimeoutMs, "one-minute auxiliary media stages stay separately capped");
  assert.equal(requestTimeoutMs("status"), REQUEST_TIMEOUT_MS, "ordinary daemon commands keep their existing IPC deadline");
  assert.equal(requestTimeoutMs("record-wait"), RECORDING_REQUEST_TIMEOUT_MS, "record-wait gets bounded recording-specific IPC headroom");
  assert.equal(requestTimeoutMs("capture-fixed"), RECORDING_REQUEST_TIMEOUT_MS, "minute-scale fixed capture gets bounded media IPC headroom");
  assert.ok(RECORDING_REQUEST_TIMEOUT_MS > REQUEST_TIMEOUT_MS, "recording IPC headroom exceeds the ordinary command deadline");
  const boundedMediaBudgetMs = RECORDING_LIMITS.maxDurationMs + RECORDING_LIMITS.maxStopTimeoutMs +
    RECORDING_LIMITS.maxMediaStageTimeoutMs + 3 * RECORDING_LIMITS.maxMediaAuxiliaryTimeoutMs + 22_000;
  assert.ok(
    RECORDING_REQUEST_TIMEOUT_MS >= boundedMediaBudgetMs + 60_000,
    "recording IPC deadline covers the bounded wait, media stages, probes, flush, and browser cleanup headroom",
  );

  const tools = checkMediaCapabilities();
  const mediaTmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-recording-contract-"));
  try {
    const png = spawnSync(tools.ffmpeg, [
      "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=navy:s=639x479,format=rgb24",
      "-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "pipe:1",
    ], { encoding: null, timeout: 15_000 });
    assert.equal(png.status, 0, `PNG fixture generation succeeds: ${String(png.stderr)}`);
    const mp4Path = path.join(mediaTmp, "fixture.mp4");
    const encoder = createPngMp4Encoder({ outputPath: mp4Path, fps: 30, tools });
    for (let index = 0; index < 15; index += 1) encoder.write(png.stdout);
    await encoder.finish();
    const diagnostics = {
      expectedAt30Fps: 15, encoded: 15, rawScreencastEvents: 5,
      sourceFramesUsed: 5, reusedSourceFrameSlots: 10, sourceCoverage: 1 / 3, deficient: true,
    };
    const media = finalizeMp4Artifacts({
      mp4Path,
      framesDir: path.join(mediaTmp, "frames"),
      contactSheetPath: path.join(mediaTmp, "contact.png"),
      targetDurationMs: 500,
      tools,
      frameDiagnostics: diagnostics,
    });
    assert.equal(media.probe.codec, "h264", "final media probe confirms H.264");
    assert.deepEqual(
      { codecTag: media.probe.codecTag, pixelFormat: media.probe.pixelFormat, container: media.probe.container, fastStart: media.probe.fastStart },
      { codecTag: "avc1", pixelFormat: "yuv420p", container: "mov,mp4,m4a,3gp,3g2,mj2", fastStart: true },
      "final media probe confirms the mobile MP4 compatibility surface",
    );
    assert.deepEqual(
      { width: media.probe.width, height: media.probe.height },
      { width: 640, height: 480 },
      "final MP4 normalizes odd source dimensions for H.264 compatibility",
    );
    assert.ok(media.probe.durationSeconds >= 0.45 && media.probe.durationSeconds <= 0.55, "final MP4 timeline is normalized to wall duration");
    assert.equal(media.probe.frameRate, "30/1", "final MP4 uses the documented 30 FPS timeline");
    assert.deepEqual(
      { expected: media.frameDiagnostics.expectedAt30Fps, encoded: media.frameDiagnostics.encoded },
      { expected: 15, encoded: 15 },
      "timeline normalization emits the expected wall-duration frame count",
    );
    assert.strictEqual(media.frameDiagnostics, diagnostics, "final media preserves measured source diagnostics without estimating them again");
    assert.equal(media.framePaths.length, RECORDING_LIMITS.maxFrames, "representative sampling fills its bound when enough frames exist");
    assert.deepEqual(
      media.framePaths.map((framePath) => path.basename(framePath)),
      media.framePaths.map((_, index) => `frame-${String(index + 1).padStart(2, "0")}.png`),
      "representative frames remain contiguous so contact-sheet input includes the final frame",
    );
    assert.ok(media.contactSheet.width > 0 && media.contactSheet.height > 0, "contact sheet is a readable PNG");
  } finally {
    fs.rmSync(mediaTmp, { recursive: true, force: true });
  }

  const failedEncoderPath = path.join(os.tmpdir(), `rts-li-failed-encoder-${process.pid}.mp4`);
  const failedEncoder = createPngMp4Encoder({
    outputPath: failedEncoderPath,
    fps: RECORDING_LIMITS.fps,
    tools: { ffmpeg: process.execPath },
  });
  void failedEncoder.write(Buffer.from("not-a-png")).catch(() => {});
  await assert.rejects(
    withDeadline(failedEncoder.finish(1_000), 2_000),
    (error) => error?.code === "mediaProcessingFailed",
    "an encoder that exits while writes are backpressured fails promptly instead of hanging on drain",
  );
  await failedEncoder.abort();

  const missingEncoderPath = path.join(os.tmpdir(), `rts-li-missing-encoder-${process.pid}.mp4`);
  const missingEncoder = createPngMp4Encoder({
    outputPath: missingEncoderPath,
    fps: RECORDING_LIMITS.fps,
    tools: { ffmpeg: "/definitely/missing/ffmpeg" },
  });
  void missingEncoder.write(Buffer.from("not-a-png")).catch(() => {});
  await assert.rejects(
    withDeadline(missingEncoder.finish(1_000), 2_000),
    (error) => error?.code === "mediaProcessingFailed" && /could not start/.test(error.message),
    "an encoder launch failure rejects normally instead of becoming an unhandled ChildProcess error",
  );
  await missingEncoder.abort();

  const watchdogDriver = fixtureRecordingDriver(root, tools);
  await assert.rejects(
    watchdogDriver.recordWait(),
    (error) => error?.code === "recordingInactive",
    "a never-started recording wait is actionable",
  );
  await watchdogDriver.recordStart({ sessionId: fixtureSessionId(), name: "watchdog", maxDurationMs: 25 });
  const watchdogResult = await withDeadline(watchdogDriver.recordWait(), 5_000);
  assert.equal(watchdogDriver.recordingStatus().last.stoppedBy, "watchdog", "duration watchdog finalizes an active recorder");
  assert.deepEqual(await watchdogDriver.recordWait(), watchdogResult, "a completed recording wait returns the same finalized result again");
  assert.ok(fs.existsSync(watchdogDriver.recordingStatus().last.contactSheetPath), "watchdog finalization retains a completed contact sheet");
  fs.rmSync(path.dirname(watchdogDriver.recordingStatus().last.videoPath), { recursive: true, force: true });

  const lateWatchdogDriver = fixtureRecordingDriver(root, tools);
  await lateWatchdogDriver.recordStart({ sessionId: fixtureSessionId(), name: "late-watchdog", maxDurationMs: 25 });
  const blockedUntil = Date.now() + 90;
  while (Date.now() < blockedUntil) { /* exercise a delayed event-loop watchdog */ }
  const lateWatchdogResult = await withDeadline(lateWatchdogDriver.recordWait(), 5_000);
  assert.deepEqual(
    { expected: lateWatchdogResult.frameDiagnostics.expectedAt30Fps, encoded: lateWatchdogResult.frameDiagnostics.encoded },
    { expected: 1, encoded: 1 },
    "a late watchdog caps cumulative wall slots at the requested duration instead of drifting past it",
  );
  fs.rmSync(path.dirname(lateWatchdogResult.videoPath), { recursive: true, force: true });

  const closeDriver = fixtureRecordingDriver(root, tools);
  const closeSessionId = fixtureSessionId();
  await closeDriver.recordStart({ sessionId: closeSessionId, name: "close", maxDurationMs: 5_000 });
  await assert.rejects(
    closeDriver.screenshot({ sessionId: closeSessionId, name: "conflicting-capture" }),
    (error) => error?.code === "recordingActive",
    "screenshots cannot change presentation or viewport while a recording is active",
  );
  await closeDriver.close();
  assert.equal(closeDriver.state, DRIVER_STATES.CLOSED, "driver close reaches the closed state while recording");
  assert.equal(closeDriver.recordingStatus().last.stoppedBy, "sessionClose", "driver close boundedly finalizes its recorder");
  fs.rmSync(path.dirname(closeDriver.recordingStatus().last.videoPath), { recursive: true, force: true });

  const delayedStopDriver = fixtureRecordingDriver(root, tools, { recorderStopDelayMs: 250 });
  await delayedStopDriver.recordStart({ sessionId: fixtureSessionId(), name: "delayed-stop", maxDurationMs: 5_000 });
  await new Promise((resolve) => setTimeout(resolve, 60));
  const manifestAliases = Array.from({ length: 400 }, (_, index) => ({ alias: `subject_${index}`, id: index + 1 }));
  const delayedStopPromise = delayedStopDriver.recordStop({ aliases: manifestAliases });
  const delayedWaitPromise = delayedStopDriver.recordWait();
  const [delayedStop, delayedWait] = await Promise.all([delayedStopPromise, delayedWaitPromise]);
  assert.deepEqual(delayedWait, delayedStop, "finalizing stop and wait share one result shape");
  const delayedManifest = JSON.parse(fs.readFileSync(delayedStop.manifestPath, "utf8"));
  assert.ok(delayedManifest.capture.wallDurationMs < 200, "capture duration excludes recorder finalization latency");
  assert.ok(delayedStop.probe.durationSeconds < 0.2, "MP4 timeline excludes recorder finalization latency");
  assert.deepEqual(
    { count: delayedManifest.aliases.count, detailed: delayedManifest.aliases.details.length, truncated: delayedManifest.aliases.truncated },
    { count: 400, detailed: LAB_INTERACT_SUMMARY_LIMITS.detailedAliases, truncated: true },
    "recording manifests count all aliases while bounding detailed alias rows",
  );
  fs.rmSync(path.dirname(delayedStop.videoPath), { recursive: true, force: true });

  const failedDriver = fixtureRecordingDriver(root, tools, { failScreencast: true });
  const failedSessionId = fixtureSessionId();
  await assert.rejects(
    failedDriver.recordStart({ sessionId: failedSessionId, name: "page-failure", maxDurationMs: 5_000 }),
    /fixture page failure/,
    "page failure rejects recording startup",
  );
  const failedRoot = path.join(root, "target", "lab-interact", failedSessionId, "recordings");
  const failedEntries = fs.existsSync(failedRoot) ? fs.readdirSync(failedRoot).filter((name) => name.startsWith("page-failure-")) : [];
  assert.deepEqual(failedEntries, [], "page failure removes its partial recording directory");
  const failedStatusDriver = fixtureRecordingDriver(root, tools, { failStartStatus: true });
  const failedStatusSessionId = fixtureSessionId();
  await assert.rejects(
    failedStatusDriver.recordStart({ sessionId: failedStatusSessionId, name: "status-failure", maxDurationMs: 5_000 }),
    /fixture status failure/,
    "status failure rejects recording startup",
  );
  assert.equal(failedStatusDriver.fixtureRecorderStops, 0, "status is verified before the CDP recorder is acquired");
  const failedStatusRoot = path.join(root, "target", "lab-interact", failedStatusSessionId, "recordings");
  const failedStatusEntries = fs.existsSync(failedStatusRoot) ? fs.readdirSync(failedStatusRoot).filter((name) => name.startsWith("status-failure-")) : [];
  assert.deepEqual(failedStatusEntries, [], "post-screencast startup failure removes its partial recording directory");
  const failedFinalizeDriver = fixtureRecordingDriver(root, tools, { failRecorderStop: true });
  const failedFinalizeSessionId = fixtureSessionId();
  await failedFinalizeDriver.recordStart({ sessionId: failedFinalizeSessionId, name: "finalize-failure", maxDurationMs: 5_000 });
  const failedFinalizeWait = failedFinalizeDriver.recordWait();
  let failedStopError;
  await assert.rejects(
    failedFinalizeDriver.recordStop().catch((error) => { failedStopError = error; throw error; }),
    /fixture recorder stop failure/,
    "explicit finalization reports recorder failure",
  );
  let failedWaitError;
  await assert.rejects(
    failedFinalizeWait.catch((error) => { failedWaitError = error; throw error; }),
    /fixture recorder stop failure/,
    "the shared waiter rejects with the same finalization failure",
  );
  assert.strictEqual(failedWaitError, failedStopError, "stop and wait observe the identical normalized finalization failure");
  assert.equal(failedFinalizeDriver.fixtureRecorderStops, 1, "failed finalization settles the recorder exactly once");
  const failedFinalizeRoot = path.join(root, "target", "lab-interact", failedFinalizeSessionId, "recordings");
  const failedFinalizeEntries = fs.existsSync(failedFinalizeRoot)
    ? fs.readdirSync(failedFinalizeRoot).filter((name) => name.startsWith("finalize-failure-"))
    : [];
  assert.deepEqual(failedFinalizeEntries, [], "failed finalization removes partial artifacts");
  assert.throws(
    () => validateCommandInput("record-start", { sessionId, crop: { x: 0, y: 0, width: 1, height: 100 } }),
    (error) => error?.code === "invalidInput",
    "record-start rejects unusable crops",
  );
  assert.throws(
    () => checkMediaCapabilities({ ffmpeg: "/definitely/missing/ffmpeg", ffprobe: "/definitely/missing/ffprobe" }),
    (error) => error instanceof LabInteractRecordingError && error.code === "ffmpegUnavailable",
    "missing FFmpeg returns an actionable capability error before recording",
  );

  await service.execute("spawn", { sessionId, spawns: [{ owner: 1, kind: "tank", x: 800, y: 800, alias: "subject" }] });
  const started = await service.execute("record-start", {
    sessionId, name: "contract-motion", maxDurationMs: 5_000,
    viewport: { width: 800, height: 600, deviceScaleFactor: 1 }, scale: 0.5,
  });
  assert.equal(started.recorder.active, true, "record-start activates one recorder");
  assert.equal((await service.execute("status", { sessionId })).recorder.active, true, "status exposes active recorder state");
  await assert.rejects(
    service.execute("record-start", { sessionId, name: "duplicate" }),
    (error) => error?.code === "recordingActive",
    "duplicate starts do not replace the active recorder",
  );
  const activeWait = service.execute("record-wait", { sessionId });
  await service.execute("order", { sessionId, playerId: 1, command: { c: "move", units: ["subject"], x: 900, y: 900 } });
  await service.execute("camera", { sessionId, camera: { action: "focus", refs: ["subject"] } });
  await service.execute("time", { sessionId, control: { action: "step", ticks: 3 } });
  const stopped = await service.execute("record-stop", { sessionId });
  const waited = await activeWait;
  assert.deepEqual(waited, stopped, "service record-wait observes the same artifact returned by record-stop");
  assert.equal(stopped.probe.codec, "h264", "record-stop returns H.264 codec probe metadata");
  assert.equal(stopped.framePaths.length, 2, "record-stop returns representative PNG paths");
  assert.equal(stopped.fixtureMetadata.operations.length, 3, "accepted order, camera, and time operations are retained");
  assert.deepEqual(stopped.fixtureMetadata.aliases, [{ alias: "subject", id: 100 }], "stop records the final bounded alias map");
  assert.deepEqual(await service.execute("record-wait", { sessionId }), stopped, "service wait returns the already completed current recording");
  await assert.rejects(
    service.execute("record-stop", { sessionId }),
    (error) => error?.code === "recordingInactive",
    "duplicate stops are correctable and do not invent an artifact",
  );

  await service.execute("record-start", { sessionId, name: "close-cleanup" });
  const closeWait = service.execute("record-wait", { sessionId });
  assert.equal(await service.close(sessionId, "contractClose"), true, "session close succeeds while a recorder is active");
  assert.equal((await closeWait).stoppedBy, "sessionClose", "session close settles active recording waiters before draining work");
  assert.equal((await service.status()).sessions.length, 0, "close removes the recording session");
  await service.shutdown();

  shutdownService = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
  const shutdownSession = await shutdownService.execute("open", {});
  testArtifacts.ownSession(shutdownSession.sessionId);
  await shutdownService.execute("record-start", { sessionId: shutdownSession.sessionId, name: "shutdown-cleanup" });
  const shutdownWait = shutdownService.execute("record-wait", { sessionId: shutdownSession.sessionId });
  assert.deepEqual(await shutdownService.execute("shutdown", {}), { shuttingDown: true }, "service shutdown waits for recorder settlement");
  assert.equal((await shutdownWait).stoppedBy, "sessionClose", "shutdown settles active recording waiters exactly once");

  console.log("✅ lab_interact_recording_contracts.mjs: bounds, state, operation metadata, errors, and close cleanup passed");
} finally {
  await service?.shutdown();
  await shutdownService?.shutdown();
  testArtifacts.cleanup();
  testArtifacts.assertClean();
}

function fixtureRecordingDriver(workspaceRoot, mediaTools, { failScreencast = false, failStartStatus = false, failRecorderStop = false, recorderStopDelayMs = 0 } = {}) {
  const driver = new LabInteractDriver({ workspaceRoot, viewport: { width: 640, height: 480, deviceScaleFactor: 1 } });
  driver.workspace = { root: workspaceRoot, branch: "fixture", head: "a".repeat(40) };
  driver.state = DRIVER_STATES.OPEN;
  driver.browserVersion = "fixture-chrome";
  let frame = 0;
  let recorderStops = 0;
  let viewport = { width: 640, height: 480, deviceScaleFactor: 1 };
  const png = spawnSync(mediaTools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=640x480",
    "-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "pipe:1",
  ], { encoding: null, timeout: 15_000 });
  assert.equal(png.status, 0, `fixture PNG succeeds: ${String(png.stderr)}`);
  driver.page = {
    viewport: () => viewport,
    setViewport: async (next) => { viewport = next; },
    close: async () => {},
    evaluate: async (fn, bridgeCall) => {
      if (bridgeCall?.method === "captureReadiness") {
        frame += 1;
        return { ok: true, value: { ready: true, frame, snapshotTick: 10, assets: {}, frameErrors: [], renderErrors: [], missingTextureSubjectIds: [] } };
      }
      if (bridgeCall?.method === "status") {
        if (failStartStatus) throw new Error("fixture status failure");
        return { ok: true, value: { ready: true, snapshotTick: 10, roomTime: { currentTick: 10, paused: true, speed: 0 } } };
      }
      if (bridgeCall?.method === "presentation") return { ok: true, value: { mode: bridgeCall.input.mode } };
      const source = String(fn);
      if (source.includes("getElementById")) return { x: 0, y: 0, width: viewport.width, height: viewport.height };
      if (source.includes("navigator.userAgent")) return "fixture-agent";
      return undefined;
    },
    createCDPSession: async () => {
      if (failScreencast) throw new Error("fixture page failure");
      const client = new EventEmitter();
      let timer = null;
      let session = 0;
      client.send = async (method) => {
        if (method === "Page.startScreencast") {
          const emit = () => client.emit("Page.screencastFrame", {
            data: png.stdout.toString("base64"), metadata: { timestamp: session / 200 }, sessionId: ++session,
          });
          queueMicrotask(emit);
          timer = setInterval(emit, 5);
          timer.unref?.();
        }
        if (method === "Page.stopScreencast") {
          recorderStops += 1;
          clearInterval(timer);
          if (failRecorderStop) throw new Error("fixture recorder stop failure");
          if (recorderStopDelayMs > 0) await new Promise((resolve) => setTimeout(resolve, recorderStopDelayMs));
        }
      };
      client.detach = async () => {};
      return client;
    },
  };
  Object.defineProperty(driver, "fixtureRecorderStops", { get: () => recorderStops });
  return driver;
}

async function withDeadline(promise, timeoutMs) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("recording wait timed out")), timeoutMs); }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
