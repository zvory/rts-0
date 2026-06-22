import {
  advanceLocalTicks,
  assertLocalOwnedAdvanced,
  assertLocalPendingClientSeqs,
  capture,
  issue,
  scenario,
  selectOwn,
} from "../dsl.mjs";

export default scenario("local_lane_simple_move", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
  },
  network: { mode: "direct" },
  steps: [
    capture("before-move"),
    selectOwn("worker", 0),
    issue("move", { dx: 160, dy: 0 }),
    advanceLocalTicks(20),
    capture("after-local-move"),
    assertLocalOwnedAdvanced({ before: "before-move", unit: "worker", minDistancePx: 1 }),
    assertLocalPendingClientSeqs([1]),
  ],
});
