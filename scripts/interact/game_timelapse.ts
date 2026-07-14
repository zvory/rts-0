import fs from "node:fs";
import path from "node:path";
import type { Page, Viewport } from "puppeteer-core";

import { resolveCaptureRegion } from "./capture_region.ts";
import type { CaptureRegion } from "./capture_region.ts";
import { createFixedCaptureEncoder, FIXED_CAPTURE_LIMITS, hashFrame } from "./fixed_capture.ts";
import { removePartialRecording } from "./recording.ts";
import { interactArtifactRoot } from "./interact_paths.ts";
import type { InteractDriver } from "./driver.ts";

type JsonObject = Record<string, unknown>;

export const GAME_TIMELAPSE_LIMITS = Object.freeze({
  defaultDurationMs: 60_000,
  maxDurationMs: 300_000,
  defaultSampleEveryMs: 1_000,
  minSampleEveryMs: 250,
  maxSampleEveryMs: 60_000,
  defaultFps: 30,
  minFps: 10,
  maxFps: 60,
  maxFrames: 1_800,
  minSpeed: 0.125,
  maxSpeed: 8,
});

export function timelapseFrameBound(durationMs: number, sampleEveryMs: number) {
  return Math.max(1, Math.ceil(durationMs / sampleEveryMs));
}

export function timelapseMayCaptureFrame(frameIndex: number, startedMs: number, maxDurationMs: number, nowMs: number) {
  return frameIndex === 0 || nowMs < startedMs + maxDurationMs;
}

export async function captureGameTimelapse(driver: InteractDriver, {
  sessionId, name = "timelapse", maxDurationMs = GAME_TIMELAPSE_LIMITS.defaultDurationMs,
  sampleEveryMs = GAME_TIMELAPSE_LIMITS.defaultSampleEveryMs, fps = GAME_TIMELAPSE_LIMITS.defaultFps,
  speed = GAME_TIMELAPSE_LIMITS.maxSpeed, viewport = null, region = "viewport", presentation = "normal",
}: {
  sessionId?: string; name?: string; maxDurationMs?: number; sampleEveryMs?: number; fps?: number;
  speed?: number; viewport?: Viewport | null; region?: CaptureRegion; presentation?: "normal" | "clean";
} = {}) {
  if (driver.recording) throw driver.decorateError(codedError("recordingActive", "Time-lapse capture is unavailable while real-time recording is active."));
  if (driver.fixedCapture) throw driver.decorateError(codedError("captureActive", "Another fixed or time-lapse capture is already active."));
  const normalizedSessionId = validSessionId(sessionId);
  const artifactName = safeName(name);
  const abortController = new AbortController();
  const startStatus = await driver.callBridge("status", {});
  driver.fixedCapture = {
    active: true, cancelled: false, name: artifactName, fps,
    frameCount: timelapseFrameBound(maxDurationMs, sampleEveryMs), frameIndex: 0,
    startStatus, abortController,
  };
  try {
    const result = await captureSampledTimelapse({
      page: driver.page!, sessionId: normalizedSessionId, name: artifactName,
      root: driver.workspace!.root, artifactRoot: interactArtifactRoot(driver.options.mode),
      viewport, region, presentation, speed, maxDurationMs, sampleEveryMs, fps,
      signal: abortController.signal,
      onProgress: (frameIndex) => { if (driver.fixedCapture) driver.fixedCapture.frameIndex = frameIndex; },
      call: (method, input) => driver.callBridge(method, input),
      ready: () => driver.waitForCaptureReadiness([]),
      metadata: {
        workspace: driver.workspace, serverBuild: driver.server?.build || null,
        map: driver.options.map, matchup: driver.options.spectate,
        runtime: { node: process.version, platform: process.platform, architecture: process.arch, browser: driver.browserVersion || null },
        camera: startStatus.camera || null,
      },
    });
    driver.lastFixedCapture = result;
    return result;
  } catch (error) {
    if (abortController.signal.aborted) throw driver.decorateError(codedError("captureCancelled", "Time-lapse capture was cancelled and its partial artifacts were removed."));
    throw driver.decorateError(error);
  } finally {
    driver.fixedCapture = null;
  }
}

async function captureSampledTimelapse({
  page, sessionId, name, root, artifactRoot, viewport, region, presentation, speed,
  maxDurationMs, sampleEveryMs, fps, signal, onProgress, call, metadata,
  ready,
}: {
  page: Page;
  sessionId: string;
  name: string;
  root: string;
  artifactRoot: string;
  viewport: Viewport | null;
  region: CaptureRegion;
  presentation: "normal" | "clean";
  speed: number;
  maxDurationMs: number;
  sampleEveryMs: number;
  fps: number;
  signal: AbortSignal;
  onProgress: (frameIndex: number, frameCount: number) => void;
  call: (method: string, input: JsonObject) => Promise<JsonObject>;
  ready: () => Promise<JsonObject>;
  metadata: JsonObject;
}) {
  const originalViewport = page.viewport?.() || null;
  const maximumFrames = timelapseFrameBound(maxDurationMs, sampleEveryMs);
  let captureDir = "";
  let encoder: Awaited<ReturnType<typeof createFixedCaptureEncoder>> | null = null;
  let originalSpeed: number | null = null;
  let speedChangeAttempted = false;
  try {
    if (viewport) await page.setViewport(viewport);
    if (region === "minimap" && presentation === "clean") {
      throw codedError("invalidPresentation", "The minimap is hidden in clean presentation; use normal presentation for a minimap time-lapse.");
    }
    await call("presentation", { mode: presentation === "clean" ? "clean" : "default" });
    await page.evaluate(() => document.fonts?.ready || Promise.resolve());
    await ready();
    const resolvedRegion = await resolveCaptureRegion(page, region);

    const suffix = new Date().toISOString().replace(/[:.]/g, "-");
    captureDir = path.join(root, artifactRoot, sessionId, "timelapse", `${name}-${suffix}`);
    fs.mkdirSync(captureDir, { recursive: true });
    const videoPath = path.join(captureDir, `${name}.mp4`);
    const contactSheetPath = path.join(captureDir, `${name}-contact-sheet.png`);
    const manifestPath = path.join(captureDir, `${name}.json`);
    encoder = await createFixedCaptureEncoder({
      outputPath: videoPath, contactSheetPath, fps, frameCount: maximumFrames, signal,
    });

    // Prepare all local media machinery before accelerating authoritative time so
    // encoder capability checks cannot consume an unrecorded portion of the match.
    const before = await call("status", {});
    originalSpeed = finiteNumber((before.roomTime as JsonObject | undefined)?.speed);
    speedChangeAttempted = true;
    const speedResult = await call("time", { action: "speed", speed });

    const startedMs = Date.now();
    const frames: JsonObject[] = [];
    let endStatus = before;
    for (let index = 0; index < maximumFrames; index += 1) {
      if (index > 0) await waitUntil(startedMs + index * sampleEveryMs, signal);
      if (signal.aborted) throw codedError("captureCancelled", "Time-lapse capture was cancelled and its partial artifacts were removed.");
      if (!timelapseMayCaptureFrame(index, startedMs, maxDurationMs, Date.now())) break;
      const png = Buffer.from(await page.screenshot({ type: "png", clip: resolvedRegion.clip }) || []);
      if (png.length === 0) throw codedError("captureEmpty", "Chrome returned an empty time-lapse frame.");
      if (png.length > FIXED_CAPTURE_LIMITS.maxFrameBytes) throw codedError("captureTooLarge", "One time-lapse PNG exceeded its bounded frame budget.");
      await encoder.write(png);
      endStatus = await call("status", {});
      frames.push({
        index, capturedAtMs: Date.now() - startedMs, snapshotTick: endStatus.snapshotTick ?? null,
        phase: endStatus.phase ?? null, sha256: hashFrame(png), bytes: png.length,
      });
      onProgress(index + 1, maximumFrames);
      if (endStatus.phase === "concluded") break;
    }
    const media = await encoder.finish(frames.length);
    encoder = null;
    const manifest = {
      schemaVersion: 1,
      kind: "interactGameTimelapse",
      createdAt: new Date(startedMs).toISOString(),
      finalizedAt: new Date().toISOString(),
      nondeterministic: true,
      ...metadata,
      source: {
        maxDurationMs, actualDurationMs: Date.now() - startedMs, sampleEveryMs, simulationSpeed: speed,
        stoppedBy: endStatus.phase === "concluded" ? "matchConcluded" : "duration",
      },
      output: { fps, frameCount: frames.length, maximumFrames },
      region: resolvedRegion,
      authoritative: {
        startTick: speedResult.snapshotTick ?? before.snapshotTick ?? null,
        endTick: endStatus.snapshotTick ?? null,
        phase: endStatus.phase ?? null,
      },
      frames,
      media: { videoPath, contactSheetPath, bytes: media.bytes, tools: media.tools, probe: media.probe },
    };
    fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
    return {
      videoPath, contactSheetPath, manifestPath, authoritative: manifest.authoritative,
      probe: media.probe, source: manifest.source, region: resolvedRegion,
      frameSummary: { count: frames.length, uniqueHashes: new Set(frames.map((frame) => frame.sha256)).size, detailsInManifest: true },
    };
  } catch (error) {
    if (encoder) await encoder.abort().catch(() => {});
    removePartialRecording([captureDir]);
    throw error;
  } finally {
    if (speedChangeAttempted && originalSpeed != null) await call("time", { action: "speed", speed: originalSpeed }).catch(() => {});
    await call("presentation", { mode: "default" }).catch(() => {});
    if (originalViewport) await page.setViewport(originalViewport).catch(() => {});
  }
}

function waitUntil(deadlineMs: number, signal: AbortSignal) {
  const remaining = Math.max(0, deadlineMs - Date.now());
  if (signal.aborted) return Promise.reject(codedError("captureCancelled", "Time-lapse capture was cancelled."));
  return new Promise<void>((resolve, reject) => {
    const timer = setTimeout(done, remaining);
    const onAbort = () => done(codedError("captureCancelled", "Time-lapse capture was cancelled."));
    signal.addEventListener("abort", onAbort, { once: true });
    function done(error?: Error) {
      clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      if (error) reject(error); else resolve();
    }
  });
}

function finiteNumber(value: unknown) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function codedError(code: string, message: string) {
  return Object.assign(new Error(message), { code });
}

function validSessionId(value: unknown) {
  const sessionId = String(value || "");
  if (!/^(?:lab|game)_[a-f0-9]{32}$/.test(sessionId)) throw codedError("invalidSession", "sessionId must be a valid Interact session id.");
  return sessionId;
}

function safeName(value: unknown) {
  const name = String(value || "");
  return /^[A-Za-z0-9_-]{1,48}$/.test(name) ? name : "timelapse";
}
