import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { spawn } from "node:child_process";
import type { ChildProcess, SpawnOptions } from "node:child_process";
import { once } from "node:events";

import { ProcessRunner, ProcessRunnerError } from "./process_runner.ts";

const HEALTH_POLL_MS = 150;
const SERVER_TERM_GRACE_MS = 1_000;

export class PrivateServerError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "PrivateServerError";
    this.code = code;
  }
}

interface WorkspaceInfo { root: string; head: string }
interface PrivateServerBuild { reused: boolean; binary: string | null; head: string; modifiedAt?: string }
type SpawnServer = (command: string, args: readonly string[], options: SpawnOptions) => ChildProcess;
interface PrivateServerOptions {
  workspace: WorkspaceInfo;
  sessionDir: string;
  startupTimeoutMs: number;
  baseUrl?: string;
  artifactCapability: string;
  signal?: AbortSignal;
  processRunner?: ProcessRunner;
  spawnServer?: SpawnServer;
  fetchHealth?: (baseUrl: string, signal?: AbortSignal) => Promise<boolean>;
  allocatePrivatePort?: (signal?: AbortSignal) => Promise<number>;
  serverTermGraceMs?: number;
}

export class PrivateServer {
  closePromise: Promise<void> | null;
  logFd: number | null;
  serverSpawnError: Error | null;
  child: ChildProcess | null;
  build: PrivateServerBuild | null;
  logPath: string;
  reused: boolean;
  baseUrl: string;
  serverTermGraceMs: number;
  allocatePrivatePort: (signal?: AbortSignal) => Promise<number>;
  fetchHealth: (baseUrl: string, signal?: AbortSignal) => Promise<boolean>;
  spawnServer: SpawnServer;
  processRunner: ProcessRunner;
  signal?: AbortSignal;
  artifactCapability: string;
  requestedBaseUrl: string;
  startupTimeoutMs: number;
  sessionDir: string;
  workspace: WorkspaceInfo;
  static async open(options: PrivateServerOptions) {
    const server = new PrivateServer(options);
    try {
      await server.open();
      return server;
    } catch (error) {
      await server.close().catch(() => {});
      throw normalizePrivateServerError(error);
    }
  }

  constructor({
    workspace,
    sessionDir,
    startupTimeoutMs,
    baseUrl = "",
    artifactCapability,
    signal,
    processRunner = new ProcessRunner(),
    spawnServer = spawn,
    fetchHealth = isHealthy,
    allocatePrivatePort = allocatePort,
    serverTermGraceMs = SERVER_TERM_GRACE_MS,
  }: PrivateServerOptions) {
    this.workspace = workspace;
    this.sessionDir = sessionDir;
    this.startupTimeoutMs = startupTimeoutMs;
    this.requestedBaseUrl = baseUrl;
    this.artifactCapability = artifactCapability;
    this.signal = signal;
    this.processRunner = processRunner;
    this.spawnServer = spawnServer;
    this.fetchHealth = fetchHealth;
    this.allocatePrivatePort = allocatePrivatePort;
    this.serverTermGraceMs = serverTermGraceMs;
    this.baseUrl = "";
    this.reused = false;
    this.logPath = "";
    this.build = null;
    this.child = null;
    this.serverSpawnError = null;
    this.logFd = null;
    this.closePromise = null;
  }

  async open() {
    throwIfAborted(this.signal);
    if (this.requestedBaseUrl) {
      const normalized = privateLoopbackUrl(this.requestedBaseUrl);
      if (!await this.fetchHealth(normalized, this.signal)) {
        throw new PrivateServerError("unhealthyServer", `Requested private server is not healthy: ${normalized}`);
      }
      throwIfAborted(this.signal);
      this.baseUrl = normalized;
      this.reused = true;
      this.build = { reused: true, binary: null, head: this.workspace.head };
      return;
    }

    const port = await this.allocatePrivatePort(this.signal);
    const targetDir = path.join(this.workspace.root, "target", "lab-interact", "cargo");
    const binary = path.join(targetDir, "debug", "rts-server");
    try {
      await this.processRunner.runOrThrow(
        "cargo",
        ["build", "--manifest-path", path.join(this.workspace.root, "server", "Cargo.toml")],
        {
          cwd: this.workspace.root,
          env: { ...process.env, CARGO_TARGET_DIR: targetDir },
          timeoutMs: this.startupTimeoutMs,
          signal: this.signal,
        },
      );
    } catch (error) {
      if (error instanceof ProcessRunnerError && error.code === "processAborted") throw abortedError();
      throw new PrivateServerError("serverBuild", conciseProcessFailure("Lab Interact server build failed", error));
    }
    throwIfAborted(this.signal);
    if (!fs.existsSync(binary)) throw new PrivateServerError("serverBuild", "Lab Interact server binary was not produced.");

    this.logPath = path.join(this.sessionDir, "server.log");
    this.logFd = fs.openSync(this.logPath, "w");
    this.child = this.spawnServer(binary, [], {
      cwd: path.join(this.workspace.root, "server"),
      env: {
        ...process.env,
        RTS_ADDR: `127.0.0.1:${port}`,
        RTS_MATCH_SEED: process.env.RTS_MATCH_SEED || "1",
        RTS_LAB_INTERACT_ARTIFACT_CAPABILITY: this.artifactCapability,
      },
      shell: false,
      stdio: ["ignore", this.logFd, this.logFd],
    });
    const child = this.child;
    child.once("error", (error) => { this.serverSpawnError = error; });
    child.once("exit", () => this.closeLog());
    this.baseUrl = `http://127.0.0.1:${port}/`;
    const deadline = Date.now() + this.startupTimeoutMs;
    while (Date.now() < deadline) {
      throwIfAborted(this.signal);
      if (this.serverSpawnError) {
        throw new PrivateServerError("serverSpawnFailed", `Private server could not start: ${String(this.serverSpawnError.message || this.serverSpawnError).slice(-800)}`);
      }
      if (child.exitCode != null || child.signalCode != null) {
        throw new PrivateServerError("serverExited", `Private server exited during startup; see ${this.logPath}`);
      }
      if (await this.fetchHealth(this.baseUrl, this.signal)) {
        throwIfAborted(this.signal);
        this.build = {
          reused: false,
          binary,
          head: this.workspace.head,
          modifiedAt: fs.statSync(binary).mtime.toISOString(),
        };
        return;
      }
      await abortableDelay(HEALTH_POLL_MS, this.signal);
    }
    throw new PrivateServerError("serverTimeout", `Private server did not become healthy; see ${this.logPath}`);
  }

  async close() {
    if (this.closePromise) return this.closePromise;
    this.closePromise = (async () => {
      const child = this.child;
      this.child = null;
      if (child && child.exitCode == null && child.signalCode == null) {
        const closed = once(child, "close").catch(() => []);
        child.kill("SIGTERM");
        const killTimer = setTimeout(() => {
          if (child.exitCode == null && child.signalCode == null) child.kill("SIGKILL");
        }, this.serverTermGraceMs);
        killTimer.unref?.();
        await closed;
        clearTimeout(killTimer);
      }
      this.closeLog();
    })();
    return this.closePromise;
  }

  closeLog() {
    if (this.logFd == null) return;
    try { fs.closeSync(this.logFd); } catch {}
    this.logFd = null;
  }
}

export function privateLoopbackUrl(value: string|URL) {
  let url;
  try { url = new URL(value); } catch {
    throw new PrivateServerError("invalidServerUrl", "baseUrl must be a valid loopback URL.");
  }
  if (!new Set(["127.0.0.1", "::1", "localhost"]).has(url.hostname) || !["http:", "https:"].includes(url.protocol)) {
    throw new PrivateServerError("invalidServerUrl", "Lab Interact may reuse only a private loopback server.");
  }
  url.pathname = url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`;
  return url.href;
}

async function isHealthy(baseUrl: string, signal?: AbortSignal) {
  try {
    const response = await fetch(baseUrl, {
      signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(500)]) : AbortSignal.timeout(500),
    });
    return response.ok;
  } catch { return false; }
}

async function allocatePort(signal?: AbortSignal): Promise<number> {
  throwIfAborted(signal);
  const server = net.createServer();
  server.unref();
  await new Promise<void>((resolve, reject) => {
    const onAbort = () => {
      server.close();
      reject(abortedError());
    };
    server.once("error", reject);
    signal?.addEventListener("abort", onAbort, { once: true });
    if (signal?.aborted) {
      onAbort();
      return;
    }
    server.listen(0, "127.0.0.1", () => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    });
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise<void>((resolve) => server.close(() => resolve()));
  if (!port) throw new PrivateServerError("portAllocation", "Could not allocate a private loopback port.");
  return port;
}

function abortableDelay(ms: number, signal?: AbortSignal): Promise<void> {
  if (!signal) return new Promise((resolve) => setTimeout(resolve, ms));
  const abortSignal = signal;
  return new Promise<void>((resolve, reject) => {
    const timer = setTimeout(done, ms);
    const onAbort = () => {
      clearTimeout(timer);
      abortSignal.removeEventListener("abort", onAbort);
      reject(abortedError());
    };
    function done() {
      abortSignal.removeEventListener("abort", onAbort);
      resolve();
    }
    abortSignal.addEventListener("abort", onAbort, { once: true });
  });
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throw abortedError();
}

function abortedError() {
  return new PrivateServerError("sessionClosed", "Lab Interact driver was closed during server startup.");
}

function normalizePrivateServerError(error: unknown) {
  if (error instanceof PrivateServerError) return error;
  const code = error instanceof Error && "code" in error && typeof error.code === "string" ? error.code : "serverStartFailed";
  return new PrivateServerError(code, error instanceof Error ? error.message : String(error || "Private server startup failed."));
}

function conciseProcessFailure(prefix: string, error: unknown) {
  const result = error instanceof ProcessRunnerError ? error.result : null;
  const detail = String(result?.stderr || result?.stdout || (error instanceof Error ? error.message : "unknown failure"))
    .trim().split("\n").slice(-4).join("; ").slice(0, 800);
  return `${prefix}: ${detail}`;
}
