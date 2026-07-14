#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { chmodSync, copyFileSync, createReadStream, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import http from "node:http";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_PORT = 8091;
export const DEFAULT_TTL_MS = 24 * 60 * 60 * 1000;
const HEALTH_PATH = "/_tailnet-preview/health";
const PREVIEW_PATH_PREFIX = "/p/";
const SERVICE_NAME = "rts-tailnet-preview";
const ARTIFACT_FILE_NAME = "artifact";
const MANIFEST_FILE_NAME = "manifest.json";
const CLEANUP_INTERVAL_MS = 5 * 60 * 1000;
const SERVER_STARTUP_TIMEOUT_MS = 5_000;
const SERVER_STOP_TIMEOUT_MS = 2_000;
const MIME_TYPES = new Map([
  [".avif", "image/avif"],
  [".gif", "image/gif"],
  [".jpeg", "image/jpeg"],
  [".jpg", "image/jpeg"],
  [".json", "application/json; charset=utf-8"],
  [".m4a", "audio/mp4"],
  [".m4v", "video/x-m4v"],
  [".mov", "video/quicktime"],
  [".mp3", "audio/mpeg"],
  [".mp4", "video/mp4"],
  [".ogg", "audio/ogg"],
  [".pdf", "application/pdf"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".wav", "audio/wav"],
  [".webm", "video/webm"],
  [".webp", "image/webp"],
]);

export function parseDuration(raw) {
  const match = /^(\d+)(s|m|h|d)$/.exec(String(raw || "").trim());
  if (!match) {
    throw new Error(`invalid TTL "${raw}"; use a positive duration such as 30m, 2h, or 1d`);
  }
  const count = Number(match[1]);
  const multiplier = { s: 1_000, m: 60_000, h: 3_600_000, d: 86_400_000 }[match[2]];
  const duration = count * multiplier;
  if (!Number.isSafeInteger(duration) || duration <= 0) {
    throw new Error(`invalid TTL "${raw}"; duration is out of range`);
  }
  return duration;
}

function parsePort(raw) {
  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error(`invalid port "${raw}"; expected an integer from 1 through 65535`);
  }
  return port;
}

export function parseArgs(argv) {
  const options = {
    keep: false,
    port: DEFAULT_PORT,
    root: path.join(os.tmpdir(), SERVICE_NAME),
    serve: false,
    stop: false,
    ttlMs: DEFAULT_TTL_MS,
    source: "",
    host: "",
  };
  const specified = new Set();

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else if (arg === "--keep") {
      options.keep = true;
    } else if (arg === "--port") {
      options.port = parsePort(argv[++index]);
    } else if (arg === "--root") {
      const root = argv[++index];
      if (!root) throw new Error("--root requires a directory");
      options.root = path.resolve(root);
    } else if (arg === "--ttl") {
      specified.add(arg);
      options.ttlMs = parseDuration(argv[++index]);
    } else if (arg === "--serve") {
      options.serve = true;
    } else if (arg === "--stop") {
      options.stop = true;
    } else if (arg === "--host") {
      specified.add(arg);
      options.host = String(argv[++index] || "").trim();
      if (!options.host) throw new Error("--host requires an address");
    } else if (arg.startsWith("-")) {
      throw new Error(`unknown argument "${arg}"`);
    } else if (!options.source) {
      options.source = path.resolve(arg);
    } else {
      throw new Error("tailnet-preview accepts exactly one file");
    }
  }

  if (options.help) return options;
  if (options.keep && specified.has("--ttl")) {
    throw new Error("--keep cannot be combined with --ttl");
  }
  if (options.serve) {
    if (!options.host) throw new Error("--serve requires --host");
    if (options.source || options.stop || options.keep || specified.has("--ttl")) {
      throw new Error("--serve only accepts --root, --host, and --port");
    }
    return options;
  }
  if (specified.has("--host")) throw new Error("--host is only valid with --serve");
  if (options.stop) {
    if (options.source || options.keep || specified.has("--ttl")) {
      throw new Error("--stop only accepts --port and --root");
    }
    return options;
  }
  if (!options.source) throw new Error("missing file path");
  return options;
}

export function usage() {
  return `Usage:
  scripts/tailnet-preview [--ttl 24h | --keep] [--port 8091] <file>
  scripts/tailnet-preview --stop [--port 8091]

Copies one regular file into a private temporary preview directory, serves it over
the current machine's Tailscale IPv4 address, and prints its URL. The default TTL
is 24 hours; --keep retains the file until it is removed manually or the OS clears
its temporary directory.`;
}

function isRegularFile(file) {
  try {
    return statSync(file).isFile();
  } catch {
    return false;
  }
}

function isPreviewHost(value) {
  return isTailnetIpv4(value) || value === "127.0.0.1" || value === "::1";
}

export function safeFileName(file) {
  const name = path.basename(file).replace(/[^A-Za-z0-9._-]+/g, "_").replace(/^\.+$/, "");
  return name || "artifact";
}

function previewId() {
  return randomBytes(18).toString("base64url");
}

function ensureRoot(root) {
  mkdirSync(root, { recursive: true, mode: 0o700 });
  const stat = lstatSync(root);
  if (!stat.isDirectory()) {
    throw new Error(`preview root must be a directory and not a symlink: ${root}`);
  }
  chmodSync(root, 0o700);
}

function previewDirectory(root, id) {
  return path.join(root, id);
}

function manifestPath(root, id) {
  return path.join(previewDirectory(root, id), MANIFEST_FILE_NAME);
}

function artifactPath(root, id) {
  return path.join(previewDirectory(root, id), ARTIFACT_FILE_NAME);
}

function serverStatePath(root, port) {
  return path.join(root, `server-${port}.json`);
}

function validPreviewId(value) {
  return /^[A-Za-z0-9_-]{16,64}$/.test(value);
}

function validPreviewName(value) {
  return /^[A-Za-z0-9._-]{1,255}$/.test(value) && value !== "." && value !== "..";
}

function readManifest(root, id) {
  if (!validPreviewId(id)) return null;
  try {
    if (!lstatSync(previewDirectory(root, id)).isDirectory()) return null;
  } catch {
    return null;
  }
  const file = manifestPath(root, id);
  try {
    if (!lstatSync(file).isFile()) return null;
    const manifest = JSON.parse(readFileSync(file, "utf8"));
    if (
      manifest?.version !== 1 ||
      !validPreviewName(manifest.name) ||
      (manifest.expiresAt !== null && !Number.isSafeInteger(manifest.expiresAt))
    ) {
      return null;
    }
    return manifest;
  } catch {
    return null;
  }
}

export function stagePreview({ root, source, ttlMs = DEFAULT_TTL_MS, keep = false, now = Date.now() }) {
  if (!isRegularFile(source)) {
    throw new Error(`preview source must be a regular file: ${source}`);
  }
  if (!keep && (!Number.isSafeInteger(ttlMs) || ttlMs <= 0)) {
    throw new Error("preview TTL must be a positive millisecond duration");
  }
  if (!Number.isSafeInteger(now) || now < 0) {
    throw new Error("preview clock must be a non-negative safe millisecond timestamp");
  }
  const expiresAt = keep ? null : now + ttlMs;
  if (expiresAt !== null && !Number.isSafeInteger(expiresAt)) {
    throw new Error("preview expiration is out of range");
  }

  ensureRoot(root);
  const id = previewId();
  const name = safeFileName(source);
  const staging = path.join(root, `.${id}.staging`);
  const destination = path.join(staging, ARTIFACT_FILE_NAME);
  const manifest = { version: 1, name, createdAt: now, expiresAt };

  try {
    mkdirSync(staging, { mode: 0o700 });
    copyFileSync(source, destination);
    writeFileSync(path.join(staging, MANIFEST_FILE_NAME), `${JSON.stringify(manifest)}\n`, { mode: 0o600 });
    renameSync(staging, path.join(root, id));
  } catch (error) {
    rmSync(staging, { recursive: true, force: true });
    throw error;
  }

  return { id, name, expiresAt, path: artifactPath(root, id) };
}

export function cleanupExpiredPreviews(root, now = Date.now()) {
  try {
    ensureRoot(root);
  } catch {
    return 0;
  }

  let entries;
  try {
    entries = readdirSync(root, { withFileTypes: true });
  } catch {
    return 0;
  }

  let removed = 0;
  for (const entry of entries) {
    if (!entry.isDirectory() || !validPreviewId(entry.name)) continue;
    const manifest = readManifest(root, entry.name);
    if (!manifest || (manifest.expiresAt !== null && manifest.expiresAt <= now)) {
      try {
        rmSync(previewDirectory(root, entry.name), { recursive: true, force: true });
        removed += 1;
      } catch {
        // A concurrent request or manual cleanup may have already removed the preview.
      }
    }
  }
  return removed;
}

export function parseByteRange(value, size) {
  if (!Number.isSafeInteger(size) || size < 0) return null;
  if (value == null || value === "") return { start: 0, end: size - 1, partial: false };
  if (size < 1) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(String(value).trim());
  if (!match || (!match[1] && !match[2])) return null;

  if (!match[1]) {
    const suffixLength = Number(match[2]);
    if (!Number.isSafeInteger(suffixLength) || suffixLength < 1) return null;
    return { start: Math.max(0, size - suffixLength), end: size - 1, partial: true };
  }

  const start = Number(match[1]);
  const requestedEnd = match[2] ? Number(match[2]) : size - 1;
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(requestedEnd) ||
    start > requestedEnd ||
    start >= size
  ) {
    return null;
  }
  return { start, end: Math.min(requestedEnd, size - 1), partial: true };
}

export function mimeType(file) {
  return MIME_TYPES.get(path.extname(file).toLowerCase()) || "application/octet-stream";
}

function sendNotFound(response) {
  response.writeHead(404, { "Cache-Control": "no-store", "Content-Type": "text/plain; charset=utf-8" });
  response.end("Not found\n");
}

function sendHealth(response, root) {
  const body = JSON.stringify({ service: SERVICE_NAME, rootTag: rootTag(root) });
  response.writeHead(200, {
    "Cache-Control": "no-store",
    "Content-Length": String(Buffer.byteLength(body)),
    "Content-Type": "application/json; charset=utf-8",
  });
  response.end(body);
}

function rootTag(root) {
  return createHash("sha256").update(path.resolve(root)).digest("base64url").slice(0, 16);
}

function readServerState(root, port) {
  try {
    const state = JSON.parse(readFileSync(serverStatePath(root, port), "utf8"));
    if (
      !Number.isInteger(state?.pid) ||
      state.pid < 1 ||
      state.port !== port ||
      state.rootTag !== rootTag(root) ||
      !isTailnetIpv4(state.host) ||
      typeof state.script !== "string" ||
      !path.isAbsolute(state.script)
    ) {
      return null;
    }
    return state;
  } catch {
    return null;
  }
}

export function writeServerState({ root, host, port }) {
  const state = { host, pid: process.pid, port, rootTag: rootTag(root), script: serverScriptPath() };
  const destination = serverStatePath(root, port);
  const staging = `${destination}.${process.pid}.${randomBytes(6).toString("hex")}.staging`;
  try {
    writeFileSync(staging, `${JSON.stringify(state)}\n`, { mode: 0o600 });
    renameSync(staging, destination);
  } catch (error) {
    rmSync(staging, { force: true });
    throw error;
  }
}

export function clearServerState({ root, port, ownerPid }) {
  if (ownerPid !== undefined) {
    const state = readServerState(root, port);
    if (!state || state.pid !== ownerPid) return false;
  }
  rmSync(serverStatePath(root, port), { force: true });
  return true;
}

function previewRequest(pathname) {
  if (!pathname.startsWith(PREVIEW_PATH_PREFIX)) return null;
  const segments = pathname.slice(PREVIEW_PATH_PREFIX.length).split("/");
  if (segments.length !== 2 || !validPreviewId(segments[0]) || !validPreviewName(segments[1])) return null;
  return { id: segments[0], name: segments[1] };
}

function servePreview(root, request, response, preview, now) {
  const manifest = readManifest(root, preview.id);
  if (!manifest || manifest.name !== preview.name) {
    sendNotFound(response);
    return;
  }
  if (manifest.expiresAt !== null && manifest.expiresAt <= now()) {
    try {
      rmSync(previewDirectory(root, preview.id), { recursive: true, force: true });
    } catch {
      // A concurrent cleanup may have removed the directory already.
    }
    sendNotFound(response);
    return;
  }

  const file = artifactPath(root, preview.id);
  let size;
  try {
    const stat = lstatSync(file);
    if (!stat.isFile()) {
      sendNotFound(response);
      return;
    }
    size = stat.size;
  } catch {
    sendNotFound(response);
    return;
  }

  const range = parseByteRange(request.headers.range, size);
  if (!range) {
    response.writeHead(416, { "Content-Range": `bytes */${size}` });
    response.end();
    return;
  }

  const contentLength = size === 0 ? 0 : range.end - range.start + 1;
  const headers = {
    "Accept-Ranges": "bytes",
    "Cache-Control": "private, no-store",
    "Content-Disposition": `inline; filename="${preview.name}"`,
    "Content-Length": String(contentLength),
    "Content-Type": mimeType(preview.name),
    "Content-Security-Policy": "default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
  };
  if (range.partial) headers["Content-Range"] = `bytes ${range.start}-${range.end}/${size}`;
  response.writeHead(range.partial ? 206 : 200, headers);
  if (request.method === "HEAD" || size === 0) {
    response.end();
    return;
  }
  const stream = createReadStream(file, { start: range.start, end: range.end });
  response.once("close", () => stream.destroy());
  stream.once("error", () => response.destroy());
  stream.pipe(response);
}

export function createPreviewServer({ root, now = () => Date.now() }) {
  ensureRoot(root);
  const server = http.createServer((request, response) => {
    let pathname;
    try {
      pathname = new URL(request.url || "/", "http://localhost").pathname;
    } catch {
      sendNotFound(response);
      return;
    }
    if (request.method === "GET" && pathname === HEALTH_PATH) {
      sendHealth(response, root);
      return;
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405, { Allow: "GET, HEAD" });
      response.end();
      return;
    }
    const preview = previewRequest(pathname);
    if (!preview) {
      sendNotFound(response);
      return;
    }
    servePreview(root, request, response, preview, now);
  });
  const cleanupTimer = setInterval(() => cleanupExpiredPreviews(root, now()), CLEANUP_INTERVAL_MS);
  cleanupTimer.unref();
  server.once("close", () => clearInterval(cleanupTimer));
  return server;
}

export function isTailnetIpv4(value) {
  if (typeof value !== "string") return false;
  if (net.isIP(value) !== 4) return false;
  const [first, second] = value.split(".").map(Number);
  return first === 100 && second >= 64 && second <= 127;
}

export function tailnetIpv4FromStatus(status) {
  const addresses = [
    ...(Array.isArray(status?.TailscaleIPs) ? status.TailscaleIPs : []),
    ...(Array.isArray(status?.Self?.TailscaleIPs) ? status.Self.TailscaleIPs : []),
  ];
  return addresses.find(isTailnetIpv4) || null;
}

function tailscaleIpv4() {
  let raw;
  try {
    raw = execFileSync("tailscale", ["status", "--json"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 2_000,
      maxBuffer: 1_024 * 1_024,
    });
  } catch (error) {
    throw new Error(`could not read Tailscale status: ${error.message}`);
  }
  let status;
  try {
    status = JSON.parse(raw);
  } catch {
    throw new Error("tailscale status did not return valid JSON");
  }
  if (status.BackendState !== "Running") {
    throw new Error(`Tailscale is not running (state: ${status.BackendState || "unknown"})`);
  }
  const ipv4 = tailnetIpv4FromStatus(status);
  if (!ipv4) throw new Error("Tailscale did not report a Tailnet IPv4 address");
  return ipv4;
}

function serverScriptPath() {
  return fileURLToPath(import.meta.url);
}

function serviceHealthy(host, port, root) {
  return new Promise((resolve) => {
    let settled = false;
    let timeout;
    const finish = (healthy) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      resolve(healthy);
    };
    const request = http.get({ host, port, path: HEALTH_PATH, timeout: 400 }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => {
        try {
          const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          finish(response.statusCode === 200 && body.service === SERVICE_NAME && body.rootTag === rootTag(root));
        } catch {
          finish(false);
        }
      });
      response.on("error", () => finish(false));
    });
    timeout = setTimeout(() => {
      request.destroy();
      finish(false);
    }, 500);
    timeout.unref();
    request.on("timeout", () => request.destroy());
    request.on("error", () => finish(false));
  });
}

function ownedServerProcess({ root, state }) {
  try {
    const command = execFileSync("ps", ["-ww", "-p", String(state.pid), "-o", "command="], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return (
      command.includes(state.script) &&
      command.includes("--serve") &&
      command.includes(root) &&
      command.includes(`--host ${state.host}`) &&
      command.includes(`--port ${state.port}`)
    );
  } catch {
    return false;
  }
}

async function waitForProcessExit(pid) {
  const deadline = Date.now() + SERVER_STOP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if (error?.code === "ESRCH") return;
      throw new Error(`could not inspect preview server process ${pid}: ${error.message}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`preview server process ${pid} did not stop after ${SERVER_STOP_TIMEOUT_MS / 1_000}s`);
}

async function stopServer({ root, port }) {
  const state = readServerState(root, port);
  if (state && ownedServerProcess({ root, state })) {
    try {
      process.kill(state.pid, "SIGTERM");
      await waitForProcessExit(state.pid);
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  clearServerState({ root, port });
}

function launchServer({ host, port, root }) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [
        serverScriptPath(),
        "--serve",
        "--root",
        root,
        "--host",
        host,
        "--port",
        String(port),
      ],
      { detached: true, stdio: "ignore" },
    );
    child.once("error", (error) => reject(new Error(`could not start persistent preview server: ${error.message}`)));
    child.once("spawn", () => {
      child.unref();
      resolve();
    });
  });
}

async function ensureServer({ host, port, root }) {
  if (await serviceHealthy(host, port, root)) return;
  await stopServer({ root, port });
  await launchServer({ host, port, root });

  const deadline = Date.now() + SERVER_STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (await serviceHealthy(host, port, root)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`preview server did not become healthy on ${host}:${port}; the port may already be in use`);
}

function serve({ host, port, root }) {
  if (!isPreviewHost(host)) {
    throw new Error("preview server must bind to a Tailscale IPv4 address (or loopback for tests)");
  }
  const server = createPreviewServer({ root });
  let stopping = false;
  const stop = () => {
    if (stopping) return;
    stopping = true;
    clearServerState({ root, port, ownerPid: process.pid });
    if (!server.listening) {
      process.exit(0);
      return;
    }
    server.close(() => process.exit(0));
    server.closeAllConnections?.();
  };
  process.once("SIGINT", stop);
  process.once("SIGHUP", stop);
  process.once("SIGTERM", stop);
  server.once("error", (error) => {
    clearServerState({ root, port, ownerPid: process.pid });
    console.error(`${SERVICE_NAME} failed: ${error.message}`);
    process.exitCode = 1;
  });
  server.listen(port, host, () => {
    writeServerState({ root, host, port });
    console.log(`${SERVICE_NAME} listening on http://${host}:${port}`);
  });
}

export async function publishTailnetPreview({
  source,
  root = path.join(os.tmpdir(), SERVICE_NAME),
  host = tailscaleIpv4(),
  port = DEFAULT_PORT,
  ttlMs = DEFAULT_TTL_MS,
  keep = false,
} = {}) {
  if (!isRegularFile(source)) {
    throw new Error(`preview source must be a regular file: ${source}`);
  }
  if (!isPreviewHost(host)) {
    throw new Error("preview host must be a Tailscale IPv4 address (or loopback for tests)");
  }
  ensureRoot(root);
  cleanupExpiredPreviews(root);
  const preview = stagePreview({ root, source, ttlMs, keep });
  try {
    await ensureServer({ host, port, root });
  } catch (error) {
    try {
      rmSync(previewDirectory(root, preview.id), { recursive: true, force: true });
    } catch {
      // Preserve the startup failure; a later invocation will clean the TTL-bound preview.
    }
    throw error;
  }
  return {
    url: `http://${host}:${port}${PREVIEW_PATH_PREFIX}${preview.id}/${preview.name}`,
    expiresAt: preview.expiresAt,
  };
}

async function runPreview(options) {
  const preview = await publishTailnetPreview(options);
  console.log(`Preview URL: ${preview.url}`);
  if (preview.expiresAt === null) {
    console.log("Expires: retained until manually removed or the OS clears its temporary directory");
  } else {
    console.log(`Expires: ${new Date(preview.expiresAt).toISOString()}`);
  }
}

async function main(argv) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(`tailnet-preview: ${error.message}`);
    console.error(usage());
    process.exitCode = 2;
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }
  if (options.serve) {
    serve(options);
    return;
  }
  if (options.stop) {
    await stopServer({ root: options.root, port: options.port });
    console.log(`Stopped ${SERVICE_NAME} on port ${options.port}.`);
    return;
  }
  await runPreview(options);
}

const isDirectExecution = process.argv[1] && path.resolve(process.argv[1]) === serverScriptPath();
if (isDirectExecution) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`tailnet-preview: ${error.message}`);
    process.exitCode = 1;
  });
}
