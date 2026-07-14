import { spawn } from "node:child_process";
import type { ChildProcess, SpawnOptions } from "node:child_process";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_OUTPUT_BYTES = 256 * 1024;
const DEFAULT_TERM_GRACE_MS = 1_000;

export class ProcessRunnerError extends Error {
  code: string;
  result: ProcessResult | null;
  constructor(code: string, message: string, result: ProcessResult | null = null) {
    super(message);
    this.name = "ProcessRunnerError";
    this.code = code;
    this.result = result;
  }
}

export interface ProcessResult {
  command: string;
  args: string[];
  pid: number | null;
  status: number | null;
  signal: NodeJS.Signals | null;
  stdout: string;
  stderr: string;
  stdoutTruncated: boolean;
  stderrTruncated: boolean;
  durationMs: number;
  terminatedBy: "abort" | "timeout" | null;
}

type SpawnProcess = (command: string, args: readonly string[], options: SpawnOptions) => ChildProcess;
export interface ProcessRunOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  timeoutMs?: number;
  signal?: AbortSignal;
  maxOutputBytes?: number;
  onSpawn?: ((child: ChildProcess) => void) | null;
}

interface ProcessRunnerOptions {
  spawnProcess?: SpawnProcess;
  maxOutputBytes?: number;
  termGraceMs?: number;
}

export class ProcessRunner {
  termGraceMs: number;
  maxOutputBytes: number;
  spawnProcess: SpawnProcess;
  constructor({
    spawnProcess = spawn,
    maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES,
    termGraceMs = DEFAULT_TERM_GRACE_MS,
  }: ProcessRunnerOptions = {}) {
    this.spawnProcess = spawnProcess;
    this.maxOutputBytes = positiveInteger(maxOutputBytes, "maxOutputBytes");
    this.termGraceMs = positiveInteger(termGraceMs, "termGraceMs");
  }

  run(command: string, args: string[] = [], {
    cwd,
    env,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    signal,
    maxOutputBytes = this.maxOutputBytes,
    onSpawn = null,
  }: ProcessRunOptions = {}): Promise<ProcessResult> {
    if (typeof command !== "string" || command.length === 0) {
      throw new TypeError("ProcessRunner command must be a non-empty string.");
    }
    if (!Array.isArray(args) || !args.every((value) => typeof value === "string")) {
      throw new TypeError("ProcessRunner args must be an array of strings.");
    }
    const boundedTimeoutMs = positiveInteger(timeoutMs, "timeoutMs");
    const outputLimit = positiveInteger(maxOutputBytes, "maxOutputBytes");
    if (signal?.aborted) {
      return Promise.reject(new ProcessRunnerError("processAborted", `${command} was aborted before it started.`));
    }

    return new Promise<ProcessResult>((resolve, reject) => {
      const startedAt = Date.now();
      let child: ChildProcess;
      try {
        child = this.spawnProcess(command, args, {
          cwd,
          env,
          shell: false,
          stdio: ["ignore", "pipe", "pipe"],
        });
      } catch (error) {
        reject(new ProcessRunnerError("processSpawnFailed", processFailureMessage(command, error)));
        return;
      }

      let stdout: Buffer<ArrayBufferLike> = Buffer.alloc(0);
      let stderr: Buffer<ArrayBufferLike> = Buffer.alloc(0);
      let stdoutTruncated = false;
      let stderrTruncated = false;
      let termination: ProcessResult["terminatedBy"] = null;
      let spawnError: Error | null = null;
      let timeout: NodeJS.Timeout | undefined;
      let killFallback: NodeJS.Timeout | undefined;
      let settled = false;

      try { onSpawn?.(child); } catch {}
      child.stdout?.on("data", (chunk: Buffer | string) => {
        [stdout, stdoutTruncated] = appendBounded(stdout, chunk, outputLimit, stdoutTruncated);
      });
      child.stderr?.on("data", (chunk: Buffer | string) => {
        [stderr, stderrTruncated] = appendBounded(stderr, chunk, outputLimit, stderrTruncated);
      });
      child.once("error", (error) => { spawnError = error; });
      child.once("close", (status, exitSignal) => {
        if (settled) return;
        settled = true;
        cleanup();
        const result = {
          command,
          args: [...args],
          pid: child.pid ?? null,
          status,
          signal: exitSignal,
          stdout: stdout.toString("utf8"),
          stderr: stderr.toString("utf8"),
          stdoutTruncated,
          stderrTruncated,
          durationMs: Date.now() - startedAt,
          terminatedBy: termination,
        };
        if (spawnError) {
          reject(new ProcessRunnerError("processSpawnFailed", processFailureMessage(command, spawnError), result));
        } else if (termination === "abort") {
          reject(new ProcessRunnerError("processAborted", `${command} was aborted.`, result));
        } else if (termination === "timeout") {
          reject(new ProcessRunnerError("processTimeout", `${command} timed out after ${boundedTimeoutMs}ms.`, result));
        } else {
          resolve(result);
        }
      });

      const terminate = (reason: Exclude<ProcessResult["terminatedBy"], null>) => {
        if (termination || child.exitCode != null || child.signalCode != null) return;
        termination = reason;
        child.kill("SIGTERM");
        killFallback = setTimeout(() => {
          if (child.exitCode == null && child.signalCode == null) child.kill("SIGKILL");
        }, this.termGraceMs);
        killFallback.unref?.();
      };
      const onAbort = () => terminate("abort");
      signal?.addEventListener("abort", onAbort, { once: true });
      if (signal?.aborted) onAbort();
      timeout = setTimeout(() => terminate("timeout"), boundedTimeoutMs);
      timeout.unref?.();

      function cleanup() {
        clearTimeout(timeout);
        clearTimeout(killFallback);
        signal?.removeEventListener("abort", onAbort);
      }
    });
  }

  async runOrThrow(command: string, args: string[] = [], options: ProcessRunOptions = {}): Promise<ProcessResult> {
    const result = await this.run(command, args, options);
    if (result.status !== 0) {
      throw new ProcessRunnerError(
        "processFailed",
        `${command} exited with status ${result.status}${result.signal ? ` (${result.signal})` : ""}.`,
        result,
      );
    }
    return result;
  }
}

function appendBounded(current: Buffer<ArrayBufferLike>, chunk: Buffer<ArrayBufferLike> | string, maximum: number, truncated: boolean): [Buffer<ArrayBufferLike>, boolean] {
  const next = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
  if (next.length >= maximum) return [next.subarray(next.length - maximum), true];
  const combined = Buffer.concat([new Uint8Array(current), new Uint8Array(next)]);
  if (combined.length <= maximum) return [combined, truncated];
  return [combined.subarray(combined.length - maximum), true];
}

function positiveInteger(value: unknown, name: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 1) throw new TypeError(`${name} must be a positive integer.`);
  return value;
}

function processFailureMessage(command: string, error: unknown) {
  const detail = error instanceof Error ? error.message : error;
  return `${command} could not start: ${String(detail || "unknown failure").slice(-800)}`;
}
