import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { InteractService, validateCommandInput } from "../scripts/interact/command_service.ts";
import {
  createFixedCaptureEncoder, FIXED_CAPTURE_LIMITS, fixedFrameTick, fixedRepresentativeIndices,
} from "../scripts/interact/fixed_capture.ts";
import {
  GAME_TIMELAPSE_LIMITS, timelapseFrameBound, timelapseMayCaptureFrame,
} from "../scripts/interact/game_timelapse.ts";
import { checkMediaCapabilities } from "../scripts/interact/recording.ts";
import { openInteractDriver } from "./fixtures/interact_fake_driver.mjs";
import { InteractTestArtifacts } from "./fixtures/interact_test_artifacts.mjs";
import { boundedSummary, INTERACT_SUMMARY_LIMITS } from "../scripts/interact/manifest_summary.ts";

const sessionId = `lab_${"a".repeat(32)}`;
const root = process.cwd();
const testArtifacts = new InteractTestArtifacts(root);
let service;

try {
  const driverSource = fs.readFileSync(new URL("../scripts/interact/driver.ts", import.meta.url), "utf8");
  assert.match(
    driverSource,
    /import\s*\{[^}]*FIXED_CAPTURE_LIMITS[^}]*\}\s*from\s*"\.\/fixed_capture\.ts"/s,
    "the real fixed-capture driver imports the limits it enforces at runtime",
  );
  assert.throws(() => validateCommandInput("capture-fixed", { sessionId, fps: 61 }), (error) => error?.code === "invalidInput");
  assert.throws(() => validateCommandInput("capture-fixed", { sessionId, frameCount: FIXED_CAPTURE_LIMITS.maxFrames + 1 }), (error) => error?.code === "invalidInput");
  assert.doesNotThrow(
    () => validateCommandInput("open", { scenario: "supply-300-hellhole" }),
    "Lab opens accept hyphenated bundled scenario ids",
  );
  const gameSessionId = `game_${"b".repeat(32)}`;
  const scenarioSessionId = `scenario_${"c".repeat(32)}`;
  assert.doesNotThrow(() => validateCommandInput("game-open", { spectate: ["ai_2_1", "ai_turtle"] }));
  assert.throws(() => validateCommandInput("game-open", { opponent: "ai_2_1", spectate: ["ai_2_1", "ai_turtle"] }), (error) => error?.code === "invalidInput");
  assert.doesNotThrow(() => validateCommandInput("scenario-open", { id: "direct_reverse_order", unit: "tank", count: 1 }));
  assert.throws(() => validateCommandInput("scenario-open", { id: "bad/scenario", unit: "tank", count: 1 }), (error) => error?.code === "invalidInput");
  assert.doesNotThrow(() => validateCommandInput("scenario-capture-timelapse", { sessionId: scenarioSessionId, maxDurationMs: 1000, sampleEveryMs: 500 }));
  assert.equal(timelapseFrameBound(60_000, 1_000), 60, "one-minute default time-lapse samples exactly 60 frames");
  assert.equal(timelapseMayCaptureFrame(0, 1_000, 500, 1_500), true, "time-lapse always retains its initial frame");
  assert.equal(timelapseMayCaptureFrame(1, 1_000, 500, 1_499), true, "time-lapse samples later frames before its deadline");
  assert.equal(timelapseMayCaptureFrame(1, 1_000, 500, 1_500), false, "time-lapse does not start another frame at its duration ceiling");
  assert.throws(
    () => validateCommandInput("game-capture-timelapse", { sessionId: gameSessionId, maxDurationMs: GAME_TIMELAPSE_LIMITS.maxDurationMs, sampleEveryMs: 100 }),
    (error) => error?.code === "invalidInput",
    "time-lapse rejects requests beyond its sampled-frame bound",
  );
  assert.throws(
    () => validateCommandInput("game-screenshot", { sessionId: gameSessionId, region: { x: 0, y: 0, width: 1, height: 100 } }),
    (error) => error?.code === "invalidInput",
    "region screenshots reject unusable custom crops",
  );
  assert.equal(FIXED_CAPTURE_LIMITS.maxFrames, 1_800, "fixed capture supports one minute at 30 FPS");
  assert.deepEqual([0, 1, 2, 3].map((index) => fixedFrameTick(9, index, 60)), [9, 9, 10, 10]);
  assert.deepEqual(
    [...fixedRepresentativeIndices(1_800)],
    [0, 360, 720, 1079, 1439, 1799],
    "minute-scale fixed capture summaries include the first and final frames",
  );
  assert.deepEqual(
    boundedSummary(Array.from({ length: 400 }, (_, index) => index), INTERACT_SUMMARY_LIMITS.detailedAliases),
    { count: 400, details: Array.from({ length: 40 }, (_, index) => index), truncated: true },
    "fixed-capture manifest summaries preserve total/truncated metadata without 400 detailed rows",
  );

  service = new InteractService({ workspaceRoot: root, driverFactory: openInteractDriver });
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
  await assert.rejects(service.execute("capture-cancel", { sessionId: opened.sessionId }), (error) => error?.code === "captureInactive", "cancel reports an actionable error when no capture is active");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-fixed-contract-"));
  try {
    const tools = await checkMediaCapabilities();
    const generated = spawnSync(tools.ffmpeg, ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=63x63:rate=30:duration=0.1", "-frames:v", "3", "-f", "image2pipe", "-c:v", "png", "pipe:1"], { encoding: null, timeout: 15_000 });
    assert.equal(generated.status, 0, String(generated.stderr));
    const outputPath = path.join(tmp, "fixed.mp4");
    const contactSheetPath = path.join(tmp, "contact.png");
    const encoder = await createFixedCaptureEncoder({ outputPath, contactSheetPath, fps: 30, frameCount: 6 });
    encoder.write(generated.stdout);
    const media = await encoder.finish(3);
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

  await service.execute("close", { sessionId: opened.sessionId });
  const spectator = await service.execute("game-open", { spectate: ["ai_2_1", "ai_turtle"] });
  testArtifacts.ownGameSession(spectator.sessionId);
  assert.equal(spectator.capabilities.role, "spectator", "AI-vs-AI game sessions expose their spectator role");
  const timelapse = await service.execute("game-capture-timelapse", {
    sessionId: spectator.sessionId, maxDurationMs: 1_000, sampleEveryMs: 500, region: "minimap",
  });
  assert.equal(timelapse.frameSummary.count, 2, "game time-lapse returns sampled media through the bounded service surface");
  await service.execute("close", { sessionId: spectator.sessionId });

  const scenario = await service.execute("scenario-open", {
    id: "vehicle_corner_wall", unit: "tank", count: 1,
  });
  testArtifacts.ownScenarioSession(scenario.sessionId);
  assert.equal(scenario.capabilities.role, "observer", "scenario capabilities describe the observation namespace rather than its server seat");
  assert.deepEqual(scenario.capabilities.orders, [], "scenario capabilities expose no gameplay orders");
  assert.equal(scenario.capabilities.giveUp, false, "scenario capabilities expose no surrender mutation");
  const scenarioTimelapse = await service.execute("scenario-capture-timelapse", {
    sessionId: scenario.sessionId, maxDurationMs: 1_000, sampleEveryMs: 500,
  });
  assert.equal(scenarioTimelapse.frameSummary.count, 2, "dev scenarios retain the bounded time-lapse surface");

  console.log("✅ interact_fixed_capture_contracts.mjs: bounds, tick mapping, service, and media passed");
} finally {
  await service?.shutdown();
  testArtifacts.cleanup();
  testArtifacts.assertClean();
}
