#!/usr/bin/env node

import assert from "node:assert/strict";
import test from "node:test";
import { summarize } from "../scripts/deploy-timings.mjs";

function record(atMs, line) {
  return { atMs, line };
}

test("deploy timing summary reports phase durations and cache state", () => {
  const markdown = summarize([
    record(0, "==> Building image"),
    record(1_000, "Waiting for depot builder..."),
    record(14_000, "==> Building image with Depot"),
    record(15_000, "#17 [wasm-builder] RUN RTS_SIM_WASM_OUT_DIR=/app/out ./scripts/build-sim-wasm.sh"),
    record(85_000, "#17 DONE 70.0s"),
    record(85_100, "#20 [server-builder] RUN cargo build --release --locked -p rts-server --bin rts-server"),
    record(205_100, "#20 DONE 120.0s"),
    record(205_200, "#24 exporting to image"),
    record(207_700, "#24 DONE 2.5s"),
    record(208_000, "Updating existing machines in 'beta' with rolling strategy"),
    record(214_000, "Visit your newly deployed app"),
  ], 0);

  assert.match(markdown, /Deploy succeeded/);
  assert.match(markdown, /\| Builder wait \| 13\.0s \| — \|/);
  assert.match(markdown, /\| Prediction WASM \| 1m 10\.0s \| miss \|/);
  assert.match(markdown, /\| Native rts-server \| 2m 0\.0s \| miss \|/);
  assert.match(markdown, /\| Image export \| 2\.5s \| miss \|/);
  assert.match(markdown, /\| Machine rollout \| 6\.0s \| — \|/);
  assert.match(markdown, /\| Fly deploy total \| 3m 34\.0s \| — \|/);
});

test("deploy timing summary identifies cached steps and preserves failures", () => {
  const markdown = summarize([
    record(0, "#9 [wasm-builder] RUN ./scripts/build-sim-wasm.sh"),
    record(10, "#9 CACHED"),
    record(20, "#11 [server-builder] RUN cargo build --release --locked -p rts-server --bin rts-server"),
    record(30, "#11 CACHED"),
    record(40, "Error: rollout failed"),
  ], 1);

  assert.match(markdown, /Deploy failed \(exit 1\)/);
  assert.match(markdown, /\| Prediction WASM \| 0\.0s \| hit \|/);
  assert.match(markdown, /\| Native rts-server \| 0\.0s \| hit \|/);
  assert.match(markdown, /\| Machine rollout \| unavailable \| — \|/);
});
