import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { LabInteractService, validateCommandInput } from "../scripts/lab-interact/command_service.mjs";
import {
  createFixedCaptureEncoder, FIXED_CAPTURE_LIMITS, fixedFrameTick, fixedRepresentativeIndices,
} from "../scripts/lab-interact/fixed_capture.mjs";
import { checkMediaCapabilities } from "../scripts/lab-interact/recording.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";
import { LabInteractTestArtifacts } from "./fixtures/lab_interact_test_artifacts.mjs";
import { boundedSummary, LAB_INTERACT_SUMMARY_LIMITS } from "../scripts/lab-interact/manifest_summary.mjs";

const sessionId = `lab_${"a".repeat(32)}`;
const root = process.cwd();
const testArtifacts = new LabInteractTestArtifacts(root);
let service;

try {
const driverSource = fs.readFileSync(new URL("../scripts/lab-interact/driver.mjs", import.meta.url), "utf8");
assert.match(
  driverSource,
  /import\s*\{[^}]*FIXED_CAPTURE_LIMITS[^}]*\}\s*from\s*"\.\/fixed_capture\.mjs"/s,
  "the real fixed-capture driver imports the limits it enforces at runtime",
);
assert.throws(() => validateCommandInput("capture-fixed", { sessionId, fps: 61 }), (error) => error?.code === "invalidInput");
assert.throws(() => validateCommandInput("capture-fixed", { sessionId, frameCount: FIXED_CAPTURE_LIMITS.maxFrames + 1 }), (error) => error?.code === "invalidInput");
assert.equal(FIXED_CAPTURE_LIMITS.maxFrames, 1_800, "fixed capture supports one minute at 30 FPS");
assert.deepEqual([0, 1, 2, 3].map((index) => fixedFrameTick(9, index, 60)), [9, 9, 10, 10]);
assert.deepEqual(
  [...fixedRepresentativeIndices(1_800)],
  [0, 360, 720, 1079, 1439, 1799],
  "minute-scale fixed capture summaries include the first and final frames",
);
assert.deepEqual(
  boundedSummary(Array.from({ length: 400 }, (_, index) => index), LAB_INTERACT_SUMMARY_LIMITS.detailedAliases),
  { count: 400, details: Array.from({ length: 40 }, (_, index) => index), truncated: true },
  "fixed-capture manifest summaries preserve total/truncated metadata without 400 detailed rows",
);

service = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
const opened = await service.execute("open", {});
testArtifacts.ownSession(opened.sessionId);
await service.execute("spawn", { sessionId: opened.sessionId, spawns: [{ owner: 1, kind: "tank", x: 800, y: 800, alias: "subject" }] });
const result = await service.execute("capture-fixed", { sessionId: opened.sessionId, name: "contract", fps: 60, frameCount: 4 });
assert.deepEqual(
  { count: result.frameSummary.count, representatives: result.frameSummary.representativeFramePaths.length },
  { count: 4, representatives: 4 },
  "service returns a compact frame summary with bounded representative paths",
);
assert.deepEqual(result.authoritative, { startTick: 1, endTick: 2 }, "service preserves exact simulation-to-frame mapping");
assert.equal(result.fixtureMetadata.sceneRevision, 1, "manifest input identifies mutations after the launch scene");
assert.deepEqual(result.fixtureMetadata.aliases, [{ alias: "subject", id: 100 }], "manifest input retains bounded alias identity");
await assert.rejects(service.execute("capture-cancel", { sessionId: opened.sessionId }), (error) => error?.code === "captureInactive", "cancel reports an actionable error when no fixed capture is active");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-fixed-contract-"));
try {
  const tools = checkMediaCapabilities();
  const generated = spawnSync(tools.ffmpeg, ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=63x63:rate=30:duration=0.1", "-frames:v", "3", "-f", "image2pipe", "-c:v", "png", "pipe:1"], { encoding: null, timeout: 15_000 });
  assert.equal(generated.status, 0, String(generated.stderr));
  const outputPath = path.join(tmp, "fixed.mp4");
  const contactSheetPath = path.join(tmp, "contact.png");
  const encoder = createFixedCaptureEncoder({ outputPath, contactSheetPath, fps: 30, frameCount: 3 });
  encoder.write(generated.stdout);
  const media = await encoder.finish();
  assert.ok(media.bytes > 0, "fixed frame sequence encodes to non-empty H.264 MP4 media");
  assert.deepEqual({ codec: media.probe.codec, frames: media.probe.frameCount, fps: media.probe.frameRate }, { codec: "h264", frames: 3, fps: "30/1" }, "ffprobe confirms fixed codec, frame count, and FPS");
  assert.deepEqual({ width: media.probe.width, height: media.probe.height }, { width: 64, height: 64 }, "fixed capture pads odd dimensions for H.264 compatibility");
  assert.deepEqual(
    { codecTag: media.probe.codecTag, pixelFormat: media.probe.pixelFormat, container: media.probe.container },
    { codecTag: "avc1", pixelFormat: "yuv420p", container: "mov,mp4,m4a,3gp,3g2,mj2" },
    "fixed capture preserves the mobile MP4 compatibility surface",
  );
  assert.deepEqual(media.contactSheet, { width: 1456, height: 612 }, "contact-sheet PNG dimensions remain bounded and readable");
  assert.ok(fs.statSync(contactSheetPath).size > 0, "fixed capture creates a contact sheet");
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}

console.log("✅ lab_interact_fixed_capture_contracts.mjs: bounds, tick mapping, service, and media passed");
} finally {
  await service?.shutdown();
  testArtifacts.cleanup();
  testArtifacts.assertClean();
}
