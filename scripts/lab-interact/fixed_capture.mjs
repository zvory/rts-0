import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { checkMediaCapabilities, LabInteractRecordingError } from "./recording.mjs";

export const FIXED_CAPTURE_LIMITS = Object.freeze({
  minFps: 10,
  maxFps: 60,
  maxFrames: 180,
  maxFrameBytes: 16 * 1024 * 1024,
  maxSequenceBytes: 256 * 1024 * 1024,
  maxBytes: 64 * 1024 * 1024,
});

export function fixedFrameTick(startTick, frameIndex, fps) {
  return startTick + Math.floor(frameIndex * 30 / fps);
}

export function encodeFixedCapture({ framesDir, outputPath, contactSheetPath, fps, frameCount }) {
  const tools = checkMediaCapabilities({ requireVp9: false });
  run(tools.ffmpeg, [
    "-hide_banner", "-loglevel", "error", "-y", "-framerate", String(fps),
    "-i", path.join(framesDir, "frame-%04d.png"), "-an", "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-c:v", "libx264",
    "-preset", "veryfast", "-crf", "23", "-profile:v", "main",
    "-pix_fmt", "yuv420p", "-tag:v", "avc1", "-movflags", "+faststart", outputPath,
  ], "fixed capture encode");
  run(tools.ffmpeg, ["-hide_banner", "-loglevel", "error", "-y", "-i", outputPath, "-vf", `select='not(mod(n\\,${Math.max(1, Math.floor(frameCount / 6))}))',scale=480:300:force_original_aspect_ratio=decrease,pad=480:300:(ow-iw)/2:(oh-ih)/2:black,tile=3x2:padding=4:margin=4`, "-frames:v", "1", contactSheetPath], "fixed capture contact sheet");
  const stat = fs.statSync(outputPath);
  if (stat.size > FIXED_CAPTURE_LIMITS.maxBytes) throw new LabInteractRecordingError("captureTooLarge", "Fixed capture exceeded the 64 MiB bound.");
  const probeResult = spawnSync(tools.ffprobe, ["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries", "stream=codec_name,codec_tag_string,pix_fmt,width,height,r_frame_rate,nb_read_frames:format=format_name,duration", "-of", "json", outputPath], { encoding: "utf8", timeout: 10_000 });
  if (probeResult.status !== 0) throw new LabInteractRecordingError("mediaProbeFailed", `fixed capture probe failed: ${String(probeResult.stderr || probeResult.error?.message || "unknown failure").slice(-800)}`);
  const parsed = JSON.parse(probeResult.stdout);
  const stream = parsed.streams?.[0] || {};
  const probe = { codec: stream.codec_name, codecTag: stream.codec_tag_string, pixelFormat: stream.pix_fmt, container: parsed.format?.format_name, width: Number(stream.width), height: Number(stream.height), frameRate: stream.r_frame_rate, frameCount: Number(stream.nb_read_frames), durationSeconds: Number(parsed.format?.duration) };
  if (probe.codec !== "h264" || probe.codecTag !== "avc1" || probe.pixelFormat !== "yuv420p" || probe.container !== "mov,mp4,m4a,3gp,3g2,mj2" || probe.frameCount !== frameCount || probe.frameRate !== `${fps}/1` || !hasFastStart(outputPath)) {
    throw new LabInteractRecordingError("mediaProbeFailed", `Fixed capture media did not preserve mobile H.264 MP4/${fps} FPS/${frameCount} frames.`);
  }
  const contact = fs.readFileSync(contactSheetPath);
  const contactSheet = { width: contact.readUInt32BE(16), height: contact.readUInt32BE(20) };
  return { tools, bytes: stat.size, probe, contactSheet };
}

export function hashFrame(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function run(command, args, label) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 30_000 });
  if (result.status !== 0) throw new LabInteractRecordingError("mediaProcessingFailed", `${label} failed: ${String(result.stderr || result.error?.message || "unknown failure").slice(-800)}`);
}

function hasFastStart(file) {
  const bytes = fs.readFileSync(file);
  const moov = bytes.indexOf(Buffer.from("moov"));
  const mdat = bytes.indexOf(Buffer.from("mdat"));
  return moov >= 0 && mdat >= 0 && moov < mdat;
}
