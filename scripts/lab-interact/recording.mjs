// Bounded real-time media recording for one Lab Interact page.

import fs from "node:fs";
import path from "node:path";
import { once } from "node:events";
import { spawn, spawnSync } from "node:child_process";

export const RECORDING_LIMITS = Object.freeze({
  fps: 30,
  defaultDurationMs: 10_000,
  maxDurationMs: 60_000,
  maxBytes: 64 * 1024 * 1024,
  maxFrames: 6,
  maxOperations: 200,
  maxAliases: 400,
  maxDetailedAliases: 40,
  minimumSourceCoverage: 0.8,
  stopTimeoutMs: 15_000,
  maxStopTimeoutMs: 45_000,
  mediaStageTimeoutMs: 15_000,
  maxMediaStageTimeoutMs: 75_000,
  maxMediaAuxiliaryTimeoutMs: 30_000,
});

export function recordingStopTimeoutMs(targetDurationMs) {
  return derivedTimeout(targetDurationMs, RECORDING_LIMITS.stopTimeoutMs, RECORDING_LIMITS.maxStopTimeoutMs, 0.5);
}

export function mediaStageTimeoutMs(targetDurationMs) {
  return derivedTimeout(targetDurationMs, RECORDING_LIMITS.mediaStageTimeoutMs, RECORDING_LIMITS.maxMediaStageTimeoutMs, 1);
}

export function mediaAuxiliaryTimeoutMs(targetDurationMs) {
  return derivedTimeout(targetDurationMs, RECORDING_LIMITS.mediaStageTimeoutMs, RECORDING_LIMITS.maxMediaAuxiliaryTimeoutMs, 0.25);
}

export function representativeFrameIndices(frameCount, limit = RECORDING_LIMITS.maxFrames) {
  const total = Math.max(1, Math.trunc(frameCount));
  const count = Math.min(total, Math.max(1, Math.trunc(limit)));
  if (count === 1) return new Set([0]);
  return new Set(Array.from(
    { length: count },
    (_, index) => Math.round(index * (total - 1) / (count - 1)),
  ));
}

export class LabInteractRecordingError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "LabInteractRecordingError";
    this.code = code;
    this.details = details;
  }
}

export function checkMediaCapabilities({
  ffmpeg = process.env.RTS_LAB_INTERACT_FFMPEG || "ffmpeg",
  ffprobe = process.env.RTS_LAB_INTERACT_FFPROBE || "ffprobe",
  requireH264 = true,
} = {}) {
  const encoderCheck = spawnSync(ffmpeg, ["-hide_banner", "-encoders"], { encoding: "utf8", timeout: 5_000 });
  if (encoderCheck.error?.code === "ENOENT") throw new LabInteractRecordingError("ffmpegUnavailable", `FFmpeg was not found at ${JSON.stringify(ffmpeg)}. Install FFmpeg or set RTS_LAB_INTERACT_FFMPEG.`);
  if (encoderCheck.status !== 0) throw new LabInteractRecordingError("ffmpegUnavailable", conciseToolFailure("FFmpeg capability check failed", encoderCheck));
  if (requireH264 && !/\blibx264\b/.test(encoderCheck.stdout || "")) throw new LabInteractRecordingError("h264Unavailable", "FFmpeg does not provide the libx264 encoder required for mobile-compatible MP4 output.");
  const probeCheck = spawnSync(ffprobe, ["-version"], { encoding: "utf8", timeout: 5_000 });
  if (probeCheck.error?.code === "ENOENT") throw new LabInteractRecordingError("ffprobeUnavailable", `ffprobe was not found at ${JSON.stringify(ffprobe)}. Install FFmpeg or set RTS_LAB_INTERACT_FFPROBE.`);
  if (probeCheck.status !== 0) throw new LabInteractRecordingError("ffprobeUnavailable", conciseToolFailure("ffprobe capability check failed", probeCheck));
  return {
    ffmpeg,
    ffprobe,
    ffmpegVersion: firstLine(encoderCheck.stderr) || "available",
    ffprobeVersion: firstLine(probeCheck.stdout) || "available",
  };
}

// Chrome's screencast timestamps describe compositor events, not a complete video timeline.
// This recorder deliberately uses one authority: cumulative elapsed monotonic wall time mapped
// to 30 FPS slots. Each slot receives the newest acknowledged CDP frame, and any reuse is counted.
export async function createWallClockRecorder({ page, outputPath, clip, scale, tools, maxDurationMs, timeoutMs = 15_000 }) {
  const fps = RECORDING_LIMITS.fps;
  const client = await page.createCDPSession();
  const viewport = page.viewport?.() || { deviceScaleFactor: 1 };
  const dpr = Number(viewport.deviceScaleFactor) || 1;
  const crop = {
    x: Math.max(0, Math.round(clip.x * dpr)),
    y: Math.max(0, Math.round(clip.y * dpr)),
    width: Math.max(2, Math.round(clip.width * dpr)),
    height: Math.max(2, Math.round(clip.height * dpr)),
  };
  const encoder = createPngMp4Encoder({ outputPath, fps, crop, scale, tools });
  let currentFrame = null;
  let currentFrameId = 0;
  let rawEvents = 0;
  let recordingEvents = 0;
  let firstChromeTimestamp = null;
  let lastChromeTimestamp = null;
  let largestChromeGapMs = 0;
  let startedNs = null;
  let interval = null;
  let stopping = false;
  let slotsQueued = 0;
  const sourceFramesUsed = new Set();
  const maximumSlots = Math.max(1, Math.ceil(maxDurationMs * fps / 1000));
  let firstFrameResolve;
  let firstFrameReject;
  const firstFrame = new Promise((resolve, reject) => { firstFrameResolve = resolve; firstFrameReject = reject; });

  const onFrame = ({ data, metadata = {}, sessionId }) => {
    void client.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
    if (stopping) return;
    const buffer = Buffer.from(data || "", "base64");
    if (buffer.length === 0) return;
    rawEvents += 1;
    if (startedNs != null) recordingEvents += 1;
    const timestamp = Number(metadata.timestamp);
    if (Number.isFinite(timestamp)) {
      if (firstChromeTimestamp == null) firstChromeTimestamp = timestamp;
      if (lastChromeTimestamp != null) largestChromeGapMs = Math.max(largestChromeGapMs, (timestamp - lastChromeTimestamp) * 1000);
      lastChromeTimestamp = timestamp;
    }
    currentFrame = buffer;
    currentFrameId += 1;
    firstFrameResolve();
  };
  client.on("Page.screencastFrame", onFrame);
  try {
    await client.send("Page.startScreencast", { format: "png", everyNthFrame: 1 });
    const timer = setTimeout(() => firstFrameReject(new LabInteractRecordingError("recordingEmpty", "Chrome did not provide an initial screencast frame.")), timeoutMs);
    timer.unref?.();
    try { await firstFrame; } finally { clearTimeout(timer); }
  } catch (error) {
    stopping = true;
    client.off("Page.screencastFrame", onFrame);
    await client.send("Page.stopScreencast").catch(() => {});
    await client.detach?.().catch(() => {});
    await encoder.abort();
    throw error;
  }

  const queueElapsedSlots = (nowNs = process.hrtime.bigint()) => {
    if (startedNs == null || stopping || !currentFrame) return;
    const elapsedMs = Math.min(maxDurationMs, Number(nowNs - startedNs) / 1e6);
    const due = Math.min(maximumSlots, Math.max(1, Math.ceil(elapsedMs * fps / 1000)));
    while (slotsQueued < due) {
      // The interval cannot await backpressure, but it must still observe a
      // failed encoder immediately enough to avoid an unhandled rejection.
      void encoder.write(currentFrame).catch(() => {});
      sourceFramesUsed.add(currentFrameId);
      slotsQueued += 1;
    }
  };

  return {
    start() {
      if (startedNs != null) return;
      startedNs = process.hrtime.bigint();
      queueElapsedSlots();
      interval = setInterval(queueElapsedSlots, 5);
      interval.unref?.();
    },
    async stop() {
      if (startedNs == null) this.start();
      clearInterval(interval);
      const stoppedNs = process.hrtime.bigint();
      const wallDurationMs = Math.min(
        maxDurationMs,
        Math.max(1, Number(stoppedNs - startedNs) / 1e6),
      );
      queueElapsedSlots(stoppedNs);
      stopping = true;
      client.off("Page.screencastFrame", onFrame);
      try {
        try {
          await client.send("Page.stopScreencast");
        } finally {
          await client.detach?.().catch(() => {});
        }
        const encodedFrames = Math.max(1, Math.ceil(wallDurationMs * fps / 1000));
        while (slotsQueued < encodedFrames) {
          void encoder.write(currentFrame).catch(() => {});
          sourceFramesUsed.add(currentFrameId);
          slotsQueued += 1;
        }
        const extra = slotsQueued - encodedFrames;
        if (extra > 0) {
          throw new LabInteractRecordingError("recordingClockDrift", `Recorder queued ${extra} frame slots beyond its measured wall duration.`);
        }
        await encoder.finish(recordingStopTimeoutMs(wallDurationMs));
        const used = sourceFramesUsed.size;
        const sourceCoverage = used / encodedFrames;
        const deficient = sourceCoverage < RECORDING_LIMITS.minimumSourceCoverage;
        return {
          wallDurationMs,
          diagnostics: {
            expectedAt30Fps: encodedFrames,
            encoded: encodedFrames,
            rawScreencastEvents: rawEvents,
            rawEventsDuringRecording: recordingEvents,
            sourceFramesUsed: used,
            reusedSourceFrameSlots: encodedFrames - used,
            sourceCoverage,
            deficient,
            minimumSourceCoverage: RECORDING_LIMITS.minimumSourceCoverage,
            chromeTimestampSpanSeconds: firstChromeTimestamp == null || lastChromeTimestamp == null ? null : lastChromeTimestamp - firstChromeTimestamp,
            largestChromeTimestampGapMs: largestChromeGapMs,
            warning: deficient
              ? `Source capture covered only ${(sourceCoverage * 100).toFixed(1)}% of output frame slots; use capture-fixed for reliable dense-scene video.`
              : null,
          },
        };
      } catch (error) {
        await encoder.abort().catch(() => {});
        throw error;
      }
    },
    async abort() {
      clearInterval(interval);
      stopping = true;
      client.off("Page.screencastFrame", onFrame);
      await client.send("Page.stopScreencast").catch(() => {});
      await client.detach?.().catch(() => {});
      await encoder.abort();
    },
  };
}

export function createPngMp4Encoder({ outputPath, fps, crop = null, scale = 1, tools }) {
  const filters = [];
  if (crop) filters.push(`crop=${crop.width}:${crop.height}:${crop.x}:${crop.y}`);
  if (scale !== 1) filters.push(`scale=ceil(iw*${scale}/2)*2:ceil(ih*${scale}/2)*2`);
  filters.push("pad=ceil(iw/2)*2:ceil(ih/2)*2");
  const child = spawn(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-f", "image2pipe", "-framerate", String(fps), "-i", "pipe:0",
    "-an", "-vf", filters.join(","), "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-profile:v", "main",
    "-pix_fmt", "yuv420p", "-tag:v", "avc1", "-movflags", "+faststart", outputPath,
  ], { stdio: ["pipe", "ignore", "pipe"] });
  let stderr = "";
  child.stderr.on("data", (chunk) => { stderr = `${stderr}${chunk}`.slice(-4000); });
  let tail = Promise.resolve();
  let spawnFailure = null;
  let writeFailure = null;
  // ChildProcess emits `error` (rather than a normal non-zero close) when the
  // executable cannot be started. Always observe it so a post-capability-check
  // launch failure rejects this recording instead of crashing the daemon.
  child.on("error", (error) => { spawnFailure = error; });
  child.stdin.on("error", (error) => { writeFailure = error; });
  return {
    write(buffer) {
      tail = tail.then(async () => {
        if (spawnFailure) throw encoderSpawnError(spawnFailure);
        if (writeFailure) throw encoderInputError(writeFailure);
        if (child.exitCode != null || child.signalCode != null) {
          throw encoderExitError(child.exitCode, child.signalCode, stderr);
        }
        if (!child.stdin.write(buffer)) {
          await waitForEncoderDrain(child, () => stderr, () => spawnFailure);
        }
      });
      return tail;
    },
    async finish(timeoutMs = 45_000) {
      await tail;
      if (spawnFailure) throw encoderSpawnError(spawnFailure);
      if (writeFailure) throw encoderInputError(writeFailure);
      if (child.exitCode != null || child.signalCode != null) {
        throw encoderExitError(child.exitCode, child.signalCode, stderr);
      }
      child.stdin.end();
      await waitForChild(child, timeoutMs, stderrFailure, spawnError);
    },
    async abort() {
      child.stdin.destroy();
      if (child.exitCode == null && child.signalCode == null) {
        const closed = once(child, "close");
        child.kill("SIGKILL");
        await closed.catch(() => {});
      }
      fs.rmSync(outputPath, { force: true });
    },
  };
  function stderrFailure() { return stderr; }
  function spawnError() { return spawnFailure; }
}

export function finalizeMp4Artifacts({ mp4Path, framesDir, contactSheetPath, targetDurationMs, tools, frameDiagnostics }) {
  const stat = safeStat(mp4Path);
  if (!stat || stat.size === 0) throw new LabInteractRecordingError("recordingEmpty", "FFmpeg produced no MP4 recording bytes.");
  if (stat.size > RECORDING_LIMITS.maxBytes) {
    fs.rmSync(mp4Path, { force: true });
    throw new LabInteractRecordingError("recordingTooLarge", `Final MP4 exceeded the ${RECORDING_LIMITS.maxBytes} byte limit and was deleted.`);
  }
  const probe = probeMedia(mp4Path, tools.ffprobe, "h264", "final MP4");
  const fastStart = hasFastStart(mp4Path);
  if (probe.container !== "mov,mp4,m4a,3gp,3g2,mj2" || probe.pixelFormat !== "yuv420p" || probe.codecTag !== "avc1" || !fastStart) {
    throw new LabInteractRecordingError("mediaProbeFailed", "Final MP4 is missing its mobile-compatible container, yuv420p/avc1 video, or fast-start metadata.");
  }
  probe.fastStart = fastStart;
  const expected = Math.max(1, Math.ceil(targetDurationMs * RECORDING_LIMITS.fps / 1000));
  if (probe.frameCount !== expected) throw new LabInteractRecordingError("mediaProbeFailed", `Final MP4 contains ${probe.frameCount ?? "an unknown number of"} frames; expected ${expected} from the wall clock.`);
  fs.mkdirSync(framesDir, { recursive: true });
  const representativeIndices = representativeFrameIndices(probe.frameCount);
  const selection = [...representativeIndices].map((index) => `eq(n\\,${index})`).join("+");
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-i", mp4Path,
    "-vf", `select='${selection}'`, "-fps_mode", "vfr",
    "-frames:v", String(representativeIndices.size), path.join(framesDir, "frame-%02d.png"),
  ], "representative frame extraction", mediaAuxiliaryTimeoutMs(targetDurationMs));
  const framePaths = representativeFrameNames(framesDir).map((name) => path.join(framesDir, name));
  if (framePaths.length === 0) throw new LabInteractRecordingError("frameExtractionFailed", "FFmpeg produced no representative PNG frames.");
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-framerate", "1", "-i", path.join(framesDir, "frame-%02d.png"),
    "-vf", "scale=480:300:force_original_aspect_ratio=decrease,pad=480:300:(ow-iw)/2:(oh-ih)/2:black,tile=3x2:padding=4:margin=4",
    "-frames:v", "1", contactSheetPath,
  ], "contact sheet generation", mediaAuxiliaryTimeoutMs(targetDurationMs));
  const contact = readPngDimensions(fs.readFileSync(contactSheetPath));
  return {
    bytes: stat.size,
    videoPath: mp4Path,
    probe,
    framePaths,
    contactSheet: { path: contactSheetPath, width: contact.width, height: contact.height },
    frameDiagnostics,
  };
}

function representativeFrameNames(framesDir) {
  return fs.readdirSync(framesDir).filter((name) => /^frame-\d+\.png$/.test(name)).sort().slice(0, RECORDING_LIMITS.maxFrames);
}

export function removePartialRecording(paths) {
  for (const value of paths || []) if (value) fs.rmSync(value, { recursive: true, force: true });
}

function probeMedia(file, ffprobe, expectedCodec, label) {
  const result = spawnSync(ffprobe, [
    "-v", "error", "-select_streams", "v:0", "-count_frames",
    "-show_entries", "stream=codec_name,codec_tag_string,pix_fmt,width,height,r_frame_rate,nb_read_frames:format=format_name,duration,size",
    "-of", "json", file,
  ], { encoding: "utf8", timeout: 10_000 });
  if (result.status !== 0) throw new LabInteractRecordingError("mediaProbeFailed", conciseToolFailure(`ffprobe rejected the ${label}`, result));
  let parsed;
  try { parsed = JSON.parse(result.stdout); } catch { throw new LabInteractRecordingError("mediaProbeFailed", "ffprobe returned invalid JSON."); }
  const stream = parsed.streams?.[0] || {};
  if (stream.codec_name !== expectedCodec) throw new LabInteractRecordingError("mediaProbeFailed", `Expected ${expectedCodec} ${label}, received ${stream.codec_name || "an unknown codec"}.`);
  return {
    codec: stream.codec_name,
    codecTag: stream.codec_tag_string || null,
    pixelFormat: stream.pix_fmt || null,
    container: parsed.format?.format_name || null,
    width: Number(stream.width) || null,
    height: Number(stream.height) || null,
    frameRate: stream.r_frame_rate || null,
    frameCount: /^\d+$/.test(stream.nb_read_frames || "") ? Number(stream.nb_read_frames) : null,
    durationSeconds: Number(parsed.format?.duration) || null,
    probedBytes: Number(parsed.format?.size) || null,
  };
}

function runTool(command, args, label, timeoutMs = RECORDING_LIMITS.mediaStageTimeoutMs) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: timeoutMs });
  if (result.status !== 0) throw new LabInteractRecordingError("mediaProcessingFailed", conciseToolFailure(`${label} failed`, result));
}

function waitForEncoderDrain(child, stderr, spawnFailure) {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      child.stdin.off("drain", onDrain);
      child.stdin.off("error", onError);
      child.off("close", onClose);
    };
    const onDrain = () => {
      cleanup();
      resolve();
    };
    const onError = (error) => {
      cleanup();
      reject(spawnFailure() ? encoderSpawnError(spawnFailure()) : encoderInputError(error));
    };
    const onClose = (code, signal) => {
      cleanup();
      reject(spawnFailure() ? encoderSpawnError(spawnFailure()) : encoderExitError(code, signal, stderr()));
    };
    child.stdin.once("drain", onDrain);
    child.stdin.once("error", onError);
    child.once("close", onClose);
  });
}

async function waitForChild(child, timeoutMs, stderr, spawnFailure) {
  if (spawnFailure()) throw encoderSpawnError(spawnFailure());
  if (child.exitCode != null || child.signalCode != null) {
    if (child.exitCode !== 0) throw encoderExitError(child.exitCode, child.signalCode, stderr());
    return;
  }
  let timer;
  try {
    const [code, signal] = await Promise.race([
      once(child, "close"),
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          child.kill("SIGKILL");
          reject(new LabInteractRecordingError("recordingFinalizeTimeout", `Recording encoder did not finalize within ${timeoutMs}ms.`));
        }, timeoutMs);
        timer.unref?.();
      }),
    ]);
    if (spawnFailure()) throw encoderSpawnError(spawnFailure());
    if (code !== 0) throw new LabInteractRecordingError("mediaProcessingFailed", `H.264 encoder failed${signal ? ` (${signal})` : ""}: ${String(stderr() || "unknown failure").trim().slice(-800)}`);
  } finally {
    clearTimeout(timer);
  }
}

function encoderSpawnError(error) {
  return new LabInteractRecordingError(
    "mediaProcessingFailed",
    `H.264 encoder could not start: ${String(error?.message || error || "unknown failure").trim().slice(-800)}`,
  );
}

function encoderInputError(error) {
  return new LabInteractRecordingError(
    "mediaProcessingFailed",
    `H.264 encoder input failed: ${String(error?.message || error || "unknown failure").trim().slice(-800)}`,
  );
}

function encoderExitError(code, signal, stderr) {
  return new LabInteractRecordingError(
    "mediaProcessingFailed",
    `H.264 encoder failed${signal ? ` (${signal})` : code == null ? "" : ` with exit ${code}`}: ${String(stderr || "unknown failure").trim().slice(-800)}`,
  );
}

function derivedTimeout(targetDurationMs, minimumMs, maximumMs, durationScale) {
  const duration = Number.isFinite(targetDurationMs) && targetDurationMs > 0 ? targetDurationMs : 0;
  return Math.min(maximumMs, Math.max(minimumMs, Math.ceil(minimumMs + duration * durationScale)));
}

function conciseToolFailure(prefix, result) {
  const detail = String(result.error?.message || result.stderr || result.stdout || "unknown failure").trim().split("\n").slice(-4).join("; ").slice(0, 800);
  return `${prefix}: ${detail}`;
}
function firstLine(value) { return String(value || "").split("\n").find(Boolean) || ""; }
function safeStat(file) { try { return fs.statSync(file); } catch { return null; } }
function hasFastStart(file) {
  const bytes = fs.readFileSync(file);
  const moov = bytes.indexOf(Buffer.from("moov"));
  const mdat = bytes.indexOf(Buffer.from("mdat"));
  return moov >= 0 && mdat >= 0 && moov < mdat;
}
function readPngDimensions(buffer) {
  if (!Buffer.isBuffer(buffer) || buffer.length < 24 || buffer.toString("ascii", 1, 4) !== "PNG") throw new LabInteractRecordingError("contactSheetInvalid", "Contact sheet is not a valid PNG.");
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}
