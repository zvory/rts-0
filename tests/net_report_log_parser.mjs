#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const script = path.join(repoRoot, "scripts", "parse-net-report-logs.mjs");
const logs = [
  path.join(
    repoRoot,
    "docs",
    "network-incident-examples",
    "2026-06-19-beta-matt-alex",
    "fly-match-54-all.jsonl"
  ),
  path.join(
    repoRoot,
    "docs",
    "network-incident-examples",
    "2026-06-19-beta-matt-alex",
    "fly-match-55-all.jsonl"
  ),
];

function run(args) {
  return execFileSync("node", [script, ...args], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

const help = run(["--help"]);
assert.match(help, /client_net_report/);
assert.match(help, /performance writer timing/);

const parsed = JSON.parse(run(["--format", "json", ...logs]));
assert.equal(parsed.warnings.length, 0);
assert.equal(parsed.matches.length, 2);

const match54 = parsed.matches.find((match) => match.match === "54");
assert.ok(match54, "expected match 54 summary");
assert.equal(match54.matchRunId, "main-1781830463862-000004");
assert.deepEqual(match54.participants, ["alex", "<b>matt</b>"]);
assert.equal(match54.reportRows, 35);
assert.equal(match54.serverTickRows, 0);

const player5 = match54.players.find((player) => player.playerId === "5");
assert.ok(player5, "expected Matt/player 5 summary");
assert.equal(player5.metrics.rtt_max_ms.max, 759);
assert.equal(player5.metrics.snapshot_gap_max_ms.max, 1077);
assert.equal(player5.metrics.frame_gap_max_ms.max, 700);
assert.equal(player5.metrics.fps_estimate.min, 15);
assert.equal(player5.metrics.snapshot_bytes_max, null);

const serverClassification = match54.classifications.find(
  (item) => item.id === "server_tick_scheduler"
);
const networkClassification = match54.classifications.find(
  (item) => item.id === "client_network_delivery"
);
const browserClassification = match54.classifications.find((item) => item.id === "browser_processing");
assert.equal(serverClassification.result, "not indicated");
assert.equal(networkClassification.result, "indicated");
assert.equal(browserClassification.result, "indicated");
assert.match(match54.transportNote, /Unsupported/);
assert.ok(
  match54.missing.some((item) => item.includes("server snapshot projection")),
  "expected unavailable newer server snapshot perf data to be reported as missing"
);

const markdown = run([...logs]);
assert.match(markdown, /## Match 54/);
assert.match(markdown, /player 5 frame_gap_max_ms max 700/);
assert.match(markdown, /packet loss, retransmits, or per-packet browser transport data/);

const tsv = run(["--format=tsv", ...logs]);
assert.match(tsv, /^match\tplayer_id\treports/m);
assert.match(tsv, /^54\t5\t17\tprediction_disabled:17/m);

const outDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-"));
try {
  const out = run(["--out-dir", outDir, ...logs]);
  assert.match(out, /incident-summary\.md/);
  assert.ok(existsSync(path.join(outDir, "incident-summary.md")));
  assert.ok(existsSync(path.join(outDir, "incident-summary.json")));
  assert.ok(existsSync(path.join(outDir, "incident-rows.tsv")));
  assert.match(readFileSync(path.join(outDir, "incident-summary.md"), "utf8"), /## Match 55/);
} finally {
  rmSync(outDir, { recursive: true, force: true });
}

console.log("net report log parser test passed");
