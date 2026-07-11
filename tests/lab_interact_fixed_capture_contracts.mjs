import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { LabInteractService, validateCommandInput } from "../scripts/lab-interact/command_service.mjs";
import { encodeFixedCapture, FIXED_CAPTURE_LIMITS, fixedFrameTick } from "../scripts/lab-interact/fixed_capture.mjs";
import { checkMediaCapabilities } from "../scripts/lab-interact/recording.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";

const sessionId = `lab_${"a".repeat(32)}`;
const driverSource = fs.readFileSync(new URL("../scripts/lab-interact/driver.mjs", import.meta.url), "utf8");
assert.match(
  driverSource,
  /import\s*\{[^}]*FIXED_CAPTURE_LIMITS[^}]*\}\s*from\s*"\.\/fixed_capture\.mjs"/s,
  "the real fixed-capture driver imports the limits it enforces at runtime",
);
assert.throws(() => validateCommandInput("capture-fixed", { sessionId, fps: 61 }), (error) => error?.code === "invalidInput");
assert.throws(() => validateCommandInput("capture-fixed", { sessionId, frameCount: FIXED_CAPTURE_LIMITS.maxFrames + 1 }), (error) => error?.code === "invalidInput");
assert.ok(FIXED_CAPTURE_LIMITS.maxSequenceBytes >= FIXED_CAPTURE_LIMITS.maxFrameBytes, "fixed PNG sequences have explicit per-frame and aggregate disk bounds");
assert.deepEqual([0, 1, 2, 3].map((index) => fixedFrameTick(9, index, 60)), [9, 9, 10, 10]);

const service = new LabInteractService({ workspaceRoot: process.cwd(), driverFactory: openLabInteractDriver });
const opened = await service.execute("open", {});
await service.execute("spawn", { sessionId: opened.sessionId, spawns: [{ owner: 1, kind: "tank", x: 800, y: 800, alias: "subject" }] });
const result = await service.execute("capture-fixed", { sessionId: opened.sessionId, name: "contract", fps: 60, frameCount: 4 });
assert.equal(result.framePaths.length, 4, "service returns one PNG path per frame");
assert.deepEqual(result.authoritative, { startTick: 1, endTick: 2 }, "service preserves exact simulation-to-frame mapping");
assert.equal(result.fixtureMetadata.sceneRevision, 1, "manifest input identifies mutations after the launch scene");
assert.deepEqual(result.fixtureMetadata.aliases, [{ alias: "subject", id: 100 }], "manifest input retains bounded alias identity");
await assert.rejects(service.execute("capture-cancel", { sessionId: opened.sessionId }), (error) => error?.code === "captureInactive", "cancel reports an actionable error when no fixed capture is active");
await service.shutdown();

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-fixed-contract-"));
try {
  const framesDir = path.join(tmp, "frames");
  fs.mkdirSync(framesDir);
  const tools = checkMediaCapabilities();
  const generated = spawnSync(tools.ffmpeg, ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=64x64:rate=30:duration=0.1", "-frames:v", "3", path.join(framesDir, "frame-%04d.png")], { encoding: "utf8", timeout: 15_000 });
  assert.equal(generated.status, 0, generated.stderr);
  // FFmpeg numbers image sequences from one; normalize to the capture command's zero-based names.
  for (let index = 1; index <= 3; index += 1) fs.renameSync(path.join(framesDir, `frame-${String(index).padStart(4, "0")}.png`), path.join(framesDir, `tmp-${String(index - 1).padStart(4, "0")}.png`));
  for (let index = 0; index < 3; index += 1) fs.renameSync(path.join(framesDir, `tmp-${String(index).padStart(4, "0")}.png`), path.join(framesDir, `frame-${String(index).padStart(4, "0")}.png`));
  const outputPath = path.join(tmp, "fixed.webm");
  const contactSheetPath = path.join(tmp, "contact.png");
  const media = encodeFixedCapture({ framesDir, outputPath, contactSheetPath, fps: 30, frameCount: 3 });
  assert.ok(media.bytes > 0, "fixed frame sequence encodes to non-empty VP9 media");
  assert.deepEqual({ codec: media.probe.codec, frames: media.probe.frameCount, fps: media.probe.frameRate }, { codec: "vp9", frames: 3, fps: "30/1" }, "ffprobe confirms fixed codec, frame count, and FPS");
  assert.deepEqual(media.contactSheet, { width: 1456, height: 612 }, "contact-sheet PNG dimensions remain bounded and readable");
  assert.ok(fs.statSync(contactSheetPath).size > 0, "fixed capture creates a contact sheet");
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}

console.log("✅ lab_interact_fixed_capture_contracts.mjs: bounds, tick mapping, service, and media passed");
