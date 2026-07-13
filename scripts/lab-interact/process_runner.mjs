import { spawn } from "node:child_process";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_OUTPUT_BYTES = 256 * 1024;
const DEFAULT_TERM_GRACE_MS = 1_000;

export class ProcessRunnerError extends Error {
  constructor(code, message, result = null) {
    super(message);
    this.name = "ProcessRunnerError";
    this.code = code;
    this.result = result;
  }
}

export class ProcessRunner {
  constructor({
    spawnProcess = spawn,
    maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES,
    termGraceMs = DEFAULT_TERM_GRACE_MS,
  } = {}) {
    this.spawnProcess = spawnProcess;
    this.maxOutputBytes = positiveInteger(maxOutputBytes, "maxOutputBytes");
    this.termGraceMs = positiveInteger(termGraceMs, "termGraceMs");
  }

  run(command, args = [], {
    cwd,
    env,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    signal,
    maxOutputBytes = this.maxOutputBytes,
    onSpawn = null,
  } = {}) {
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

    return new Promise((resolve, reject) => {
      const startedAt = Date.now();
      let child;
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

      let stdout = Buffer.alloc(0);
      let stderr = Buffer.alloc(0);
      let stdoutTruncated = false;
      let stderrTruncated = false;
      let termination = null;
      let spawnError = null;
      let timeout = null;
      let killFallback = null;
      let settled = false;

      try { onSpawn?.(child); } catch {}
      child.stdout?.on("data", (chunk) => {
        [stdout, stdoutTruncated] = appendBounded(stdout, chunk, outputLimit, stdoutTruncated);
      });
      child.stderr?.on("data", (chunk) => {
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

      const terminate = (reason) => {
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

  async runOrThrow(command, args = [], options = {}) {
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

function appendBounded(current, chunk, maximum, truncated) {
  const next = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
  if (next.length >= maximum) return [next.subarray(next.length - maximum), true];
  const combined = Buffer.concat([current, next]);
  if (combined.length <= maximum) return [combined, truncated];
  return [combined.subarray(combined.length - maximum), true];
}

function positiveInteger(value, name) {
  if (!Number.isInteger(value) || value < 1) throw new TypeError(`${name} must be a positive integer.`);
  return value;
}

function processFailureMessage(command, error) {
  return `${command} could not start: ${String(error?.message || error || "unknown failure").slice(-800)}`;
}
