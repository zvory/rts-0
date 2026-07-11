// Bounded real-time media recording for one Lab Interact page.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export const RECORDING_LIMITS = Object.freeze({
  defaultDurationMs: 10_000,
  maxDurationMs: 30_000,
  maxBytes: 64 * 1024 * 1024,
  maxFrames: 6,
  maxOperations: 200,
  maxAliases: 100,
  stopTimeoutMs: 15_000,
});

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
} = {}) {
  const encoderCheck = spawnSync(ffmpeg, ["-hide_banner", "-encoders"], { encoding: "utf8", timeout: 5_000 });
  if (encoderCheck.error?.code === "ENOENT") throw new LabInteractRecordingError("ffmpegUnavailable", `FFmpeg was not found at ${JSON.stringify(ffmpeg)}. Install FFmpeg or set RTS_LAB_INTERACT_FFMPEG.`);
  if (encoderCheck.status !== 0) throw new LabInteractRecordingError("ffmpegUnavailable", conciseToolFailure("FFmpeg capability check failed", encoderCheck));
  if (!/\blibvpx-vp9\b/.test(encoderCheck.stdout || "")) throw new LabInteractRecordingError("vp9Unavailable", "FFmpeg does not provide the libvpx-vp9 encoder required by Puppeteer WebM recording.");
  if (!/\blibx264\b/.test(encoderCheck.stdout || "")) throw new LabInteractRecordingError("h264Unavailable", "FFmpeg does not provide the libx264 encoder required for mobile-compatible MP4 output.");
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

export async function stopRecorderWithin(recorder, timeoutMs = RECORDING_LIMITS.stopTimeoutMs) {
  let timer;
  try {
    await Promise.race([
      recorder.stop(),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new LabInteractRecordingError("recordingFinalizeTimeout", `Recording did not finalize within ${timeoutMs}ms.`)), timeoutMs);
        timer.unref?.();
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export async function waitForMediaFile(file, timeoutMs = 2_000) {
  const deadline = Date.now() + timeoutMs;
  let previousSize = -1;
  while (Date.now() < deadline) {
    const size = safeStat(file)?.size || 0;
    if (size > 0 && size === previousSize) return size;
    previousSize = size;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new LabInteractRecordingError("recordingEmpty", "Finalized recording bytes were not available within the bounded flush wait.");
}

export function finalizeMedia({ webmPath, mp4Path, framesDir, contactSheetPath, targetDurationMs, tools }) {
  const sourceStat = safeStat(webmPath);
  if (!sourceStat || sourceStat.size === 0) throw new LabInteractRecordingError("recordingEmpty", "Chrome produced no recording bytes.");
  if (sourceStat.size > RECORDING_LIMITS.maxBytes) {
    fs.rmSync(webmPath, { force: true });
    throw new LabInteractRecordingError("recordingTooLarge", `Recording exceeded the ${RECORDING_LIMITS.maxBytes} byte limit and was deleted.`);
  }
  if (!Number.isFinite(targetDurationMs) || targetDurationMs <= 0) throw new LabInteractRecordingError("recordingDurationInvalid", "Recording wall duration was unavailable during finalization.");
  const sourceProbe = probeMedia(webmPath, tools.ffprobe, "vp9", "source WebM");
  const targetDurationSeconds = Math.max(targetDurationMs / 1000, 1 / 30);
  const capturedFrames = Math.max(1, Number(sourceProbe.frameCount) || Math.round((sourceProbe.durationSeconds || 0) * 25) || 1);
  const capturedFrameInterval = targetDurationSeconds / capturedFrames;
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-i", webmPath,
    "-an", "-vf", `setpts=N*${capturedFrameInterval.toFixed(12)}/TB,fps=30,tpad=stop_mode=clone:stop_duration=${targetDurationSeconds.toFixed(6)}`,
    "-t", targetDurationSeconds.toFixed(6), "-c:v", "libx264", "-preset", "veryfast", "-crf", "23",
    "-profile:v", "main", "-pix_fmt", "yuv420p", "-tag:v", "avc1",
    "-movflags", "+faststart", mp4Path,
  ], "mobile MP4 transcode");
  const mp4Stat = safeStat(mp4Path);
  if (!mp4Stat || mp4Stat.size === 0) throw new LabInteractRecordingError("recordingEmpty", "FFmpeg produced no MP4 recording bytes.");
  if (mp4Stat.size > RECORDING_LIMITS.maxBytes) {
    fs.rmSync(mp4Path, { force: true });
    fs.rmSync(webmPath, { force: true });
    throw new LabInteractRecordingError("recordingTooLarge", `Final MP4 exceeded the ${RECORDING_LIMITS.maxBytes} byte limit and was deleted.`);
  }
  const probe = probeMedia(mp4Path, tools.ffprobe, "h264", "final MP4");
  if (probe.container !== "mov,mp4,m4a,3gp,3g2,mj2" || probe.pixelFormat !== "yuv420p" || probe.codecTag !== "avc1" || !hasFastStart(mp4Path)) {
    throw new LabInteractRecordingError("mediaProbeFailed", "Final MP4 is missing its mobile-compatible container, yuv420p/avc1 video, or fast-start metadata.");
  }
  fs.rmSync(webmPath, { force: true });
  fs.mkdirSync(framesDir, { recursive: true });
  const duration = Math.max(Number(probe.durationSeconds) || 0.001, 0.001);
  const activityFrameLimit = RECORDING_LIMITS.maxFrames - 1;
  const interval = Math.max(duration / Math.max(activityFrameLimit, 1), 0.05);
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-i", mp4Path,
    "-vf", `fps=1/${interval}`, "-frames:v", String(activityFrameLimit),
    path.join(framesDir, "frame-%02d.png"),
  ], "representative frame extraction");
  const activityFrameCount = representativeFrameNames(framesDir).length;
  const finalFrameNumber = Math.min(activityFrameCount + 1, RECORDING_LIMITS.maxFrames);
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-sseof", "-0.001", "-i", mp4Path,
    "-frames:v", "1", path.join(framesDir, `frame-${String(finalFrameNumber).padStart(2, "0")}.png`),
  ], "final frame extraction");
  const framePaths = representativeFrameNames(framesDir)
    .map((name) => path.join(framesDir, name));
  if (framePaths.length === 0) throw new LabInteractRecordingError("frameExtractionFailed", "FFmpeg produced no representative PNG frames.");
  runTool(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-framerate", "1", "-i", path.join(framesDir, "frame-%02d.png"),
    "-vf", "scale=480:300:force_original_aspect_ratio=decrease,pad=480:300:(ow-iw)/2:(oh-ih)/2:black,tile=3x2:padding=4:margin=4",
    "-frames:v", "1", contactSheetPath,
  ], "contact sheet generation");
  const contact = readPngDimensions(fs.readFileSync(contactSheetPath));
  const expectedFrames = Math.max(1, Math.round(targetDurationSeconds * 30));
  const encodedFrames = Number.isInteger(probe.frameCount) ? probe.frameCount : null;
  return {
    bytes: mp4Stat.size,
    videoPath: mp4Path,
    probe,
    framePaths,
    contactSheet: { path: contactSheetPath, width: contact.width, height: contact.height },
    frameDiagnostics: {
      expectedAt30Fps: expectedFrames,
      captured: capturedFrames,
      encoded: encodedFrames,
      droppedEstimate: Math.max(0, expectedFrames - capturedFrames),
      duplicatedEstimate: encodedFrames == null ? null : Math.max(0, encodedFrames - capturedFrames),
      sourceDurationSeconds: sourceProbe.durationSeconds,
      wallDurationSeconds: targetDurationSeconds,
      caveat: "The MP4 timeline is normalized to measured wall duration at 30 FPS; duplicated frames compensate for nondeterministic Chrome screencast delivery.",
    },
  };
}

function representativeFrameNames(framesDir) {
  return fs.readdirSync(framesDir)
    .filter((name) => /^frame-\d+\.png$/.test(name))
    .sort()
    .slice(0, RECORDING_LIMITS.maxFrames);
}

export function removePartialRecording(paths) {
  for (const value of paths || []) {
    if (value) fs.rmSync(value, { recursive: true, force: true });
  }
}

function probeMedia(file, ffprobe, expectedCodec, label) {
  const result = spawnSync(ffprobe, [
    "-v", "error", "-select_streams", "v:0",
    "-show_entries", "stream=codec_name,codec_tag_string,pix_fmt,width,height,r_frame_rate,nb_frames:format=format_name,duration,size",
    "-of", "json", file,
  ], { encoding: "utf8", timeout: 10_000 });
  if (result.status !== 0) throw new LabInteractRecordingError("mediaProbeFailed", conciseToolFailure(`ffprobe rejected the ${label}`, result));
  let parsed;
  try { parsed = JSON.parse(result.stdout); } catch { throw new LabInteractRecordingError("mediaProbeFailed", "ffprobe returned invalid JSON."); }
  const stream = parsed.streams?.[0] || {};
  if (stream.codec_name !== expectedCodec) throw new LabInteractRecordingError("mediaProbeFailed", `Expected ${expectedCodec} ${label}, received ${stream.codec_name || "an unknown codec"}.`);
  const packetTimeline = parsed.format?.duration && stream.nb_frames
    ? null
    : probePacketTimeline(file, ffprobe);
  return {
    codec: stream.codec_name,
    codecTag: stream.codec_tag_string || null,
    pixelFormat: stream.pix_fmt || null,
    container: parsed.format?.format_name || null,
    width: Number(stream.width) || null,
    height: Number(stream.height) || null,
    frameRate: stream.r_frame_rate || null,
    frameCount: /^\d+$/.test(stream.nb_frames || "") ? Number(stream.nb_frames) : packetTimeline?.frameCount || null,
    durationSeconds: Number(parsed.format?.duration) || packetTimeline?.durationSeconds || null,
    probedBytes: Number(parsed.format?.size) || null,
  };
}

function probePacketTimeline(file, ffprobe) {
  const result = spawnSync(ffprobe, [
    "-v", "error", "-select_streams", "v:0",
    "-show_entries", "packet=pts_time,duration_time", "-of", "csv=p=0", file,
  ], { encoding: "utf8", timeout: 10_000, maxBuffer: 1024 * 1024 });
  if (result.status !== 0) return null;
  const packets = result.stdout.trim().split("\n").map((line) => {
    const [pts, duration] = line.split(",").map(Number);
    return Number.isFinite(pts) ? { pts, duration: Number.isFinite(duration) ? duration : 0 } : null;
  }).filter(Boolean);
  if (packets.length === 0) return null;
  const firstPts = packets[0].pts;
  const last = packets[packets.length - 1];
  return {
    frameCount: packets.length,
    durationSeconds: Math.max(0, last.pts + last.duration - firstPts),
  };
}

function runTool(command, args, label) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 15_000 });
  if (result.status !== 0) throw new LabInteractRecordingError("mediaProcessingFailed", conciseToolFailure(`${label} failed`, result));
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
