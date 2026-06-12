import {
  advanceLocalTicks,
  assertLocalCorrectionAtMost,
  assertLocalOwnedStable,
  assertLocalPendingClientSeqs,
  capture,
  scenario,
} from "../dsl.mjs";

export default scenario("local_lane_noop_ticks", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    capture("before-noop"),
    advanceLocalTicks(20),
    capture("after-noop"),
    assertLocalOwnedStable({ before: "before-noop", after: "after-noop", unit: "worker" }),
    assertLocalPendingClientSeqs([]),
    assertLocalCorrectionAtMost(0),
  ],
});
