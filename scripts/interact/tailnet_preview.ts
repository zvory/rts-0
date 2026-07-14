// Durable, artifact-only Tailnet delivery for Interact captures.
//
// Captures are validated against this worktree, then copied into the machine-level
// Tailnet preview service. The copied artifact and stable port outlive the Lab daemon,
// its browser/private server session, and removal of the originating worktree.

import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";

import {
  DEFAULT_TTL_MS,
  publishTailnetPreview,
  type PublishedTailnetPreview,
} from "../tailnet-preview.mjs";

export const MAX_TAILNET_PREVIEW_ARTIFACT_BYTES = 64 * 1024 * 1024;
export const INTERACT_PREVIEW_TTL_MS = DEFAULT_TTL_MS;

type PreviewMimeType = "image/png" | "video/mp4";
const MIME_TYPES = new Set<PreviewMimeType>(["image/png", "video/mp4"]);

interface ArtifactInfo { realPath: string; size: number; fingerprint: string }
interface PreviewEntry extends ArtifactInfo {
  mimeType: PreviewMimeType;
  url: string;
  expiresAt: number | null;
}
interface TailnetPreviewOptions {
  workspaceRoot?: string;
  ttlMs?: number;
  publishArtifact?: (options: { source: string; ttlMs: number }) => Promise<PublishedTailnetPreview>;
}

export class InteractTailnetPreviewError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "InteractTailnetPreviewError";
    this.code = code;
  }
}

export class InteractTailnetPreview {
  fingerprints: Map<string, PreviewEntry>;
  publications: Map<string, Promise<PreviewEntry>>;
  artifactRoot: string;
  workspaceRoot: string;
  ttlMs: number;
  publishArtifact: (options: { source: string; ttlMs: number }) => Promise<PublishedTailnetPreview>;

  constructor({
    workspaceRoot = process.cwd(),
    ttlMs = INTERACT_PREVIEW_TTL_MS,
    publishArtifact = publishDurableArtifact,
  }: TailnetPreviewOptions = {}) {
    this.workspaceRoot = realDirectory(workspaceRoot, "invalidWorkspace", "Interact workspace does not exist.");
    if (!Number.isSafeInteger(ttlMs) || ttlMs < INTERACT_PREVIEW_TTL_MS) {
      throw new InteractTailnetPreviewError(
        "invalidPreviewTtl",
        "Interact preview retention must be at least 24 hours.",
      );
    }
    this.artifactRoot = path.join(this.workspaceRoot, "target", "interact");
    this.ttlMs = ttlMs;
    this.publishArtifact = publishArtifact;
    this.fingerprints = new Map();
    this.publications = new Map();
  }

  async publish({ filePath, mimeType }: { filePath: string; mimeType: string }) {
    if (!MIME_TYPES.has(mimeType as PreviewMimeType)) {
      throw new InteractTailnetPreviewError("invalidPreviewMimeType", "Interact preview accepts only PNG images and MP4 videos.");
    }
    const artifact = inspectArtifact(this.artifactRoot, filePath);
    const existing = this.fingerprints.get(artifact.fingerprint);
    if (existing && (existing.expiresAt === null || existing.expiresAt > Date.now())) {
      return this.describe(existing);
    }
    const pending = this.publications.get(artifact.fingerprint);
    if (pending) return this.describe(await pending);

    const publication = this.publishEntry(artifact, mimeType as PreviewMimeType);
    this.publications.set(artifact.fingerprint, publication);
    try {
      return this.describe(await publication);
    } finally {
      if (this.publications.get(artifact.fingerprint) === publication) {
        this.publications.delete(artifact.fingerprint);
      }
    }
  }

  async close() {
    // The machine-level preview service and its copied artifacts intentionally outlive
    // the per-worktree Interact daemon. Only forget this daemon's deduplication cache.
    this.fingerprints.clear();
  }

  async publishEntry(artifact: ArtifactInfo, mimeType: PreviewMimeType) {
    let published: PublishedTailnetPreview;
    try {
      published = await this.publishArtifact({ source: artifact.realPath, ttlMs: this.ttlMs });
    } catch (error) {
      throw new InteractTailnetPreviewError(
        "tailnetPreviewUnavailable",
        `Interact could not publish a durable Tailnet preview (${errorMessage(error)}).`,
      );
    }
    if (!published || typeof published.url !== "string" || !/^http:\/\//.test(published.url) ||
        (published.expiresAt !== null && !Number.isSafeInteger(published.expiresAt))) {
      throw new InteractTailnetPreviewError("tailnetPreviewUnavailable", "The Tailnet preview service returned an invalid publication result.");
    }
    const entry: PreviewEntry = { mimeType, ...artifact, ...published };
    this.fingerprints.set(artifact.fingerprint, entry);
    return entry;
  }

  describe(entry: PreviewEntry) {
    return {
      url: entry.url,
      mimeType: entry.mimeType,
      bytes: entry.size,
      expiresAt: entry.expiresAt,
      availability: entry.expiresAt === null
        ? "retained until manually removed"
        : "available for at least 24 hours after publication",
    };
  }
}

function inspectArtifact(artifactRoot: string, filePath: fs.PathLike): ArtifactInfo {
  if (typeof filePath !== "string" || !filePath) {
    throw new InteractTailnetPreviewError("unsafePreviewArtifact", "Interact preview requires a confined artifact file.");
  }
  const root = realDirectory(artifactRoot, "unsafePreviewArtifact", "Interact preview artifact root is unavailable.");
  let realPath;
  try {
    realPath = fs.realpathSync(filePath);
  } catch {
    throw new InteractTailnetPreviewError("previewArtifactMissing", "Interact preview artifact is no longer available.");
  }
  if (!isWithin(root, realPath)) {
    throw new InteractTailnetPreviewError("unsafePreviewArtifact", "Interact preview may publish only this worktree's Interact artifacts.");
  }
  let stat;
  try {
    stat = fs.statSync(realPath);
  } catch {
    throw new InteractTailnetPreviewError("previewArtifactMissing", "Interact preview artifact is no longer available.");
  }
  if (!stat.isFile() || stat.size <= 0) {
    throw new InteractTailnetPreviewError("previewArtifactMissing", "Interact preview artifact is not a readable file.");
  }
  if (stat.size > MAX_TAILNET_PREVIEW_ARTIFACT_BYTES) {
    throw new InteractTailnetPreviewError("previewArtifactTooLarge", "Interact preview artifact exceeds the 64 MiB delivery limit.");
  }
  return {
    realPath,
    size: stat.size,
    fingerprint: `${realPath}\u0000${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeMs}`,
  };
}

function isWithin(root: string, target: string) { return target.startsWith(`${root}${path.sep}`); }
function publishDurableArtifact({ source, ttlMs }: { source: string; ttlMs: number }) {
  const testHost = String(process.env.RTS_INTERACT_TEST_TAILNET_PREVIEW_HOST || "").trim();
  if (!testHost) return publishTailnetPreview({ source, ttlMs });
  if (!["127.0.0.1", "::1"].includes(testHost) || net.isIP(testHost) === 0) {
    return Promise.reject(new Error("the test Tailnet preview host must be loopback"));
  }
  const root = String(process.env.RTS_INTERACT_TEST_TAILNET_PREVIEW_ROOT ||
    path.join(os.tmpdir(), "rts-interact-tailnet-preview-test"));
  const port = Number(process.env.RTS_INTERACT_TEST_TAILNET_PREVIEW_PORT || 8091);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    return Promise.reject(new Error("the test Tailnet preview port must be an integer from 1 through 65535"));
  }
  return publishTailnetPreview({ source, ttlMs, host: testHost, root, port });
}
function realDirectory(value: fs.PathLike, code: string, message: string) {
  try {
    const resolved = fs.realpathSync(value);
    if (!fs.statSync(resolved).isDirectory()) throw new Error("not a directory");
    return resolved;
  } catch {
    throw new InteractTailnetPreviewError(code, message);
  }
}
function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }
