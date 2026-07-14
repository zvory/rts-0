import crypto from "node:crypto";
import fs from "node:fs";

import {
  checkMediaCapabilities, createPngMp4Encoder, InteractRecordingError,
  representativeFrameIndices,
} from "./recording.ts";
import { ProcessRunner, ProcessRunnerError } from "./process_runner.ts";

export const FIXED_CAPTURE_LIMITS = Object.freeze({
  minFps: 10,
  maxFps: 60,
  maxFrames: 1_800,
  maxFrameBytes: 16 * 1024 * 1024,
  maxBytes: 64 * 1024 * 1024,
  representativeFrames: 6,
});

export function fixedFrameTick(startTick: number, frameIndex: number, fps: number) {
  return startTick + Math.floor(frameIndex * 30 / fps);
}

export function fixedRepresentativeIndices(frameCount: number, limit = FIXED_CAPTURE_LIMITS.representativeFrames) {
  return representativeFrameIndices(frameCount, limit);
}

export async function createFixedCaptureEncoder({
  outputPath, contactSheetPath, fps, frameCount,
  processRunner = new ProcessRunner(), signal,
}: {
  outputPath: string;
  contactSheetPath: string;
  fps: number;
  frameCount: number;
  processRunner?: ProcessRunner;
  signal?: AbortSignal;
}) {
  const tools = await checkMediaCapabilities({ processRunner, signal });
  const encoder = createPngMp4Encoder({ outputPath, fps, tools });
  return {
    write(buffer: Buffer) { return encoder.write(buffer); },
    async abort() { await encoder.abort(); },
    async finish() {
      await encoder.finish(75_000);
      const selection = [...fixedRepresentativeIndices(frameCount)]
        .map((index) => `eq(n\\,${index})`)
        .join("+");
      await run(tools.ffmpeg, [
        "-hide_banner", "-loglevel", "error", "-y", "-i", outputPath,
        "-vf", `select='${selection}',scale=480:300:force_original_aspect_ratio=decrease,pad=480:300:(ow-iw)/2:(oh-ih)/2:black,tile=3x2:padding=4:margin=4`,
        "-frames:v", "1", contactSheetPath,
      ], "fixed capture contact sheet", processRunner, signal);
      const stat = fs.statSync(outputPath);
      if (stat.size > FIXED_CAPTURE_LIMITS.maxBytes) throw new InteractRecordingError("captureTooLarge", "Fixed capture exceeded the 64 MiB bound.");
      let probeResult;
      try {
        probeResult = await processRunner.run(tools.ffprobe, [
        "-v", "error", "-select_streams", "v:0", "-count_frames",
        "-show_entries", "stream=codec_name,codec_tag_string,pix_fmt,width,height,r_frame_rate,nb_read_frames:format=format_name,duration",
        "-of", "json", outputPath,
        ], { timeoutMs: 15_000, signal });
      } catch (error) {
        throw new InteractRecordingError("mediaProbeFailed", `fixed capture probe failed: ${processFailure(error)}`);
      }
      if (probeResult.status !== 0) throw new InteractRecordingError("mediaProbeFailed", `fixed capture probe failed: ${String(probeResult.stderr || "unknown failure").slice(-800)}`);
      const parsed = parseProbe(probeResult.stdout);
      const stream = parsed.streams?.[0] || {};
      const probe = {
        codec: stream.codec_name, codecTag: stream.codec_tag_string, pixelFormat: stream.pix_fmt,
        container: parsed.format?.format_name, width: Number(stream.width), height: Number(stream.height),
        frameRate: stream.r_frame_rate, frameCount: Number(stream.nb_read_frames), durationSeconds: Number(parsed.format?.duration),
      };
      if (probe.codec !== "h264" || probe.codecTag !== "avc1" || probe.pixelFormat !== "yuv420p" || probe.container !== "mov,mp4,m4a,3gp,3g2,mj2" || probe.frameCount !== frameCount || probe.frameRate !== `${fps}/1` || !hasFastStart(outputPath)) {
        throw new InteractRecordingError("mediaProbeFailed", `Fixed capture media did not preserve mobile H.264 MP4/${fps} FPS/${frameCount} frames.`);
      }
      const contact = fs.readFileSync(contactSheetPath);
      return {
        tools, bytes: stat.size, probe,
        contactSheet: { width: contact.readUInt32BE(16), height: contact.readUInt32BE(20) },
      };
    },
  };
}

export function hashFrame(buffer: crypto.BinaryLike) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

async function run(command: string, args: string[], label: string, processRunner: ProcessRunner, signal?: AbortSignal) {
  let result;
  try {
    result = await processRunner.run(command, args, { timeoutMs: 30_000, signal });
  } catch (error) {
    throw new InteractRecordingError("mediaProcessingFailed", `${label} failed: ${processFailure(error)}`);
  }
  if (result.status !== 0) throw new InteractRecordingError("mediaProcessingFailed", `${label} failed: ${String(result.stderr || "unknown failure").slice(-800)}`);
}

function processFailure(error: unknown) {
  if (error instanceof ProcessRunnerError) return String(error.result?.stderr || error.result?.stdout || error.message).slice(-800);
  return String(error instanceof Error ? error.message : "unknown failure").slice(-800);
}

function hasFastStart(file: fs.PathOrFileDescriptor) {
  const bytes = fs.readFileSync(file);
  const moov = bytes.indexOf("moov");
  const mdat = bytes.indexOf("mdat");
  return moov >= 0 && mdat >= 0 && moov < mdat;
}

function parseProbe(text: string): {
  streams: Array<Record<string, unknown>>;
  format: Record<string, unknown>;
} {
  const value: unknown = JSON.parse(text);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new TypeError("ffprobe returned a non-object response.");
  const record = value as Record<string, unknown>;
  return {
    streams: Array.isArray(record.streams) ? record.streams.filter(isRecord) : [],
    format: isRecord(record.format) ? record.format : {},
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
