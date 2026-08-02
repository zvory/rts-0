#!/usr/bin/env node

import fs from "node:fs";
import readline from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ANSI_ESCAPE = /\x1b\[[0-?]*[ -/]*[@-~]/g;

function clean(line) {
  return line.replace(ANSI_ESCAPE, "");
}

function elapsed(records, startPattern, endPattern) {
  const start = records.findIndex((record) => startPattern.test(record.line));
  if (start < 0) return null;
  const endOffset = records.slice(start + 1).findIndex((record) => endPattern.test(record.line));
  if (endOffset < 0) return null;
  return Math.max(0, records[start + 1 + endOffset].atMs - records[start].atMs) / 1000;
}

function buildkitStep(records, instructionPattern) {
  const instruction = records.find((record) => instructionPattern.test(record.line));
  const stepId = instruction?.line.match(/^#(\d+)\b/)?.[1];
  if (!stepId) return { seconds: null, cached: null };

  const resultPattern = new RegExp(`^#${stepId}\\s+(?:DONE(?:\\s+([0-9.]+)s)?|CACHED)\\s*$`);
  for (const record of records) {
    const match = record.line.match(resultPattern);
    if (!match) continue;
    return {
      seconds: match[1] === undefined ? 0 : Number(match[1]),
      cached: record.line.endsWith("CACHED"),
    };
  }
  return { seconds: null, cached: null };
}

function formatSeconds(seconds) {
  if (!Number.isFinite(seconds)) return "unavailable";
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}m ${(seconds - minutes * 60).toFixed(1)}s`;
}

function phaseRow(name, measurement) {
  const cache = measurement.cached === true ? "hit" : measurement.cached === false ? "miss" : "—";
  return `| ${name} | ${formatSeconds(measurement.seconds)} | ${cache} |`;
}

export function summarize(records, exitStatus) {
  const normalized = records.map((record) => ({
    atMs: Number(record.atMs),
    line: clean(String(record.line)),
  }));
  const first = normalized.at(0);
  const last = normalized.at(-1);
  const total = first && last ? Math.max(0, last.atMs - first.atMs) / 1000 : null;
  const builderWait = elapsed(
    normalized,
    /Waiting for (?:depot|remote) builder/i,
    /Building image with (?:Depot|remote builder)/i,
  );
  const rollout = elapsed(
    normalized,
    /Updating existing machines|Creating a new machine/i,
    /Visit your newly deployed app|DNS configuration verified|is now in a good state/i,
  );

  const rows = [
    phaseRow("Builder wait", { seconds: builderWait, cached: null }),
    phaseRow("Prediction WASM", buildkitStep(normalized, /RUN .*build-sim-wasm\.sh/)),
    phaseRow("Native rts-server", buildkitStep(normalized, /RUN cargo build .*--bin rts-server/)),
    phaseRow("Image export", buildkitStep(normalized, /exporting to image\s*$/)),
    phaseRow("Machine rollout", { seconds: rollout, cached: null }),
    phaseRow("Fly deploy total", { seconds: total, cached: null }),
  ];
  const outcome = Number(exitStatus) === 0 ? "succeeded" : `failed (exit ${exitStatus})`;

  return [
    "## Beta deploy timing",
    "",
    `Deploy ${outcome}. Timings come from the streamed flyctl and BuildKit output.`,
    "",
    "| Phase | Time | Build cache |",
    "| --- | ---: | :---: |",
    ...rows,
    "",
  ].join("\n");
}

async function capture(outputPath) {
  const output = fs.createWriteStream(outputPath, { flags: "w" });
  const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of input) {
    process.stdout.write(`${line}\n`);
    output.write(`${JSON.stringify({ atMs: Date.now(), line })}\n`);
  }
  await new Promise((resolve, reject) => {
    output.once("error", reject);
    output.end(resolve);
  });
}

function readRecords(inputPath) {
  return fs.readFileSync(inputPath, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function main() {
  const [mode, recordPath, status = "0"] = process.argv.slice(2);
  if (mode === "capture" && recordPath) {
    await capture(recordPath);
    return;
  }
  if (mode === "summarize" && recordPath) {
    process.stdout.write(`${summarize(readRecords(recordPath), Number(status))}\n`);
    return;
  }
  console.error("usage: deploy-timings.mjs capture <record.jsonl> | summarize <record.jsonl> [exit-status]");
  process.exitCode = 2;
}

if (fileURLToPath(import.meta.url) === path.resolve(process.argv[1] || "")) {
  await main();
}
