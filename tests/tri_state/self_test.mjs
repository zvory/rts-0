#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import assert from "node:assert/strict";
import { ArtifactWriter } from "./artifacts.mjs";
import { forceFailure, scenario, selectOwn } from "./dsl.mjs";
import { WasmLocalLane } from "./lanes/local_lane.mjs";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rts-tri-state-test-"));

{
  const s = scenario("unit_contract", {
    setup: { kind: "liveRoom", prediction: "disabled" },
    steps: [selectOwn("worker", 0)],
  });
  assert.equal(s.name, "unit_contract");
  assert.equal(s.setup.kind, "liveRoom");
  assert.equal(s.setup.quickstart, true);
  assert.throws(() => scenario("Bad Name", { steps: [selectOwn("worker")] }), /invalid scenario name/);
}

{
  const writer = new ArtifactWriter("artifact_contract", { root, runId: "run" });
  writer.writeScenario({ name: "artifact_contract", setup: {}, network: {}, steps: [] });
  writer.timeline({ event: "begin", index: 0 });
  writer.remote({ event: "snapshot", summary: { tick: 1 } });
  writer.client({ event: "capture", summary: { tick: 1 } });
  writer.local({ localLane: "unavailable" });
  writer.diff({ ok: true });
  writer.writeSummary({ status: "passed", command: "node tests/tri_state/run.mjs --scenario artifact_contract" });
  for (const name of ["scenario.json", "timeline.jsonl", "remote.jsonl", "client.jsonl", "local.jsonl", "diffs.jsonl", "summary.md"]) {
    assert.equal(fs.existsSync(path.join(writer.dir, name)), true, `${name} exists`);
  }
  assert.match(fs.readFileSync(path.join(writer.dir, "summary.md"), "utf8"), /Status: passed/);
}

{
  const writer = new ArtifactWriter("local_lane_contract", { root, runId: "run" });
  const lane = new WasmLocalLane({
    artifacts: writer,
    gluePath: path.join(root, "missing-glue.js"),
    wasmPath: path.join(root, "missing.wasm"),
  });
  const frame = await lane.start({
    startInfo: {
      playerId: 1,
      spectator: false,
      debugMode: false,
      tick: 0,
      map: { width: 1, height: 1, tileSize: 32, terrain: [0], resources: [] },
      players: [{ id: 1, name: "A", color: "#f00", startTileX: 0, startTileY: 0 }],
    },
  });
  assert.equal(frame.localLane, "wasm");
  assert.equal(frame.disabledReason, "wasmAssetsMissing");
  await lane.capture("sample");
  assert.match(fs.readFileSync(path.join(writer.dir, "local.jsonl"), "utf8"), /"localLane":"wasm"/);
  assert.match(fs.readFileSync(path.join(writer.dir, "local.jsonl"), "utf8"), /"disabledReason":"wasmAssetsMissing"/);
}

{
  const writer = new ArtifactWriter("forced_failure_contract", { root, runId: "run" });
  const step = forceFailure("intentional test failure");
  writer.timeline({ event: "step.begin", step });
  writer.writeSummary({
    status: "failed",
    failure: { message: step.message, step: step.op },
    command: "node tests/tri_state/run.mjs --scenario forced_failure_artifact --allow-failure",
  });
  const summary = fs.readFileSync(path.join(writer.dir, "summary.md"), "utf8");
  assert.match(summary, /Status: failed/);
  assert.match(summary, /intentional test failure/);
}

console.log("tri-state harness self-test passed");
