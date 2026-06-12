import {
  assertClientAuthoritativeOwnedStable,
  assertClientPrediction,
  assertClientRenderedOwnedStable,
  capture,
  issue,
  scenario,
  selectOwn,
} from "../dsl.mjs";

export default scenario("prediction_disabled_authoritative_only", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "disabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "prediction-off-snapshot-delay",
    tickMs: 10,
    snapshotLatencyTicks: 20,
    seed: 4516,
  },
  steps: [
    capture("before-move"),
    selectOwn("worker", 0),
    issue("move", { dx: 192, dy: 0 }),
    capture("after-authoritative-only-issue"),
    assertClientPrediction({ enabled: false, mode: "disabled" }),
    assertClientAuthoritativeOwnedStable({ before: "before-move", after: "after-authoritative-only-issue", unit: "worker" }),
    assertClientRenderedOwnedStable({ before: "before-move", after: "after-authoritative-only-issue", unit: "worker" }),
  ],
});
