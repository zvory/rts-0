import {
  assertClientCorrectionBudget,
  assertLocalBaselineOwnerSafe,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
} from "../dsl.mjs";

export default scenario("hidden_blocker_correction_no_leak", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "hidden-state-correction-guard",
    tickMs: 10,
    snapshotLatencyTicks: 10,
    snapshotJitterTicks: 5,
    seed: 4515,
  },
  steps: [
    waitForClientPredictionReady(),
    capture("owner-safe-baseline"),
    assertLocalBaselineOwnerSafe(),
    selectOwn("worker", 0),
    issue("move", { dx: 192, dy: 96 }),
    waitForAck(1, { timeoutMs: 10000 }),
    capture("after-authoritative-correction"),
    assertClientCorrectionBudget({ maxPx: 192, maxSnapCorrections: 2 }),
  ],
});
