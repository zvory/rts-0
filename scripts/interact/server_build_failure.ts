import fs from "node:fs";

import { ProcessRunnerError } from "./process_runner.ts";

export function serverBuildFailure(error: unknown, logPath: string, timeoutMs: number) {
  const result = error instanceof ProcessRunnerError ? error.result : null;
  const output = [result?.stdout, result?.stderr].filter(Boolean).join("\n").trim();
  const fallback = error instanceof Error ? error.message : "unknown failure";
  try { fs.writeFileSync(logPath, `${output || fallback}\n`); } catch {}
  const timedOut = error instanceof ProcessRunnerError && error.code === "processTimeout";
  const detail = String(result?.stderr || result?.stdout || fallback)
    .trim().split("\n").slice(-8).join("; ").slice(0, 800);
  const status = result
    ? `exit=${result.status == null ? "none" : result.status}${result.signal ? ` signal=${result.signal}` : ""}`
    : "exit=unavailable";
  return {
    code: timedOut ? "serverBuildTimeout" : "serverBuild",
    message: timedOut
      ? `Interact server build exceeded its ${timeoutMs}ms cold-build deadline (${status}); see ${logPath}. Last output: ${detail}`
      : `Interact server build failed (${status}); see ${logPath}. Last output: ${detail}`,
    details: {
      buildLog: logPath,
      processCode: error instanceof ProcessRunnerError ? error.code : null,
      exitCode: result?.status ?? null,
      signal: result?.signal ?? null,
      durationMs: result?.durationMs ?? null,
      timedOut,
      stdoutTruncated: result?.stdoutTruncated ?? false,
      stderrTruncated: result?.stderrTruncated ?? false,
    },
  };
}
