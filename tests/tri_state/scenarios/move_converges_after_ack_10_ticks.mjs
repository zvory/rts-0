import {
  assertClientCorrectionBudget,
  assertClientPrediction,
  assertClientRenderedConverged,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
  waitForSnapshot,
} from "../dsl.mjs";

export default scenario("move_converges_after_ack_10_ticks", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "snapshot-delay-10",
    tickMs: 10,
    snapshotLatencyTicks: 10,
    seed: 4510,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issue("move", { dx: 192, dy: 0 }),
    waitForAck(1, { timeoutMs: 8000 }),
    waitForSnapshot({ minTickDelta: 3, timeoutMs: 8000 }),
    capture("after-ack"),
    assertClientPrediction({ pendingClientSeqs: [], latestAckSeq: 1, minAcknowledgedCount: 1 }),
    assertClientRenderedConverged({ unit: "worker", tolerancePx: 8 }),
    assertClientCorrectionBudget({ maxPx: 128, maxSnapCorrections: 1 }),
  ],
});
