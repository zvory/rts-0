import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { PNG } from "pngjs";
import {
  analyzeDecodedRgba,
  assertCaptureContent,
  assertCaptureHealthy,
  assertCaptureInputsEqual,
  assertCaptureSequenceVaries,
  compareDecodedRgba,
  parseClientRenderParityArgs,
  readExplicitTicks,
  selectDeterministicTicks,
  withIsolatedServers,
} from "../../scripts/client-render-parity.mjs";

const parsed = parseClientRenderParityArgs([
  "--baseline-worktree", "/tmp/base",
  "--candidate-worktree", "/tmp/candidate",
  "--workload", "supply-300-hellhole-stream",
  "--seed", "phase-1-seed",
  "--samples", "16",
  "--viewport", "1440x900",
  "--dpr", "1",
  "--alpha", "1",
  "--visual-time-ms", "90000",
  "--output-root", "/tmp/parity",
]);
assert.deepEqual(parsed.viewport, { width: 1440, height: 900 });
assert.equal(parsed.samples, 16);
assert.equal(parsed.alpha, 1);
assert.equal(parsed.visualTimeMs, 90_000);
assert.throws(() => parseClientRenderParityArgs([]), /baseline-worktree/);
assert.throws(() => parseClientRenderParityArgs([
  "--baseline-worktree", "/tmp/base",
  "--candidate-worktree", "/tmp/candidate",
  "--viewport", "wide",
]), /viewport/);
assert.throws(() => parseClientRenderParityArgs([
  "--baseline-worktree", "/tmp/base",
  "--candidate-worktree", "/tmp/candidate",
  "--alpha", "1.1",
]), /between 0 and 1/);
assert.throws(() => parseClientRenderParityArgs([
  "--baseline-worktree", "/tmp/base",
  "--candidate-worktree", "/tmp/candidate",
  "--alpha", "0.5",
]), /production fixed-capture path/);

const firstTicks = selectDeterministicTicks({ frameCount: 900, samples: 16, seed: "phase-1-seed" });
const secondTicks = selectDeterministicTicks({ frameCount: 900, samples: 16, seed: "phase-1-seed" });
assert.deepEqual(firstTicks, secondTicks, "seeded tick selection is deterministic");
assert.equal(firstTicks.length, 16);
assert.equal(new Set(firstTicks).size, 16);
assert.deepEqual([...firstTicks].sort((a, b) => a - b), firstTicks);
assert.throws(() => selectDeterministicTicks({ frameCount: 3, samples: 4, seed: "x" }), /between 1 and 3/);

const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "rts-client-render-parity-contract-"));
try {
  const ticksPath = path.join(temporary, "ticks.json");
  fs.writeFileSync(ticksPath, JSON.stringify({ ticks: [7, 2, 19] }));
  assert.deepEqual(readExplicitTicks(ticksPath, 20), [2, 7, 19]);
  fs.writeFileSync(ticksPath, JSON.stringify([2, 2]));
  assert.throws(() => readExplicitTicks(ticksPath, 20), /unique integers/);

  const baseline = new PNG({ width: 2, height: 2 });
  baseline.data.fill(17);
  const identical = new PNG({ width: 2, height: 2 });
  baseline.data.copy(identical.data);
  const match = compareDecodedRgba(baseline, identical);
  assert.equal(match.identical, true, "byte-identical decoded RGBA passes");
  assert.equal(match.changedPixels, 0);

  const changed = new PNG({ width: 2, height: 2 });
  baseline.data.copy(changed.data);
  changed.data[4] += 1;
  const mismatch = compareDecodedRgba(baseline, changed);
  assert.equal(mismatch.identical, false, "one changed channel fails exact parity");
  assert.equal(mismatch.changedPixels, 1);
  assert.deepEqual(mismatch.bounds, { minX: 1, minY: 0, maxX: 1, maxY: 0 });
  assert.ok(PNG.sync.write(mismatch.diff).length > 0, "one-pixel failures produce a PNG diff artifact");

  const black = new PNG({ width: 32, height: 32 });
  black.data.fill(0);
  const blackContent = analyzeDecodedRgba(black);
  assert.equal(blackContent.uniqueColors, 1);
  assert.throws(
    () => assertCaptureContent(blackContent, "black framebuffer"),
    /visually empty/,
    "a cleared black framebuffer can never pass parity",
  );

  const varied = new PNG({ width: 32, height: 32 });
  varied.data.fill(255);
  for (let index = 0; index < 128; index += 1) {
    varied.data[index * 4] = index;
    varied.data[index * 4 + 1] = 255 - index;
  }
  assert.doesNotThrow(() => assertCaptureContent(analyzeDecodedRgba(varied), "varied frame"));
  assert.doesNotThrow(() => assertCaptureSequenceVaries([
    { rgbaSha256: "frame-a" },
    { rgbaSha256: "frame-b" },
  ]));
  assert.throws(
    () => assertCaptureSequenceVaries([{ rgbaSha256: "same" }, { rgbaSha256: "same" }]),
    /same RGBA frame/,
    "a frozen or repeatedly cleared capture sequence fails",
  );
} finally {
  fs.rmSync(temporary, { recursive: true, force: true });
}

const healthy = {
  readiness: { failedAssets: [], pendingAssets: [], renderErrors: [], missingTextureSubjectIds: [] },
  pageErrors: [],
  consoleErrors: [],
  requestFailures: [],
};
assert.doesNotThrow(() => assertCaptureHealthy(healthy));
assert.throws(
  () => assertCaptureHealthy({ ...healthy, readiness: { ...healthy.readiness, failedAssets: [{ id: "tank", message: "404" }] } }),
  /asset tank failed/,
  "missing or failed assets fail the capture",
);
assert.throws(
  () => assertCaptureHealthy({ ...healthy, pageErrors: ["render exploded"] }),
  /page error: render exploded/,
  "browser errors fail the capture",
);
assert.throws(
  () => assertCaptureHealthy({ ...healthy, readiness: { ...healthy.readiness, renderErrors: [{ label: "hp", message: "draw failed" }] } }),
  /render hp/,
  "renderer errors fail the capture",
);

const exactInputs = {
  frameIndex: 12,
  stateTick: 13,
  stateSha256: "abc",
  viewport: { width: 1440, height: 900 },
  dpr: 1,
  alpha: 1,
  visualTimeMs: 120_000,
  camera: { version: 1, focus: { x: 20, y: 30 } },
};
assert.doesNotThrow(() => assertCaptureInputsEqual(exactInputs, structuredClone(exactInputs)));
assert.throws(
  () => assertCaptureInputsEqual(exactInputs, { ...exactInputs, stateTick: 14 }),
  /input mismatch/,
  "state, tick, and capture inputs must match exactly",
);

const closed = [];
await assert.rejects(
  withIsolatedServers(
    ["base", "candidate"],
    async (name) => ({ name, close: async () => { closed.push(name); } }),
    async () => { throw new Error("capture failed"); },
  ),
  /capture failed/,
);
assert.deepEqual(closed.sort(), ["base", "candidate"], "both isolated local servers close after capture failure");

console.log("✅ client_render_parity_contracts.mjs: CLI, ticks, framebuffer canaries, RGBA diff, failures, and cleanup passed");
