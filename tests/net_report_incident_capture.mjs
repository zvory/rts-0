#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const script = path.join(repoRoot, "scripts", "capture-net-incident.mjs");

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

const outDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-incident-capture-"));
try {
  const stdout = execFileSync(
    "node",
    [
      script,
      "--fixture",
      "soupman-alex",
      "--out-dir",
      outDir,
      "--force",
      "--require-coverage",
      "command,snapshot,pathing,client-context",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
      maxBuffer: 128 * 1024 * 1024,
    },
  );
  assert.match(stdout, /incident package:/);
  assert.match(stdout, /coverage: command=present, snapshot=present, pathing=present, client-context=present/);

  const expectedFiles = [
    "README.md",
    "raw/alex-s-lobby-1782778605186-000004-logs.jsonl",
    "parser/README.md",
    "parser/incident-summary.md",
    "parser/incident-summary.json",
    "parser/incident-rows.tsv",
    "parser/client-net-rows.tsv",
    "parser/server-tick-rows.tsv",
    "agent-digest.md",
    "agent-digest.json",
    "key-metrics.json",
    "diagnostic-coverage.json",
    "replay/replay.json",
    "db/db-summary.json",
    "player-report-notes.md",
    "analysis.md",
    "beta-evidence-checklist.md",
    "package-manifest.json",
  ];
  for (const file of expectedFiles) {
    assert.ok(existsSync(path.join(outDir, file)), `expected ${file}`);
  }

  const readme = readFileSync(path.join(outDir, "README.md"), "utf8");
  assert.match(readme, /Match run id \| `alex-s-lobby-1782778605186-000004`/);
  assert.match(readme, /UTC window \| `2026-06-30T00:16:45\.186897894Z to 2026-06-30T00:41:25\.868590778Z`/);
  assert.match(readme, /Build \| `5d33dc1e4d7c`/);
  assert.match(readme, /Participants \| `soupman, alex`/);
  assert.match(readme, /Neutral Diagnosis/);
  assert.match(readme, /does not prescribe a fix/);
  assert.match(readme, /raw\/alex-s-lobby-1782778605186-000004-logs\.jsonl/);

  const coverage = readJson(path.join(outDir, "diagnostic-coverage.json"));
  assert.deepEqual(coverage.missing, []);
  for (const id of ["command", "snapshot", "pathing", "client-context"]) {
    assert.equal(
      coverage.requirements.find((item) => item.id === id)?.present,
      true,
      `expected ${id} coverage`,
    );
  }
  assert.ok(
    coverage.requirements
      .find((item) => item.id === "pathing")
      ?.evidence.some((item) => item.includes("awaiting_paths")),
    "expected pathing coverage to include slowest-phase evidence",
  );

  const digest = readFileSync(path.join(outDir, "agent-digest.md"), "utf8");
  assert.match(digest, /# Agent Digest/);
  assert.match(digest, /## Coverage Requirements/);
  assert.match(digest, /command response top windows/);
  assert.match(digest, /snapshot payload top windows/);

  const keyMetrics = readJson(path.join(outDir, "key-metrics.json"));
  assert.equal(keyMetrics.matches[0].matchRunId, "alex-s-lobby-1782778605186-000004");
  assert.ok(
    keyMetrics.unknowns.some((item) => item.text.includes("writer send detail: not logged or unavailable")),
    "expected explicit missing writer detail unknown",
  );

  const analysis = readFileSync(path.join(outDir, "analysis.md"), "utf8");
  for (const heading of ["## Supported", "## Contradicted", "## Unknown", "## Next Diagnostic Gaps"]) {
    assert.match(analysis, new RegExp(heading.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  assert.doesNotMatch(analysis, /we should fix|optimize pathing|reduce payload/i);

  const notes = readFileSync(path.join(outDir, "player-report-notes.md"), "utf8");
  assert.match(notes, /absolute UTC timestamps/);
  assert.match(notes, /No player report notes were provided/);

  const manifest = readJson(path.join(outDir, "package-manifest.json"));
  assert.equal(manifest.incident.matchId, "103");
  assert.equal(manifest.artifacts.replay.present, true);
  assert.equal(manifest.artifacts.dbSummary.present, true);
  assert.ok(manifest.files.some((file) => file.path === "analysis.md"));
} finally {
  rmSync(outDir, { recursive: true, force: true });
}

console.log("net report incident capture test passed");
