import { assert } from "./assertions.mjs";
import {
  beginUncappedPerfBenchmark,
  endUncappedPerfBenchmark,
} from "../../client/src/match_perf_benchmark.js";

const savedCancel = globalThis.cancelAnimationFrame;
const savedRequest = globalThis.requestAnimationFrame;
const cancelled = [];
const requested = [];
globalThis.cancelAnimationFrame = (id) => cancelled.push(id);
globalThis.requestAnimationFrame = (callback) => { requested.push(callback); return 19; };

try {
  const match = {
    running: true,
    rafId: 7,
    tickFn() {},
    lastFrame: 0,
    captureClock: null,
    uncappedPerfBenchmark: null,
  };
  const entered = beginUncappedPerfBenchmark(match);
  assert(entered.resumeRaf === true && cancelled[0] === 7 && match.rafId === undefined,
    "uncapped benchmark takes exclusive ownership from the ordinary animation-frame loop");
  const exited = endUncappedPerfBenchmark(match);
  assert(exited.resumed === true && requested.length === 1 && match.rafId === 19,
    "uncapped benchmark restores exactly one ordinary animation-frame callback");
  assert(endUncappedPerfBenchmark(match).resumed === false && requested.length === 1,
    "uncapped benchmark teardown is idempotent");
} finally {
  if (savedCancel === undefined) delete globalThis.cancelAnimationFrame;
  else globalThis.cancelAnimationFrame = savedCancel;
  if (savedRequest === undefined) delete globalThis.requestAnimationFrame;
  else globalThis.requestAnimationFrame = savedRequest;
}
