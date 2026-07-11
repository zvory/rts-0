import assert from "node:assert/strict";

import { CaptureRenderClock } from "../../client/src/visual_clock.js";
import { createMatchRenderClock, enterFixedCapture, exitFixedCapture } from "../../client/src/match_fixed_capture.js";
import { fixedFrameTick } from "../../scripts/lab-interact/fixed_capture.mjs";

const clock = new CaptureRenderClock(100);
assert.equal(clock.now(), 100, "capture clock starts at the injected visual timestamp");
assert.equal(clock.advanceTo(133.25), 133.25, "capture clock advances to exact fractional milliseconds");
assert.throws(() => clock.advanceTo(120), /monotonically/, "capture visual time cannot move backward");
assert.throws(() => new CaptureRenderClock(-1), /non-negative/, "capture visual time remains bounded to its own valid domain");

assert.deepEqual(
  Array.from({ length: 6 }, (_, index) => fixedFrameTick(20, index, 60)),
  [20, 20, 21, 21, 22, 22],
  "60 FPS capture repeats authoritative 30 Hz states without interpolating them",
);

const savedRaf = globalThis.requestAnimationFrame;
const savedCancel = globalThis.cancelAnimationFrame;
let cancelled = null;
let resumed = 0;
globalThis.cancelAnimationFrame = (id) => { cancelled = id; };
globalThis.requestAnimationFrame = () => ++resumed;
try {
  const match = {
    ...createMatchRenderClock(), running: true, rafId: 7, tickFn() {}, lastFrame: 0,
    renderer: { setRenderClock(clockValue) { this.clock = clockValue; } },
  };
  const entered = enterFixedCapture(match);
  assert.equal(cancelled, 7, "entering fixed capture suspends the owned rAF callback");
  assert.equal(match.renderer.clock, match.captureClock, "renderer receives only the capture visual clock");
  assert.equal(entered.visualStartMs, match.captureClock.now(), "capture reports its exact visual origin");
  assert.deepEqual(exitFixedCapture(match), { resumed: true }, "exiting capture reports normal loop restoration");
  assert.equal(resumed, 1, "normal rAF ownership resumes exactly once");
  assert.notEqual(match.renderer.clock, match.captureClock, "renderer returns to an isolated normal clock");
  const stopped = { ...createMatchRenderClock(), running: true, rafId: 9, tickFn() {}, lastFrame: 0, renderer: { setRenderClock() {} } };
  enterFixedCapture(stopped);
  stopped.running = false;
  stopped.captureRafWasRunning = false;
  assert.deepEqual(exitFixedCapture(stopped), { resumed: false }, "teardown during capture does not restart rAF");
} finally {
  globalThis.requestAnimationFrame = savedRaf;
  globalThis.cancelAnimationFrame = savedCancel;
}
assert.deepEqual(
  Array.from({ length: 4 }, (_, index) => fixedFrameTick(20, index, 15)),
  [20, 22, 24, 26],
  "15 FPS capture advances two authoritative ticks per visual frame",
);

console.log("✅ visual_clock_contracts.mjs: isolated monotonic visual clock and tick mapping passed");
