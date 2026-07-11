import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  LAB_INTERACT_LIMITS, LabInteractService, validateCommandInput,
} from "../scripts/lab-interact/command_service.mjs";
import {
  checkMediaCapabilities, finalizeMedia, LabInteractRecordingError, RECORDING_LIMITS,
} from "../scripts/lab-interact/recording.mjs";
import { DRIVER_STATES, LabInteractDriver } from "../scripts/lab-interact/driver.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const service = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
const opened = await service.execute("open", {});
const sessionId = opened.sessionId;

assert.equal(RECORDING_LIMITS.defaultDurationMs, 10_000, "recordings default to a short review clip");
assert.equal(RECORDING_LIMITS.maxDurationMs, 30_000, "recordings retain a hard 30-second duration ceiling");
assert.equal(RECORDING_LIMITS.maxBytes, 64 * 1024 * 1024, "recordings retain a hard 64 MiB file ceiling");
assert.equal(LAB_INTERACT_LIMITS.maxRecordingOperations, 200, "recording action manifests remain bounded");
assert.equal(RECORDING_LIMITS.maxOperations, LAB_INTERACT_LIMITS.maxRecordingOperations, "service and driver share the operation bound");

assert.throws(
  () => validateCommandInput("record-start", { sessionId, maxDurationMs: 30_001 }),
  (error) => error?.code === "invalidInput",
  "record-start rejects durations over the hard bound",
);

const tools = checkMediaCapabilities();
const mediaTmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-recording-contract-"));
try {
  const oversizedPath = path.join(mediaTmp, "oversized.webm");
  fs.writeFileSync(oversizedPath, "x");
  fs.truncateSync(oversizedPath, RECORDING_LIMITS.maxBytes + 1);
  assert.throws(
    () => finalizeMedia({ webmPath: oversizedPath, framesDir: path.join(mediaTmp, "oversized-frames"), contactSheetPath: path.join(mediaTmp, "oversized.png"), tools }),
    (error) => error?.code === "recordingTooLarge",
    "oversized recordings are rejected before media processing",
  );
  assert.equal(fs.existsSync(oversizedPath), false, "oversized recording bytes are deleted");
  const webmPath = path.join(mediaTmp, "fixture.webm");
  const generated = spawnSync(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=navy:s=640x480:r=30:d=0.12",
    "-an", "-c:v", "libvpx-vp9", "-b:v", "0", webmPath,
  ], { encoding: "utf8", timeout: 15_000 });
  assert.equal(generated.status, 0, `VP9 fixture generation succeeds: ${generated.stderr}`);
  const media = finalizeMedia({
    webmPath,
    framesDir: path.join(mediaTmp, "frames"),
    contactSheetPath: path.join(mediaTmp, "contact.png"),
    tools,
  });
  assert.equal(media.probe.codec, "vp9", "final media probe confirms VP9");
  assert.deepEqual({ width: media.probe.width, height: media.probe.height }, { width: 640, height: 480 }, "final media probe confirms dimensions");
  assert.ok(media.probe.durationSeconds >= 0.1 && media.probe.durationSeconds <= 0.2, "final media probe confirms bounded duration");
  assert.ok(media.framePaths.length >= 2 && media.framePaths.length <= RECORDING_LIMITS.maxFrames, "representative sampling remains bounded");
  assert.deepEqual(
    media.framePaths.map((framePath) => path.basename(framePath)),
    media.framePaths.map((_, index) => `frame-${String(index + 1).padStart(2, "0")}.png`),
    "representative frames remain contiguous so contact-sheet input includes the final frame",
  );
  assert.ok(media.contactSheet.width > 0 && media.contactSheet.height > 0, "contact sheet is a readable PNG");
} finally {
  fs.rmSync(mediaTmp, { recursive: true, force: true });
}

const watchdogDriver = fixtureRecordingDriver(root, tools);
await watchdogDriver.recordStart({ sessionId: `lab_${"b".repeat(32)}`, name: "watchdog", maxDurationMs: 25 });
await waitFor(() => watchdogDriver.recordingStatus().active === false, 5_000);
assert.equal(watchdogDriver.recordingStatus().last.stoppedBy, "watchdog", "duration watchdog finalizes an active recorder");
assert.ok(fs.existsSync(watchdogDriver.recordingStatus().last.contactSheetPath), "watchdog finalization retains a completed contact sheet");
fs.rmSync(path.dirname(watchdogDriver.recordingStatus().last.webmPath), { recursive: true, force: true });

const closeDriver = fixtureRecordingDriver(root, tools);
await closeDriver.recordStart({ sessionId: `lab_${"c".repeat(32)}`, name: "close", maxDurationMs: 5_000 });
await assert.rejects(
  closeDriver.screenshot({ sessionId: `lab_${"c".repeat(32)}`, name: "conflicting-capture" }),
  (error) => error?.code === "recordingActive",
  "screenshots cannot change presentation or viewport while a recording is active",
);
await closeDriver.close();
assert.equal(closeDriver.state, DRIVER_STATES.CLOSED, "driver close reaches the closed state while recording");
assert.equal(closeDriver.recordingStatus().last.stoppedBy, "sessionClose", "driver close boundedly finalizes its recorder");
fs.rmSync(path.dirname(closeDriver.recordingStatus().last.webmPath), { recursive: true, force: true });

const failedDriver = fixtureRecordingDriver(root, tools, { failScreencast: true });
const failedSessionId = `lab_${"d".repeat(32)}`;
await assert.rejects(
  failedDriver.recordStart({ sessionId: failedSessionId, name: "page-failure", maxDurationMs: 5_000 }),
  /fixture page failure/,
  "page failure rejects recording startup",
);
const failedRoot = path.join(root, "target", "lab-interact", failedSessionId, "recordings");
const failedEntries = fs.existsSync(failedRoot) ? fs.readdirSync(failedRoot).filter((name) => name.startsWith("page-failure-")) : [];
assert.deepEqual(failedEntries, [], "page failure removes its partial recording directory");
const failedStatusDriver = fixtureRecordingDriver(root, tools, { failStartStatus: true });
const failedStatusSessionId = `lab_${"e".repeat(32)}`;
await assert.rejects(
  failedStatusDriver.recordStart({ sessionId: failedStatusSessionId, name: "status-failure", maxDurationMs: 5_000 }),
  /fixture status failure/,
  "status failure rejects recording startup",
);
assert.equal(failedStatusDriver.fixtureRecorderStops, 1, "post-screencast startup failure stops the owned recorder");
const failedStatusRoot = path.join(root, "target", "lab-interact", failedStatusSessionId, "recordings");
const failedStatusEntries = fs.existsSync(failedStatusRoot) ? fs.readdirSync(failedStatusRoot).filter((name) => name.startsWith("status-failure-")) : [];
assert.deepEqual(failedStatusEntries, [], "post-screencast startup failure removes its partial recording directory");
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
await service.execute("order", { sessionId, playerId: 1, command: { c: "move", units: ["subject"], x: 900, y: 900 } });
await service.execute("camera", { sessionId, camera: { action: "focus", refs: ["subject"] } });
await service.execute("time", { sessionId, control: { action: "step", ticks: 3 } });
const stopped = await service.execute("record-stop", { sessionId });
assert.equal(stopped.probe.codec, "vp9", "record-stop returns codec probe metadata");
assert.equal(stopped.framePaths.length, 2, "record-stop returns representative PNG paths");
assert.equal(stopped.fixtureMetadata.operations.length, 3, "accepted order, camera, and time operations are retained");
assert.deepEqual(stopped.fixtureMetadata.aliases, [{ alias: "subject", id: 100 }], "stop records the final bounded alias map");
await assert.rejects(
  service.execute("record-stop", { sessionId }),
  (error) => error?.code === "recordingInactive",
  "duplicate stops are correctable and do not invent an artifact",
);

await service.execute("record-start", { sessionId, name: "close-cleanup" });
assert.equal(await service.close(sessionId, "contractClose"), true, "session close succeeds while a recorder is active");
assert.equal((await service.status()).sessions.length, 0, "close removes the recording session");
await service.shutdown();

console.log("✅ lab_interact_recording_contracts.mjs: bounds, state, operation metadata, errors, and close cleanup passed");

function fixtureRecordingDriver(workspaceRoot, mediaTools, { failScreencast = false, failStartStatus = false } = {}) {
  const driver = new LabInteractDriver({ workspaceRoot, viewport: { width: 640, height: 480, deviceScaleFactor: 1 } });
  driver.workspace = { root: workspaceRoot, branch: "fixture", head: "a".repeat(40) };
  driver.state = DRIVER_STATES.OPEN;
  driver.browserVersion = "fixture-chrome";
  let frame = 0;
  let recorderStops = 0;
  let viewport = { width: 640, height: 480, deviceScaleFactor: 1 };
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
    screencast: async ({ path: outputPath }) => {
      if (failScreencast) {
        fs.writeFileSync(outputPath, "partial");
        throw new Error("fixture page failure");
      }
      const generated = spawnSync(mediaTools.ffmpeg, [
        "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=black:s=640x480:r=30:d=0.2",
        "-an", "-c:v", "libvpx-vp9", "-b:v", "0", outputPath,
      ], { encoding: "utf8", timeout: 15_000 });
      assert.equal(generated.status, 0, `fixture screencast succeeds: ${generated.stderr}`);
      return { stop: async () => { recorderStops += 1; } };
    },
  };
  Object.defineProperty(driver, "fixtureRecorderStops", { get: () => recorderStops });
  return driver;
}

async function waitFor(predicate, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.fail("timed out waiting for recording lifecycle");
}
