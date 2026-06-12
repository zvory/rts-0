import {
  assertLocalOrderPlan,
  assertLocalPendingClientSeqs,
  capture,
  issue,
  scenario,
  selectOwn,
} from "../dsl.mjs";

export default scenario("local_lane_queued_move", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { x: 3392, y: 3216 }),
    issue("attackMove", { x: 3488, y: 3280, queued: true }),
    capture("after-queued-local-orders"),
    assertLocalOrderPlan({
      unit: "worker",
      expected: [
        { kind: "move", x: 3392, y: 3216 },
        { kind: "attackMove", x: 3488, y: 3280 },
      ],
    }),
    assertLocalPendingClientSeqs([1, 2]),
  ],
});
