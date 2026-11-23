// Machine-local immutable copies survive daemon shutdown and worktree cleanup.
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { MEDIA_ID_RE, mediaError } from "./media_inputs.ts";

export const MAX_MEDIA_BYTES = 64 * 1024 * 1024;
export const MAX_LOSSLESS_BYTES = 512 * 1024 * 1024;
export const MEDIA_STORE_ROOT = path.join(os.tmpdir(), `rts-interact-media-${process.getuid?.() ?? "local"}`);
export interface MediaArtifact {
  schemaVersion: 1; artifactId: string; name: string; createdAt: string;
  format: "mp4" | "webm"; bytes: number; lossless: boolean;
  provenance: Record<string, unknown>;
}
export class MediaStore {
  root: string;
  constructor(root = MEDIA_STORE_ROOT) { this.root = root; }
  register(videoPath: string, manifestPath: string, losslessPath?: string) {
    const stat = checkedFile(videoPath, MAX_MEDIA_BYTES);
    checkedFile(manifestPath, 4 * 1024 * 1024);
    const provenance = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    const artifactId = `media_${crypto.createHash("sha256").update(`${fs.realpathSync(videoPath)}:${stat.ino}:${stat.size}:${stat.mtimeMs}`).digest("hex").slice(0, 32)}`;
    this.prepareRoot();
    if (fs.existsSync(path.join(this.root, artifactId))) return this.read(artifactId);
    const staging = fs.mkdtempSync(path.join(this.root, ".pending-"));
    try {
      const format = path.extname(videoPath).slice(1);
      if (format !== "mp4" && format !== "webm") throw mediaError("invalidMedia", "Only MP4 and WebM videos can be registered.");
      fs.copyFileSync(videoPath, path.join(staging, `video.${format}`), fs.constants.COPYFILE_FICLONE);
      if (losslessPath) {
        checkedFile(losslessPath, MAX_LOSSLESS_BYTES);
        fs.copyFileSync(losslessPath, path.join(staging, "source.mkv"), fs.constants.COPYFILE_FICLONE);
      }
      const artifact: MediaArtifact = { schemaVersion: 1, artifactId, name: path.basename(videoPath, `.${format}`),
        createdAt: new Date().toISOString(), format, bytes: stat.size, lossless: Boolean(losslessPath), provenance };
      fs.writeFileSync(path.join(staging, "artifact.json"), JSON.stringify(artifact), { mode: 0o600 });
      try { fs.renameSync(staging, path.join(this.root, artifactId)); }
      catch (error) { if (!fs.existsSync(path.join(this.root, artifactId))) throw error; }
      return this.read(artifactId);
    } finally { fs.rmSync(staging, { recursive: true, force: true }); }
  }
  read(artifactId: string): MediaArtifact {
    if (!MEDIA_ID_RE.test(artifactId)) throw mediaError("invalidInput", "Invalid media artifact ID.");
    try {
      const dir = path.join(this.root, artifactId);
      this.confined(dir);
      const manifest = path.join(dir, "artifact.json"); this.confined(manifest);
      checkedFile(manifest, 4 * 1024 * 1024);
      const artifact = JSON.parse(fs.readFileSync(manifest, "utf8"));
      if (artifact.schemaVersion !== 1 || artifact.artifactId !== artifactId || !["webm", "mp4"].includes(artifact.format)) throw Error("Invalid artifact record");
      const video = path.join(dir, `video.${artifact.format}`); this.confined(video); checkedFile(video, MAX_MEDIA_BYTES);
      if (artifact.lossless) { const source = path.join(dir, "source.mkv"); this.confined(source); checkedFile(source, MAX_LOSSLESS_BYTES); }
      return artifact;
    } catch (error) {
      throw mediaError("mediaArtifactMissing", `Artifact ${artifactId} is unavailable or invalid: ${error instanceof Error ? error.message : error}. Recapture if OS temporary cleanup removed it.`);
    }
  }
  file(artifact: MediaArtifact, source = false) {
    this.read(artifact.artifactId);
    return path.join(this.root, artifact.artifactId, source && artifact.lossless ? "source.mkv" : `video.${artifact.format}`);
  }
  describe(artifact: MediaArtifact) {
    return { artifactId: artifact.artifactId, name: artifact.name, format: artifact.format, bytes: artifact.bytes,
      losslessSource: artifact.lossless, createdAt: artifact.createdAt,
      retention: "Machine-local copy survives session/worktree cleanup; retained until manually removed or OS temporary cleanup." };
  }
  prepareRoot() {
    fs.mkdirSync(this.root, { recursive: true, mode: 0o700 });
    if (fs.lstatSync(this.root).isSymbolicLink()) throw mediaError("unsafeMediaArtifact", "Media store must not be a symlink.");
    fs.chmodSync(this.root, 0o700);
  }
  confined(file: string) {
    const root = fs.realpathSync(this.root), real = fs.realpathSync(file);
    if (!real.startsWith(`${root}${path.sep}`) || fs.lstatSync(file).isSymbolicLink()) throw Error("Media artifact escaped its store");
  }
}
function checkedFile(file: string, maximum: number) {
  const stat = fs.statSync(file);
  if (!stat.isFile() || stat.size < 1 || stat.size > maximum) throw mediaError("mediaTooLarge", `Media file must be nonempty and at most ${maximum} bytes.`);
  return stat;
}
