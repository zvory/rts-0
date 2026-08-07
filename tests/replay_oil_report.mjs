import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "replay-oil-report-"));
const analyses = path.join(root, "analyses");
const out = path.join(root, "out");
fs.mkdirSync(analyses);
const manifest = {
  aliases: ["soupman", "tomis", "alex"],
  selection: "fixture",
  matches: [{ id: 7, started_at: "2026-08-01T00:00:00Z", map_name: "Fixture, Map", duration_ticks: 100, build_sha: "abc", participants: ["Alex", "Soupman"] }],
};
fs.writeFileSync(path.join(root, "manifest.json"), JSON.stringify(manifest));
const vehicles = [
  [1, "scout_car", 1, true],
  [2, "scout_car", 2, false],
  [3, "scout_car", 3, true],
  [4, "scout_car", 4, true],
  [5, "scout_car", 0, true],
  [6, "command_car", 5, true],
].map(([entityId, unitKind, lifetimeOilSpend, survivedToEnd]) => ({ entityId, ownerId: 1, unitKind, firstSeenTick: 1, lastSeenTick: 99, firstMovedTick: lifetimeOilSpend ? 2 : null, lastMovedTick: lifetimeOilSpend ? 90 : null, lifetimeOilSpend, survivedToEnd }));
const analysis = { matchId: 7, analysisBuildSha: "abc", replay: { serverBuildSha: "abc", mapName: "Fixture, Map", durationTicks: 100, players: [{ id: 1, name: "Alex" }, { id: 2, name: "Soupman" }] }, vehicles };
fs.writeFileSync(path.join(analyses, "7.json"), JSON.stringify(analysis));

const run = spawnSync(process.execPath, ["--no-warnings", "scripts/replay-oil-report.mjs", "--analyses", analyses, "--manifest", path.join(root, "manifest.json"), "--out", out], { cwd: path.resolve(import.meta.dirname, ".."), encoding: "utf8" });
assert.equal(run.status, 0, run.stderr);
const db = new DatabaseSync(path.join(out, "replay_oil_analysis.sqlite"), { readOnly: true });
assert.deepEqual({ ...db.prepare("select total_observed,moved_count,unmoved_count,mean,p50,p75,p95,p99,max from unit_oil_summary where unit_kind='scout_car'").get() }, { total_observed: 5, moved_count: 4, unmoved_count: 1, mean: 2.5, p50: 2, p75: 3, p95: 4, p99: 4, max: 4 });
assert.equal(db.prepare("select count(*) count from unit_oil_spend").get().count, 6);
assert.equal(db.prepare("select count(*) count from unit_oil_spend where moved=1").get().count, 5);
db.close();
const rerun = spawnSync(process.execPath, ["--no-warnings", "scripts/replay-oil-report.mjs", "--analyses", analyses, "--manifest", path.join(root, "manifest.json"), "--out", out], { cwd: path.resolve(import.meta.dirname, ".."), encoding: "utf8" });
assert.equal(rerun.status, 0, rerun.stderr);
const rawCsv = fs.readFileSync(path.join(out, "unit_oil_spend.csv"), "utf8");
assert.match(rawCsv, /"Fixture, Map"/);
assert.match(fs.readFileSync(path.join(out, "selected_replays.csv"), "utf8"), /\[""Alex"",""Soupman""\]/);
for (const filename of ["oil-spend-percentiles.svg", "oil-spend-ecdf.svg", "oil-spend-inclusion.svg", "oil-spend-report.html"]) {
  const text = fs.readFileSync(path.join(out, filename), "utf8");
  assert.ok(text.length > 200);
  assert.doesNotMatch(text, /NaN|undefined/);
}

analysis.replay.durationTicks = 99;
fs.writeFileSync(path.join(analyses, "7.json"), JSON.stringify(analysis));
const rejected = spawnSync(process.execPath, ["--no-warnings", "scripts/replay-oil-report.mjs", "--analyses", analyses, "--manifest", path.join(root, "manifest.json"), "--out", path.join(root, "rejected")], { cwd: path.resolve(import.meta.dirname, ".."), encoding: "utf8" });
assert.notEqual(rejected.status, 0);
assert.match(rejected.stderr, /duration mismatch/);
console.log("replay oil report fixture passed");
