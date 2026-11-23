import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { ProcessRunner } from "./process_runner.ts";
import { MediaStore, MAX_MEDIA_BYTES, type MediaArtifact } from "./media_store.ts";
import { MEDIA_PRESETS, mediaError, validateMediaInput, type MediaInput } from "./media_inputs.ts";
import { writeMediaLabel } from "./media_labels.ts";
import { publishTailnetPreview } from "../tailnet-preview.mjs";

interface Probe { width: number; height: number; fps: number; frames: number; duration: number; codec: string }
export class MediaFinisher {
  store: MediaStore;
  runner: ProcessRunner;
  ffmpeg: string;
  ffprobe: string;
  publisher: typeof publishTailnetPreview;
  constructor({ store = new MediaStore(), runner = new ProcessRunner(), env = process.env, publisher = publishTailnetPreview } = {}) {
    this.store = store; this.runner = runner; this.publisher = publisher;
    this.ffmpeg = env.RTS_INTERACT_FFMPEG || "ffmpeg"; this.ffprobe = env.RTS_INTERACT_FFPROBE || "ffprobe";
  }
  async execute(command: string, rawInput: unknown) {
    const input = validateMediaInput(command, rawInput);
    if (command === "media-inspect") {
      const artifact = this.store.read(input.artifactId!);
      return { ...this.store.describe(artifact), probe: await this.probe(this.store.file(artifact)),
        processing: artifact.provenance.processing || null };
    }
    if (command === "media-publish") return this.publish(this.store.read(input.artifactId!));
    if (command === "media-export" || command === "media-compare") return this.finish(input, command === "media-compare");
    throw mediaError("unknownCommand", "Unknown media command.");
  }
  async publish(artifact: MediaArtifact) {
    const preview = await this.publisher({ source: this.store.file(artifact), ttlMs: 24 * 60 * 60 * 1000 });
    return { ...this.store.describe(artifact), preview: { ...preview, mimeType: `video/${artifact.format}`,
      instruction: preview.localPath ? "Display or attach localPath directly; do not troubleshoot Tailscale." : "Share this URL as a clickable link." } };
  }
  async finish(input: MediaInput, comparison: boolean) {
    const artifacts = (comparison ? input.artifacts! : [input.artifactId!]).map(id => this.store.read(id));
    const difference = comparison && input.layout === "difference";
    if (difference) assertSynchronizedSources(artifacts);
    const sources = artifacts.map(a => this.store.file(a, a.lossless));
    const probes = await Promise.all(sources.map(source => this.probe(source)));
    if (difference && !isDeepStrictEqual(probes[0], probes[1])) throw mediaError("mediaNotSynchronized", "Difference sources must have identical dimensions, frame count, frame rate and duration.");
    const format = input.format || "webm", preset = input.preset || "balanced", speed = input.speed || 1;
    const encoders = await this.run(this.ffmpeg, ["-hide_banner", "-encoders"], 5_000);
    if (!encoders.stdout.includes(format === "webm" ? "libvpx-vp9" : "libx264")) throw mediaError("encoderUnavailable", `FFmpeg lacks ${format === "webm" ? "libvpx-vp9" : "libx264"}.`);
    const work = fs.mkdtempSync(path.join(os.tmpdir(), "rts-media-finish-"));
    try {
      const output = path.join(work, `${input.name || (comparison ? input.layout || "grid" : "export")}.${format}`);
      const args = ["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-filter_complex_threads", "1"];
      sources.forEach(source => args.push("-i", source));
      const filters: string[] = [];
      let result = "v0", duration = probes[0].duration / speed;
      if (!comparison) {
        filters.push(`[0:v]setpts=(PTS-STARTPTS)/${speed},fps=30:eof_action=pass,pad=ceil(iw/2)*2:ceil(ih/2)*2[v0]`);
      } else if (difference) {
        filters.push("[0:v]setpts=PTS-STARTPTS,format=gbrp[b]", "[1:v]setpts=PTS-STARTPTS,format=gbrp[a]",
          `[b][a]blend=all_mode=difference:shortest=1,setpts=PTS/${speed},fps=30:eof_action=pass,pad=ceil(iw/2)*2:ceil(ih/2)*2[v0]`);
      } else {
        const width = Math.max(320, Math.min(960, ...probes.map(p => p.width))) & ~1;
        const height = Math.max(240, Math.min(720, ...probes.map(p => p.height))) & ~1;
        const labels = input.labels || artifacts.map((a, i) => input.layout === "before-after" ? ["Before", "After"][i] : `Case ${i+1}`);
        probes.forEach((_, i) => {
          const label = path.join(work, `label-${i}.png`); writeMediaLabel(label, labels[i], width);
          args.push("-loop", "1", "-i", label);
          filters.push(`[${i}:v]setpts=(PTS-STARTPTS)/${speed},fps=30:eof_action=pass,scale=${width}:${height}:force_original_aspect_ratio=decrease,pad=${width}:${height}:(ow-iw)/2:(oh-ih)/2,setsar=1,pad=iw:ih+48:0:48:color=0x101010[n${i}]`,
            `[n${i}][${probes.length+i}:v]overlay=0:0:shortest=1,format=yuv420p[c${i}]`);
        });
        const names = probes.map((_,i) => `[c${i}]`).join("");
        result = "out";
        if (input.layout === "concat") {
          // Each resampled clip occupies whole output frames, including one-frame clips.
          // Rebuild timestamps after concat: it gives single-frame segments zero duration.
          duration = probes.reduce((total, p) => total + Math.max(1, Math.round(p.duration / speed * 30)) / 30, 0);
          filters.push(`${names}concat=n=${probes.length}:v=1:a=0,setpts=N/(30*TB)[out]`);
        } else {
          duration = Math.min(...probes.map(p => p.duration)) / speed;
          const positions = probes.map((_,i) => `${i%2*width}_${Math.floor(i/2)*(height+48)}`).join("|");
          filters.push(`${names}xstack=inputs=${probes.length}:layout=${positions}:shortest=1:fill=black[out]`);
        }
      }
      duration = Math.max(1 / 30, duration);
      if (duration > 600) throw mediaError("mediaTooLong", "Finished videos are limited to ten minutes.");
      args.push("-filter_complex", filters.join(";"), "-map", `[${result}]`, "-an", "-t", String(duration), "-threads", "2");
      if (format === "webm") args.push("-c:v", "libvpx-vp9", "-row-mt", "1", "-cpu-used", "4", "-crf", String(MEDIA_PRESETS[preset]), "-b:v", "0");
      else args.push("-c:v", "libx264", "-preset", "veryfast", "-crf", String({compact:28,balanced:23,quality:18}[preset]), "-tag:v", "avc1", "-movflags", "+faststart");
      args.push("-pix_fmt", "yuv420p", "-fs", String(MAX_MEDIA_BYTES), output);
      await this.run(this.ffmpeg, args, 180_000);
      const probe = await this.probe(output);
      const expectedFrames = Math.max(1, Math.round(duration * 30));
      if (Math.abs(probe.frames - expectedFrames) > 1 || Math.abs(probe.duration - duration) > 0.1 || probe.codec !== (format === "webm" ? "vp9" : "h264")) {
        throw mediaError("mediaVerificationFailed", "Output was truncated or failed codec/duration/frame-count verification.");
      }
      await this.run(this.ffmpeg, ["-v", "error", "-xerror", "-i", output, "-f", "null", "-"], 60_000);
      const processing = { operation: comparison ? "compare" : "export", layout: comparison ? input.layout || "grid" : null,
        artifacts: artifacts.map(a=>a.artifactId), labels: input.labels || null, speed, format, preset,
        durationPolicy: input.layout === "concat" ? "concatenated full clips" : comparison ? "shortest source" : "full clip",
        differenceSource: difference ? "RGB lossless capture frames before any scaling, labeling or lossy encoding" : null };
      const manifest = path.join(work, "manifest.json");
      fs.writeFileSync(manifest, JSON.stringify({ kind: "interactFinishedMedia", processing, probe }), { mode: 0o600 });
      const artifact = this.store.register(output, manifest);
      return { ...this.store.describe(artifact), probe, processing,
        next: `node scripts/interact/cli.mjs media publish '${JSON.stringify({artifactId:artifact.artifactId})}'` };
    } finally { fs.rmSync(work, { recursive: true, force: true }); }
  }
  async probe(file: string): Promise<Probe> {
    const result = await this.run(this.ffprobe, ["-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=width,height,codec_name,r_frame_rate,nb_read_frames:format=duration", "-of", "json", file], 30_000);
    const data = JSON.parse(result.stdout), stream = data.streams?.[0];
    const [n,d] = String(stream?.r_frame_rate).split("/").map(Number);
    const probe = { width: Number(stream?.width), height: Number(stream?.height), fps:n/d,
      frames:Number(stream?.nb_read_frames), duration:Number(data.format?.duration), codec:String(stream?.codec_name) };
    if (![probe.width,probe.height,probe.fps,probe.frames,probe.duration].every(v=>Number.isFinite(v)&&v>0) || probe.width*probe.height>8192*8192 || probe.frames>18000 || probe.duration>600) {
      throw mediaError("invalidMedia", "Source dimensions, duration or frame count exceed media bounds.");
    }
    return probe;
  }
  async run(command: string, args: string[], timeoutMs: number) {
    const result = await this.runner.run(command, args, {timeoutMs});
    if (result.status !== 0 || result.stderr.trim() && args.includes("-xerror")) throw mediaError("mediaProcessingFailed", result.stderr.trim().slice(-1000) || `${command} exited ${result.status}.`);
    return result;
  }
}
export function assertSynchronizedSources(artifacts: MediaArtifact[]) {
  const [a,b] = artifacts;
  if (!a.lossless || !b.lossless) throw mediaError("losslessSourceRequired", "Difference videos require two original fixed-step captures with retained lossless sources; compressed videos are rejected.");
  const signature = (artifact: MediaArtifact) => {
    const p = artifact.provenance as Record<string, any>;
    return { kind:p.kind, mapping:p.mapping, authoritative:p.authoritative, camera:p.camera,
      capture:p.capture && { frameCount:p.capture.frameCount,clip:p.capture.clip,viewport:p.capture.viewport,visualStartMs:p.capture.visualStartMs },
      frames:p.frames?.map((f: Record<string, unknown>)=>({index:f.index,tick:f.tick,visualTimeMs:f.visualTimeMs})),
      runtime:p.runtime, scene:p.scene?.identity && {source:p.scene.identity.source,kind:p.scene.identity.kind,map:p.scene.identity.map,seed:p.scene.identity.seed,scenario:p.scene.identity.scenario,artifactId:p.scene.identity.artifactId} };
  };
  const left = signature(a), right = signature(b);
  if (left.kind !== "interactFixedCapture" || !left.frames?.length || !isDeepStrictEqual(left,right)) {
    throw mediaError("mediaNotSynchronized", "Difference captures must share ticks, render timestamps, camera, viewport/DPR, map/seed/scenario and browser environment. Recreate the same setup and use a shared visualStartMs.");
  }
}
