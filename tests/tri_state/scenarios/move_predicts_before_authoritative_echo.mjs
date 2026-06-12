import {
  assertClientAuthoritativeOwnedStable,
  assertClientRenderedOwnedAdvanced,
  advanceClientPredictionVisual,
  capture,
  issue,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
  waitForClientPredictionReady,
  waitMs,
} from "../dsl.mjs";

export default scenario("move_predicts_before_authoritative_echo", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "snapshot-delay-300",
    tickMs: 10,
    snapshotLatencyTicks: 300,
    seed: 45,
  },
  steps: [
    waitForClientPredictionReady(),
    capture("before-move"),
    selectOwn("scout_car", 0),
    setClientSnapshotDelivery(false),
    issue("move", { dx: 192, dy: 0 }),
    waitMs(1500),
    advanceClientPredictionVisual(),
    capture("after-predicted-move"),
    assertClientAuthoritativeOwnedStable({ before: "before-move", after: "after-predicted-move", unit: "scout_car" }),
    assertClientRenderedOwnedAdvanced({ before: "before-move", after: "after-predicted-move", unit: "scout_car", minDistancePx: 1 }),
  ],
});
