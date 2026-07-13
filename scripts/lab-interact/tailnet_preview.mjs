// Ephemeral, artifact-only Tailnet delivery for Lab Interact captures.
//
// The Lab game server intentionally remains loopback-only. This server binds only to this
// machine's Tailnet IP and serves opaque, registered image/video artifacts; it never exposes a
// directory listing, a filesystem path, or any of the private Lab/game routes.

import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import net from "node:net";
import path from "node:path";

import { ProcessRunner } from "./process_runner.mjs";

export const LAB_INTERACT_PREVIEW_ROUTE = "/lab-interact-preview/";
export const MAX_TAILNET_PREVIEW_ARTIFACT_BYTES = 64 * 1024 * 1024;
export const MAX_TAILNET_PREVIEWS = 128;

const MIME_TYPES = new Set(["image/png", "video/mp4"]);
const TOKEN_RE = /^[a-f0-9]{64}$/;

export class LabInteractTailnetPreviewError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "LabInteractTailnetPreviewError";
    this.code = code;
  }
}

export class LabInteractTailnetPreview {
  constructor({
    workspaceRoot = process.cwd(),
    host = null,
    resolveHost = () => resolveTailnetHost(),
    onAccess = () => {},
    maxArtifacts = MAX_TAILNET_PREVIEWS,
  } = {}) {
    this.workspaceRoot = realDirectory(workspaceRoot, "invalidWorkspace", "Lab Interact workspace does not exist.");
    if (!Number.isInteger(maxArtifacts) || maxArtifacts < 1 || maxArtifacts > MAX_TAILNET_PREVIEWS) {
      throw new LabInteractTailnetPreviewError("invalidPreviewLimit", `Lab preview retention must be an integer from 1 to ${MAX_TAILNET_PREVIEWS}.`);
    }
    this.artifactRoot = path.join(this.workspaceRoot, "target", "lab-interact");
    this.host = host;
    this.resolveHost = resolveHost;
    this.onAccess = onAccess;
    this.maxArtifacts = maxArtifacts;
    this.server = null;
    this.baseUrl = null;
    this.starting = null;
    this.artifacts = new Map();
    this.fingerprints = new Map();
  }

  async publish({ filePath, mimeType }) {
    if (!MIME_TYPES.has(mimeType)) {
      throw new LabInteractTailnetPreviewError("invalidPreviewMimeType", "Lab preview accepts only PNG images and MP4 videos.");
    }
    const artifact = inspectArtifact(this.artifactRoot, filePath);
    await this.start();
    const cachedToken = this.fingerprints.get(artifact.fingerprint);
    const existing = cachedToken ? this.artifacts.get(cachedToken) : null;
    if (existing) return this.describe(existing);

    while (this.artifacts.size >= this.maxArtifacts) this.evictOldest();
    const token = crypto.randomBytes(32).toString("hex");
    const entry = { token, mimeType, ...artifact };
    this.artifacts.set(token, entry);
    this.fingerprints.set(artifact.fingerprint, token);
    return this.describe(entry);
  }

  async start() {
    if (this.server) return;
    if (this.starting) return this.starting;
    this.starting = this.startServer();
    try {
      await this.starting;
    } finally {
      this.starting = null;
    }
  }

  async close() {
    await this.starting?.catch(() => {});
    const server = this.server;
    this.server = null;
    this.baseUrl = null;
    this.artifacts.clear();
    this.fingerprints.clear();
    if (!server) return;
    server.closeAllConnections?.();
    await new Promise((resolve) => server.close(() => resolve()));
  }

  describe(entry) {
    return {
      url: `${this.baseUrl}${LAB_INTERACT_PREVIEW_ROUTE}${entry.token}`,
      mimeType: entry.mimeType,
      bytes: entry.size,
      availability: "available while the Lab Interact daemon remains running",
    };
  }

  evictOldest() {
    const oldest = this.artifacts.values().next().value;
    if (!oldest) return;
    this.artifacts.delete(oldest.token);
    this.fingerprints.delete(oldest.fingerprint);
  }

  async startServer() {
    const host = this.host || await this.resolveHost();
    if (!validPreviewHost(host)) {
      throw new LabInteractTailnetPreviewError("invalidTailnetHost", "Tailnet preview requires a valid Tailnet IP address.");
    }
    const server = http.createServer((request, response) => this.handle(request, response));
    server.on("error", () => {});
    server.keepAliveTimeout = 5_000;
    try {
      await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen({ host, port: 0, exclusive: true }, () => {
          server.removeListener("error", reject);
          resolve();
        });
      });
    } catch (error) {
      try { server.close(); } catch {}
      throw new LabInteractTailnetPreviewError(
        "tailnetPreviewBindFailed",
        `Lab Interact could not bind an artifact preview on the Tailnet IP (${String(error?.code || "unknown error")}).`,
      );
    }
    const address = server.address();
    if (!address || typeof address === "string") {
      try { server.close(); } catch {}
      throw new LabInteractTailnetPreviewError("tailnetPreviewBindFailed", "Lab Interact did not receive a usable Tailnet preview listener address.");
    }
    this.server = server;
    this.baseUrl = `http://${urlHost(host)}:${address.port}`;
  }

  handle(request, response) {
    if (!request || !response) return;
    if (!["GET", "HEAD"].includes(request.method || "")) {
      response.writeHead(405, { Allow: "GET, HEAD", "cache-control": "no-store" });
      response.end();
      return;
    }
    const token = requestToken(request.url);
    const entry = token ? this.artifacts.get(token) : null;
    if (!entry) {
      response.writeHead(404, { "cache-control": "no-store" });
      response.end();
      return;
    }
    let artifact;
    try {
      artifact = inspectArtifact(this.artifactRoot, entry.realPath);
    } catch {
      this.artifacts.delete(entry.token);
      this.fingerprints.delete(entry.fingerprint);
      response.writeHead(404, { "cache-control": "no-store" });
      response.end();
      return;
    }
    if (artifact.fingerprint !== entry.fingerprint) {
      this.artifacts.delete(entry.token);
      this.fingerprints.delete(entry.fingerprint);
      response.writeHead(404, { "cache-control": "no-store" });
      response.end();
      return;
    }
    try { this.onAccess(); } catch {}
    const range = parseRange(request.headers.range, entry.size);
    if (range.invalid) {
      response.writeHead(416, {
        "content-range": `bytes */${entry.size}`,
        "cache-control": "no-store",
      });
      response.end();
      return;
    }
    const length = range.end - range.start + 1;
    const headers = {
      "accept-ranges": "bytes",
      "cache-control": "private, no-store, max-age=0",
      "content-length": String(length),
      "content-security-policy": "default-src 'none'; img-src 'self'; media-src 'self'",
      "content-type": entry.mimeType,
      "x-content-type-options": "nosniff",
      "content-disposition": `inline; filename=\"${previewFilename(entry.mimeType)}\"`,
    };
    if (range.partial) headers["content-range"] = `bytes ${range.start}-${range.end}/${entry.size}`;
    response.writeHead(range.partial ? 206 : 200, headers);
    if (request.method === "HEAD") {
      response.end();
      return;
    }
    const stream = fs.createReadStream(entry.realPath, { start: range.start, end: range.end });
    // `pipe()` only unpipes when a client disconnects. Destroy the source too so an
    // abandoned range request cannot leave its file descriptor paused until process teardown.
    response.once("close", () => stream.destroy());
    stream.once("error", () => {
      if (!response.headersSent) response.writeHead(404, { "cache-control": "no-store" });
      response.end();
    });
    stream.pipe(response);
  }
}

export async function resolveTailnetHost({ env = process.env, readStatus = readTailnetStatus } = {}) {
  const testHost = String(env.RTS_LAB_INTERACT_TEST_TAILNET_PREVIEW_HOST || "").trim();
  if (testHost) {
    if (!["127.0.0.1", "::1"].includes(testHost)) {
      throw new LabInteractTailnetPreviewError("invalidTailnetHost", "The test Tailnet preview host must be loopback.");
    }
    return testHost;
  }
  const configuredHost = String(env.RTS_LAB_INTERACT_TAILNET_HOST || "").trim();
  if (configuredHost) {
    if (!isTailnetIpv4(configuredHost)) {
      throw new LabInteractTailnetPreviewError("invalidTailnetHost", "RTS_LAB_INTERACT_TAILNET_HOST must be a Tailscale IPv4 address.");
    }
    return configuredHost;
  }
  let status;
  try {
    status = await readStatus();
  } catch {
    throw new LabInteractTailnetPreviewError("tailnetUnavailable", "Tailnet preview requires Tailscale to be installed and running.");
  }
  const host = tailnetHostFromStatus(status);
  if (!host) {
    throw new LabInteractTailnetPreviewError("tailnetUnavailable", "Tailnet preview requires this machine to have a Tailscale IPv4 address.");
  }
  return host;
}

export function tailnetHostFromStatus(status) {
  const values = [
    ...(Array.isArray(status?.TailscaleIPs) ? status.TailscaleIPs : []),
    ...(Array.isArray(status?.Self?.TailscaleIPs) ? status.Self.TailscaleIPs : []),
  ];
  return values.find((value) => typeof value === "string" && isTailnetIpv4(value)) || null;
}

export function isTailnetIpv4(value) {
  if (net.isIP(value) !== 4) return false;
  const [, second] = value.split(".").map(Number);
  return value.startsWith("100.") && second >= 64 && second <= 127;
}

async function readTailnetStatus() {
  const result = await new ProcessRunner({ maxOutputBytes: 1024 * 1024 })
    .run("tailscale", ["status", "--json"], { timeoutMs: 2_000 });
  if (result.status !== 0) throw new Error("tailscale status failed");
  return JSON.parse(String(result.stdout || ""));
}

function inspectArtifact(artifactRoot, filePath) {
  if (typeof filePath !== "string" || !filePath) {
    throw new LabInteractTailnetPreviewError("unsafePreviewArtifact", "Lab preview requires a confined artifact file.");
  }
  const root = realDirectory(artifactRoot, "unsafePreviewArtifact", "Lab preview artifact root is unavailable.");
  let realPath;
  try {
    realPath = fs.realpathSync(filePath);
  } catch {
    throw new LabInteractTailnetPreviewError("previewArtifactMissing", "Lab preview artifact is no longer available.");
  }
  if (!isWithin(root, realPath)) {
    throw new LabInteractTailnetPreviewError("unsafePreviewArtifact", "Lab preview may serve only this worktree's Lab artifacts.");
  }
  let stat;
  try {
    stat = fs.statSync(realPath);
  } catch {
    throw new LabInteractTailnetPreviewError("previewArtifactMissing", "Lab preview artifact is no longer available.");
  }
  if (!stat.isFile() || stat.size <= 0) {
    throw new LabInteractTailnetPreviewError("previewArtifactMissing", "Lab preview artifact is not a readable file.");
  }
  if (stat.size > MAX_TAILNET_PREVIEW_ARTIFACT_BYTES) {
    throw new LabInteractTailnetPreviewError("previewArtifactTooLarge", "Lab preview artifact exceeds the 64 MiB delivery limit.");
  }
  return {
    realPath,
    size: stat.size,
    fingerprint: `${realPath}\u0000${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeMs}`,
  };
}

function requestToken(rawUrl) {
  let url;
  try {
    url = new URL(rawUrl || "", "http://lab-interact-preview.invalid");
  } catch {
    return null;
  }
  const prefix = LAB_INTERACT_PREVIEW_ROUTE;
  if (!url.pathname.startsWith(prefix)) return null;
  const token = url.pathname.slice(prefix.length);
  return TOKEN_RE.test(token) ? token : null;
}

function parseRange(value, size) {
  if (!value) return { start: 0, end: size - 1, partial: false, invalid: false };
  const match = /^bytes=(\d*)-(\d*)$/.exec(String(value).trim());
  if (!match) return { invalid: true };
  const [, startText, endText] = match;
  if (!startText && !endText) return { invalid: true };
  let start;
  let end;
  if (!startText) {
    const suffix = Number(endText);
    if (!Number.isSafeInteger(suffix) || suffix <= 0) return { invalid: true };
    start = Math.max(0, size - suffix);
    end = size - 1;
  } else {
    start = Number(startText);
    end = endText ? Number(endText) : size - 1;
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start) return { invalid: true };
    if (start >= size) return { invalid: true };
    end = Math.min(end, size - 1);
  }
  return { start, end, partial: true, invalid: false };
}

function validPreviewHost(host) {
  return isTailnetIpv4(host) || host === "127.0.0.1" || host === "::1";
}

function urlHost(host) { return host.includes(":") ? `[${host}]` : host; }
function previewFilename(mimeType) { return mimeType === "video/mp4" ? "lab-interact-preview.mp4" : "lab-interact-preview.png"; }
function isWithin(root, target) { return target.startsWith(`${root}${path.sep}`); }
function realDirectory(value, code, message) {
  try {
    const resolved = fs.realpathSync(value);
    if (!fs.statSync(resolved).isDirectory()) throw new Error("not a directory");
    return resolved;
  } catch {
    throw new LabInteractTailnetPreviewError(code, message);
  }
}
